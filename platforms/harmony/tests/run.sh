#!/usr/bin/env bash
# 在 node 下运行移植过来的键盘逻辑。HarmonyOS 自己的 hypium 测试需要插桩并依赖设备；这些类不依赖 ArkUI 或 NAPI，所以可以改在这里检查。
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$here/../../.." && pwd)
tsc="$repo_root/apps/desktop/node_modules/.bin/tsc"
if [[ ! -x "$tsc" ]]; then
  echo "TypeScript compiler not found at $tsc; run pnpm install at the repository root" >&2
  exit 1
fi
# grep rather than rg: the HarmonyOS CI job installs no ripgrep, and a missing rg made this guard report a regression that was not there.
if ! grep -qF 'StagedResources.removeDirectory(this.cacheRoot)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeySoundPlayer.ets"; then
  echo "key sound reloads must remove the rendered cache tree, not only the empty root" >&2
  exit 1
fi
# Resource staging must build off to the side and swap a complete tree into place; copying into the
# live directory lets the settings and keyboard processes observe a half-staged engine.
if ! grep -qF 'StagedResources.replaceDirectory(staging, destination)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/StagedResources.ets"; then
  echo "Harmony resource staging must replace the live directory only after the staged tree is complete" >&2
  exit 1
fi
# Starter insertion is several separately locked native operations; the returned panel snapshot
# must be reloaded after that sequence so a concurrent settings edit cannot leave it stale.
if ! grep -qF "await KeyboardSession.commonPhrasesCall(directory, { operation: 'load' })" \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"; then
  echo "Harmony common phrases must reload after starter seeding" >&2
  exit 1
