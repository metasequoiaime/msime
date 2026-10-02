"""Keep platform factory configuration aligned with shared client defaults."""

import re
import tomllib
from pathlib import Path

from reference_source import reference_root, show_file


ROOT = Path(__file__).resolve().parent.parent
CORE_PREFERENCES = ROOT / "crates/client-core/src/preferences.rs"
WINDOWS_DEFAULTS = ROOT / "platforms/windows/installer/config.default.toml"
REFERENCE_CONFIG = "installer/default_config/config.default.toml"

core_source = CORE_PREFERENCES.read_text(encoding="utf-8")
mixed_input_default = re.search(
    r"impl Default for MixedInputPreferences\s*\{.*?minimum_prefix:\s*(\d+)",
    core_source,
    re.DOTALL,
)
assert mixed_input_default, "MixedInputPreferences default was not found"

with WINDOWS_DEFAULTS.open("rb") as config_file:
    windows_defaults = tomllib.load(config_file)

shared_minimum_prefix = int(mixed_input_default.group(1))
windows_minimum_prefix = windows_defaults["general"]["cn_en_mixed_input_min_chars"]
assert windows_minimum_prefix == shared_minimum_prefix, (
    "Windows cn_en_mixed_input_min_chars must match the shared default "
    f"({shared_minimum_prefix}): {windows_minimum_prefix}"
)
reference = show_file(ROOT, REFERENCE_CONFIG)
if reference is None:
    print(
        "skipped the mixed-input default reference comparison: no MSIME-Windows checkout at "
        f"{reference_root(ROOT)}"
    )
else:
    reference_text, reference_ref, reference_sha = reference
    reference_defaults = tomllib.loads(reference_text)
    reference_minimum_prefix = reference_defaults["general"]["cn_en_mixed_input_min_chars"]
    assert shared_minimum_prefix == reference_minimum_prefix, (
        "MixedInputPreferences minimum_prefix must match the fixed Windows source "
        f"({reference_minimum_prefix}): {shared_minimum_prefix}"
    )
    assert windows_minimum_prefix == reference_minimum_prefix, (
        "Windows cn_en_mixed_input_min_chars must match the fixed Windows source "
        f"({reference_minimum_prefix}): {windows_minimum_prefix}"
    )
    print(
        "Windows and shared mixed-input minimum prefix match "
        f"{reference_ref} ({reference_sha[:8]}): {reference_minimum_prefix}"
    )
    reference_mixed_emoji = reference_defaults["general"]["emoji_mixed_input"]
    assert windows_defaults["general"]["emoji_mixed_input"] == reference_mixed_emoji, (
        "Windows emoji_mixed_input must match the fixed Windows source "
        f"({reference_mixed_emoji}): {windows_defaults['general']['emoji_mixed_input']}"
    )

# Emoji mixed-input ships on in the source and in the Windows template. The running host reads the shared document, so the shared default has to say the same on Windows and on macOS, which follows the source; kaomoji stays off everywhere.
assert windows_defaults["general"]["emoji_mixed_input"] is True, (
    "the Windows template must ship emoji_mixed_input on, as the source does"
)
mixed_emoji_field = re.search(
    r"impl Default for MixedInputPreferences\s*\{.*?emoji:\s*([^,]+),.*?kaomoji:\s*([^,]+),",
    core_source,
    re.DOTALL,
)
assert mixed_emoji_field, "MixedInputPreferences emoji default was not found"
assert mixed_emoji_field.group(1) == "source_mixed_emoji_default()", (
    "MixedInputPreferences::default() must take emoji from source_mixed_emoji_default, not: "
    f"{mixed_emoji_field.group(1)}"
)
assert mixed_emoji_field.group(2) == "false", "kaomoji mixed-input must default to off"
source_mixed_emoji_default = re.search(
    r"fn source_mixed_emoji_default\(\) -> bool \{\s*(.+?)\s*\}",
    core_source,
    re.DOTALL,
)
assert source_mixed_emoji_default, "source_mixed_emoji_default was not found"
assert source_mixed_emoji_default.group(1) == 'cfg!(any(windows, target_os = "macos"))', (
    "the shared first-run default for emoji mixed-input must be on for Windows and macOS and "
    f"unchanged elsewhere, not: {source_mixed_emoji_default.group(1)}"
)
print("Windows and macOS ship emoji mixed-input on, as the source does")

