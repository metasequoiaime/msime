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

- A doc comment that names a preference is not a reader. `crates/host-api/src/ffi/host.rs` documents
  `traditional_chinese_variant` in prose and nothing reads it; counting prose made the gate pass a field
  macOS silently ignores. Doc-comment lines are dropped before matching.
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
    "touch_voice_shortcut": "the touch keyboard",
    "touch_quick_punctuation_symbols": "仅 iOS 原生触屏键盘提供自定义快捷符号；macOS 屏幕键盘不读取此列表，共享设置页不提供此项",
    "touch_number_keypad_order": "the touch keyboard's nine-key digit layer",
    "touch_twenty_six_key_number_layout": "the touch keyboard's 26-key digit layer",
    "touch_shuangpin_key_hints": "the touch keyboard's 26-key shuangpin letter keys",
    "touch_spelling_position": "where the touch keyboard puts the spelling; macOS composes inline and its candidate window follows `candidate_preedit_style`",
    "touch_toolbar": "the touch keyboard's row above the keys",
    "harmony_toolbar": "the HarmonyOS phone keyboard's toolbar tools and display mode; the page offers them only where the host reports `touch_toolbar_tools`",
    "number_row_selection": "releasing the number row back to the editor, offered only where the host advertises it - Linux and HarmonyOS; Windows and macOS both keep number selection",
    "english_suggestions": "the mobile suggestion strip; desktop hosts show English candidates through mixed_input instead",
    "game_compatibility": "Windows TSF only: whether the TIP skips ITfUIElementMgr for games that declare UI-less and draw nothing; macOS has no UI-less host contract",
    # 下面几条是嵌套子字段。父结构整体不适用时门禁不下降，所以这里列出的是父结构仍在 macOS 被读、但这一项本身不读的字段。
    "keybindings.toggle_english_candidates": "共享 KeyChord 契约的预留绑定字段；宿主路由与录键设置尚未接入，当前设置页不提供编辑入口，macOS 继续使用原有固定组合键（见 2026-10-21-rebindable-toggle-hotkeys 决策）",
    "keybindings.toggle_full_width": "共享 KeyChord 契约的预留绑定字段；宿主路由与录键设置尚未接入，当前设置页不提供编辑入口，macOS 继续使用原有固定组合键（见 2026-10-21-rebindable-toggle-hotkeys 决策）",
    "keybindings.toggle_punctuation": "共享 KeyChord 契约的预留绑定字段；宿主路由与录键设置尚未接入，当前设置页不提供编辑入口，macOS 继续使用原有固定组合键（见 2026-10-21-rebindable-toggle-hotkeys 决策）",
    "keybindings.switch_language_ctrl_space": "Ctrl+Space 换语言只在 Linux 提供：共享设置页按 `linux` 决定是否写这个字段（`packages/ui/src/settings/input-mode-shortcuts-model.ts` 的 `languageSwitchPatch`），macOS 走 Ctrl+Alt+Space 与 Caps Lock，不读这一项",
    "keybindings.toggle_japanese_ctrl_shift_j": "由 `HostCapabilities::japanese_toggle_shortcut` 门控，macOS 为 false（`crates/client-core/src/host_surface.rs`：macOS 把「日」声明成系统输入模式，用 Ctrl+空格和地球键轮换，不需要这个组合键），设置页不显示这一项",
    "wubi_candidate_selection.second_key": "由共享运行时经 `WubiCandidateSelectionPreferences::slot` 消费（`crates/input-runtime/src/runtime.rs` 的 `set_wubi_candidate_selection` / `candidate_for_slot`）；宿主不直接拼字段名，所以按名匹配看不见它",
    "wubi_candidate_selection.third_key": "同上：与 `second_key` 一起交给 `slot()` 决定第二、第三候选的取词键",
    "traditional_chinese_variant": "macOS 的简繁转换固定用 OpenCC `s2t`（`platforms/macos/src/core/ChineseTextConversion.mm` 调 `msime_client_simplified_to_traditional`），不选变体；带变体的 `msime_client_convert_to_traditional` 目前只有 Linux 的 IBus / Fcitx5 宿主调用（`crates/client-core/src/host_surface.rs` 的 `traditional_chinese_variants: matches!(platform, HostPlatform::Linux)`）。macOS 用 s2t 而不是 `s2tw` / `s2twp` 是刻意的：来源 MSIME-Apple 只有「简体 / 繁体」二选一的 `outputScript`，没有台湾正体 / 台湾用词三态，设置页也只给这两项；从 Linux 或云同步带过来的 `s2tw` / `s2twp` 在 macOS 上按 s2t 处理，属于「这项在 macOS 不适用」而不是「开关坏了」",
    "smart_punctuation_migrated": "not a setting: the one-time marker client-core's PreferencesStore sets while reading the document, after turning the old macOS default (smart punctuation off) on; the host reads the migrated values through msime_client_load_preferences",
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
