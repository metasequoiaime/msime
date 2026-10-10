import SwiftUI
import UniformTypeIdentifiers

struct CloudDictionaryFilesView: View {
  let accountID: String
  let kind: BackendAccountClient.DictionaryKind
  let session: BackendAccountSession
  let imported: () async throws -> Void
  @State private var format = BackendAccountClient.DictionaryFileFormat.standard
  @State private var choosing = false
  @State private var confirming = false
  @State private var choosingSnapshot = false
  @State private var confirmingRestore = false
  @State private var preparedSnapshot: BackendPreparedSnapshot?
  @State private var restoreRevision: Int64?
  @State private var text: String?
  @State private var fileName = ""
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  @State private var exported: Export?
  @State private var sessionID: UUID?
  private let client = BackendAccountClient()
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
        SettingsGroupHeader(title: "导入到 \(kind.title)")
      } footer: {
        Text(format == .hans
             ? "汉字格式由服务器调用输入引擎注音，请导入后检查多音字读音；默认权重为 100000。每次最多 500 条，包含 JSON 转义后的请求不超过 64 KiB。选择文件只在本机预览；确认上传后才写入云端，重复或无效词条会让整批导入失败，原有云端词条保持不变。"
             : "每次最多 500 条，包含 JSON 转义后的请求不超过 64 KiB。选择文件只在本机预览；确认上传后才写入云端，重复或无效词条会让整批导入失败，原有云端词条保持不变。")
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
                          enabled: format != .hans) { run { try await export() } }
      } header: {
        SettingsGroupHeader(title: "导出云端个人词条")
      } footer: {
        Text(format == .windows
          ? "按 Windows 规则导出：拼音仅多字词，其他类型仅用户添加的词条。文件通过系统分享面板保存到你选择的位置。"
          : "导出所选类型的全部云端个人词条，包括搜索结果之外的词条；文件通过系统分享面板保存到你选择的位置。")
      }
      .designRow()
      Section {
        SettingsActionRow(title: "导出完整快照", detail: "四类词库加排序记录，打包成一个文件",
                          symbol: "archivebox.fill") { run { try await exportSnapshot() } }
        SettingsActionRow(title: "选择快照恢复到云端", detail: "先校验再确认，不会直接写",
                          symbol: "arrow.uturn.backward.circle.fill") {
          choosingSnapshot = true; choosing = true
        }
        if let snapshot = preparedSnapshot, restoreRevision != nil {
          SettingsFactRow(title: "已校验这份快照",
                          detail: "\(snapshot.envelope.entries) 个词条 · \(snapshot.envelope.overlays) 条覆盖 · \(snapshot.envelope.positions) 个固定位置",
                          symbol: "checkmark.seal.fill")
          SettingsActionRow(title: "恢复此快照到云端", detail: "替换全部四类云词库及排序记录",
                            symbol: "arrow.left.arrow.right", destructive: true) { confirmingRestore = true }
          SettingsActionRow(title: "取消恢复", symbol: "xmark.circle.fill") {
            preparedSnapshot = nil; restoreRevision = nil
          }
        }
      } header: {
        SettingsGroupHeader(title: "完整云词库备份")
      } footer: {
        Text(preparedSnapshot == nil
             ? "包含全部四类词库以及删除、调频和固定位置记录。导出不会改变本机词库。"
             : "恢复只写云端，本机词库需另行下载更新。")
      }
      .designRow()
    }
    .designPage()
    .settingsStatus(busy: busy, busyTitle: "正在传输…", message: message)
    .disabled(busy)
    .navigationTitle("词库文件")
    .onDisappear { pending?.cancel(); text = nil; preparedSnapshot = nil; restoreRevision = nil }
    .fileImporter(isPresented: $choosing, allowedContentTypes: choosingSnapshot ? [.data] : [.plainText, .tabSeparatedText]) { result in
      if choosingSnapshot {
        preparedSnapshot = nil; restoreRevision = nil
        do {
          let url = try result.get()
          run {
            let prepared = try await BackendPreparedSnapshot.prepareDocument(url)
            let current = try await client.dictionaryCatalog(.quick, code: "", session: session,
              matchingUserID: accountID, matchingSessionID: sessionID)
            preparedSnapshot = prepared; restoreRevision = current.revision
          }
        } catch { message = "未选择可读取的快照文件。" }
        return
      }
      text = nil; message = nil
      do {
        let url = try result.get()
        run {
          let content = try await Task.detached(priority: .userInitiated) {
            try Self.readTextFile(from: url)
          }.value
          text = content; fileName = url.lastPathComponent
        }
      } catch { message = error.localizedDescription }
    }
    .alert("上传词库文件？", isPresented: $confirming) {
      Button("取消", role: .cancel) { }
      Button("上传") {
        guard let text else { return }
        run {
          let result = try await client.importDictionary(kind, text: text, format: format, session: session,
            matchingUserID: accountID, matchingSessionID: sessionID)
          self.text = nil
          message = "云端已导入 \(result.imported) 条；需要下载到本机后才会影响本机输入。"
          try await imported()
        }
      }
    } message: { Text("将按“\(format.title)”导入 \(kind.title)词库，重复或无效词条会让整批导入失败。") }
    .alert("替换全部云词库？", isPresented: $confirmingRestore) {
      Button("取消", role: .cancel) { }
      Button("替换云词库", role: .destructive) {
        guard let snapshot = preparedSnapshot, let revision = restoreRevision else { return }
        run {
          let result = try await client.restoreDictionarySnapshot(file: snapshot.url,
            expectedSHA256: snapshot.envelope.sha256, revision: revision, session: session,
            matchingUserID: accountID, matchingSessionID: sessionID)
          preparedSnapshot = nil; restoreRevision = nil
          message = "云词库已恢复，版本 \(result.revision)。本机词库尚未更新。"
          try await imported()
        }
      }
    } message: {
      Text("将删除快照之外的云端词条，并恢复快照中的删除、调频和固定位置记录。建议先导出当前云词库备份；如果其他设备已修改云词库，本次恢复会被拒绝。")
    }
    .sheet(item: $exported) { item in
      CloudDictionaryShareView(url: item.url)
        .onDisappear { try? FileManager.default.removeItem(at: item.url.deletingLastPathComponent()) }
    }
  }
  private struct InvalidTextFile: LocalizedError, Sendable {
    var errorDescription: String? { "无法读取文件，请确认是大小不超过 64 KiB 的 UTF-8 文本。" }
  }

  private static func readTextFile(from url: URL) throws -> String {
    let content = try PersonalDictionaryImport.readText(from: url)
    guard content.utf8.count <= 65536, !content.contains("\0") else { throw InvalidTextFile() }
    return content
  }

  @MainActor private func exportSnapshot() async throws {
    let snapshot = try await client.dictionarySnapshot(session: session, matchingUserID: accountID,
                                                       matchingSessionID: sessionID)
    do { try Task.checkCancellation() }
    catch { try? FileManager.default.removeItem(at: snapshot.url.deletingLastPathComponent()); throw error }
    message = "备份文件摘要已校验：云端版本 \(snapshot.envelope.revision)，共 \(snapshot.envelope.records) 条记录。"
    exported = Export(url: snapshot.url)
  }
  @MainActor private func export() async throws {
    let url = try await client.exportDictionary(kind, format: format, session: session,
      matchingUserID: accountID, matchingSessionID: sessionID)
    do { try Task.checkCancellation() }
    catch { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()); throw error }
    exported = Export(url: url)
  }
  @MainActor private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do {
        let identity = try await session.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
        sessionID = identity.sessionID
        try await action()
      }
      catch is CancellationError { }
      catch { message = error.localizedDescription }
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
