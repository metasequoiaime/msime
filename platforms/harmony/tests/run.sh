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
# 引擎的 character(0x20) 从不选词：触屏空格必须走 pressSpace（选高亮候选，运行时不处理时键盘自己输入空格），回到 press(0x20) 时中文模式下空格既不上屏候选也打不出空格。
if ! grep -qF 'KeyboardSession.shared.pressSpace();' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets" \
  || ! grep -qF 'this.command(InputCommand.SELECT_HIGHLIGHTED) || this.inputClient === undefined' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"; then
  echo "Harmony touch Space must select the highlighted candidate and type a space when the runtime declines it" >&2
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
for field in this.twentySixKeyStyle this.fourteenKeyStyle this.nineKeyStyle this.numberRow this.standardRowsField this.twentySixSymbolFirst this.numberPadSpace this.languageKeyLeft; do
  if ! grep -qF "$field" <<< "$face_key_body"; then
    echo "KeyboardView.faceKey must include $field so a live layout style change redraws the keys" >&2
    exit 1
  fi
done
# 组字中 Shift 加字母是辅助码：引擎只认大写字母，不看 Shift 标志，所以字母键要按 Shift 发大写字母，规则由 keyboard-logic.test.ts 覆盖，这里钉住视图的接线。
if ! grep -qF 'const input: string = ChineseHelpcodePolicy.letterInput(letter, shifted);' \
  "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"; then
  echo "KeyboardView.tapLetter must send a shifted letter as a capital: the Engine reads helpcode from the case, not from the shift flag" >&2
  exit 1
fi
# 26 键组字时的上滑、下滑辅助码（鸿蒙手机）：规则由 helpcode-swipe.test.ts 覆盖，这里钉住接线。只在 qwerty 26 键且已有全拼或双拼组字时跟踪，触发后沿 press(..., true) 的辅助码路径送大写，并吞掉 ArkUI 之后到达的点击；长按仍交给角标提示。
keyboard_view="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
helpcode_swipe="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/input/HelpcodeSwipePolicy.ts"
if ! grep -qF 'private helpcodeSwipes: HelpcodeSwipeGesture' "$keyboard_view" 2>/dev/null \
  || ! grep -qF 'this.helpcodeSwipes.down(' "$keyboard_view" \
  || ! grep -qF 'this.helpcodeSwipeEnabled()' "$keyboard_view" \
  || ! grep -qF 'this.typeHelpcodeSwipe(helpcode);' "$keyboard_view" \
  || ! grep -qF 'KeyboardSession.shared.press(input.charCodeAt(0), true);' "$keyboard_view" \
  || ! grep -qF 'KeyboardSession.shared.entersHelpcode(true)' "$keyboard_view" \
  || ! grep -qF 'static swiped(' "$helpcode_swipe"; then
  echo "Harmony 26-key helpcode swipes must be wired through HelpcodeSwipeGesture and the existing uppercase Engine path" >&2
  exit 1
