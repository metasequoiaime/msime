import SwiftUI
import UniformTypeIdentifiers

private struct PersonalDictionaryDocument: FileDocument {
  static var readableContentTypes: [UTType] { [.json] }
  var data: Data
  init() throws { data = try PersonalDictionaryImport.example.encoded() }
  init(configuration: ReadConfiguration) throws {
    data = configuration.file.regularFileContents ?? Data()
  }
  func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
    FileWrapper(regularFileWithContents: data)
  }
}

struct PersonalDictionaryImportView: View {
  /// Windows imports a JSON-free word list as its own action (“导入纯中文”), and a word-first or code-first dictionary file per kind; here each is a source of the same preview, so every import is confirmed the same way.
  private enum Source: Hashable { case json, hans, file }
  let save: ([PersonalWord]) throws -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var choosing = false
  @State private var exporting = false
  @State private var document: PersonalDictionaryDocument?
  @State private var preview: PersonalDictionaryImport?
  @State private var fileName = ""
  @State private var error: String?
  @State private var readTask: Task<Void, Never>?
  @State private var loading = false
  @State private var source = Source.json
  @State private var hansText = ""
  @State private var choosingText = false
  @State private var fileKind = PersonalWordKind.pinyin
  @State private var fileFormat = "standard"
  @State private var choosingDictionary = false
  @State private var notice = ""

