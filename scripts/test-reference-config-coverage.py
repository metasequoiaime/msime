#!/usr/bin/env python3
"""Every setting the reference ships a default for has a field here that still exists.

`test-windows-config-keys.py` already asks the other half of this question - whether the reference
has a configuration key this repository's Windows template does not - and asks it against the pinned
reference commit in `reference_source.py`. This one does not repeat that. What it adds is the mapping that
check has no room for: which field on *this* side answers each reference setting, so a rename here
cannot quietly orphan one.

The distinction matters because the two products name almost nothing the same way. Checking the
settings pages against each other by eye has been done several times and keeps producing the same
two false results: a key that looks missing because it was deliberately renamed (`y_mode` is
`local_modes.temporary_english`, `cn_en_mixed_input_min_chars` is `mixed_input.minimum_prefix`), and
a key that looks present because some unrelated identifier happens to contain the same word. The
mapping is written down once, here, and every target is checked to still exist.

A target is a field path from `Preferences` (`floating_toolbar.scale_percent`), or from another root struct when it names one first (`TypingStatistics.retention`). Every segment is resolved against the `pub <field>: <Type>` declarations of the structs in the shared Rust sources, so a target is satisfied by that field on that struct and not by the same word in a comment, a local variable or a field of some unrelated struct. The first version matched bare words, and `appearance.theme_menu -> menu` or `general.floating_toolbar_scale -> scale` passed while naming no field at all.

Eleven keys are mapped to a reason instead of a field, written as `!kind: why`. Six are written by the reference's own template and read by nothing in it, two are internal switches of the reference (its old and new session implementations, and its Direct2D and WebView2 surfaces), two are the reference's single prompt boxes that the custom prompt slots replace here, and one is a path this repository takes as a host runtime option rather than a preference.

A field is also checked to be *named in the shared settings page*, not only present in the
preferences crate. The arrangement this client is built around puts shared behaviour and its
interface in the Tauri page, so a setting with a field and nothing on that page is supported on
paper: it exists, it round-trips, and no user can change it. `PLATFORM_LOCAL` records the ones
whose subject does not exist here at all. Finding this check's first entry also corrected a
mapping - `general.candidate_arrow_navigation` pointed at the serde alias rather than at `arrows`,
the field the page actually spells, which made a reachable setting look unreachable.
"""

from __future__ import annotations

