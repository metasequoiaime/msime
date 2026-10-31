#!/usr/bin/env python3
"""Every shared preference is either honoured on macOS or listed here with the reason it is not.

The settings page is shared, so a preference added for another platform appears on macOS too. When the host
does not read it, the switch saves, the page says it saved, and nothing changes - the user is told the
opposite of the truth. That is not hypothetical: 本地 Whisper accepted a model file and recognised through
Apple's service instead, and the two smart-punctuation direct switches did nothing while the host behaved
as though both were on.

Nothing catches that by itself, because the host compiles and the tests pass either way. So the coverage is
asserted here: a field reachable from neither the macOS sources nor the shared runtime has to be named in
NOT_APPLICABLE with a reason, and a name that no longer needs to be there is a failure too, so the list
cannot quietly outlive its entries.

`Preferences` is not flat. A top-level field can be a struct of its own (`keybindings`, `voice_input`,
`wubi_candidate_selection`, ...), and a switch that saves without being read is just as invisible one level
down as at the top. So the scan recurses into every nested struct and names a nested field by its dotted
path (`keybindings.switch_language_ctrl_space`) in the report; the reachability test stays on the bare field
name, because macOS reads a nested item through an Objective-C accessor or the shared document's JSON key
(`AppearancePreferences.mm`'s `merged[@"voice_input"]`) and never spells the Rust `struct.field` path. An
exempt top-level struct is not descended into: the entry already explains the whole block.

Two deliberate limits, both narrower than "the name appears somewhere":

- 文档注释提到偏好不等于实际读取。`ffi/host.rs` 曾只在说明中提到
  `traditional_chinese_variant`，把散文也算进去会漏掉未接入的偏好；扫描时排除文档注释。
  现在 `ffi/input.rs` 的多语言逐段输出真正读取该字段，所以不再将它列为豁免。
- A field read through a helper on its own struct (`WubiCandidateSelectionPreferences::slot`) does not spell
  the field name in the scan set. That is what the two `wubi_candidate_selection` entries record, and why
  the reason has to point at the consuming call.

Usage: preference_coverage.py <repository root>
"""

import re
import sys
from pathlib import Path

