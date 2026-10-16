import SwiftUI
import UniformTypeIdentifiers

struct NineKeyShuangpinSettingsView: View {
  @State private var profile = "migala3"
  @State private var draft: NineKeyShuangpinLayout.Table = [:]
  @State private var layout: NineKeyShuangpinLayout?
  @State private var error: String?
  @State private var saved = false
  @State private var importing = false
  @State private var exporting = false
  @State private var exportFile = NineKeyShuangpinDocument(data: Data())

  var body: some View {
    Form {
      Section {
        Picker("方案", selection: Binding(get: { profile }, set: choose)) {
          ForEach(NineKeyShuangpinLayout.profiles, id: \.self) { value in
            Text(NineKeyShuangpinLayout.title(value)).tag(value)
          }
        }.accessibilityIdentifier("nineKeyShuangpinProfile")
        Text("每个音节按声母、韵母输入两位数字。1 和 0 依方案参与组码，分词仍用独立按键。")
          .font(.footnote).foregroundStyle(.secondary)
      }
      Section("键位预览") {
        ForEach(layout?.keys ?? [], id: \.key) { key in
          HStack(alignment: .top) {
            Text(key.key).font(.headline).frame(width: 24)
            Text(key.accessibilityLabel).font(.subheadline)
          }
        }
      }
      if profile == "custom" {
        Section {
          Menu("从内置方案开始") {
            ForEach(NineKeyShuangpinLayout.profiles.filter { $0 != "custom" }, id: \.self) { value in
              Button(NineKeyShuangpinLayout.title(value)) { seed(value) }
            }
          }
          Button("导入 JSON 码表") { importing = true }.accessibilityIdentifier("nineKeyShuangpinImport")
          Button("导出 JSON 码表") { prepareExport() }.accessibilityIdentifier("nineKeyShuangpinExport")
        } footer: { Text("导入导出包含声母、韵母和零声母音节映射，可与 Android 的九键双拼码表互用。") }
        ForEach(NineKeyShuangpinLayout.parts, id: \.self) { part in
          Section(NineKeyShuangpinLayout.partTitle(part)) {
            ForEach(units(part), id: \.self) { unit in
              HStack {
                Text(unit).frame(minWidth: 48, alignment: .leading)
                TextField(part == "zero_initials" ? "两位数字" : "一位数字", text: Binding(
                  get: { draft[part]?[unit] ?? "" },
                  set: { draft[part, default: [:]][unit] = $0; validate() }))
                  .keyboardType(.numberPad).multilineTextAlignment(.trailing)
                  .accessibilityIdentifier("nineKeyShuangpin_\(part)_\(unit)")
              }
              .swipeActions {
                if layout?.table[part]?[unit] == nil {
                  Button("删除多余项", role: .destructive) { draft[part]?.removeValue(forKey: unit); validate() }
                }
              }
            }
          }
        }
      }
      Section {
        if let error { Text(error).foregroundStyle(.red).accessibilityIdentifier("nineKeyShuangpinError") }
        if saved { Text("已保存，键盘将使用这张方案表").foregroundStyle(.secondary) }
        Button("保存并使用") { save() }
          .disabled(layout == nil || layout?.problem != nil || error != nil)
          .accessibilityIdentifier("nineKeyShuangpinSave")
      }
    }
    .navigationTitle("九键双拼").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
      do {
        let url = try result.get()
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let data = try file.read(upToCount: 16_385) ?? Data()
        let table = try NineKeyShuangpinLayout.importTable(data)
        // 保留用户导入的表，不能用无效表的运行时回退结果替换它。
        draft = table
        profile = "custom"
        validate()
      } catch { self.error = "导入失败：\(error.localizedDescription)"; saved = false }
    }
    .fileExporter(isPresented: $exporting, document: exportFile, contentType: .json,
                  defaultFilename: "nine-key-shuangpin") { result in
      if case .failure(let failure) = result { error = "导出失败：\(failure.localizedDescription)" }
    }
  }

  private func units(_ part: String) -> [String] {
    let canonical = Array((layout?.table[part] ?? [:]).keys)
    return Set(canonical + Array((draft[part] ?? [:]).keys)).sorted()
  }

  private func reload() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    profile = NineKeyShuangpinLayout.profile(in: document)
    do {
      let current = try NineKeyShuangpinLayout.current(in: document)
      draft = profile == "custom" ? (document?[NineKeyShuangpinLayout.tableKey] as? NineKeyShuangpinLayout.Table ?? [:]) : current.table
      validate()
    } catch { self.error = error.localizedDescription }
  }

  private func choose(_ value: String) {
    profile = value
    if value == "custom" {
      if let table = MetasequoiaInputSessionBridge.loadSharedPreferences()?[NineKeyShuangpinLayout.tableKey] as? NineKeyShuangpinLayout.Table {
        draft = table
      }
      validate()
    } else { seed(value) }
  }

  private func seed(_ value: String) {
    do {
      draft = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: value).table
      validate()
    } catch { self.error = error.localizedDescription; saved = false }
  }

  private func validate() {
    saved = false
    do {
      layout = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: profile, table: profile == "custom" ? draft : nil)
      error = layout?.problem?.message
    } catch { layout = nil; self.error = error.localizedDescription }
  }

  private func save() {
    do {
      guard try NineKeyShuangpinLayout.save(profile: profile, table: profile == "custom" ? draft : nil) else {
        error = SettingsSaveFailure.message
        return
      }
      saved = true
      error = nil
    } catch { self.error = error.localizedDescription; saved = false }
  }

  private func prepareExport() {
    do {
      exportFile = NineKeyShuangpinDocument(data: try NineKeyShuangpinLayout.exportTable(draft))
      exporting = true
    } catch { self.error = error.localizedDescription }
  }
}

private struct NineKeyShuangpinDocument: FileDocument {
  static var readableContentTypes: [UTType] { [.json] }
  let data: Data

  init(data: Data) { self.data = data }
  init(configuration: ReadConfiguration) throws { data = configuration.file.regularFileContents ?? Data() }
  func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}