import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHARED = [
    ROOT / "crates/client-core/src/preferences.rs",
    ROOT / "packages/ui/src/index.tsx",
    # The typing statistics switch and retention live in their own document rather than in the preferences.
    ROOT / "crates/client-core/src/typing_statistics.rs",
]
# reference `section.key` -> the field path that answers it on this side (see the module docstring), or `!kind: why` for the ones that deliberately have no target. A per-provider token such as `token_deepseek` maps to the provider-keyed `tokens` map that holds it.
MAPPING: dict[str, str] = {
    "ai_assistant.candidate_limit": "ai_assistant.candidate_limit",
    "ai_assistant.enabled": "ai_assistant.enabled",
    "ai_assistant.endpoint": "ai_assistant.endpoint",
    "ai_assistant.model": "ai_assistant.model",
    "ai_assistant.prompt": "!replaced: the reference's single prompt box; here the three prompt_custom slots hold custom prompts and an empty slot uses the built-in prompt",
    "ai_assistant.prompt_custom_1": "ai_assistant.prompt_custom_1",
    "ai_assistant.prompt_custom_2": "ai_assistant.prompt_custom_2",
    "ai_assistant.prompt_custom_3": "ai_assistant.prompt_custom_3",
    "ai_assistant.prompt_id": "ai_assistant.prompt_id",
    "ai_assistant.provider": "ai_assistant.provider",
    "ai_assistant.token": "ai_assistant.token",
    "ai_assistant.token_deepseek": "ai_assistant.tokens",
    "ai_assistant.token_groq": "ai_assistant.tokens",
    "ai_assistant.token_openai": "ai_assistant.tokens",
    "ai_assistant.token_siliconflow": "ai_assistant.tokens",
    "appearance.cand_text_color": "custom_theme.candidate_colors.text",
    "appearance.candidate_skin": "custom_theme.candidate_skin",
    "appearance.candidate_window_follow_cursor": "candidate_follow_cursor",
    "appearance.candidate_window_layout": "candidate_layout",
    "appearance.candidate_window_preedit_font_size": "candidate_preedit_font_size",
    "appearance.candidate_window_preedit_style": "candidate_preedit_style",
    "appearance.default_font": "candidate_fallback_fonts",
    "appearance.english_font": "candidate_english_font",
    "appearance.fallback_fonts": "candidate_fallback_fonts",
    "appearance.font": "candidate_font_family",
    "appearance.font_size": "candidate_font_size",
    "appearance.page_size": "candidate_page_size",
    "appearance.theme_cand": "candidate_theme",
    "appearance.theme_emoji": "emoji_theme",
    "appearance.theme_ftb": "toolbar_theme",
    "appearance.theme_handwriting": "handwriting_theme",
    "appearance.theme_menu": "menu_theme",
    "appearance.theme_mode": "theme",
    "appearance.theme_screen_keyboard": "screen_keyboard_theme",
    "appearance.theme_settings": "settings_theme",
    "appearance.theme_voice": "voice_theme",
    "appearance.tsf_preedit_style": "tsf_preedit_style",
    "appearance.ui_backend": "!internal: chooses between the reference's Direct2D surfaces and its WebView2 ones; every platform here draws the candidate window, floating toolbar and menu natively, so there is no second backend to pick",
    "custom_translation.api_key": "custom_translation.api_key",
    "custom_translation.enabled": "custom_translation.enabled",
    "custom_translation.endpoint": "custom_translation.endpoint",
    "dictionary.dictionary_path": "!runtime: the dictionary directory is a host runtime option here, not a preference",
    "frequency_adjustment.linear_step": "frequency.linear_step",
    "frequency_adjustment.mode": "frequency.mode",
    "frequency_adjustment.trigger_count": "frequency.trigger_count",
    # The field is `arrows`; `candidate_arrow_navigation` is the serde alias kept for profiles
    # written before the rename, and naming the alias here hid the fact that the page spells the
    # field.
    "general.candidate_arrow_navigation": "navigation.arrows",
    "general.candidate_translations": "candidate_translations",
    "general.candidate_window_diagnostic_log": "diagnostic_log",
    "general.clean_mode": "!dead: written by the reference's config template and read by nothing in it",
    "general.cloud_candidates": "cloud_candidates",
    "general.cn_en_mixed_input": "mixed_input.english",
    "general.cn_en_mixed_input_min_chars": "mixed_input.minimum_prefix",
    "general.diagnostic_log": "diagnostic_log",
    "general.emoji_mixed_input": "mixed_input.emoji",
    "general.enable_emoji": "!dead: written by the reference's config template and read by nothing in it",
    "general.floating_toolbar": "floating_toolbar.enabled",
    "general.floating_toolbar_character_set": "floating_toolbar.character_set",
    "general.floating_toolbar_emoji": "floating_toolbar.emoji",
    "general.floating_toolbar_font_size": "floating_toolbar.font_size",
    "general.floating_toolbar_fullwidth": "floating_toolbar.fullwidth",
    "general.floating_toolbar_punctuation": "floating_toolbar.punctuation",
    "general.floating_toolbar_scale": "floating_toolbar.scale_percent",
    "general.floating_toolbar_screen_keyboard": "floating_toolbar.screen_keyboard",
    "general.floating_toolbar_settings": "floating_toolbar.settings",
    "general.kaomoji_mixed_input": "mixed_input.kaomoji",
    "general.paging_brackets": "navigation.brackets",
    "general.paging_comma_period": "navigation.comma_period",
    "general.paging_minus_equal": "navigation.minus_equal",
    "general.paging_mouse_wheel": "navigation.mouse_wheel",
    "general.paging_page_up_down": "navigation.page_up_down",
    "general.paging_tab": "navigation.tab",
    "helpcode.quanpin_helpcode": "quanpin_helpcode.enabled",
    "helpcode.quanpin_helpcode_schema": "quanpin_helpcode.schema",
    "helpcode.show_qp_helpcode_in_candidate_window": "quanpin_helpcode.show_in_candidate_window",
    "helpcode.show_sp_helpcode_in_candidate_window": "shuangpin_helpcode.show_in_candidate_window",
    "helpcode.shuangpin_helpcode": "shuangpin_helpcode.enabled",
    "helpcode.shuangpin_helpcode_schema": "shuangpin_helpcode.schema",
    "input.character_set": "traditional_chinese_output",
    "input.default_ime_mode": "default_ime_mode",
    "input.fuzzy_an_ang": "fuzzy_pinyin.rules",
    "input.fuzzy_c_ch": "fuzzy_pinyin.rules",
    "input.fuzzy_en_eng": "fuzzy_pinyin.rules",
    "input.fuzzy_f_h": "fuzzy_pinyin.rules",
    "input.fuzzy_ian_iang": "fuzzy_pinyin.rules",
    "input.fuzzy_in_ing": "fuzzy_pinyin.rules",
    "input.fuzzy_n_l": "fuzzy_pinyin.rules",
    "input.fuzzy_pinyin": "fuzzy_pinyin.enabled",
    "input.fuzzy_r_l": "fuzzy_pinyin.rules",
    "input.fuzzy_s_sh": "fuzzy_pinyin.rules",
    "input.fuzzy_seeded": "fuzzy_pinyin.seeded",
    "input.fuzzy_uan_uang": "fuzzy_pinyin.rules",
    "input.fuzzy_z_zh": "fuzzy_pinyin.rules",
    "input.ime_mode_scope": "ime_mode_scope",
    "input.japanese_schema": "scheme",
    "input.mode": "scheme",
    "input.paired_punctuation": "paired_punctuation",
    "input.punctuation_lock": "punctuation_lock",
    "input.schema": "scheme",
    "input.session_backend": "!internal: the reference Server's own legacy/new session switch, and this repo has one runtime",
    "input.shuangpin_preedit_mode": "shuangpin_preedit_uses_raw",
    "input.shuangpin_schema": "shuangpin_profile",
    "input.smart_punctuation": "smart_punctuation",
    "input.smart_punctuation_direct_digit": "smart_punctuation_direct_digit",
    "input.smart_punctuation_direct_letter": "smart_punctuation_direct_letter",
    "input.smart_punctuation_repeat_to_chinese": "smart_punctuation_repeat",
    "input.smart_punctuation_space_convert": "smart_punctuation_space_convert",
    "input.word_to_character": "word_character.enabled",
    "input.word_to_character_keys": "word_character.keys",
    "input.wubi_schema": "wubi_profile",
    "keybindings.switch_language_ctrl": "keybindings.switch_language_ctrl",
    "keybindings.switch_language_ctrl_alt_space": "keybindings.switch_language_ctrl_alt_space",
    "keybindings.switch_language_shift": "keybindings.switch_language_shift",
    "keybindings.toggle_character_set_ctrl_shift_f": "keybindings.toggle_character_set_ctrl_shift_f",
    "niutrans.apikey": "niutrans.apikey",
    "niutrans.app_id": "niutrans.app_id",
    "niutrans.enabled": "niutrans.enabled",
    "quanpin.autocorrect_neighbor": "quanpin.autocorrect_neighbor",
    "quanpin.autocorrect_transposition": "quanpin.autocorrect_transposition",
    "settings_page.theme": "!dead: written by the reference's config template and read by nothing in it",
    "skin.skin_name": "custom_theme.candidate_skin",
    "soft_keyboard.background_img": "!dead: written by the reference's config template and read by nothing in it",
    "soft_keyboard.theme_mode": "!dead: written by the reference's config template and read by nothing in it",
    "statistics.enabled": "TypingStatistics.enabled",
    "statistics.retention": "TypingStatistics.retention",
    "tencent_tmt.enabled": "tencent_tmt.enabled",
    "tencent_tmt.region": "tencent_tmt.region",
    "tencent_tmt.secret_id": "tencent_tmt.secret_id",
    "tencent_tmt.secret_key": "tencent_tmt.secret_key",
    "tencent_tmt.target_language": "translation_target_language",
    "utility.clipboard_history": "clipboard_history",
    "utility.date_time_mode": "local_modes.date_time",
    "utility.emoji_mode": "local_modes.emoji",
    "utility.jianpin_mode": "local_modes.super_jianpin",
    "utility.kaomoji_mode": "local_modes.kaomoji",
    "utility.quick_phrase": "local_modes.quick_phrase",
    "utility.r_mode": "local_modes.temporary_japanese",
    "utility.study_english_word": "!dead: written by the reference's config template and read by nothing in it",
    "utility.unicode_mode": "local_modes.unicode",
    "utility.y_mode": "local_modes.temporary_english",
    "voice_input.asr_app_key": "voice_input.asr_app_key",
    "voice_input.asr_endpoint": "voice_input.asr_endpoint",
    "voice_input.asr_model": "voice_input.asr_model",
    "voice_input.asr_provider": "voice_input.asr_provider",
    "voice_input.asr_resource_id": "voice_input.asr_resource_id",
    "voice_input.asr_token": "voice_input.asr_token",
    "voice_input.asr_token_doubao": "voice_input.asr_tokens",
    "voice_input.asr_token_groq": "voice_input.asr_tokens",
    "voice_input.asr_token_openai": "voice_input.asr_tokens",
    "voice_input.asr_token_siliconflow": "voice_input.asr_tokens",
    "voice_input.commit_mode": "voice_input.commit_mode",
    "voice_input.doubao_auth_mode": "voice_input.doubao_auth_mode",
    "voice_input.doubao_boosting_table_id": "voice_input.doubao_boosting_table_id",
    "voice_input.doubao_enable_ddc": "voice_input.doubao_enable_ddc",
    "voice_input.doubao_enable_itn": "voice_input.doubao_enable_itn",
    "voice_input.doubao_enable_punc": "voice_input.doubao_enable_punc",
    "voice_input.end_sound": "voice_input.end_sound",
    "voice_input.hotkey_ctrl_f9": "voice_input.hotkey_ctrl_f9",
    "voice_input.hotkey_ctrl_win": "voice_input.hotkey_ctrl_win",
    "voice_input.hotkey_hold_space_lock": "voice_input.hotkey_hold_space_lock",
    "voice_input.hotkey_ralt": "voice_input.hotkey_ralt",
    "voice_input.hotkey_rctrl_ralt": "voice_input.hotkey_rctrl_ralt",
    "voice_input.language": "voice_input.language",
    "voice_input.mute_system_audio": "voice_input.mute_system_audio",
    "voice_input.polish_endpoint": "voice_input.polish_endpoint",
    "voice_input.polish_model": "voice_input.polish_model",
    "voice_input.polish_prompt": "!replaced: the reference's single prompt box; here the three polish_prompt_custom slots hold custom prompts and an empty slot uses the built-in prompt",
    "voice_input.polish_prompt_custom_1": "voice_input.polish_prompt_custom_1",
    "voice_input.polish_prompt_custom_2": "voice_input.polish_prompt_custom_2",
    "voice_input.polish_prompt_custom_3": "voice_input.polish_prompt_custom_3",
    "voice_input.polish_prompt_id": "voice_input.polish_prompt_id",
    "voice_input.polish_provider": "voice_input.polish_provider",
    "voice_input.polish_text": "voice_input.polish_text",
    "voice_input.polish_token": "voice_input.polish_token",
    "voice_input.polish_token_deepseek": "voice_input.polish_tokens",
    "voice_input.polish_token_groq": "voice_input.polish_tokens",
    "voice_input.polish_token_openai": "voice_input.polish_tokens",
    "voice_input.polish_token_siliconflow": "voice_input.polish_tokens",
    "voice_input.start_sound": "voice_input.start_sound",
    "voice_input.stream_inline_preedit": "voice_input.stream_inline_preedit",
    "voice_input.voice_input": "voice_input.enabled",
}


