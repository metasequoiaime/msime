import SwiftUI
import UniformTypeIdentifiers

/// 词库文件：导入、导出这一类词条，以及整份云词库的备份和恢复，都经 Rust 的 `msime_client_cloud_dictionary`（`import`、`export_file`、`snapshot_export_file`、`snapshot_restore_preview_file`、`snapshot_restore_file`，#8201 迁移计划 4c）。导出和备份由 Rust 下载、整份校验后写进本 App 的私有目录，再交给分享面板。
struct CloudDictionaryFilesView: View {
  let accountID: String
  let kind: BackendAccountClient.DictionaryKind
  let imported: () async throws -> Void
  @State private var format = BackendAccountClient.DictionaryFileFormat.standard
  @State private var choosing = false
  @State private var confirming = false
  @State private var choosingSnapshot = false
  @State private var confirmingRestore = false
  /// 用户选的备份复制进本 App 私有目录的那一份，以及 Rust 校验后给出的元数据；恢复时 Rust 再核对一次摘要。
  @State private var preparedSnapshot: (file: URL, snapshot: IOSCloudDictionary.Snapshot)?
  @State private var restoreRevision: Int64?
  @State private var text: String?
  @State private var fileName = ""
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  @State private var exported: Export?
  @State private var request = ViewRequestGeneration()
  private var dictionary: IOSCloudDictionary { IOSCloudDictionary(accountID: accountID) }
  private struct Export: Identifiable { let id = UUID(); let url: URL }
  private var lines: [String] { (text ?? "").components(separatedBy: .newlines).filter { !$0.isEmpty } }
  /// 每一行长什么样,取决于格式和词库类型 —— 这是选文件之前唯一需要知道的事,所以跟着那个动作走。
  private var rowFormatHint: String {
    if format == .hans { return "每行一个汉字词条，由服务器注音" }
    return format == .windows && (kind == .english || kind == .quick)
      ? "每行：编码、词条、权重，制表符分隔"
      : "每行：词条、编码、权重，制表符分隔"
  }

