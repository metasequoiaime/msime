#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
# Several guards below are "fail if rg finds this". Without rg each of those exits 127, which the if reads as "not found", so a missing binary would pass every one of them silently.
if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) is required by the Android host contract checks" >&2
  exit 1
fi
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
if [[ -z "$android_sdk" ]]; then
  echo "Set ANDROID_SDK_ROOT to an installed Android SDK" >&2
  exit 1
fi
android_jar="$android_sdk/platforms/android-35/android.jar"
if [[ ! -f "$android_jar" ]]; then
  echo "Android API 35 platform is required" >&2
  exit 1
fi
output_dir=$(mktemp -d)
trap 'rm -f "$output_dir/manifest.apk" "$output_dir/resources.zip"; find "$output_dir" -name "*.class" -delete; find "$output_dir" -depth -type d -empty -delete' EXIT
# Command 9 was unmapped when this guard was added; it is now Action::Finish in
# crates/host-api/src/ffi/input.rs, and the declined-punctuation path needs it.
# What must not come back is the literal, which is how the unmapped call got in.
if rg -n 'NativeClient\.command\([^,]+, 9\)' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android input service must name command 9 (FINISH_COMPOSITION_COMMAND), not inline it" >&2
  exit 1
fi
if ! rg -q 'msime_client_command' "$repo_root/crates/host-api/src/ffi/input.rs" \
  || ! rg -q '^\s*9 => Action::Finish,' "$repo_root/crates/host-api/src/ffi/input.rs"; then
  echo "Shared host command 9 is no longer Action::Finish; FINISH_COMPOSITION_COMMAND is stale" >&2
  exit 1
fi
# Japanese kana variants are Engine state, not a host-maintained lookup table. Keep the named
# Android command and its shared FFI mapping together so a future enum change cannot silently
# turn the visible 小゛゜ key into a no-op.
if rg -n 'command\(10\)' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android input service must name command 10 (CYCLE_KANA_VARIANT_COMMAND), not inline it" >&2
  exit 1
fi
if ! rg -q '^\s*10 => Action::Command\(Command::CycleKanaVariant\),' \
    "$repo_root/crates/host-api/src/ffi/input.rs"; then
  echo "Shared host command 10 is no longer CycleKanaVariant; Android command is stale" >&2
  exit 1
fi
if rg -n 'VariantGroup|showJapaneseVariants' \
    "$repo_root/platforms/android/java/app/msime/android/keyboard/JapaneseNineKeyLayout.java" \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android must not duplicate Engine-owned Japanese kana variant tables" >&2
  exit 1
fi
# Hangul composition is Engine state. Android labels the Dubeolsik keys and sends their ASCII letters; a syllable table here would be a second automaton that can drift from the Engine's.
if ! rg -q '4 korean' "$repo_root/crates/host-api/include/msime_client.h" \
  || ! rg -q 'KOREAN_SCHEME = 4;' \
    "$repo_root/platforms/android/java/app/msime/android/policy/KoreanInputPolicy.java"; then
  echo "Android KOREAN_SCHEME no longer matches the shared View.scheme ordinal" >&2
  exit 1
fi
if rg -n -i '0xac00|44032|0x3131|12593' "$repo_root/platforms/android/java/app/msime/android"; then
  echo "Android must not compose Hangul syllables itself; the Engine owns the Korean automaton" >&2
  exit 1
fi
# 粤拼、注音、越南语、藏文和笔画是共享头文件里的 View.scheme 5、6、7、8、9；InputSchemeTraits 用同样的序号命名，本宿主每个按方案决定的判断都从那里读取。
for pair in '5 cantonese:CANTONESE = 5;' '6 zhuyin:ZHUYIN = 6;' '7 vietnamese:VIETNAMESE = 7;' '8 tibetan:TIBETAN = 8;' '9 stroke:STROKE = 9;'; do
  if ! rg -qF "${pair%%:*}" "$repo_root/crates/host-api/include/msime_client.h" \
    || ! rg -qF "${pair#*:}" "$repo_root/platforms/android/java/app/msime/android/policy/InputSchemeTraits.java"; then
    echo "Android InputSchemeTraits no longer matches the shared View.scheme ordinal ${pair%%:*}" >&2
    exit 1
  fi
done
# 藏文的 EWTS 转换同样是 Engine 的状态（ewts crate）。Android 只发送拉丁字母和拼写符号，宿主里出现藏文码位（Java 转义或字面字符）就意味着多了一张会和 Engine 走偏的转换表或音节点、垂符的自行插入。
if rg -n -i '\\u0f[0-9a-f]{2}|[\x{0F00}-\x{0FFF}]' "$repo_root/platforms/android/java/app/msime/android"; then
  echo "Android must not transliterate Wylie or insert tsheg and shad itself; the Engine owns the Tibetan converter" >&2
  exit 1
fi
# The Zhuyin list opens with MSIME_OPEN_CANDIDATE_LIST, the Hanja command under its general name; the Android constant aliases the Korean one so the two cannot drift apart.
if ! rg -q 'MSIME_OPEN_CANDIDATE_LIST = 16,' "$repo_root/crates/host-api/include/msime_client.h" \
  || ! rg -q 'OPEN_CANDIDATE_LIST_COMMAND = KoreanInputPolicy\.CONVERT_HANJA_COMMAND;' \
    "$repo_root/platforms/android/java/app/msime/android/policy/ZhuyinInputPolicy.java"; then
  echo "Android OPEN_CANDIDATE_LIST_COMMAND no longer matches the shared Host API command 16" >&2
  exit 1
fi
# Bopomofo composition is Engine state too. Android labels the Dachen keys and sends their ASCII keys; a syllable or phrase table here would be a second editor that can drift from the Engine's.
if rg -n 'U\+3105|0x3105|12549' "$repo_root/platforms/android/java/app/msime/android"; then
  echo "Android must not compose bopomofo itself; the Engine owns the Zhuyin editor" >&2
  exit 1
fi
# 笔画的组字与查字都在 Engine 里：宿主只把笔画键印成字形并发送字母 h s p n z x，不保存笔顺码表，也不自己打开 msime-stroke.db（它只用文件名判断方案是否可用）。
if rg -n '"[hspnzx]{3,}"' "$repo_root/platforms/android/java/app/msime/android" \
  || rg -n '"stroke\.db"' "$repo_root/platforms/android/java/app/msime/android" \
    | rg -v '/keyboard/KeyboardScheme\.java:'; then
  echo "Android must not look up strokes itself; the Engine owns the Stroke scheme and msime-stroke.db" >&2
  exit 1
fi
# The Stroke inline composition is the glyphs in View.reading, marked through the same policy as Korean and Zhuyin; editing_text holds only the stroke letters.
if ! rg -q 'StrokeInputPolicy\.active' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android must mark the Stroke composition's reading through StrokeInputPolicy" >&2
  exit 1
fi
# The Hanja command is shared command 16 at both ends: the header's MSIME_CONVERT_HANJA, the FFI's ConvertHanja and this host's named constant must agree, and the service reaches it by name so a renumbering cannot leave a bare 16 behind.
if ! rg -q 'MSIME_CONVERT_HANJA = 16,' "$repo_root/crates/host-api/include/msime_client.h" \
  || ! rg -q '^\s*16 => Action::Command\(Command::ConvertHanja\),' "$repo_root/crates/host-api/src/ffi/input.rs" \
  || ! rg -q 'CONVERT_HANJA_COMMAND = 16;' \
    "$repo_root/platforms/android/java/app/msime/android/policy/KoreanInputPolicy.java"; then
  echo "Android CONVERT_HANJA_COMMAND no longer matches the shared Host API command 16" >&2
  exit 1