# Preferences that no macOS input method can act on. Each entry says why, because "macOS does not read it"
# is exactly what a defect looks like from the outside.
NOT_APPLICABLE = {
    "android_keyboard_skin_light": "仅 Android 原生皮肤页保存浅色键盘皮肤选择；macOS 屏幕键盘跟随自己的颜色模式，不读取 Android 键盘槽位",
    "android_keyboard_skin_dark": "仅 Android 原生皮肤页保存深色键盘皮肤选择；macOS 屏幕键盘跟随自己的颜色模式，不读取 Android 键盘槽位",
    "android_text_trackpad": "仅 Android 原生键盘的 TextTrackpadSettings 和 KeyboardOptionsPage 使用的文本拖拽手势；macOS 没有 Android 输入法键面或对应设置入口",
    "input_mode_corner_hud": "仅 Linux 的 IBus 与 Fcitx5 在无法定位光标时使用屏幕角落徽章；共享设置页只在 Linux 显示此项，macOS 没有对应角落提示",
    "input_mode_focus_hud": "仅 Linux 的 IBus 与 Fcitx5 在聚焦输入框时显示模式提示；共享设置页只在 Linux 显示此项，macOS 没有聚焦提示入口",
    "settings_theme": "the settings application's own appearance; the input method has no window to theme",
    # macOS 的这些界面一律跟随颜色模式（HostCapabilities 的 surface_themes_follow），设置页不给这几行；固定明暗的主题由 InputController 钉住传给面板的 `theme`。
    "toolbar_theme": "the floating toolbar and the mode badge follow the colour mode on macOS (surface_themes_follow)",
    "menu_theme": "the input menu and the candidate context menu follow the colour mode on macOS (surface_themes_follow)",
    "screen_keyboard_theme": "the screen keyboard follows the colour mode on macOS (surface_themes_follow)",
    "emoji_theme": "the emoji panel follows the colour mode on macOS (surface_themes_follow)",
    "handwriting_theme": "the handwriting panel follows the colour mode on macOS (surface_themes_follow)",
    "voice_theme": "the voice overlay follows the colour mode on macOS (surface_themes_follow)",
    "touch_key_spacing_tenths": "the touch keyboard; the macOS screen keyboard panel draws the default spacing whatever is stored, and the page offers no slider (`touch_keyboard_spacing` is false)",
    "touch_row_spacing_tenths": "the touch keyboard; the macOS screen keyboard panel draws the default spacing whatever is stored, and the page offers no slider (`touch_keyboard_spacing` is false)",
    "touch_keyboard_height_adjustment": "the touch keyboard and the Windows panel window; the macOS screen keyboard window has a fixed height, and the page offers no slider (`touch_keyboard_height` is false)",
    "touch_keyboard_split_portrait": "仅 Android 与鸿蒙的大屏触屏键盘支持竖屏分离；macOS 屏幕键盘不采用分离布局，共享设置页只在鸿蒙平板和可折叠手机上给出此开关",
    "touch_keyboard_split": "仅鸿蒙大屏触屏键盘读它（Android 的主开关在本机设置里，iOS 在 App Group 里）；macOS 屏幕键盘不采用分离布局，共享设置页只在鸿蒙平板和可折叠手机上给出此开关",
    "touch_keyboard_split_threshold": "仅鸿蒙大屏触屏键盘的分离启用宽度；macOS 屏幕键盘不采用分离布局，共享设置页只在鸿蒙平板和可折叠手机上给出此滑块",
    "touch_keyboard_split_gap": "仅鸿蒙大屏触屏键盘的分离中间空隙；macOS 屏幕键盘不采用分离布局，共享设置页只在鸿蒙平板和可折叠手机上给出此滑块",
    "android_text_trackpad": "仅 Android 原生键盘提供文本轨迹板；macOS 使用系统编辑器的鼠标与触控板编辑，不提供该移动端面板",
    "touch_voice_shortcut": "the touch keyboard",
    "android_text_trackpad": "仅 Android 原生文本编辑页提供触控板、长按打开、点击选区和放大镜；macOS 使用系统文本编辑与物理键盘，共享设置页不提供此项",
    "touch_japanese_toggle_input": "仅 Android 原生日语九键支持连点循环；macOS 使用物理键盘罗马字输入，共享设置页不提供此开关",
    "touch_quick_punctuation_symbols": "仅 iOS 原生触屏键盘提供自定义快捷符号；macOS 屏幕键盘不读取此列表，共享设置页不提供此项",
    "touch_number_keypad_order": "the touch keyboard's nine-key digit layer",
    "touch_text_edit_button_order": "仅 Android 原生文本编辑面板读取侧边动作的排序；macOS 不提供此面板，共享设置页也不显示排序入口",
    "touch_twenty_six_key_number_layout": "the touch keyboard's 26-key digit layer",
    "touch_shuangpin_key_hints": "the touch keyboard's 26-key shuangpin letter keys",
    "touch_quanpin_shift": "仅 Android、iOS 与鸿蒙的触屏全拼 26 键读它（组字时 ⇧ 保留还是换成「分词」）；macOS 使用物理键盘，Shift 组合键的辅助码语义由引擎处理，屏幕键盘不存在这个问题",
    "touch_twenty_six_key_style": "仅 Android、iOS 与鸿蒙手机的触屏中文 26 键底行读它；macOS 使用物理键盘，屏幕键盘面板是固定的整排键盘，不画这些底行样式",
    "touch_fourteen_key_style": "仅 Android、iOS 与鸿蒙手机的触屏 14 键与 18 键底行读它；macOS 没有 14 键或 18 键键面",
    "touch_nine_key_style": "仅 Android、iOS 与鸿蒙手机的触屏九键底行读它；macOS 没有九键字母键面",
    "touch_nine_key_sidebar_symbols": "仅 Android、iOS 与鸿蒙手机的触屏九键左侧符号栏读它；macOS 屏幕键盘没有九键符号栏",
    "touch_nine_key_digit_sidebar_symbols": "仅 Android、iOS 与鸿蒙手机的触屏九宫格数字键盘左侧符号栏读它；macOS 没有九宫格数字键盘",
    "touch_number_row": "仅 Android、iOS 与鸿蒙手机的触屏字母层上方画 1 到 0 数字行；macOS 的物理键盘与屏幕键盘面板本来就有数字行",
    "touch_twenty_six_symbol_first": "仅 Android、iOS 与鸿蒙手机的触屏 26 键底行读它（符 与 123 谁在左边）；macOS 使用物理键盘，屏幕键盘面板是固定的整排键盘，没有这两个底行键",
    "touch_number_pad_space": "仅 Android、iOS 与鸿蒙手机的触屏九宫格数字键盘读它（空格放在哪）；macOS 没有九宫格数字键盘",
    "touch_one_handed_side_panel": "仅 Android、iOS 与鸿蒙的大屏触屏键盘在单手模式下读它（空出来的一侧放不放数字键盘）；macOS 没有触屏单手模式",
    "touch_language_key_side": "仅 Android、iOS 与鸿蒙手机的触屏底行读它（中/英键在空格哪一边）；macOS 使用物理键盘，屏幕键盘面板是固定的整排键盘，没有中/英这个底行键",
    "touch_nine_key_swipe_digits": "仅 Android、iOS 与鸿蒙手机的触屏拼音九键读它（1–9 键滑动输入数字）；macOS 没有九键字母键面",
    "touch_spelling_position": "where the touch keyboard puts the spelling; macOS composes inline and its candidate window follows `candidate_preedit_style`",
    "touch_toolbar": "the touch keyboard's row above the keys",
    "harmony_toolbar": "the HarmonyOS phone keyboard's toolbar tools and display mode; the page offers them only where the host reports `touch_toolbar_tools`",
    "number_row_selection": "releasing the number row back to the editor, offered only where the host advertises it - Linux and HarmonyOS; Windows and macOS both keep number selection",
    "english_suggestions": "the mobile suggestion strip; desktop hosts show English candidates through mixed_input instead",
    "next_word_prediction_buttons": "仅 Android 原生键盘在上屏后联想候选行画关闭键与展开键；host-api 只在 Android 打开引擎的上屏后联想，macOS 没有联想状态，也就没有这一行候选可放按键，共享设置页不提供此项",
    "game_compatibility": "Windows TSF only: whether the TIP skips ITfUIElementMgr for games that declare UI-less and draw nothing; macOS has no UI-less host contract",
    # 下面几条是嵌套子字段。父结构整体不适用时门禁不下降，所以这里列出的是父结构仍在 macOS 被读、但这一项本身不读的字段。
    "keybindings.switch_language_ctrl_space": "Ctrl+Space 换语言只在 Linux 提供：共享设置页按 `linux` 决定是否写这个字段（`packages/ui/src/settings/input-mode-shortcuts-model.ts` 的 `languageSwitchPatch`），macOS 走 Ctrl+Alt+Space 与 Caps Lock，不读这一项",
    "keybindings.toggle_japanese_ctrl_shift_j": "由 `HostCapabilities::japanese_toggle_shortcut` 门控，macOS 为 false（`crates/client-core/src/host_surface.rs`：macOS 把「日」声明成系统输入模式，用 Ctrl+空格和地球键轮换，不需要这个组合键），设置页不显示这一项",
    "wubi_candidate_selection.second_key": "由共享运行时经 `WubiCandidateSelectionPreferences::slot` 消费（`crates/input-runtime/src/runtime.rs` 的 `set_wubi_candidate_selection` / `candidate_for_slot`）；宿主不直接拼字段名，所以按名匹配看不见它",
    "wubi_candidate_selection.third_key": "同上：与 `second_key` 一起交给 `slot()` 决定第二、第三候选的取词键",
    "voice_input.pause_millis": "只有触屏键盘点按开始的本地识别按停顿结束录音（鸿蒙读共享文档，iOS 主 App 读共享文档，Android 在本机设置里）；macOS 的语音由按住或再按快捷键结束，没有停顿自动结束，共享设置页只在报告 `voice_pause_settings` 的鸿蒙手机和平板上给出此滑块",
    "voice_input.stop_sensitivity": "判停灵敏度只配合按停顿结束的本地识别调整 VAD 阈值；macOS 没有停顿自动结束，共享设置页只在报告 `voice_pause_settings` 的鸿蒙手机和平板上给出此滑块",
    "smart_punctuation_migrated": "not a setting: the one-time marker client-core's PreferencesStore sets while reading the document, after turning the old macOS default (smart punctuation off) on; the host reads the migrated values through msime_client_load_preferences",
    "translation_ai_migrated": "not a setting: the one-time marker client-core's PreferencesStore sets while reading the document; only Android and iOS turn their old implicit AI translation into an explicit translation_ai choice before setting it, macOS just sets it",
    "diagnostic_log.command_line_candidates": "不是输入法读的偏好：只由终端里的 `msime candidates`（`crates/mcp-server` 的 msime-mcp）在查询前检查，关着就提示并以错误码退出；输入法宿主打字时不看它。共享设置页在 macOS 的「维护与诊断」里显示这个开关",
    "fuzzy_pinyin.seeded": "not a setting: `FuzzyPinyinPreferences::seeded` 是内部标记，用来区分「首次开启」和「有意清空规则」，落盘但不渲染（`crates/client-core/src/preferences.rs` 的 `seed_fuzzy_pinyin_rules` 读它、设置页不显示）；宿主不消费它是设计如此",
    "custom_theme.keyboard.key_background": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.key_foreground": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.action_background": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.border_width": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.key_shape": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.key_material": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.key_opacity": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.gradient_end": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.gradient_horizontal": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.pattern_opacity": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.custom_border_color": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.photo_shade": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
    "custom_theme.keyboard.photo_position": "触屏键盘的自定义设计；macOS 的 AppearancePreferences 仅透传 keyboard，屏幕键盘不消费此设计",
}