fi
# 123 层和数字九宫格上的辅助码触发键先问 Engine，收下就不按原样上屏。
for body in tapLayerCharacter typeNumberPadText; do
  view_body=$(sed -n "/^  private $body(text: string): void {\$/,/^  }\$/p" \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets")
  if ! grep -qF 'if (this.typeHelpcodeTrigger(text)) {' <<< "$view_body"; then
    echo "KeyboardView.$body must offer a helpcode trigger to the Engine before inserting the key's face" >&2
    exit 1
  fi
done
# 「符」与「123」互换和数字键盘空格位置（#10056）：排法由 touch-layout-styles.test.ts 覆盖，这里钉住视图的接线：设计底行按开关互换，借来的数字九宫格按互换后的字母层排前两键，数字层底行按空格位置排，右列最下面一格在空格位置为右列时画空格；两项从会话偏好读，设置页改了就随 applySettings 换。
keyboard_view="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
if ! grep -qF 'const row: DesignRowEntry[] = TouchLayoutStylePolicy.symbolFirst(TouchLayoutStylePolicy.designRow(' "$keyboard_view" \
  || ! grep -qF '|| this.letterRowSymbolLeads());' "$keyboard_view" \
  || ! grep -qF 'return NineKeyLayout.actionRow(this.symbols, this.english, this.nineKeyFrameStyle(), nineKeyDigits,' "$keyboard_view" \
  || ! grep -qF 'if (this.symbols && this.numberPadSpace === TouchLayoutStylePolicy.RIGHT_COLUMN) {' "$keyboard_view" \
  || ! grep -qF 'this.twentySixSymbolFirst = this.settings.twentySixSymbolFirst;' "$keyboard_view" \
  || ! grep -qF 'this.numberPadSpace = this.settings.numberPadSpace;' "$keyboard_view" \
  || ! grep -qF 'numberPadSpace: TouchLayoutStylePolicy.numberPadSpace(preferences.touch_number_pad_space)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'twentySixSymbolFirst: preferences.touch_twenty_six_symbol_first === true,' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"; then
  echo "Harmony must lay the 26-key row by touch_twenty_six_symbol_first and the number pad by touch_number_pad_space" >&2
  exit 1
fi
# 触屏 123 一律打开数字九宫格（维护者的产品决定）：规则由 keyboard-logic.test.ts 的 opensNumberPad 覆盖，这里钉住视图的接线。numberPadShowing 只看符号层、#+= 和触屏，不再按长空格、英文、本地模式或独立语言排除；假名网格只在字母层（和 2in1）画，它的 123 进数字九宫格；数字、小数点和出厂运算符经 typeNumberPadText 输入（交不交给会话见下一段）；返回键按借出的键面印；大千注音 #+= 的共享符号行用 123 回到数字九宫格。
number_pad_body=$(sed -n '/^  private numberPadShowing(): boolean {$/,/^  }$/p' "$keyboard_view")
if ! grep -qF 'return NineKeyLayout.opensNumberPad(this.symbols, this.moreSymbols, this.touchDesign());' <<< "$number_pad_body" \
  || grep -qE 'longSpaceActive|english|localMode|japanese|koreanFace|vietnameseFace|tibetanFace|zhuyinFace' <<< "$number_pad_body" \
  || ! grep -qF 'if (this.kanaGridShowing()) {' "$keyboard_view" \
  || ! grep -qF 'return this.touchDesign() && !this.kanaGridShowing() && this.gridActionRow();' "$keyboard_view" \
  || ! grep -qF 'this.typeNumberPadText(digit);' "$keyboard_view" \
  || ! grep -qF "this.typeNumberPadText('.');" "$keyboard_view" \
  || ! grep -qF 'this.typeNumberPadText(symbol);' "$keyboard_view" \
  || ! grep -qF '? this.numberPadReturnTitle()' "$keyboard_view" \
  || ! grep -qF 'face: NumberSymbolLayout.layerKeyFace(this.symbols, this.moreSymbols),' "$keyboard_view"; then
  echo "Harmony touch 123 must open the number pad on every scheme and keep each scheme's digit routing and return title" >&2
  exit 1
fi
# 数字九宫格的后续决定（维护者，三端相同）：规则由 keyboard-logic.test.ts 的「the ASCII number pad column, its extras and the Zhuyin #+= page」和「number pad keys reach the session only as spelling」覆盖，这里钉住视图的接线。(1) 半角语境的左列是 NineKeyLayout.digitSidebar 按 numberPadPunctuation 给出的表（出厂运算符后接 @ : ; ( ) ' " $，自定义过就照用户的表），画在滚动栈里；出厂项和半角符号与出厂运算符同经 numberPadMarkOptions → typeNumberPadText 输入，用户的表照旧经 insertSidebarSymbol 按字面上屏（藏文里也是）。(2) 数字九宫格上的键（数字、0、小数点、左列出厂项、顶部补充行）只由 NineKeyLayout.numberPadToSession 决定：本地模式、越南语、藏文里引擎此刻列为拼写符号的字符和藏文的 ' 交给会话（press），其余一律经 insertNumberPadLiteral 按字面上屏；typeNumberPadText 不再按日语、韩文、注音分流，也不借 123 层的路由。(3) 注音的 #+= 由 NumberSymbolLayout.drawsDesignLayer 保留共享符号行，设计底行那一格按 layerKeyFace 印 123、在 #+= 上点按关掉 #+= 回到数字九宫格。
digit_items_body=$(sed -n '/^  private digitSidebarItems(): NineKeySidebarItem\[\] {$/,/^  }$/p' "$keyboard_view")
type_pad_body=$(sed -n '/^  private typeNumberPadText(text: string): void {$/,/^  }$/p' "$keyboard_view")
extra_key_body=$(sed -n '/^  symbolExtraKey(text: string) {$/,/^  }$/p' "$keyboard_view")
custom_item_body=$(sed -n '/^  private customSidebarOptions(symbol: string, digits: boolean): KeyOptions {$/,/^  }$/p' "$keyboard_view")
design_row_key_body=$(sed -n '/^  designRowKey(item: DesignRowItem) {$/,/^  }$/p' "$keyboard_view")
if ! grep -qF 'NineKeyLayout.digitSidebar(this.digitSidebarTable(), this.numberPadPunctuation()),' <<< "$digit_items_body" \
  || ! grep -qF "if (NineKeyLayout.numberPadToSession(text, this.localMode !== 'none', this.vietnameseFace(), this.tibetanFace()," <<< "$type_pad_body" \
  || ! grep -qF 'KeyboardSession.shared.hardwareSpelling())) {' <<< "$type_pad_body" \
  || ! grep -qF 'KeyboardSession.shared.press(text.charCodeAt(0), false);' <<< "$type_pad_body" \
  || ! grep -qF 'this.insertNumberPadLiteral(text);' <<< "$type_pad_body" \
  || grep -qE 'japanese|koreanFace|zhuyinFace|english|tapLayerCharacter|tapSymbol' <<< "$type_pad_body" \
  || grep -qF 'numberPadTypesLiterally' "$keyboard_view" \
  || ! grep -qF 'KeyboardSession.shared.insertSidebarSymbol(symbol, digits);' <<< "$custom_item_body" \
  || grep -qE 'numberPadToSession|typeNumberPadText|tibetanFace|press\(' <<< "$custom_item_body" \
  || ! grep -qF 'if (this.numberPadShowing()) {' <<< "$extra_key_body" \
  || ! grep -qF 'this.typeNumberPadText(text);' <<< "$extra_key_body" \
  || ! grep -qF 'return NumberSymbolLayout.drawsDesignLayer(this.touchDesign(), this.symbols, this.zhuyinFace());' "$keyboard_view" \
  || ! grep -qF 'this.toggleMoreSymbols();' <<< "$design_row_key_body"; then
  echo "Harmony number pad must append the ASCII extras, hand only spelling symbols and the Tibetan ' to the session, and keep the Zhuyin #+= rows with 123 back to the pad" >&2
  exit 1
fi
# 九键左侧符号栏自定义（#10223）：空表回出厂表、右侧大回车只给出厂表接 !、一屏几项、ForEach 键和整对符号的光标由 nine-key-sidebar.test.ts 覆盖。视图和会话是 ArkTS，这里钉住接线：两张表从会话偏好读、随 applySettings 换；拼音九键左列、数字层左列和笔画标点栏在自定义时走各自的滚动栈，英文九键不读；自定义项经 insertSidebarSymbol 字面上屏，数字层与出厂运算符一样原样结束组字；两张表与本端出厂表相同时按出厂表读；ForEach 用带下标和高度的 item.key；长按带上那张表的共享偏好名打开「按键布局」里的编辑页，数字层两处（出厂与自定义）传数字表，字母层和笔画四处传字母表。
session_file="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
if ! grep -qF 'preferences.touch_nine_key_sidebar_symbols,' "$session_file" \
  || ! grep -qF 'TouchLayoutStylePolicy.NINE_KEY_SIDEBAR_DEFAULTS' "$session_file" \
  || ! grep -qF 'preferences.touch_nine_key_digit_sidebar_symbols,' "$session_file" \
  || ! grep -qF 'TouchLayoutStylePolicy.DIGIT_SIDEBAR_DEFAULTS' "$session_file" \
  || [[ $(grep -cF 'TouchLayoutStylePolicy.sidebarSymbols(' "$session_file") -ne 2 ]] \
  || ! grep -qF 'this.fixedKey(this.digitSidebarOptions(item.symbol));' "$keyboard_view" \
  || ! grep -qF '? this.customSidebarOptions(symbol, true) : this.numberPadMarkOptions(symbol);' "$keyboard_view" \
  || ! grep -qF 'if (PairedPunctuationPolicy.literalPairClosing(text) !== null) {' "$session_file" \
  || ! grep -qF 'this.nineKeySidebarSymbols = this.settings.nineKeySidebarSymbols;' "$keyboard_view" \
  || ! grep -qF 'this.nineKeyDigitSidebarSymbols = this.settings.nineKeyDigitSidebarSymbols;' "$keyboard_view" \
  || ! grep -qF 'if (this.symbols && NineKeyLayout.digitSidebarScrolls(this.digitSidebarTable(), this.numberPadPunctuation())) {' "$keyboard_view" \
  || ! grep -qF '} else if (NineKeyLayout.customSidebar(this.letterSidebarTable())) {' "$keyboard_view" \
  || [[ $(grep -cF 'NineKeyLayout.customSidebar(this.letterSidebarTable())' "$keyboard_view") -ne 2 ]] \
  || ! grep -qF 'return this.english ? [] : this.nineKeySidebarSymbols;' "$keyboard_view" \
  || ! grep -qF 'return this.english ? [] : NineKeyLayout.numberPadSymbols(this.nineKeyDigitSidebarSymbols,' "$keyboard_view" \
  || [[ $(grep -cF 'KeyboardSession.shared.insertSidebarSymbol(' "$keyboard_view") -ne 2 ]] \
  || [[ $(grep -cF '(item: NineKeySidebarItem) => item.key)' "$keyboard_view") -ne 3 ]] \
  || [[ $(grep -cF '.gesture(LongPressGesture().onAction(() => this.openSidebarSettings(true)))' "$keyboard_view") -ne 2 ]] \
  || [[ $(grep -cF '.gesture(LongPressGesture().onAction(() => this.openSidebarSettings(false)))' "$keyboard_view") -ne 4 ]] \
  || ! grep -qF 'digits ? SettingsPageLink.NINE_KEY_DIGIT_SIDEBAR_TARGET : SettingsPageLink.NINE_KEY_SIDEBAR_TARGET' <<< "$(sed -n '/^  private openSidebarSettings(digits: boolean): void {$/,/^  }$/p' "$keyboard_view")"; then
  echo "Harmony must draw the nine-key sidebars from touch_nine_key_sidebar_symbols and touch_nine_key_digit_sidebar_symbols" >&2
  exit 1
fi
# 「中/英键放在空格左边」（#10228）：排法由 touch-layout-styles.test.ts 覆盖，这里钉住视图的接线：设计底行在「符」与「123」互换之后再挪 中/英，九键外框（字母层、数字层、借来的数字九宫格）和右侧大回车量到宽度后的固定宽度都带上这一项，手写底行左边时把 中/英 画在 123 和 ， 之间、右边时照原样画在 。 后面；值从会话偏好读，设置页改了就随 applySettings 换。
handwriting_row_body=$(sed -n '/^  handwritingActionRow() {$/,/^  }$/p' "$keyboard_view")
left_language_line=$(grep -nF 'this.handwritingLeftLanguageKey()' <<< "$handwriting_row_body" | cut -d: -f1 || true)
comma_line=$(grep -nF 'action: () => this.typeComma()' <<< "$handwriting_row_body" | cut -d: -f1 || true)
if ! grep -qF 'return TouchLayoutStylePolicy.languageBeforeSpace(row, this.languageKeyLeft);' "$keyboard_view" \
  || ! grep -qF 'this.numberPadSpace, this.languageKeyLeft);' "$keyboard_view" \
  || ! grep -qF 'NineKeyLayout.tallReturnRow(this.symbols, this.tallReturnFaceWidth, this.keySpacing(), this.languageKeyLeft)' "$keyboard_view" \
  || ! grep -qF 'this.languageKeyLeft = this.settings.languageKeyLeft;' "$keyboard_view" \
  || ! grep -qF 'languageKeyLeft: TouchLayoutStylePolicy.languageKeyLeft(preferences.touch_language_key_side)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets" \
  || [[ $(grep -cF 'if (this.languageKeyLeft) {' <<< "$handwriting_row_body") -ne 1 ]] \
  || [[ $(grep -cF 'if (!this.languageKeyLeft) {' <<< "$handwriting_row_body") -ne 1 ]] \
  || [[ -z "$left_language_line" || -z "$comma_line" || "$left_language_line" -ge "$comma_line" ]]; then
  echo "Harmony must move the 中/英 key left of the space by touch_language_key_side on the design row, the nine-key frame, the tall return segment and the handwriting row" >&2
  exit 1
fi
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
# 冷启动第一帧的高度（`TouchLayoutStylePolicy.firstPanel`，规则由 touch-layout-styles.test.ts 覆盖）：扩展能力要在挂上视图之前定下形状，视图出现时在任何带 @Watch 的状态被赋值之前从同一个形状出发核对，否则两边起点不同，记下的键面与实际不符时面板会停在错的高度。第一帧的工具条也要按「候选栏大小」定高（#10137），和视图的 `stripHeight` 一致。两边都是 ArkTS，这里钉住接线次序。
ability="$repo_root/platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
decide_line=$(grep -nF 'KeyboardSession.shared.decideFirstTouchPanel(' "$ability" | head -n 1 | cut -d: -f1)
mount_line=$(grep -nF "await panel.setUiContent('keyboard/KeyboardView');" "$ability" | head -n 1 | cut -d: -f1)
if [[ -z "$decide_line" || -z "$mount_line" ]] || (( decide_line >= mount_line )) \
  || ! grep -qF 'settings.candidatePreeditFontSize, false, opened.strip, opened.extraRows, settings.candidateScalePercent,' "$ability" \
  || ! grep -qF 'KeyboardSession.shared.firstTouchLift());' "$ability"; then
  echo "KeyboardExtensionAbility must decide the first touch panel shape (and the bottom lift it counts) before mounting KeyboardView and size the panel from it" >&2
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
# 设计底行和九键外框底行用 ForEach 按策略表逐个画键（`TouchLayoutStylePolicy.designRowItems`、`NineKeyLayout.actionItems`，键序、份额、固定宽度、让位和 ForEach 的键由 touch-layout-styles.test.ts 覆盖）。按值传给 @Builder 的参数在编译后只在建键时取一次，键建好之后状态再变也不会刷新，所以份额、宽度和让位只能经 ForEach 的数组和键传下去，不能在行里直接写 `this.spaceKey(true, 随状态变的份额, …)`；底行表和「右侧大回车」的 `tallReturnRow` 也因此每次重建这一行只算一次。九键「右侧大回车」的外框量自己的宽度、消失时清零（下次出现时宽度可能已变，比如在别的键面上转了屏，清零后第一帧按份额排，不拿旧宽度定出比网格还宽的固定键）。
view_file="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
design_row_body=$(sed -n '/^  designActionRow() {$/,/^  }$/p' "$view_file")
design_key_body=$(sed -n '/^  designRowKey(item: DesignRowItem) {$/,/^  }$/p' "$view_file")
design_items_body=$(sed -n '/^  private designRowItems(): DesignRowItem\[\] {$/,/^  }$/p' "$view_file")
nine_row_body=$(sed -n '/^  nineKeyActionRow() {$/,/^  }$/p' "$view_file")
nine_key_body=$(sed -n '/^  nineKeyActionKey(item: NineKeyActionItem) {$/,/^  }$/p' "$view_file")
nine_items_body=$(sed -n '/^  private nineKeyActionItems(): NineKeyActionItem\[\] {$/,/^  }$/p' "$view_file")
tall_frame_body=$(sed -n '/^  tallReturnNineKeyFrame() {$/,/^  }$/p' "$view_file")
if ! grep -qF 'ForEach(this.designRowItems(), (item: DesignRowItem) => {' <<< "$design_row_body" \
  || ! grep -qF '(item: DesignRowItem): string => TouchLayoutStylePolicy.designRowItemKey(item))' <<< "$design_row_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.designRowItems(this.designRowEntries())' <<< "$design_items_body" \
  || ! grep -qF '.layoutWeight(item.share)' <<< "$design_key_body" \
  || ! grep -qF 'this.spaceKey(true, item.share, 0);' <<< "$design_key_body" \
  || ! grep -qF 'hitRegion: item.besideReturn ? this.languageKeyHitRegion() : undefined,' <<< "$design_key_body" \
  || ! grep -qF 'this.designReturnKey(item.share, item.besideReturn);' <<< "$design_key_body" \
  || grep -qF 'this.designRowEntries()' <<< "$design_key_body"; then
  echo "KeyboardView.designActionRow must draw TouchLayoutStylePolicy.designRowItems through ForEach keyed by designRowItemKey, passing each key its share and return yield from the item" >&2
  exit 1
fi
if ! grep -qF 'ForEach(this.nineKeyActionItems(), (item: NineKeyActionItem) => {' <<< "$nine_row_body" \
  || ! grep -qF '(item: NineKeyActionItem): string => NineKeyLayout.actionItemKey(item))' <<< "$nine_row_body" \
  || ! grep -qF 'NineKeyLayout.tallReturnRow(this.symbols, this.tallReturnFaceWidth, this.keySpacing(), this.languageKeyLeft)' <<< "$nine_items_body" \
  || ! grep -qF 'NineKeyLayout.actionItems(this.nineKeyActions(), fixed, NineKeyLayout.tallReturnFill(this.symbols))' <<< "$nine_items_body" \
  || ! grep -qF '.width(item.width)' <<< "$nine_key_body" \
  || ! grep -qF '.layoutWeight(item.share)' <<< "$nine_key_body" \
  || ! grep -qF 'this.nineKeyZeroKey(item.width, item.share)' <<< "$nine_key_body" \
  || ! grep -qF 'this.spaceKey(true, item.share, item.width);' <<< "$nine_key_body" \
  || ! grep -qF 'width: item.width,' <<< "$nine_key_body" \
  || ! grep -qF 'hitRegion: item.besideReturn ? this.languageKeyHitRegion() : undefined,' <<< "$nine_key_body" \
  || ! grep -qF 'this.designReturnKey(item.share, item.besideReturn);' <<< "$nine_key_body" \
  || ! grep -qF 'this.tallReturnFaceWidth = Number(now.width);' <<< "$tall_frame_body" \
  || ! grep -qF 'this.tallReturnFaceWidth = 0;' <<< "$tall_frame_body"; then
  echo "KeyboardView.nineKeyActionRow must draw NineKeyLayout.actionItems through ForEach keyed by actionItemKey, sizing the tall return middle row from the measured frame and passing each key its width, share and return yield from the item" >&2
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
comma_body=$(sed -n '/^  nineKeyCommaKey(share: number) {$/,/^  }$/p' "$view")
if ! grep -qF 'if (NineKeyLayout.commaOpensQuickPunctuation(this.symbols, this.english, this.nineKeyFrameStyle())) {' <<< "$comma_body" \
  || ! grep -qF '.gesture(LongPressGesture().onAction(() => {' <<< "$comma_body" \
  || ! grep -qF 'this.punctuation = true;' <<< "$comma_body"; then
  echo "KeyboardView.nineKeyCommaKey must open the quick punctuation list on long press when NineKeyLayout.commaOpensQuickPunctuation says so" >&2
  exit 1
fi
# 长空格逗号上滑或下滑输入句号，滑过门槛的那一刻和满长按时长都要在移动时判定（`CommaSwipeGesture` 由 touch-layout-styles.test.ts 覆盖，与 Android #10087 相同）。视图是 ArkTS，这里钉住接线：触摸的按下、移动、抬起都带着时间交给手势，设计底行逗号的长按先问 `longPress()`，已经滑出去时不再输入句号或打开列表。
comma_touch_body=$(sed -n '/^  private commaSwipeTouch(event: TouchEvent): void {$/,/^  }$/p' "$view")
design_comma_body=$(sed -n '/^  designCommaKey(share: number) {$/,/^  }$/p' "$view")
if ! grep -qF 'this.commaSwipe.down(touch.windowX, touch.windowY, Date.now());' <<< "$comma_touch_body" \
  || ! grep -qF '} else if (event.type === TouchType.Move) {' <<< "$comma_touch_body" \
  || ! grep -qF 'this.commaSwipe.move(touch.windowX, touch.windowY, Date.now());' <<< "$comma_touch_body" \
  || ! grep -qF 'this.commaSwipe.up(touch.windowX, touch.windowY, Date.now())' <<< "$comma_touch_body" \
  || ! grep -qF '} else if (!this.commaSwipe.longPress()) {' <<< "$design_comma_body"; then
  echo "KeyboardView must feed the comma touch's down, move and up with their time into CommaSwipeGesture, and the design-row comma's long press must ask commaSwipe.longPress() first" >&2
  exit 1
fi
# 九键滑动输入数字（#10229）：判定的方向、阈值、1 键、锁定和长按与点击的先后由 nine-key-swipe.test.ts 覆盖。会话和视图是 ArkTS，这里钉住接线：会话经 `NineKeySwipePolicy.mode` 读共享偏好 `touch_nine_key_swipe_digits`；拼音九键字母层的键（`nineKeyHoldKey`，两处网格都经它）把触摸交给 `nineKeySwipeTouch`，按下时读当前设置，滑出去之后点击和长按都不再做事；数字层和九键双拼不经 `nineKeyHoldKey`，不受影响；抬手按长按选数字的路径上屏（先结束组字再字面上屏），并补上点按的按键音、振动和键位统计；滑出数字时角标藏起来；键被替换时丢掉进行中的判定。
nine_hold_body=$(sed -n '/^  nineKeyHoldKey(key: NineKey) {$/,/^  }$/p' "$view")
nine_swipe_body=$(sed -n '/^  private nineKeySwipeTouch(key: NineKey, event: TouchEvent): void {$/,/^  }$/p' "$view")
nine_digit_body=$(sed -n '/^  private typeNineKeySwipeDigit(key: NineKey): void {$/,/^  }$/p' "$view")
drop_nine_body=$(sed -n '/^  private dropLetterGestures(): void {$/,/^  }$/p' "$view")
if ! grep -qF 'nineKeySwipeDigits: NineKeySwipePolicy.mode(preferences.touch_nine_key_swipe_digits)' "$session" \
  || [[ $(grep -cF 'if (!this.symbols && !this.nineKeyShuangpinFaceActive()' "$view") -ne 2 ]] \
  || ! grep -qF '.onTouch((event: TouchEvent): void => this.nineKeySwipeTouch(key, event))' <<< "$nine_hold_body" \
  || ! grep -qF 'if (this.nineKeySwipes.swallows(key.input)) {' <<< "$nine_hold_body" \
  || ! grep -qF 'if (!this.nineKeySwipes.hold(key.input)) {' <<< "$nine_hold_body" \
  || ! grep -qF '.visibility(this.nineKeySwipeKey === key.input ? Visibility.Hidden : Visibility.Visible)' <<< "$nine_hold_body" \
  || ! grep -qF 'this.settings.nineKeySwipeDigits' <<< "$nine_swipe_body" \
  || ! grep -qF 'this.nineKeySwipes.down(key.input, touch.id, touch.windowY, mode, hasLetters);' <<< "$nine_swipe_body" \
  || ! grep -qF 'this.nineKeySwipes.move(key.input, touch.id, touch.windowY);' <<< "$nine_swipe_body" \
  || ! grep -qF 'if (this.nineKeySwipes.up(key.input, touch.id) === NineKeySwipe.DIGIT) {' <<< "$nine_swipe_body" \
  || ! grep -qF 'this.nineKeySwipes.cancel(key.input);' <<< "$nine_swipe_body" \
  || ! grep -qF 'this.openKeyHold(NineKeyLayout.holdOptions(key));' <<< "$nine_swipe_body" \
  || ! grep -qF 'this.typeNineKeySwipeDigit(key);' <<< "$nine_swipe_body" \
  || ! grep -qF 'KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));' <<< "$nine_digit_body" \
  || ! grep -qF 'KeyboardSession.shared.holdFeedback();' <<< "$nine_digit_body" \
  || [[ $(grep -A1 -F 'KeyboardSession.shared.finishComposition();' <<< "$nine_digit_body" | grep -cF 'KeyboardSession.shared.insert(digit);') -ne 1 ]] \
  || ! grep -qF 'this.nineKeySwipes.clear();' <<< "$drop_nine_body" \
  || ! grep -qF 'nineKeyInput: this.symbols || shuangpin ? undefined : key.input,' "$view" \
  || ! grep -qF ': this.nineKeySwipeFace($$.nineKeyInput, this.eighteenSwipeFace($$.swipeLetters, $$.face)))' "$view"; then
  echo "Harmony must read touch_nine_key_swipe_digits through NineKeySwipePolicy, feed the pinyin nine-key letter keys' touches into NineKeySwipeGesture, swallow the click and the long press once a swipe took the touch, type the digit the way a held key's digit is typed, hide the corner digit while the face shows it, and drop the tracker when keys are replaced" >&2
  exit 1
fi
# 密码框和只收数字的编辑框里三种键面类型都退回标准底行（`TouchLayoutStylePolicy.fieldRowStyle`、`EditorPolicy.keepsStandardRows` 由测试覆盖）：会话从编辑框类型认出只收数字，视图每次接上编辑框都刷新这个状态，九键外框与 26/14/18 键底行都经 fieldRowStyle 取样式。
traits_body=$(sed -n '/^  private static traitsFor(pattern: number): EditorTraits {$/,/^  }$/p' "$session")
editor_face_body=$(sed -n '/^  private applyEditorFace(): void {$/,/^  }$/p' "$view")
nine_style_body=$(sed -n '/^  private nineKeyFrameStyle(): string {$/,/^  }$/p' "$view")
design_style_body=$(sed -n '/^  private designRowStyle(styled: StyledFace): string {$/,/^  }$/p' "$view")
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
# 长空格样式把工具条那一行一直留着（符 在上面），看的是这块键盘的中文键面和用户的 26 键样式，不看此刻的中/英：同一块键盘切到英文 26 键时也留着，否则工具栏隐藏时每次切中/英这一行都出没一次，面板高度跟着跳。判定本身在 `TouchLayoutStylePolicy.chineseTwentySixKeyboard`，没有 english 参数，由 touch-layout-styles.test.ts 逐个方案覆盖（中文 26 键停在英文 26 键上仍留着这一行）；视图是 ArkTS，这里钉住接线：工具条、隐藏工具栏时藏工具和冷启动记下的键面都读视图的 `chineseTwentySixKeyboard`，它交给上面那个判定、交过去的键盘状态（`touchKeyboard`）里没有 `this.english`；`chineseTwentySixFace` 是它再加上此刻是中文。
strip_body=$(sed -n '/^  private stripShown(): boolean {$/,/^  }$/p' "$view")
tool_shown_body=$(sed -n '/^  private toolbarToolShown(enabled: boolean): boolean {$/,/^  }$/p' "$view")
remember_face_body=$(sed -n '/^  private rememberPanelFace(): void {$/,/^  }$/p' "$view")
keyboard_body=$(sed -n '/^  private chineseTwentySixKeyboard(): boolean {$/,/^  }$/p' "$view")
toolbar_row_body=$(sed -n '/^  private longSpaceToolbarRow(): boolean {$/,/^  }$/p' "$view")
face_body=$(sed -n '/^  private chineseTwentySixFace(): boolean {$/,/^  }$/p' "$view")
touch_keyboard_body=$(sed -n '/^  private touchKeyboard(): TouchKeyboard {$/,/^  }$/p' "$view")
if ! grep -qF 'TouchToolbarPolicy.rowHidden(this.toolbarTools) && !this.longSpaceToolbarRow(),' <<< "$strip_body" \
  || ! grep -qF 'this.toolbarTools.hidden && this.longSpaceToolbarRow()' <<< "$tool_shown_body" \
  || ! grep -qF 'chineseTwentySix: this.chineseTwentySixKeyboard()' <<< "$remember_face_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.longSpaceActive(this.chineseTwentySixKeyboard(), this.twentySixKeyStyle,' <<< "$toolbar_row_body" \
  || [[ -z "$keyboard_body" ]] || grep -qF 'this.english' <<< "$keyboard_body" \
  || ! grep -qF 'TouchLayoutStylePolicy.chineseTwentySixKeyboard(this.touchKeyboard())' <<< "$keyboard_body" \
  || [[ -z "$touch_keyboard_body" ]] || grep -qF 'this.english' <<< "$touch_keyboard_body" \
  || ! grep -qF 'return !this.english && this.chineseTwentySixKeyboard();' <<< "$face_body"; then
  echo "The long-space toolbar row must follow the keyboard's Chinese 26-key face and the user's style, not the momentary 中/英 mode, so the panel height stays put across language toggles" >&2
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
# 词库快照队列只在键盘显示时查、没有待处理请求就停，重建会话的重试按 `IdleDictionaryRetryPolicy` 退避（规则由 keyboard-logic.test.ts 覆盖）。会话是 ArkTS，这里钉住接线：失焦和系统收起键盘（keyboardHide）时取消检查、停下没有进展的个人词库重试，显示时排一次检查并接着排个人词库，键盘没显示时不排、查到没有请求时不再自己续排，两个重试都按退避取间隔而不是固定两秒。
session_ets="$ets/keyboard/KeyboardSession.ets"
detach_body=$(sed -n '/^  detach(): void {$/,/^  }$/p' "$session_ets")
attach_body=$(sed -n '/^  attach(inputClient: inputMethodEngine.InputClient): void {$/,/^  }$/p' "$session_ets")
poll_body=$(sed -n '/^  private scheduleDictionarySnapshotPoll(): void {$/,/^  }$/p' "$session_ets")
drain_body=$(sed -n '/^  private schedulePersonalDictionaryDrain(): void {$/,/^  }$/p' "$session_ets")
hidden_body=$(sed -n '/^  keyboardHidden(): void {$/,/^  }$/p' "$session_ets")
pause_body=$(sed -n '/^  private pauseIdleDictionaryWork(): void {$/,/^  }$/p' "$session_ets")
if ! grep -qF 'this.pauseIdleDictionaryWork();' <<< "$detach_body" \
  || ! grep -qF 'this.pauseIdleDictionaryWork();' <<< "$hidden_body" \
  || ! grep -qF 'this.cancelDictionarySnapshotPoll();' <<< "$pause_body" \
  || ! grep -qF 'this.idleDictionaryVisible = false;' <<< "$pause_body" \
  || ! grep -qF 'this.idleDictionaryVisible = true;' <<< "$attach_body" \
  || ! grep -qF 'this.personalDictionaryStalls > 0' <<< "$pause_body" \
  || ! grep -qF 'clearTimeout(this.personalDictionaryTimer);' <<< "$pause_body" \
  || ! grep -qF 'this.scheduleDictionarySnapshotPoll();' <<< "$attach_body" \
  || ! grep -qF 'this.schedulePersonalDictionaryDrain();' <<< "$attach_body" \
  || ! grep -qF 'this.inputClient === undefined) return;' <<< "$poll_body" \
  || ! grep -qF '!this.idleDictionaryVisible' <<< "$poll_body" \
  || [[ $(grep -cF 'this.scheduleDictionarySnapshotPoll();' <<< "$poll_body") -ne 1 ]] \
  || ! grep -qF 'IdleDictionaryRetryPolicy.delay(this.snapshotQueueStalls)' <<< "$poll_body" \
  || ! grep -qF 'IdleDictionaryRetryPolicy.delay(this.personalDictionaryStalls)' <<< "$drain_body"; then
  echo "KeyboardSession must poll the dictionary snapshot queue only while shown and until it is empty, and back off both idle rebuild retries" >&2
  exit 1
fi
# 候选释义的词库连接和自定义译文按文件身份缓存在 UI 线程上（engine 的 `release_gloss_caches` 由 Rust 测试覆盖）。会话是 ArkTS，这里钉住接线：失焦和系统收起键盘时释放，隐藏的键盘不留着它们。
if ! grep -qF 'client.releaseGlossCaches();' <<< "$detach_body" \
  || ! grep -qF 'client.releaseGlossCaches();' <<< "$hidden_body"; then
  echo "KeyboardSession must release the UI thread's candidate gloss caches on focus loss and keyboardHide" >&2
  exit 1
fi
# 打字统计的提交和语音时长不在 UI 线程上写：都排进同一个 SequenceRunner（`writeTypingStatistics`）在 TaskPool 线程上调用原生库，原生层攒着的计数在键盘收起（dismiss 与系统的 keyboardHide）、编辑框失去焦点（detach）和扩展销毁（shutdown）时 flush；detach 等 flush 写完再把统计文件发布给设置应用。会话是 ArkTS，这里钉住接线。
record_body=$(sed -n '/^  private recordTypingStatistics(text: string, source: string): void {$/,/^  }$/p' "$session_ets")
voice_body=$(sed -n '/^  insertVoiceResult(text: string): void {$/,/^  }$/p' "$session_ets")
dismiss_body=$(sed -n '/^  dismiss(): void {$/,/^  }$/p' "$session_ets")
shutdown_body=$(sed -n '/^  shutdown(): void {$/,/^  }$/p' "$session_ets")
ability_ets="$ets/inputmethodextability/KeyboardExtensionAbility.ets"
if ! grep -qF 'this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request))' <<< "$record_body" \
  || grep -qF 'client.typingStatistics(' <<< "$record_body" \
  || ! grep -qF 'this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request))' <<< "$voice_body" \
  || grep -qF 'client.typingStatistics(' <<< "$voice_body" \
  || ! grep -qF 'this.flushTypingStatistics().then((): void => {' <<< "$detach_body" \
  || ! grep -qF 'SharedStateSync.keyboardPublish(mirror);' <<< "$detach_body" \
  || ! grep -qF 'this.flushTypingStatistics();' <<< "$dismiss_body" \
  || ! grep -qF 'this.flushTypingStatistics();' <<< "$shutdown_body" \
  || ! grep -qF "engine.on('keyboardHide', this.keyboardHidden);" "$ability_ets" \
  || ! grep -qF "engine.off('keyboardHide', this.keyboardHidden);" "$ability_ets"; then
  echo "KeyboardSession must write typing statistics on its TaskPool SequenceRunner and flush them on hide, focus loss and teardown" >&2
  exit 1
fi
# 完整体验模式下键盘失焦、获得焦点各发布一次共享状态，文件身份没变时不读文件、不再算指纹（缓存规则由 shared-state-mirror-policy.test.ts 的 `FingerprintCache` 覆盖）；每次获得焦点比偏好修订号时，文档身份没变就不经 NAPI 读整份文档；构建会话时处理词库快照队列、显示后查一次队列状态，队列空着都不扫词库算本机版本。这些都是 ArkTS，这里钉住接线。
mirror_ets="$ets/keyboard/SharedStateMirror.ets"
fingerprint_body=$(sed -n '/^  private static fingerprintOf(entry: MirrorEntry, path: string): string {$/,/^  }$/p' "$mirror_ets")
reload_body=$(sed -n '/^  reloadChangedPreferences(): void {$/,/^  }$/p' "$session_ets")
snapshot_body=$(sed -n '/^  private processDictionarySnapshot(options: Object, stateRoot: string): void {$/,/^  }$/p' "$session_ets")
needs_work_body=$(sed -n '/^  private dictionarySnapshotNeedsWork(): boolean {$/,/^  }$/p' "$session_ets")
if ! grep -qF 'SharedStateMirror.localFingerprints.fingerprint(' <<< "$fingerprint_body" \
  || ! grep -qF 'stat.mtimeNs' <<< "$fingerprint_body" \
  || ! grep -qF 'this.storedRevisionIfMoved(this.stateDirectory)' <<< "$reload_body" \
  || grep -qF 'this.storedRevision(' <<< "$reload_body" \
  || ! grep -qF 'skip_when_idle: true' <<< "$snapshot_body" \
  || ! grep -qF 'skip_when_idle: true' <<< "$needs_work_body"; then
  echo "Shared state fingerprints, the focus-time preference revision check and the snapshot queue pass and poll must skip work when nothing changed" >&2
  exit 1
fi
# 候选皮肤位图在候选条放手后停放一份，同一套皮肤不再每个词重新解码；停放规则由 candidate-canvas-lease.test.ts 覆盖。解码器和扩展能力是 ArkTS，这里钉住接线：解码器经 CandidateCanvasLeaseCache 取用和放手，键盘收起、停止输入和销毁时 trim。
frame_ets="$ets/keyboard/candidate/CandidateCanvasFrame.ets"
hidden_body=$(sed -n '/^  private readonly keyboardHidden: () => void = (): void => {$/,/^  };$/p' "$ability_ets")
stopped_body=$(sed -n '/^  private readonly stopped: () => void = (): void => {$/,/^  };$/p' "$ability_ets")
teardown_body=$(sed -n '/^  private teardown(): void {$/,/^  }$/p' "$ability_ets")
if ! grep -qF 'new CandidateCanvasLeaseCache<CanvasDecodeLease>(' "$frame_ets" \
  || grep -qF 'Map<string, CanvasDecodeLease>' "$frame_ets" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$hidden_body" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$stopped_body" \
  || ! grep -qF 'CandidateCanvasDecoder.trim();' <<< "$teardown_body"; then
  echo "Candidate canvas decodes must park in CandidateCanvasLeaseCache and be trimmed on keyboard hide, input stop and teardown" >&2
  exit 1
fi
# 键盘收起时停掉键盘内的语音识别（取舍规则由 kb-view.test.ts 的 `VoiceRecognitionPolicy.cancelsOnHide` 覆盖）。面板收起时视图不会销毁，会话和视图是 ArkTS，这里钉住接线：会话在系统的 keyboardHide 和 inputStop（detach）时都通知视图，视图按 cancelsOnHide 取消录音、取消还在走的空格长按计时，销毁时清掉回调。
view_ets="$ets/keyboard/KeyboardView.ets"
session_hidden_body=$(sed -n '/^  keyboardHidden(): void {$/,/^  }$/p' "$session_ets")
detach_body=$(sed -n '/^  detach(): void {$/,/^  }$/p' "$session_ets")
panel_hidden_body=$(sed -n '/^    KeyboardSession.shared.onPanelHidden = (): void => {$/,/^    };$/p' "$view_ets")
if ! grep -qF 'this.onPanelHidden();' <<< "$session_hidden_body" \
  || ! grep -qF 'this.onPanelHidden();' <<< "$detach_body" \
  || ! grep -qF 'VoiceRecognitionPolicy.cancelsOnHide(' <<< "$panel_hidden_body" \
  || ! grep -qF 'this.cancelVoice();' <<< "$panel_hidden_body" \
  || ! grep -qF 'this.cancelSpaceLongPress();' <<< "$panel_hidden_body" \
  || ! grep -qF 'KeyboardSession.shared.onPanelHidden = undefined;' "$view_ets"; then
  echo "Hiding the keyboard must cancel in-keyboard voice recognition on touch devices" >&2
  exit 1
fi
# 书写中每个移动事件只补画最后一段，不清屏重画整个字（墨色是否可补画由 keyboard-logic.test.ts 的 `HandwritingStrokePolicy.opaqueInk` 覆盖）。视图是 ArkTS，这里钉住接线：移动事件走 drawHandwritingSegment，补画不清屏、半透明墨色退回整幅重画，抬笔仍整幅重画让定稿画面与逐点重画一致。
touch_body=$(sed -n '/^  private handwritingTouch(event: TouchEvent): void {$/,/^  }$/p' "$view_ets")
segment_body=$(sed -n '/^  private drawHandwritingSegment(): void {$/,/^  }$/p' "$view_ets")
move_branch=$(sed -n '/^    if (event.type === TouchType.Move) {$/,/^    }$/p' <<< "$touch_body")
up_branch=$(sed -n '/^    if (event.type === TouchType.Up) {$/,/^    }$/p' <<< "$touch_body")
if ! grep -qF 'this.drawHandwritingSegment();' <<< "$move_branch" \
  || grep -qF 'this.redrawHandwriting();' <<< "$move_branch" \
  || ! grep -qF 'HandwritingStrokePolicy.opaqueInk(' <<< "$segment_body" \
  || grep -qF 'clearRect' <<< "$segment_body" \
  || ! grep -qF 'this.redrawHandwriting();' <<< "$up_branch"; then
  echo "Handwriting moves must draw only the new segment, and lifting the pen must redraw the whole pad" >&2
  exit 1
fi
# 登录面板和首次启动弹窗的协议链接都经 `SettingsBridge.openExternalUrl` 交给系统浏览器，桥接是同步的，页面拿不到失败。提示文字由 privacy-consent.test.ts 覆盖；桥接是 ArkTS，这里钉住接线：打开失败时弹出带地址的提示，而不是只写日志、看起来点了没反应。
open_url_body=$(sed -n '/^  openExternalUrl(url: string): void {$/,/^  }$/p' "$ets/pages/Settings.ets")
if ! grep -qF 'showToast({ message: TextPolicy.unopenedLinkNotice(url)' <<< "$open_url_body"; then
  echo "SettingsBridge.openExternalUrl must tell the user the address when the system cannot open a link" >&2
  exit 1
fi
# 社区投稿类功能的总闸（`CommunityContributionGate`，取自 Rust 的 `HostCapabilities.community_contribution`）：规则由 community-contribution-gate.test.ts 覆盖；设置桥是 ArkTS，node 下编不了，这里钉住接线：设置进程启动时读宿主能力，社区的发布、改分类、评分和举报在发出前问它；社区请求按 AI 生成类功能的总闸带上排除 AI 生成皮肤的标记（规则由 account-requests.test.ts 覆盖）。
for wiring in \
  "pages/Settings.ets:CommunityContributionGate.adopt(client.hostCapabilities(PLATFORM))" \
  "pages/Settings.ets:} else if (!CommunityContributionGate.allowsSettingsRequest(kind, payload)) {" \
  "account/HarmonyAccountCloudBridge.ets:communityRequest(this.directory, this.directory, action, !AiGenerationGate.enabled())"; do
  if ! grep -qF "${wiring#*:}" "$ets/${wiring%%:*}"; then
    echo "${wiring%%:*} must consult the community gates: missing \"${wiring#*:}\"" >&2
    exit 1
  fi
done
# 两道闸都要在分发社区请求之前判断：放在 `kind === 'community_*'` 分支之后就拦不住。
dispatch_body=$(sed -n '/^  startRequest(kind: string, id: number, payload: string): string {$/,/^  }$/p' \
  "$ets/pages/Settings.ets")
gate_line=$(grep -nF 'CommunityContributionGate.allowsSettingsRequest(kind, payload)' <<< "$dispatch_body" | head -1 | cut -d: -f1)
community_line=$(grep -nF "kind === 'community_skin'" <<< "$dispatch_body" | head -1 | cut -d: -f1)
if [[ -z "$gate_line" || -z "$community_line" || "$gate_line" -ge "$community_line" ]]; then
  echo "Settings.startRequest must ask CommunityContributionGate before it dispatches community requests" >&2
  exit 1
fi
# 屏蔽作者走自己的 `community_block` 请求（名单和隐私授权由 account-requests.test.ts、privacy-consent.test.ts 覆盖，总闸不拦它由 community-contribution-gate.test.ts 覆盖）；设置桥是 ArkTS，这里钉住接线：startRequest 分发它，账号桥经 `communityBlockRequest` 拼请求、先等会话采用再交给 `msime_client_community`。
block_body=$(sed -n '/^  async communityBlock(action: string): Promise<string> {$/,/^  }$/p' \
  "$ets/account/HarmonyAccountCloudBridge.ets")
if ! grep -qF "} else if (kind === 'community_block') {" <<< "$dispatch_body" \
  || ! grep -qF 'this.communityBlock(payload).then(deliver)' <<< "$dispatch_body" \
  || ! grep -qF 'return await this.accountBridge.communityBlock(actionDocument);' "$ets/pages/Settings.ets" \
  || ! grep -qF 'communityBlockRequest(this.directory, action);' <<< "$block_body" \
  || ! grep -qF 'await AccountSessionAdoption.beforeRequest();' <<< "$block_body" \
  || ! grep -qF 'return await client.community(plan.request);' <<< "$block_body"; then
  echo "Settings.startRequest must dispatch community_block to the account bridge, which builds it with communityBlockRequest and adopts the session before msime_client_community" >&2
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
# 编辑器在每次 insertTextSync 之后也回报一次选区变化。选区回报里直接清空英文建议，会让手机纠错模式下每一键刚出的候选马上消失，用户看不到候选，也无法点原文拒绝纠错。核对规则由 keyboard-logic.test.ts 的 `EnglishSuggestionPolicy.refresh` 覆盖；会话是 ArkTS，这里钉住接线：光标折叠时按当前词重新核对，选区回报从扩展传来是否折叠。
selection_body=$(sed -n '/^  editorSelectionChanged(collapsed: boolean = true): void {$/,/^  }$/p' \
  "$ets/keyboard/KeyboardSession.ets")
if ! grep -qF 'this.refreshEnglishSuggestions();' <<< "$selection_body" \
  || ! grep -qF 'if (collapsed) {' <<< "$selection_body" \
  || ! grep -qF 'KeyboardSession.shared.editorSelectionChanged(newBegin === newEnd);' \
    "$ets/inputmethodextability/KeyboardExtensionAbility.ets"; then
  echo "editorSelectionChanged must re-validate English suggestions against the current word instead of clearing them" >&2
  exit 1
fi
# 双拼触点（#10018）：带按下点的入口（`pressTouch` → `msime_client_touch_character`）只给触屏键盘上 26 键双拼的字母和 `;`。何时带由 keyboard-logic.test.ts 的 `TouchPointPolicy.usesTouch` 覆盖；视图是 ArkTS，node 下编不了，这里钉住接线：视图只在 tapLetter 里经 touchKey 调 pressTouch，touchKey 问 usesTouch 并带上 touchDesign()，按下点只在 touchDesign() 时记；press 本身永远不带按下点（硬件键盘经 HardwareKeyDispatch 调它）；触点入口和几何注册只出现在这两个文件里。
view="$ets/keyboard/KeyboardView.ets"
session="$ets/keyboard/KeyboardSession.ets"
tap_letter_body=$(sed -n '/^  private tapLetter(letter: string, touch: TouchPoint | null): void {$/,/^  }$/p' "$view")
touch_key_body=$(sed -n '/^  private touchKey(touch: TouchPoint | null, shifted: boolean): TouchKey | null {$/,/^  }$/p' "$view")
record_touch_body=$(sed -n '/^  private recordLetterTouch(letter: string | undefined, event: TouchEvent): void {$/,/^  }$/p' "$view")
press_body=$(sed -n '/^  press(ascii: number, shift: boolean): boolean {$/,/^  }$/p' "$session")
touch_callers=$(grep -rlF 'pressTouch(' "$ets" | sed "s|^$ets/||" | sort | tr '\n' ' ')
touch_natives=$(grep -rlE 'client\.(touchCharacter|setKeyGeometry)\(' "$ets" | sed "s|^$ets/||" | sort | tr '\n' ' ')
if [[ $(grep -cF 'KeyboardSession.shared.pressTouch(' "$view") -ne 1 ]] \
  || ! grep -qF 'KeyboardSession.shared.pressTouch(' <<< "$tap_letter_body" \
  || ! grep -qF 'this.touchKey(touch, shifted)' <<< "$tap_letter_body" \
  || ! grep -qF 'TouchPointPolicy.usesTouch(' <<< "$touch_key_body" \
  || ! grep -qF 'touchDesign: this.touchDesign(),' <<< "$touch_key_body" \
  || ! grep -qF '!this.touchDesign()' <<< "$record_touch_body" \
  || ! grep -qF 'return this.pressKey(ascii, shift, null);' <<< "$press_body" \
  || [[ "$touch_callers" != "keyboard/KeyboardSession.ets keyboard/KeyboardView.ets " ]] \
  || [[ "$touch_natives" != "keyboard/KeyboardSession.ets " ]]; then
  echo "Only the touch keyboard's 26-key shuangpin letters may reach pressTouch / msime_client_touch_character, through KeyboardView.tapLetter and touchKey (TouchPointPolicy.usesTouch with touchDesign()); press() must stay pointless" >&2
  exit 1
fi
# 双拼触点的按下点和几何要来自同一帧的布局（#10018 评审）。换算和几何本身由 keyboard-logic.test.ts 覆盖（`PendingTouches` 按键的当前位置重建按下点、`geometryLayout` 每键现算），这里钉住视图那一侧不能丢的接线：每个字母键的触摸都经 recordLetterTouch，按下时按被按的键（`event.target.area`）记、取消时丢掉、键面替换时清空；`;` 的位置由 recordGlideKey 记下；两处字母键的点击都经 takeTouch 按这个键现在的位置取按下点；touchLayout 每键现算，带上 `;` 是否画着，不缓存（缓存要靠作废标记，漏一处就一直报旧的几何代次）。
glide_key_body=$(sed -n '/^  private recordGlideKey(letter: string | undefined, area: Area): void {$/,/^  }$/p' "$view")
take_touch_body=$(sed -n '/^  private takeTouch(letter: string): TouchPoint | null {$/,/^  }$/p' "$view")
touch_layout_body=$(sed -n '/^  private touchLayout(): string | null {$/,/^  }$/p' "$view")
drop_gestures_body=$(sed -n '/^  private dropLetterGestures(): void {$/,/^  }$/p' "$view")
if ! grep -qF 'this.recordLetterTouch($$.letter, event);' "$view" \
  || ! grep -qF 'event.target.area.globalPosition.x' <<< "$record_touch_body" \
  || ! grep -qF 'this.pendingTouches.down(letter, touch.windowX, touch.windowY, key, Date.now());' <<< "$record_touch_body" \
  || ! grep -qF 'this.pendingTouches.cancel(letter);' <<< "$(grep -A1 -F 'event.type === TouchType.Cancel' <<< "$record_touch_body")" \
  || ! grep -qF 'this.pendingTouches.clear();' <<< "$drop_gestures_body" \
  || ! grep -qF "letter !== ';'" <<< "$glide_key_body" \
  || ! grep -qF 'this.semicolonKey = rect;' <<< "$glide_key_body" \
  || ! grep -qF 'TouchPointPolicy.keyRect(this.glideKeys, this.semicolonKey, letter), this.glideOriginX, this.glideOriginY);' <<< "$take_touch_body" \
  || [[ $(grep -cF 'action: () => this.tapLetter(letter, this.takeTouch(letter))' "$view") -ne 2 ]] \
  || [[ $(grep -cF 'this.pendingTouches.take(' "$view") -ne 1 ]] \
  || ! grep -qF 'return TouchPointPolicy.geometryLayout(this.glideKeys, this.secondRowSemicolon(), this.semicolonKey,' <<< "$touch_layout_body" \
  || [[ $(grep -cF 'TouchPointPolicy.geometryLayout(' "$view") -ne 1 ]] \
  || grep -qE 'touchLayout(Cache|Dirty)' "$view"; then
  echo "KeyboardView must record each letter's touch-down against the key it pressed, drop it on Cancel and when the face is replaced, record the semicolon key, take it through takeTouch at the key's current rect, and compute touchLayout on every key without a cache" >&2
  exit 1
fi
# 硬件键盘快捷键 LOCK 锁定录音后，松开按键不再结束录音，会话按点按开始对待：面板给「说完了」「取消」，流式识别停顿自动结束。按钮文字由 voice-listening-policy.test.ts 的 `VoiceListeningPolicy.primaryLabel`、`cancelLabel` 覆盖；视图是 ArkTS，这里钉住接线：LOCK 分支清掉 `voiceHeldByKey`，面板按钮从策略取。
lock_branch=$(sed -n '/^      if (action === VoiceHotkeyAction.LOCK) {$/,/^      }$/p' "$view")
if ! grep -qF 'this.voiceHeldByKey = false;' <<< "$lock_branch" \
  || ! grep -qF 'primaryLabel: VoiceListeningPolicy.primaryLabel(this.voiceListeningSession()),' "$view" \
  || ! grep -qF 'cancelLabel: VoiceListeningPolicy.cancelLabel(this.voiceListeningSession()),' "$view"; then
  echo "A hotkey LOCK must turn the held session into a tap session (clear voiceHeldByKey), and the listening panel must take its button labels from VoiceListeningPolicy" >&2
  exit 1
fi
# 双拼纠错坐上首选的显式标记（#10018）：引擎只在那一行带 `confident_correction: true`。读法、读屏后缀、标记色以及展开面板选哪套令牌由 keyboard-logic.test.ts 的 `CorrectionMarkPolicy` 覆盖；会话和视图是 ArkTS，这里钉住接线和每处传进去的令牌：会话把引擎行变成候选栏、展开面板的三处都经 `CorrectionMarkPolicy.confident` 读这个键，翻译页的行一律不标；视图在候选窗、触屏胶囊上的候选字经 `candidateWordColor`、注释释义经 `candidateDetailTint` 换成标记色（两处里不再有没经过它的次要色），展开面板格子经 `CorrectionMarkPolicy.cellTextColor` 用 `expandedSurfaceColors`；两套令牌逐项钉住，交换高亮字色与强调色、换掉画布判断都会失败；两处读屏朗读都接上「已纠正」后缀。去掉空白和换行后比较整段调用，参数换行不影响。
view_body() { sed -n "/^  $1\$/,/^  }\$/p" "$view" | tr -d ' \n'; }
label_body=$(sed -n '/^  private candidateLabel(candidate: CandidateEntry, index: number): string {$/,/^  }$/p' "$view")
expanded_label_body=$(sed -n '/^  private expandedLabel(index: number): string {$/,/^  }$/p' "$view")
primary_body=$(view_body 'candidatePrimaryLine(candidate: CandidateEntry) {')
cell_body=$(view_body 'candidateCell(candidate: CandidateEntry, index: number) {')
chip_body=$(view_body 'candidateChip(candidate: CandidateEntry, index: number) {')
grid_body=$(view_body 'expandedGrid() {')
surface_body=$(view_body 'private candidateSurfaceColors(): CorrectionSurfaceColors {')
word_body=$(view_body 'private candidateWordColor(candidate: CandidateEntry): string {')
detail_body=$(view_body 'private candidateDetailTint(candidate: CandidateEntry, base: string): string {')
expanded_surface_body=$(view_body 'private expandedSurfaceColors(): CorrectionSurfaceColors {')
if [[ $(grep -cF 'confidentCorrection: CorrectionMarkPolicy.confident(candidate.confident_correction)' "$session") -ne 3 ]] \
  || ! grep -qF 'fixedPosition: 0, source: 0, translation: null, confidentCorrection: false' "$session" \
  || [[ $(grep -oF '.fontColor(this.candidateWordColor(candidate))' <<< "$primary_body" | wc -l) -ne 1 ]] \
  || [[ $(grep -oF '.fontColor(this.candidateDetailTint(candidate,this.candidateDetailColor()))' <<< "$primary_body" | wc -l) -ne 6 ]] \
  || grep -qF '.fontColor(this.candidateDetailColor())' <<< "$primary_body" \
  || ! grep -qF '.fontColor(this.candidateDetailTint(candidate,this.candidateDetailColor()))' <<< "$cell_body" \
  || grep -qF '.fontColor(this.candidateDetailColor())' <<< "$cell_body" \
  || [[ $(grep -oF '.fontColor(this.candidateWordColor(candidate))' <<< "$chip_body" | wc -l) -ne 1 ]] \
  || [[ $(grep -oF '.fontColor(this.candidateDetailTint(candidate,this.skin.secondary))' <<< "$chip_body" | wc -l) -ne 4 ]] \
  || grep -qF '.fontColor(this.skin.secondary)' <<< "$chip_body" \
  || ! grep -qF '.fontColor(CorrectionMarkPolicy.cellTextColor(this.expanded[cell.index].confidentCorrection,this.expandedHighlighted(cell.index),this.expandedSurfaceColors()))' <<< "$grid_body" \
  || ! grep -qF 'text:this.candidateTextColor(),highlightedText:this.candidateSelectedTextColor(),accent:this.candidateAccentColor()' <<< "$surface_body" \
  || ! grep -qF 'if(CandidatePresentationPolicy.aiPlaceholder(candidate.source))returnthis.candidateDetailColor();returnCorrectionMarkPolicy.tint(candidate.confidentCorrection,candidate.highlighted,CandidateSkinPolicy.rowTextColor(candidate.highlighted,candidate.fixedPosition>0,this.candidateTextColor(),this.candidateSelectedTextColor()),this.candidateSurfaceColors());' <<< "$word_body" \
  || ! grep -qF 'returnCorrectionMarkPolicy.tint(candidate.confidentCorrection,candidate.highlighted,base,this.candidateSurfaceColors());' <<< "$detail_body" \
  || ! grep -qF 'text:this.skin.keyForeground,highlightedText:this.skin.toggleForeground,accent:this.skin.accent' <<< "$expanded_surface_body" \
  || ! grep -qF 'returnCorrectionMarkPolicy.expandedSurface(this.settings.candidateCanvas!==null,this.candidateSurfaceColors(),keyboard);' <<< "$expanded_surface_body" \
  || grep -qF 'correctionDecoration' "$view" \
  || ! grep -qF 'CorrectionMarkPolicy.accessibilitySuffix(candidate.confidentCorrection)' <<< "$label_body" \
  || ! grep -qF 'CorrectionMarkPolicy.accessibilitySuffix(this.expanded[index].confidentCorrection)' <<< "$expanded_label_body"; then
  echo "KeyboardSession must read confident_correction through CorrectionMarkPolicy for the strip and the expanded panel, and KeyboardView must draw those rows' word and every detail text in the mark colour from the right surface tokens in the candidate window, the touch chip and the expanded grid, and append the screen reader suffix to both labels" >&2
  exit 1
fi
# 候选英文音标只占了预留的那一行（`translationRows()` 为 0）时下面没有释义行：2in1 横排的 `candidatePrimaryLine` 和触屏胶囊的 `chipAnnotationBeside` 都要把译文画回词旁边，否则打开音标后译文就不见了。规则和高度由 keyboard-logic.test.ts、kb-view.test.ts 覆盖，视图是 ArkTS，这里钉住接线。
beside_body=$(view_body 'private chipAnnotationBeside(candidate: CandidateEntry): boolean {')
if ! grep -qF 'if(candidate.annotation.length>0&&(!this.candidateUsesGlossLine()||KeyboardSession.shared.translationRows()===0)){' <<< "$primary_body" \
  || ! grep -qF '(!candidate.annotationIsTranslation||KeyboardSession.shared.translationRows()===0)' <<< "$beside_body" \
  || ! grep -qF 'if(KeyboardSession.shared.translationRows()>0){' <<< "$cell_body"; then
  echo "With only the pronunciation row reserved, the 2in1 candidate cell and the touch chip must draw the translation beside the word rather than on a gloss line that is not there" >&2
  exit 1
fi
# 展开候选网格分批建格子：规则由 keyboard-logic.test.ts 的 `ExpandedCandidateBatchPolicy` 覆盖；视图是 ArkTS，node 下编不了，这里钉住接线：网格只画已建到 expandedBuilt 的格子，重取候选按 initialCount 定已建数（同一代比 candidateListIdentity），滚动、视口和内容尺寸变化都去问要不要追加，关面板时清零。
refresh_body=$(view_body 'private refreshExpanded(): void {')
indices_body=$(view_body 'private expandedIndices(): number\[\] {')
if ! grep -qF 'constbuilt:number=Math.min(this.expandedBuilt,this.expanded.length);' <<< "$indices_body" \
  || ! grep -qF 'this.expandedListIdentity=KeyboardSession.shared.candidateListIdentity();' <<< "$refresh_body" \
  || ! grep -qF 'this.expandedBuilt=ExpandedCandidateBatchPolicy.initialCount(this.expanded.length,this.expandedBuilt,sameGeneration);' <<< "$refresh_body" \
  || ! grep -qF 'Scroll(this.expandedScroller){' <<< "$grid_body" \
  || [[ $(grep -oF 'this.appendExpandedIfNeeded()' <<< "$grid_body" | wc -l) -ne 3 ]] \
  || ! grep -qF 'this.expandedBuilt = 0;' "$view"; then
  echo "The expanded grid must draw only the cells built so far, rebuild them through ExpandedCandidateBatchPolicy when the list is fetched again, append as it scrolls or resizes and reset when the panel closes" >&2
  exit 1
fi
# The expanded grid follows the candidate font size and 候选栏大小 (#10400): the sizes come from keyboard-logic.test.ts's CandidateScalePolicy.expandedFontSize / expandedCellHeight group; the view is ArkTS and does not compile under node, so this pins the wiring: cells draw and measure their span at the scaled font size, and take the scaled height.
span_body=$(view_body 'private expandedSpan(index: number): number {')
if ! grep -qF '.height(this.expandedCellHeight())' <<< "$grid_body" \
  || ! grep -qF '.fontSize(this.expandedFontSize())' <<< "$grid_body" \
  || ! grep -qF 'ExpandedCandidateLayout.width(this.expanded[index].text,this.expandedFontSize(),' <<< "$span_body" \
  || ! grep -qF 'return CandidateScalePolicy.expandedFontSize(EXPANDED_FONT_SIZE, this.settings.candidateFontSize);' "$view" \
  || ! grep -qF 'return CandidateScalePolicy.expandedCellHeight(EXPANDED_CELL_HEIGHT_VP, this.settings.candidateFontSize,' "$view"; then
  echo "The expanded grid must size its text, span measurement and cell height through CandidateScalePolicy from the candidate font size and 候选栏大小" >&2
  exit 1
fi
# 大屏单手的侧边数字键盘（#10203）：宽度、取值和显示条件由 keyboard-logic.test.ts 的 `OneHandedPolicy` 覆盖；视图、会话和设置桥是 ArkTS，node 下编不了，这里钉住接线：会话按 sidePanel 读共享偏好，视图随设置重读、转屏时按按键这一行的宽度重算，handedKeys 只在 sidePadShown 时画面板，数字和小数点走字面上屏，设置桥按设备形态（含可折叠）报告能力位。
side_pad_body=$(awk '/^  oneHandSidePad\(\) \{/,/^  \}$/' "$keyboard_view")
side_pad_key_body=$(awk '/^  private sidePadDigitKey\(digit: string\): KeyOptions \{/,/^  \}$/' "$keyboard_view")
if ! grep -qF 'oneHandedSidePanel: OneHandedPolicy.sidePanel(preferences.touch_one_handed_side_panel)' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'this.oneHandedSidePanel = this.settings.oneHandedSidePanel;' "$keyboard_view" \
  || ! grep -qF 'widthVp - 2 * KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP, largeScreen);' "$keyboard_view" \
  || ! grep -qF 'return OneHandedPolicy.sidePadShown(this.oneHandedSidePanel, this.oneHandActive(), this.oneHandSidePadVp,' "$keyboard_view" \
  || [[ $(grep -cF 'if (this.oneHandSidePadShown() && ' "$keyboard_view") -ne 2 ]] \
  || ! grep -qF ".id('oneHandSidePad')" <<< "$side_pad_body" \
  || ! grep -qF "this.insertNumberPadLiteral('.');" <<< "$side_pad_body" \
  || ! grep -qF 'this.deleteKey(' <<< "$side_pad_body" \
  || ! grep -qF 'this.insertNumberPadLiteral(digit);' <<< "$side_pad_key_body" \
  || ! grep -qF 'KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(digit));' <<< "$side_pad_key_body" \
  || ! grep -qF "value['touch_one_handed_side_panel'] = projection.oneHandedSidePanel;" \
    "$repo_root/platforms/harmony/entry/src/main/ets/pages/Settings.ets" \
  || ! grep -qF 'SettingsFormFactorCapabilities.resolve(deviceInfo.deviceType, this.foldable());' \
    "$repo_root/platforms/harmony/entry/src/main/ets/pages/Settings.ets"; then
  echo "Harmony must draw the one-handed side number pad from touch_one_handed_side_panel only when OneHandedPolicy.sidePadShown allows, type its digits literally, and report the capability by form factor" >&2
  exit 1
fi
# 大屏分离式键盘（#10224）：启用条件和几何由 split-keyboard.test.ts 的 `SplitKeyboardPolicy` 覆盖；视图、会话和设置桥是 ArkTS，node 下编不了，这里钉住接线：会话按 SplitKeyboardPolicy.settings 读四项共享偏好，视图随设置重读、转屏时记下屏幕宽高，分离时单手让位，字母三排、数字行、补充行、123 / #+= 层和两种底行都插入空隙，重复的 G、V 不记滑行键位，设置桥按设备形态报告能力位。
split_letter_body=$(awk '/^  splitLetterFace\(\) \{/,/^  \}$/' "$keyboard_view")
split_action_body=$(awk '/^  designActionRow\(\) \{/,/^  \}$/' "$keyboard_view")
if ! grep -qF 'splitKeyboard: SplitKeyboardPolicy.settings(preferences.touch_keyboard_split,' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets" \
  || ! grep -qF 'this.splitKeyboard = this.settings.splitKeyboard;' "$keyboard_view" \
  || ! grep -qF 'this.screenWidthVp = widthVp;' "$keyboard_view" \
  || ! grep -qF 'return OneHandedPolicy.active(this.feedback.oneHanded, KeyboardFormFactor.isDesktop()) && !this.splitDrawn() && !this.floatingDrawn();' "$keyboard_view" \
  || ! grep -qF '} else if (this.splitDrawn()) {' "$keyboard_view" \
  || ! grep -qF 'this.splitGap(this.splitLetterRow(row).gapShare)' <<< "$split_letter_body" \
  || ! grep -qF 'this.duplicateLetterKey(' <<< "$split_letter_body" \
  || ! grep -qF 'this.splitGap(this.designRowSplitSpace().gap)' <<< "$split_action_body" \
  || [[ $(grep -cF 'this.splitGap(' "$keyboard_view") -ne 8 ]] \
  || ! grep -qF 'if ($$.duplicate !== true) {' "$keyboard_view" \
  || ! grep -qF "value['touch_keyboard_split'] = projection.splitKeyboard;" \
    "$repo_root/platforms/harmony/entry/src/main/ets/pages/Settings.ets"; then
  echo "Harmony must draw the split keyboard from the touch_keyboard_split preferences only when SplitKeyboardPolicy.drawn allows, put a dead gap into every split row, give way to one-handed mode, and report the capability by form factor" >&2
  exit 1
fi
# 「防止截屏」（#10194）：切换规则和镜像条目由 screen-capture-protection.test.ts 覆盖；窗口、面板和认证是 ArkTS，node 下编不了，这里钉住接线：声明 PRIVACY_WINDOW；软键盘（或候选窗）、浮动工具条、输入模式提示三块面板新建时各设一次隐私模式，编辑框获得焦点导入镜像后（含面板建好前就来的那一个）再按开关重设，值没变的面板不再调用 `setPrivacyMode`；设置应用主窗口在载入页面前设上，设置页写入后当场改窗口并立即发布给键盘（发布失败只记日志，仍回答已保存），写入前先过切换规则；没有锁屏密码时认证不可用，不能放行。
capture_ability="$repo_root/platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
capture_settings="$repo_root/platforms/harmony/entry/src/main/ets/pages/Settings.ets"
capture_auth="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/settings/ScreenCaptureAuthentication.ets"
capture_protection="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/settings/ScreenCaptureProtection.ets"
capture_bridge=$(sed -n '/^  private async screenCaptureProtectionSettings(payload: string): Promise<string> {$/,/^  }$/p' "$capture_settings")
capture_available=$(sed -n '/^  static available(): boolean {$/,/^  }$/p' "$capture_auth")
if ! grep -qF '"name": "ohos.permission.PRIVACY_WINDOW"' "$repo_root/platforms/harmony/entry/src/main/module.json5" \
  || ! grep -qF "ScreenCaptureProtection.protectPanel(toolbar, this.screenCaptureProtected(), 'toolbar');" "$capture_ability" \
  || ! grep -qF "ScreenCaptureProtection.protectPanel(hud, this.screenCaptureProtected(), 'mode badge');" "$capture_ability" \
  || ! grep -qF 'ScreenCaptureProtection.protectPanel(panel, this.screenCaptureProtected(),' "$capture_ability" \
  || [[ $(grep -cF 'this.applyScreenCaptureProtection();' "$capture_ability") -ne 2 ]] \
  || ! grep -qF 'if (ScreenCaptureProtection.panelModes.get(panel) === enabled) return;' "$capture_protection" \
  || ! grep -qF 'ScreenCaptureProtection.panelModes.set(panel, enabled);' "$capture_protection" \
  || ! grep -qF 'ScreenCaptureProtection.protectWindow(stage.getMainWindowSync(), true);' \
    "$repo_root/platforms/harmony/entry/src/main/ets/entryability/EntryAbility.ets" \
  || ! grep -qF "kind === 'screen_capture_protection_settings'" "$capture_settings" \
  || ! grep -qF 'ScreenCaptureProtectionPolicy.decide(current, next,' <<< "$capture_bridge" \
  || ! grep -qF 'ScreenCaptureProtection.protectLastWindow(context, saved.enabled);' <<< "$capture_bridge" \
  || ! grep -qF 'this.publishShared();' <<< "$capture_bridge" \
  || ! grep -qF 'screen capture protection not published to the keyboard' <<< "$capture_bridge" \
  || ! grep -qF 'return false;' <<< "$capture_available" \
  || [[ $(grep -cF 'return true;' <<< "$capture_available") -ne 1 ]]; then
  echo "Screen capture protection must declare PRIVACY_WINDOW, set privacy mode on all three keyboard panels and on the settings window, re-apply it after each focus import, skip panels whose mode has not changed, publish every change to the keyboard without reporting a saved change as failed when that publish fails, and refuse when no system authentication is set up" >&2
  exit 1
fi
# 浮动键盘（#10225）：大小、位置钳制和转屏由 floating-keyboard.test.ts 覆盖。扩展能力和视图是 ArkTS，这里钉住接线：`changeFlag` 要求先隐藏面板，所以只在 `applyFloatingSetting` 里、`await panel.hide()` 之后调用，换完无论成败都显示回来；`moveTo` 对 `FLG_FIXED` 不可用，停靠路径只 resize；转屏经 `display.on('change')` 重新摆浮动面板，销毁时退订；浮动时单手模式不生效，分离式键盘也不画（`SplitKeyboardPolicy.drawn` 收到浮动状态，规则由 split-keyboard.test.ts 覆盖），单手磁贴在分离和浮动时都置灰。
floating_ability="$repo_root/platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
floating_apply=$(sed -n '/^  private async applyFloatingSetting(panel: inputMethodEngine.Panel): Promise<void> {$/,/^  }$/p' "$floating_ability")
floating_fit=$(sed -n '/^  private async fitTouchPanel(panel: inputMethodEngine.Panel): Promise<void> {$/,/^  }$/p' "$floating_ability")
floating_fixed=$(sed -n '/^    if (!this.floating) {$/,/^    }$/p' <<< "$floating_fit")
if [[ $(grep -cF 'panel.changeFlag(' "$floating_ability") -ne 1 ]] \
  || ! grep -qF 'await panel.hide();' <<< "$floating_apply" \
  || ! grep -qF 'panel.changeFlag(wanted ? inputMethodEngine.PanelFlag.FLG_FLOATING : inputMethodEngine.PanelFlag.FLG_FIXED);' <<< "$floating_apply" \
  || ! grep -qF '} finally {' <<< "$floating_apply" \
  || ! grep -qF 'await panel.show();' <<< "$floating_apply" \
  || grep -qF 'moveTo' <<< "$floating_fixed" \
  || ! grep -qF 'FloatingKeyboardPolicy.position(this.floatingFraction[0], this.floatingFraction[1],' <<< "$floating_fit" \
  || ! grep -qF "display.on('change', this.displayChanged);" "$floating_ability" \
  || ! grep -qF "display.off('change', this.displayChanged);" "$floating_ability" \
  || ! grep -qF 'KeyboardSession.shared.saveFloatingPosition(this.floatingFraction[0], this.floatingFraction[1]);' "$floating_ability" \
  || ! grep -qF 'return OneHandedPolicy.active(this.feedback.oneHanded, KeyboardFormFactor.isDesktop()) && !this.splitDrawn() && !this.floatingDrawn();' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets" \
  || ! grep -qF 'KeyboardFormFactor.isDesktop(), this.splitsFace(), this.floatingDrawn());' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets" \
  || ! grep -qF 'return this.touchDesign() && !this.splitDrawn() && !this.floatingDrawn();' \
    "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"; then
  echo "The floating keyboard must change the panel flag only while the panel is hidden and show it again afterwards, never move a fixed panel, refit the floating panel when the display changes, save the dragged position, suspend one-handed mode while floating or split, and stop drawing the split keyboard while floating" >&2
  exit 1
fi
# 键盘调整遮罩（取代高度条）：四项的范围、生效条件、外框换算、各边拖动约束、存放和面板高度含抬高由 keyboard-adjust.test.ts 覆盖。视图、会话和扩展能力是 ArkTS，这里钉住接线：离开键盘调整的唯一出口 `show` 在离开时保存（没有恢复快照的「取消」路径），收起键盘、换输入框、切应用（`onPanelHidden`）都经它离开；「确认」也走它；设置页中途重载不盖掉预览；根节点按 `adjustFrame` 让出两侧和底部，面板高度和冷启动第一帧都算进抬高；旧的高度条不再存在。
adjust_view="$repo_root/platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
adjust_show=$(sed -n '/^  private show(surface: string): void {$/,/^  }$/p' "$adjust_view")
adjust_hidden=$(sed -n '/^    KeyboardSession.shared.onPanelHidden = (): void => {$/,/^    };$/p' "$adjust_view")
adjust_click=$(sed -n '/^  private adjustButtonClick(kind: string): void {$/,/^  }$/p' "$adjust_view")
adjust_settings=$(sed -n '/^  private applySettings(): void {$/,/^  }$/p' "$adjust_view")
adjust_resize=$(sed -n '/^  private resizeTouchPanel(): void {$/,/^  }$/p' "$adjust_view")
adjust_sync=$(sed -n '/^  private syncStripHeight(): void {$/,/^  }$/p' "$adjust_view")
adjust_root=$(sed -n '/^  keyboard() {$/,/^  }$/p' "$adjust_view")
if [[ -z "$adjust_show" ]] \
  || ! grep -qF 'if (this.surface === SURFACE_GEOMETRY && surface !== SURFACE_GEOMETRY) {' <<< "$adjust_show" \
  || ! grep -qF 'this.saveAdjust();' <<< "$adjust_show" \
  || ! grep -qF 'if (this.surface === SURFACE_GEOMETRY) {' <<< "$adjust_hidden" \
  || ! grep -qF 'this.show(SURFACE_NONE);' <<< "$adjust_hidden" \
  || ! grep -qF 'this.previewAdjust(KeyboardAdjustPolicy.DEFAULTS);' <<< "$adjust_click" \
  || ! grep -qF 'this.show(SURFACE_NONE);' <<< "$adjust_click" \
  || grep -qF 'saveAdjust' <<< "$adjust_click" \
  || ! grep -qF 'if (this.surface !== SURFACE_GEOMETRY) {' <<< "$adjust_settings" \
  || ! grep -qF 'this.readAdjustValues();' <<< "$adjust_settings" \
  || grep -qF 'this.geometryHeight = this.settings.heightAdjustment;' <<< "$adjust_settings" \
  || ! grep -qF 'this.extraRowsWere, this.settings.candidateScalePercent, this.liftWas));' <<< "$adjust_resize" \
  || ! grep -qF 'this.bottomLiftVp() !== this.liftWas' <<< "$adjust_sync" \
  || ! grep -qF 'bottom: KeyboardMetrics.TOUCH_ROOT_BOTTOM_PADDING_VP + this.adjustFrame().lift' <<< "$adjust_root" \
  || ! grep -qF 'left: KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP + this.adjustFrame().left,' <<< "$adjust_root" \
  || ! grep -qF 'this.liftWas = KeyboardSession.shared.firstTouchLift();' "$adjust_view" \
  || grep -qF 'HeightBar' "$adjust_view" \
  || [[ -e "$repo_root/platforms/harmony/entry/src/main/ets/keyboard/faces/HeightBar.ets" ]]; then
  echo "The keyboard adjust overlay must save whenever it is left (confirm, another panel, hiding the keyboard, a new field, another app) with no restoring cancel path, keep its preview across a settings reload, inset the root by the adjust frame, count the bottom lift in every panel height, and replace HeightBar entirely" >&2
  exit 1
fi
"$tsc" --project "$here/tsconfig.json"
node "$here/idle-dictionary-lifecycle.test.cjs"
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
