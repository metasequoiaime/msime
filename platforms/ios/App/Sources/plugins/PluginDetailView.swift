import SwiftUI

/// 一个已安装的包：信息、内容预览，怎么让它起作用（启用、选为按键音效、用于全拼或双拼、去背单词），能编辑的包可以编辑，不是内置的包可以删除。各类型的说明与共享设置页的 `plugin-detail-view.tsx` 相同，打开快捷模式的说法换成 iOS 的入口。
struct PluginDetailView: View {
  @ObservedObject var library: PluginLibrary
  let key: String
  @Environment(\.dismiss) private var dismiss
  @State private var editing = false
  @State private var confirmingRemove = false
  @State private var message: String?
  @State private var saveFailed = false
  @State private var openingWordbook = false
  @State private var showsVocabulary = false

  private var pack: PluginPackage? { library.catalog.packages.first { $0.key == key } }

  var body: some View {
    Form {
      if let pack {
        if library.justSaved == key, let note = saveNote(pack) {
          Section { Text(note).accessibilityIdentifier("pluginSavedNote") }.designRow()
        }
        info(pack)
        PluginContentSections(pack: pack)
        actions(pack)
        if pack.editable {
          Section {
            Button("编辑") { editing = true }
              .disabled(library.working)
              .accessibilityLabel("编辑\(pack.name)")
              .accessibilityIdentifier("pluginEdit")
          } footer: {
            Text("不用写代码，在设置页里修改名字、介绍和内容；保存后替换本机的这一份。")
          }
          .designRow()
        }
        if !pack.builtin {
          Section {
            Button("删除", role: .destructive) { confirmingRemove = true }
              .disabled(library.working)
              .accessibilityLabel("删除\(pack.name)")
              .accessibilityIdentifier("pluginRemove")
          } footer: {
            Text("删除后包里的文件会从本机移除；正在使用的话，设置会回到默认选择。")
          }
          .designRow()
        }
      } else {
        Section { Text(library.state == .loading ? "正在读取插件…" : "这个插件已不在本机。").foregroundStyle(.secondary) }
          .designRow()
      }
    }
    .designPage()
    .navigationTitle(pack?.name ?? "插件")
    .navigationBarTitleDisplayMode(.inline)
    .onDisappear { if library.justSaved == key { library.justSaved = nil } }
    .sheet(isPresented: $editing) {
      if let pack {
        PluginEditorView(library: library, target: pack) { _ in editing = false }
      }
    }
    .navigationDestination(isPresented: $showsVocabulary) { VocabularyReviewSettingsView() }
    .confirmationDialog(pack.map { PluginCatalogPolicy.removeConfirmation($0).title } ?? "删除",
                        isPresented: $confirmingRemove, titleVisibility: .visible) {
      Button("删除", role: .destructive) { remove() }
    } message: {
      Text(pack.map { PluginCatalogPolicy.removeConfirmation($0).message } ?? "")
    }
    .alert("插件", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) {}
    } message: { Text(message ?? "") }
  }

  private func info(_ pack: PluginPackage) -> some View {
    Section("信息") {
      LabeledContent("类型", value: pack.kindLabel)
      LabeledContent("版本", value: pack.version)
      if let author = pack.author { LabeledContent("作者", value: author) }
      LabeledContent("许可证", value: pack.license)
      if pack.builtin { LabeledContent("来源", value: "内置，不能删除") }
      if let description = pack.description {
        Text(description).font(.footnote).foregroundStyle(.secondary)
      }
    }
    .designRow()
  }

  @ViewBuilder private func actions(_ pack: PluginPackage) -> some View {
    switch pack.kind {
    case .sound: soundActions(pack)
    case .commandTable: tableActions(pack)
    case .phraseTable: tableActions(pack)
    case .helpcode: helpcodeActions(pack)
    case .wordbook: wordbookActions(pack)
    case .symbolSet:
      Section("使用") {
        Text("装上即在符号面板里显示：符号组以插件名为分类排在内置符号之后，颜文字组排在颜文字的 All 之后。卸载后不再显示。")
          .font(.footnote).foregroundStyle(.secondary)
      }
      .designRow()
    default:
      Section("使用") {
        Text("这台设备用不上\(pack.kindLabel)。").font(.footnote).foregroundStyle(.secondary)
      }
      .designRow()
    }
  }

  /// 按键音效包：选用即使用，包已选中但按键音关着时按钮仍可点，点了会把按键音打开。旋律包 iOS 不播。
  @ViewBuilder private func soundActions(_ pack: PluginPackage) -> some View {
    Section {
      if pack.isMelody {
        Text("这台设备不播放按键旋律。").font(.footnote).foregroundStyle(.secondary)
      } else {
        let selected = library.preferences.keySoundPack == pack.id
        let playing = library.soundEnabled
        Button(selected ? (playing ? "使用中" : "打开按键音") : "设为当前音效包") {
          saveFailed = !library.chooseKeySound(pack)
        }
        .disabled(selected && playing)
        .accessibilityIdentifier("pluginChooseKeySound")
      }
    } header: {
      DesignSectionHeader(title: "使用", info: pack.isMelody ? nil
        : "按键音由水杉输入法自己播放，需要「允许完全访问」；开关和其他音效在「键盘 → 按键反馈」里。")
    } footer: {
      // 保存失败和「按键音现在没有打开」是状态，留在原处。
      if !pack.isMelody {
        if saveFailed {
          Text(SettingsSaveFailure.message)
        } else if !library.soundEnabled {
          Text("按键音现在没有打开。设为当前音效包会同时打开按键音。")
        }
      }
    }
    .designRow()
  }

  /// 短语表和指令表：「启用」开关排在已启用的之后，最多 16 个；快捷模式关着、当前方案打不开时说明。
  @ViewBuilder private func tableActions(_ pack: PluginPackage) -> some View {
    let command = pack.kind == .commandTable
    let enabled = command ? library.preferences.commandTables : library.preferences.phraseTables
    let position = enabled.firstIndex(of: pack.id)
    let full = position == nil && enabled.count >= PluginPreferences.maximumTables
    let modeOn = command ? library.preferences.commandMode : library.preferences.quickPhraseMode
    let schemeNote = PluginCatalogPolicy.schemeTableModeNote(library.scheme, kind: pack.kind)
    Section {
      Toggle(isOn: Binding(get: { position != nil }, set: { saveFailed = !library.setTable(pack, enabled: $0) })) {
        SettingsOptionLabel(title: "启用", detail: position.map {
          command ? "第 \($0 + 1) 位。同一指令以靠前的表为准。" : "第 \($0 + 1) 位。同一编码下靠前的表先列出。"
        } ?? (full ? "最多启用 \(PluginPreferences.maximumTables) 个\(pack.kind.label)。"
              : "启用后排在已启用的\(pack.kind.label)之后。"))
      }
      .disabled(full)
      .accessibilityIdentifier("pluginTableEnabled")
      if !modeOn {
        if command {
          Text("/ 指令（「输入 → 快捷模式」里的开关）已关闭，启用的指令表不会生效。打开后，在中文标点下没有输入时按 / 再输入指令字母即可使用。")
            .font(.footnote).foregroundStyle(.secondary)
          Button("打开 / 指令") {
            saveFailed = !library.update { PluginCatalogPolicy.setLocalMode("command", on: true, document: &$0) }
          }
          .accessibilityIdentifier("pluginEnableCommandMode")
        } else {
          Text("快捷短语（K 模式）已关闭。在「输入 → 快捷模式」打开后，\(PluginCatalogPolicy.localModeEntry("快捷短语"))再输入编码即可用到短语表。")
            .font(.footnote).foregroundStyle(.secondary)
          NavigationLink("前往快捷模式设置") { LocalModeSettingsView() }
            .accessibilityIdentifier("pluginOpenLocalModes")
        }
      }
      if let schemeNote {
        Text(schemeNote).font(.footnote).foregroundStyle(.secondary)
      }
    } header: {
      Text("使用")
    } footer: {
      if saveFailed { Text(SettingsSaveFailure.message) }
    }
    .designRow()
  }

  @ViewBuilder private func helpcodeActions(_ pack: PluginPackage) -> some View {
    Section {
      Toggle(isOn: helpcodeBinding(pack, quanpin: true)) {
        SettingsOptionLabel(title: "用于全拼", info: "替换全拼的辅助码方案；关闭后回到原来的方案。")
      }
      .accessibilityIdentifier("pluginHelpcodeQuanpin")
      Toggle(isOn: helpcodeBinding(pack, quanpin: false)) {
        SettingsOptionLabel(title: "用于双拼", info: "替换双拼的辅助码方案；关闭后回到原来的方案。")
      }
      .accessibilityIdentifier("pluginHelpcodeShuangpin")
      NavigationLink("前往辅助码设置") { HelpcodeSettingsView() }
    } header: {
      DesignSectionHeader(subject: "使用", info: "辅助码的开关和显示方式在「输入 → 辅助码」里。") { Text("使用") }
    } footer: {
      if saveFailed { Text(SettingsSaveFailure.message) }
    }
    .designRow()
  }

  private func helpcodeBinding(_ pack: PluginPackage, quanpin: Bool) -> Binding<Bool> {
    Binding(get: {
      (quanpin ? library.preferences.helpcodeQuanpin : library.preferences.helpcodeShuangpin) == pack.id
    }, set: { on in
      saveFailed = !library.update { PluginCatalogPolicy.setHelpcode(pack.id, quanpin: quanpin, on: on, document: &$0) }
    })
  }

  @ViewBuilder private func wordbookActions(_ pack: PluginPackage) -> some View {
    Section {
      Button("去背单词") { openWordbook(pack) }
        .disabled(openingWordbook)
        .accessibilityIdentifier("pluginOpenWordbook")
    } header: {
      DesignSectionHeader(subject: "使用", info: "这本书出现在背单词的词书里。卸载插件后复习进度仍会保留，重新安装后可以接着复习。") { Text("使用") }
    }
    .designRow()
  }

  /// 在背单词里选中这本书，再推入背单词页。选不上时（例如背单词的存储读不了）照样打开，让用户在那里自己选。
  private func openWordbook(_ pack: PluginPackage) {
    openingWordbook = true
    let book = PluginCatalogPolicy.wordbookBookID(pack.id)
    Task {
      _ = await Task.detached(priority: .userInitiated) {
        let store = VocabularyReviewStore()
        guard let status = try? store.load() else { return }
        _ = try? store.setSettings(wordbook: book, newPerDay: status.newPerDay, sessionLimit: status.sessionLimit)
      }.value
      openingWordbook = false
      showsVocabulary = true
    }
  }

  private func saveNote(_ pack: PluginPackage) -> String? {
    switch pack.kind {
    case .phraseTable:
      return PluginEditorModel.saveNote(kind: pack.kind, enabled: library.preferences.phraseTables.contains(pack.id),
                                        modeOn: library.preferences.quickPhraseMode)
    case .commandTable:
      return PluginEditorModel.saveNote(kind: pack.kind, enabled: library.preferences.commandTables.contains(pack.id),
                                        modeOn: library.preferences.commandMode)
    case .symbolSet, .wordbook:
      return PluginEditorModel.saveNote(kind: pack.kind, enabled: false, modeOn: false)
    default:
      return nil
    }
  }

  private func remove() {
    guard let pack else { return }
    Task {
      if let failure = await library.remove(pack) {
        message = failure
      } else {
        dismiss()
      }
    }
  }
}

