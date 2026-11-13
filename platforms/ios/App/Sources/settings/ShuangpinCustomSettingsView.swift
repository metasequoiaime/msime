import SwiftUI
import UniformTypeIdentifiers

struct ShuangpinCustomSettingsView: View {
  @State private var profile = "xiaohe"
  @State private var draft: ShuangpinCustomLayout.Table = [:]
  @State private var layout: ShuangpinCustomLayout?
  @State private var error: String?
  @State private var saved = false
  @State private var importing = false
  @State private var exporting = false
  @State private var exportFile = ShuangpinCustomDocument(data: Data())

  var body: some View {
    Form {
      Section {
        Picker("方案", selection: Binding(get: { profile }, set: choose)) {
          ForEach(ShuangpinCustomLayout.profiles, id: \.self) { value in
            Text(ShuangpinCustomLayout.title(value)).tag(value)
          }
        }.accessibilityIdentifier("shuangpinCustomProfile")
      } header: {
        DesignSectionHeader(title: "方案", info: "每个音节按声母、韵母输入两位字母；零声母直接输入两位字母，分号可作为第二个键。")
      }
      Section("键位提示") {
        ForEach(layout?.keys ?? [], id: \.key) { key in
          HStack(alignment: .top) {
            Text(key.key).font(.headline).frame(width: 24)
            Text(key.hint).font(.subheadline)
          }
        }
      }
      if profile == "custom" {
        Section {
          Menu("从内置方案开始") {
            ForEach(ShuangpinCustomLayout.profiles.filter { $0 != "custom" }, id: \.self) { value in
              Button(ShuangpinCustomLayout.title(value)) { seed(value) }
            }
          }
          Button("导入 JSON 码表") { importing = true }.accessibilityIdentifier("shuangpinCustomImport")
          Button("导出 JSON 码表") { prepareExport() }.accessibilityIdentifier("shuangpinCustomExport")
        } header: {
          DesignSectionHeader(title: "自定义", info: "导入导出包含声母、韵母和零声母音节映射，可与 Android 的双拼码表互用。")
        }
        ForEach(ShuangpinCustomLayout.parts, id: \.self) { part in
          Section(ShuangpinCustomLayout.partTitle(part)) {
            ForEach(units(part), id: \.self) { unit in
              HStack {
                Text(unit).frame(minWidth: 48, alignment: .leading)
                TextField(part == "zero_initials" ? "两位字母" : "一位字母或分号", text: Binding(
                  get: { draft[part]?[unit] ?? "" },
                  set: { draft[part, default: [:]][unit] = $0; validate() }))
                  .keyboardType(.asciiCapable).multilineTextAlignment(.trailing)
                  .accessibilityIdentifier("shuangpinCustom_\(part)_\(unit)")
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
        if let error { Text(error).foregroundStyle(.red).accessibilityIdentifier("shuangpinCustomError") }
        if saved { Text("已保存，键盘将使用这张方案表").foregroundStyle(.secondary) }
        Button("保存并使用") { save() }
          .disabled(layout == nil || layout?.problem != nil || error != nil)
          .accessibilityIdentifier("shuangpinCustomSave")
      }
    }
    .navigationTitle("双拼自定义").navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: reload)
    .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { result in
      do {
        let url = try result.get()
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let data = try file.read(upToCount: 16_385) ?? Data()
        let table = try ShuangpinCustomLayout.importTable(data)
        // 保留用户导入的表，不能用无效表的运行时回退结果替换它。
        draft = table
        profile = "custom"
        validate()
      } catch { self.error = "导入失败：\(error.localizedDescription)"; saved = false }
    }
    .fileExporter(isPresented: $exporting, document: exportFile, contentType: .json,
                  defaultFilename: "shuangpin-custom") { result in
      if case .failure(let failure) = result { error = "导出失败：\(failure.localizedDescription)" }
    }
  }

  private func units(_ part: String) -> [String] {
    let canonical = Array((layout?.table[part] ?? [:]).keys)
    return Set(canonical + Array((draft[part] ?? [:]).keys)).sorted()
  }

  private func reload() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    profile = ShuangpinCustomLayout.profile(in: document)
    do {
      let current = try ShuangpinCustomLayout.current(in: document)
      draft = profile == "custom" ? (document?[ShuangpinCustomLayout.tableKey] as? ShuangpinCustomLayout.Table ?? [:]) : current.table
      validate()
    } catch { self.error = error.localizedDescription }
  }

  private func choose(_ value: String) {
    profile = value
    if value == "custom" {
      if let table = MetasequoiaInputSessionBridge.loadSharedPreferences()?[ShuangpinCustomLayout.tableKey] as? ShuangpinCustomLayout.Table {
        draft = table
      }
      validate()
    } else { seed(value) }
  }

  private func seed(_ value: String) {
    do {
      draft = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: value).table
      validate()
    } catch { self.error = error.localizedDescription; saved = false }
  }

  private func validate() {
    saved = false
    do {
      layout = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: profile, table: profile == "custom" ? draft : nil)
      error = layout?.problem?.message
    } catch { layout = nil; self.error = error.localizedDescription }
  }

  private func save() {
    do {
      guard try ShuangpinCustomLayout.save(profile: profile, table: profile == "custom" ? draft : nil) else {
        error = SettingsSaveFailure.message
        return
      }
      saved = true
      error = nil
    } catch { self.error = error.localizedDescription; saved = false }
  }

  private func prepareExport() {
    do {
      exportFile = ShuangpinCustomDocument(data: try ShuangpinCustomLayout.exportTable(draft))
      exporting = true
    } catch { self.error = error.localizedDescription }
  }
}

private struct ShuangpinCustomDocument: FileDocument {
  static var readableContentTypes: [UTType] { [.json] }
  let data: Data

  init(data: Data) { self.data = data }
  init(configuration: ReadConfiguration) throws { data = configuration.file.regularFileContents ?? Data() }
  func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}