voice_auth_field = re.search(
    r"impl Default for VoiceInputPreferences\s*\{.*?doubao_auth_mode:\s*default_doubao_auth_mode\(\)",
    core_source,
    re.DOTALL,
)
assert voice_auth_field, "VoiceInputPreferences::default() must take doubao_auth_mode from default_doubao_auth_mode"
voice_auth_default = re.search(
    r'fn default_doubao_auth_mode\(\) -> String \{\s*"([^"]+)"\.to_owned\(\)\s*\}',
    core_source,
)
assert voice_auth_default, "default_doubao_auth_mode was not found"

shared_voice_auth_mode = voice_auth_default.group(1)
windows_voice_auth_mode = windows_defaults["voice_input"]["doubao_auth_mode"]
assert windows_voice_auth_mode == shared_voice_auth_mode, (
    "Windows doubao_auth_mode does not match VoiceInputPreferences::default(): "
    f"{windows_voice_auth_mode} != {shared_voice_auth_mode}"
)

print(f"Windows Doubao auth mode matches the shared default: {shared_voice_auth_mode}")

fallback_fonts_default = re.search(
    r"fn default_candidate_fallback_fonts\(\)\s*->\s*Vec<String>\s*\{(.*?)\}",
    core_source,
    re.DOTALL,
)
assert fallback_fonts_default, "candidate fallback font defaults were not found"

shared_fallback_fonts = re.findall(
    r'"([^"]+)"\.to_owned\(\)', fallback_fonts_default.group(1)
)
windows_fallback_fonts = windows_defaults["appearance"]["fallback_fonts"]
assert windows_fallback_fonts == shared_fallback_fonts, (
    "Windows fallback_fonts do not match the shared candidate font defaults: "
    f"{windows_fallback_fonts} != {shared_fallback_fonts}"
)

print(f"Windows fallback fonts match the shared defaults: {shared_fallback_fonts}")

translation_target_default = re.search(
    r"pub enum TranslationTargetLanguage\s*\{.*?#\[default\]\s*([A-Za-z0-9_]+)",
    core_source,
    re.DOTALL,
)
assert translation_target_default, "translation target language default was not found"

shared_translation_target = translation_target_default.group(1).lower()
windows_translation_target = windows_defaults["tencent_tmt"]["target_language"]
assert windows_translation_target == shared_translation_target, (
    "Windows target_language does not match TranslationTargetLanguage::default(): "
    f"{windows_translation_target} != {shared_translation_target}"
)

custom_translation_default = re.search(
    r"#\[derive\([^)]*Default[^)]*\)\]\s*"
    r"pub struct CustomTranslationPreferences\s*\{(.*?)\}",
    core_source,
    re.DOTALL,
)
assert custom_translation_default, "CustomTranslationPreferences derived default was not found"

custom_fields = re.findall(
    r"pub\s+([A-Za-z0-9_]+):\s*(bool|String),", custom_translation_default.group(1)
)
all_custom_fields = re.findall(
    r"pub\s+[A-Za-z0-9_]+\s*:", custom_translation_default.group(1)
)
assert len(custom_fields) == len(all_custom_fields), (
    "CustomTranslationPreferences has a field whose derived default is not supported"
)
shared_custom_translation = {
    name: False if field_type == "bool" else "" for name, field_type in custom_fields
}
windows_custom_translation = windows_defaults["custom_translation"]
assert windows_custom_translation == shared_custom_translation, (
    "Windows custom_translation does not match CustomTranslationPreferences::default(): "
    f"{windows_custom_translation} != {shared_custom_translation}"
)

print(
    "Windows translation target and custom provider match the shared defaults: "
    f"{shared_translation_target}, {shared_custom_translation}"
)

mouse_wheel_default = re.search(
    r"impl Default for NavigationPreferences\s*\{.*?mouse_wheel:\s*(true|false)",
    core_source,
    re.DOTALL,
)
assert mouse_wheel_default, "NavigationPreferences mouse-wheel default was not found"