fi
# 字母键的 ForEach 按 faceKey() 判定子树是否重建。「双拼键位提示」不在这个键里时，键盘开着切换开关，已经画好的字母键会一直留着旧提示。
face_key_body=$(sed -n '/^  private faceKey(): string {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
# 先取进变量再判断：管道交给 `grep -q` 时它命中即退出，sed 还没写完的输出会收到 SIGPIPE，pipefail 下这一步偶发失败。
if ! grep -qF 'this.shuangpinKeyHints' <<< "$face_key_body"; then
  echo "KeyboardView.faceKey must include the shuangpin key hint switch so a live change redraws the letter keys" >&2
  exit 1
fi
# 底行样式和数字行同理：设置页改了之后键盘开着也要按新排法重建键，所以这四个值都要在 faceKey() 里；密码框、数字框退回标准底行也换键，所以编辑框这一项也要在。
for field in this.twentySixKeyStyle this.fourteenKeyStyle this.nineKeyStyle this.numberRow this.standardRowsField; do
  if ! grep -qF "$field" <<< "$face_key_body"; then
    echo "KeyboardView.faceKey must include $field so a live layout style change redraws the keys" >&2
    exit 1
  fi
done
# 统计页认证默认关闭，认证成功后五分钟冷却。冷却规则由 keyboard-logic.test.ts 的 `StatisticsAuthenticationCooldown` 覆盖；认证本身是 ArkTS，node 下编不了，这里钉住接线：默认值是关、冷却期内直接放行、认证成功开启冷却、离开时从现在算起，时钟用含深度睡眠的开机时间。
statistics_auth="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/settings/StatisticsAuthentication.ets"
if [[ $(grep -cF "getSync('enabled', false)" "$statistics_auth") -ne 2 ]] \
  || grep -qF "getSync('enabled', true)" "$statistics_auth" \
  || ! grep -qF 'if (cooldown.active()) {' "$statistics_auth" \
  || ! grep -qF 'if (success) cooldown.granted();' "$statistics_auth" \
  || ! grep -qF 'cooldown.ended();' "$statistics_auth" \
  || ! grep -qF 'systemDateTime.TimeType.STARTUP' "$statistics_auth"; then
  echo "Statistics authentication must default to off and skip re-authentication within the cooldown" >&2
  exit 1
fi
# 功能面板「手写」磁贴再点一次要回到进入手写前的入口和中英状态（#9703）。入口怎么选由 keyboard-logic.test.ts 的 `KeyboardScheme.handwritingReturn` 覆盖；视图是 ArkTS，node 下编不了，这里钉住接线：离开手写经 handwritingReturn 取入口，进入前是英文时回到英文。
toggle_body=$(sed -n '/^  private toggleHandwritingScheme(): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'KeyboardScheme.handwritingReturn(before' <<< "$toggle_body" \
  || ! grep -qF 'this.englishBeforeHandwriting' <<< "$toggle_body"; then
  echo "Leaving handwriting from the function panel must return to the scheme and English mode it was entered from" >&2
  exit 1
fi
# 候选胶囊的释义列按候选取舍（`CandidateChipWidth.reservesColumn`），最小宽度只有触控下限（`CandidateChipWidth.touchMinimum`），规则由 keyboard-logic.test.ts 覆盖；视图是 ArkTS，这里钉住接线：手机胶囊的最小宽度按候选算，不能退回整页一律留一栏，也不能再取皮肤桌面格宽 `itemMinWidth`。
min_width_body=$(sed -n '/^  private chipMinWidth(candidate: CandidateEntry): number {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'CandidateChipWidth.reservesColumn(' <<< "$min_width_body" \
  || ! grep -qF 'CandidateChipWidth.touchMinimum(' <<< "$min_width_body" \
  || grep -qF 'itemMinWidth' <<< "$min_width_body"; then
  echo "KeyboardView.chipMinWidth must ask CandidateChipWidth.reservesColumn and touchMinimum, and must not use the skin's desktop itemMinWidth" >&2
  exit 1
fi
# 数字行的数字在组字中先交给引擎，收没收只能看引擎的 handled：韩语、越南语、藏文的引擎会先上屏组字再把数字交回来，`press()` 按组字变了算作收下，数字就丢了。视图和会话都是 ArkTS，这里钉住接线：数字行走 `pressElseCommitThenType`，它按 apply() 的 handled 决定，不收时 `commitThenType`。
digit_body=$(sed -n '/^  private typeNumberRowDigit(digit: string): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
press_type_body=$(sed -n '/^  pressElseCommitThenType(ascii: number): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets")
if ! grep -qF 'KeyboardSession.shared.pressElseCommitThenType(' <<< "$digit_body" \
  || grep -qF 'KeyboardSession.shared.press(' <<< "$digit_body" \
  || ! grep -qF 'if (this.apply(client.character(this.handle, ascii, false))) return;' <<< "$press_type_body" \
  || ! grep -qF 'this.commitThenType(ascii);' <<< "$press_type_body"; then
  echo "A number-row digit typed while composing must go through KeyboardSession.pressElseCommitThenType, which decides on the engine's handled flag and types the digit after the commit when the engine declines it" >&2
  exit 1
fi
# 冷启动第一帧的高度（`TouchLayoutStylePolicy.firstPanel`，规则由 touch-layout-styles.test.ts 覆盖）：扩展能力要在挂上视图之前定下形状，视图出现时在任何带 @Watch 的状态被赋值之前从同一个形状出发核对，否则两边起点不同，记下的键面与实际不符时面板会停在错的高度。两边都是 ArkTS，这里钉住接线次序。
ability="$repo_root/platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
decide_line=$(grep -nF 'KeyboardSession.shared.decideFirstTouchPanel(' "$ability" | head -n 1 | cut -d: -f1)
mount_line=$(grep -nF "await panel.setUiContent('keyboard/KeyboardView');" "$ability" | head -n 1 | cut -d: -f1)
if [[ -z "$decide_line" || -z "$mount_line" ]] || (( decide_line >= mount_line )) \
  || ! grep -qF 'settings.candidatePreeditFontSize, false, opened.strip, opened.extraRows);' "$ability"; then
  echo "KeyboardExtensionAbility must decide the first touch panel shape before mounting KeyboardView and size the panel from it" >&2
  exit 1
fi
appear_body=$(sed -n '/^  aboutToAppear(): void {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
opened_line=$(grep -nF 'KeyboardSession.shared.firstTouchPanel();' <<< "$appear_body" | head -n 1 | cut -d: -f1)
settings_line=$(grep -nF 'this.applySettings();' <<< "$appear_body" | head -n 1 | cut -d: -f1)
if [[ -z "$opened_line" || -z "$settings_line" ]] || (( opened_line >= settings_line )); then
  echo "KeyboardView.aboutToAppear must take its height baseline from KeyboardSession.firstTouchPanel before applySettings assigns any watched state" >&2
  exit 1
fi
# 九键「右侧大回车」的回车在右列、不与中/英同行（`NineKeyLayout.languageBesideReturn` 由 touch-layout-styles.test.ts 覆盖）：九键底行只在中/英紧挨回车时让出键帽，底行里没有回车时不画回车。
nine_row_body=$(sed -n '/^  nineKeyActionRow() {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'hitRegion: NineKeyLayout.languageBesideReturn(this.nineKeyActions()) ? this.languageKeyHitRegion() : undefined,' <<< "$nine_row_body" \
  || ! grep -qF 'if (this.nineKeyActionShown(NineKeyActionKind.RETURN)) {' <<< "$nine_row_body"; then
  echo "KeyboardView.nineKeyActionRow must yield the return key only when the language key sits beside it, and draw return only when the row has one" >&2
  exit 1
fi
# 九键「右侧大回车」网格下方那一段（`NineKeyLayout.tallReturnRow` 由 touch-layout-styles.test.ts 覆盖）：外框量自己的宽度、消失时清零（下次出现时宽度可能已变，比如在别的键面上转了屏，清零后第一帧按份额排，不拿旧宽度定出比网格还宽的固定键），123、中/英、空格和 0 按 `nineKeyActionWidth` / `nineKeyActionShare` 定宽，数字层的 0 排在空格左边。
tall_frame_body=$(sed -n '/^  tallReturnNineKeyFrame() {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
zero_body=$(sed -n '/^  nineKeyZeroKey() {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
# 找不到时 grep 返回 1，pipefail 会让脚本不打印原因就退出，所以这里吞掉退出码，由下面的判断报告。
zero_before_line=$(grep -nF 'if (this.nineKeyActionBeforeSpace(NineKeyActionKind.ZERO)) {' <<< "$nine_row_body" | head -n 1 | cut -d: -f1 || true)
nine_space_line=$(grep -nF 'this.spaceKey(true, this.nineKeyActionShare(NineKeyActionKind.SPACE),' <<< "$nine_row_body" | head -n 1 | cut -d: -f1 || true)
if ! grep -qF 'this.tallReturnFaceWidth = Number(now.width);' <<< "$tall_frame_body" \
  || ! grep -qF 'this.tallReturnFaceWidth = 0;' <<< "$tall_frame_body" \
  || ! grep -qF 'this.nineKeyActionWidth(NineKeyActionKind.SPACE));' <<< "$nine_row_body" \
  || ! grep -qF '.width(this.nineKeyActionWidth(NineKeyActionKind.LAYER_TOGGLE))' <<< "$nine_row_body" \
  || ! grep -qF '.layoutWeight(this.nineKeyActionShare(NineKeyActionKind.LAYER_TOGGLE))' <<< "$nine_row_body" \
  || ! grep -qF 'width: this.nineKeyActionWidth(NineKeyActionKind.LANGUAGE),' <<< "$nine_row_body" \
  || ! grep -qF 'share: this.nineKeyActionShare(NineKeyActionKind.LANGUAGE),' <<< "$nine_row_body" \
  || ! grep -qF 'width: this.nineKeyActionWidth(NineKeyActionKind.ZERO),' <<< "$zero_body" \
  || ! grep -qF 'share: this.nineKeyActionShare(NineKeyActionKind.ZERO),' <<< "$zero_body" \
  || [[ -z "$zero_before_line" || -z "$nine_space_line" ]] || (( zero_before_line >= nine_space_line )); then
  echo "KeyboardView must measure the tall return frame, size its middle bottom row from NineKeyLayout.tallReturnRow, and draw the digit layer's 0 left of space when the row says so" >&2
  exit 1
fi
# 九键「右侧大回车」右列与中间网格逐行对齐（`NineKeyLayout.tallReturnGridRowHeight` 由 touch-layout-styles.test.ts 覆盖）：一格的高度按这块网格自己的行数算，笔画是 `StrokeLayout.ROWS` 的两行，⌫ 只占一格，回车跨网格最后一行和底行。按三行切时笔画的回车从第二行中间开始。
grid_row_body=$(sed -n '/^  private tallReturnGridRowHeight(): number {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
grid_rows_body=$(sed -n '/^  private tallReturnGridRows(): number {$/,/^  }$/p' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
if ! grep -qF 'NineKeyLayout.tallReturnGridRowHeight(this.gridHeight(), this.rowSpacing(), this.tallReturnGridRows());' <<< "$grid_row_body" \
  || ! grep -qF 'return StrokeLayout.ROWS.length;' <<< "$grid_rows_body" \
  || grep -qF 'this.tallReturnGridRowHeight() *' <<< "$tall_frame_body" \
  || (( $(grep -cF '.height(this.tallReturnGridRowHeight())' <<< "$tall_frame_body") != 3 )); then
  echo "The tall return frame's right column must cut one key per grid row from the face's own row count (two stroke rows), so return spans the last grid row and the bottom row" >&2
  exit 1
fi
# 视图出现前（`setUiContent` 与 `aboutToAppear` 谁先完成没有保证）请求的面板高度不能丢（`TouchHeightRelay`，规则由 touch-layout-styles.test.ts 覆盖）：会话经它转交请求，扩展能力第一帧优先用留着的请求、挂回调时补发、换面板时作废旧请求。
view="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
session="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
resize_body=$(sed -n '/^  resizeTouchKeyboard(heightVp: number): void {$/,/^  }$/p' "$session")
if ! grep -qF 'this.touchHeight.request(heightVp);' <<< "$resize_body" \
  || ! grep -qF 'const requested: number | null = desktop ? null : KeyboardSession.shared.touchHeight.takePending();' "$ability" \
  || ! grep -qF ': requested !== null ? requested : KeyboardMetrics.totalHeightVp(' "$ability" \
  || ! grep -qF 'KeyboardSession.shared.touchHeight.attach((requestedHeightVp: number): void => {' "$ability" \
  || (( $(grep -cF 'KeyboardSession.shared.touchHeight.detach();' "$ability") < 2 )); then
  echo "A touch height requested before KeyboardExtensionAbility registers its resize callback must size the first panel or be replayed on attach, and a released panel's requests must be dropped" >&2
  exit 1
fi
attach_line=$(grep -nF 'KeyboardSession.shared.touchHeight.attach(' "$ability" | head -n 1 | cut -d: -f1)
take_line=$(grep -nF 'KeyboardSession.shared.touchHeight.takePending();' "$ability" | head -n 1 | cut -d: -f1)
reset_line=$(grep -nF 'KeyboardSession.shared.touchHeight.detach();' "$ability" | head -n 1 | cut -d: -f1)
if (( reset_line >= mount_line || take_line <= mount_line || attach_line <= take_line )); then
  echo "KeyboardExtensionAbility must drop stale touch heights before mounting the view, take a pending one after mounting, and attach the resize callback last" >&2
  exit 1
fi
# 视图每次核对或重定面板高度都要记下键面，否则 touch_panel_face.json 不会写，扩展能力冷启动永远按没记过建面板。
for fn in 'private syncStripHeight(): void {' 'private resizeTouchPanel(): void {'; do
  body=$(sed -n "/^  ${fn}\$/,/^  }\$/p" "$view")
  if ! grep -qF 'this.rememberPanelFace();' <<< "$body"; then
    echo "KeyboardView.${fn%%(*} must call rememberPanelFace so the next cold start sizes the first panel from the face it drew" >&2
    exit 1
  fi
done
# 九键「底栏带标点」的逗号长按打开常用标点（`NineKeyLayout.commaOpensQuickPunctuation` 由 touch-layout-styles.test.ts 覆盖），这里钉住视图接上了它和长按手势。
comma_body=$(sed -n '/^  nineKeyCommaKey() {$/,/^  }$/p' "$view")
if ! grep -qF 'if (NineKeyLayout.commaOpensQuickPunctuation(this.symbols, this.english, this.nineKeyFrameStyle())) {' <<< "$comma_body" \
  || ! grep -qF '.gesture(LongPressGesture().onAction(() => {' <<< "$comma_body" \
  || ! grep -qF 'this.punctuation = true;' <<< "$comma_body"; then
  echo "KeyboardView.nineKeyCommaKey must open the quick punctuation list on long press when NineKeyLayout.commaOpensQuickPunctuation says so" >&2
  exit 1
fi
# 密码框和只收数字的编辑框里三种键面类型都退回标准底行（`TouchLayoutStylePolicy.fieldRowStyle`、`EditorPolicy.keepsStandardRows` 由测试覆盖）：会话从编辑框类型认出只收数字，视图每次接上编辑框都刷新这个状态，九键外框与 26/14/18 键底行都经 fieldRowStyle 取样式。
traits_body=$(sed -n '/^  private static traitsFor(pattern: number): EditorTraits {$/,/^  }$/p' "$session")
editor_face_body=$(sed -n '/^  private applyEditorFace(): void {$/,/^  }$/p' "$view")
nine_style_body=$(sed -n '/^  private nineKeyFrameStyle(): string {$/,/^  }$/p' "$view")
design_style_body=$(sed -n '/^  private designRowStyle(): string {$/,/^  }$/p' "$view")
if ! grep -qF 'digits: pattern === inputMethodEngine.PATTERN_NUMBER' <<< "$traits_body" \
  || ! grep -qF '|| pattern === inputMethodEngine.PATTERN_PHONE' <<< "$traits_body" \
  || ! grep -qF '|| pattern === inputMethodEngine.PATTERN_NUMBER_DECIMAL' <<< "$traits_body" \
  || ! grep -qF 'this.standardRowsField = KeyboardSession.shared.editorKeepsStandardRows();' <<< "$editor_face_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.fieldRowStyle(this.nineKeyStyle, this.standardRowsField)' <<< "$nine_style_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.fieldRowStyle(this.twentySixKeyStyle, this.standardRowsField)' <<< "$design_style_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.fieldRowStyle(this.fourteenKeyStyle, this.standardRowsField)' <<< "$design_style_body"; then
  echo "Password and digit-only fields must fall back to the standard row for every key type: KeyboardSession.traitsFor sets digits from PATTERN_NUMBER/PHONE/NUMBER_DECIMAL, applyEditorFace refreshes standardRowsField, and nineKeyFrameStyle and designRowStyle go through TouchLayoutStylePolicy.fieldRowStyle" >&2
  exit 1
fi
# AI 生成类功能的总闸（`AiGenerationGate`，取自 Rust 的 `HostCapabilities.ai_generation`）：规则由 ai-generation-gate.test.ts 覆盖；键盘、语音润色和设置桥是 ArkTS，node 下编不了，这里钉住接线：两个进程启动时都读宿主能力，功能面板按它去掉 AI 格子，AI 联想、高情商回复、润色和设置桥的 AI 请求在发出前都问它。
ets="$repo_root/platforms/harmony/entry/src/main/ets"
for wiring in \
  "keyboard/KeyboardSession.ets:AiGenerationGate.adopt(client.hostCapabilities('harmony'))" \
  "keyboard/KeyboardSession.ets:aiQuery.ai_eligible && AiGenerationGate.enabled()" \
  "keyboard/KeyboardSession.ets:request === null || !AiGenerationGate.enabled()" \
  "keyboard/KeyboardView.ets:FunctionPanelPolicy.items(AiGenerationGate.enabled())" \
  "keyboard/input/HarmonyVoicePolisher.ets:if (!AiGenerationGate.enabled()) return null;" \
  "pages/Settings.ets:AiGenerationGate.adopt(client.hostCapabilities(PLATFORM))" \
  "pages/Settings.ets:if (!AiGenerationGate.allowsSettingsRequest(kind, payload)) {"; do
  if ! grep -qF "${wiring#*:}" "$ets/${wiring%%:*}"; then
    echo "${wiring%%:*} must consult AiGenerationGate: missing \"${wiring#*:}\"" >&2
    exit 1
  fi
done
if grep -qF 'FunctionPanelPolicy.items()' "$ets/keyboard/KeyboardView.ets"; then
  echo "KeyboardView must list function tiles through FunctionPanelPolicy.items(AiGenerationGate.enabled())" >&2
  exit 1
fi
# 按键振动的 `startVibration` 在 NAPI 层同步发 IPC，放在 UI 线程上开了振动打字就跟不上手；这里钉住接线：振动经 TaskPool 以高优先级发出，上一下还没发完时不再排队。
if ! grep -qF 'new taskpool.Task(startKeyVibration,' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'taskpool.Priority.HIGH);' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'this.vibrationPending) {' "$ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF '@Concurrent' "$ets/keyboard/KeyVibration.ets"; then
  echo "key vibration must go through a high-priority taskpool.Task(startKeyVibration, ...) and skip while one is pending" >&2
  exit 1
fi
"$tsc" --project "$here/tsconfig.json"
# 上面已编译所有 tests/*.test.ts；逐个运行它们的输出。遍历源文件而不是输出目录，可以避免已删除测试遗留的旧 .js 被运行。
for source in "$here"/*.test.ts; do
  node "$repo_root/target/harmony-tests/tests/$(basename "${source%.ts}").js"
done
# The key-sound renderer is plain C++ over miniaudio with no NAPI in it, so the build machine's compiler checks what it writes and what it refuses.
out="$repo_root/target/harmony-tests"
miniaudio="$repo_root/platforms/windows/third_party/miniaudio"
"${CXX:-c++}" -std=c++17 -O1 -c "$here/../native/miniaudio.cpp" -I"$miniaudio" -o "$out/miniaudio.o"
"${CXX:-c++}" -std=c++17 -O1 -Wall -Wextra -Werror -I"$here/../native" -I"$miniaudio" \
  "$here/key-sound-render.test.cpp" "$here/../native/key_sound_render.cpp" "$out/miniaudio.o" \
  -lm -lpthread -o "$out/key-sound-render"
scratch="$out/key-sound-render.d"
rm -rf "$scratch"
mkdir -p "$scratch"
"$out/key-sound-render" "$scratch"
rm -rf "$scratch"
