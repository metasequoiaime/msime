import SwiftUI

/// 设置页的插件编辑器：选类型、填基本信息和内容，保存后由 `authoring_save` 按导入同样的规则校验并安装。结构和文字与共享设置页的 `plugin-editor-view.tsx` 相同：新建时先选类型；短语表和单词本可以批量粘贴；第一处问题写在保存按钮上方。以表单页的形式弹出，有未保存的修改时关掉前先确认；行的删除用 iOS 的左滑。
struct PluginEditorView: View {
  @ObservedObject var library: PluginLibrary
  /// 编辑已安装的包时是它；新建时为 nil，先选类型。
  let target: PluginPackage?
  /// 保存成功：交回装好的包，由调用方关掉编辑器并打开详情。
  let onSaved: (PluginPackage) -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var draft: PluginEditorModel.Draft?
  @State private var baseline: PluginEditorModel.Draft?
  @State private var loadError: String?
  @State private var saveError: String?
  @State private var saving = false
  @State private var shown = PluginEditorModel.rowPage
  @State private var bulk = ""
  @State private var bulkIssue: String?
  @State private var confirmingDiscard = false

  private var dirty: Bool { draft != nil && draft != baseline }
  private var editing: Bool { target != nil }

  var body: some View {
    NavigationStack {
      Form { content }
        .designPage()
        .scrollDismissesKeyboard(.immediately)
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) {
            Button("取消") { if dirty { confirmingDiscard = true } else { dismiss() } }
              .disabled(saving)
              .accessibilityIdentifier("pluginEditorCancel")
          }
        }
        .confirmationDialog("放弃修改", isPresented: $confirmingDiscard, titleVisibility: .visible) {
          Button("放弃", role: .destructive) { dismiss() }
        } message: {
          Text("还没有保存的修改会丢掉。")
        }
    }
    .interactiveDismissDisabled(dirty || saving)
    .task { await load() }
  }

  private var title: String {
    if let target { return "编辑「\(baseline?.name ?? target.name)」" }
    return draft.map { "新建\($0.kind.label)" } ?? "新建插件"
  }

  @ViewBuilder private var content: some View {
    if draft == nil {
      if editing {
        Section { Text(loadError ?? "正在读取插件…").foregroundStyle(loadError == nil ? Color.secondary : Color.red) }.designRow()
      } else {
        Section("想做哪一种插件？") {
          ForEach(PluginEditorModel.kinds) { entry in
            Button { pick(entry.kind) } label: {
              PluginRowLabel(title: entry.title, detail: entry.description, marker: nil)
            }
            .accessibilityIdentifier("pluginCreate.\(entry.kind.rawValue)")
          }
        }
        .designRow()
      }
    } else {
      let kind = draft?.kind ?? .phraseTable
      basics
      switch kind {
      case .phraseTable: phraseRows
      case .commandTable: commandRows
      case .symbolSet: symbolGroups
      case .wordbook: wordRows
      default: EmptyView()
      }
      if kind == .phraseTable || kind == .wordbook { bulkSection }
      saveSection
    }
  }

  private func binding<Value>(_ keyPath: WritableKeyPath<PluginEditorModel.Draft, Value>, _ fallback: Value) -> Binding<Value> {
    Binding(get: { draft?[keyPath: keyPath] ?? fallback }, set: { value in
      saveError = nil
      draft?[keyPath: keyPath] = value
    })
  }

  /// 一行里的一个字段。左滑删掉一行之后，SwiftUI 还会再读一次旧行的绑定，所以越界时给空值、不写。
  private func row<Row, Value>(_ rows: WritableKeyPath<PluginEditorModel.Draft, [Row]>, _ index: Int,
                               _ field: WritableKeyPath<Row, Value>, _ fallback: Value) -> Binding<Value> {
    Binding(get: {
      guard let list = draft?[keyPath: rows], list.indices.contains(index) else { return fallback }
      return list[index][keyPath: field]
    }, set: { value in
      guard var current = draft, current[keyPath: rows].indices.contains(index) else { return }
      current[keyPath: rows][index][keyPath: field] = value
      draft = current
      saveError = nil
    })
  }

  private var basics: some View {
    Section {
      LabeledContent("名字") {
        TextField("名字", text: binding(\.name, ""), prompt: Text("我的\(draft?.kind.label ?? "插件")"))
          .multilineTextAlignment(.trailing)
          .accessibilityLabel("插件名字").accessibilityIdentifier("pluginEditorName")
      }
      LabeledContent("作者") {
        TextField("作者", text: binding(\.author, ""), prompt: Text("可不填"))
          .multilineTextAlignment(.trailing)
          .accessibilityLabel("作者")
      }
      LabeledContent("版本") {
        TextField("版本", text: binding(\.version, "")).codeField().multilineTextAlignment(.trailing)
      }
      LabeledContent("许可证") {
        TextField("许可证", text: binding(\.license, "")).codeField().multilineTextAlignment(.trailing)
      }
      TextField("介绍（可不填）", text: binding(\.description, ""), axis: .vertical)
        .lineLimit(2...6)
        .accessibilityLabel("介绍")
    } header: {
      DesignSectionHeader(subject: "基本信息", info: "许可证决定别人能怎样使用这个插件。\(PluginEditorModel.defaultLicense) 表示放弃全部权利，任何人都能随意使用和改编；想让别人注明出自你，可以写 CC-BY-4.0。这里只能写 SPDX 许可证代码。") { Text("基本信息") }
    }
    .designRow()
  }

  /// 每行的删除：左滑。
  private var phraseRows: some View {
    let rows = draft?.phrases ?? []
    return Section {
      ForEach(Array(rows.indices.prefix(shown)), id: \.self) { index in
        VStack(alignment: .leading, spacing: 6) {
          TextField("编码", text: row(\.phrases, index, \.key, ""), prompt: Text("编码，例如 dz")).codeField()
            .accessibilityLabel("第 \(index + 1) 条短语的编码")
          TextField("文字", text: row(\.phrases, index, \.text, ""), prompt: Text("文字"))
            .accessibilityLabel("第 \(index + 1) 条短语的文字")
        }
      }
      .onDelete { draft?.phrases.remove(atOffsets: $0) }
      moreRows(total: rows.count)
      Button("添加一条") {
        shown = max(shown, rows.count + 1)
        draft?.phrases.append(.init())
      }
      .disabled(rows.count >= PluginEditorModel.maximumPhrases)
      .accessibilityIdentifier("pluginEditorAddPhrase")
    } header: {
      DesignSectionHeader(subject: "短语（\(rows.count)）", info: "编码是 K 之后输入的小写字母，最多 32 个；同一个编码可以对应几段文字。文字最多 \(PluginEditorModel.maximumTextUTF16) 个字符，不能换行。左滑删除一条。\n在文字中用 ${cursor} 指定上屏后的光标位置，例如 title:\"${cursor}\" 会输入 title:\"\"，光标停在引号内。只定位第一个标记；要输入标记本身，请写 $${cursor}。编辑器不支持定位时光标留在末尾。") { Text("短语（\(rows.count)）") }
    }
    .designRow()
  }

  private var commandRows: some View {
    let rows = draft?.commands ?? []
    return Section {
      ForEach(Array(rows.indices), id: \.self) { index in
        VStack(alignment: .leading, spacing: 6) {
          TextField("指令", text: row(\.commands, index, \.trigger, ""), prompt: Text("指令，例如 sig")).codeField()
            .accessibilityLabel("第 \(index + 1) 条指令的字母")
          TextField("标题", text: row(\.commands, index, \.title, ""), prompt: Text("标题，例如 签名"))
            .accessibilityLabel("第 \(index + 1) 条指令的标题")
          TextField("要插入的文字", text: row(\.commands, index, \.template, ""))
            .accessibilityLabel("第 \(index + 1) 条指令要插入的文字")
        }
      }
      .onDelete { draft?.commands.remove(atOffsets: $0) }
      Button("添加一条") { draft?.commands.append(.init()) }
        .disabled(rows.count >= PluginEditorModel.maximumCommands)
    } header: {
      DesignSectionHeader(subject: "指令（\(rows.count)）", info: "指令是 / 之后输入的小写字母。要插入的文字里可以写 {date}（日期）、{time}（时间）和 {weekday}（星期几），例如「{date} 张三」；日期和时间也可以带格式，例如 {date:%m月%d日}。左滑删除一条。") { Text("指令（\(rows.count)）") }
    }
    .designRow()
  }

  @ViewBuilder private var symbolGroups: some View {
    let groups = draft?.groups ?? []
    ForEach(Array(groups.indices), id: \.self) { index in
      Section("第 \(index + 1) 组") {
        Picker("放在", selection: row(\.groups, index, \.tab, "symbols")) {
          Text("符号").tag("symbols")
          Text("颜文字").tag("kaomoji")
        }
        .accessibilityLabel("第 \(index + 1) 组放在哪一页")
        TextField("标题", text: row(\.groups, index, \.title, ""), prompt: Text("标题，例如 箭头"))
          .accessibilityLabel("第 \(index + 1) 组的标题")
        TextField("搜索关键词（可不填）", text: row(\.groups, index, \.keywords, ""), prompt: Text("搜索关键词（可不填），例如 jiantou arrow"))
          .codeField()
          .accessibilityLabel("第 \(index + 1) 组的搜索关键词")
        VStack(alignment: .leading, spacing: 4) {
          Text("符号").font(.footnote).foregroundStyle(.secondary)
          TextEditor(text: row(\.groups, index, \.items, ""))
            .frame(minHeight: 96)
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)
            .accessibilityLabel("第 \(index + 1) 组的符号")
          Text("每行一个，可以直接粘贴。").font(.footnote).foregroundStyle(.secondary)
        }
        Button("删除这一组", role: .destructive) { draft?.groups.remove(at: index) }
          .accessibilityLabel("删除第 \(index + 1) 组")
      }
      .designRow()
    }
    Section {
      Button("添加一组") { draft?.groups.append(.init()) }
        .disabled(groups.count >= PluginEditorModel.maximumGroups)
    } footer: {
      Text("符号组以插件名为分类排在符号面板的内置符号之后，颜文字组排在颜文字的 All 之后。")
    }
    .designRow()
  }

  private var wordRows: some View {
    let rows = draft?.words ?? []
    return Section {
      ForEach(Array(rows.indices.prefix(shown)), id: \.self) { index in
        VStack(alignment: .leading, spacing: 6) {
          TextField("单词", text: row(\.words, index, \.word, "")).codeField()
            .accessibilityLabel("第 \(index + 1) 个单词")
          TextField("音标（可不填）", text: row(\.words, index, \.phonetic, "")).codeField()
            .accessibilityLabel("第 \(index + 1) 个单词的音标")
          TextField("释义", text: row(\.words, index, \.gloss, ""))
            .accessibilityLabel("第 \(index + 1) 个单词的释义")
        }
      }
      .onDelete { draft?.words.remove(atOffsets: $0) }
      moreRows(total: rows.count)
      Button("添加一个") {
        shown = max(shown, rows.count + 1)
        draft?.words.append(.init())
      }
      .disabled(rows.count >= PluginEditorModel.maximumWords)
    } header: {
      DesignSectionHeader(subject: "单词（\(rows.count)）", info: "每个单词要有释义，音标可以不填。单词本的名字最多 64 个字，最多 \(PluginEditorModel.maximumWords) 个单词。左滑删除一个。") { Text("单词（\(rows.count)）") }
    }
    .designRow()
  }

  @ViewBuilder private func moreRows(total: Int) -> some View {
    if total > shown {
      Button("再显示 \(min(PluginEditorModel.rowPage, total - shown)) 条（共 \(total) 条）") {
        shown += PluginEditorModel.rowPage
      }
    }
  }

  private var bulkSection: some View {
    let phrases = draft?.kind == .phraseTable
    return Section {
      TextEditor(text: Binding(get: { bulk }, set: { bulk = $0; bulkIssue = nil }))
        .frame(minHeight: 96)
        .autocorrectionDisabled()
        .textInputAutocapitalization(.never)
        .accessibilityLabel("批量添加的内容")
      if let bulkIssue { Text(bulkIssue).foregroundStyle(.red) }
      Button("添加到列表") { addBulk() }
        .disabled(PluginEditorModel.blank(bulk))
    } header: {
      DesignSectionHeader(subject: "批量添加 · \(phrases ? "每行一条短语" : "每行一个单词")", info: phrases
        ? "编码在前，文字在后，中间用空格隔开，例如 dz 北京市海淀区。"
        : "从表格复制过来即可：单词、释义两列，或单词、音标、释义三列，列之间用 Tab 隔开。") { Text("批量添加 · \(phrases ? "每行一条短语" : "每行一个单词")") }
    }
    .designRow()
  }

  private var saveSection: some View {
    let issue = draft.flatMap(PluginEditorModel.issue)
    return Section {
      if let issue, dirty { Text(issue).foregroundStyle(.red).accessibilityIdentifier("pluginEditorIssue") }
      if let saveError { Text(saveError).foregroundStyle(.red).accessibilityIdentifier("pluginEditorSaveError") }
      Button { save() } label: {
        HStack {
          Text(editing ? "保存修改" : "保存并安装").fontWeight(.semibold)
          if saving { Spacer(); ProgressView() }
        }
      }
      .disabled(saving || !dirty || issue != nil)
      .accessibilityIdentifier("pluginEditorSave")
    } footer: {
      Text(editing
        ? "保存后替换本机的这一份。从社区装来的插件改过之后就是你自己的副本，原作者那边不受影响。"
        : "保存后装进本机，和导入的插件一样出现在「我的插件」里，也可以在「社区插件」里发布给别人。")
    }
    .designRow()
  }

  private func pick(_ kind: PluginKind) {
    let fresh = PluginEditorModel.Draft(kind: kind)
    draft = fresh
    baseline = fresh
  }

  private func load() async {
    guard let target, draft == nil else { return }
    switch await library.loadAuthored(kind: target.kind, id: target.id) {
    case .success(let loaded):
      draft = loaded
      baseline = loaded
    case .failure(let failure):
      loadError = failure.message
    }
  }

  private func addBulk() {
    guard let current = draft else { return }
    if current.kind == .phraseTable {
      let parsed = PluginEditorModel.parsePhraseLines(bulk)
      bulkIssue = parsed.issue
      guard parsed.issue == nil else { return }
      draft?.phrases = PluginEditorModel.appendRows(current.phrases, parsed.rows) { $0.isBlank }
    } else {
      let parsed = PluginEditorModel.parseWordLines(bulk)
      bulkIssue = parsed.issue
      guard parsed.issue == nil else { return }
      draft?.words = PluginEditorModel.appendRows(current.words, parsed.rows) { $0.isBlank }
    }
    bulk = ""
  }

  private func save() {
    guard let draft, PluginEditorModel.issue(draft) == nil, !saving else { return }
    saving = true
    saveError = nil
    Task {
      let outcome = await library.saveAuthored(draft)
      saving = false
      switch outcome {
      case .done(let pack):
        baseline = self.draft
        onSaved(pack)
      case .failed(let message):
        saveError = message
      }
    }
  }
}

private extension View {
  /// 编码、版本、许可证这类只写 ASCII 的输入框：不自动大写、不自动更正。
  func codeField() -> some View {
    textInputAutocapitalization(.never).autocorrectionDisabled()
  }
}