fi
if rg -n 'command\((session, )?16\)' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'command\(KoreanInputPolicy\.CONVERT_HANJA_COMMAND\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android input service must send the Hanja command as KoreanInputPolicy.CONVERT_HANJA_COMMAND" >&2
  exit 1
fi
# The Korean inline composition is the Hangul in View.reading; editing_text holds only the key letters.
if ! rg -q 'KoreanInputPolicy\.composing' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android must mark the Korean composition through KoreanInputPolicy" >&2
  exit 1
fi
# Hardware navigation must use the shared command numbers through one named policy. Keep the
# service from growing another inline key-code table that can drift from the FFI mapping.
if ! rg -q 'HardwareKeyPolicy\.commandFor' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android hardware navigation must route through HardwareKeyPolicy" >&2
  exit 1
fi
if ! rg -q 'KEYCODE_FORWARD_DEL.*-> 8' \
    "$repo_root/platforms/android/java/app/msime/android/policy/HardwareKeyPolicy.java" \
    || ! rg -q '^\s*8 => Action::Command\(Command::DeleteForward\),' \
    "$repo_root/crates/host-api/src/ffi/input.rs"; then
  echo "Android forward-delete mapping no longer matches the shared Host API" >&2
  exit 1
fi
# Double-pinyin labels belong to the Engine profile tables. Android may gate visibility and
# decode the bounded response, but it must not carry a second profile keymap that can drift.
if rg -n 'PROFILES|uai=k|ing=;' \
    "$repo_root/platforms/android/java/app/msime/android/keyboard/ShuangpinKeyHintPolicy.java"; then
  echo "Android must not duplicate Engine-owned double-pinyin profile tables" >&2
  exit 1
fi
if ! rg -q 'shuangpinKeyHintsRaw' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java" \
    || ! rg -q 'msime_client_shuangpin_key_hints' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android double-pinyin hints must cross the shared Host API through JNI" >&2
  exit 1
fi
# Smart-punctuation repeat/space decisions belong to the shared Host API. Android may hold
# editor-scoped snapshots, but must not reimplement timing or replacement rules locally.
if ! rg -q 'smartPunctuationArmRaw|smartPunctuationDecideRaw' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java" \
    || ! rg -q 'msime_client_smart_punctuation_(arm|decide)' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android smart punctuation must cross the shared Host API through JNI" >&2
  exit 1
fi
# 「自动补全成对标点」曾经只是设置页上的一个开关：这个宿主从不读 paired_punctuation，开着也只上屏半个括号（#5608）。键盘标点键的补全规则与 iOS、HarmonyOS 同一份（PairedPunctuationPolicy.completion），符号面板走 symbolClosing，在面板里轻点自动补上的后半个要跨过它（stepOverPairedSymbol），否则面板里补出（|）再点 ）会多一个；补完书名号要经 JNI 通知 Engine 平衡嵌套。
if ! rg -q 'optBoolean\("paired_punctuation"' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'PairedPunctuationPolicy\.completion\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'PairedPunctuationPolicy\.symbolClosing\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'stepOverPairedSymbol\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'stepOverSymbol\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'msime_client_balance_paired_punctuation_after_auto_close' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android must honour the shared paired_punctuation preference on punctuation keys and in the symbol panel" >&2
  exit 1
fi
# 候选带 emoji / 颜文字是 Engine 已有的混输（共享偏好 mixed_input.emoji / kaomoji），Android 设置曾经没有开关（#5667）。`MixedInputPreferences` 四个字段都必填，只写一个字段的对象会让整份偏好被拒绝，所以写之前要补齐。
expression_page="$repo_root/platforms/android/java/app/msime/android/home/ExpressionPage.java"
for field in english minimum_prefix emoji kaomoji; do
  if ! rg -q "mixed\.has\(\"$field\"\)" "$expression_page"; then
    echo "Android expression page must fill mixed_input.$field before writing the object back" >&2
    exit 1
  fi
done
if ! rg -q 'mixedInput\(edit\)\.put\("emoji"' "$expression_page" \
  || ! rg -q 'mixedInput\(edit\)\.put\("kaomoji"' "$expression_page"; then
  echo "Android expression page must offer the shared emoji and kaomoji candidate switches" >&2
  exit 1
fi
# The fullwidth state belongs to the runtime, not to a private SharedPreferences file: the Engine
# widens what it commits, and it can only do that if the host has told it the width. The second
# guard is the reason the first one matters - this host used to keep its own latch, and the shared
# settings page's 全角输入 switch did nothing here at all.
if ! rg -q 'setCharacterWidthRaw' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java" \
  || ! rg -q 'msime_client_set_character_width' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android fullwidth input must cross the shared Host API through JNI" >&2
  exit 1
fi
if rg -n 'full-width-input|keyboardLayoutPreferences' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android must read the fullwidth state from the shared preference, not a private store" >&2
  exit 1
fi
# 以词定字 belongs to the Engine: it picks the Han character and decides whether the candidate has
# one at all. This host may route the key and commit the fallback the source's host commits, but it
# must not grow its own idea of which candidates qualify.
if ! rg -q 'selectEdgeRaw' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java" \
  || ! rg -q 'msime_client_select_edge' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android word-to-character must cross the shared Host API through JNI" >&2
  exit 1
fi
# Candidate paging keys are a user preference, not a fixed table: the source lets the pair be
# chosen and this host reads the same shared `navigation` document the desktop hosts do. Keeping
# the routing in one named policy is what stops a second, drifting key table growing in the service.
if ! rg -q 'CandidateNavigationPolicy\.commandFor' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android candidate paging must route through CandidateNavigationPolicy" >&2
  exit 1
fi
# A key code written as a bare number reads correctly to everybody and is only ever falsified by
# pressing the key. One of them was wrong for as long as this host has had hardware chords: 简繁 sat
# on 33, which is KEYCODE_E, while the preference is named ..._ctrl_shift_f and the settings page
# promises Ctrl+Shift+F. Comparisons in these files name their key.
for policy in HardwareShortcutPolicy HardwareKeyPolicy NumberRowSelectionPolicy CandidateNavigationPolicy; do
  source_file="$repo_root/platforms/android/java/app/msime/android/policy/$policy.java"
  [[ -f "$source_file" ]] || source_file="$repo_root/platforms/android/java/app/msime/android/keyboard/$policy.java"
  if rg -q '(key|keyCode|keycode)\s*(==|>=|<=|>|<)\s*[0-9]+' "$source_file"; then
    echo "Android $policy compares a key code against a bare number; name it with KeyEvent" >&2
    exit 1
  fi
done
# Both ends of the first/last-candidate commands, so neither side can drift alone: the shared header
# owns the numbers and this host must reach them by name rather than by repeating them at a call site.
if ! rg -q 'MSIME_FIRST_CANDIDATE = 104, MSIME_LAST_CANDIDATE = 105' \
    "$repo_root/crates/host-api/include/msime_client.h" \
  || ! rg -q 'FIRST_CANDIDATE = 104' \
    "$repo_root/platforms/android/java/app/msime/android/policy/CandidateNavigationPolicy.java" \
  || ! rg -q 'LAST_CANDIDATE = 105' \
    "$repo_root/platforms/android/java/app/msime/android/policy/CandidateNavigationPolicy.java"; then
  echo "Android Home/End must map to the shared first/last candidate commands" >&2
  exit 1
