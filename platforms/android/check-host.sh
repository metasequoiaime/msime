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
# 日语九键左列始终有 符（#10439）：原来只在工具栏没有表情按钮时多一格 ☺，没有符号面板入口。表情改由 符 的长按承担，左列不再单独放 ☺，不会多出第六格。
japanese_nine_key_rows=$(sed -n '/    void rebuildJapaneseNineKeyRows() {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java")
japanese_symbol_key=$(sed -n '/    private Button japaneseSymbolPanelKey(boolean emojiLongPress) {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java")
if ! rg_q_stdin -F 'addJapaneseSideKey(modeColumn, japaneseSymbolPanelKey(s.japaneseSideEmojiKey()), 1);' <<< "$japanese_nine_key_rows" \
  || rg_q_stdin -F '"SoftEmoji"' <<< "$japanese_nine_key_rows" \
  || ! rg_q_stdin -F 's.imePanels.showSymbolPanel();' <<< "$japanese_symbol_key" \
  || ! rg_q_stdin -F '}), "SoftSymbol");' <<< "$japanese_symbol_key" \
  || ! rg_q_stdin -U 'if \(emojiLongPress\) \{\n\s*symbol\.setOnLongClickListener' <<< "$japanese_symbol_key" \
  || ! rg_q_stdin -F 's.imePanels.showEmojiPicker();' <<< "$japanese_symbol_key"; then
  echo "The Japanese nine-key left column must always carry the 符 symbol-panel key, with emoji on its long press when the toolbar has no emoji button" >&2
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
# 26 键双拼的按下点（#10018）只经 type(char, View) 的最后一步送给引擎：辅助码的 Shift、英文直出、标点、字母网格和实体键盘都照常走 msime_client_character。哪一键带按下点由 TouchPointPolicySmoke 覆盖；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：NativeClient.touchCharacter 全宿主只有一处调用，在 touchCharacter 里，而它只由 type(char, View) 在 ImeTouchPoints.geometryFor 给出代次时调用。
android_java="$repo_root/platforms/android/java/app/msime/android"
touch_calls=$({ rg -c 'NativeClient\.touchCharacter\(' "$android_java" || true; } | awk -F: '{ total += $2 } END { print total + 0 }')
type_body=$(sed -n '/^    void type(char key, View source) {$/,/^    }$/p' "$android_java/core/MSIMEInputService.java")
touch_body=$(sed -n '/^    private boolean touchCharacter(int ascii, int geometryId, float x, float y) {$/,/^    }$/p' \
  "$android_java/core/MSIMEInputService.java")
if [[ "$touch_calls" != 1 ]] || ! rg -q 'NativeClient\.touchCharacter\(' <<< "$touch_body" \
  || ! rg -q 'imeTouchPoints\.geometryFor\(' <<< "$type_body" \
  || [[ $(rg -c 'touchCharacter\(output' "$android_java/core/MSIMEInputService.java") != 1 ]] \
  || ! rg -q 'touchCharacter\(output' <<< "$type_body"; then
  echo "Double-pinyin touch-downs must reach msime_client_touch_character only from type(char, View)" >&2
  exit 1
fi
# 按键区只给点击时会取走按下点的键（shuangpinLetterKey 造的 ShuangpinHintButton：26 个字母和微软双拼的 `;`）记按下点。数字行、Shift、退格的按下从不被取走，记下来只会占着槽位；槽满时让出哪一个由 TouchPointPolicySmoke 覆盖，这里钉住接线。
if [[ $(rg -c 'setTouchPoints\(' "$android_java/core/MSIMEInputService.java") != 1 ]] \
  || ! rg -q 'setTouchPoints\(imeTouchPoints\.pending, key -> key instanceof ShuangpinHintButton\);' \
    "$android_java/core/MSIMEInputService.java" \
  || ! rg -q 'pressed != null && touchKey\.test\(pressed\)' "$android_java/keyboard/KeyboardKeyArea.java"; then
  echo "Only double-pinyin letter keys may record touch-downs in the key area" >&2
  exit 1
fi
# 文本编辑面板只在同一输入框重启连接时留着（#10141）。手势改认新连接由 TextTrackpadGestureSmoke 和 TextTrackpadHoldSmoke 覆盖；判断是不是同一输入框、走哪条收尾的接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：同一输入框还要求初始选区等于最近的回报（只比 fieldId 分不出同一个 WebView 里的两个输入框），两条 onStartInput 路径都按它在 editorRestarted 和 editorChanged 之间选，stop 只因它跳过关这个面板、更多工具照关；editorRestarted 全宿主只在 onStartInput 里调用。
start_input_body=$(sed -n '/^    @Override public void onStartInput(EditorInfo info, boolean restarting) {$/,/^    }$/p' \
  "$android_java/core/MSIMEInputService.java")
stop_body=$(sed -n '/^    private void stop(boolean finish, boolean keepMoreTools, boolean keepSession, boolean keepTextEdit) {$/,/^    }$/p' \
  "$android_java/core/MSIMEInputService.java")
restarted_calls=$({ rg -c 'editorRestarted\(' "$android_java" || true; } | awk -F: '{ total += $2 } END { print total + 0 }')
if ! rg -q 'sameEditor\(info\) && imeTextEditPanel\.reportedSelection\(info\.initialSelStart, info\.initialSelEnd\);' <<< "$start_input_body" \
  || [[ $(rg -c 'if \(sameEditorRestart\) \{' <<< "$start_input_body" || true) != 2 ]] \
  || [[ $(rg -c 'imeTextEditPanel\.editorRestarted\(' <<< "$start_input_body" || true) != 2 ]] \
  || [[ "$restarted_calls" != 3 ]] \
  || ! rg -q 'if \(keepSession\) stopKeepingSession\(sameEditorRestart\);' <<< "$start_input_body" \
  || ! rg -q 'else stop\(false, sameEditorRestart\);' <<< "$start_input_body" \
  || ! rg -q 'if \(keepMoreTools \|\| keepTextEdit\) \{' <<< "$stop_body" \
  || ! rg -q 'if \(!keepMoreTools\) closeMoreTools\(\);' <<< "$stop_body" \
  || ! rg -q 'if \(!keepTextEdit\) imeTextEditPanel\.close\(\);' <<< "$stop_body"; then
  echo "The text edit panel may survive only a same-field restart (same editor and the last reported selection); stop must still close More Tools" >&2
  exit 1
fi
# 功能面板「手写」磁贴再点一次要回到进入手写前的入口和中英状态（#9703）。0.5.0 取的是启用列表里第一个非手写入口，默认全拼 9 键、英文 26 键或小鹤双拼的用户关掉手写后都落到全拼 26 键。入口怎么选由 KeyboardSchemeSmoke 覆盖；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：离开手写经 handwritingReturn 取入口，并把记下的英文状态交给方案保存，保存成功后再进英文。
toggle_body=$(sed -n '/^    void toggleHandwritingScheme() {$/,/^    }$/p' \
  "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java")
if ! rg -q 'KeyboardScheme\.handwritingReturn\(schemeBeforeHandwriting' <<< "$toggle_body" \
  || ! rg -q 'englishBeforeHandwriting' <<< "$toggle_body" \
  || ! rg -q 'if \(english && !dedicatedEnglish\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Leaving handwriting from the function panel must return to the scheme and English mode it was entered from" >&2
  exit 1
fi
# 剪贴板单条、左滑和多选删除前的系统认证门禁（#10408，.agents/notes/implemented/feature/2026-10-31-android-clipboard-delete-auth.md）。何时要求认证、没有锁屏凭据时何时放行由 ClipboardDeleteAuthenticationPolicySmoke 覆盖；接线在 InputMethodService 和面板里，JVM 冒烟碰不到，这里钉住：两个删除入口都经 authenticateClipboardDelete，它按 ClipboardDeleteAuthenticationPolicy 决定是否认证，并把「设备没有可用认证器」交给 allowsWithoutAuthenticator，拒绝时的提示告诉用户可以关掉开关；设置页开关在认证桥返回 2002 时按 togglesWithoutAuthenticator 只放行关闭，打开开关后又移除锁屏凭据的用户才关得掉它；真正删除的两个方法各只有删除入口一处调用，绕不过门禁；认证桥的 RESULT_UNAVAILABLE（2002）在服务里单独处理，不落到「未通过系统认证」；多选只在删除真正发生后退出选择，取消认证时选择还在；认证 Activity 被直接结束时补发取消，发起方的等待标记才能复位。
clipboard_service_java="$android_java/core/MSIMEInputService.java"
clipboard_auth_activity="$android_java/ClipboardAuthenticationActivity.java"
clipboard_toggle_body=$(sed -n '/^    private void requestClipboardDeleteAuthentication(GroupCard\.Row row, boolean current, boolean next) {$/,/^    }$/p' "$android_java/home/KeyboardOptionsPage.java")
remove_item_body=$(sed -n '/^    void removeClipboardItem(ClipboardHistory\.Item item) {$/,/^    }$/p' "$clipboard_service_java")
remove_items_body=$(sed -n '/^    void removeClipboardItems(java\.util\.List<String> texts, Runnable removed) {$/,/^    }$/p' "$clipboard_service_java")
delete_gate_body=$(sed -n '/^    private void authenticateClipboardDelete(Runnable delete) {$/,/^    }$/p' "$clipboard_service_java")
clipboard_auth_body=$(sed -n '/^    private void authenticateClipboard(String operation, Runnable after, Runnable whenUnavailable) {$/,/^    }$/p' "$clipboard_service_java")
auth_activity_destroy_body=$(sed -n '/^    @Override protected void onDestroy() {$/,/^    }$/p' "$clipboard_auth_activity")
remove_item_calls=$(rg -c 'removeClipboardItemAfterAuthentication\(' "$clipboard_service_java" || true)
remove_items_calls=$(rg -c 'removeClipboardItemsAfterAuthentication\(' "$clipboard_service_java" || true)
if ! rg -q 'authenticateClipboardDelete\(\(\) -> removeClipboardItemAfterAuthentication\(item\)\);' <<< "$remove_item_body" \
  || ! rg -q 'authenticateClipboardDelete\(\(\) -> \{' <<< "$remove_items_body" \
  || ! rg -q 'removeClipboardItemsAfterAuthentication\(snapshot\);' <<< "$remove_items_body" \
  || ! rg -q 'removed\.run\(\);' <<< "$remove_items_body" \
  || [[ "$remove_item_calls" != 2 ]] || [[ "$remove_items_calls" != 2 ]] \
  || ! rg -q 'ClipboardDeleteAuthenticationPolicy\.requiresAuthentication\(enabled,' <<< "$delete_gate_body" \
  || ! rg -q 'clipboardMode == ClipboardModePolicy\.Mode\.VISITOR' <<< "$delete_gate_body" \
  || ! rg -U -q 'ClipboardDeleteAuthenticationPolicy\.allowsWithoutAuthenticator\(enabled\)\n\s+\? delete : \(\) -> notice\(CLIPBOARD_DELETE_UNAVAILABLE_NOTICE\)\);' <<< "$delete_gate_body" \
  || ! rg -q 'private static final String CLIPBOARD_DELETE_UNAVAILABLE_NOTICE = ".*关闭「剪贴板删除数据验证」";' "$clipboard_service_java" \
  || ! rg -U -q 'code == ClipboardAuthenticationActivity\.RESULT_UNAVAILABLE\n\s+&& ClipboardDeleteAuthenticationPolicy\.togglesWithoutAuthenticator\(next\)\) \{\n\s+saveLocalSetting\(AndroidLocalSettings\.CLIPBOARD_DELETE_AUTHENTICATION, next\);' <<< "$clipboard_toggle_body" \
  || ! rg -q 'code == CLIPBOARD_AUTH_SUCCEEDED' <<< "$clipboard_auth_body" \
  || ! rg -q 'code == CLIPBOARD_AUTH_UNAVAILABLE' <<< "$clipboard_auth_body" \
  || ! rg -q 'private static final int CLIPBOARD_AUTH_SUCCEEDED = 2001;' "$clipboard_service_java" \
  || ! rg -q 'private static final int CLIPBOARD_AUTH_UNAVAILABLE = 2002;' "$clipboard_service_java" \
  || ! rg -q 'public static final int RESULT_AUTHENTICATED = 2001;' "$clipboard_auth_activity" \
  || ! rg -q 'public static final int RESULT_UNAVAILABLE = 2002;' "$clipboard_auth_activity" \
  || ! rg -q 'finishWith\(RESULT_UNAVAILABLE\);' "$clipboard_auth_activity" \
  || ! rg -q 'if \(!sent && isFinishing\(\)\) \{' <<< "$auth_activity_destroy_body" \
  || ! rg -q 'result\.send\(RESULT_CANCELED, null\);' <<< "$auth_activity_destroy_body" \
  || ! rg -U -q 's\.removeClipboardItems\(\n\s+new java\.util\.ArrayList<>\(clipboardSelectedTexts\), this::exitClipboardSelection\)\);' "$android_java/core/ImePanels.java"; then
  echo "Clipboard single, swipe and multi-select delete must go through the delete authentication gate, handle RESULT_UNAVAILABLE (2002) separately (refused deletes point at the toggle, and the settings toggle can always be turned off without an authenticator), and keep the selection until the delete actually happens" >&2
  exit 1
fi
# 引擎会话还在建时工具栏的表情、常用语、皮肤、输入方式和「中」键照常可用（#10086，.agents/notes/implemented/bug-fix/2026-10-29-android-toolbar-before-session.md）。可用性规则由 ToolbarReadinessPolicySmoke 覆盖；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：五个入口的可用性都经 ToolbarReadinessPolicy；表情面板不再按 session 拦；皮肤和输入方式先问 deferUntilSessionReady；会话建好（startEngineSession）和第一次读到偏好（reloadPreferences）时执行推迟的点按；建会话前从运行时选项填上资源目录和偏好目录；建好会话时记下耗时；常用语在自己的工作线程上读，不排在建会话的偏好读取和词库同步后面；推迟中的皮肤、输入方式再点一次即取消，打开不经 closeToolbarPanels 的功能面板（更多）时作废；开始打字或删字时推迟的面板和记下的「中」键一起作废；收起键盘时作废记下的「中」键；会话建好后再点「中」键时作废记下的那一下，读到偏好时不再补按。
service_java="$android_java/core/MSIMEInputService.java"
panels_java="$android_java/core/ImePanels.java"
emoji_body=$(sed -n '/^    void showEmojiPicker() {$/,/^    }$/p' "$panels_java")
skin_menu_body=$(sed -n '/^    void showSkinMenu(Button anchor) {$/,/^    }$/p' "$panels_java")
scheme_picker_body=$(sed -n '/^    void showSchemePicker() {$/,/^    }$/p' "$panels_java")
start_session_body=$(sed -n '/^    private void startEngineSession(String optionsText, String livePreferences) {$/,/^    }$/p' "$service_java")
reload_body=$(sed -n '/^    private void reloadPreferences(String response) {$/,/^    }$/p' "$service_java")
if ! rg -q 'setEnabled\(emojiShortcutButton, ToolbarReadinessPolicy\.emojiEnabled\(' "$service_java" \
  || ! rg -q 'setEnabled\(phraseShortcutButton, ToolbarReadinessPolicy\.phraseEnabled\(' "$service_java" \
  || ! rg -q 'setEnabled\(schemeButton, ToolbarReadinessPolicy\.preferencePanelEnabled\(' "$service_java" \
  || ! rg -q 'setEnabled\(skinButton, ToolbarReadinessPolicy\.preferencePanelEnabled\(' "$service_java" \
  || ! rg -q 'boolean canToggle = ToolbarReadinessPolicy\.languageKeyActive\(toolbarSession\(\)\);' "$service_java" \
  || ! rg -q 'ToolbarReadinessPolicy\.languageTap\(toolbarSession\(\)' "$service_java" \
  || ! rg -q 'ToolbarReadinessPolicy\.emojiEnabled\(s\.toolbarSession\(\)' <<< "$emoji_body" \
  || rg -q 's\.session == 0' <<< "$emoji_body" \
  || ! rg -q 'deferUntilSessionReady\(ToolbarOrderPolicy\.Item\.SKIN\)' <<< "$skin_menu_body" \
  || ! rg -q 'deferUntilSessionReady\(ToolbarOrderPolicy\.Item\.SCHEME\)' <<< "$scheme_picker_body" \
  || ! rg -q 'main\.post\(this::runDeferredToolbarTaps\);' <<< "$start_session_body" \
  || ! rg -q 'recordEngineSessionTiming\(createMillis, SystemClock\.uptimeMillis\(\) - startedAt\);' <<< "$start_session_body" \
  || ! rg -q 'runDeferredToolbarTaps\(\);' <<< "$reload_body" \
  || ! rg -q 'prefillSessionIndependentDirectories\(options\);' "$service_java" \
  || ! sed -n '/^    void showCommonPhrases() {$/,/^    }$/p' "$panels_java" | rg_q_stdin 's\.commonPhrasesWorker\.execute' \
  || ! rg -q 'deferrablePanelToggle\(ToolbarOrderPolicy\.Item\.SKIN,' "$service_java" \
  || ! rg -q 'deferrablePanelToggle\(ToolbarOrderPolicy\.Item\.SCHEME,' "$service_java" \
  || ! sed -n '/^    private void finishInputViewPresentation() {$/,/^    }$/p' "$service_java" | rg_q_stdin 'pendingLanguageTap = false;' \
  || ! sed -n '/^    boolean commitText(String text, TypingSource source) {$/,/^    }$/p' "$service_java" | rg_q_stdin 'dropDeferredToolbarTaps\(\);' \
  || ! sed -n '/^    void deleteCodePointBeforeCursor() {$/,/^    }$/p' "$service_java" | rg_q_stdin 'dropDeferredToolbarTaps\(\);' \
  || ! sed -n '/^    private void dropDeferredToolbarTaps() {$/,/^    }$/p' "$service_java" | rg_q_stdin 'pendingLanguageTap = false;' \
  || ! sed -n '/^    void showFeedbackMenu() {$/,/^    }$/p' "$panels_java" | rg_q_stdin 's\.dropDeferredToolbarPanel\(\);' \
  || ! sed -n '/languageButton\.setOnClickListener/,/});/p' "$service_java" | rg -A2 'case ENGINE' | rg_q_stdin 'pendingLanguageTap = false;'; then
  echo "Toolbar emoji, phrases, skin, scheme and the language key must stay usable while the engine session starts" >&2
  exit 1
fi
# 没读到实时偏好时按记住的全拼九键建会话（#10389，.agents/notes/implemented/bug-fix/2026-10-31-android-nine-key-first-frame.md）。改哪种组合由 TouchLayoutHintPolicySmoke 覆盖；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：只在 livePreferences 为 null 时取选项里的偏好副本，用 touchLayoutHint() 经 TouchLayoutHintPolicy.startupTouchKeyboardLayout 算布局，写回副本的 touch_keyboard_layout 在 NativeClient.create 之前；displayedTouchLayout 只在没有视图时用记住的布局，有视图时按视图画，不再经 TouchLayoutHintPolicy 让提示压过视图。
startup_seed_match=$(rg -n -m 1 'startupPreferences\.put\("touch_keyboard_layout", startupLayout\)' <<< "$start_session_body" || true)
startup_create_match=$(rg -n -m 1 'String created = NativeClient\.create\(options\.toString\(\)\);' <<< "$start_session_body" || true)
startup_seed_line=${startup_seed_match%%:*}
startup_create_line=${startup_create_match%%:*}
displayed_layout_body=$(sed -n '/^    int displayedTouchLayout(JSONObject value) {$/,/^    }$/p' "$service_java")
if ! rg -q 'livePreferences == null \? options\.optJSONObject\("preferences"\) : null' <<< "$start_session_body" \
  || ! rg -q 'TouchLayoutHintPolicy\.startupTouchKeyboardLayout\(touchLayoutHint\(\),' <<< "$start_session_body" \
  || [ -z "$startup_seed_line" ] || [ -z "$startup_create_line" ] \
  || [ "$startup_seed_line" -ge "$startup_create_line" ] \
  || ! rg -q 'if \(value == null\) return touchLayoutHint\(\);' <<< "$displayed_layout_body" \
  || [ "$(rg -c 'touchLayoutHint\(\)' <<< "$displayed_layout_body")" != 1 ] \
  || rg -q 'TouchLayoutHintPolicy' <<< "$displayed_layout_body"; then
  echo "Android must seed the startup preference copy with the remembered nine-key layout before NativeClient.create when live preferences are missing, and draw from the engine view once it exists (#10389)" >&2
  exit 1
fi
# 首帧布局诊断（#10389，.agents/notes/implemented/bug-fix/2026-11-01-android-first-frame-layout-diagnostics.md）。种类怎么取由 TouchLayoutDiagnosticsSmoke 覆盖，种类清单由 ImePrivacyGateSmoke 和 client-services 的诊断包测试钉住；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：会话到达前画的布局在 NativeClient.create 之前取（focus 可能已经按会话重画键区），窗口没显示时不取（键行是上一个输入框留下的）；会话建好后记偏好来源（是否读到实时偏好、副本是否按提示改过）、会话到达前画的布局和会话给出的布局；实时偏好在建会话时应用上就在建会话里记应用后的布局，否则在 reloadPreferences 里有快照时记，不看重读前有没有快照（键盘里的保存或触控板可能先换上快照），每个会话只记一次、会话结束时清掉标记；不认识的布局不交给日志。
first_drawn_match=$(rg -n -m 1 'int firstDrawnLayout = imeWindowVisible \? imeLetterRows\.drawnTouchLayout\(\) : -1;' <<< "$start_session_body" || true)
first_drawn_line=${first_drawn_match%%:*}
record_touch_layout_body=$(sed -n '/^    private void recordTouchLayout(TouchLayoutDiagnostics\.Stage stage, int layout) {$/,/^    }$/p' "$service_java")
record_live_layout_body=$(sed -n '/^    private void recordLiveTouchLayout() {$/,/^    }$/p' "$service_java")
stop_session_state_body=$(sed -n '/^    private void stopSessionState() {$/,/^    }$/p' "$service_java")
if [ -z "$first_drawn_line" ] || [ "$first_drawn_line" -ge "$startup_create_line" ] \
  || ! rg -q 'hintedPreferences = true;' <<< "$start_session_body" \
  || ! rg -q 'imeDebugOverlay\.recordEvent\(TouchLayoutDiagnostics\.sessionPreferences\(livePreferences != null, hintedPreferences\)\);' <<< "$start_session_body" \
  || ! rg -q 'recordTouchLayout\(TouchLayoutDiagnostics\.Stage\.FIRST_DRAWN, firstDrawnLayout\);' <<< "$start_session_body" \
  || ! rg -q 'recordTouchLayout\(TouchLayoutDiagnostics\.Stage\.CREATED, displayedTouchLayout\(view\)\);' <<< "$start_session_body" \
  || ! rg -q 'if \(liveApplied\) recordLiveTouchLayout\(\);' <<< "$start_session_body" \
  || ! rg -q '^        if \(preferencesSnapshot != null\) recordLiveTouchLayout\(\);$' <<< "$reload_body" \
  || ! rg -q 'if \(liveTouchLayoutRecorded\) return;' <<< "$record_live_layout_body" \
  || ! rg -q 'liveTouchLayoutRecorded = true;' <<< "$record_live_layout_body" \
  || ! rg -q 'recordTouchLayout\(TouchLayoutDiagnostics\.Stage\.LIVE, displayedTouchLayout\(view\)\);' <<< "$record_live_layout_body" \
  || ! rg -q 'liveTouchLayoutRecorded = false;' <<< "$stop_session_state_body" \
  || ! rg -q 'if \(kind != null\) imeDebugOverlay\.recordEvent\(kind\);' <<< "$record_touch_layout_body"; then
  echo "Android must record where the engine session's preferences came from and the touch layout drawn before the session, at creation and once live preferences apply (#10389)" >&2
  exit 1
fi
# 键盘弹出各阶段的耗时和「建会话等第一帧」（#9916，.agents/notes/implemented/bug-fix/2026-10-29-android-first-show-session-after-first-frame.md）。等不等、等多久由 InputViewRefreshPolicySmoke 覆盖；接线在 InputMethodService 里，JVM 冒烟碰不到，这里钉住：onStartInputView 一开头就开始等第一帧；工作线程交回的建会话经 startEngineSessionAfterFirstFrame，放行时仍核对启动代次；第一帧在画帧前登记帧提交回调（API 28 退到下一帧回调），交出去后才放行等着的建会话，而不是在画帧前直接 post（API 30 上那样会抢在画完的报告前面）；收起时马上放行等着的建会话；onCreate、onCreateInputView、建会话前的偏好读取与词库同步、主线程建会话的整段和销毁旧会话都记下耗时。
start_view_body=$(sed -n '/^    @Override public void onStartInputView(EditorInfo info, boolean restarting) {$/,/^    }$/p' "$service_java")
schedule_body=$(sed -n '/^    private void scheduleEngineStartup(String options, long generation) {$/,/^    }$/p' "$service_java")
first_frame_body=$(sed -n '/^    private void firstFrameCommitted(long armedAt) {$/,/^    }$/p' "$service_java")
frame_commit_body=$(sed -n '/^    private void awaitFirstFrameCommit(android\.view\.ViewTreeObserver observer, long armedAt) {$/,/^    }$/p' "$service_java")
window_hidden_body=$(sed -n '/^    @Override public void onWindowHidden() {$/,/^    }$/p' "$service_java")
if ! sed -n '2p' <<< "$start_view_body" | rg_q_stdin 'super\.onStartInputView\(info, restarting\);' \
  || ! sed -n '3p' <<< "$start_view_body" | rg_q_stdin '^        armFirstFrameWait\(\);$' \
  || ! rg -q 'startEngineSessionAfterFirstFrame\(\(\) -> \{' <<< "$schedule_body" \
  || ! rg -A 3 'startEngineSessionAfterFirstFrame\(\(\) -> \{' <<< "$schedule_body" | rg_q_stdin 'if \(generation != engineStartGeneration \|\| connection == null \|\| session != 0\) return;' \
  || rg -q '^                    startEngineSession\(startOptions' <<< "$schedule_body" \
  || ! rg -q 'runSessionStartAfterFirstFrame\(\);' <<< "$first_frame_body" \
  || rg -q 'main\.post\(this::runSessionStartAfterFirstFrame\)' "$service_java" \
  || ! rg -q 'observer\.registerFrameCommitCallback\(\(\) -> main\.post\(committed\)\);' <<< "$frame_commit_body" \
  || ! rg -q 'Choreographer\.getInstance\(\)\.postFrameCallback\(frameTimeNanos -> main\.post\(committed\)\);' <<< "$frame_commit_body" \
  || ! rg -q 'awaitFirstFrameCommit\(observer, armedAt\);' "$service_java" \
  || ! rg -A 1 '^        firstFrameArmedAt = -1;$' <<< "$window_hidden_body" | rg_q_stdin '^        runSessionStartAfterFirstFrame\(\);$' \
  || ! rg -q 'DiagnosticEventKind\.KEYBOARD_FIRST_DRAW' <<< "$first_frame_body" \
  || ! rg -q 'DiagnosticEventKind\.ENGINE_STARTUP_PREFERENCES, preferencesMillis' <<< "$schedule_body" \
  || ! rg -q 'DiagnosticEventKind\.ENGINE_STARTUP_DICTIONARY_SYNC, syncMillis' <<< "$schedule_body" \
  || ! sed -n '/^    @Override public void onCreate() {$/,/^    }$/p' "$service_java" | rg_q_stdin 'imeDebugOverlay\.holdServiceCreate\(' \
  || ! rg -A 3 '^        allowLearning = info != null && EditorPolicy\.allowLearning\(info\.imeOptions\);$' "$service_java" | rg_q_stdin '^        imeDebugOverlay\.flushServiceCreate\(\);$' \
  || ! sed -n '/^    @Override public View onCreateInputView() {$/,/^    }$/p' "$service_java" | rg_q_stdin 'DiagnosticEventKind\.INPUT_VIEW_CREATE' \
  || ! rg -q 'DiagnosticEventKind\.ENGINE_SESSION_MAIN, mainThreadMillis' "$service_java" \
  || ! rg -q 'DiagnosticEventKind\.ENGINE_SESSION_DESTROY,' "$service_java"; then
  echo "Android keyboard first-show timing and the after-first-frame session build must stay wired (#9916)" >&2
  exit 1
fi
# 候选条按钮的最小宽度只有触控下限（CandidateAppearance.STRIP_CHIP_MIN_WIDTH_DP，CandidateAppearanceSmoke 覆盖它的值）；皮肤的 itemMinWidth 是桌面候选窗的格宽，`base = "microsoft"` 出厂 3.5em，套到候选条上一个字宽的候选也撑成一大格。样式在 ImeCandidates 里，JVM 冒烟碰不到，这里钉住接线。
style_body=$(sed -n '/^    void styleCandidateButton(Button button) {$/,/^    }$/p' \
  "$repo_root/platforms/android/java/app/msime/android/core/ImeCandidates.java")
if ! rg -q 'CandidateAppearance\.STRIP_CHIP_MIN_WIDTH_DP' <<< "$style_body" \
  || rg -q '\.itemMinWidth' <<< "$style_body"; then
  echo "Candidate strip buttons must keep only the touch minimum width, not the skin's desktop itemMinWidth" >&2
  exit 1
fi
# 双拼触点纠错坐上首选的显式标记（#10018）：字段怎么读、读屏补什么、候选字和注释取什么颜色、不带标记的行颜色和改动前一样，都由 CandidateCorrectionMarkPolicySmoke 覆盖；接线在 InputMethodService 和 ImeCandidates 里，JVM 冒烟碰不到，这里钉住：候选条（含固定槽位）的 updateCandidateButton 和展开面板的 expandedCandidateButton 都经 CandidateCorrectionMarkPolicy 读 View JSON 候选上的字段，把结果交给 candidateLabel，并把「，已纠正」接进读屏描述；candidateLabel 是否建 span、候选字和注释的颜色都经这个策略决定，候选字的标记色只在策略给出时盖上，并且同一段加且只加一道下划线（首选 chip 上候选字本来就是强调色，只换颜色看不出来）；不带 corrected 的四参数重载传 false，翻译义项等其他调用方不会被标记。
service_source="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
update_body=$(sed -n '/^    void updateCandidateButton(Button button, JSONObject candidate, int slot, int visibleSlot) {$/,/^    }$/p' "$service_source")
expanded_body=$(sed -n '/^    Button expandedCandidateButton(JSONObject candidate) {$/,/^    }$/p' \
  "$repo_root/platforms/android/java/app/msime/android/core/ImeCandidates.java")
label_body=$(sed -n '/^                                        boolean highlighted, boolean corrected) {$/,/^    }$/p' "$service_source")
for body in "$update_body" "$expanded_body"; do
  if ! rg -q 'CandidateCorrectionMarkPolicy\.confident\(candidate\.opt\(CandidateCorrectionMarkPolicy\.FIELD\)\)' <<< "$body" \
    || ! rg -q 'candidateLabel\([^;]*, corrected\)\)' <<< "$body" \
    || ! rg -q 'CandidateCorrectionMarkPolicy\.accessibilitySuffix\(corrected\)' <<< "$body"; then
    echo "Candidate strip and expanded panel must mark a confident shuangpin correction in colour and in the spoken description" >&2
    exit 1
  fi
done
plain_label_body=$(sed -n '/^                                        boolean highlighted) {$/,/^    }$/p' "$service_source")
if ! rg -q 'CandidateCorrectionMarkPolicy\.plainLabel\(prefix, annotation, corrected\)' <<< "$label_body" \
  || ! rg -q 'Integer mark = CandidateCorrectionMarkPolicy\.wordMark\(candidateAppearance, highlighted, corrected\);' <<< "$label_body" \
  || ! rg -qU 'if \(mark != null\) \{\n\s*label\.setSpan\(new ForegroundColorSpan\(mark\), prefix\.length\(\), primary\.length\(\),\n\s*Spanned\.SPAN_EXCLUSIVE_EXCLUSIVE\);\n\s*\}' <<< "$label_body" \
  || rg -q 'UnderlineSpan' <<< "$label_body" \
  || ! rg -q 'int secondary = CandidateCorrectionMarkPolicy\.annotationColour\(candidateAppearance, highlighted, corrected\);' <<< "$label_body" \
  || ! rg -q 'new ForegroundColorSpan\(secondary\), annotationStart' <<< "$label_body" \
  || ! rg -q 'return candidateLabel\(prefix, text, annotation, highlighted, false\);' <<< "$plain_label_body"; then
  echo "candidateLabel must take its spans and colours from CandidateCorrectionMarkPolicy and mark a confident correction in colour only, like iOS and HarmonyOS" >&2
  exit 1
fi
# 每次按键的 render 末尾都会把候选重新上色一遍：外观和密度没变、按钮的底也还是上次装上的那块时，不能再给每个候选新建五块 GradientDrawable 和一个 StateListDrawable，文字配色也共用一份 ColorStateList。低端机上每键几十个候选、几百个短命 Drawable，是打字发热和卡顿的来源之一。
if ! rg -q 'if \(!keepsChipFace\(button\)\) \{' <<< "$style_body" \
  || rg -q 'ColorPolicy\.stateList' <<< "$style_body"; then
  echo "Candidate chip styling must keep the installed chip face when appearance and density are unchanged" >&2
  exit 1
fi
# 触屏候选条按位置复用按钮（ImeCandidateStrip.pool），每次 render 只重新绑定内容和点击目标；改回每次新建按钮，每键就要整排重建几十个 Button 再量一遍字宽。
strip_add_body=$(sed -n '/^    private void add(int target, boolean style) {$/,/^    }$/p' \
  "$repo_root/platforms/android/java/app/msime/android/core/ImeCandidateStrip.java")
if ! rg -q 'pool\.get\(visible\)' <<< "$strip_add_body" \
  || ! rg -q 'bindStripCandidateButton\(' <<< "$strip_add_body"; then
  echo "The touch candidate strip must reuse its pooled buttons instead of creating new ones on every render" >&2
  exit 1
fi
# 触屏候选条每键只取它建得出来的一批（NativeClient.candidateRange），滚近尾部再取下一批。改回每键取整代候选（allCandidates / currentCandidateSnapshot），每键都要在主线程上解析几百到几千行的候选 JSON，首字母还会逼引擎把暂扣的整张声母表查出来；Rust 侧的分段语义由 input-runtime 和 host-api 的 candidate_range 测试覆盖。
strip_source="$repo_root/platforms/android/java/app/msime/android/core/ImeCandidateStrip.java"
if rg -q 'currentCandidateSnapshot\(|allCandidates\(' "$strip_source" \
  || ! rg -q 's\.candidateRangeSnapshot\(0, wanted\)' "$strip_source" \
  || ! rg -q 's\.candidateRangeSnapshot\(loaded, CandidateGridBatchPolicy\.BATCH\)' "$strip_source"; then
  echo "The touch candidate strip must fetch one batch per render through candidateRange, not the whole candidate generation" >&2
  exit 1
fi
# 英文释义和音标也在 render 里每键跑：只显示候选条时它们取引擎已放出的那一段（displayedCandidateSnapshot），整代只留给展开面板。直接调 currentCandidateSnapshot 会让开了释义或音标的用户首字母仍逼引擎查出整张声母表、主线程仍解析整代 JSON。
service_source="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
if awk '/private void scheduleCandidateGlosses\(\)/ || /private void scheduleCandidatePronunciations\(\)/ { inside = 1 } inside && /currentCandidateSnapshot\(\)/ { bad = 1 } inside && /^    }$/ { inside = 0 } END { exit !bad }' "$service_source" \
  || ! awk '/private JSONObject displayedCandidateSnapshot\(\)/ { inside = 1 } inside && /candidateRangeSnapshot\(0, CandidateGlossModel\.MAX_CANDIDATES\)/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' "$service_source"; then
  echo "Android candidate glosses and pronunciations must not fetch the whole candidate generation while only the strip is showing" >&2
  exit 1
fi
# applySkin 每次 render 都走整棵键盘树，收着的面板（表情、符号网格可有几百格）不能每键跟着上一遍色：只在 applySkin 自己那几遍里跳过 GONE 子树并记下来，被直接显示出来时由画帧前的兜底补一次换肤。
if ! rg -q 'skipHidden = true;' "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java" \
  || ! rg -q 'skippedHidden\.add\(child\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java" \
  || ! rg -q 'addOnPreDrawListener\(restyleShown\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java"; then
  echo "applySkin must skip hidden subtrees and restyle them when they are shown" >&2
  exit 1
fi
# APNG 候选皮肤只在换帧那一刻重画（CandidateApngFrames.millisUntilNextFrame，CandidateCanvasDeviceSmoke 覆盖它的帧边界）。改回每次绘制都 postInvalidateOnAnimation，候选在屏上时整列按屏幕刷新率空转重画，而 APNG 一般 100 毫秒左右才换一帧。
renderer_source="$repo_root/platforms/android/java/app/msime/android/candidate/CandidateCanvasRenderer.java"
if rg -q 'postInvalidateOnAnimation' "$renderer_source" \
  || ! rg -q 'millisUntilNextFrame\(elapsed, motion\)' "$renderer_source" \
  || ! rg -q 'host\.postDelayed\(tick, nextFrame\)' "$renderer_source"; then
  echo "The candidate canvas must schedule APNG redraws at the next frame boundary, not on every vsync" >&2
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
# 候选词翻译的范围（`candidate_translation_scope`）：键盘不调 msime_client_translation_query，范围要自己守。账号和用户服务两条取译文的路径、以及交给共享层的合并载荷，都经 CandidateTranslationPolicy.requestsTranslation 决定一个候选要不要在线译文（范围生效时只有高亮那一个）；两条偏好路径都读范围；离线释义的回答（CandidateGlossModel.decode 和合并载荷）都带 gloss 标记，否则共享层在「仅当前高亮」时只把它们画在高亮行。规则本身由 CandidateTranslationPolicySmoke 覆盖。
if [ "$(rg -c 'CandidateTranslationPolicy\.requestsTranslation\(' "$account_service")" != 3 ] \
  || [ "$(rg -c '= translationScopeFrom\(preferences\);' "$account_service")" != 2 ] \
  || ! rg -qF '.put("gloss", answer.gloss())' "$account_service" \
  || ! rg -qF '.put("gloss", true)' "$repo_root/platforms/android/java/app/msime/android/candidate/CandidateGlossModel.java"; then
  echo "Android must request candidate translations only for the highlighted candidate under the highlighted scope and mark offline glosses" >&2
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
# 整段方法体都查，不按固定行数截：菜单项往往加在方法末尾挂到面板之前，截到前几十行会漏掉那里新加的弹窗。
if awk '/ (void|LinearLayout) (clearClipboardHistory|renderClipboardHistory|renderClipboardMoreMenu|renderClipboardItemActions|setClipboardItemPinned|removeClipboardItem)\(/ { inside = 1 } inside && /new (AlertDialog|PopupMenu|PopupWindow)/ { bad = 1 } inside && /^    }$/ { inside = 0 } END { exit !bad }' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java"; then
  echo "Android clipboard panel must not raise dialogs or popup menus from the input method" >&2
  exit 1
fi
# 单条删除（长按操作行的「删除」、左滑的垃圾桶）和多选删除一样能撤销（#10080）：走 remove_many 拿到删除前的完整条目，交给 holdClipboardUndo 倒数。改回单条的 remove 时拿不到快照，撤销按钮不会出现，撤销也无从恢复时间戳和固定状态。删除门禁（#10408）之后真正删除的是 removeClipboardItemAfterAuthentication，入口 removeClipboardItem 只经 authenticateClipboardDelete 调它，接线由上方门禁检查钉住。
if ! rg -A 6 'private void removeClipboardItemAfterAuthentication\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg_q_stdin 'holdClipboardUndo\(clipboardHistory\.removeMany\('; then
  echo "Android single clipboard deletion must keep an undo snapshot via removeMany (#10080)" >&2
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
# 「添加到常用语」经常用语存储写入（校验、去重、上限和同步标记都在那里），它要等文件锁，必须放在工作线程上，不能在主线程卡住键盘（#5909）。和常用语面板的读取同在 commonPhrasesWorker 上，不排在建会话的偏好读取和词库同步后面（#10086）。
if ! rg -A 12 'void addClipboardTextToPhrases\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'commonPhrasesWorker\.execute' \
  || ! rg -A 12 'void addClipboardTextToPhrases\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'CommonPhrasesStore\.add\(s, text\)'; then
  echo "Android clipboard add-to-phrases must write through CommonPhrasesStore on the common phrases worker (#5909, #10086)" >&2
  exit 1
fi
# 左滑删除只在 ClipboardSwipePolicy 判定为横滑后才接手：接手时不让外层面板拦截去滚动，并给卡片补一个取消，撤掉按下态、长按和点按，松手不会插入（#5962）。未被外层列表接手的纵向拖动也要取消卡片按压；列表只有一条时不可滚，Button 否则会把整段拖动当成点按。
swipe_body=$(rg -A 80 'private final class ClipboardSwipeCell implements' \
  "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" || true)
if ! rg -q 'ClipboardSwipePolicy\.claims\(' <<< "$swipe_body" \
  || ! rg -q 'requestDisallowInterceptTouchEvent\(true\)' <<< "$swipe_body" \
  || ! rg -q 'cancelledPress' <<< "$swipe_body" \
  || ! rg -q 'ACTION_CANCEL\);' <<< "$swipe_body"; then
  echo "Android clipboard swipe must claim only through ClipboardSwipePolicy, keep the panel from scrolling and cancel the card's own press after any drag (#5962)" >&2
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
# 搜索框固定在剪贴板历史页顶部；结果列表滚动时不得把查询框一起卷走（#9958）。
clipboard_search_page="$repo_root/platforms/android/java/app/msime/android/home/ClipboardSearchPage.java"
if ! rg -q 'protected View fixedNavigation\(\)' "$clipboard_search_page" \
  || ! rg -q 'searchNavigation = new SearchPill' "$clipboard_search_page" \
  || ! rg -q 'searchNavigation' "$clipboard_search_page" \
  || rg -q 'target\.addView\(search' "$clipboard_search_page"; then
  echo "Android clipboard search must keep its search field fixed above the scrolling results (#9958)" >&2
  exit 1
fi
# 进剪贴板历史页不自动聚焦搜索框、不主动弹键盘，等用户点搜索框（#10074）；点胶囊任何地方都要和点输入框一样弹出键盘，否则点放大镜只有光标没有键盘。从设置首页搜索结果压页时，下面那一页搜索框拉起的键盘要先收起，否则它留在新页面上、不连着任何输入框；收起之后从历史页点「编辑」进的编辑页要自己弹键盘，否则只有光标。
search_pill="$repo_root/platforms/android/java/app/msime/android/home/SearchPill.java"
settings_navigator="$repo_root/platforms/android/java/app/msime/android/home/SettingsNavigator.java"
if rg -q 'requestFocus\(\)|Type\.ime\(\)|showSoftInput' "$clipboard_search_page" \
  || ! rg -A 3 'private void focusField\(' "$search_pill" | rg_q_stdin 'showSoftInput\(field' \
  || ! rg -q 'bindClick\(this, this::focusField\)' "$search_pill" \
  || ! rg -B 1 'transaction\.hide\(below\)' "$settings_navigator" | rg_q_stdin 'dismissKeyboard\(home, below\)' \
  || ! rg -A 5 'private static void dismissKeyboard\(' "$settings_navigator" | rg_q_stdin 'hideSoftInputFromWindow' \
  || ! rg -A 3 'input\.requestFocus\(\);' "$repo_root/platforms/android/java/app/msime/android/home/ClipboardEditPage.java" \
    | rg_q_stdin 'getInsetsController\(requireActivity\(\)\.getWindow\(\), input\)\.show\(WindowInsetsCompat\.Type\.ime\(\)\)'; then
  echo "Android clipboard search must not focus its search field or show the keyboard on entry, tapping anywhere on SearchPill must focus the field and show the keyboard, SettingsNavigator must dismiss the keyboard of the page it hides, and ClipboardEditPage must show the keyboard itself once its field is ready (#10074)" >&2
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
# 多选剪贴板最多保留 20 项，并同时提供选中项导出和删除；导出走同一份认证与文件目录流程（#9957）。
if ! rg -q 'ClipboardSelectionPolicy\.canSelect' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'clipboardAction\(header, "导出"' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'exportClipboardItems' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'exportSelectedText' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardHistoryStore.java"; then
  echo "Android clipboard multi-selection must cap at 20 and export the selected records (#9957)" >&2
  exit 1
fi
# 多选「插入」按 ①②③④ 的顺序一次插入 1 到 4 项（#10411）。上限、序号和拼接由 ClipboardSelectionPolicySmoke 覆盖；接线在面板和 InputMethodService 里，JVM 冒烟碰不到，这里钉住：顶行有「插入」并按 canInsert 置灰，选中项的读屏前缀从 ordinal 算出，角标画在承载单列和网格两种排法的左滑格里、并加进卡片自己的浮层（不做兄弟视图，带阴影的皮肤抬高卡片后会把它盖住），每次重画按读出的历史收窄选择（多选中左滑删掉的记录不能再被插入），服务按 canInsert 守住条数、经 insertionText 拼成一次 commitText。
if ! rg -q 'clipboardAction\(header, "插入"' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'ClipboardSelectionPolicy\.canInsert\(clipboardSelectedTexts\.size\(\)\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -q 'ClipboardSelectionPolicy\.selectedDescription\(ordinal\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -A 40 'private FrameLayout clipboardSwipeCell\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'ClipboardSelectionPolicy\.ordinalLabel\(ordinal\)' \
  || ! rg -A 40 'private FrameLayout clipboardSwipeCell\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'card\.getOverlay\(\)\.add\(badge\)' \
  || ! rg -q 'clipboardSelectedTexts\.retainAll\(listed\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -A 6 'void insertClipboardTexts\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
    | rg_q_stdin 'ClipboardSelectionPolicy\.insertionText\(texts\)'; then
  echo "Android clipboard multi-selection must insert 1 to 4 records in ordinal order with badges (#10411)" >&2
  exit 1
fi
# 长按全文预览的「详情」排在「插入」左边，详情页返回时回到这个预览（#9963）。返回到哪一级由 ClipboardDetailPolicySmoke 覆盖；接线在 PopupWindow 和面板里，JVM 冒烟碰不到，这里钉住：预览先加「详情」再加「插入」，本机记录的预览把详情交给 Origin.PREVIEW，返回经 reopensPreview 决定是否重开预览，顶栏和底部操作行的两个「返回」都按来源给读屏说明。
if ! rg -U -q 'addAction\(actions, "详情",[^\n]*\n\s*if \(insert != null\) addAction\(actions, "插入"' \
    "$repo_root/platforms/android/java/app/msime/android/clipboard/ClipboardTextPreview.java" \
  || ! rg -q 'openClipboardDetail\(item, ClipboardDetailPolicy\.Origin\.PREVIEW\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
  || ! rg -A 10 'private void returnFromClipboardDetail\(' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java" \
    | rg_q_stdin 'ClipboardDetailPolicy\.reopensPreview\(' \
  || [ "$(rg -c 'ClipboardDetailPolicy\.backDescription\(clipboardDetailOrigin\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java")" != 2 ]; then
  echo "Android clipboard full-text preview must offer 详情 left of 插入 and return to the preview (#9963)" >&2
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
# 复制后快捷键（#10079）的显隐只由 RecentClipboardSuggestion.quickAction 判断（RecentClipboardSuggestionSmoke 覆盖：总开关关着、这一条是图片时不显示），点按经 ImePanels.openRecentClipAction 进入长按菜单里同一套分词点选和「移动到分类」，真正进去了才把这一条记为用过。接线在 InputMethodService 和面板里，JVM 冒烟碰不到，这里钉住。
panels_java="$repo_root/platforms/android/java/app/msime/android/core/ImePanels.java"
if ! rg -q 'RecentClipboardSuggestion\.quickAction\(clipboardSuggestionActionEnabled,' "$service_java" \
  || ! rg -q 's\.recentClipQuickAction\(image\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeToolbar.java" \
  || ! rg -A 12 'void runRecentClipQuickAction\(' "$service_java" | rg_q_stdin 'if \(imePanels\.openRecentClipAction\(' \
  || ! rg -A 12 'boolean openRecentClipAction\(' "$panels_java" | rg_q_stdin 'startClipboardSegmentation\(text\)' \
  || ! rg -A 18 'boolean openRecentClipAction\(' "$panels_java" | rg_q_stdin 'clipboardCategoryMovingText = text;'; then
  echo "Android recent-clip quick action must follow RecentClipboardSuggestion.quickAction and open the clipboard segmentation or category picker (#10079)" >&2
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
# 长按「中/英」弹出系统输入法选择框（#5615）。这个键在没有会话的输入框里也必须保持可用：禁用的按钮收不到长按，而密码框正是最需要换到密码管理器键盘的地方。没有会话、也没有会话在建时把键画淡，点按在反馈和计数之前就忽略（会话在建时照常生效，见 #10086 的 ToolbarReadinessPolicy.languageTap）。
if ! rg -q 'bindInputMethodPicker\(languageButton\)' "$account_service" \
  || ! rg -q 'manager\.showInputMethodPicker\(\)' "$account_service" \
  || rg -q 'setEnabled\(languageButton, session != 0\)' "$account_service" \
  || ! rg -q 'setActiveAlpha\(languageButton, canToggle' "$account_service" \
  || ! sed -n '/languageButton\.setOnClickListener/,/});/p' "$account_service" | rg_q_stdin 'if \(tap == ToolbarReadinessPolicy\.LanguageTap\.IGNORE\) return;' \
  || ! rg -q 's\.bindInputMethodPicker\(language\)' \
    "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"; then
  echo "Android 中/英 keys must open the system input method picker on long press" >&2
  exit 1
fi
# 删除键上滑快速删除（#5585、#9893）：判定和组字边界只在 BackspaceSwipePolicy，键的触摸监听按它决定松手清哪一层；清正文经服务的 deleteAllBeforeCursor 分段删除，连删间隔由 BackspaceRepeatPolicy 逐级加速。
letter_rows="$repo_root/platforms/android/java/app/msime/android/core/ImeLetterRows.java"
if ! rg -q 'BackspaceSwipePolicy\.clearTarget\(backspaceSwipePhase' "$letter_rows" \
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
# 触屏按键布局样式与顶部数字行（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）：排布和迁移规则由 TouchKeyStyleSmoke 覆盖，接线在 InputMethodService、云同步和设置页里，JVM 冒烟碰不到，这里钉住：本机旧的长空格和数字行两个键只有迁移读（AndroidLocalSettings 声明它们、KeyLayoutStyleMigration 读后删掉）；云同步导出前和键盘读实时偏好前先迁移；键盘从共享偏好读样式与数字行，变了经 touchGeometryKey 触发重画，底行签名带着样式；14 键与 18 键的字母块上方画顶部数字行。
style_migration="$repo_root/platforms/android/java/app/msime/android/settings/KeyLayoutStyleMigration.java"
legacy_readers=$(rg -l 'AndroidLocalSettings\.(NUMBER_ROW|LONG_SPACE_LAYOUT)\b' "$repo_root/platforms/android/java" | sed "s#^$repo_root/##" | LC_ALL=C sort | tr '\n' ' ')
if [[ "$legacy_readers" != "platforms/android/java/app/msime/android/settings/KeyLayoutStyleMigration.java " ]] \
  || ! sed -n '/public static Round run(/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/account/RustCloudSync.java" \
    | rg_q_stdin -U 'boolean settingsMigrated = KeyLayoutStyleMigration\.migrate\(context, preferencesDirectory\);\n\s*JSONObject inputs' \
  || ! sed -n '/public static Round run(/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/account/RustCloudSync.java" \
    | rg_q_stdin 'if \(!settingsMigrated\) inputs\.put\("settings_locked", true\);' \
  || ! sed -n '/private EngineStartOptions withLivePreferences(/,/^    }$/p' "$account_service" \
    | rg_q_stdin -U 'KeyLayoutStyleMigration\.migrate\(this, directory\);\n\s*String response = candidateSkinResolver\.read\(directory\);' \
  || ! rg -q 'removal\.put\(AndroidLocalSettings\.LONG_SPACE_LAYOUT, null\);' "$style_migration" \
  || ! rg -q 'removal\.put\(AndroidLocalSettings\.NUMBER_ROW, null\);' "$style_migration" \
  || ! rg -q -U '\+ ":" \+ touchKeyStyles \+ ":" \+ numberRowEnabled \+ ":" \+ twentySixSymbolFirst \+ ":" \+ numberPadSpace\.resolved\(\) \+ ":" \+ languageKeyLeft\n\s*\+ ":" \+ NineKeySidebarPolicy\.format\(nineKeySidebarSymbols\) \+ ":" \+ NineKeySidebarPolicy\.format\(nineKeyDigitSidebarSymbols\);' "$account_service" \
  || ! rg -q 'preferences\.optBoolean\(TouchKeyStyle\.NUMBER_ROW_PREFERENCE, false\)' "$account_service" \
  || ! rg -q '\+ ":" \+ english \+ ":" \+ style' "$repo_root/platforms/android/java/app/msime/android/core/ImeBottomRow.java" \
  || ! sed -n '/private void rebuildFourteenKeyRows() {$/,/^    }$/p' "$letter_rows" | rg_q_stdin 'if \(builtNumberRow\) addNumberRow\(rows\.size\(\)\);'; then
  echo "Android touch key layout styles must read the shared preferences, migrate the two legacy local keys before any settings export or live read, re-render when they change, and draw the number row on 14/18 keys" >&2
  exit 1
fi
# 九键滑动输入数字（#10229）：方向在共享偏好 touch_nine_key_swipe_digits 里、随账号同步，判定规则、宽松读法和迁移的纯逻辑由 NineKeySwipePolicySmoke 和 TouchKeyStyleSmoke 覆盖；接线在 InputMethodService、键盘页和迁移里，JVM 冒烟碰不到，这里钉住：本机旧键 platform.android.nine_key_swipe 只有迁移读（AndroidLocalSettings 声明它、KeyLayoutStyleMigration 把它交给 client-core 后删掉，距离不删）；键盘的两条偏好路径都从共享偏好读，滑动判定读服务上的当前值；键盘页这一行读写共享偏好，不再写本机键；反方向弹出的选项跟着这次按压拖选（#10443），移动、抬手和取消都交给字母选项的拖选入口。
nine_key_legacy_readers=$(rg -l 'AndroidLocalSettings\.NINE_KEY_SWIPE\b' "$repo_root/platforms/android/java" | sed "s#^$repo_root/##" | LC_ALL=C sort | tr '\n' ' ')
keyboard_options="$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java"
if [[ "$nine_key_legacy_readers" != "platforms/android/java/app/msime/android/settings/KeyLayoutStyleMigration.java " ]] \
  || ! rg -q 'request\.put\("nine_key_swipe", local\.choice\(AndroidLocalSettings\.NINE_KEY_SWIPE\)\);' "$style_migration" \
  || ! rg -q 'removal\.put\(AndroidLocalSettings\.NINE_KEY_SWIPE, null\);' "$style_migration" \
  || rg -q 'removal\.put\(AndroidLocalSettings\.NINE_KEY_SWIPE_DISTANCE' "$style_migration" \
  || ! rg -q 'local\.has\(AndroidLocalSettings\.NINE_KEY_SWIPE\)' "$style_migration" \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'nineKeySwipeDigits = nineKeySwipeDigitsFrom\(preferences\);' \
  || ! sed -n '/private void applyPreferencesSnapshot(JSONObject snapshot) throws JSONException {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'String nextNineKeySwipeDigits = nineKeySwipeDigitsFrom\(preferences\);' \
  || ! rg -q 'nineKeySwipeDigits = nextNineKeySwipeDigits;' "$account_service" \
  || ! sed -n '/private static String nineKeySwipeDigitsFrom(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'NineKeySwipePolicy\.mode\(' \
  || ! sed -n '/private static final String\[\] SKIN_HINT_KEYS = {/,/};$/p' "$account_service" | rg_q_stdin 'NineKeySwipePolicy\.PREFERENCE' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" | rg_q_stdin 's\.nineKeySwipeDigits, downY\[0\],' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" \
    | rg_q_stdin 's\.localSettings\.integer\(AndroidLocalSettings\.NINE_KEY_SWIPE_DISTANCE\)' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" | rg_q_stdin 'moveLetterOptions\(keyButton, event\);' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" | rg_q_stdin 'finishLetterOptions\(keyButton, event\)' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" | rg_q_stdin 'cancelLetterOptions\(keyButton\);' \
  || ! sed -n '/private void bindNineKeySwipe(/,/^    }$/p' "$layout_rows" \
    | rg_q_stdin 'key\.label\(\)\.toLowerCase\(java\.util\.Locale\.ROOT\), true\);' \
  || ! rg -q 'JsonPolicy\.strictString\(preferences\.opt\(NineKeySwipePolicy\.PREFERENCE\)\)' "$keyboard_options" \
  || ! sed -n '/private void pickNineKeySwipe(/,/^    }$/p' "$keyboard_options" | rg_q_stdin 'savePreference\(NineKeySwipePolicy\.PREFERENCE, value\);' \
  || ! rg -q '"沿所选方向滑动输入数字，反方向弹出字母。"' "$keyboard_options" \
  || ! sed -n '/private static String nineKeySwipeLabel(/,/^    }$/p' "$keyboard_options" \
    | rg_q_stdin '"上滑" : SwipeHintPolicy\.DOWN\.equals\(value\) \? "下滑" : "关闭"'; then
  echo "Android nine-key swipe digits must read and write the shared touch_nine_key_swipe_digits, read the current value on every touch, and leave the legacy local key to KeyLayoutStyleMigration only, and let the swipe-opened options be drag-selected (#10443)" >&2
  exit 1
fi
# 长空格样式留着的工具栏行看用户的 26 键样式和这块键盘的中文键面，不看此刻的中/英（TouchKeyStyle.Styles.keepsToolbarRow，规则由 TouchKeyStyleSmoke 覆盖）；这里钉住宿主的接线：工具栏行和隐藏工具栏时藏起按钮都读 longSpaceToolbarRowKept，英文模式下中文键面取只在中文时更新的 touchLayoutHint。之前读的是按此刻语言算的 longSpaceLayoutActive，工具栏隐藏时每次切中/英这一行都出没一次，键盘高度跟着跳。自动数字行与共享偏好分开传给 drawsNumberRow，自动数字行不扩到 14/18 键。
if ! sed -n '/boolean toolbarRowHidden() {$/,/^    }$/p' "$account_service" | rg_q_stdin 'if \(longSpaceToolbarRowKept\(\)\) return false;' \
  || ! sed -n '/boolean longSpaceToolbarRowKept() {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'int chineseLayout = dedicatedEnglish \? touchLayoutHint\(\) : displayedTouchLayout\(view\);' \
  || ! sed -n '/boolean longSpaceToolbarRowKept() {$/,/^    }$/p' "$account_service" | rg_q_stdin 'touchKeyStyles\.keepsToolbarRow\(chineseLayout,' \
  || ! rg -q 's\.toolbarHidden && s\.longSpaceToolbarRowKept\(\)' "$repo_root/platforms/android/java/app/msime/android/core/ImeToolbar.java" \
  || ! sed -n '/boolean numberRowDrawn() {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'drawsNumberRow\(numberRowEnabled, automaticNumberRow\.active\(\),' \
  || ! sed -n '/boolean symbolExtraRowDrawn() {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'drawsSymbolExtraRow\(numberRowEnabled, automaticNumberRow\.active\(\),'; then
  echo "Android must keep the long-space toolbar row by the user's 26-key style and the keyboard's Chinese face, not the momentary 中/英 mode, and pass the automatic number row separately from touch_number_row" >&2
  exit 1
fi
# 底行样式和顶部数字行在 applyTouchGeometry 里读，冷启动的第一帧按皮肤片段算；片段里没有它们时每次键盘进程冷启动都先画标准底行、没有数字行，一两秒后实时偏好到了键盘整体高出一行、底行换样。这里钉住 applyTouchGeometry 读到的每个 TouchKeyStyle 偏好键都在 SKIN_HINT_KEYS 里。
skin_hint_keys=$(sed -n '/private static final String\[\] SKIN_HINT_KEYS = {/,/};$/p' "$account_service")
touch_style_reads=$(sed -n '/private static TouchKeyStyle\.Styles touchKeyStylesFrom(JSONObject preferences) {$/,/^    }$/p;/private static boolean numberRowFrom(JSONObject preferences) {$/,/^    }$/p;/private static boolean symbolFirstFrom(JSONObject preferences) {$/,/^    }$/p;/private static TouchKeyStyle\.NumberPadSpace numberPadSpaceFrom(JSONObject preferences) {$/,/^    }$/p;/private static boolean languageKeyLeftFrom(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
  | rg -o 'TouchKeyStyle\.[A-Z_]+_PREFERENCE' | LC_ALL=C sort -u)
if [[ -z "$skin_hint_keys" ]] || [[ "$(printf '%s\n' "$touch_style_reads" | wc -l | tr -d ' ')" -ne 7 ]] \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'touchKeyStyles = touchKeyStylesFrom\(preferences\);' \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'twentySixSymbolFirst = symbolFirstFrom\(preferences\);' \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'numberPadSpace = numberPadSpaceFrom\(preferences\);' \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin 'languageKeyLeft = languageKeyLeftFrom\(preferences\);'; then
  echo "Android check-host could not locate SKIN_HINT_KEYS or the touch key style readers in MSIMEInputService" >&2
  exit 1
fi
while IFS= read -r key; do
  if ! rg -qF "$key" <<< "$skin_hint_keys"; then
    echo "Android SKIN_HINT_KEYS must remember $key so a cold-started keyboard draws the user's bottom row and number row from the first frame" >&2
    exit 1
  fi
done <<< "$touch_style_reads"
# 「按键布局」页（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）：顶部是本版本提供的键盘类型的分段控件，下面是整块键盘预览、样式胶囊和当前样式的说明（说明下面的开关卡片里 26 键分段先是「符」与「123」互换开关，每个分段都有「中/英键放在空格左边」，接着是「数字键盘」一组，这几项在下面 #10056、#10228 两段钉住），最后是没有标题的一组里的「顶部数字行」开关。哪几类、默认停在哪一类、胶囊名字、高亮哪些键、说明文字和预览画哪些键都在 KeyLayoutStylesPolicy 里，由 KeyLayoutStylesPolicySmoke 覆盖；这里钉住页面的接线：按 TouchKeyStyle.Groups 列类型，只有一种时不画分段，默认值按键盘页读的同一批偏好（storedScheme、双拼和五笔的排法）定，切分段只重画不写偏好，点胶囊先就地换预览再写这一类的偏好、点已选中的不做事，说明没有时不画，数字行开关在两类键面都没有时不列，胶囊等宽铺满一行、不横向滚动，读成按钮并带「已选择」「未选择」，预览对读屏隐藏。
style_page="$repo_root/platforms/android/java/app/msime/android/home/KeyLayoutStylesPage.java"
style_policy="$repo_root/platforms/android/java/app/msime/android/home/KeyLayoutStylesPolicy.java"
style_preview="$repo_root/platforms/android/java/app/msime/android/home/KeyLayoutPreview.java"
style_render=$(sed -n '/    private void render(@Nullable JSONObject preferences) {$/,/^    }$/p' "$style_page")
style_section=$(sed -n '/    private void styleSection(/,/^    }$/p' "$style_page")
style_refresh=$(sed -n '/    private void refreshStyles() {$/,/^    }$/p' "$style_page")
style_chip=$(sed -n '/    private static TextView chip(/,/^    }$/p' "$style_page")
style_add_chip=$(sed -n '/    private static void addChip(/,/^    }$/p' "$style_page")
if ! rg_q_stdin -U 'if \(!groups\.numberRow\(\)\) return;\n\s*GroupCard numbers = GroupCard\.add\(target, null\);' <<< "$style_render" \
  || ! rg_q_stdin -F 'TouchKeyStyle.Groups groups = TouchKeyStyle.Groups.offeredBy(AppEdition.current());' <<< "$style_render" \
  || ! rg_q_stdin -F 'List<KeyLayoutStylesPolicy.Kind> kinds = KeyLayoutStylesPolicy.offered(groups);' <<< "$style_render" \
  || ! rg_q_stdin -F 'KeyboardScheme current = SchemePreferences.storedScheme(preferences, edition);' <<< "$style_render" \
  || ! rg_q_stdin -F 'kind = KeyLayoutStylesPolicy.initialKind(kinds, current, shuangpinLayout, wubiLayout);' <<< "$style_render" \
  || ! rg_q_stdin -F 'eighteen = KeyLayoutStylesPolicy.eighteen(current, shuangpinLayout);' <<< "$style_render" \
  || ! rg_q_stdin -F 'numbers.toggle("顶部数字行", null, numberRow, checked -> {' <<< "$style_render" \
  || ! rg_q_stdin -F '}).help(NUMBER_ROW_NOTE);' <<< "$style_render" \
  || ! rg_q_stdin -F 'save(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked));' <<< "$style_render" \
  || ! rg_q_stdin -F 'if (kinds.size() > 1) {' <<< "$style_section" \
  || ! rg_q_stdin -U 'segments\.setOnSelect\(index -> \{\n\s*kind = kinds\.get\(index\);\n\s*refreshStyles\(\);\n\s*\}\);' <<< "$style_section" \
  || rg_q_stdin -F 'HorizontalScrollView' <<< "$style_section" \
  || ! rg_q_stdin -F 'LinearLayout.LayoutParams params = LayoutPolicy.weightedWrapParams(1f);' <<< "$style_add_chip" \
  || ! rg_q_stdin -F 'addChip(context, row, chip(context, KeyLayoutStylesPolicy.label(shown, option), checked,' <<< "$style_refresh" \
  || ! rg_q_stdin -F '() -> choose(shown, option)));' <<< "$style_refresh" \
  || ! rg_q_stdin -F 'ViewPolicy.setAutoSizeSp(chip, 9, Math.round(Ui.TEXT_BUTTON_SMALL), 1);' <<< "$style_chip" \
  || rg_q_stdin 'save\(|savePreferences\(' <<< "$style_refresh" \
  || ! rg_q_stdin -F 'board.show(KeyLayoutStylesPolicy.keyboard(shown, selected, numberRow, eighteen, symbolFirst, languageLeft, nineKeySidebar),' <<< "$style_refresh" \
  || ! rg_q_stdin -F 'KeyLayoutStylesPolicy.highlights(shown, selected, symbolFirst, languageLeft, nineKeySidebar));' <<< "$style_refresh" \
  || ! rg_q_stdin -F 'ViewPolicy.setTextAndVisibility(caption, KeyLayoutStylesPolicy.note(shown, selected));' <<< "$style_refresh" \
  || ! rg_q_stdin -F '() -> { if (!checked) choose.run(); });' <<< "$style_chip" \
  || ! rg_q_stdin -F 'info.setClassName(Button.class.getName());' <<< "$style_chip" \
  || ! rg_q_stdin -F 'info.setStateDescription(checked ? "已选择" : "未选择");' <<< "$style_chip" \
  || ! sed -n '/    private void choose(/,/^    }$/p' "$style_page" \
    | rg_q_stdin -U 'styles = shown\.with\(styles, option\);\n\s*refreshStyles\(\);\n\s*save\(edited -> edited\.put\(shown\.preference, option\.wire\)\);' \
  || ! rg -qF 'KeyboardSheets.save(this, edit, null, this::reload);' "$style_page" \
  || ! rg -qF 'setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);' "$style_preview" \
  || rg -q 'BottomRowPreview|styleGroup\(|optionRow\(|group\.footer\(' "$style_page" \
  || [[ -e "$repo_root/platforms/android/java/app/msime/android/home/BottomRowPreview.java" ]]; then
  echo "Android 按键布局 must list key layout kinds by TouchKeyStyle.Groups in a segmented control defaulting to the current Chinese keyboard, redraw the whole-keyboard preview without saving when the segment changes, lay the style chips out at equal width without horizontal scrolling, save a chip choice after redrawing, show the selected style's note only when it has one, and keep 顶部数字行 in an untitled group hidden when the edition offers neither 26-key nor 14/18-key faces" >&2
  exit 1
fi
# 「按键布局」当前样式的说明、「顶部数字行」的说明和键盘页入口的副标题与 iOS、鸿蒙逐字相同；原来三条组末说明由四条按样式的说明取代（26 键长空格、26 键与 14/18 键表情键、9 键右侧大回车），其他样式不显示说明。键盘页只在本版本至少有一组样式时列入口（与鸿蒙 keyLayoutOffered 同一条规则），设置搜索同样不列这一页。
if ! rg -qF 'static final String LONG_SPACE_NOTE = "符号键移到工具栏。";' "$style_policy" \
  || ! rg -qF 'static final String EMOJI_NOTE = "长按 123 打开符号。";' "$style_policy" \
  || ! rg -qF 'static final String TALL_RETURN_NOTE = "回车竖占两行，符号键在左下角。";' "$style_policy" \
  || rg -q 'TWENTY_SIX_KEY_NOTE|FOURTEEN_KEY_NOTE|NINE_KEY_NOTE' "$style_page" "$style_policy" \
  || ! rg -qF 'static final String NUMBER_ROW_NOTE = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。";' "$style_page" \
  || ! rg -q -U 'if \(TouchKeyStyle\.Groups\.offeredBy\(edition\)\.any\(\)\)\n\s*layout\.nav\("按键布局", null, null,\n[^\n]*\)\.help\("各类键盘的底行样式、键位和顶部数字行"\);' \
    "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java" \
  || ! rg -qF 'if (page == PageId.KEY_LAYOUT_STYLES && !keyLayoutOffered) continue;' \
    "$repo_root/platforms/android/java/app/msime/android/home/KeyboardFragment.java" \
  || ! rg -qF 'boolean keyLayoutOffered = TouchKeyStyle.Groups.offeredBy(AppEdition.current()).any();' \
    "$repo_root/platforms/android/java/app/msime/android/home/KeyboardFragment.java"; then
  echo "Android 按键布局 notes and entry subtitle must match iOS and HarmonyOS word for word, and the entry and its search result must only appear when the edition offers a style group" >&2
  exit 1
fi
# 「26 键「符」与「123」互换」和数字键盘空格位置（#10056）：排布由 TouchKeyStyleSmoke 覆盖，预览里 符 排到 123 左边并高亮这两键由 KeyLayoutStylesPolicySmoke 覆盖，这里钉住接线：底行签名带着两项、两种底行都按它们排；数字层右列按空格位置放空格或 ！，换了位置时 render 经 numberPadSpaceStale 重建；设置页的互换开关接在样式说明后面、只在 26 键分段显示，改它先重画预览再写 touch_twenty_six_symbol_first；「数字键盘」是样式之后、顶部数字行之前的独立一组，选项用与样式同一个 chip 画的等宽胶囊，点了先换胶囊再写 touch_number_pad_space，说明在胶囊下方。
bottom_row_file="$repo_root/platforms/android/java/app/msime/android/core/ImeBottomRow.java"
number_pad_section=$(sed -n '/    private void numberPadSection(/,/^    }$/p' "$style_page")
number_pad_refresh=$(sed -n '/    private void refreshNumberPad() {$/,/^    }$/p' "$style_page")
if ! rg -qF '+ ":" + english + ":" + style + ":" + symbolFirst + ":" + numberPadSpace.resolved()' "$bottom_row_file" \
  || ! rg -qF 'KeyboardActionRow.numberPadEntries(layout, globe, swapLanguageNumber, style, symbolFirst, numberPadSpace, languageLeft)' "$bottom_row_file" \
  || ! rg -qF 'KeyboardActionRow.designEntries(layout, globe, swapLanguageNumber, false, english, style, symbolFirst, numberPadSpace, languageLeft)' "$bottom_row_file" \
  || ! sed -n '/    void rebuildNineKeyRows() {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java" \
    | rg_q_stdin -F 'if (spaceInColumn) adoptBarKey(actions, s.spaceButton, KeyboardKeyRole.KEY);' \
  || ! rg -qF '|| imeLayoutRows.numberPadSpaceStale()' "$account_service" \
  || ! rg_q_stdin -F 'symbolFirst = preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false);' <<< "$style_render" \
  || ! rg_q_stdin -U 'note = caption;\n\n\s*//.*\n\s*GroupCard toggles = GroupCard\.add\(target, null\);\n\s*if \(kinds\.contains\(KeyLayoutStylesPolicy\.Kind\.TWENTY_SIX\)\) \{\n\s*symbolFirstRow = toggles\.toggle\(' <<< "$style_section" \
  || ! rg_q_stdin -U 'toggles\.toggle\(TouchKeyStyle\.SYMBOL_FIRST_TITLE, null, symbolFirst, checked -> \{\n\s*symbolFirst = checked;\n\s*refreshStyles\(\);\n\s*save\(edited -> edited\.put\(TouchKeyStyle\.SYMBOL_FIRST_PREFERENCE, checked\)\);\n\s*\}\)\.help\(TouchKeyStyle\.SYMBOL_FIRST_NOTE\);' <<< "$style_section" \
  || ! rg_q_stdin -F 'if (swap != null) swap.setVisible(shown == KeyLayoutStylesPolicy.Kind.TWENTY_SIX);' <<< "$style_refresh" \
  || ! rg_q_stdin -U 'if \(kind != null\) styleSection\(context, target, kinds\);\n\s*//.*\n\s*if \(groups\.any\(\)\) numberPadSection\(context, target\);\n\s*//.*\n\s*if \(!groups\.numberRow\(\)\) return;' <<< "$style_render" \
  || ! rg_q_stdin -U 'GroupCard numberPad = GroupCard\.add\(target, TouchKeyStyle\.NUMBER_PAD_SPACE_GROUP\);\n.*\n.*\n\s*numberPad\.addView\(row\);\n\s*numberPadChips = row;\n\s*numberPad\.help\(TouchKeyStyle\.NUMBER_PAD_SPACE_NOTE\);' <<< "$number_pad_section" \
  || rg_q_stdin -F 'HorizontalScrollView' <<< "$number_pad_section" \
  || ! rg_q_stdin -F 'for (TouchKeyStyle.NumberPadSpace choice : TouchKeyStyle.NumberPadSpace.CHOICES)' <<< "$number_pad_refresh" \
  || ! rg_q_stdin -F 'addChip(context, row, chip(context, choice.title, choice == numberPadSpace, () -> chooseNumberPadSpace(choice)));' <<< "$number_pad_refresh" \
  || ! sed -n '/    private void chooseNumberPadSpace(/,/^    }$/p' "$style_page" \
    | rg_q_stdin -U 'numberPadSpace = choice;\n\s*refreshNumberPad\(\);\n\s*save\(edited -> edited\.put\(TouchKeyStyle\.NUMBER_PAD_SPACE_PREFERENCE, choice\.wire\)\);' \
  || ! rg -qF 'preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false), languageLeft),' "$repo_root/platforms/android/java/app/msime/android/home/StatisticsFragment.java"; then
  echo "Android must lay both bottom rows by touch_twenty_six_symbol_first and touch_number_pad_space, rebuild the digit right column when the space moves, put the swap toggle after the 26-key style note and the 数字键盘 chips in their own group in 按键布局, and draw the heatmap with the swap" >&2
  exit 1
fi
# 九键左侧符号栏的两张表（#10223，.agents/notes/implemented/feature/2026-10-29-nine-key-sidebar-shared.md）：空表回到出厂表、右侧大回车只给出厂表接 ！、编辑器整表校验和「等于出厂表写空数组」由 NineKeySidebarPolicySmoke 覆盖，迁移与备份恢复的转换由 TouchKeyStyleSmoke 覆盖，9 键预览左列与高亮、摘要由 KeyLayoutStylesPolicySmoke 覆盖；接线在 InputMethodService、设置页和迁移里，JVM 冒烟碰不到，这里钉住：本机两个旧键只有迁移读（请求带上两张表、迁完与另两个旧键一起删）；键盘从共享偏好读两张表（冷启动的皮肤片段也记着），变了经 touchGeometryKey 重画、sidebarStale 重建；长按符号栏打开「按键布局」并带上那张表的偏好名；键盘页不再列这两行；「按键布局」在 9 键分段的说明后、「数字键盘」一组说明后各列一行，只在 9 键分段显示字母表那一行，预览按字母表画左列并高亮；编辑页按修订号只写这一张表的共享偏好，不再写本机设置。
sidebar_readers=$(rg -l 'AndroidLocalSettings\.(NINE_KEY_SYMBOLS|NINE_KEY_DIGIT_SYMBOLS)\b' "$repo_root/platforms/android/java" | sed "s#^$repo_root/##" | LC_ALL=C sort | tr '\n' ' ')
sidebar_sheet="$repo_root/platforms/android/java/app/msime/android/home/NineKeySidebarSheet.java"
sidebar_layout_rows="$repo_root/platforms/android/java/app/msime/android/core/ImeLayoutRows.java"
sidebar_section=$(sed -n '/    private void styleSection(/,/^    }$/p' "$style_page")
sidebar_number_pad=$(sed -n '/    private void numberPadSection(/,/^    }$/p' "$style_page")
sidebar_refresh=$(sed -n '/    private void refreshStyles() {$/,/^    }$/p' "$style_page")
if [[ "$sidebar_readers" != "platforms/android/java/app/msime/android/settings/KeyLayoutStyleMigration.java " ]] \
  || ! rg -q 'removal\.put\(AndroidLocalSettings\.NINE_KEY_SYMBOLS, null\);' "$style_migration" \
  || ! rg -q 'removal\.put\(AndroidLocalSettings\.NINE_KEY_DIGIT_SYMBOLS, null\);' "$style_migration" \
  || ! rg -qF '.put("nine_key_symbols", new JSONArray(nineKeySymbols))' "$style_migration" \
  || ! rg -qF '.put("nine_key_digit_symbols", new JSONArray(nineKeyDigitSymbols));' "$style_migration" \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin -F 'nineKeySidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.LETTER_PREFERENCE);' \
  || ! sed -n '/private void applyTouchGeometry(JSONObject preferences) {$/,/^    }$/p' "$account_service" \
    | rg_q_stdin -F 'nineKeyDigitSidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.DIGIT_PREFERENCE);' \
  || ! rg -qF 'nineKeySidebarSymbols = nextSidebarSymbols;' "$account_service" \
  || ! rg -qF 'nineKeyDigitSidebarSymbols = nextDigitSidebarSymbols;' "$account_service" \
  || ! rg_q_stdin -F 'NineKeySidebarPolicy.LETTER_PREFERENCE, NineKeySidebarPolicy.DIGIT_PREFERENCE,' <<< "$skin_hint_keys" \
  || ! rg -qF '|| imeLetterRows.splitStale() || imeLayoutRows.sidebarStale()' "$account_service" \
  || ! sed -n '/    java.util.List<String> sidebarSymbols(boolean digits, boolean tallReturn) {$/,/^    }$/p' "$sidebar_layout_rows" \
    | rg_q_stdin -F 'NineKeySidebarPolicy.sidebarSymbols(digits, s.nineKeySidebarSymbols, s.nineKeyDigitSidebarSymbols,' \
  || ! rg -qF 's.openHostPage("KEY_LAYOUT_STYLES", args);' "$sidebar_layout_rows" \
  || ! rg -qF 'args.putString(NineKeySidebarPolicy.EDIT_ARGUMENT, digits' "$sidebar_layout_rows" \
  || rg -q 'NineKeySidebar|edit_sidebar_key|"九键左侧符号"|"九键数字键盘左侧符号"' "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java" \
  || ! rg_q_stdin -F 'sidebar.nav(KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_TITLE, null,' <<< "$sidebar_section" \
  || ! rg_q_stdin -F 'sidebar.nav(KeyLayoutStylesPolicy.DIGIT_SIDEBAR_TITLE, null,' <<< "$sidebar_number_pad" \
  || ! rg_q_stdin -U 'numberPad\.help\(TouchKeyStyle\.NUMBER_PAD_SPACE_NOTE\);\n\s*refreshNumberPad\(\);\n\s*//.*\n\s*GroupCard sidebar = GroupCard\.add\(target, null\);' <<< "$sidebar_number_pad" \
  || ! rg_q_stdin -F 'if (sidebar != null) ViewPolicy.setVisible(sidebar, shown == KeyLayoutStylesPolicy.Kind.NINE);' <<< "$sidebar_refresh" \
  || ! rg -qF 'KeyboardSheets.save(owner, edited -> edited.put(key, new JSONArray(stored)),' "$sidebar_sheet" \
  || ! rg -qF 'List<String> stored = NineKeySidebarPolicy.toPreference(symbols, defaults);' "$sidebar_sheet" \
  || rg -q 'saveLocal' "$sidebar_sheet"; then
  echo "Android nine-key sidebar tables must come from touch_nine_key_sidebar_symbols / touch_nine_key_digit_sidebar_symbols: only KeyLayoutStyleMigration reads and deletes the legacy local keys, the keyboard re-renders and the long press opens 按键布局, and 按键布局 lists both rows and saves only that shared table" >&2
  exit 1
fi
# 「文本编辑按键顺序」「符号分类顺序」按住把手拖动排序（#10392，.agents/notes/implemented/feature/2026-10-31-android-order-sheets-drag.md）：挪动、位次、落点、贴边滚动步长和序号由 ReorderPolicySmoke 覆盖；拖动和读屏操作在 ReorderList 里，JVM 冒烟碰不到，这里钉住：两张面板都经 ReorderList 画行、不再有上下箭头；拖动模型与「九键左侧符号」编辑页相同，按下把手就拿起、只改 translationY，落点按布局中线算，松手经 ReorderPolicy.moved 改草稿；手指贴近面板上下边缘时滚动外层 NestedScrollView，滚过的距离算进跟手位移；读屏模型也相同：名称念「第 N 项，名称」，把手状态念「第 N 项」，「上移」「下移」挂在把手上，键盘焦点在把手上时 Alt+↑ / Alt+↓ 挪一格；把手的触控区与原来的箭头一样宽 48 dp、高占满整行。
reorder_list="$repo_root/platforms/android/java/app/msime/android/home/ReorderList.java"
for reorder_sheet in TextEditOrderSheet SymbolCategoryOrderSheet; do
  reorder_sheet_file="$repo_root/platforms/android/java/app/msime/android/home/$reorder_sheet.java"
  if ! rg -qF 'rows = new ReorderList<>(' "$reorder_sheet_file" \
    || rg -q 'addArrow|"↑"|"↓"|上下箭头' "$reorder_sheet_file"; then
    echo "Android $reorder_sheet must reorder through ReorderList's drag handle, not up/down arrows" >&2
    exit 1
  fi
done
if rg -q 'ItemTouchHelper|RecyclerView' "$reorder_list" \
  || ! rg -qF 'handle.setOnTouchListener((touched, event) -> handleDrag(views, event));' "$reorder_list" \
  || ! rg -qF 'int target = ReorderPolicy.dragTarget(centers[dragFrom] + offset, dragFrom, centers);' "$reorder_list" \
  || ! rg -qF 'items = ReorderPolicy.moved(items, from, to);' "$reorder_list" \
  || ! rg -qF 'float scrolled = rowsTopAtDown - screenTop(rows);' "$reorder_list" \
  || ! rg -qF 'scroller.scrollBy(0, step);' "$reorder_list" \
  || ! rg -qF 'step = ReorderPolicy.scrollTowardRoom(step,' "$reorder_list" \
  || ! rg -qF 'ViewCompat.addAccessibilityAction(handle, "上移",' "$reorder_list" \
  || ! rg -qF 'ViewCompat.addAccessibilityAction(handle, "下移",' "$reorder_list" \
  || ! rg -qF 'views.title().setContentDescription(ReorderPolicy.ordinalDescription(position, views.label()));' "$reorder_list" \
  || ! rg -qF 'ViewCompat.setStateDescription(views.handle(), ReorderPolicy.positionDescription(position));' "$reorder_list" \
  || ! rg -qF 'handle.setOnKeyListener((focused, keyCode, event) -> onKey(views, keyCode, event));' "$reorder_list" \
  || ! rg -qF 'row.addView(handle, new LinearLayout.LayoutParams(DimensionPolicy.pixels(context, 48),' "$reorder_list"; then
  echo "Android order sheets must drag from the handle the way the nine-key sidebar editor does (translationY, layout midlines, edge-scrolling the sheet), keep 上移 / 下移 and the 第 N 项 position on the handle for screen readers and Alt+Up / Alt+Down for keyboards, and keep a 48 dp handle" >&2
  exit 1
fi
# 「中/英键放在空格左边」（#10228，`touch_language_key_side`）：排法由 TouchKeyStyleSmoke 覆盖，预览与高亮由 KeyLayoutStylesPolicySmoke 覆盖，这里钉住接线：底行签名带着它、字母层底行和数字九宫格都按它排；右侧大回车九宫格下方那一行按它排，变了经 tallReturnStale 整块重建；设置页三个分段都显示这个开关（不在 26 键的 if 里，排在互换开关之后），改它先重画预览再写 left / right；热力图跟着画；设置搜索登记了标题。
if ! rg -qF 'boolean languageLeft = s.languageKeyLeft;' "$bottom_row_file" \
  || ! rg -qF '+ ":" + numberPadSpace.resolved() + ":" + languageLeft' "$bottom_row_file" \
  || ! sed -n '/    private LinearLayout tallReturnMiddleColumn(/,/^    }$/p' "$layout_rows" \
    | rg_q_stdin -F 's.localSettings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS), digits, languageLeft)) {' \
  || ! sed -n '/    boolean tallReturnStale() {$/,/^    }$/p' "$layout_rows" | rg_q_stdin -F 'builtTallReturnLanguageLeft != s.languageKeyLeft' \
  || ! rg_q_stdin -F 'languageLeft = TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE));' <<< "$style_render" \
  || ! rg_q_stdin -U '\n        \}\n\s*//.*\n\s*toggles\.toggle\(TouchKeyStyle\.LANGUAGE_LEFT_TITLE, null, languageLeft, checked -> \{\n\s*languageLeft = checked;\n\s*refreshStyles\(\);\n\s*save\(edited -> edited\.put\(TouchKeyStyle\.LANGUAGE_KEY_SIDE_PREFERENCE, TouchKeyStyle\.languageKeySideWire\(checked\)\)\);\n\s*\}\)\.help\(TouchKeyStyle\.LANGUAGE_LEFT_NOTE\);' <<< "$style_section" \
  || ! rg -qF 'boolean languageLeft = TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE));' "$repo_root/platforms/android/java/app/msime/android/home/StatisticsFragment.java" \
  || ! rg -qF 'KeyHeatmapLayout.nine(styles.nine(), languageLeft)' "$repo_root/platforms/android/java/app/msime/android/home/StatisticsFragment.java" \
  || ! rg -qF '"中/英键放在空格左边"' "$repo_root/platforms/android/java/app/msime/android/home/PageId.java" \
  || ! rg -qF 'static final String LANGUAGE_LEFT_TITLE = "中/英键放在空格左边";' "$repo_root/platforms/android/java/app/msime/android/keyboard/TouchKeyStyle.java"; then
  echo "Android must lay the bottom rows, the borrowed number pads and the tall-return strip by touch_language_key_side, rebuild the tall-return frame when it changes, list 中/英键放在空格左边 in every 按键布局 segment, draw the heatmap with it and index it in settings search" >&2
  exit 1
fi
# 九键「右侧大回车」（tall_return）：排布由 TouchKeyStyleSmoke、KeyHeatmapLayoutSmoke 和 NineKeySidebarPolicySmoke 覆盖，接线在 InputMethodService 和设置页里，JVM 冒烟碰不到，这里钉住：拼音九键（含数字层）和笔画按 tallReturnDrawn 建一整块外框、高度按三行加底行；换了样式或进出密码框时 render 经 tallReturnStale 重建；符号栏按大回车取符号表；设置页把它叫「右侧大回车」，预览画热力图的同一套整块外框。
for builder in 'void rebuildNineKeyRows() {' 'void rebuildStrokeRows() {'; do
  body=$(sed -n "/    $builder\$/,/^    }\$/p" "$layout_rows")
  if ! rg_q_stdin 'boolean tallReturn = tallReturnDrawn\(\);' <<< "$body" \
    || ! rg_q_stdin 'if \(tallReturn\) s\.imeStyler\.adjustBottomRowBlockHeight\(container\);' <<< "$body" \
    || ! rg_q_stdin 'addTallReturnKey\(actions\);' <<< "$body" \
    || ! rg_q_stdin 'hideActionRowForTallReturn\(\);' <<< "$body"; then
    echo "Android $builder must build the tall-return frame as one block with the double-height return" >&2
    exit 1
  fi
done
if ! rg -q 'imeLetterRows\.numberPadStale\(\) \|\| imeLayoutRows\.tallReturnStale\(\)\)' "$account_service" \
  || ! rg -q 'FrameLayout sidebar = symbolSidebar\(digits, true, tallReturn, true\);' "$layout_rows" \
  || ! rg -q 'case TALL_RETURN -> "右侧大回车";' "$style_policy" \
  || ! rg -qF 'case NINE -> nineActionColumn(converted(KeyHeatmapLayout.nine(offered(kind, style), languageLeft)));' "$style_policy"; then
  echo "Android tall-return wiring is missing: render must rebuild on tallReturnStale, the sidebar must read the tall table, and the settings page must label 右侧大回车 and preview the whole nine-key frame" >&2
  exit 1
fi
# 皮肤页的整键盘预览（KeyboardPreview：AI 皮肤页、皮肤列表、社区皮肤卡片和详情）按用户的偏好画（KeyLayoutStylesPolicy.Preview，由 KeyboardPreviewLayoutSmoke 覆盖），不能再写死一套键面：以前它的 NINE_KEY_ROWS / FULL_ROWS 把底行写成 `123 | ， | 空格 | 。 | 中 | ↵`，不认用户选的样式。这里钉住预览从 Preview 取行、偏好由 KeyboardPreview.layout 读。
keyboard_preview="$repo_root/platforms/android/java/app/msime/android/home/KeyboardPreview.java"
if ! rg -qF 'this.rows = layout.rows();' "$keyboard_preview" \
  || ! rg -qF 'return KeyLayoutStylesPolicy.preview(TouchKeyStyle.Groups.offeredBy(edition),' "$keyboard_preview" \
  || rg -q 'NINE_KEY_ROWS|FULL_ROWS|String\[\]\[\]' "$keyboard_preview"; then
  echo "Android skin previews must draw the user's own rows from KeyLayoutStylesPolicy.Preview, not a fixed table" >&2
  exit 1
fi
# 右侧大回车外框的列宽按画着的键面取（KeyboardActionRow.tallReturnSideWeight / tallReturnGridWeight / tallReturnGridShare，TouchKeyStyleSmoke 钉住份额）：笔画的数字层经 rebuildNineKeyRows 借九键数字九宫格，键面仍是笔画，大回车分支必须按 displayedTouchLayout 取份额，不能写死九键的 3 / 14，否则切 123 时 符、123 和回车横移。
nine_body=$(sed -n '/    void rebuildNineKeyRows() {$/,/^    }$/p' "$layout_rows")
if ! rg_q_stdin 'int frameLayout = s\.displayedTouchLayout\(s\.view\);' <<< "$nine_body" \
  || ! rg_q_stdin 'float tallSide = KeyboardActionRow\.tallReturnSideWeight\(frameLayout\);' <<< "$nine_body" \
  || ! rg_q_stdin 'container\.addView\(tallReturnLeftColumn\(sidebar, more\), LayoutPolicy\.weightedMatchParentParams\(tallSide\)\);' <<< "$nine_body" \
  || ! rg_q_stdin 'tallReturnMiddleColumn\(grid, KeyboardActionRow\.tallReturnGridShare\(frameLayout\), digits\)' <<< "$nine_body" \
  || ! rg_q_stdin 'LayoutPolicy\.weightedMatchParentParams\(KeyboardActionRow\.tallReturnGridWeight\(frameLayout\)\)' <<< "$nine_body" \
  || [[ $(rg -c 'container\.addView\(actions, LayoutPolicy\.weightedMatchParentParams\(tallSide\)\);' <<< "$nine_body") != 1 ]]; then
  echo "Android rebuildNineKeyRows must size the tall-return frame by the displayed face, so stroke's number layer keeps the stroke frame" >&2
  exit 1
fi
# 右侧大回车接到符号栏末尾的 ！ 接替拼音九键右列 ！ 的手势（右上角印 0、沿「九键滑动输入数字」的方向滑出 0、长按上下标 0），否则字母层没有直接输入 0 的地方；哪一格由 NineKeySidebarPolicy.appendedExclamationIndex 定（NineKeySidebarPolicySmoke 钉住），绑定在 symbolSidebar 里。笔画右列的 ！ 本来没有这个手势，笔画不带。
sidebar_body=$(sed -n '/    private FrameLayout symbolSidebar(boolean digits, boolean capped, int visibleRows, boolean tallReturn,$/,/^    }$/p' "$layout_rows")
if ! rg_q_stdin 'NineKeySidebarPolicy\.appendedExclamationIndex\(digits,' <<< "$sidebar_body" \
  || ! rg_q_stdin 'exclamation\.setDigitText\(SuperscriptShortcutPolicy\.ZERO\);' <<< "$sidebar_body" \
  || ! rg_q_stdin 's\.imeLetterRows\.bindNineKeySuperscriptShortcut\(exclamation, symbol\);' <<< "$sidebar_body" \
  || ! rg -q 'FrameLayout sidebar = tallReturn \? symbolSidebar\(false, true, true, false\)' "$layout_rows"; then
  echo "Android tall-return sidebar ！ must carry the right-column ！ zero gestures on nine-key and not on stroke" >&2
  exit 1
fi
# 拼音九键字母层的 ！（右列，及右侧大回车接到符号栏末尾的那个）滑出 0 的方向跟「九键滑动输入数字」走，选下滑时下滑出 0，与 1–9 键一致（#10441）；123 / #+= 符号页的 ！ 仍是固定上滑。方向和阈值每次按下时从服务上读，不在建键时定死。
superscript_nine_body=$(sed -n '/    private void bindSuperscriptShortcut(Button key, String text, boolean nineKey) {$/,/^    }$/p' "$letter_rows")
if ! rg -q 's\.imeLetterRows\.bindNineKeySuperscriptShortcut\(exclamation, last\);' "$layout_rows" \
  || [[ $(rg -c 'bindNineKeySuperscriptShortcut\(' "$layout_rows") != 2 ]] \
  || ! rg -q 'bindSuperscriptShortcut\(key, text\);' "$letter_rows" \
  || ! rg_q_stdin 'SuperscriptShortcutPolicy\.zeroThresholdDp\(s\.nineKeySwipeDigits,' <<< "$superscript_nine_body" \
  || ! rg_q_stdin 'SuperscriptShortcutPolicy\.swiped\(direction\[0\], threshold\[0\], downY\[0\],' <<< "$superscript_nine_body" \
  || ! rg -q 'return SuperscriptShortcutPolicy\.zeroDirection\(s\.nineKeySwipeDigits\);' "$letter_rows"; then
  echo "Android nine-key ！ must swipe out 0 in the 九键滑动输入数字 direction read at touch time, while the 123 / #+= ！ keeps the fixed up-swipe" >&2
  exit 1
fi
# 拼音九键格子上的长按数字画在右上角，字号和上边距与 26 键字母的角标相同（KeyHintButton 的 CORNER_HINT_SP、CORNER_HINT_TOP_DP，基线按 ascent 算），右边单独向内收至 12 dp，给圆角和胶囊键帽的顶部曲线让出空间；iOS 的 keyNumberHint 仍是上 3、右 5。绘制在 onDraw 里，JVM 冒烟碰不到，这里钉住它使用专用常量，而不是让数字贴到圆角外。
nine_key_digit_draw=$(sed -n '/protected void onDraw(Canvas canvas) {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/keyboard/NineKeyDigitButton.java")
for pattern in 'KeyHintButton\.CORNER_HINT_SP' 'KeyHintButton\.CORNER_HINT_TOP_DP\) - metrics\.ascent' \
    'getWidth\(\) - DimensionPolicy\.floatPixels\(getContext\(\), DIGIT_HINT_RIGHT_DP\)'; do
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
# 「朗读」靠 queryIntentServices(TTS_SERVICE) 判断装没装语音引擎；Android 11 起清单不声明这个查询就永远看不见引擎，菜单和背单词卡片上都不会出现这一项。
if ! rg -q '<action android:name="android\.intent\.action\.TTS_SERVICE" />' "$repo_root/platforms/android/AndroidManifest.xml"; then
  echo "Android manifest must query android.intent.action.TTS_SERVICE for read aloud" >&2
  exit 1
fi
# 「朗读」的接线在 ImeCandidates、MSIMEInputService 和 VocabularyReviewPage 里，JVM 冒烟碰不到，这里钉住：长按菜单加上这一项、点下去交给服务朗读，键盘服务销毁时和背单词页离开时都断开语音引擎。
read_aloud_candidates="$repo_root/platforms/android/java/app/msime/android/core/ImeCandidates.java"
read_aloud_service="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
read_aloud_review="$repo_root/platforms/android/java/app/msime/android/home/VocabularyReviewPage.java"
if ! rg -qF 'popup.getMenu().add(Menu.NONE, CandidateManagementAction.READ_ALOUD_MENU_ITEM,' "$read_aloud_candidates" \
  || ! rg -qF 'item.getItemId() == CandidateManagementAction.READ_ALOUD_MENU_ITEM' "$read_aloud_candidates" \
  || ! rg -qF 's.readAloud(readAloud);' "$read_aloud_candidates" \
  || ! awk '/public void onDestroy\(\)/ { inside = 1 } inside && /readAloudSpeaker\.shutdown\(\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' "$read_aloud_service" \
  || ! rg -qF 'if (speaker != null) speaker.shutdown();' "$read_aloud_review"; then
  echo "Android read aloud must add and route the candidate menu item and shut the speaker down with the service and the review page" >&2
  exit 1
fi
# 键区里的系统识别服务是 SpeechRecognizer 回调接线，JVM 冒烟只能覆盖 PlatformSpeechPolicy 和 ImeVoiceEntry.choose 这些纯逻辑，这里守住回调里不能被悄悄改回去的几处（#5553）：两条入口都按错误码提示、空结果不冒用错误码，权限拒绝或聆听前启动失败时转交识别窗口，系统识别服务和豆包流式不被 1.5 s 停顿截断（豆包整句流式说到 15 秒才回第一段结果，#9886），说完后收回音量光圈。
voice_entry="$repo_root/platforms/android/java/app/msime/android/core/ImeVoiceEntry.java"
if ! rg -qF 'fail(PlatformSpeechPolicy.message(error))' "$voice_activity" \
  || ! rg -qF 'fail(PlatformSpeechPolicy.emptyResult())' "$voice_activity" \
  || ! rg -qF 'PlatformSpeechPolicy.message(error)' "$voice_entry" \
  || ! rg -qF 'PlatformSpeechPolicy.emptyResult()' "$voice_entry" \
  || ! rg -qF 's.launchVoiceActivity();' "$voice_entry" \
  || ! rg -qF 'heard(session, firstResult(partialResults), false);' "$voice_entry" \
  || ! rg -qF 'heard(session, update, false)' "$voice_entry" \
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
    "$repo_root/platforms/android/native/JniCallbacks.cpp" \
    "$repo_root/platforms/common/CrashRecorder.cpp"
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
# ML Kit 带进来的 Google datatransport 会把 FIREBASE_ML_SDK 诊断事件上传到 firebaselogging.googleapis.com（2026-10-25 真机抓包）。清单删掉它的后端发现服务、上传任务和闹钟入口，三者缺一都留着一条上传途径。
for component in \
    'service android:name="com\.google\.android\.datatransport\.runtime\.backends\.TransportBackendDiscovery" tools:node="remove"' \
    'service android:name="com\.google\.android\.datatransport\.runtime\.scheduling\.jobscheduling\.JobInfoSchedulerService" tools:node="remove"' \
    'receiver android:name="com\.google\.android\.datatransport\.runtime\.scheduling\.jobscheduling\.AlarmManagerSchedulerBroadcastReceiver" tools:node="remove"'; do
  if ! rg -q "<$component />" "$repo_root/platforms/android/AndroidManifest.xml"; then
    echo "Android host manifest must remove the ML Kit datatransport upload path: $component" >&2
    exit 1
  fi
done
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
# 离开输入框后的快照处理每次都要扫一遍整份用户词库算版本，必须像空闲同步一样延后排队、并在下一个要建会话的输入框开始时取消；改回失焦即排进工作线程，换输入框时新会话又要排在这次扫描后面。被取消的那一轮要记在 DictionarySnapshotBacklog 里：欠满一分钟就不再延后，服务销毁时还欠着的要补上，否则一直在几个输入框之间切换的用户永远等不到快照激活和输入记录合并。
if ! awk '/public void onFinishInput\(\)/ { inside = 1 } inside && /scheduleDictionarySnapshotProcessing\(false\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! awk '/public void onStartInput\(EditorInfo info, boolean restarting\)/ { inside = 1 } inside && /cancelDictionarySnapshotProcessing\(\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! rg -q 'if \(generation != dictionarySnapshotGeneration\) return;' \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! awk '/private void scheduleDictionarySnapshotProcessing\(boolean immediate\)/ { inside = 1 } inside && /dictionarySnapshotBacklog\.owe\(SystemClock\.uptimeMillis\(\)\)/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java" \
  || ! awk '/public void onDestroy\(\)/ { inside = 1 } inside && /if \(dictionarySnapshotBacklog\.owed\(\)\) scheduleDictionarySnapshotProcessing\(true\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' \
      "$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"; then
  echo "Android must defer dictionary snapshot processing after a blur and cancel it when the next field starts a session" >&2
  exit 1
fi
# 上屏统计在 native 里按目录攒着，最多每半分钟写一次，最后一段要靠宿主在失焦、收起键盘和销毁时叫一次 flush；销毁时的那次必须排在统计工作线程 shutdown 之前，否则排不进去，进程一退这段统计就丢了。
if ! awk '/public void onFinishInput\(\)/ { inside = 1 } inside && /flushTypingStatistics\(\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' "$service_java" \
  || ! awk '/public void onWindowHidden\(\)/ { inside = 1 } inside && /flushTypingStatistics\(\);/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' "$service_java" \
  || ! awk '/public void onDestroy\(\)/ { inside = 1 } inside && /flushTypingStatistics\(\);/ { flush = NR } inside && /typingStatisticsWorker\.shutdown\(\);/ { shutdown = NR } inside && /^    }$/ { exit } END { exit !(flush && shutdown && flush < shutdown) }' "$service_java" \
  || ! rg -q '"operation", "flush"' "$service_java"; then
  echo "Android must flush pending typing statistics when the field loses focus, the keyboard hides, and before the statistics worker shuts down" >&2
  exit 1
fi
# 同一输入框重启连接时留着会话，跳过了 startInputRuntime 里按资源包重算方案列表和音标可用的那一步；资源包装好或删掉之后必须走重建，否则切换器和音标要等换了输入框才跟上。
if ! awk '/private boolean keepsSessionAcrossRestart\(/ { inside = 1 } inside && /ResourcePacks\.installedIds\(this\)\.equals\(installedResourcePackIds\)/ { found = 1 } inside && /^    }$/ { exit } END { exit !found }' "$service_java" \
  || ! rg -q 'installedResourcePackIds = installedPacks;' "$service_java"; then
  echo "Android must rebuild the engine session on a same-field restart when the installed resource packs changed" >&2
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
# 「词库 › 学习」的「清除学习记录」（.agents/notes/implemented/feature/2026-10-29-mobile-clear-learning.md）：文案和失败提示由 ClearLearningSmoke 覆盖，Rust 那一半由 host-api 的 cloud_sync 测试覆盖；页面的接线 JVM 冒烟碰不到，这里钉住：学习组里有这一行，点了先弹确认框，确认后在工作线程上走 DictionaryCollectionsStore.resetLearning；resetLearning 只在 reset 成功后调云同步的 dictionary_reset（取消等激活的快照、给词库分类打待上传标记）。
clear_store="$repo_root/platforms/android/java/app/msime/android/dictionary/DictionaryCollectionsStore.java"
clear_reset=$(sed -n '/    public static Result<ResetReport> resetLearning(Context context) {$/,/^    }$/p' "$clear_store")
if ! rg -qF 'learning.button(ClearLearningCopy.TITLE, ClearLearningCopy.SUBTITLE, ClearLearningCopy.ACTION, this::confirmClearLearning);' "$lexicon_page" \
  || ! rg -qF 'MessageDialog.confirm(requireContext(), ClearLearningCopy.CONFIRM_TITLE, ClearLearningCopy.CONFIRM_MESSAGE,' "$lexicon_page" \
  || ! rg -qF 'HostTask.run(this, DictionaryCollectionsStore::resetLearning, result -> {' "$lexicon_page" \
  || ! rg_q_stdin -F '.put("action", action("reset"))' <<< "$clear_reset" \
  || ! awk '/strictTrue\(value\.opt\("reset"\)\)/ { checked = NR } /SyncSignals\.dictionaryReset\(context\)/ { marked = NR } END { exit !(checked && marked > checked) }' <<< "$clear_reset"; then
  echo "Android clear-learning wiring is missing: the learning group row, its confirmation, the worker-thread reset through DictionaryCollectionsStore.resetLearning, and the cloud sync dictionary_reset only after the reset succeeded" >&2
  exit 1
fi
# 大屏单手侧边数字键盘（#10203，.agents/notes/implemented/feature/2026-10-29-tablet-one-hand-side-number-pad.md）：宽度由 KeyboardFormFactorPolicySmoke、显示条件和偏好读法由 OneHandSidePadPolicySmoke 覆盖；接线在 InputMethodService、ImeFrame 和设置页里，JVM 冒烟碰不到，这里钉住：键盘从共享偏好读侧边面板，进 touchGeometryKey（变了就重画）和皮肤片段（冷启动第一帧就按它画）；ImeFrame.applyOneHanded 按宽度和 OneHandSidePadPolicy.shown 摆上或摘下面板，签名带着面板宽度；面板的数字和小数点走数字层的字面上屏，⌫ 带连删；键距、行高跟着主键区重排；设置页的标题和说明与 iOS、鸿蒙逐字相同。
side_pad_service="$repo_root/platforms/android/java/app/msime/android/core/MSIMEInputService.java"
side_pad_frame=$(sed -n '/    void applyOneHanded() {$/,/^    }$/p' "$repo_root/platforms/android/java/app/msime/android/core/ImeFrame.java")
side_pad_builder="$repo_root/platforms/android/java/app/msime/android/core/ImeSidePad.java"
if ! rg -qF 'oneHandedSidePanel = oneHandedSidePanelFrom(preferences);' "$side_pad_service" \
  || ! rg -qF 'oneHandedSidePanel = nextOneHandedSidePanel;' "$side_pad_service" \
  || ! rg -qF '            + ":" + oneHandedSidePanel.wire' "$side_pad_service" \
  || ! rg -qF '        OneHandSidePadPolicy.PREFERENCE_KEY,' "$side_pad_service" \
  || ! rg_q_stdin -F 'KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(' <<< "$side_pad_frame" \
  || ! rg_q_stdin -F 'OneHandSidePadPolicy.shown(s.oneHandedSidePanel, mode, padDp,' <<< "$side_pad_frame" \
  || ! rg_q_stdin -F 'String applied = mode + ":" + blockDp + ":" + (padShown ? padDp : 0);' <<< "$side_pad_frame" \
  || ! rg -qF 's.imeLayoutRows.commitNineKeyLiteral(digit)' "$side_pad_builder" \
  || ! rg -qF 's.imeLayoutRows.nineKeyDecimalKey()' "$side_pad_builder" \
  || ! rg -qF 's.imeLetterRows.bindBackspaceRepeat(delete, deleteAction);' "$side_pad_builder" \
  || ! rg -qF 'if (sidePad != null) applySidePadGeometry(sidePad);' "$repo_root/platforms/android/java/app/msime/android/core/ImeStyler.java" \
  || ! rg -qF 'layout.toggle("大屏单手时侧边显示数字键盘", null,' \
    "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java" \
  || ! rg -qF '.help("单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。");' \
    "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java" \
  || ! rg -qF 'savePreference(OneHandSidePadPolicy.PREFERENCE_KEY,' "$repo_root/platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java"; then
  echo "Android one-handed side number pad wiring is missing: read touch_one_handed_side_panel into the geometry key and skin hint, place the pad in ImeFrame.applyOneHanded, reuse the digit-layer literal commit and backspace repeat, follow the key geometry, and keep the shared settings wording" >&2
  exit 1
fi
# 「防止截屏」（#10098）：FLAG_SECURE 只作用于设了它的那一个窗口，切换规则由 ScreenCaptureProtectionPolicySmoke 覆盖，窗口接线 JVM 冒烟碰不到，这里钉住：每个 Activity 在 onCreate 里经 ScreenCaptureProtection.apply(this) 加上；对话框和底部面板一律经 ScreenCaptureProtection.show / protect 创建；输入法自己开的 PopupWindow 在显示后经 ScreenCaptureProtection.apply 处理，单开的子窗口提示经 ScreenCaptureProtectionPolicy.windowFlags 带上标志；输入法窗口在 applyLocalSettings 里跟随设置。新加的窗口漏掉这一步，截屏里就会露出内容。
while IFS= read -r activity; do
  if ! rg -q 'ScreenCaptureProtection\.apply\(this\);' "$activity"; then
    echo "Android activity must apply screen capture protection in onCreate: ${activity#"$repo_root/"}" >&2
    exit 1
  fi
done < <(rg -l 'extends [A-Za-z.]*Activity\b' "$android_java")
# 按出现次数数，不按行数：同一行里一处经过保护、一处没有时也要拦下。
# 框架对话框类型列全：AppCompatDialog（InputDialog 的凭据、口令输入）漏数过一次。InputDialog 这类自家包装不是窗口，在它内部那一处 new 上数。
dialog_types='MaterialAlertDialogBuilder|AlertDialog\.Builder|(AppCompat|BottomSheet|Alert)?Dialog'
dialog_total=$({ rg -o --no-filename "\\bnew ($dialog_types)\\(" "$android_java" || true; } | awk 'END { print NR }')
dialog_protected=$({ rg -o --no-filename "ScreenCaptureProtection\\.(show|protect)\\(new ($dialog_types)\\(" "$android_java" || true; } | awk 'END { print NR }')
if [[ "$dialog_total" != "$dialog_protected" ]]; then
  echo "Android dialogs and bottom sheets must be created through ScreenCaptureProtection.show/protect ($dialog_protected of $dialog_total)" >&2
  exit 1
fi
while IFS= read -r popup_source; do
  if ! rg -q 'ScreenCaptureProtection\.apply\((popup|window)\);' "$popup_source"; then
    echo "Android PopupWindow must apply screen capture protection after it is shown: ${popup_source#"$repo_root/"}" >&2
    exit 1
  fi
done < <(rg -l 'new PopupWindow\(' "$android_java")
while IFS= read -r child_window; do
  if ! rg -q 'ScreenCaptureProtectionPolicy\.windowFlags\(' "$child_window"; then
    echo "Android IME child windows must carry the screen capture protection flag: ${child_window#"$repo_root/"}" >&2
    exit 1
  fi
done < <(rg -l 'TYPE_APPLICATION_ATTACHED_DIALOG' "$android_java")
apply_local_body=$(sed -n '/^    private void applyLocalSettings() {$/,/^    }$/p' "$android_java/core/MSIMEInputService.java")
if ! rg -q 'ScreenCaptureProtection\.apply\(imeWindow == null \? null : imeWindow\.getWindow\(\), screenCaptureProtected\);' <<< "$apply_local_body"; then
  echo "The Android input method window must follow the screen capture protection setting in applyLocalSettings" >&2
  exit 1
fi
# 登录前的明示同意（.agents/notes/implemented/bug-fix/2026-10-27-login-privacy-consent.md）：勾选规则由 LoginConsentPolicySmoke 覆盖，接线在 LoginSheet 里，JVM 冒烟碰不到，这里钉住：面板上不再有「登录即表示同意」；每一个发请求的登录操作（经 begin 进入忙碌状态的 Apple、Google、发送验证码和验证码登录）都在同一行先过 confirmConsent；Google、Apple 和邮箱验证码的登录流程只从 LoginSheet 发起，新入口不能绕过面板；文字与提示和共享设置页 login-consent.tsx 的一字不差。
login_sheet="$repo_root/platforms/android/java/app/msime/android/home/LoginSheet.java"
login_consent="$repo_root/platforms/android/java/app/msime/android/home/LoginConsentPolicy.java"
shared_login_consent="$repo_root/packages/ui/src/account/login-consent.tsx"
if rg -q '登录即' "$login_sheet" \
  || ! awk '/[^a-zA-Z]begin\("/ { calls++; if ($0 !~ /!confirmConsent\(\) \|\|/) bad = 1 } END { exit (bad || calls != 4) }' "$login_sheet" \
  || rg -l 'SignIn\.(startGoogle|startApple|requestEmailCode|verifyEmailCode)\(' "$repo_root/platforms/android/java" \
    | rg -v '/home/LoginSheet\.java$' >/dev/null \
  || ! rg -qF 'export const LOGIN_CONSENT_PROMPT = "请先阅读并勾选同意《用户协议》和《隐私政策》";' "$shared_login_consent" \
  || ! rg -qF 'aria-label="我已阅读并同意《用户协议》和《隐私政策》"' "$shared_login_consent" \
  || ! rg -qF 'TEXT = "我已阅读并同意" + TERMS_TITLE + "和" + PRIVACY_TITLE;' "$login_consent" \
  || ! rg -qF 'PROMPT = "请先阅读并勾选同意" + TERMS_TITLE + "和" + PRIVACY_TITLE;' "$login_consent"; then
  echo "Every Android sign-in request must first pass LoginSheet's explicit consent checkbox, worded as the shared login-consent.tsx" >&2
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
# 详情页最多四个顶部分类（.agents/notes/implemented/feature/2026-10-13-android-settings-top-tabs.md）：分类是等宽分段，第五个会把每段挤得比其他页面窄。条件性分类也算在内，它出现时同样会挤。
for page in "$repo_root"/platforms/android/java/app/msime/android/home/*.java; do
  titles=$(tr '\n' ' ' < "$page" | grep -oE 'sectionTitles\(\) *\{ *return +(java\.util\.)?List\.of\([^)]*\)' || true)
  [ -n "$titles" ] || continue
  count=$({ grep -o '"[^"]*"' <<< "$titles" || true; } | wc -l | tr -d ' ')
  if [ "$count" -gt 4 ]; then
    echo "Android detail page declares $count section tabs, more than four: ${page#"$repo_root/"}" >&2
    exit 1
  fi
done
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
  "$repo_root/platforms/android/java/app/msime/android/home/LoginConsentPolicy.java" \
  "$repo_root/platforms/android/java/app/msime/android/home/OnboardingChoicePolicy.java" \
  "$repo_root/platforms/android/java/app/msime/android/home/HelpcodeChoicePolicy.java" \
  "$repo_root/platforms/android/java/app/msime/android/home/KeyLayoutStylesPolicy.java" \
  "$repo_root/platforms/android/java/app/msime/android/home/ReorderPolicy.java" \
  "$repo_root/platforms/android/gradle-app/ocr-module/src/main/java/app/msime/ocr/module/OcrPipeline.java" \
  "${test_sources[@]}"
# 可选 OCR 模块的前后处理只用 Java 标准库，跟宿主的纯 Java 模型一起编译，由 tests/ocr/OcrPipelineSmoke.java 覆盖；模块里调用 ONNX Runtime 和 Android 图形接口的 OcrModule 只由 Gradle 编译。
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