/// 各类型自己的内容：指令表的指令、短语表的行数与前几行、辅助码表的条数与前几条、单词本的单词数与前几个、符号集每组的前几项。
struct PluginContentSections: View {
  let pack: PluginPackage

  var body: some View {
    switch pack.kind {
    case .symbolSet:
      ForEach(Array(pack.groups.enumerated()), id: \.offset) { _, group in
        Section("\(group.tab == "kaomoji" ? "颜文字" : "符号") · \(group.title)（\(group.items.count)）") {
          Text(group.items.prefix(PluginCatalogPolicy.symbolPreviewItems).joined(separator: " ")
            + (group.items.count > PluginCatalogPolicy.symbolPreviewItems ? " …" : ""))
            .font(.footnote)
        }
        .designRow()
      }
    case .wordbook:
      let words = Array(pack.firstWords.prefix(PluginCatalogPolicy.wordbookPreviewWords))
      Section {
        ForEach(words, id: \.self) { Text($0) }
      } header: {
        Text("单词（\(pack.wordCount ?? words.count)）")
      } footer: {
        if (pack.wordCount ?? 0) > words.count { Text("只显示前 \(words.count) 个单词。") }
      }
      .designRow()
    case .helpcode:
      Section {
        ForEach(Array(pack.preview.enumerated()), id: \.offset) { _, entry in
          LabeledContent(entry.character, value: entry.code)
        }
      } header: {
        Text("辅助码（\(pack.entries ?? pack.preview.count) 个字）")
      } footer: {
        if (pack.entries ?? 0) > pack.preview.count { Text("只显示前 \(pack.preview.count) 个字。") }
      }
      .designRow()
    case .phraseTable:
      let shown = Array(pack.phrases.prefix(PluginCatalogPolicy.phrasePreviewRows))
      Section {
        ForEach(Array(shown.enumerated()), id: \.offset) { _, phrase in
          LabeledContent(phrase.key, value: phrase.text)
        }
      } header: {
        Text("短语（\(pack.phrases.count)）")
      } footer: {
        if pack.phrases.count > shown.count { Text("只显示前 \(PluginCatalogPolicy.phrasePreviewRows) 条。") }
      }
      .designRow()
    case .commandTable:
      Section("指令（\(pack.commands.count)）") {
        if pack.commands.isEmpty {
          Text("没有指令。").foregroundStyle(.secondary)
        }
        ForEach(Array(pack.commands.enumerated()), id: \.offset) { _, command in
          LabeledContent("/\(command.trigger)", value: command.title)
        }
      }
      .designRow()
    default:
      EmptyView()
    }
  }
}