fi
# 「候选栏预编辑」 governs the spelling only. The chosen part of a phrase is held out of the
# document at this host's request, so it must keep being drawn whatever the setting says - that is
# the same half-state scripts/test-phrase-preedit-hosts.py guards from the other side.
if ! rg -q 'CandidatePreeditStylePolicy\.composedText' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || rg -q 'CandidatePreeditStylePolicy[^;]*phrase_prefix' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android candidate preedit style must gate the spelling, never the phrase prefix" >&2
  exit 1
fi
# The polish presets carry their own prompt-injection wording and already exist in four places in
# this repository. This host reads the shared table through JNI rather than adding a fifth copy,
# and the transcript travels inside the tags that wording refers to.
if ! rg -q 'msime::windows::polish_prompt_for' \
    "$repo_root/platforms/android/native/client_jni.cpp" \
  || rg -q '语音转写整理助手' "$repo_root/platforms/android/java"; then
  echo "Android must read the polish presets from the shared table, not a copy" >&2
  exit 1
fi
# Candidate words reach api.msime.app only after an explicit account choice (PRIVACY.md). The policy smoke covers accountSelected itself; this pins the service to it: the fetch, the apply and the reserved rows read candidateTranslationAccount, and both preference paths derive it through the policy.
account_service="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
if ! rg -qU 'void scheduleCandidateTranslations\(\) \{\s*if \(!candidateTranslationAccount ' "$account_service" \
  || ! rg -qU 'void applyCandidateTranslations\(long generation\) \{\s*if \(!candidateTranslationAccount ' "$account_service" \
  || ! rg -qU 'glossLines\(\s*candidateTranslationTargets, candidateEnglishGloss, candidateTranslationAccount,\s*candidateOfflineTargets\(\)\)' "$account_service" \
  || [ "$(rg -c '= candidateTranslationAccountFrom\(preferences\);' "$account_service")" != 2 ]; then
  echo "Android must fetch candidate translations from the account only after an explicit choice" >&2
  exit 1
fi
# Turning 匿名使用统计 off has to stop reporting and clear the queue at once (the toggle promises it), not on the next start: the host process keeps Telemetry's own flag, so the privacy page must hand the saved value to Telemetry.setEnabled.
if ! rg -q 'toggle == InputFeatureToggle\.USAGE_REPORTING\) Telemetry\.setEnabled\(' \
    "$repo_root/platforms/android/java/app/msime/android/home/PrivacyPage.java"; then
  echo "Android privacy page must apply the usage-reporting toggle to Telemetry when it is saved" >&2
  exit 1
fi
# org.json's optBoolean accepts string values such as "true". Notice feeds and dismissal
# acknowledgements are native envelopes, so malformed JSON must not be treated as success.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/home/NoticeBanner.java"; then
  echo "Android notice responses must require a typed boolean ok field" >&2
  exit 1
fi
# App theme resolution is another native envelope; only a JSON boolean can authorize caching
# the returned palette and season.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/home/AppThemeController.java"; then
  echo "Android app theme responses must require a typed boolean ok field" >&2
  exit 1
fi
# The keyboard-side resolver has the same native envelope contract as the settings app. Keep its
# fallback path from accepting string booleans and caching an untrusted palette.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java"; then
  echo "Android keyboard theme responses must require a typed boolean ok field" >&2
  exit 1
fi
# Dictionary pinyin lookup is a native envelope too; a string status must fall back to no
# pronunciation rather than being parsed as a successful value.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"; then
  echo "Android handwriting dictionary responses must require a typed boolean ok field" >&2
  exit 1
fi
# Sound-pack metadata is consumed by the keyboard process; malformed native status must leave
# the pack disabled instead of constructing sounds from an untrusted value object.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeKeyFeedback.java"; then
  echo "Android key-sound responses must require a typed boolean ok field" >&2
  exit 1
fi
# Bootstrap controls first install and package refresh. A coerced status could accept a malformed
# preparation response and persist incomplete runtime options.
if rg -n 'getBoolean\("ok"\)|optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/core/Bootstrap.java"; then
  echo "Android bootstrap responses must require a typed boolean ok field" >&2
  exit 1
fi
# MSIMEInputService.value is the shared envelope reader for several keyboard operations. Keep its
# central status check strict so one malformed response cannot reach all those callers.
if sed -n '/private JSONObject value(String response)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android input service envelope reader must require a typed boolean ok field" >&2
  exit 1
fi
# The live preference reload feeds the next Engine session. It must use the same strict native
# envelope rule instead of accepting a string status from a malformed store response.
if sed -n '/private static String withLivePreferences/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android live preferences must require a typed boolean ok field" >&2
  exit 1
fi
# Online candidate queries gate network provider work. Require a typed success status before
# exposing the query object to the cloud and AI policy checks.
if sed -n '/private JSONObject onlineQuery(long targetSession)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android online candidate queries must require a typed boolean ok field" >&2
  exit 1
fi
# Cloud request URL generation is a native envelope. Reject a malformed status before handing the
# returned URL to the network transport.
if sed -n '/private String cloudRequestUrl(String document)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android cloud request URLs must require a typed boolean ok field" >&2
  exit 1
fi
# AI request descriptors also come from a native envelope; malformed success flags must not expose
# a provider endpoint to the client.
if sed -n '/private static JSONObject aiRequestDescriptor(String raw)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android AI request descriptors must require a typed boolean ok field" >&2
  exit 1
fi
# Host capability discovery feeds the typing settings page; only a typed status may replace the
# built-in helpcode schema list.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/home/TypingPage.java"; then
  echo "Android typing host capabilities must require a typed boolean ok field" >&2
  exit 1
fi
# The typing statistics badge reads a native dictionary count; malformed status must leave the
# optional badge unavailable instead of accepting a coerced success.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/home/StatisticsFragment.java"; then
  echo "Android statistics dictionary responses must require a typed boolean ok field" >&2
  exit 1
fi
# Developer diagnostics consume a native value envelope; malformed success must stay on the
# unavailable path rather than populating diagnostic controls.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/home/DeveloperPage.java"; then
  echo "Android developer responses must require a typed boolean ok field" >&2
  exit 1
fi
# Cloud sync routes several native operations through nativeValue; keep that shared failure gate
# strict so malformed envelopes cannot be merged into account state.
if sed -n '/private static JSONObject nativeValue(String response)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/home/CloudSync.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android cloud sync native envelopes must require a typed boolean ok field" >&2
  exit 1
fi
# Personal dictionary imports are acknowledged separately from the value envelope. A malformed
# acknowledgement must not be reported as a successful merge item.
if sed -n '/private boolean queueImport(String options, List<SyncMergePolicy.Word> words)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/home/CloudSync.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android cloud sync imports must require a typed boolean ok field" >&2
  exit 1
fi
# Typing statistics writes are optional, but their acknowledgement still controls failure
# reporting. Do not let org.json coerce a malformed status into success.
if sed -n '/private void submitTypingStatistics(String request, Runnable nothingRecorded/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android typing statistics responses must require a typed boolean ok field" >&2
  exit 1
fi
# Emoji catalog pages are native envelopes; reject malformed status before decoding entries.
if sed -n '/private EmojiCatalogModel.Page decodeEmojiPage/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android emoji catalog responses must require a typed boolean ok field" >&2
  exit 1
fi
# Transition acknowledgements are native protocol values too. Do not let org.json accept strings
# for deferred/handled flags or for the emoji page cursor's completion marker.
if sed -n '/private void reloadPreferences/,/^    }$/p;/boolean apply(String response)/,/^    }$/p;/private EmojiCatalogModel.Page decodeEmojiPage/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("(deferred|handled|complete)"\)|optBoolean\("(deferred|handled|complete)"'; then
  echo "Android transition and emoji cursor flags must require typed booleans" >&2
  exit 1