shared_mouse_wheel = mouse_wheel_default.group(1) == "true"
windows_mouse_wheel = windows_defaults["general"]["paging_mouse_wheel"]
assert windows_mouse_wheel == shared_mouse_wheel, (
    "Windows paging_mouse_wheel does not match NavigationPreferences::default(): "
    f"{windows_mouse_wheel} != {shared_mouse_wheel}"
)

print(f"Windows mouse-wheel paging matches the shared default: {shared_mouse_wheel}")

fuzzy_rule_enum = re.search(
    r"pub enum FuzzyPinyinRule\s*\{(.*?)\n\}", core_source, re.DOTALL
)
assert fuzzy_rule_enum, "FuzzyPinyinRule identifiers were not found"
shared_fuzzy_rule_ids = re.findall(
    r'#\[serde\(rename = "([^"]+)"\)\]', fuzzy_rule_enum.group(1)
)
assert shared_fuzzy_rule_ids, "FuzzyPinyinRule has no serialized identifiers"

fuzzy_preferences_default = re.search(
    r"#\[derive\([^)]*Default[^)]*\)\]\s*"
    r"#\[serde\(deny_unknown_fields\)\]\s*"
    r"pub struct FuzzyPinyinPreferences\s*\{(.*?)\}",
    core_source,
    re.DOTALL,
)
assert fuzzy_preferences_default, "FuzzyPinyinPreferences derived default was not found"
fuzzy_fields = fuzzy_preferences_default.group(1)
assert re.search(r"pub enabled:\s*bool", fuzzy_fields)
assert re.search(r"pub rules:\s*BTreeSet<FuzzyPinyinRule>", fuzzy_fields)
assert re.search(r"pub seeded:\s*bool", fuzzy_fields)

windows_input = windows_defaults["input"]
assert windows_input["fuzzy_pinyin"] is False, "Windows fuzzy-pinyin default must be disabled"
assert windows_input["fuzzy_seeded"] is False, "Windows fuzzy-pinyin seed marker must be false"
windows_fuzzy_rules = {
    key.removeprefix("fuzzy_").replace("_", "-"): value
    for key, value in windows_input.items()
    if key.startswith("fuzzy_") and key not in {"fuzzy_pinyin", "fuzzy_seeded"}
}
assert set(windows_fuzzy_rules) == set(shared_fuzzy_rule_ids), (
    "Windows fuzzy-pinyin rule keys do not match shared rule identifiers: "
    f"{sorted(windows_fuzzy_rules)} != {sorted(shared_fuzzy_rule_ids)}"
)
assert not any(windows_fuzzy_rules.values()), "Windows fuzzy-pinyin rules must default to off"

print(
    "Windows fuzzy-pinyin factory keys match the shared disabled default: "
    f"{len(shared_fuzzy_rule_ids)} rules"
)

# Smart punctuation rewrites a character the user already saw land, so the source ships all five switches off. The installed TOML is only a template — the running host reads the shared preferences document — so the two have to be checked against each other rather than assumed to agree. macOS ports the same desktop product and follows it; the other hosts keep what they have shipped.
smart_punctuation_default = re.search(
    r"fn smart_punctuation_default\(\) -> bool \{\s*(.+?)\s*\}",
    core_source,
    re.DOTALL,
)
assert smart_punctuation_default, "smart_punctuation_default was not found"
assert smart_punctuation_default.group(1) == '!cfg!(any(windows, target_os = "macos"))', (
    "the shared first-run default for smart punctuation must be off on Windows and macOS "
    f"and unchanged elsewhere, not: {smart_punctuation_default.group(1)}"
)

windows_smart_punctuation = {
    key: value
    for key, value in windows_defaults["input"].items()
    if key.startswith("smart_punctuation")
}
assert len(windows_smart_punctuation) == 5, (
    "the Windows template must ship all five smart-punctuation switches: "
    f"{sorted(windows_smart_punctuation)}"
)
assert not any(windows_smart_punctuation.values()), (
    "Windows smart-punctuation switches must default to off: "
    f"{sorted(key for key, value in windows_smart_punctuation.items() if value)}"
)
# The space rewrite has no equivalent in the reference and changes a character the user already saw land, so it is off until asked for. The two halves of 智能标点 follow the parent instead: the reference has one switch there and this page shows its description verbatim - ASCII after a letter or a digit - so halves that were off on their own left the parent on and doing nothing. Following the parent keeps a fresh Windows or macOS profile with the whole family off, which is what the template above ships and what this check exists for.
assert re.search(
    r"#\[serde\(default\)\]\s*pub smart_punctuation_space_convert: bool", core_source
), "smart_punctuation_space_convert must default to off on every host, matching the template"
for field in ("smart_punctuation_direct_digit", "smart_punctuation_direct_letter"):
    assert re.search(
        rf'#\[serde\(default = "smart_punctuation_default"\)\]\s*(?:///[^\n]*\n\s*)*pub {field}: bool',
        core_source,
    ), f"{field} must follow the 智能标点 default, which is off on Windows and macOS"

