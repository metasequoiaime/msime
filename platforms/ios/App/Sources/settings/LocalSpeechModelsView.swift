import SwiftUI
import UniformTypeIdentifiers

/// The on-device model catalog on the voice page: what can be downloaded, what is installed, and which model `voice_input.asr_model_path` names. Downloads, verification and removal go through the shared host-api, as on the desktops.
@MainActor
final class LocalSpeechModelManager: ObservableObject {
  @Published private(set) var models: [LocalSpeechModelInfo] = []
  @Published private(set) var progress: [String: LocalSpeechInstallProgress] = [:]
  @Published private(set) var installing: Set<String> = []
  /// 正在从本地文件安装的模型，是 `installing` 的子集。
  @Published private(set) var importing: Set<String> = []
  /// 「从文件导入」的文件选择器是否开着，以及它在为哪个模型选文件。选择器关掉时只清前者：用户取消时不会有回调。
  @Published var importPickerShown = false
  @Published private(set) var importTarget: LocalSpeechModelInfo?

  func chooseImportFiles(for model: LocalSpeechModelInfo) {
    importTarget = model
    importPickerShown = true
  }
  @Published private(set) var selectedPath = ""
  @Published private(set) var savedMirror = ""
  @Published var mirror = ""
  @Published var status = ""
  /// Saves the mirror address as it is typed.
  let mirrorAutosave = SettingsAutosave()
  /// Created the first time the model list is shown, so the AI page, which shares the view type, never makes the directory.
  private(set) lazy var root: URL? = {
    do { return try LocalSpeechModelLocation.root() }
    catch { status = "无法创建模型目录：\(error.localizedDescription)"; return nil }
  }()

  init() {
    let voice = MetasequoiaInputSessionBridge.loadSharedPreferences()?["voice_input"] as? [String: Any] ?? [:]
    selectedPath = voice["asr_model_path"] as? String ?? ""
    savedMirror = voice["asr_model_mirror"] as? String ?? ""
    mirror = savedMirror
  }

  /// The installed model dictation will use, found again under the current container if iOS moved it.
  var selectedDirectory: URL? { LocalSpeechModelLocation.resolve(storedPath: selectedPath, root: root) }

  func isSelected(_ model: LocalSpeechModelInfo) -> Bool {
    model.installed && LocalSpeechModelLocation.names(selectedPath, model: model.id)
  }

  func refresh() {
    guard let root else { return }
    Task {
      do {
        let catalog = try await Task.detached(priority: .userInitiated) { try LocalSpeechModelStore.catalog(root: root) }.value
        // FunASR-nano needs more memory than an iOS app is given.
        models = catalog.models.filter { !$0.desktopOnly }
        // A stored path from before an app update points into the old container; keep it pointing at the same model.
        if let directory = selectedDirectory, directory.path != selectedPath { writeSelection(directory.path) }
      } catch { status = error.localizedDescription }
    }
  }

  func install(_ model: LocalSpeechModelInfo) {
    guard let root, !installing.contains(model.id) else { return }
    // A mirror typed just before tapping 下载 is used, not the one saved before it.
    mirrorAutosave.flush()
    let mirror = savedMirror
    installing.insert(model.id)
    progress[model.id] = nil
    status = ""
    Task {
      do {
        let id = model.id
        let path = try await Task.detached(priority: .userInitiated) {
          try LocalSpeechModelStore.install(root: root, id: id, mirror: mirror) { update in
            Task { @MainActor [weak self] in
              if self?.installing.contains(id) == true { self?.progress[id] = update }
            }
          }
        }.value
        installing.remove(model.id)
        progress[model.id] = nil
        // The first model downloaded is the one to use; a later one waits for 使用.
        if selectedDirectory == nil { writeSelection(path.path) }
        status = "已下载“\(model.title)”。"
      } catch {
        installing.remove(model.id)
        progress[model.id] = nil
        status = error.localizedDescription
      }
      refresh()
    }
  }