fi
# Candidate and online provider writes acknowledge whether the native operation took effect. A
# coerced string must never make the host publish a view it did not receive as applied.
if rg -n 'optBoolean\("applied"' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android candidate writes must require a typed boolean applied field" >&2
  exit 1
fi
# The keyboard skin save writes the local animation only after a successful native CAS response;
# keep that acknowledgement strict to avoid persisting a change after malformed JSON.
if sed -n '/void saveKeyboardSkin(String identifier, JSONObject design)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android keyboard skin saves must require a typed boolean ok field" >&2
  exit 1
fi
# Layout preference saves likewise update a local setting only after a native CAS success; keep the
# acknowledgement type strict.
if sed -n '/private void saveTouchGeometry(boolean reset)/,/^    }$/p' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -n 'getBoolean\("ok"\)|optBoolean\("ok"'; then
  echo "Android keyboard layout saves must require a typed boolean ok field" >&2
  exit 1
fi
# Sync rounds download over any section that is not dirty, so a preference write that forgets to mark settings dirty is reverted by the next cloud change. HostStore.savePreferences owns that mark for every caller.
if ! rg -qU 'NativeClient\.savePreferences\(directory, revision, document\)\)\);\s*(//[^\n]*\s*)?if \(saved != null\) SyncSignals\.markDirty\(context, SyncSwitch\.SETTINGS\);' \
    "$repo_root/platforms/android/java/app/msime/android/home/HostStore.java"; then
  echo "Android HostStore.savePreferences must mark the settings sync section dirty after a saved write" >&2
  exit 1
fi
# The shared translation query answers Korean Hanja rows, so neither the offline targets nor the account path may gate the Korean scheme out again; only Japanese stays ungated into other languages.
if rg -n 'KOREAN_SCHEME' <(sed -n '/void scheduleCandidateGlosses()/,/^    }$/p;/void scheduleCandidateTranslations()/,/^    }$/p' "$account_service") \
  || ! rg -q 'CandidateGlossPolicy\.hanjaAnnotation' "$account_service" \
  || ! rg -q 'CandidateTranslationPolicy\.reservedGlossRows\(candidateGlossLineCount\(\), koreanHanjaRows\(\)\)' "$account_service"; then
  echo "Android must gloss Korean Hanja rows and draw their 훈음 on its own reserved row" >&2
  exit 1
fi
if ! rg -q '<asr_text>' \
    "$repo_root/platforms/android/java/app/msime/android/voice/VoicePolishPolicy.java"; then
  echo "Android polish must wrap the transcript in the boundary the presets name" >&2
  exit 1
fi
# The streaming protocol's framing and authentication are the shared implementation's; this host
# adds only the transport Android has no platform API for. A frame built here would be a second
# encoder to keep in step with the provider.
if ! rg -q 'NativeClient\.doubaoStartFrame|NativeClient\.doubaoAudioFrame' \
    "$repo_root/platforms/android/java/app/msime/android/voice/DoubaoRecognizer.java" \
  || ! rg -q 'msime_client_doubao_(start|audio)_frame' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android streaming recognition must build its frames through the shared Host API" >&2
  exit 1
fi
# 识别线程遇到 JNI、TLS 或音频组件的意外异常时也必须结束请求；否则录音窗口会永久停在转写中。
voice_activity="$repo_root/platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"
if [[ "$(rg -c 'catch \(RuntimeException \| LinkageError' "$voice_activity" || true)" -lt 3 ]]; then
  echo "Android voice recognition workers must catch unexpected runtime/linkage failures" >&2
  exit 1
fi
# 匿名账号注册由每次 HomeActivity 重建触发，但同一进程只能保留一个网络注册任务。
identity_source="$repo_root/platforms/android/java/app/msime/android/account/AccountIdentity.java"
if ! rg -q 'AtomicBoolean REGISTERING' "$identity_source" \
    || ! rg -q 'REGISTERING\.compareAndSet\(false, true\)' "$identity_source" \
    || ! rg -q 'REGISTERING\.set\(false\)' "$identity_source"; then
  echo "Android anonymous account registration must be single-flight" >&2
  exit 1
fi
# 语音插件销毁时 executor 可能已经拒绝轮询任务；提交失败必须回滚活动请求并取消识别页。
voice_plugin="$repo_root/platforms/android/java/app/msime/android/voice/VoicePlugin.kt"
if ! rg -qU 'try \{\s*worker\.execute \{ pollResult\(job\) \}\s*\} catch \(_: RuntimeException\)' "$voice_plugin" \
    || ! rg -q 'VoiceRecognitionActivity\.clearRequest\(args\.requestId\)' "$voice_plugin"; then
  echo "Android voice plugin must roll back a rejected result poll" >&2
  exit 1
fi
# Reject an oversized response before JNI obtains a native view of the Java byte array. The shared
# decoder has the same one-megabyte wire bound, but checking after GetByteArrayElements can briefly
# duplicate an untrusted oversized WebSocket message.
if ! rg -q 'if \(length > 1024 \* 1024\)' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android Doubao decoding must bound the Java frame before copying" >&2
  exit 1
fi
# Opaque local speech handles are native pointers. Java rejects non-positive values, and JNI keeps
# the same boundary so a direct native call cannot turn a negative sentinel into an invalid dereference.
if ! rg -q 'if \(handle <= 0\) return nullptr;' \
    "$repo_root/platforms/android/native/client_jni.cpp" \
  || ! rg -q 'if \(handle > 0\) delete speech\(handle\);' \
    "$repo_root/platforms/android/native/client_jni.cpp" \
  || ! rg -q 'NativeHandlePolicy\.requirePositive\(handle\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java"; then
  echo "Android local speech handles must reject non-positive values before JNI pointer use" >&2
  exit 1
fi
# The Engine decides what a punctuation key produces, so the Chinese/English state has to reach it.
# A toggle that only changed this keyboard's key faces would show one mark and commit the other.
if ! rg -q 'setChinesePunctuationRaw' \
    "$repo_root/platforms/android/java/app/msime/android/core/NativeClient.java" \
  || ! rg -q 'msime_client_set_chinese_punctuation' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android punctuation switching must cross the shared Host API through JNI" >&2
  exit 1
fi
# The clipboard history is one file every mobile host shares, and the settings page reads it. A
# second implementation here is what made this keyboard and that page disagree about what the
# history contained, so the host may render entries but must not keep its own.
if ! rg -q 'NativeClient\.mobileClipboardHistory' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardHistoryStore.java" \
  || ! rg -q 'msime_client_mobile_clipboard_history' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android clipboard history must go through the shared mobile store" >&2
  exit 1
fi
if rg -q 'putString\(ITEMS_KEY' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardHistoryStore.java"; then
  echo "Android must not write clipboard entries to its own private document" >&2
  exit 1
fi
# 实时读到的偏好说开关关着时清空历史，是顺手的整理。存储写不进去不能成为不画键盘的理由：这次清空曾在 onCreateInputView 里抛异常，异常从框架的 showWindow 里抛出去，输入法进程退出，Android 换成了别的键盘，用户根本看不到这一个。「清空」按钮仍用 `clear()`：那是用户自己要求的，失败了不吭声就是撒谎。
if ! rg -q 'clearQuietly' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardHistoryStore.java"; then
  echo "Android clipboard housekeeping needs a clear that cannot stop the caller" >&2
  exit 1
