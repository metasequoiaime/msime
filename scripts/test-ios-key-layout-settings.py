#!/usr/bin/env python3
"""iOS「按键布局」设置：键盘页的入口只在本版本至少提供一组时显示，入口副标题取自 SharedUI 里三端共用的文字；子页是键盘类型分段、整块预览加等宽样式胶囊和当前样式说明的一张卡片（26 键卡片里还有「符」与「123」互换开关，每张卡片都有「中/英键放在空格左边」开关，9 键卡片里还有「九键左侧符号」导航行），下面是「数字键盘」一组胶囊（组末是「数字键盘左侧符号」导航行）和顶部数字行开关；两张符号表共用一个编辑页（KeyLayoutStyleTests 与 NineKeySidebarSettingsTests 钉住文字本身、预览的底行和高亮、编辑规则）。"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
KEYBOARD = ROOT / "platforms/ios/App/Sources/keyboard"
LAYOUT_PAGE = KEYBOARD / "KeyboardLayoutSettingsView.swift"
KEY_LAYOUT_PAGE = KEYBOARD / "KeyboardKeyLayoutSettingsView.swift"
SIDEBAR_EDITOR = KEYBOARD / "NineKeySidebarEditorView.swift"
SETTINGS_DESIGN = ROOT / "platforms/ios/App/Sources/app/SettingsDesign.swift"


def member(source: str, signature: str) -> str:
    """从 `signature` 起到同一类型里下一个成员（两格缩进的声明或文档注释）为止的源码。"""
    start = source.index(signature)
    following = re.search(r"\n  (?:private |@ViewBuilder|///|var |func |init)", source[start + len(signature):])
    return source[start:start + len(signature) + following.start()] if following else source[start:]


class KeyLayoutSettingsContract(unittest.TestCase):
    def setUp(self):
        self.source = KEY_LAYOUT_PAGE.read_text()
        self.body = member(self.source, "  var body: some View {")

    def test_entry_row_is_gated_on_the_offering(self):
        source = LAYOUT_PAGE.read_text()
        gate = source.index("if TouchKeyLayoutOffering.edition.offersAnyGroup {")
        link = source.index("NavigationLink(destination: KeyboardKeyLayoutSettingsView())")
        identifier = source.index('.accessibilityIdentifier("keyLayoutSettingsLink")')
        self.assertLess(gate, link)
        # 入口和它上面的分隔线都在条件里：到入口的无障碍标识为止只剩条件块本身没闭合，紧接着闭合。
        block = source[gate:identifier]
        self.assertEqual(block.count("{") - block.count("}"), 1, "the link sits inside the offering check")
        self.assertTrue(source[identifier:].split("\n", 2)[1].strip() == "}", "the offering check closes right after the link")
        self.assertIn("DesignDivider()", block)
        self.assertEqual(source.count("KeyboardKeyLayoutSettingsView()"), 1)

    def test_entry_row_uses_the_shared_subtitle(self):
        source = LAYOUT_PAGE.read_text()
        self.assertIn('DesignNavRowLabel(title: "按键布局", subtitle: TouchKeyLayoutOffering.entrySubtitle)', source)

    def test_old_group_list_is_gone(self):
        # 改版前每类一组单选列表、组末一段说明；改版后不再有这些零件。
        for old in ("choiceGroup(", "KeyLayoutRowPreview", "twentySixKeyFooter", "letterGridFooter", "nineKeyFooter"):
            self.assertNotIn(old, self.source, old)

    def test_kind_segments_list_the_offered_kinds_and_start_on_the_current_keyboard(self):
        self.assertIn("if offering.kinds.count > 1 { kindPicker(kind) }", self.body)
        self.assertLess(self.body.index("kindPicker(kind)"), self.body.index("styleCard(kind)"))
        picker = member(self.source, "  private func kindPicker(")
        self.assertIn("items: offering.kinds.map { ($0.title, $0) },", picker)
        self.assertIn("itemIdentifier: { $0.identifier }", picker)
        # 切分段只换显示的那一类，不写偏好。
        self.assertNotIn("save(", picker)
        self.assertIn("initialKind(current: current.kind)", self.source)
        self.assertIn("if !kindPicked { kind = offering.initialKind(current: current.kind) }", self.source)

    def test_style_card_holds_preview_pills_and_the_current_styles_note(self):
        card = member(self.source, "  private func styleCard(")
        preview = card.index("KeyLayoutKeyboardPreview(kind: kind, style: style,")
        pills = card.index("stylePills(kind)")
        note = card.index("if let note = kind.note(style) {")
        self.assertLess(preview, pills)
        self.assertLess(pills, note)
        self.assertIn('.accessibilityIdentifier("keyLayoutStyleNote")', card[note:])
        style_pills = member(self.source, "  private func stylePills(")
        for cases, selected, prefix, key in (
            ("TouchTwentySixKeyStyle", "twentySixKey", "keyLayout26_", "twentySixKey"),
            ("TouchFourteenKeyStyle", "fourteenKey", "keyLayout14_", "fourteenKey"),
            ("TouchNineKeyStyle", "nineKey", "keyLayout9_", "nineKey"),
        ):
            self.assertIn(f"pills({cases}.allCases, selected: style.{selected}, title: \\.title,", style_pills)
            self.assertIn(f'identifier: {{ "{prefix}\\($0.rawValue)" }}) {{ save(TouchKeyLayoutStyle.save({key}: $0)) }}', style_pills)

    def test_pills_share_the_row_equally_without_scrolling(self):
        pills = member(self.source, "  private func pills<Value: Hashable>(")
        self.assertIn("HStack(spacing: 6) {", pills)
        self.assertIn(".frame(maxWidth: .infinity, minHeight: 36)", pills)
        self.assertIn(".minimumScaleFactor(", pills)
        self.assertNotIn("ScrollView", pills)
        self.assertIn(".accessibilityIdentifier(identifier(option))", pills)
        self.assertIn('.accessibilityValue(isSelected ? "已选择" : "未选择")', pills)

    def test_symbol_first_toggle_sits_in_the_twenty_six_key_card_after_the_note(self):
        # #10056：互换开关只在 26 键分段的样式卡片里，排在当前样式说明之后，文字取自三端共用的常量。
        card = member(self.source, "  private func styleCard(")
        note = card.index("if let note = kind.note(style) {")
        gate = card.index("if kind == .twentySixKey {")
        self.assertLess(note, gate)
        toggle = card[gate:]
        self.assertIn("DesignToggleRow(title: TouchKeyLayoutOffering.symbolFirstTitle, subtitle: TouchKeyLayoutOffering.symbolFirstSubtitle,", toggle)
        self.assertIn("isOn: Binding(get: { style.symbolFirst }, set: { save(TouchKeyLayoutStyle.save(symbolFirst: $0)) }))", toggle)
        self.assertIn('.accessibilityIdentifier("keyLayoutSymbolFirstToggle")', toggle)
        self.assertEqual(self.source.count("TouchKeyLayoutOffering.symbolFirstTitle"), 1, "one toggle, only in the 26-key card")

    def test_language_left_toggle_sits_in_every_card_after_the_other_toggles(self):
        # #10228：「中/英键放在空格左边」在每个分段的样式卡片里，不按分段设条件；26 键里排在「符」与「123」互换之后，文字取自三端共用的常量，打开写 left、关上写 right。
        card = member(self.source, "  private func styleCard(")
        note = card.index("if let note = kind.note(style) {")
        symbol_first = card.index("TouchKeyLayoutOffering.symbolFirstTitle")
        toggle = card.index("DesignToggleRow(title: TouchKeyLayoutOffering.languageLeftTitle, subtitle: TouchKeyLayoutOffering.languageLeftSubtitle,")
        self.assertLess(note, toggle)
        self.assertLess(symbol_first, toggle)
        # 互换开关的条件块在中/英开关之前就闭合了：中/英开关不在 `if kind == .twentySixKey` 里。
        gated = card[card.index("if kind == .twentySixKey {"):toggle]
        self.assertEqual(gated.count("{") - gated.count("}"), 0, "the language toggle is shown on every kind")
        self.assertIn("isOn: Binding(get: { style.languageKeySide == .left },", card[toggle:])
        self.assertIn("set: { save(TouchKeyLayoutStyle.save(languageKeySide: $0 ? .left : .right)) }))", card[toggle:])
        self.assertIn('.accessibilityIdentifier("keyLayoutLanguageLeftSwitch")', card[toggle:])
        self.assertEqual(self.source.count("TouchKeyLayoutOffering.languageLeftTitle"), 1, "one toggle, in the shared card")

    def test_number_pad_group_uses_the_same_pills_below_the_style_card(self):
        # #10056：「数字键盘」一组有入口就列，在样式卡片和顶部数字行开关之间；选项是同一套等宽胶囊，说明在胶囊下方，不画预览。
        gate = self.body.index("if offering.offersAnyGroup { numberPadGroup }")
        self.assertLess(self.body.index("styleCard(kind)"), gate)
        self.assertLess(gate, self.body.index("if offering.numberRow {"))
        group = member(self.source, "  private var numberPadGroup: some View {")
        self.assertIn("DesignGroup(title: TouchKeyLayoutOffering.numberPadTitle) {", group)
        pills = group.index("pills(TouchNumberPadSpace.allCases, selected: style.numberPadSpace, title: \\.title,")
        self.assertIn('identifier: { "keyLayoutNumberPad_\\($0.rawValue)" }) { save(TouchKeyLayoutStyle.save(numberPadSpace: $0)) }', group)
        self.assertLess(pills, group.index("Text(TouchKeyLayoutOffering.numberPadFooter)"))
        self.assertNotIn("KeyLayoutKeyboardPreview", group)

    def test_nine_key_card_lists_the_letter_sidebar_row_after_the_note(self):
        # #10223：「九键左侧符号」是 9 键分段卡片里说明之后的一行，与 26 键卡片的互换开关同一种写法。
        card = member(self.source, "  private func styleCard(")
        note = card.index("if let note = kind.note(style) {")
        gate = card.index("if kind == .nineKey {")
        self.assertLess(note, gate)
        self.assertIn("DesignDivider()\n        sidebarLink(.letter)", card[gate:])
        self.assertEqual(self.source.count("sidebarLink(.letter)"), 1)

    def test_number_pad_group_ends_with_the_digit_sidebar_row(self):
        group = member(self.source, "  private var numberPadGroup: some View {")
        footer = group.index("Text(TouchKeyLayoutOffering.numberPadFooter)")
        link = group.index("sidebarLink(.digit)")
        self.assertLess(footer, link)
        self.assertEqual(self.source.count("sidebarLink(.digit)"), 1)

    def test_sidebar_rows_open_the_editor_with_the_summary(self):
        link = member(self.source, "  private func sidebarLink(")
        self.assertIn("NavigationLink(destination: NineKeySidebarEditorView(table: table)) {", link)
        self.assertIn("DesignNavRowLabel(title: table.title, value: NineKeySidebar.summary(style.sidebarSymbols(table)))", link)
        self.assertIn(".buttonStyle(PressFillButtonStyle())", link)
        self.assertIn('.accessibilityIdentifier("keyLayoutSidebar_\\(table.rawValue)")', link)

    def test_nine_key_preview_draws_the_letter_table_and_highlights_a_custom_one(self):
        preview = self.source[self.source.index("private struct KeyLayoutKeyboardPreview: View {"):]
        self.assertIn("let marks = TouchKeyLayoutKind.previewSidebar(style)", preview)
        self.assertIn("let sidebarHighlighted = kind.highlightsSidebar(style)", preview)
        self.assertEqual(preview.count("sidebar(marks, highlighted: sidebarHighlighted)"), 2)
        self.assertNotIn("TypingKeyHeatmap.nineKeySidebar(", preview)

    def test_sidebar_editor_saves_one_table_and_draws_no_preview(self):
        editor = SIDEBAR_EDITOR.read_text()
        self.assertIn("Text(table.note)", editor)
        self.assertIn(".onDelete { draft.remove(at: $0) }", editor)
        self.assertIn(".onMove { draft.move(from: $0, to: $1) }", editor)
        self.assertIn("ForEach(draft.addable, id: \\.self)", editor)
        self.assertIn("draft.restoreDefaults()", editor)
        self.assertIn(".disabled(!draft.canSave)", editor)
        self.assertIn("TouchKeyLayoutStyle.save(sidebar: draft.symbols, table: table)", editor)
        self.assertIn("SettingsSaveFailure.message", editor)
        self.assertIn(".navigationTitle(table.title)", editor)
        self.assertNotIn("KeyLayoutKeyboardPreview", editor)
        self.assertNotIn("updateSharedPreferences", editor, "the editor writes only through TouchKeyLayoutStyle.save")

    def test_settings_search_finds_both_sidebar_rows(self):
        layout = next(line for line in SETTINGS_DESIGN.read_text().splitlines() if "case .layout: return [" in line)
        self.assertIn("NineKeySidebar.letterTitle", layout)
        self.assertIn("NineKeySidebar.digitTitle", layout)

    def test_number_row_toggle_closes_the_page(self):
        rows = self.body[self.body.index("if offering.numberRow {"):]
        self.assertIn('DesignToggleRow(title: "顶部数字行", subtitle: TouchKeyLayoutOffering.numberRowSubtitle,', rows)
        self.assertIn("save(TouchKeyLayoutStyle.save(numberRow: $0))", rows)
        self.assertIn('.accessibilityIdentifier("keyLayoutNumberRowToggle")', rows)

    def test_preview_is_hidden_from_accessibility(self):
        preview = self.source[self.source.index("private struct KeyLayoutKeyboardPreview: View {"):]
        body = member(preview, "  var body: some View {")
        self.assertIn(".accessibilityElement(children: .ignore)", body)
        self.assertIn(".accessibilityHidden(true)", body)
        # 加了标识的子元素会重新出现在读屏和 UI 测试的树里，预览里不给任何键标识。
        self.assertNotIn(".accessibilityIdentifier(", preview)


if __name__ == "__main__":
    unittest.main()