  /// 用用户在文件选择器里选中的文件安装 `model`，不联网。文件按长度和 SHA-256 对上目录，不看文件名；选择器给的是带安全范围的 URL，整个安装期间都保持访问权。
  func importFiles(_ model: LocalSpeechModelInfo, from urls: [URL]) {
    guard let root, !installing.contains(model.id), !urls.isEmpty else { return }
    installing.insert(model.id)
    importing.insert(model.id)
    progress[model.id] = nil
    status = ""
    Task {
      do {
        let id = model.id
        let path = try await Task.detached(priority: .userInitiated) {
          let accessed = urls.map { $0.startAccessingSecurityScopedResource() }
          defer { for (url, granted) in zip(urls, accessed) where granted { url.stopAccessingSecurityScopedResource() } }
          return try LocalSpeechModelStore.install(root: root, id: id, mirror: "", files: urls) { update in
            Task { @MainActor [weak self] in
              if self?.installing.contains(id) == true { self?.progress[id] = update }
            }
          }
        }.value
        finishImport(model.id)
        if selectedDirectory == nil { writeSelection(path.path) }
        status = "已导入“\(model.title)”。"
      } catch let failure as LocalSpeechModelStore.HostFailure {
        finishImport(model.id)
        status = Self.importMessage(for: failure)
      } catch {
        finishImport(model.id)
        status = error.localizedDescription
      }
      refresh()
    }
  }

  private func finishImport(_ id: String) {
    installing.remove(id)
    importing.remove(id)
    progress[id] = nil
  }

  /// 导入失败的提示：文件对不上时说清楚是所选文件的问题，其余和下载相同。
  static func importMessage(for failure: LocalSpeechModelStore.HostFailure) -> String {
    switch failure.code.split(separator: ":", maxSplits: 1).first.map(String.init) ?? failure.code {
    case "local_model_cancelled": "已取消导入。"
    case "local_model_size_mismatch", "local_model_checksum_mismatch":
      "所选文件和模型目录里的校验值不符，可能没下载完整或版本不对，请重新下载后再导入。"
    default: failure.errorDescription ?? failure.code
    }
  }

  func cancel(_ model: LocalSpeechModelInfo) {
    let id = model.id
    Task.detached { LocalSpeechModelStore.cancel(id: id) }
  }

  func use(_ model: LocalSpeechModelInfo) {
    guard model.installed else { return }
    if writeSelection(model.path) { status = "已选用“\(model.title)”。" }
  }

  func remove(_ model: LocalSpeechModelInfo) {
    guard let root else { return }
    let wasSelected = isSelected(model)
    // Release the recognizer before removing its files, but keep the selection until the
    // removal succeeds. If the host cannot delete the model (for example, storage or I/O
    // failure), the installed model must remain the selected one rather than silently
    // disabling voice input.
    LocalSpeechEngine.shared.release()
    let id = model.id
    Task {
      do {
        try await Task.detached { try LocalSpeechModelStore.remove(root: root, id: id) }.value
        if wasSelected { _ = writeSelection("") }
        status = "已删除“\(model.title)”。"
      } catch { status = error.localizedDescription }
      refresh()
    }
  }

  /// `voice_input.asr_model_mirror`: empty for the catalog's own URLs, otherwise an https prefix put before each download URL. Saved once typing pauses; an address that is not a valid one yet is shown as such and left unsaved.
  func mirrorChanged() {
    let value = mirror.trimmingCharacters(in: .whitespacesAndNewlines)
    guard value != savedMirror || mirrorAutosave.hasPending else { return }
    guard Self.isValidMirror(value) else {
      mirrorAutosave.reject(LocalSpeechModelStore.message(for: "local_model_invalid_mirror"))
      return
    }
    mirrorAutosave.schedule { [weak self] in try self?.saveMirror(value) }
  }