fi
# 只有实时读到的偏好能触发清空。onCreateInputView 原先按字段初始值清空，每次进入编辑器又先应用 runtime-options 副本，而副本的 clipboard_history 永远是出厂默认的关，于是每换一个输入框、每切回本键盘一次，历史就被抹掉（#5602）。
if rg -A 40 'onCreateInputView\(\) \{' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q 'clipboardHistory\.clear'; then
  echo "Android onCreateInputView must not clear the clipboard history before live preferences arrive" >&2
  exit 1
fi
if ! rg -q 'ClipboardHistoryRetentionPolicy\.clearsHistory' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android clipboard housekeeping must ask ClipboardHistoryRetentionPolicy before clearing" >&2
  exit 1
fi
# 偏好的来源由调用处明说：读 runtime-options.json 之后应用的那一份必须标成 RUNTIME_OPTIONS_COPY，剪贴板开关也只从这个来源推出，不能再借一个意思不同的布尔量。那一处一旦被当成实时偏好，每换一个输入框历史又会被清空（#5602）。
if ! rg -A 12 'HostOptionsPolicy\.readRuntimeOptions\(getFilesDir\(\)\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q 'applyEditorPreferences\(preferences, ClipboardHistoryRetentionPolicy\.Source\.RUNTIME_OPTIONS_COPY\)' \
  || rg -A 12 'HostOptionsPolicy\.readRuntimeOptions\(getFilesDir\(\)\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q 'Source\.LIVE|applyEditorPreferences\(preferences\)' \
  || ! rg -q 'applyClipboardPreference\(JSONObject preferences,$' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -A 1 'applyClipboardPreference\(JSONObject preferences,$' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q 'ClipboardHistoryRetentionPolicy\.Source source\)'; then
  echo "Android must apply the runtime-options preference copy as RUNTIME_OPTIONS_COPY, and derive the clipboard switch only from that source (#5602)" >&2
  exit 1
fi
for site in onCreateInputView applyClipboardPreference; do
  if rg -A 40 "$site\([^)]*\) \{" \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
      | rg -q 'clipboardHistory\.clear\(\)'; then
    echo "Android clipboard housekeeping in $site must use clearQuietly" >&2
    exit 1
  fi
done
# 输入法服务没有 Activity 的窗口令牌。从剪贴板面板弹出的 AlertDialog 或 PopupMenu，要么加不上窗口、把键盘进程带崩，要么抢走编辑器的窗口焦点，Via 这类 WebView 浏览器随即让输入框失焦、收起键盘（#5605、#5653）。确认和条目操作都画在面板里。
if rg -A 30 'void (clearClipboardHistory|renderClipboardHistory|renderClipboardItemActions|setClipboardItemPinned|removeClipboardItem)\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg -q 'new (AlertDialog|PopupMenu)'; then
  echo "Android clipboard panel must not raise dialogs or popup menus from the input method" >&2
  exit 1
fi
if rg -q 'void manageClipboardItem|new PopupMenu\(this, anchor\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android clipboard entries must not be managed through a PopupMenu (#5653)" >&2
  exit 1
fi
# 分词界面的词片要放在它自己的滚动区里，「取消」「插入」那一行留在滚动区外、贴着面板底边；操作行跟着词片一起滚时，长文字滚到末尾它就不见了（#5645）。
if ! rg -A 30 'void renderClipboardSegmentation\(\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg -q 'new ScrollView\('; then
  echo "Android clipboard segmentation must scroll its word pieces separately from the pinned action bar (#5645)" >&2
  exit 1
fi
# 「最近复制」占着工具栏那一行时，剪贴板历史入口要留在这一行上，不能逼用户先点 × 把这条永久关掉；从剪贴板插入过之后它也不能再回来，否则工具栏又换成刚插入的文字（#5692）。
if ! rg -A 20 'void addRecentClipRow\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeToolbar.java" \
    | rg -q 'Icon\.CLIPBOARD'; then
  echo "Android recent-clip row must keep the clipboard history entry (#5692)" >&2
  exit 1
fi
if ! rg -A 12 'void insertClipboardText\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q 'recentClip\.dismiss\(\)'; then
  echo "Android clipboard insertion must retire the recent-clip suggestion (#5692)" >&2
  exit 1
fi
# 剪贴板历史关着时，「最近复制」既不显示也不读剪贴板：本地开关只能经 recentClipEnabled（RecentClipboardSuggestion.enabled）和历史开关一起判断，补看在读剪贴板之前就要问它，系统标为敏感的内容从不提供。
service_java="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
offer_body=$(rg -A 14 'void offerRecentClip\(' "$service_java" || true)
offer_gate_line=$(printf '%s\n' "$offer_body" | rg -n 'if \(!recentClipEnabled\(\)' | head -n 1 | cut -d: -f1 || true)
offer_read_line=$(printf '%s\n' "$offer_body" | rg -n 'getPrimaryClipDescription|readPrimaryClip|getPrimaryClip\(' | head -n 1 | cut -d: -f1 || true)
if ! rg -q 'return RecentClipboardSuggestion\.enabled\(clipboardHistoryEnabled, clipboardSuggestionEnabled\);' "$service_java" \
  || rg -n 'clipboardSuggestionEnabled' "$service_java" \
    | rg -v 'boolean clipboardSuggestionEnabled = true;|clipboardSuggestionEnabled = localSettings\.bool|RecentClipboardSuggestion\.enabled\(clipboardHistoryEnabled, clipboardSuggestionEnabled\)' \
    | rg -q . \
  || [[ -z "$offer_gate_line" || -z "$offer_read_line" || "$offer_read_line" -lt "$offer_gate_line" ]] \
  || ! printf '%s\n' "$offer_body" | rg -q 'clip == null \|\| clip\.sensitive\(\)\) return;' \
  || ! rg -B 2 -A 2 'String recent = ' "$service_java" | rg -q 'recentClipEnabled\(\)'; then
  echo "Android recent-clip suggestion must stay off, and must not read the clipboard, while clipboard history is off; sensitive clips are never offered (#5692)" >&2
  exit 1
fi
if ! rg -q 'ClipboardCapturePolicy\.captures' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android clipboard capture must skip a clip it already handled (ClipboardCapturePolicy)" >&2
  exit 1
fi
# 存储有了答复才把这一条记为已处理：写入抛异常时先记下，之后打开面板的补读就永远跳过它，它再也进不了历史。
capture_body=$(rg -A 30 'void captureClipboard\(' \
  "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" || true)
capture_add_line=$(printf '%s\n' "$capture_body" | rg -n 'clipboardHistory\.add\(' | head -n 1 | cut -d: -f1 || true)
capture_remember_line=$(printf '%s\n' "$capture_body" | rg -n 'rememberHandledClip\(identity\)' | head -n 1 | cut -d: -f1 || true)
if [[ -z "$capture_add_line" || -z "$capture_remember_line" \
    || "$capture_remember_line" -lt "$capture_add_line" ]]; then
  echo "Android clipboard capture must mark a clip handled only after the store answered" >&2
  exit 1
fi
# 已处理身份里有文字的散列和长度，短密码、验证码凭它能穷举还原。系统标为敏感的内容从不记录，清空或删除后也不能把它的身份落盘。
if ! rg -A 6 'private void forgetCurrentClip\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg -q '!clip\.sensitive\(\)'; then
  echo "Android must not persist the identity of a clip the system marked sensitive" >&2
  exit 1
fi
# Both maintenance chords are Ctrl+Shift+Alt, and the modifier branch in onKeyDown hands every
# such combination to the application. Routing them through one named policy, ahead of that branch,
# is what keeps them reachable at all on a keyboard that has no long press.
if ! rg -q 'HardwareMaintenancePolicy\.action' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'msime_client_reset_cache' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android maintenance chords must route through HardwareMaintenancePolicy and the shared API" >&2
  exit 1
fi
# Simplified/Traditional output is the shared OpenCC s2t tables, phrase-level, and the same
# conversion Windows and Linux use. This host converted one character at a time through
# android.icu.Transliterator, which turns 头发 into 頭發 and is silently unavailable below API 29.
if ! rg -q 'NativeClient::simplifiedToTraditional' \
    "$repo_root/platforms/android/java/app/msime/android/core/AndroidChineseTextConversion.java" \
  || rg -q '^import android\.icu\.text\.Transliterator' "$repo_root/platforms/android/java"; then
  echo "Android Simplified/Traditional output must use the shared converter" >&2
  exit 1
fi
# The keyboard has its own voice entry and never goes through the settings app, so it asks the
# shared resolution the same question rather than launching the platform recogniser regardless of
# what the user configured. A second copy of the provider rules in Java is how the two entries
# would start transcribing with different services on the same device.
if ! rg -q 'VoiceConfiguration\.read' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'msime_client_mobile_voice_configuration' \
    "$repo_root/platforms/android/native/client_jni.cpp"; then
  echo "Android keyboard voice must read the shared provider resolution" >&2
  exit 1
fi
# 键盘每换一个输入框都会重建引擎会话（#5680）。会话建好时要直接用上建会话前读到的那份实时偏好作为第一份快照，没有会话的那一段工具栏按钮开关要用上次真正读到的；否则冷启动的应用里皮肤、输入方式两个按钮先灰约一秒，剪贴板按钮先缺一格、其余按钮跟着挪位。
if ! sed -n '/private void startEngineSession(String optionsText, String livePreferences)/,/^    }$/p' "$account_service" \
    | rg -q 'applyPreferencesSnapshot\(value\(livePreferences\)\)' \
  || ! rg -q '"handwriting_theme", "touch_toolbar"\}' "$account_service" \
  || ! rg -q 'live \|\| rememberedToolbar == null' "$account_service"; then
  echo "Android toolbar must not start each editor from the factory-default preference copy" >&2
  exit 1
fi
# 候选条的配色和字号同理（#5933）：副本是出厂的薄荷设计，按它重算会让每个新输入框的候选条先铺一层薄荷底、字号回到出厂的 18/15，实时偏好到了才换回来。只有实时偏好重算候选条外观，冷启动的第一帧按皮肤片段算，片段里要记着候选条用到的字段；按片段着色之前要先读本地设置，否则应用主题种子按默认值缓存。
if ! sed -n '/private void applyEditorPreferences(JSONObject preferences,$/,/^    }$/p' "$account_service" \
    | rg -q '^\s*if \(live\) applyCandidateAppearance\(preferences\);' \
  || ! sed -n '/JSONObject hint = readSkinHint();/,/^        }$/p' "$account_service" \
    | rg -q 'applyCandidateAppearance\(hint\);' \
  || ! sed -n '/super\.onCreate();/,/JSONObject hint = readSkinHint();/p' "$account_service" \
    | rg -q 'refreshLocalSettings\(\);' \
  || ! rg -q '"candidate_theme", "candidate_font_family"' "$account_service" \
  || ! rg -q 'private boolean candidateHorizontal = true;' "$account_service"; then
  echo "Android candidate strip must not start each editor from the factory-default preference copy" >&2
  exit 1
fi
# 长按「中/英」弹出系统输入法选择框（#5615）。这个键在没有会话的输入框里也必须保持可用：禁用的按钮收不到长按，而密码框正是最需要换到密码管理器键盘的地方。没有会话时把键画淡，点按在反馈和计数之前就忽略。
if ! rg -q 'bindInputMethodPicker\(languageButton\)' "$account_service" \
  || ! rg -q 'manager\.showInputMethodPicker\(\)' "$account_service" \
  || rg -q 'setEnabled\(languageButton, session != 0\)' "$account_service" \
  || ! rg -q 'setActiveAlpha\(languageButton, canToggle' "$account_service" \
  || ! sed -n '/languageButton\.setOnClickListener/,/});/p' "$account_service" | rg -q 'if \(session == 0\) return;' \
  || ! rg -q 's\.bindInputMethodPicker\(language\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"; then
  echo "Android 中/英 keys must open the system input method picker on long press" >&2
  exit 1
fi
# 删除键上滑快速删除（#5585）：判定只在 BackspaceSwipePolicy，键的触摸监听按它决定松手是否清空，清空经服务的 deleteAllBeforeCursor 分段删除；连删的间隔按 BackspaceRepeatPolicy 逐级加速，不能写回固定间隔。
letter_rows="$repo_root/platforms/android/java/app/msime/android/core/ImeLetterRows.java"
if ! rg -q 'BackspaceSwipePolicy\.clearsOnRelease\(backspaceSwipePhase\)' "$letter_rows" \
  || ! rg -q 'BackspaceRepeatPolicy\.repeatInterval\(repeats\)' "$letter_rows" \
  || rg -q 'BackspaceRepeatPolicy\.REPEAT_INTERVAL_MS' "$letter_rows" \
  || ! rg -q 'BackspaceSwipePolicy\.clearBeforeCursor' "$account_service"; then
  echo "Android delete keys must accelerate and offer the quick-delete swipe through the shared policies" >&2
  exit 1
fi
# 各布局（九键、注音、笔画、手写等）自建的删除键都要经 bindBackspaceRepeat 绑定，否则那个布局按住不连删、也没有上滑快速删除；手写布局的删除键曾经漏绑。
layout_rows="$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"
if [[ $(rg -c 's\.backspaceKey\(' "$layout_rows") != $(rg -c 's\.imeLetterRows\.bindBackspaceRepeat\(' "$layout_rows") ]]; then
  echo "Every Android layout delete key must be bound through ImeLetterRows.bindBackspaceRepeat" >&2
  exit 1
fi
# 文本编辑面板（#5625）是工具栏面板的一员：closeToolbarPanels 要关掉它、anyToolbarPanelOpen 要算上它，否则换输入框时它会留在下一个编辑器的键盘上，收起键也不会变成「返回键盘」。
if ! sed -n '/void closeToolbarPanels()/,/^    }$/p' "$account_service" | rg -q 'imeTextEditPanel\.close\(\)' \
  || ! sed -n '/boolean anyToolbarPanelOpen()/,/^    }$/p' "$account_service" | rg -q 'shown\(textEditPanel\)'; then
  echo "Android text edit panel must close and count like the other toolbar panels" >&2
  exit 1
fi
# SpeechRecognizer 绑定的是 RecognitionService；Android 11 起只声明 RECOGNIZE_SPEECH 的话，识别服务与识别界面分属两个包的设备上会判为没有系统识别服务。原生宿主与 Tauri 壳共用同一个识别窗口，两份清单都要声明。
for manifest in \
    "$repo_root/platforms/android/AndroidManifest.xml" \
    "$repo_root/apps/desktop/src-tauri/gen/android/app/src/main/AndroidManifest.xml"; do
  if ! rg -q '<action android:name="android\.speech\.RecognitionService" />' "$manifest"; then
    echo "Android manifests must query android.speech.RecognitionService for SpeechRecognizer: $manifest" >&2
    exit 1
  fi
done
# 键区里的系统识别服务是 SpeechRecognizer 回调接线，JVM 冒烟只能覆盖 PlatformSpeechPolicy 和 ImeVoiceEntry.choose 这些纯逻辑，这里守住回调里不能被悄悄改回去的几处（#5553）：两条入口都按错误码提示、空结果不冒用错误码，没开始聆听就被拒时转交识别窗口，系统识别服务不被 1.5 s 停顿截断，说完后收回音量光圈。
voice_entry="$repo_root/platforms/android/java/app/msime/android/core/ImeVoiceEntry.java"
if ! rg -qF 'fail(PlatformSpeechPolicy.message(error))' "$voice_activity" \
  || ! rg -qF 'fail(PlatformSpeechPolicy.emptyResult())' "$voice_activity" \
  || ! rg -qF 'PlatformSpeechPolicy.message(error)' "$voice_entry" \
  || ! rg -qF 'PlatformSpeechPolicy.emptyResult()' "$voice_entry" \
  || ! rg -qF 's.launchVoiceActivity();' "$voice_entry" \
  || ! rg -qF 'if (platform != null) return;' "$voice_entry" \
  || ! rg -qF 'listening.resetLevel();' "$voice_entry"; then
  echo "Android platform speech callbacks must keep coded errors, the activity hand-off, the uncut pause and the level reset" >&2
  exit 1
fi
# The JNI translation unit is the one place a Java declaration and a shared FFI signature have to agree, and nothing else in this script reads it: a method declared native in Java compiles whether or not the C++ side exists. Compiling it for the real target catches that without the full native build, which needs vcpkg, the Rust Android targets and the pinned speech runtime. A machine without the pinned NDK skips it and says so.
ndk=${MSIME_ANDROID_NDK:-${android_sdk}/ndk/28.2.13676358}
case $(uname -s) in
  Darwin) host_tag=darwin-x86_64 ;;
  Linux) host_tag=linux-x86_64 ;;
  *) host_tag="" ;;