def struct_fields(source: str) -> dict[str, list[tuple[str, str]]]:
    """解析文件里每个 `pub struct` 的 (字段名, 类型) 列表，按大括号配对取正文。"""
    structs: dict[str, list[tuple[str, str]]] = {}
    for match in re.finditer(r"\n(?:#\[[^\]]*\]\n)*pub struct (\w+) \{", source):
        name = match.group(1)
        index = match.end()
        depth = 1
        start = index
        while index < len(source) and depth:
            character = source[index]
            if character == '"':
                index += 1
                while index < len(source) and source[index] != '"':
                    if source[index] == "\\":
                        index += 1
                    index += 1
            elif character in "([{":
                depth += 1
            elif character in ")]}":
                depth -= 1
                if depth == 0:
                    break
            index += 1
        body = source[start:index]
        structs[name] = re.findall(r"\n    pub (\w+):\s*([^,\n]+)", body)
    return structs


def type_name(field_type: str) -> str:
    """取参与递归的结构名，只解开 Option 可选包装，不展开集合。"""
    name = field_type.strip()
    while match := re.fullmatch(r"(?:std::option::)?Option\s*<(.+)>", name):
        name = match.group(1).strip()
    return name


def preference_paths(structs: dict[str, list[tuple[str, str]]], exempt: set[str]) -> list[str]:
    """按字段路径展开嵌套结构；同类型的兄弟字段各自检查，只有当前路径遇环才停。"""
    paths: list[str] = []

    def walk(struct: str, prefix: str, ancestors: frozenset[str]) -> None:
        for field, field_type in structs.get(struct, []):
            path = f"{prefix}.{field}" if prefix else field
            paths.append(path)
            nested = type_name(field_type)
            if nested in structs and nested not in ancestors and path not in exempt:
                walk(nested, path, ancestors | {nested})

    walk("Preferences", "", frozenset({"Preferences"}))
    return paths