print(
    "Windows and macOS smart punctuation is off on a fresh profile, as the template ships: "
    f"{len(windows_smart_punctuation)} switches"
)

# Muting, DDC and text polishing ship on in the source and in the Windows template. The running host reads the shared document, not the template, so the shared default has to say the same on Windows - and on macOS, which follows the source - or the template's value never reaches a fresh profile.
source_voice_default = re.search(
    r"fn source_voice_default\(\) -> bool \{\s*(.+?)\s*\}",
    core_source,
    re.DOTALL,
)
assert source_voice_default, "source_voice_default was not found"
assert source_voice_default.group(1) == 'cfg!(any(windows, target_os = "macos"))', (
    "the shared first-run default for the source-on voice switches must be on for Windows and "
    f"macOS and unchanged elsewhere, not: {source_voice_default.group(1)}"
)
source_voice_switches = ("mute_system_audio", "doubao_enable_ddc", "polish_text")
for field in source_voice_switches:
    assert windows_defaults["voice_input"][field] is True, (
        f"the Windows template must ship voice_input.{field} on, as the source does"
    )
    assert re.search(
        rf'#\[serde\(default = "source_voice_default"\)\]\s*pub {field}: bool', core_source
    ), f"voice_input.{field} must use source_voice_default for a missing key"
    assert f"{field}: source_voice_default()," in core_source, (
        f"VoiceInputPreferences::default() must take {field} from source_voice_default"
    )
print(f"Windows and macOS ship {len(source_voice_switches)} voice switches on, as the source does")

# The polish service and the AI assistant ship pointed at DeepSeek in the source and in the Windows template, with the assistant on. As with the voice switches the running host reads the shared document, so the shared first-run default has to carry the same values on Windows and macOS; the other hosts keep SiliconFlow/Qwen and the assistant off.
source_service = {
    ("voice_input", "polish_provider"): "deepseek",
    ("voice_input", "polish_endpoint"): "https://api.deepseek.com/chat/completions",
    ("voice_input", "polish_model"): "deepseek-v4-flash",
    ("ai_assistant", "enabled"): True,
    ("ai_assistant", "provider"): "deepseek",
    ("ai_assistant", "endpoint"): "https://api.deepseek.com/chat/completions",
    ("ai_assistant", "model"): "deepseek-v4-flash",
}
if reference is not None:
    for (section, key), expected in source_service.items():
        assert reference_defaults[section][key] == expected, (
            f"the fixed Windows source changed {section}.{key}: "
            f"{reference_defaults[section][key]!r}, expected {expected!r}"
        )
for (section, key), expected in source_service.items():
    assert windows_defaults[section][key] == expected, (
        f"the Windows template must ship {section}.{key} = {expected!r}, as the source does: "
        f"{windows_defaults[section][key]!r}"
    )