esac
jni_compiler="$ndk/toolchains/llvm/prebuilt/$host_tag/bin/aarch64-linux-android28-clang++"
if [[ -n "$host_tag" && -x "$jni_compiler" ]]; then
  "$jni_compiler" -std=c++20 -fsyntax-only -Wall -Werror \
    -I"$repo_root/crates/host-api/include" -I"$repo_root/shared" \
    "$repo_root/platforms/android/native/client_jni.cpp"
  echo "client_jni.cpp: aarch64-linux-android compile against the shared header passed"
else
  echo "client_jni.cpp: skipped (pinned NDK 28.2.13676358 not installed)"
fi
# This script compiles against API 35 while the manifest declares minSdk 28, so a
# newer java.nio API passes here and only fails in the real APK build. These two
# arrived in API 34 and are the ones that actually got in; neither has a runtime
# version guard anywhere in this host. This is a targeted guard, not a general
# API-level check - Gradle lint is what covers the rest.
if rg -n 'Files\.(readString|writeString)\(' "$repo_root/platforms/android/java" --glob '*.java'; then
  echo "Files.readString/writeString need API 34; this host declares minSdk 28" >&2
  exit 1
fi
if [[ ! -f "$repo_root/platforms/android/java/app/msime/android/handwriting/MlKitHandwritingRecognizer.java" \
      || ! -f "$repo_root/platforms/android/java/app/msime/android/handwriting/MlKitImeInitProvider.java" ]]; then
  echo "Android handwriting implementation must live with the native host sources" >&2
  exit 1