  var body: some View {
    List {
      Section {
        Picker("文件格式", selection: $format) {
          ForEach(BackendAccountClient.DictionaryFileFormat.allCases.filter { kind == .pinyin || $0 != .hans }) {
            Text($0.title).tag($0)
          }
        }
        SettingsActionRow(title: "选择 UTF-8 文本文件", detail: rowFormatHint,
                          symbol: "doc.text.fill") { choosingSnapshot = false; choosing = true }
      } header: {
        DesignSectionHeader(info: format == .hans
          ? "汉字格式由服务器调用输入引擎注音，请导入后检查多音字读音；默认权重为 100000。每次最多 500 条，包含 JSON 转义后的请求不超过 64 KiB。选择文件只在本机预览；确认上传后才写入云端，重复或无效词条会让整批导入失败，原有云端词条保持不变。"
          : "每次最多 500 条，包含 JSON 转义后的请求不超过 64 KiB。选择文件只在本机预览；确认上传后才写入云端，重复或无效词条会让整批导入失败，原有云端词条保持不变。") { SettingsGroupHeader(title: "导入到 \(kind.title)") }
      }
      .designRow()
      if let text {
        Section {
          SettingsFactRow(title: fileName, detail: "\(lines.count) 行 · \(text.utf8.count) 字节",
                          symbol: "doc.fill")
          ForEach(Array(lines.prefix(12).enumerated()), id: \.offset) { _, line in Text(line).font(.caption).lineLimit(3) }
          SettingsActionRow(title: "确认上传到云端", symbol: "icloud.and.arrow.up.fill") { confirming = true }
        } header: {
          SettingsGroupHeader(title: "导入预览")
        } footer: {
          if lines.count > 12 { Text("仅显示前 12 行。") }
        }
        .designRow()
      }
      Section {
        SettingsActionRow(title: "导出文件", detail: "按上面选的格式导出这一类",
                          symbol: "square.and.arrow.up.fill",
                          enabled: format != .hans) { run { id in try await export(id: id) } }
      } header: {
        DesignSectionHeader(info: format == .windows
          ? "按 Windows 规则导出：拼音仅多字词，其他类型仅用户添加的词条。文件通过系统分享面板保存到你选择的位置。"
          : "导出所选类型的全部云端个人词条，包括搜索结果之外的词条；文件通过系统分享面板保存到你选择的位置。") { SettingsGroupHeader(title: "导出云端个人词条") }
      }
      .designRow()
      Section {
        SettingsActionRow(title: "导出完整快照", detail: "四类词库加排序记录，打包成一个文件",
                          symbol: "archivebox.fill") { run { id in try await exportSnapshot(id: id) } }
        SettingsActionRow(title: "选择快照恢复到云端", detail: "先校验再确认，不会直接写",
                          symbol: "arrow.uturn.backward.circle.fill") {
          choosingSnapshot = true; choosing = true
        }
        if let snapshot = preparedSnapshot?.snapshot, restoreRevision != nil {
          SettingsFactRow(title: "已校验这份快照",
                          detail: "\(snapshot.entries) 个词条 · \(snapshot.overlays) 条覆盖 · \(snapshot.positions) 个固定位置",
                          symbol: "checkmark.seal.fill")
          SettingsActionRow(title: "恢复此快照到云端", detail: "替换全部四类云词库及排序记录",
                            symbol: "arrow.left.arrow.right", destructive: true) { confirmingRestore = true }
          SettingsActionRow(title: "取消恢复", symbol: "xmark.circle.fill") {
            discardPrepared(); restoreRevision = nil
          }
        }
      } header: {
        DesignSectionHeader(info: "包含全部四类词库以及删除、调频和固定位置记录。导出不会改变本机词库。") {
          SettingsGroupHeader(title: "完整云词库备份")
        }
      } footer: {
        // 准备恢复时这句讲的是「恢复」按钮的后果，紧挨着按钮留在原处。
        if preparedSnapshot != nil { Text("恢复只写云端，本机词库需另行下载更新。") }
      }
      .designRow()
    }
    .designPage()
    .settingsStatus(busy: busy, busyTitle: "正在传输…", message: message)
    .disabled(busy)
    .navigationTitle("词库文件")
    .trackViewRequest(request: $request, pending: $pending, onDisappear: {
      busy = false
      text = nil
      discardPrepared()
      restoreRevision = nil
    })
    .fileImporter(isPresented: $choosing, allowedContentTypes: choosingSnapshot ? [.data] : [.plainText, .tabSeparatedText]) { result in
      guard request.isVisible else { return }
      if choosingSnapshot {
        discardPrepared(); restoreRevision = nil
        do {
          let url = try result.get()
          run { id in
            // 选中的文件只在这一刻能读（安全作用域），先复制一份到私有目录：预览和恢复都读这一份，恢复时 Rust 核对它与预览时相同。
            let copy = try await Self.copyChosenFile(url)
            do {
              let preview = try await dictionary.object(["operation": "snapshot_restore_preview_file", "path": copy.path])
              let snapshot = try IOSCloudDictionary.Snapshot(preview["snapshot"])
              guard let revision = (preview["expectedRevision"] as? NSNumber)?.int64Value else { throw BackendRustClient.Failure.invalidResponse }
              try request.checkCurrent(id)
              preparedSnapshot = (copy, snapshot); restoreRevision = revision
            } catch {
              try? FileManager.default.removeItem(at: copy.deletingLastPathComponent())
              throw error
            }
          }
        } catch { message = "未选择可读取的快照文件。" }
        return
      }
      text = nil; message = nil
      do {
        let url = try result.get()
        run { id in
          let content = try await Task.detached(priority: .userInitiated) {
            try Self.readTextFile(from: url)
          }.value
          try request.checkCurrent(id)
          text = content; fileName = url.lastPathComponent
        }
      } catch { message = error.localizedDescription }
    }
    .alert("上传词库文件？", isPresented: $confirming) {
      Button("取消", role: .cancel) { }
      Button("上传") {
        guard let text else { return }
        run { id in
          let result = try await dictionary.object(["operation": "import", "kind": kind.rawValue, "format": format.rawValue, "text": text])
          try request.checkCurrent(id)
          self.text = nil
          message = "云端已导入 \((result["imported"] as? NSNumber)?.intValue ?? 0) 条；需要下载到本机后才会影响本机输入。"
          try await imported()
        }
      }
    } message: { Text("将按“\(format.title)”导入 \(kind.title)词库，重复或无效词条会让整批导入失败。") }
    .alert("替换全部云词库？", isPresented: $confirmingRestore) {
      Button("取消", role: .cancel) { }
      Button("替换云词库", role: .destructive) {
        guard let prepared = preparedSnapshot, let revision = restoreRevision else { return }
        run { id in
          let result = try await dictionary.object(["operation": "snapshot_restore_file", "path": prepared.file.path,
                                                    "expected_sha256": prepared.snapshot.sha256, "revision": revision])
          try request.checkCurrent(id)
          discardPrepared(); restoreRevision = nil
          message = "云词库已恢复，版本 \((result["revision"] as? NSNumber)?.int64Value ?? 0)。本机词库尚未更新。"
          try await imported()
        }
      }
    } message: {
      Text("将删除快照之外的云端词条，并恢复快照中的删除、调频和固定位置记录。建议先导出当前云词库备份；如果其他设备已修改云词库，本次恢复会被拒绝。")
    }
    .sheet(item: $exported) { item in
      CloudDictionaryShareView(url: item.url)
        .onDisappear { try? FileManager.default.removeItem(at: item.url.deletingLastPathComponent()) }
        // 系统分享面板本来就从半屏弹起；包在 sheet 里时不设高度就被撑成整屏，上半截空着。
        .presentationDetents([.medium, .large])
    }
  }
  private struct InvalidTextFile: LocalizedError, Sendable {
    var errorDescription: String? { "无法读取文件，请确认是大小不超过 64 KiB 的 UTF-8 文本。" }
  }