# Targets allowed to have no control on the shared settings page, and why. Each is a setting whose
# subject does not exist here rather than one this client has not got round to.
# Targets whose field name is composed at run time rather than written out, so a literal search
# cannot see the control that reaches them. The three voice polish slots are written by
# `polish_prompt_${slot}` in the 润色提示词 textarea's onChange, which is the control for all three.
COMPOSED_AT_RUNTIME: dict[str, str] = {
    "polish_prompt_custom_1": "written through `polish_prompt_${slot}` by the 润色提示词 textarea",
    "polish_prompt_custom_2": "written through `polish_prompt_${slot}` by the 润色提示词 textarea",
    "polish_prompt_custom_3": "written through `polish_prompt_${slot}` by the 润色提示词 textarea",
}

PLATFORM_LOCAL: dict[str, str] = {
    "quanpin.autocorrect_neighbor": (
        "Quanpin typo correction is enabled by the shared Engine default on every host; the "
        "shared settings page intentionally has no toggle, while an explicit false in the "
        "document is honoured."
    ),
    "quanpin.autocorrect_transposition": (
        "Quanpin typo correction is enabled by the shared Engine default on every host; the "
        "shared settings page intentionally has no toggle, while an explicit false in the "
        "document is honoured."
    ),
}


def page_text() -> str:
    """The shared settings page, which is a directory rather than a file.

    `index.tsx` holds the page, but controls are extracted into components beside it - the
    candidate font controls, including the fallback-font editor, live in
    `candidate/candidate-font-controls.tsx`. Reading only `index.tsx` makes every such control
    invisible, and the first version of this check did exactly that.
    """
    root = ROOT / "packages/ui/src"
    if not root.is_dir():
        return ""
    return "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(root.rglob("*"))
        if path.suffix in {".tsx", ".ts"} and path.is_file()
    )