fi
if ! rg -Uq 'android:name="app\.msime\.android\.MlKitImeInitProvider"[[:space:]]+android:authorities="\$\{applicationId\}\.mlkit-ime-init"[[:space:]]+android:exported="false"[[:space:]]+android:process=":ime"' \
    "$repo_root/platforms/android/AndroidManifest.xml"; then
  echo "Android native host must initialize ML Kit inside the isolated IME process" >&2
  exit 1
fi
# The rotating refresh token must be spent by one process only. The session provider stays in the main process (no android:process) and unexported; the :ime keyboard asks it for a token instead of refreshing its own copy.
if ! rg -Uq 'android:name="app\.msime\.android\.AccountSessionProvider"[[:space:]]+android:authorities="\$\{applicationId\}\.account-session"[[:space:]]+android:exported="false"[[:space:]]*/>' \
    "$repo_root/platforms/android/AndroidManifest.xml"; then
  echo "Android account session provider must be declared unexported in the main process" >&2
  exit 1
fi
# The host compiles against AndroidX and Material now, and those are AARs that only Gradle resolves,
# so this script no longer compiles the whole source set -- `platforms/android/gradle-app` does, and
# build-apk.sh drives it. What stays here is the part that is worth having without a Gradle daemon:
# the pure-Java models and their smokes, which have no Android dependency at all and run in a second.
# Custom skin libraries are user-writable; keep the reader streaming so a file that grows after
# inspection cannot turn the one-megabyte envelope into an unbounded allocation.
if rg -n 'Files\.readAllBytes' \
    "$repo_root/platforms/android/java/app/msime/android/dictionary/CustomSkinLibrary.java"; then
  echo "Android custom skin library must use a bounded streaming read" >&2
  exit 1
fi
if rg -n 'Files\.readAllBytes' \
    "$repo_root/platforms/android/java/app/msime/android/voice/CommunityReplyLibrary.java"; then
  echo "Android community reply library must use a bounded streaming read" >&2
  exit 1
fi
if rg -n 'Files\.readAllBytes' \
    "$repo_root/platforms/android/java/app/msime/android/KeyboardFeedbackStore.java"; then
  echo "Android keyboard feedback store must use a bounded streaming read" >&2
  exit 1
fi
if rg -n 'Files\.readAllBytes' \
    "$repo_root/platforms/android/java/app/msime/android/core/Bootstrap.java"; then
  echo "Android bootstrap marker must use a bounded streaming read" >&2
  exit 1
fi
for source in \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    "$repo_root/platforms/android/java/app/msime/android/voice/VoiceRecognitionActivity.java"; do
  if rg -n 'readAllBytes.*runtime-options|runtime-options.*readAllBytes' "$source"; then
    echo "Android runtime-options readers must use HostOptionsPolicy" >&2
    exit 1
  fi
done
# 词库页的大标题行只放标题：右侧那排胶囊会在窄屏上把「词库」挤成「词…」（#5682），导入、导出和刷新放在内容里的管理卡片中。
if rg -n 'headerActions\(\)' "$repo_root/platforms/android/java/app/msime/android/home/LexiconPage.java"; then
  echo "Android lexicon page must keep its actions in the manage card, not beside the large title" >&2
  exit 1
fi
#
# Match the launcher activities by their path *inside the repository*. The absolute pattern this
# started as, `*/home/*`, also matches every source on a GitHub runner, where the checkout itself
# lives under /home/runner: the list came out empty, javac was handed nothing but the test files,
# and the gate failed with 1069 "cannot find symbol" errors on every pull request while passing on
# any developer machine whose checkout is not under /home.
client_sources=()
while IFS= read -r source; do
  case "${source#"$repo_root/"}" in
    platforms/android/java/app/msime/android/home/*) continue ;;
  esac
  if rg -q '^import (androidx|com\.google)\.' "$source"; then continue; fi
  client_sources+=("$source")
done < <(find "$repo_root/platforms/android/java/app/msime/android" -name "*.java" -print)
# An empty list means the filter above ate everything; javac would then fail on the test files with
# a wall of missing symbols rather than saying so.
if [[ ${#client_sources[@]} -eq 0 ]]; then
  echo "No Android client sources selected for compilation; the source filter is wrong" >&2
  exit 1
fi
# 冒烟测试按文件自动发现：`tests/` 下除设备套件外的每个 `.java` 都参与编译，每个以 `Smoke.java` 结尾的类都会运行，没有 `static void main(` 入口的会让检查直接失败而不是被跳过，新加冒烟不必再登记到这里。排除的三处各有原因：`tests/device/**` 是要装进模拟器的设备套件，`core/NativeSmoke.java` 要加载 `libmsime_android.so`，`settings/KeyboardGeometryStrictIntSmoke.java` 要真实的 `org.json`，而这里只有 android.jar 里抛 `Stub!` 的桩。路径同样按仓库内的相对路径匹配，理由见上面那段关于 /home/runner 的说明。
test_sources=()
smoke_classes=()
while IFS= read -r source; do
  relative=${source#"$repo_root/platforms/android/tests/"}
  case "$relative" in
    device/*|core/NativeSmoke.java|settings/KeyboardGeometryStrictIntSmoke.java) continue ;;
  esac
  test_sources+=("$source")
  case "$relative" in
    *Smoke.java) ;;
    *) continue ;;
  esac
  if ! rg -q 'static void main\(' "$source"; then
    echo "Android JVM smoke has no 'static void main(' entry point: ${source#"$repo_root/"}" >&2
    exit 1
  fi
  package=$(sed -n 's/^package \([A-Za-z0-9_.]*\);.*/\1/p' "$source" | head -n 1)
  class=$(basename "$source" .java)
  smoke_classes+=("${package:+$package.}$class")
