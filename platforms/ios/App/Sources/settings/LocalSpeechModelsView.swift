import SwiftUI

/// The on-device model catalog on the voice page: what can be downloaded, what is installed, and which model `voice_input.asr_model_path` names. Downloads, verification and removal go through the shared host-api, as on the desktops.
@MainActor
final class LocalSpeechModelManager: ObservableObject {
  @Published private(set) var models: [LocalSpeechModelInfo] = []
  @Published private(set) var progress: [String: LocalSpeechInstallProgress] = [:]
  @Published private(set) var installing: Set<String> = []
  @Published private(set) var selectedPath = ""
  @Published private(set) var savedMirror = ""
  @Published var mirror = ""
  @Published var status = ""
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
    if wasSelected { writeSelection("") }
    LocalSpeechEngine.shared.release()
    let id = model.id
    Task {
      do {
        try await Task.detached { try LocalSpeechModelStore.remove(root: root, id: id) }.value
        status = "已删除“\(model.title)”。"
      } catch { status = error.localizedDescription }
      refresh()
    }
  }

  /// `voice_input.asr_model_mirror`: empty for the catalog's own URLs, otherwise an https prefix put before each download URL.
  func saveMirror() {
    let value = mirror.trimmingCharacters(in: .whitespacesAndNewlines)
    guard Self.isValidMirror(value) else {
      status = LocalSpeechModelStore.message(for: "local_model_invalid_mirror")
      return
    }
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { preferences in
      var voice = preferences["voice_input"] as? [String: Any] ?? [:]
      voice["asr_model_mirror"] = value
      preferences["voice_input"] = voice
    }
    guard written else { status = "未能保存镜像地址，请重试。"; return }
    savedMirror = value
    mirror = value
    status = value.isEmpty ? "已改回直接从 GitHub 下载。" : "镜像地址已保存。"
  }

  static func isValidMirror(_ value: String) -> Bool {
    if value.isEmpty { return true }
    return CustomServiceConfiguration.validatedEndpoint(value, maximumCharacters: 2048) != nil
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
        if manager.mirror.trimmingCharacters(in: .whitespacesAndNewlines) != manager.savedMirror {
          Button("保存镜像地址") { manager.saveMirror() }
            .buttonStyle(.borderless)
            .accessibilityIdentifier("saveLocalModelMirror")
        }
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
    .onAppear { manager.refresh() }
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
            .font(.caption.weight(.semibold)).foregroundStyle(Color(uiColor: MetasequoiaTheme.forestUIColor))
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
          Button("取消") { manager.cancel(model) }.buttonStyle(.borderless)
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
      Button("下载（\(Self.bytes(model.archiveSize))）") { manager.install(model) }.buttonStyle(.borderless)
        .accessibilityIdentifier("downloadLocalModel_\(model.id)")
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
    guard let update else { return "准备下载…" }
    switch update.stage {
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