# The Windows template that `test-windows-config-keys.py` holds to the reference's key set. Every key it
# ships has to be in `MAPPING`, otherwise a reference setting can sit in the template with no answer on
# this side and this check never looks at it; `[statistics]` was missing from the mapping that way.
TEMPLATE = ROOT / "platforms/windows/installer/config.default.toml"


def template_keys() -> set[str]:
    if not TEMPLATE.is_file():
        return set()
    keys: set[str] = set()

    def walk(prefix: list[str], value: object) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                walk(prefix + [key], child)
        else:
            keys.add(".".join(prefix))

    walk([], tomllib.loads(TEMPLATE.read_text(encoding="utf-8")))
    return keys


def shared_text() -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in SHARED if path.is_file())


ROOT_STRUCT = "Preferences"


def rust_structs() -> dict[str, dict[str, str]]:
    """`struct name -> {field name -> declared type}` for the braced structs in the shared Rust sources."""
    structs: dict[str, dict[str, str]] = {}
    for path in SHARED:
        if path.suffix != ".rs" or not path.is_file():
            continue
        source = path.read_text(encoding="utf-8")
        for match in re.finditer(r"\bpub struct (\w+)\s*\{", source):
            depth, index = 1, match.end()
            while depth and index < len(source):
                depth += {"{": 1, "}": -1}.get(source[index], 0)
                index += 1
            body = re.sub(r"//[^\n]*", "", source[match.end() : index - 1])
            structs[match.group(1)] = {
                field.group(1): field.group(2).strip()
                for field in re.finditer(r"^\s*pub (\w+)\s*:\s*([^\n]+?),?\s*$", body, re.M)
            }
    return structs