done < <(find "$repo_root/platforms/android/tests" -name "*.java" -print | LC_ALL=C sort)
# 换一条路独立数一遍冒烟：在 `tests/` 里只按文件名找 `*Smoke.java`，得到仓库内的相对路径，再去掉同样的三处排除。两边对不上，说明上面循环的筛选或路径匹配坏了。这里原来是一个写死的下限，每个新增冒烟的 PR 都要改同一行，并行的 Android PR 因此两两冲突；现在新增或删除冒烟都不用改这里。两边都数出 0 也算失败，那是 `tests/` 的位置错了。
expected_smokes=$( (cd "$repo_root/platforms/android/tests" && find . -name "*Smoke.java" -print) \
  | grep -cvxE '\./device/.*|\./core/NativeSmoke\.java|\./settings/KeyboardGeometryStrictIntSmoke\.java' || true)
if [[ $expected_smokes -eq 0 || ${#smoke_classes[@]} -ne $expected_smokes ]]; then
  echo "Discovered ${#smoke_classes[@]} Android JVM smokes, but platforms/android/tests holds $expected_smokes; the discovery filter is wrong" >&2
  exit 1
fi
javac --release 17 -Xlint:all -Werror -cp "$android_jar" -d "$output_dir" \
  "${client_sources[@]}" \
  "$repo_root/platforms/android/java/app/msime/android/home/SignInAttemptPolicy.java" \
  "$repo_root/platforms/android/java/app/msime/android/home/OnboardingChoicePolicy.java" \
  "${test_sources[@]}"
# 统一用 `$output_dir:$android_jar` 运行：改成自动发现前逐个核对过，原先按类分别给的 classpath（有的不带 android.jar）与统一 classpath 下 108 个冒烟的输出和退出码完全相同。
for smoke in "${smoke_classes[@]}"; do
  if ! java -cp "$output_dir:$android_jar" "$smoke"; then
    echo "Android JVM smoke failed: $smoke" >&2
    exit 1
  fi
done
echo "Ran ${#smoke_classes[@]} Android JVM smokes"
# 设备测试包只在 `tests/device/smoke.sh` 里构建，而那要模拟器，CI 从不跑它；它的源文件清单是手写的，应用类挪进新的辅助类后没人补，曾经攒到 21 个编译错误，整个设备套件都构建不出来。这里按同一份清单只做编译，漏了类就在这一步失败。
device_sources=()
while IFS= read -r source; do
  [[ -z $source || $source == \#* ]] || device_sources+=("$repo_root/$source")
done < "$repo_root/platforms/android/tests/device/editor-sources.txt"
mkdir -p "$output_dir/device"
javac --release 17 -Xlint:all -Werror -cp "$android_jar" -d "$output_dir/device" "${device_sources[@]}"
echo "Compiled ${#device_sources[@]} Android device-suite sources"
# Resources are compiled but not linked here: they reference Material's theme attributes, and linking
# those needs the library's own resources, which is Gradle's job. Compiling still catches a malformed
# drawable, layout or values file, which is what this step was for.
"$android_sdk/build-tools/35.0.0/aapt2" compile --dir "$repo_root/platforms/android/res" -o "$output_dir/resources.zip"
for alias in MainActivityForest MainActivitySky MainActivityDusk MainActivityVermilion; do
  if ! rg -q "android:name=\"\\.${alias}\"" "$repo_root/platforms/android/AndroidManifest.xml"; then
    echo "Android app icon alias missing: $alias" >&2
    exit 1
  fi
done
# A disabled tool card swallows the press and the 工具 section draws no state text, so the only
# thing left to say it is unavailable is how it looks.
if ! rg -q 'card\.setAlpha\(enabled \?|ViewPolicy\.setActiveAlpha\(card, enabled, \.45f\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android tool cards must look disabled when they are" >&2
  exit 1
fi
# The layout bar's buttons carry the keyboard's action-key fill, so their text has to be the colour
# that fill is paired with. `accent` is that same fill in the shipped skins, and using it here made
# 恢复默认 and 完成 invisible - dark green on dark green, three blank tiles where the controls are.
if rg -q 'button\.setTextColor\(accent\)' \
    "$repo_root/platforms/android/java/app/msime/android/keyboard/KeyboardLayoutAdjustView.java"; then
  echo "Android layout bar buttons must take actionForeground, not the accent they sit on" >&2
  exit 1
fi
# `deleteSurroundingText` 只删选区以外的字：选中开头的「你好」按删除毫无反应，选中中间的文字会删掉选区前一个字。键盘上的删除键都要经 `deleteCodePointBeforeCursor`，由它先删选区；笔画布局曾经绕过它直接调 InputConnection。
if ! rg -q 'if \(deleteSelection\(\)\) return;' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || rg -l '\.deleteSurroundingTextInCodePoints\(' "$repo_root/platforms/android/java" \
    | rg -v '/core/MSIMEInputService\.java$' >/dev/null; then
  echo "Android delete keys must go through deleteCodePointBeforeCursor, which deletes a selection first" >&2
  exit 1
fi
# 符号面板的分类键和锁定键用 setSelected 表示当前项，但键帽颜色只在上色时读一次 isSelected()（ImeStyler.styleButton）。#5597 就是只改了选中状态、没有重新上色：点「网络」后右侧换了，左侧高亮仍停在「常用」。每一处改选中状态的地方都要紧跟一次 restyle。
symbol_panel_view="$repo_root/platforms/android/java/app/msime/android/keyboard/SymbolPanelView.java"
if ! rg -q 'ViewPolicy\.setSelected\(' "$symbol_panel_view" \
  || ! awk '/ViewPolicy\.setSelected\(/ { pending = 1; next } pending { if ($0 !~ /listener\.restyle\(/) bad = 1; pending = 0 } END { exit (bad || pending) }' "$symbol_panel_view"; then
  echo "Android symbol panel must restyle every button whose selected state it changes" >&2
  exit 1
fi
# The JVM smokes cannot load org.json, so nothing else here can reach the one place where the
# shared runtime's JSON nulls meet this host's reads of them.
python3 "$repo_root/scripts/test-android-json-null-reads.py" || exit 1
# Every other contract check that needs ripgrep is discovered by scripts/run-checks.sh, whose
# contracts job deliberately installs nothing and therefore skips this one; this job already
# installs rg, so it is the only place where the reader search actually runs.
python3 "$repo_root/scripts/test-android-preference-keys.py" || exit 1
echo "Android service Java/API and manifest/resource checks passed; no installable/native APK produced"