source_ai_default = re.search(
    r"fn source_ai_default\(\) -> bool \{\s*(.+?)\s*\}", core_source, re.DOTALL
)
assert source_ai_default, "source_ai_default was not found"
assert source_ai_default.group(1) == 'cfg!(any(windows, target_os = "macos"))', (
    "the AI assistant first-run default must be on for Windows and macOS and unchanged "
    f"elsewhere, not: {source_ai_default.group(1)}"
)
assert re.search(
    r'#\[serde\(default = "source_ai_default"\)\]\s*pub enabled: bool', core_source
), "ai_assistant.enabled must use source_ai_default for a missing key"
assert "enabled: source_ai_default()," in core_source, (
    "AiAssistantPreferences::default() must take enabled from source_ai_default"
)
assert re.search(
    r'const SOURCE_DEEPSEEK_ENDPOINT: &str = "https://api\.deepseek\.com/chat/completions";',
    core_source,
), "the shared DeepSeek endpoint must match the source template"
assert re.search(
    r'const SOURCE_DEEPSEEK_MODEL: &str = "deepseek-v4-flash";', core_source
), "the shared DeepSeek model must match the source template"
polish_service = re.search(
    r"fn default_polish_service\(\) -> PolishService \{\s*if source_voice_default\(\) \{\s*"
    r'PolishService \{\s*provider: "deepseek",\s*endpoint: SOURCE_DEEPSEEK_ENDPOINT,\s*'
    r"model: SOURCE_DEEPSEEK_MODEL,",
    core_source,
)
assert polish_service, (
    "default_polish_service must pick DeepSeek with the source endpoint and model on the desktop ports"
)
ai_service = re.search(
    r"impl Default for AiAssistantPreferences \{.*?if source_ai_default\(\) \{\s*"
    r"\(SOURCE_DEEPSEEK_ENDPOINT, SOURCE_DEEPSEEK_MODEL\)",
    core_source,
    re.DOTALL,
)
assert ai_service, (
    "AiAssistantPreferences::default() must ship the source endpoint and model on the desktop ports"
)
assert 'provider: "deepseek".into(),' in core_source, "the AI assistant provider must default to deepseek"
print("Windows and macOS ship DeepSeek polishing and the AI assistant on, as the source does")

# Statistics count what a person types, so the template and the shared default have to agree that
# they start off. The reference says so in its own feature list, and a template that shipped them
# on would turn them on for every fresh profile regardless of what the shared code says.
statistics = windows_defaults["statistics"]
assert statistics["enabled"] is False, "the statistics template must ship them off"
assert re.search(
    r"fn enabled_by_default\(\) -> bool \{\s*(?:///[^\n]*\n\s*)*false\s*\}",
    (ROOT / "crates/client-core/src/typing_statistics.rs").read_text(encoding="utf-8"),
), "the shared statistics default must be off, matching the template"

# An unrecognised retention is read as forever. The template must name one the code knows, or the
# value it ships would silently mean something other than what it says.
retentions = {"forever", "30d", "90d", "180d", "365d"}
assert statistics["retention"] in retentions, (
    f"unknown statistics retention {statistics['retention']!r}; "
    f"the shared store understands {sorted(retentions)}"
)
statistics_source = (ROOT / "crates/client-core/src/typing_statistics.rs").read_text(
    encoding="utf-8"
)
for spelling in retentions - {"forever"}:
    assert f'"{spelling}" =>' in statistics_source, (
        f"the template offers {spelling} but the shared store does not parse it"
    )

print(f"Windows statistics ship off with retention {statistics['retention']!r}")

# The source starts a fresh install in English: its factory template says so, and ours ships the same line. The running Windows host reads the shared document instead, so its default has to be English on Windows too, while every other host keeps Chinese.
windows_ime_mode = windows_defaults["input"]["default_ime_mode"]
assert windows_ime_mode == "english", (
    f"the Windows template must start in English, as the source does: {windows_ime_mode!r}"
)
if reference is not None:
    reference_ime_mode = tomllib.loads(reference[0])["input"]["default_ime_mode"]
    assert windows_ime_mode == reference_ime_mode, (
        "Windows default_ime_mode must match the fixed Windows source "
        f"({reference_ime_mode!r}): {windows_ime_mode!r}"
    )
ime_mode_default = re.search(
    r"impl Default for DefaultImeMode \{\s*fn default\(\) -> Self \{\s*(.+?)\s*\}\s*\}",
    core_source,
    re.DOTALL,
)
assert ime_mode_default, (
    "DefaultImeMode must have an explicit Default impl; a derived #[default] cannot differ by platform"
)
assert re.fullmatch(
    r"if cfg!\(windows\) \{\s*Self::English\s*\} else \{\s*Self::Chinese",
    ime_mode_default.group(1),
), f"DefaultImeMode::default() must be English on Windows and Chinese elsewhere, not: {ime_mode_default.group(1)}"
assert "default_ime_mode: DefaultImeMode::default()," in core_source, (
    "Preferences::default() must take default_ime_mode from DefaultImeMode::default()"
)
print("Windows starts a fresh install in English, as the source does; other hosts start in Chinese")