def resolve(target: str, structs: dict[str, dict[str, str]]) -> str | None:
    """Why `target` does not name a declared field path, or None when it does."""
    segments = target.split(".")
    struct = segments.pop(0) if segments[0] in structs else ROOT_STRUCT
    for position, segment in enumerate(segments):
        fields = structs.get(struct)
        if fields is None:
            return f"`{struct}` is not a struct in the shared Rust sources"
        if segment not in fields:
            return f"`{struct}` has no `pub {segment}:` field"
        if position + 1 < len(segments):
            # Descend through wrappers such as `Option<T>` to the struct the next segment belongs to.
            nested = [name for name in re.findall(r"\w+", fields[segment]) if name in structs]
            if not nested:
                return f"`{struct}.{segment}` is `{fields[segment]}`, not a struct with fields"
            struct = nested[-1]
    return None


def page_uses(name: str, text: str) -> bool:
    """Whether the page spells `name` as a field: `.name` or `?.name` access, a `name:` / `name?:` object key or member, or a `["name"]` index. A bare word in a comment, a string or an unrelated identifier does not count."""
    escaped = re.escape(name)
    return bool(
        re.search(
            rf"(?:\.{escaped}\b|(?<![\w$.]){escaped}\??\s*:(?!:)|\[\s*[\"']{escaped}[\"']\s*\])",
            text,
        )
    )