def strip_doc_comments(text: str) -> str:
    """删掉只含文档注释的行；散文里点名一个偏好不等于有人读它。"""
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("///") and not line.lstrip().startswith("//!")
    )


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: preference_coverage.py <repository root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])

    source = (root / "crates/client-core/src/preferences.rs").read_text(encoding="utf-8")
    structs = struct_fields(source)
    preferences = structs.get("Preferences")
    if not preferences:
        print("could not find the Preferences struct", file=sys.stderr)
        return 1
    top_level = [name for name, _ in preferences]
    if not top_level:
        print("the Preferences struct parsed to no fields", file=sys.stderr)
        return 1

    def read(*directories: str) -> str:
        text = []
        for directory in directories:
            for path in (root / directory).rglob("*"):
                # A field named only in a test is not a field the host acts on. Counting those made a
                # test about the touch keyboard - written for another platform, in a crate this scan
                # reaches - look like macOS consuming a preference it has no surface for, and the entry
                # explaining why it cannot was then reported as stale.
                if path.name in {"tests.rs", "test.rs"} or "tests" in path.parts:
                    continue
                if path.is_file() and path.suffix in {".h", ".m", ".mm", ".cpp", ".swift", ".rs"}:
                    text.append(strip_doc_comments(path.read_text(encoding="utf-8", errors="ignore")))
        return "\n".join(text)

    # The host may read a preference itself, or leave it to the shared runtime it drives.
    reachable = read("platforms/macos/src", "crates/input-runtime/src", "crates/host-api/src")

    # 递归到每个嵌套结构体。字段按名字核对（与顶层同一口径）：宿主读嵌套项时用的是 Objective-C 访问器或
    # 共享文档里的 JSON 键（`AppearancePreferences.mm` 的 `merged[@"voice_input"]`），不会拼出 Rust 的
    # `struct.field` 路径，所以限定名在这里会大面积误报。已豁免的父结构不再下降，它的理由已经覆盖整块。
    all_fields = preference_paths(structs, set(NOT_APPLICABLE))
    unreachable = [path for path in all_fields if path.rsplit(".", 1)[-1] not in reachable]

    failures = []
    for field in unreachable:
        if field not in NOT_APPLICABLE:
            failures.append(
                f"{field} is not read on macOS and has no entry here: either honour it in the host, "
                f"or say why the platform cannot"
            )
    for field in sorted(NOT_APPLICABLE):
        if field not in all_fields:
            failures.append(f"{field} is listed here but is no longer a preference; drop the entry")
        elif field not in unreachable:
            failures.append(f"{field} is listed as out of scope but the host reads it now; drop the entry")

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"{len(all_fields) - len(unreachable)} of {len(all_fields)} shared preferences reach the macOS host; "
          f"the other {len(unreachable)} are accounted for "
          f"({len(all_fields) - len(top_level)} nested fields checked).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
