#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
# Several guards below are "fail if rg finds this". Without rg each of those exits 127, which the if reads as "not found", so a missing binary would pass every one of them silently.
if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) is required by the Android host contract checks" >&2
  exit 1
fi
# 管道下游的 `rg -q` 一律换成它：先把标准输入整个读完，再交给 `rg -q` 判断。`rg -q` 读到第一处命中就退出，上游的 printf、sed 或 rg 若还有没写完的输出，再写就收到 SIGPIPE（`printf: write error: Broken pipe`），在 pipefail 下整条管道返回失败，于是同一份源码时过时不过，取决于上游分几次写、下游何时退出。变量本身直接用 here-string（`rg -q pattern <<< "$var"`），不用经过管道。scripts/test-gate-pipe-early-exit.py 会拦下新写的 `| rg -q`。
rg_q_stdin() {
  local input
  input=$(cat)
  rg -q "$@" <<< "$input"
}
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
trap 'rm -f "$output_dir/manifest.apk" "$output_dir/resources.zip" "$output_dir/jni-callbacks/jni-callbacks-test"; find "$output_dir" -name "*.class" -delete; find "$output_dir" -depth -type d -empty -delete' EXIT
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
# 偏好每秒重读一次，重读前后的键盘皮肤要按实际画出来的样子比（KeyboardSkin#displayKey，KeyboardSkinDisplayKeySmoke 覆盖它的语义）。直接比 `skin.key()` 时，渲染写回的当季配色和重读重建的 classic 基础色永远不等，跟随系统皮肤下键盘每秒整页重画一次。
if rg -n 'previousSkin = skin\.key\(\)|previousSkin\.equals\(skin\.key\(\)\)' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'previousSkin\.equals\(skin\.displayKey\(' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android preference reloads must compare the drawn skin (KeyboardSkin#displayKey), not skin.key()" >&2
  exit 1
fi
# 日语九键的五向提示按下就显示（#6716）：0.3.1 曾把它推迟到系统长按时长之后，用户感觉每个假名都慢半拍。提示已收进被按的键里，不再需要这个延迟。
if rg -n 'getLongPressTimeout' "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"; then
  echo "The Japanese nine-key flick guide must show on touch-down, not after the long-press timeout" >&2
  exit 1
fi
# 只改大小写时不重建键行（#6997）：重建出的新键在下一次布局前宽高为 0，Shift 之后立刻按下的字母找不到键而被丢掉。Shift、一次性大写用掉和自动大写都经 refreshLetterCase 就地改键面；字母键的气泡读按下时的键面，不能用建键时记下的那个。触发和重建之间允许夹着 `//` 注释行（Shift 处理里就有一行），不然改回 rebuildKeyRows 时这道检查认不出来。
if rg -U -n '(consumeLetter\(\)\)\s*\{?|letterCase\.toggle\(SystemClock\.uptimeMillis\(\)\);|applyAutomatic\(next\)\))(\s*//[^\n]*)*\s*imeLetterRows\.rebuildKeyRows\(\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || rg -n 'showKeyPreview\(key, face\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeLetterRows.java"; then
  echo "A letter-case change must relabel the keys in place (ImeLetterRows.refreshLetterCase), not rebuild the rows" >&2
  exit 1
fi
# 换应用、换页面时会话要在后台建一两秒，view 那之前是空的（#6997）：双拼提示按上次记下的方案画，不能因为 view 暂时为空就清掉，否则会话一到提示和下边距同时出现，整排字母跳一下。
if ! rg -q 'ShuangpinKeyHintPolicy\.profileWithoutView\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Double-pinyin key hints must keep the remembered profile while the view is briefly missing" >&2
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
# 中英混输、候选带符号和候选带 emoji / 颜文字都是 Engine 的混输（共享偏好 mixed_input.english / minimum_prefix / symbols / emoji / kaomoji），Android 设置曾经没有开关（emoji / 颜文字见 #5667，中英混输和触发字母数见 #7081）。`MixedInputPreferences` 四个字段必填，只写一个字段的对象会让整份偏好被拒绝，所以写之前要补齐；`symbols` 是可选字段，缺省按开处理，不用补。
expression_page="$repo_root/platforms/android/java/app/msime/android/home/ExpressionPage.java"
for field in english minimum_prefix emoji kaomoji; do
  if ! rg -q "mixed\.has\(\"$field\"\)" "$expression_page"; then
    echo "Android expression page must fill mixed_input.$field before writing the object back" >&2
    exit 1
  fi
done
if ! rg -q 'mixedInput\(edit\)\.put\("english"' "$expression_page" \
  || ! rg -q 'mixedInput\(edit\)\.put\("minimum_prefix"' "$expression_page" \
  || ! rg -q 'mixedInput\(edit\)\.put\("emoji"' "$expression_page" \
  || ! rg -q 'mixedInput\(edit\)\.put\("kaomoji"' "$expression_page" \
  || ! rg -q 'mixedInput\(edit\)\.put\("symbols"' "$expression_page"; then
  echo "Android expression page must offer the shared mixed English, minimum prefix, symbol, emoji and kaomoji candidate settings" >&2
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
# 拼音显示位置只管拼写：已选的那一段按这个宿主的请求留在文档之外，无论如何都要画在候选栏上，否则就是 scripts/test-phrase-preedit-hosts.py 从另一边防的那种半截状态。共享偏好由策略统一规范化，不能在宿主里复制一套。位置读 `touch_spelling_position`，不读桌面候选窗的 `candidate_preedit_style`：后者默认是 pinyin，读它会让所有没动过设置的用户默认把拼写挪出输入框（#9255 就这样让设备冒烟的 phrase composition 失败）。
if ! rg -q 'CandidatePreeditStylePolicy\.composedText' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || rg -q 'CandidatePreeditStylePolicy[^;]*phrase_prefix' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q '"touch_spelling_position"' "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q '"touch_spelling_position"' "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java" \
  || rg -q '"candidate_preedit_style"' "$repo_root/platforms/android/java"; then
  echo "Android spelling position must gate the spelling, never the phrase prefix, and must read touch_spelling_position rather than candidate_preedit_style" >&2
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
# 候选词只有在用户显式选了水杉账号之后才会发到 api.msime.app（见 PRIVACY.md）。路由本身由 CandidateTranslationRouteSmoke 覆盖，这里把键盘钉在路由上：取译文、应用译文和预留的释义行都看 candidateTranslationAccount，两条偏好路径都经过策略算出它，并且账号只是路由里的 ACCOUNT 服务。
account_service="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
if ! rg -qU 'void scheduleCandidateTranslations\(\) \{\s*if \(!candidateTranslationAccount ' "$account_service" \
  || ! rg -qU 'void applyCandidateTranslations\(long generation\) \{\s*if \(!candidateTranslationAccount ' "$account_service" \
  || ! rg -qU 'glossLines\(\s*candidateTranslationTargets, candidateEnglishGloss,\s*candidateTranslationAccount \|\| candidateTranslationRoute\.userService\(\),\s*candidateOfflineTargets\(\)\)' "$account_service" \
  || [ "$(rg -c '= candidateTranslationAccountFrom\(preferences\);' "$account_service")" != 2 ] \
  || ! rg -qF 'return candidateTranslationRouteFrom(preferences).provider() == CandidateTranslationRoute.Provider.ACCOUNT;' "$account_service"; then
  echo "Android must fetch candidate translations from the account only after an explicit choice" >&2
  exit 1
fi
# 候选词发给用户自己的翻译服务（小牛、自定义、腾讯）也只能经过同一个选路：两条偏好路径都用 CandidateTranslationPolicy.route 算出路由，取译文和应用译文都先看路由是不是用户服务。请求头（含鉴权和腾讯的签名头）一律来自共享层的描述器，Java 里不许自己拼。
user_translation_client="$repo_root/platforms/android/java/app/msime/android/candidate/UserTranslationClient.java"
user_translation_transport="$repo_root/platforms/android/java/app/msime/android/candidate/TranslationHttpTransport.java"
if ! rg -qU 'void scheduleUserCandidateTranslations\(\) \{\s*if \(!candidateTranslationRoute\.userService\(\) ' "$account_service" \
  || ! rg -qU 'void applyUserCandidateTranslations\(long generation\) \{\s*if \(!candidateTranslationRoute\.userService\(\) ' "$account_service" \
  || [ "$(rg -c '= candidateTranslationRouteFrom\(preferences\);' "$account_service")" != 2 ] \
  || ! rg -qF 'return CandidateTranslationPolicy.route(' "$account_service"; then
  echo "Android must send candidate words to the user's own translation service only through the shared route" >&2
  exit 1
fi
# 记下的回答和退避（UserTranslationMemory）跟着路由走，不跟着输入会话走：PRIVACY.md 承诺问过的词不再请求、服务失败后 30 秒内不再发请求。stop() 每次换输入框、收起键盘都会跑，它只能暂置路由，不能经 useCandidateTranslationRoute 把客户端换掉；回到同一路由时要沿用原来的客户端。
if ! rg -qF 'suspendCandidateTranslationRoute();' "$account_service" \
  || rg -qF 'useCandidateTranslationRoute(CandidateTranslationRoute.NONE)' "$account_service" \
  || ! rg -qF 'if (userTranslationClient == null || !userTranslationClient.route().equals(next))' "$account_service"; then
  echo "Android must keep the user translation client's memory across input sessions while the route is unchanged" >&2
  exit 1
fi
if rg -n 'Authorization|DeepL-Auth-Key|Bearer|X-TC-' "$user_translation_client" "$user_translation_transport"; then
  echo "Android user translation requests must take every header from the shared descriptor" >&2
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
# App theme resolution is shared by the settings app and keyboard. Only a JSON boolean can
# authorize caching the returned palette and season in either process.
if rg -n 'optBoolean\("ok"' \
    "$repo_root/platforms/android/java/app/msime/android/AppThemeResolver.java" \
    "$repo_root/platforms/android/java/app/msime/android/home/AppThemeController.java" \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java"; then
  echo "Android app theme responses must require a typed boolean ok field" >&2
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
# 输入页要有「在候选栏中显示辅助码」开关，写回当前方案那一族的 `show_in_candidate_window`，默认值和 client-core 一致（缺整个对象时双拼显示、全拼不显示，对象里缺这一项时显示）。没有它时双拼辅助码一直占着候选提示位，想看释义只能把辅助码整个关掉（#6997）。
helpcode_body=$(rg -A 12 'if \(helpcodeOn\) \{' \
  "$repo_root/platforms/android/java/app/msime/android/home/TypingPage.java" || true)
if ! rg -q 'helpcode == null \? shuangpin : helpcode\.optBoolean\("show_in_candidate_window", true\)' <<< "$helpcode_body" \
  || ! rg -q 'aux\.toggle\(shuangpin \? "在候选栏中显示双拼辅助码" : "在候选栏中显示全拼辅助码"' <<< "$helpcode_body" \
  || ! rg -q 'saveHelpcode\(family, shuangpin, "show_in_candidate_window", checked\)' <<< "$helpcode_body"; then
  echo "Android typing settings must offer the helpcode display switch that writes <family>.show_in_candidate_window (#6997)" >&2
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
# 手机上只显示一种候选翻译：键盘不读第二种语言偏好，目标语言只从主目标语言算出，否则请求第二种语言、候选栏多预留一行释义。
if rg -n 'translation_secondary_language' "$repo_root/platforms/android/java" \
  || ! rg -qF 'return CandidateTranslationPolicy.targets(primary);' "$account_service"; then
  echo "Android touch keyboards must request and reserve only the primary translation language" >&2
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
# 语音句柄的创建、查找与撤销由 LocalSpeechHandleDeviceSmoke 经真实 JNI 验证。
# 不再用指针转换和符号比较的源码形状充当有效性检查。
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
    | rg_q_stdin 'clipboardHistory\.clear'; then
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
    | rg_q_stdin 'applyEditorPreferences\(preferences, ClipboardHistoryRetentionPolicy\.Source\.RUNTIME_OPTIONS_COPY\)' \
  || rg -A 12 'HostOptionsPolicy\.readRuntimeOptions\(getFilesDir\(\)\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg_q_stdin 'Source\.LIVE|applyEditorPreferences\(preferences\)' \
  || ! rg -q 'applyClipboardPreference\(JSONObject preferences,$' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -A 1 'applyClipboardPreference\(JSONObject preferences,$' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg_q_stdin 'ClipboardHistoryRetentionPolicy\.Source source\)'; then
  echo "Android must apply the runtime-options preference copy as RUNTIME_OPTIONS_COPY, and derive the clipboard switch only from that source (#5602)" >&2
  exit 1
fi
for site in onCreateInputView applyClipboardPreference; do
  if rg -A 40 "$site\([^)]*\) \{" \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
      | rg_q_stdin 'clipboardHistory\.clear\(\)'; then
    echo "Android clipboard housekeeping in $site must use clearQuietly" >&2
    exit 1
  fi
done
# 输入法服务没有 Activity 的窗口令牌。从剪贴板面板弹出的 AlertDialog 或 PopupMenu，要么加不上窗口、把键盘进程带崩，要么抢走编辑器的窗口焦点，Via 这类 WebView 浏览器随即让输入框失焦、收起键盘（#5605、#5653）。确认和条目操作都画在面板里。
if rg -A 30 'void (clearClipboardHistory|renderClipboardHistory|renderClipboardItemActions|setClipboardItemPinned|removeClipboardItem)\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'new (AlertDialog|PopupMenu)'; then
  echo "Android clipboard panel must not raise dialogs or popup menus from the input method" >&2
  exit 1
fi
# 删除候选词的确认收在长按菜单的子菜单里（ImeCandidates），与 iOS 的嵌套 UIMenu 一致：点开「删除词条…」当场展开「确认删除此词条」，一次长按删完；不弹 AlertDialog（#5605、#5653：输入法服务没有 Activity 的窗口令牌，对话框要么加不上窗口、让输入法进程崩掉，要么抢走编辑器的窗口焦点，两种情况键盘都会被收起），也不要退出去再长按一次的两步确认。
if rg -q 'new AlertDialog' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android input method service must not raise AlertDialog; confirm inside the keyboard (#5605, #5653)" >&2
  exit 1
fi
if ! rg -q 'addSubMenu\(Menu\.NONE, Menu\.NONE, action\.ordinal\(\), action\.title\(\)\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeCandidates.java" \
  || ! rg -q 'action\.confirmationTitle\(\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeCandidates.java" \
  || ! rg -q 'String confirmationTitle\(\)' \
    "$repo_root/platforms/android/java/app/msime/android/candidate/CandidateManagementAction.java"; then
  echo "Android candidate removal must confirm through the long-press submenu (#7793)" >&2
  exit 1
fi
# 「添加到常用语」经常用语存储写入（校验、去重、上限和同步标记都在那里），它要等文件锁，必须放在工作线程上，不能在主线程卡住键盘（#5909）。
if ! rg -A 12 'void addClipboardTextToPhrases\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'preferencesWorker\.execute' \
  || ! rg -A 12 'void addClipboardTextToPhrases\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'CommonPhrasesStore\.add\(s, text\)'; then
  echo "Android clipboard add-to-phrases must write through CommonPhrasesStore on the preferences worker (#5909)" >&2
  exit 1
fi
# 左滑删除只在 ClipboardSwipePolicy 判定为横滑后才接手：接手时不让外层面板拦截去滚动，并给卡片补一个取消，撤掉按下态、长按和点按，松手不会插入（#5962）。
swipe_body=$(rg -A 60 'private final class ClipboardSwipeCell implements' \
  "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" || true)
if ! rg -q 'ClipboardSwipePolicy\.claims\(' <<< "$swipe_body" \
  || ! rg -q 'requestDisallowInterceptTouchEvent\(true\)' <<< "$swipe_body" \
  || ! rg -q 'ACTION_CANCEL\);' <<< "$swipe_body"; then
  echo "Android clipboard swipe must claim only through ClipboardSwipePolicy, keep the panel from scrolling and cancel the card's own press (#5962)" >&2
  exit 1
fi
# 「编辑」打开应用里的编辑页，不在键盘里弹对话框或放文本框（#5971）。打开之前要把系统剪贴板当前那一条记为已处理，否则改完回来一打开面板，补读又把原文记回来；交给编辑页的是 AndroidKeyStore 令牌，不是文字。编辑页的 PageId 名必须和键盘这边写死的页面名一致。
edit_body=$(rg -A 12 'void editClipboardItem\(' \
  "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" || true)
if ! rg -q 'forgetCurrentClip\(\);' <<< "$edit_body" \
  || ! rg -q 'ClipboardEditKey\.editKey\(' <<< "$edit_body" \
  || ! rg -q 'openHostPage\(ClipboardHistoryPolicy\.EDIT_PAGE, args\)' <<< "$edit_body" \
  || rg -q 'new (AlertDialog|PopupMenu|EditText)' <<< "$edit_body" \
  || ! rg -q 'public static final String EDIT_PAGE = "CLIPBOARD_EDIT";' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardHistoryPolicy.java" \
  || ! rg -q '^    CLIPBOARD_EDIT\("ClipboardEditPage", ' \
    "$repo_root/platforms/android/java/app/msime/android/home/PageId.java" \
  || ! rg -A 20 'private void renderClipboardItemActions\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" | rg_q_stdin 's\.editClipboardItem\(item\)'; then
  echo "Android clipboard edit must open the app's CLIPBOARD_EDIT page with the entry's key after forgetting the current clip, never a dialog in the input method (#5971)" >&2
  exit 1
fi
# 「搜索」打开应用里可搜索的剪贴板历史页，不在键盘里放查询框（#5973）。打开之前同样把系统剪贴板当前那一条记为已处理，用户可能在那一页删掉或改掉它。页面名必须和 PageId 一致，入口要在面板顶行里。
search_body=$(rg -A 6 'void openClipboardSearch\(' \
  "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" || true)
if ! rg -q 'forgetCurrentClip\(\);' <<< "$search_body" \
  || ! rg -q 'putReturnToCaller\(args\);' <<< "$search_body" \
  || ! rg -q 'openHostPage\(ClipboardSearchPolicy\.SEARCH_PAGE, args\)' <<< "$search_body" \
  || rg -q 'new (AlertDialog|PopupMenu|EditText)' <<< "$search_body" \
  || ! rg -q 'public static final String SEARCH_PAGE = "CLIPBOARD_SEARCH";' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardSearchPolicy.java" \
  || ! rg -q '^    CLIPBOARD_SEARCH\("ClipboardSearchPage", ' \
    "$repo_root/platforms/android/java/app/msime/android/home/PageId.java" \
  || ! rg -q 'clipboardAction\(header, "搜索", s::openClipboardSearch\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java"; then
  echo "Android clipboard search must open the app's CLIPBOARD_SEARCH page from the panel header after forgetting the current clip, never a query field in the input method (#5973)" >&2
  exit 1
fi
# 分享只把用户长按选中的一条记录交给系统选择器；输入法服务启动 chooser 必须带新任务标志，不能把历史列表或内部标识一起发出去。
share_body=$(rg -A 24 'void shareClipboardText\(' \
  "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" || true)
if ! rg -q 'ClipboardSharePolicy\.canShare\(text\)' <<< "$share_body" \
  || ! rg -q 'Intent\.ACTION_SEND' <<< "$share_body" \
  || ! rg -q 'Intent\.EXTRA_TEXT' <<< "$share_body" \
  || ! rg -q 'Intent\.FLAG_ACTIVITY_NEW_TASK' <<< "$share_body" \
  || ! rg -q 'Intent\.createChooser\(send, ClipboardSharePolicy\.CHOOSER_TITLE\)' <<< "$share_body" \
  || ! rg -q 'clipboardItemAction\(row, ClipboardSharePolicy\.ACTION' \
      "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java"; then
  echo "Android clipboard sharing must use a single selected entry through the system text chooser (#8517)" >&2
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
    | rg_q_stdin 'new ScrollView\('; then
  echo "Android clipboard segmentation must scroll its word pieces separately from the pinned action bar (#5645)" >&2
  exit 1
fi
# 「最近复制」占着工具栏那一行时，剪贴板历史入口要留在这一行上，不能逼用户先点 × 把这条永久关掉；从剪贴板插入过之后它也不能再回来，否则工具栏又换成刚插入的文字（#5692）。
if ! rg -A 20 'void addRecentClipRow\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeToolbar.java" \
    | rg_q_stdin 'Icon\.CLIPBOARD'; then
  echo "Android recent-clip row must keep the clipboard history entry (#5692)" >&2
  exit 1
fi
if ! rg -A 12 'void insertClipboardText\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg_q_stdin 'recentClip\.dismiss\(\)'; then
  echo "Android clipboard insertion must retire the recent-clip suggestion (#5692)" >&2
  exit 1
fi
# 剪贴板历史关着时，「最近复制」既不显示也不读剪贴板：本地开关只能经 recentClipEnabled（RecentClipboardSuggestion.enabled）和历史开关一起判断，补看在读剪贴板之前就要问它，系统标为敏感的内容从不提供，文字和图片（#6781）都一样。
service_java="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
offer_body=$(rg -A 22 'void offerRecentClip\(' "$service_java" || true)
offer_gate_line=$(printf '%s\n' "$offer_body" | rg -n 'if \(!recentClipEnabled\(\)' | head -n 1 | cut -d: -f1 || true)
offer_read_line=$(printf '%s\n' "$offer_body" | rg -n 'getPrimaryClipDescription|readPrimaryClip|getPrimaryClip\(' | head -n 1 | cut -d: -f1 || true)
if ! rg -q 'return RecentClipboardSuggestion\.enabled\(clipboardHistoryEnabled, clipboardSuggestionEnabled\);' "$service_java" \
  || rg -n 'clipboardSuggestionEnabled' "$service_java" \
    | rg -v 'boolean clipboardSuggestionEnabled = true;|clipboardSuggestionEnabled = localSettings\.bool|RecentClipboardSuggestion\.enabled\(clipboardHistoryEnabled, clipboardSuggestionEnabled\)' \
    | rg_q_stdin . \
  || [[ -z "$offer_gate_line" || -z "$offer_read_line" || "$offer_read_line" -lt "$offer_gate_line" ]] \
  || ! rg -q 'clip == null \|\| clip\.sensitive\(\)\) return;' <<< "$offer_body" \
  || ! rg -q 'if \(image\.sensitive\(\)\) return;' <<< "$offer_body" \
  || ! rg -B 2 -A 2 'String recent = ' "$service_java" | rg_q_stdin 'recentClipEnabled\(\)'; then
  echo "Android recent-clip suggestion must stay off, and must not read the clipboard, while clipboard history is off; sensitive clips are never offered (#5692)" >&2
  exit 1
fi
# 「最近复制」的图片（#6781）只在当前输入框经 contentMimeTypes 声明收得下、并且副本的扩展名经 FileProvider 反查仍是图片时占这一行；发出去的是本应用 FileProvider 给的缓存副本并带读权限，不是剪贴板里的原 URI，副本目录必须在 FileProvider 的路径表里。
if ! rg -A 6 'boolean recentImageSendable\(' "$service_java" \
    | rg_q_stdin 'ClipboardImagePolicy\.accepts\(info\.contentMimeTypes, mimeType\)' \
  || ! rg -A 7 'boolean recentImageSendable\(' "$service_java" \
    | rg_q_stdin 'clipboardImageExtension\(mimeType\) != null' \
  || ! rg -q 'ClipboardImageCache\.replace\(input, directory, extension,' "$service_java" \
  || ! rg -q 'boolean recentImage = recentAllowed && recentImageSendable\(nowMs\);' "$service_java" \
  || ! rg -q 'Uri uri = ClipboardImageCache\.contentUri\(this, copy\);' "$service_java" \
  || ! rg -q 'InputConnection\.INPUT_CONTENT_GRANT_READ_URI_PERMISSION' "$service_java" \
  || ! rg -qF '<cache-path name="clipboard-images" path="clipboard-images/" />' \
    "$repo_root/platforms/android/res/xml/msime_file_paths.xml"; then
  echo "Android recent-clip images must show only in editors that accept them and be sent as a FileProvider copy from cache/clipboard-images (#6781)" >&2
  exit 1
fi
# 水杉娘表情包发出去的 PNG 放在 cache/sticker-images，必须在 FileProvider 的路径表里；键盘自己放进剪贴板的贴纸会触发自己的剪贴板监听，「最近复制」不能把它当成用户复制的图片再提供一次。那一张只在写下一张时替换，不定时删、不随键盘进程启动或退出清空：贴纸是随包素材，剪贴板里的 URI 要在进程被回收后仍能粘贴。
if ! rg -qF '<cache-path name="sticker-images" path="sticker-images/" />' \
    "$repo_root/platforms/android/res/xml/msime_file_paths.xml" \
  || ! rg -q 'if \(imeStickers\.ownsClip\(image\.uri\(\)\)\) return;' <<< "$offer_body" \
  || rg -q 'postDelayed|ClipboardImageCache\.clear\(' "$repo_root/platforms/android/java/app/msime/android/core/ImeStickers.java"; then
  echo "Android stickers must be shared from cache/sticker-images through the FileProvider, replaced only by the next sticker, and never offered back as a recent copied image" >&2
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
    | rg_q_stdin '!clip\.sensitive\(\)'; then
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
# Ctrl+Enter 上屏译文（#7079）同样要排在修饰键分支之前，否则那里先上屏原文、再把回车交给应用；义项页的按键、哪些键记作输入法的键，以及在线刷新时页面是否保留，也都要经同一个策略决定，HardwareTranslationCommitPolicySmoke 才测得到真实的路由。
if ! rg -q 'HardwareTranslationCommitPolicy\.ctrlEnter' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'HardwareTranslationCommitPolicy\.pageKey' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'HardwareTranslationCommitPolicy\.holdsKey' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'imeTranslationSenses\.followOnlineRefresh\(next\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'HardwareTranslationCommitPolicy\.pageSurvivesRefresh' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeTranslationSenses.java"; then
  echo "Android Ctrl+Enter translation commit must route through HardwareTranslationCommitPolicy" >&2
  exit 1
fi
# The Japanese model is process-cached across Android input sessions. The host must release that
# cache on system memory pressure even after the current session has already been destroyed.
if ! rg -q '@Override public void onTrimMemory\(int level\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'NativeClient\.releaseProcessCache\(\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android IME must release process-level model caches on memory pressure" >&2
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
    | rg_q_stdin 'applyPreferencesSnapshot\(value\(livePreferences\)\)' \
  || ! rg -q '"handwriting_theme", "touch_toolbar"\}' "$account_service" \
  || ! rg -q 'live \|\| rememberedToolbar == null' "$account_service"; then
  echo "Android toolbar must not start each editor from the factory-default preference copy" >&2
  exit 1
fi
# 输入框切换会让旧的启动任务失效；旧任务不能继续占住单线程偏好工作队列，否则新的会话会一直显示「共享运行时准备中」（#7728）。
startup_body=$(sed -n '/private void scheduleEngineStartup(String options, long generation)/,/^    }$/p' "$account_service")
if ! rg -q 'private volatile long engineStartGeneration;' "$account_service" \
  || [[ "$(printf '%s\n' "$startup_body" | rg -c 'if \(generation != engineStartGeneration\) return;')" -lt 3 ]]; then
  echo "Android engine startup must abandon stale generations before and after background work (#7728)" >&2
  exit 1
fi
# 引擎启动是后台工作，不能把内部的「准备中」画成状态行；状态行出现会临时增加候选头高度，首帧九键随后又跳回去（#8473）。
if rg -n 'message = "共享运行时准备中"' "$account_service"; then
  echo "Android engine startup must not expose its transient preparation state in the keyboard" >&2
  exit 1
fi
# 候选条的配色和字号同理（#5933）：副本是出厂的薄荷设计，按它重算会让每个新输入框的候选条先铺一层薄荷底、字号回到出厂的 18/15，实时偏好到了才换回来。只有实时偏好重算候选条外观，冷启动的第一帧按皮肤片段算，片段里要记着候选条用到的字段；按片段着色之前要先读本地设置，否则应用主题种子按默认值缓存。
if ! sed -n '/private void applyEditorPreferences(JSONObject preferences,$/,/^    }$/p' "$account_service" \
    | rg_q_stdin '^\s*if \(live\) applyCandidateAppearance\(preferences\);' \
  || ! sed -n '/JSONObject hint = readSkinHint();/,/^        }$/p' "$account_service" \
    | rg_q_stdin 'applyCandidateAppearance\(hint\);' \
  || ! sed -n '/super\.onCreate();/,/JSONObject hint = readSkinHint();/p' "$account_service" \
    | rg_q_stdin 'refreshLocalSettings\(\);' \
  || ! rg -q '"candidate_theme", "candidate_font_family"' "$account_service" \
  || ! rg -q 'private boolean candidateHorizontal = true;' "$account_service"; then
  echo "Android candidate strip must not start each editor from the factory-default preference copy" >&2
  exit 1
fi
# 键距、行距、语音快捷键和双拼键位提示同理：按副本算，调过键距的用户每换一个输入框键盘都先按出厂间距排一帧。只有实时偏好重算键盘几何，冷启动的第一帧按皮肤片段算，片段里要记着这几个字段。
if ! sed -n '/private void applyEditorPreferences(JSONObject preferences,$/,/^    }$/p' "$account_service" \
    | rg_q_stdin '^\s*if \(live\) applyTouchGeometry\(preferences\);' \
  || ! sed -n '/JSONObject hint = readSkinHint();/,/^        }$/p' "$account_service" \
    | rg_q_stdin 'applyTouchGeometry\(hint\);' \
  || ! rg -q '"touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",' "$account_service" \
  || ! rg -q '"touch_voice_shortcut", ShuangpinKeyHintPolicy\.PREFERENCE_KEY,' "$account_service"; then
  echo "Android keyboard geometry must not start each editor from the factory-default preference copy" >&2
  exit 1
fi
# 键盘开着时切换「双拼键位提示」：reloadPreferences 只在 touchGeometryKey() 变了时才 render()，开关不在这个键里，已经画好的字母键就一直留着旧提示，直到下次重建键盘。
if ! sed -n '/private String touchGeometryKey() {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'shuangpinKeyHintsEnabled'; then
  echo "Android touchGeometryKey must include the shuangpin key hint switch so a live change redraws the keys" >&2
  exit 1
fi
# 长按「中/英」弹出系统输入法选择框（#5615）。这个键在没有会话的输入框里也必须保持可用：禁用的按钮收不到长按，而密码框正是最需要换到密码管理器键盘的地方。没有会话时把键画淡，点按在反馈和计数之前就忽略。
if ! rg -q 'bindInputMethodPicker\(languageButton\)' "$account_service" \
  || ! rg -q 'manager\.showInputMethodPicker\(\)' "$account_service" \
  || rg -q 'setEnabled\(languageButton, session != 0\)' "$account_service" \
  || ! rg -q 'setActiveAlpha\(languageButton, canToggle' "$account_service" \
  || ! sed -n '/languageButton\.setOnClickListener/,/});/p' "$account_service" | rg_q_stdin 'if \(session == 0\) return;' \
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
# 拼音九键的 1 键（@#）空闲时把常用标点放进候选条（ImeNineKeyQuickPunctuation），符号面板改从九键外框底行最左边的 符 打开（KeyboardActionRow.designEntries，KeyboardActionRowSmoke 覆盖排布）；这些标点只留到下一次 Engine 转换。候选条的接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住三处：1 键不再直接打开面板、apply() 收起标点、底行的 SYMBOL_PANEL 是 符 键。
symbol_key_body=$(sed -n '/private void symbolKey(NineKeyLayout\.Key key) {$/,/^    }$/p' "$layout_rows")
if ! rg -q 's\.imeNineKeyQuickPunctuation\.show\(\)' <<< "$symbol_key_body" \
  || rg -q 'showSymbolPanel' <<< "$symbol_key_body" \
  || ! sed -n '/boolean apply(String response) throws JSONException {$/,/^    }$/p' "$account_service" | rg_q_stdin 'imeNineKeyQuickPunctuation\.clear\(\)' \
  || ! rg -q 'case SYMBOL_PANEL -> s\.symbolPanelButton;' "$repo_root/platforms/android/java/app/msime/android/core/ImeBottomRow.java"; then
  echo "The Android nine-key 1 key must lay quick punctuation into the candidate strip until the next Engine transition, and 符 must open the symbol panel from the bottom row" >&2
  exit 1
fi
# 拼音九键格子上的长按数字画在右上角，字号和位置与 26 键字母的角标相同（KeyHintButton 的 CORNER_HINT_SP、CORNER_HINT_TOP_DP、CORNER_HINT_RIGHT_DP，基线按 ascent 算），iOS 的 keyNumberHint 也是上 3、右 5。绘制在 onDraw 里，JVM 冒烟碰不到，这里钉住它取的是同一组常量，而不是另写一套数字。
nine_key_digit_draw=$(sed -n '/protected void onDraw(Canvas canvas) {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/keyboard/NineKeyDigitButton.java")
for pattern in 'KeyHintButton\.CORNER_HINT_SP' 'KeyHintButton\.CORNER_HINT_TOP_DP\) - metrics\.ascent' \
    'getWidth\(\) - DimensionPolicy\.floatPixels\(getContext\(\), KeyHintButton\.CORNER_HINT_RIGHT_DP\)'; do
  if ! rg -q "$pattern" <<< "$nine_key_digit_draw"; then
    echo "The Android nine-key digit hint must sit in the top-right corner exactly like the letter-key corner hint (KeyHintButton): missing $pattern" >&2
    exit 1
  fi
done
# 数字角标与字母角标同色：皮肤的 hint 色，不用强调色（原生皮肤下是系统蓝，压在每个键上太显眼）。
if ! rg -q 'digitButton\.setDigitColor\(color\(target\.hint\(\)\)\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java"; then
  echo "The Android nine-key digit hint must use the skin's hint colour like the letter-key corner hint" >&2
  exit 1
fi
# 文本编辑面板（#5625）是工具栏面板的一员：closeToolbarPanels 要关掉它、anyToolbarPanelOpen 要算上它，否则换输入框时它会留在下一个编辑器的键盘上，收起键也不会变成「返回键盘」。
if ! sed -n '/void closeToolbarPanels()/,/^    }$/p' "$account_service" | rg_q_stdin 'imeTextEditPanel\.close\(\)' \
  || ! sed -n '/boolean anyToolbarPanelOpen()/,/^    }$/p' "$account_service" | rg_q_stdin 'shown\(textEditPanel\)|ViewPolicy\.isVisible\(textEditPanel\)'; then
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
# 键区里的系统识别服务是 SpeechRecognizer 回调接线，JVM 冒烟只能覆盖 PlatformSpeechPolicy 和 ImeVoiceEntry.choose 这些纯逻辑，这里守住回调里不能被悄悄改回去的几处（#5553）：两条入口都按错误码提示、空结果不冒用错误码，权限拒绝或聆听前启动失败时转交识别窗口，系统识别服务不被 1.5 s 停顿截断，说完后收回音量光圈。
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
    "$repo_root/platforms/android/native/client_jni.cpp" \
    "$repo_root/platforms/android/native/JniCallbacks.cpp"
  echo "client_jni.cpp, JniCallbacks.cpp: aarch64-linux-android compile against the shared header passed"
else
  echo "client_jni.cpp: skipped (pinned NDK 28.2.13676358 not installed)"
fi
# 会话存储和 listener 的回调进 Java 的那一半（native/JniCallbacks.cpp）只能在真的 JVM 上验：没 attach 的线程要自己 attach、返回前 detach，本来就 attach 着的线程不能被它 detach，Java 抛出的异常要清掉、不留给调用 Rust 的一方，release 要放掉全局引用。模拟器上的设备冒烟造不出没 attach 的线程（Rust 在自己的线程上回调存储，例如 AI 皮肤出图遇到 401 时刷新令牌，离线的设备测试触发不了），所以这里用 JDK 的 JNI 调用接口起一个带 -Xcheck:jni 的 JVM 直接驱动它，-Xcheck:jni 报出的 WARNING 也算失败。没有 JDK 头文件、libjvm 或宿主 C++ 编译器的机器跳过并说明。
java_home=$(java -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java\.home = //p' | head -n 1)
case $(uname -s) in
  Darwin) jni_platform=darwin; libjvm=libjvm.dylib ;;
  Linux) jni_platform=linux; libjvm=libjvm.so ;;
  *) jni_platform=""; libjvm="" ;;
esac
host_cxx=${CXX:-c++}
if [[ -n "$jni_platform" && -n "$java_home" && -f "$java_home/include/jni.h" && -f "$java_home/lib/server/$libjvm" ]] \
    && command -v "$host_cxx" >/dev/null 2>&1; then
  jni_test_dir="$output_dir/jni-callbacks"
  mkdir -p "$jni_test_dir"
  javac --release 17 -Xlint:all -Werror -d "$jni_test_dir" "$repo_root/platforms/android/tests/native/JniCallbacksFixture.java"
  "$host_cxx" -std=c++17 -Wall -Wextra -Werror -pthread \
    -I"$java_home/include" -I"$java_home/include/$jni_platform" \
    "$repo_root/platforms/android/tests/native/JniCallbacksTest.cpp" \
    "$repo_root/platforms/android/native/JniCallbacks.cpp" \
    -L"$java_home/lib/server" -ljvm -Wl,-rpath,"$java_home/lib/server" \
    -o "$jni_test_dir/jni-callbacks-test"
  jni_test_output=$("$jni_test_dir/jni-callbacks-test" "$jni_test_dir" 2>&1) || {
    printf '%s\n' "$jni_test_output" >&2
    echo "JniCallbacks.cpp failed on the desktop JVM" >&2
    exit 1
  }
  printf '%s\n' "$jni_test_output"
  if [[ "$jni_test_output" == *WARNING* ]]; then
    echo "JniCallbacks.cpp: -Xcheck:jni reported a JNI misuse" >&2
    exit 1
  fi
else
  echo "JniCallbacks.cpp: desktop JVM check skipped (no JDK headers and libjvm, or no host C++ compiler)"
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
# 登录会话、匿名会话、登录和刷新都由 Rust 持有（#8201 迁移计划 5b）：Java 不再自己发 /v1/auth/ 下的请求（登录、刷新、挑战、Apple 网页授权、登录方式），这三份会话存储的名字只出现在注入给 Rust 的存储适配器 AccountStore 里。存储注册时不置 SHARED，Rust 读一次就缓存；主进程里再有一个 Java 写入方，Rust 就会拿旧的刷新令牌去刷新，服务端随即吊销整个会话。
if rg -n '"/v1/auth/' "$repo_root/platforms/android/java"; then
  echo "Android Java must not send /v1/auth/ requests itself; Rust owns sign-in and refresh" >&2
  exit 1
fi
if rg -n 'msime_account_session_v2|msime_anonymous_session_v1|msime_anonymous_account_v1' "$repo_root/platforms/android/java" \
    | rg -v '/account/AccountStore\.java:'; then
  echo "Only AccountStore may name the session stores Rust owns" >&2
  exit 1
fi
# 账号页、我的设备、云端数据、反馈、诊断上传、下载链接、AI 皮肤和社区都改调 Rust 的 msime_client_account、msime_client_community 和 msime_client_ai_skin_generate（#8201 迁移计划 5c）：这些请求路径不能再出现在 Java 里，Java 只拼 Rust 的请求、读它的回答。云剪贴板页不涉及内容的开关、保留时长、置顶、删除和清空也经 Rust，只有端到端加密驱动还用 CloudClipboardApi.PATH；AI 对话页经 Rust 的流式入口，不再用 BackendAccount 的 Java 实现。
if rg -n '"/v1/(users/me/(sessions|data|avatar|diagnostics|download-link)|feedback|skins/jobs|community/|models)' "$repo_root/platforms/android/java"; then
  echo "Android Java must not send account page, feedback, diagnostics, download-link, AI skin or community requests itself; Rust owns them" >&2
  exit 1
fi
if rg -n 'PATH \+ "/' "$repo_root/platforms/android/java/app/msime/android/account/CloudClipboardApi.java" \
  || rg -n 'BackendAccount' "$repo_root/platforms/android/java/app/msime/android/home/ChatFragment.java"; then
  echo "Android cloud clipboard settings and AI chat must go through Rust" >&2
  exit 1
fi
# 「水杉账号」的候选释义（#8201 迁移计划 5e）：请求体怎么拼、回答怎么读都在 Rust 的 msime_client_account_gloss，路径也由它给；Java 只带着 provider 的 shared_access_token 把那一次请求发出去，不能再自己拼 /v1/translate 或解读译文。
if rg -n '"/v1/translate"' "$repo_root/platforms/android/java" \
  || ! rg -q 'NativeClient\.accountGloss\(' "$repo_root/platforms/android/java/app/msime/android/voice/BackendTranslationClient.java"; then
  echo "Android account glosses must be built and read by Rust's msime_client_account_gloss" >&2
  exit 1
fi
# 重复实现删掉以后（#8201 迁移计划 5f），Java 里还自己拼水杉云端请求路径的只剩三处：云剪贴板的端到端加密驱动（设置页的 CloudClipboardApi 和两边共用的 CloudClipboardE2e，路径由 Rust 的驱动给、这里只核对前缀），以及不在迁移范围内的打字统计与 QQ 绑定（TypingUploadApi）。账号、登录、同步、对话、云剪贴板开关和删除、`/v1/users/me` 都经 Rust，其他地方再出现 `"/v1/` 就是又在 Java 里写了一份。
if rg -n '"/v1/' "$repo_root/platforms/android/java" \
    | rg -v '/account/(CloudClipboardApi|CloudClipboardE2e|TypingUploadApi)\.java:'; then
  echo "Android Java must not build Metasequoia cloud request paths outside the clipboard driver and typing upload; Rust owns them" >&2
  exit 1
fi
# 词库快照激活队列和云同步的状态都由 Rust 持有（#8201 迁移计划 5d）：Java 的 FileChannel.lock 是 fcntl，与 Rust 的 flock 互相看不见，所以 Java 里不能再有读写队列状态（旧的 state.bin）或加队列锁的代码，也不能再读写云同步的进度（cursor_、gen_、clean_ 这几类键、开关、上次同步时间），记改动只经 SyncSignals 调 Rust 的 mark_dirty。旧状态只在 RustCloudSync 里整份交给 Rust 的 import_legacy。
if rg -n 'FileChannel|FileLock|DataOutputStream|DataInputStream' \
    "$repo_root/platforms/android/java/app/msime/android/dictionary/DictionarySnapshotQueue.java" \
    "$repo_root/platforms/android/java/app/msime/android/dictionary/DictionarySnapshotWorker.java" \
  || rg -n '"state\.bin"|"bootstrap/state/dictionary-snapshots"|dictionary-snapshots"' "$repo_root/platforms/android/java" \
    | rg -v '/dictionary/DictionarySnapshotQueue\.java:' \
  || rg -n '"(cursor|gen|clean)_"|"last_synced_at"|"phrases_unheld"|KEY_ENABLED' "$repo_root/platforms/android/java"; then
  echo "Android Java must not read or write the snapshot queue or cloud sync progress itself; Rust owns both" >&2
  exit 1
fi
if ! rg -q '"import_android_v1"' "$repo_root/platforms/android/java/app/msime/android/dictionary/DictionarySnapshotQueue.java" \
  || ! rg -q '"import_legacy"' "$repo_root/platforms/android/java/app/msime/android/account/RustCloudSync.java" \
  || ! awk '/static void migrate\(Context context\)/ { inside = 1 } inside && /importAndroidV1\(queue\)/ { queue = NR } inside && /importLegacy\(/ { legacy = NR } inside && /^    }$/ { exit } END { exit !(queue && legacy && queue < legacy) }' \
      "$repo_root/platforms/android/java/app/msime/android/account/RustCloudSync.java"; then
  echo "Android must convert the old snapshot queue before importing the old cloud sync progress" >&2
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
# 词库页的大标题行只放标题：右侧那排胶囊会在窄屏上把「词库」挤成「词…」（#5682）。低频管理动作收进「已安装」标题旁的菜单，正常列表下面不再放四行操作卡片；首次读取失败的单独刷新入口仍保留。
lexicon_page="$repo_root/platforms/android/java/app/msime/android/home/LexiconPage.java"
if ! rg -q 'installed\.headerAction\(manageButton\(context\)\)' "$lexicon_page" \
  || ! rg -q 'new PopupMenu\(requireContext\(\), anchor\)' "$lexicon_page" \
  || ! rg -q 'menu\.getMenu\(\)\.add\("新建词库"\)' "$lexicon_page" \
  || ! rg -q 'menu\.getMenu\(\)\.add\("导入词库"\)' "$lexicon_page" \
  || ! rg -q 'menu\.getMenu\(\)\.add\("刷新词库"\)' "$lexicon_page" \
  || ! rg -q 'menu\.getMenu\(\)\.add\("导出词库"\)' "$lexicon_page" \
  || rg -q 'GroupCard manage =|manage\.addView\(KeyboardSheets\.actionRow' "$lexicon_page"; then
  echo "Android lexicon page must expose its four management actions from the installed header menu" >&2
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
  "$repo_root/platforms/android/java/app/msime/android/home/HelpcodeChoicePolicy.java" \
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
# 设备测试包里编进了 NativeClient，却没有 libmsime_android.so：targetPackage 是测试包自己（app.msime.android.test）的 instrumentation 跑在测试包的进程里，第一次碰到 NativeClient 就是 UnsatisfiedLinkError。要经 JNI 调 Rust 的设备冒烟得以产品包为目标，跑进应用进程，那里的库路径带着应用的原生库（测试包的类排在前面，见 CloudClipboardDriverDeviceSmoke）。BackendAccountSessionDeviceSmoke 的释义用例曾经这样只编译过、一上设备就失败。这里按 jdeps 给出的类引用从每个这样的 instrumentation 往下找，能走到 NativeClient 就失败，并打印那条引用链。
device_self_targets=$(rg -o -r '$1' 'instrumentation android:name="([^"]+)" android:targetPackage="app\.msime\.android\.test"' \
  "$repo_root/platforms/android/tests/device/AndroidManifest.xml" || true)
device_class_deps=$(jdeps -verbose:class -filter:none -cp "$android_jar" "$output_dir/device" 2>/dev/null)
if ! DEVICE_SELF_TARGETS="$device_self_targets" awk '
  $2 == "->" && $3 ~ /^app\.msime\./ { from = $1; to = $3; sub(/\$.*/, "", from); sub(/\$.*/, "", to); edges[from] = edges[from] " " to }
  END {
    native = "app.msime.android.NativeClient"; bad = 0
    count = split(ENVIRON["DEVICE_SELF_TARGETS"], list, "\n")
    for (i = 1; i <= count; i++) {
      delete seen; delete via; head = 1; tail = 1; queue[1] = list[i]; seen[list[i]] = 1
      while (head <= tail) {
        current = queue[head++]
        width = split(edges[current], targets, " ")
        for (j = 1; j <= width; j++) if (targets[j] != "" && !(targets[j] in seen)) { seen[targets[j]] = 1; via[targets[j]] = current; queue[++tail] = targets[j] }
      }
      if (native in seen) {
        chain = native; step = native
        while (step in via) { step = via[step]; chain = step " -> " chain }
        print "Android device smoke reaches NativeClient but runs in the test package without libmsime_android.so; target app.msime.android instead: " chain > "/dev/stderr"
        bad = 1
      }
    }
    exit bad || count == 0
  }' <<< "$device_class_deps"; then
  echo "Android device smokes that call Rust over JNI must instrument app.msime.android" >&2
  exit 1
fi
# Resources are compiled but not linked here: they reference Material's theme attributes, and linking
# those needs the library's own resources, which is Gradle's job. Compiling still catches a malformed
# drawable, layout or values file, which is what this step was for.
"$android_sdk/build-tools/35.0.0/aapt2" compile --dir "$repo_root/platforms/android/res" -o "$output_dir/resources.zip"
for alias in MainActivityClassic MainActivityForest MainActivitySky MainActivityDusk MainActivityVermilion; do
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
# `deleteSurroundingText` 只删选区以外的字：选中开头的「你好」按删除毫无反应，选中中间的文字会删掉选区前一个字。键盘上的删除键都要经 `deleteCodePointBeforeCursor`，由它先删选区；笔画布局曾经绕过它直接调 InputConnection。只允许该入口和它使用的断开保护层调用码位删除，布局不能绕过入口。
if ! rg -q 'if \(deleteSelection\(\)\) return;' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || rg -l '\.deleteSurroundingTextInCodePoints\(' "$repo_root/platforms/android/java" \
    | rg -v '/core/(MSIMEInputService|InputConnectionGuard)\.java$' >/dev/null; then
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