def main() -> int:
    text = shared_text()
    if not text:
        print("skipped: the shared preferences and settings page are not present")
        return 0
    unmapped = sorted(template_keys() - MAPPING.keys())
    for key in unmapped:
        print(
            f"{key} is in the Windows template but not in MAPPING: record the field that answers it, "
            f"or `!kind: why` if nothing does.",
            file=sys.stderr,
        )
    if unmapped:
        return 1

    orphaned = []
    structs = rust_structs()
    for key, target in sorted(MAPPING.items()):
        if target.startswith("!"):
            continue
        problem = resolve(target, structs)
        if problem:
            orphaned.append(f"{key} -> {target} ({problem})")

    # And a field alone is not the setting. This client's arrangement is that shared behaviour and
    # its interface both live in the Tauri settings page, so a reference setting with a field here
    # and no mention at all on that page is supported on paper only. The check above is satisfied
    # by `preferences.rs` on its own, which is exactly the state that hides such a setting.
    #
    # What this can see is the field's name appearing in the page's source, which is weaker than
    # "a control is rendered for it": a type declaration alone would satisfy it. It catches the
    # failure that actually happens - a preference added to the crate and nothing done in the page
    # - and not a control deleted while its type stays. Reported wording says only that much.
    unreachable = []
    page_source = page_text()
    for key, target in sorted(MAPPING.items()):
        if target.startswith("!") or target in PLATFORM_LOCAL:
            continue
        # Every field segment has to be spelled as a field, so `floating_toolbar.enabled` needs both `floating_toolbar` and `enabled`; a root struct name is not a field and is skipped.
        segments = [segment for segment in target.split(".") if segment not in structs]
        if not all(page_uses(segment, page_source) for segment in segments):
            unreachable.append(f"{key} -> {target}")

    # A declaration is not code. A field can be added to the page's types and defaults and then
    # touched by nothing, which looks the same to a name search as a setting that works.
    #
    # This says "nothing reads or writes it", not "no control renders it", and the difference is
    # real: deleting the fallback-font editor from its component leaves this quiet, because
    # `candidate-font-family.ts` and `resolved-candidate-fonts.ts` still read the field for
    # validation and font resolution. Measured, not assumed. Telling a control apart from a helper
    # needs the JSX parsed, which is more machinery than this check is worth; what it catches is a
    # field declared and wired to nothing at all.
    declaration_only = []
    page = page_text().splitlines()
    for key, target in sorted(MAPPING.items()):
        name = target.split(".")[-1]
        if target.startswith("!") or target in PLATFORM_LOCAL or name in COMPOSED_AT_RUNTIME:
            continue
        mentions = [line for line in page if re.search(rf"\b{re.escape(name)}\b", line)]
        if mentions and all(
            re.fullmatch(rf"{re.escape(name)}\??:\s*[^=]+;", line.strip())
            or re.fullmatch(rf"{re.escape(name)}:\s*.+,", line.strip())
            for line in mentions
        ):
            declaration_only.append(f"{key} -> {target}")

    for entry in orphaned:
        print(f"the shared layer no longer has the target for {entry}", file=sys.stderr)
    for entry in declaration_only:
        print(
            f"{entry} appears in the shared settings page only as a declaration or a default: "
            f"nothing in the shared tree reads or writes it.",
            file=sys.stderr,
        )
    unreachable += declaration_only
    for entry in unreachable:
        print(
            f"{entry} has a field but its name appears nowhere in the shared settings page, so "
            f"nothing there can be reaching it. Add the control, or record it in PLATFORM_LOCAL "
            f"with the reason.",
            file=sys.stderr,
        )
    if unreachable:
        return 1
    if orphaned:
        print(
            "\nA renamed field needs its entry updated. A setting the reference has *added* is "
            "test-windows-config-keys.py's half of this question, not this one's.",
            file=sys.stderr,
        )
        return 1

    reasons = sum(1 for target in MAPPING.values() if target.startswith("!"))
    print(
        f"reference config coverage: {len(MAPPING)} settings mapped "
        f"({reasons} to a reason rather than a field), every target present"
    )
    print(
        f"  and named in the shared settings page, bar {len(PLATFORM_LOCAL)} recorded as "
        f"platform-local"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