  nonisolated private static func readTextFile(from url: URL) throws -> String {
    let content = try PersonalDictionaryImport.readText(from: url)
    guard content.utf8.count <= 65536, !content.contains("\0") else { throw InvalidTextFile() }
    return content
  }

  @MainActor private func exportSnapshot(id: UUID) async throws {
    let file = try IOSCloudDictionary.privateFile("msime-dictionary-snapshot.ndjson")
    let snapshot: IOSCloudDictionary.Snapshot
    do {
      let result = try await dictionary.object(["operation": "snapshot_export_file", "path": file.path])
      snapshot = try IOSCloudDictionary.Snapshot(result["snapshot"])
      try request.checkCurrent(id)
    } catch { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()); throw error }
    message = "备份文件摘要已校验：云端版本 \(snapshot.cloudRevision)，共 \(snapshot.records) 条记录。"
    exported = Export(url: file)
  }
  @MainActor private func export(id: UUID) async throws {
    let file = try IOSCloudDictionary.privateFile("dictionary-" + kind.rawValue + ".tsv")
    do {
      _ = try await dictionary.request(["operation": "export_file", "kind": kind.rawValue, "format": format.rawValue, "path": file.path])
      try request.checkCurrent(id)
    } catch { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()); throw error }
    exported = Export(url: file)
  }

  /// 删掉复制进来、还没恢复的备份。
  @MainActor private func discardPrepared() {
    if let file = preparedSnapshot?.file { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
    preparedSnapshot = nil
  }

  /// 把用户选的备份复制进私有目录，最多 512 MiB；复制在后台线程上做。
  nonisolated private static func copyChosenFile(_ source: URL) async throws -> URL {
    try await Task.detached(priority: .userInitiated) {
      let scoped = source.startAccessingSecurityScopedResource()
      defer { if scoped { source.stopAccessingSecurityScopedResource() } }
      let size = (try source.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey]))
      guard size.isRegularFile == true, let bytes = size.fileSize, bytes > 0, bytes <= 512 * 1024 * 1024 else {
        throw CocoaError(.fileReadCorruptFile)
      }
      let copy = try IOSCloudDictionary.privateFile("restore.ndjson")
      do { try FileManager.default.copyItem(at: source, to: copy) }
      catch { try? FileManager.default.removeItem(at: copy.deletingLastPathComponent()); throw error }
      return copy
    }.value
  }

  @MainActor private func run(_ action: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending) { id in
      try await action(id)
    }
  }
}

private struct CloudDictionaryShareView: UIViewControllerRepresentable {
  let url: URL
  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: [url], applicationActivities: nil)
  }
  func updateUIViewController(_ controller: UIActivityViewController, context: Context) { }
}