  var body: some View {
    NavigationView {
      List {
        Section {
          Picker("导入来源", selection: $source) {
            Text("JSON 文件").tag(Source.json)
            Text("纯中文词表").tag(Source.hans)
            Text("词库文件").tag(Source.file)
          }
          .pickerStyle(.segmented)
          .accessibilityIdentifier("personalDictionaryImportSource")
          .onChange(of: source) { _ in clearPreview() }
        }
        .designRow()
        if source == .json {
          Section {
            Button("选择 JSON 文件") { choosing = true }.disabled(loading)
              .accessibilityIdentifier("choosePersonalDictionaryFile")
            Button("保存示例文件") {
              do { document = try PersonalDictionaryDocument(); exporting = true }
              catch { self.error = error.localizedDescription }
            }
          } footer: {
            Text("支持拼音、五笔、英文和快捷短语，每次最多 128 条、文件不超过 1 MB。请按示例填写；不支持其他输入法的专有词库文件。")
          }
          .designRow()
        } else if source == .file {
          Section {
            Picker("词库类型", selection: $fileKind) {
              ForEach(PersonalWordKind.allCases) { Text($0.title).tag($0) }
            }
            .accessibilityIdentifier("personalDictionaryFileKind")
            .onChange(of: fileKind) { _ in clearPreview() }
            Picker("文件格式", selection: $fileFormat) {
              ForEach(PersonalDictionaryImport.fileFormats, id: \.format) { Text($0.title).tag($0.format) }
            }
            .accessibilityIdentifier("personalDictionaryFileFormat")
            .onChange(of: fileFormat) { _ in clearPreview() }
            Button("选择词库文件") { choosingDictionary = true }.disabled(loading)
              .accessibilityIdentifier("choosePersonalDictionaryDictionaryFile")
          } footer: {
            Text("与电脑版设置页导入的是同一种文件：每行一条，用 Tab 分隔词、编码和可选的权重。标准格式词在前，Windows 格式编码在前；两列放反时会按文件本身的顺序读取。Rime 格式读取 userdb 导出或 dict.yaml 的词条部分。无法导入的行会被跳过并在预览里说明，每次最多导入 128 条，文件不超过 1 MB，需要 UTF-8 编码。")
          }
          .designRow()
        } else {
          Section {
            TextEditor(text: $hansText)
              .frame(minHeight: 120)
              .accessibilityIdentifier("personalDictionaryHansText")
            Button("选择文本文件") { choosingText = true }.disabled(loading)
              .accessibilityIdentifier("choosePersonalDictionaryText")
            Button("生成拼音预览") { let text = hansText; annotate(name: "纯中文词表") { text } }
              .disabled(loading || hansText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
              .accessibilityIdentifier("annotatePersonalDictionaryHans")
          } footer: {
            Text("每行一个词语，只含汉字；以 # 开头的行会被跳过。拼音由本机词典自动标注，多音字可能需要在导入后逐条修改。每次最多 128 个词语，导入为拼音词条。")
          }
          .designRow()
        }
        if loading { Section { ProgressView("正在读取并校验词库…") }.designRow() }
        if let preview {
          Section {
            Text(fileName).font(.headline)
            Text(source == .hans
                 ? "已为 \(preview.entries.count) 个词语标注拼音，请确认读音。"
                 : "已校验 \(preview.entries.count) 条，请确认内容。编码已按输入引擎规范化。")
              .font(.footnote).foregroundStyle(.secondary)
            if !notice.isEmpty {
              Text(notice).font(.footnote).foregroundStyle(.orange)
                .accessibilityIdentifier("personalDictionaryImportNotice")
            }
            ForEach(PersonalWordKind.allCases) { kind in
              let count = preview.entries.filter { $0.kind == kind }.count
              if count > 0 { LabeledContentCompat(title: kind.title, value: "\(count) 条") }
            }
          }
          .designRow()
          Section {
            ForEach(preview.entries) { word in
              VStack(alignment: .leading, spacing: 4) {
                Text(word.value).lineLimit(3)
                Text("\(word.kind.title) · \(word.key)").font(.caption).foregroundStyle(.secondary)
              }
            }
          } header: {
            SettingsGroupHeader(title: "词条预览")
          }
          .designRow()
        }
        Section {
          Text("确认后加入本机同步队列，打开水杉键盘后逐条生效。同步失败的词条可单独重试；相同类型、编码和内容的已有词条将更新权重。文件内容不会上传。")
            .font(.footnote).foregroundStyle(.secondary)
        }
        .designRow()
      }
      .designPage()
      .navigationTitle("导入个人词库")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("确认导入") {
            guard let preview else { return }
            do { try save(preview.entries); dismiss() } catch { self.error = error.localizedDescription }
          }.disabled(preview == nil || loading).accessibilityIdentifier("confirmPersonalDictionaryImport")
        }
      }
      .fileImporter(isPresented: $choosing, allowedContentTypes: [.json]) { result in
        // Invalidate an earlier preview before attempting to load a replacement file.
        do {
          let url = try result.get()
          clearPreview()
          loading = true
          readTask = Task { @MainActor in
            do {
              let imported = try await Task.detached(priority: .userInitiated) {
                try PersonalDictionaryImport.read(from: url)
              }.value
              guard !Task.isCancelled else { return }
              preview = imported
              fileName = url.lastPathComponent
            } catch {
              guard !Task.isCancelled else { return }
              self.error = error.localizedDescription
            }
            loading = false
          }
        } catch { self.error = error.localizedDescription }
      }
      .background(EmptyView().fileImporter(isPresented: $choosingText, allowedContentTypes: [.plainText]) { result in
        do {
          let url = try result.get()
          annotate(name: url.lastPathComponent) { try PersonalDictionaryImport.readText(from: url) }
        } catch { self.error = error.localizedDescription }
      })
      .background(EmptyView().fileImporter(isPresented: $choosingDictionary, allowedContentTypes: [.text]) { result in
        do {
          let url = try result.get()
          readDictionary(url)
        } catch { self.error = error.localizedDescription }
      })
      .fileExporter(isPresented: $exporting, document: document, contentType: .json,
                    defaultFilename: "msime-personal-dictionary-example") { result in
        if case .failure(let error) = result { self.error = error.localizedDescription }
      }
      .alert("导入个人词库", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
        Button("好", role: .cancel) { error = nil }
      } message: { Text(error ?? "") }
      .onDisappear { readTask?.cancel(); readTask = nil }
    }
  }

  private func clearPreview() {
    readTask?.cancel()
    preview = nil
    notice = ""
    loading = false
  }

  /// The Engine parses the file against the packaged dictionary, so like annotation it runs off the main thread.
  private func readDictionary(_ url: URL) {
    clearPreview()
    loading = true
    let kind = fileKind, format = fileFormat
    readTask = Task { @MainActor in
      do {
        let imported = try await Task.detached(priority: .userInitiated) {
          try PersonalDictionaryImport.file(PersonalDictionaryImport.readText(from: url), kind: kind, format: format)
        }.value
        guard !Task.isCancelled else { return }
        preview = imported.file
        notice = imported.notice
        fileName = url.lastPathComponent
      } catch {
        guard !Task.isCancelled else { return }
        self.error = error.localizedDescription
      }
      loading = false
    }
  }

  /// Annotation opens the packaged dictionary and walks it once per word, so it runs off the main thread like a file read.
  private func annotate(name: String, read: @escaping @Sendable () throws -> String) {
    clearPreview()
    loading = true
    readTask = Task { @MainActor in
      do {
        let imported = try await Task.detached(priority: .userInitiated) {
          try PersonalDictionaryImport.hans(read())
        }.value
        guard !Task.isCancelled else { return }
        preview = imported
        fileName = name
      } catch {
        guard !Task.isCancelled else { return }
        self.error = error.localizedDescription
      }
      loading = false
    }
  }
}

private struct LabeledContentCompat: View {
  let title: String
  let value: String
  var body: some View {
    HStack { Text(title); Spacer(); Text(value).foregroundStyle(.secondary) }
  }
}