  private func saveMirror(_ value: String) throws {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      var voice = preferences["voice_input"] as? [String: Any] ?? [:]
      voice["asr_model_mirror"] = value
      preferences["voice_input"] = voice
    }
    guard written else { throw ServiceFailure(message: "未能保存镜像地址，请重试。") }
    savedMirror = value
  }

  static func isValidMirror(_ value: String) -> Bool {
    LocalSpeechModelStore.isValidMirror(value)
  }

  @discardableResult
  private func writeSelection(_ path: String) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      var voice = preferences["voice_input"] as? [String: Any] ?? [:]
      voice["asr_model_path"] = path
      preferences["voice_input"] = voice
    }
    if written { selectedPath = path } else { status = "未能保存模型选择，请重试。" }
    return written
  }
}

struct LocalSpeechModelsSection: View {
  @ObservedObject var manager: LocalSpeechModelManager
  let disabled: Bool

  var body: some View {
    Section {
      if manager.models.isEmpty {
        HStack { Text("正在读取模型列表…").foregroundStyle(.secondary); Spacer(); ProgressView() }
      }
      ForEach(manager.models) { model in
        LocalSpeechModelRow(model: model, manager: manager)
      }
      VStack(alignment: .leading, spacing: 8) {
        Text("下载镜像（可选）").font(.caption).foregroundStyle(.secondary)
        TextField("https://镜像地址/", text: $manager.mirror)
          .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
          .accessibilityIdentifier("localModelMirror")
          .onChange(of: manager.mirror) { _, _ in manager.mirrorChanged() }
        SettingsAutosaveStatus(autosave: manager.mirrorAutosave).font(.footnote)
      }.padding(.vertical, 4)
      if !manager.status.isEmpty {
        Text(manager.status).font(.footnote).foregroundStyle(.secondary)
          .accessibilityIdentifier("localModelStatus")
      }
    } header: {
      Text("本地模型")
    } footer: {
      Text("模型在本机运行，录音不会离开设备。下载模型时只连接 GitHub；访问不畅时可以填写镜像地址，它会加在每个下载地址前面。模型较大，建议在 Wi-Fi 下下载。")
    }
    .disabled(disabled)
    .fileImporter(
      isPresented: $manager.importPickerShown,
      allowedContentTypes: [.item],
      allowsMultipleSelection: true
    ) { result in
      guard let model = manager.importTarget else { return }
      // 用户关掉选择器不算失败；选择器自己报错时照实显示。
      switch result {
      case .success(let urls): manager.importFiles(model, from: urls)
      case .failure(let error): manager.status = error.localizedDescription
      }
    }
    .onAppear { manager.refresh() }
    .flushesAutosave(manager.mirrorAutosave)
  }
}