/// 设置里选中、但本机已经没有的包（或者按键音选成了旋律包），只有一条出路：像删掉它那样把这个选择清掉。与 `MissingPluginView` 相同。
struct MissingPluginView: View {
  @ObservedObject var library: PluginLibrary
  let key: String
  @Environment(\.dismiss) private var dismiss
  @State private var saveFailed = false

  private var entry: PluginMissingSelection? { library.missing.first { $0.key == key } }

  var body: some View {
    Form {
      if let entry {
        let label = entry.kind.label
        let action = Self.action(entry.kind)
        Section {
          Text(entry.mismatched
            ? "设置里\(entry.uses.joined(separator: "、"))是「\(entry.id)」，\(entry.reason)。可以\(action)，或者在「我的插件」里打开另一个\(label)。"
            : "设置里\(entry.uses.joined(separator: "、"))是「\(entry.id)」，但这个\(label)已不在本机，可能被删除或无法载入。可以重新导入它，或者\(action)。")
          Button(action) {
            saveFailed = !library.update { PluginCatalogPolicy.withoutRemovedPack(entry.kind, id: entry.id, document: &$0) }
            if !saveFailed { dismiss() }
          }
          .accessibilityIdentifier("pluginMissingDrop")
        } footer: {
          if saveFailed { Text(SettingsSaveFailure.message) }
        }
        .designRow()
      } else {
        Section { Text("这个选择已经清掉了。").foregroundStyle(.secondary) }.designRow()
      }
    }
    .designPage()
    .navigationTitle(entry?.title ?? "插件")
    .navigationBarTitleDisplayMode(.inline)
  }

  static func action(_ kind: PluginKind) -> String {
    switch kind {
    case .sound: "改回默认"
    case .commandTable, .phraseTable: "移除"
    case .helpcode: "改回原来的方案"
    default: "不再使用"
    }
  }
}