private struct LocalSpeechModelRow: View {
  let model: LocalSpeechModelInfo
  @ObservedObject var manager: LocalSpeechModelManager

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      HStack(spacing: 6) {
        Text(model.title).font(.headline)
        if model.streaming { badge("边说边出字") }
        if model.isDefault { badge("推荐") }
        Spacer()
        if manager.isSelected(model) {
          Label("使用中", systemImage: "checkmark.circle.fill").labelStyle(.titleAndIcon)
            .font(.caption.weight(.semibold)).foregroundStyle(MetasequoiaTheme.accent)
            .accessibilityIdentifier("localModelInUse_\(model.id)")
        }
      }
      if !model.description.isEmpty {
        Text(model.description).font(.subheadline).foregroundStyle(.secondary)
      }
      Text(details).font(.caption).foregroundStyle(.secondary)
      if !model.licenseSPDX.isEmpty || !model.licenseNotice.isEmpty {
        VStack(alignment: .leading, spacing: 2) {
          Text("许可：\(model.licenseSPDX.isEmpty ? "见说明" : model.licenseSPDX)").font(.caption2)
          if !model.licenseNotice.isEmpty { Text(model.licenseNotice).font(.caption2) }
          if !model.licenseTerms.isEmpty { Text(model.licenseTerms).font(.caption2) }
          if let source = URL(string: model.licenseSource), source.scheme == "https" {
            Link("来源：\(source.host ?? model.licenseSource)", destination: source).font(.caption2)
          }
        }.foregroundStyle(.secondary)
      }
      if !model.installed && !model.importFiles.isEmpty && !manager.installing.contains(model.id) {
        VStack(alignment: .leading, spacing: 2) {
          Text("不联网安装：先下载下面的文件，再点“从文件导入”一起选中。文件改过名也能认出。").font(.caption2)
          ForEach(model.importFiles, id: \.self) { file in
            if let url = URL(string: file.url), url.scheme == "https" {
              Link("\(file.name)（\(Self.bytes(file.size))）", destination: url).font(.caption2)
            }
          }
        }.foregroundStyle(.secondary)
      }
      actions
    }
    .padding(.vertical, 4)
    .accessibilityIdentifier("localModel_\(model.id)")
  }

  @ViewBuilder private var actions: some View {
    if manager.installing.contains(model.id) {
      let update = manager.progress[model.id]
      VStack(alignment: .leading, spacing: 4) {
        if let update, update.total > 0 {
          ProgressView(value: Double(min(update.downloaded, update.total)), total: Double(update.total))
        } else {
          ProgressView().frame(maxWidth: .infinity, alignment: .leading)
        }
        HStack {
          Text(stageText(update)).font(.caption).foregroundStyle(.secondary)
          Spacer()
          Button(manager.importing.contains(model.id) ? "取消导入" : "取消") { manager.cancel(model) }.buttonStyle(.borderless)
            .accessibilityIdentifier("cancelLocalModel_\(model.id)")
        }
      }
    } else if model.installed {
      HStack {
        if !manager.isSelected(model) {
          Button("使用") { manager.use(model) }.buttonStyle(.borderless)
            .accessibilityIdentifier("useLocalModel_\(model.id)")
        }
        Spacer()
        Button("删除", role: .destructive) { manager.remove(model) }.buttonStyle(.borderless)
          .accessibilityIdentifier("removeLocalModel_\(model.id)")
      }
    } else {
      HStack {
        Button("下载（\(Self.bytes(model.archiveSize))）") { manager.install(model) }.buttonStyle(.borderless)
          .accessibilityIdentifier("downloadLocalModel_\(model.id)")
        Spacer()
        Button("从文件导入") { manager.chooseImportFiles(for: model) }.buttonStyle(.borderless)
          .accessibilityIdentifier("importLocalModel_\(model.id)")
      }
    }
  }

  private var details: String {
    var parts: [String] = []
    if !model.languages.isEmpty { parts.append(model.languages.joined(separator: "、")) }
    parts.append(model.installed ? "占用 \(Self.bytes(model.installedSize))" : "安装后约 \(Self.bytes(model.installedSize))")
    if model.memory > 0 { parts.append("运行内存约 \(Self.bytes(model.memory))") }
    return parts.joined(separator: " · ")
  }

  private func stageText(_ update: LocalSpeechInstallProgress?) -> String {
    guard let update else { return manager.importing.contains(model.id) ? "准备导入…" : "准备下载…" }
    switch update.stage {
    case "import": return update.total > 0
      ? "正在导入 \(Self.bytes(update.downloaded)) / \(Self.bytes(update.total))" : "正在导入 \(Self.bytes(update.downloaded))"
    case "download": return update.total > 0
      ? "正在下载 \(Self.bytes(update.downloaded)) / \(Self.bytes(update.total))" : "正在下载 \(Self.bytes(update.downloaded))"
    case "verify": return "正在校验…"
    case "extract": return "正在解压…"
    default: return "即将完成…"
    }
  }

  private func badge(_ text: String) -> some View {
    Text(text).font(.caption2.weight(.semibold)).padding(.horizontal, 6).padding(.vertical, 2)
      .background(Color(uiColor: .tertiarySystemFill)).clipShape(Capsule())
  }

  static func bytes(_ value: UInt64) -> String {
    ByteCountFormatter.string(fromByteCount: Int64(clamping: value), countStyle: .file)
  }
}
