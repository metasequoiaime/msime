import { HarmonyMicrophonePermission } from './input/HarmonyMicrophonePermission';
import { CandidateDecorationView } from './candidate/CandidateDecorationView';
import { CandidateCanvasFrame } from './candidate/CandidateCanvasFrame';
import {
  KeyboardSession, KeyboardPreferences, KeyboardState, CandidateEntry, ExpandedCandidate,
  EmojiPage, EmojiRow, ClipboardHistoryMutation
} from './KeyboardSession';
import { KeyboardScheme, KeyGrid, SchemeDefinition } from './KeyboardScheme';
import { KeyIdPolicy } from './KeyIdPolicy';
import { SettingsPageLink } from './SettingsPageLink';
import { VoiceHotkeyAction } from './input/VoiceHotkeyPolicy';
import { BusinessErrorPolicy } from './BusinessErrorPolicy';
import { VoiceRecordingBehaviourPolicy } from './input/VoiceRecordingBehaviourPolicy';
import { KeyboardSkin } from './skin/KeyboardSkin';
import { CandidateColors, GlobalTheme } from './skin/GlobalTheme';
import { EnglishLetterCaseState, LetterCaseMode } from './input/EnglishLetterCaseState';
import { LetterKeyFacePolicy } from './input/LetterKeyFacePolicy';
import { ChineseHelpcodePolicy } from './input/ChineseHelpcodePolicy';
import { KeyAccessibilityPolicy } from './input/KeyAccessibilityPolicy';
import { CandidateGlossPolicy, CandidatePronunciationPolicy } from './candidate/CandidateGlossPolicy';
import { CorrectionMarkPolicy, CorrectionSurfaceColors } from './candidate/CorrectionMarkPolicy';
import { CandidatePresentationPolicy } from './candidate/CandidatePresentationPolicy';
import { ExpandedCandidateCell, ExpandedCandidateLayout } from './candidate/ExpandedCandidateLayout';
import { CandidateScalePolicy } from './candidate/CandidateScalePolicy';
import { ExpandedCandidateBatchPolicy } from './candidate/ExpandedCandidateBatchPolicy';
import { CandidatePanelSelectionPolicy } from './candidate/CandidatePanelSelectionPolicy';
import { ShuangpinKeyHintPolicy } from './input/ShuangpinKeyHintPolicy';
import { ReturnDispatch, ReturnKeyAction } from './input/ReturnKeyAction';
import { InputDiagnosticPolicy } from './input/InputDiagnosticPolicy';
import { LocalInputMode, LocalInputModeDefinition } from './input/LocalInputMode';
import { KeyboardFeedback, FeedbackSettings, HapticStrength } from './KeyboardFeedback';
import { QuickPunctuationPolicy, PunctuationEntry } from './input/QuickPunctuationPolicy';
import {
  JapaneseNineKeyLayout, JapaneseKey, VariantGroup, DIRECTION_CENTRE
} from './input/JapaneseNineKeyLayout';
import { JapaneseNineKeyActions } from './input/JapaneseNineKeyActions';
import { JapaneseVariantPolicy } from './input/JapaneseVariantPolicy';
import { DubeolsikLayout } from './input/DubeolsikLayout';
import { ZhuyinLayout } from './input/ZhuyinLayout';
import { StrokeKey, StrokeLayout } from './input/StrokeLayout';
import { SchemeCompositionPolicy } from './input/SchemeCompositionPolicy';
import { SchemeTraits } from './SchemeTraits';
import { HardwareKeyRouter } from './HardwareKeyRouter';
import { KoreanCompositionPolicy } from './input/KoreanCompositionPolicy';
import { CandidateManagementAction, ManagementAction } from './candidate/CandidateManagementAction';
import { NextWordPredictionPolicy } from './candidate/NextWordPredictionPolicy';
import { CandidateFontFamilyPolicy } from './candidate/CandidateFontFamilyPolicy';
import { CandidateChipWidth } from './candidate/CandidateChipWidth';
import { CandidateContextMenuPolicy, PointerAction, PointerButton }
  from './input/CandidateContextMenuPolicy';
import { CandidateNumberFontPolicy } from './candidate/CandidateNumberFontPolicy';
import { CandidateWidthPolicy } from './candidate/CandidateWidthPolicy';
import { CandidatePagerPolicy } from './candidate/CandidatePagerPolicy';
import { CandidateLogoPolicy } from './candidate/CandidateLogoPolicy';
import { CandidateDecorationLayout, CandidateDecorationRect } from './candidate/CandidateDecorationLayout';
import { PreeditCaretPolicy } from './candidate/PreeditCaretPolicy';
import { CandidatePreeditStylePolicy, VisibleComposition } from './candidate/CandidatePreeditStylePolicy';
import { CandidateTranslationStyle } from './candidate/CandidateTranslationStyle';
import { CandidateLayoutPolicy, CandidateSkinGeometry, CandidateSkinPolicy } from './candidate/CandidateSkinPolicy';
import { CandidateWheelPolicy } from './candidate/CandidateWheelPolicy';
import { KeyboardMetrics } from './KeyboardMetrics';
import {
  COMBO_IDLE_RESET_MILLIS, TypingEffect, TypingEffectPolicy, TypingEffectSettings
} from './TypingEffectPolicy';
import { HitOffset, KeyboardGeometry } from './KeyboardGeometry';
import {
  NineKeyAction, NineKeyActionItem, NineKeyActionKind, NineKeyLayout, NineKey, NineKeySidebarItem, NumberPadOrigin,
  NumberPadPunctuation
} from './input/NineKeyLayout';
import { NineKeyShuangpinFace, nineKeyShuangpinFace, nineKeyShuangpinLabel } from './input/NineKeyShuangpinKeypad';
import { NineKeySwipe, NineKeySwipeGesture, NineKeySwipePolicy } from './input/NineKeySwipePolicy';
import { FourteenKey, FourteenKeyLayout } from './input/FourteenKeyLayout';
import { EighteenKeyLayout } from './input/EighteenKeyLayout';
import { EighteenKeySwipeGesture, EighteenKeyTap } from './input/EighteenKeySwipePolicy';
import { LayerCharacterRoute, LayerKey, LayerKeyKind, NumberSymbolLayout } from './input/NumberSymbolLayout';
import { ChineseSymbolFaces } from './input/ChineseSymbolFaces';
import {
  NineKeyPanelBackspace, NineKeyPanelMode, NineKeyPanelPolicy, NineKeyStroke
} from './input/NineKeyPanelPolicy';
import { SpaceCursorMovement } from './input/SpaceCursorMovement';
import {
  SPACE_LONG_PRESS_MS,
  SpaceGestureOutcome,
  SpaceGestureState,
  SpaceVoiceGesture
} from './input/SpaceVoiceGesture';
import { GlideArming, GlideKeyRect, GlidePoint, GlideTypingPolicy } from './input/GlideTypingPolicy';
import { PendingTouches, TouchKey, TouchPoint, TouchPointPolicy } from './input/TouchPointPolicy';
import { LetterHintTable } from './input/LetterHintTable';
import { HintKeyRect, LetterHintGesture } from './input/LetterHintGesture';
import { HelpcodeSwipeGesture } from './input/HelpcodeSwipePolicy';
import { KeyPreviewFrame, KeyPreviewLayout } from './KeyPreviewLayout';
import {
  EmojiCatalogModel, EmojiCategory, EmojiItem, EMOJI_PAGE_SIZE, EMOJI_RECENTS_ICON
} from './emoji/EmojiCatalogModel';
import {
  EmojiPanelGroupTab, PluginSymbolGroup, PluginSymbolGroupPolicy
} from './emoji/PluginSymbolGroupPolicy';
import {
  EmojiPanelKey, EmojiPanelKeyAction, EmojiPanelKeyDecision, EmojiPanelKeyPolicy
} from './emoji/EmojiPanelKeyPolicy';
import { EMOJI_TOOLTIP_DELAY_MS, CLIPBOARD_TOOLTIP_MAX_LINES, EmojiPanelTooltipPolicy } from './emoji/EmojiPanelTooltipPolicy';
import {
  MASCOT_STICKER_INSET_VP, MASCOT_STICKER_PANEL, MascotSticker, MascotStickerPolicy
} from './emoji/MascotStickerPolicy';
import { MascotStickerSource } from './emoji/MascotStickerStore';
import { KeyboardLog, describeError } from './KeyboardLog';
import { KeyboardFormFactor } from './KeyboardFormFactor';
import { DesktopSurface } from './DesktopSurface';
import { SurfaceRoutingPolicy } from './SurfaceRoutingPolicy';
import { ClipboardHistoryItem, ClipboardFailure } from './clipboard/ClipboardHistoryStore';
import { ClipboardHistoryPolicy } from './clipboard/ClipboardHistoryPolicy';
import { ClipboardSelectionPolicy } from './clipboard/ClipboardSelectionPolicy';
import {
  CLOUD_CLIPBOARD_SEND, CLOUD_CLIPBOARD_TAB, CloudClipboardItem, CloudClipboardPolicy,
  CloudClipboardSendOutcome, CloudClipboardState
} from './clipboard/CloudClipboardPolicy';
import { CloudClipboardBoundListing } from './clipboard/CloudClipboardClient';
import {
  HandwritingPoint, HandwritingStroke, HandwritingStrokePolicy,
  HANDWRITING_CANVAS_SIZE, HANDWRITING_MAX_POINTS, HANDWRITING_MAX_STROKES
} from './input/HandwritingStrokePolicy';
import {
  HandwritingRecognitionQueue, HandwritingRecognitionTicket
} from './input/HandwritingRecognitionQueue';
import { HarmonyVoiceRecognizer } from './input/HarmonyVoiceRecognizer';
import { VoiceRecognitionPolicy, VOICE_DONE_NOTICE_MS, VOICE_PAUSE_DEFAULT_MS } from './input/VoiceRecognitionPolicy';
import { VoiceInputConfigurationPolicy } from './input/VoiceInputConfiguration';
import { ReplyKeyboardPolicy, ReplyStyle } from './ReplyKeyboardPolicy';
import { CommunityReplyTemplate } from './CommunityReplyLibraryPolicy';
import { SymbolPanelCategory, SymbolPanelPolicy } from './input/SymbolPanelPolicy';
import { BackspaceHoldAction, BackspaceHoldPolicy } from './input/BackspaceHoldPolicy';
import { FunctionPanelItem, FunctionPanelPolicy, PickerGrid } from './FunctionPanelPolicy';
import { PROMOTION_TEXT } from './PromotionText';
import { KeyboardFormFactorPolicy } from './KeyboardFormFactorPolicy';
import display from '@ohos.display';
import { KeyboardIconPaths } from './KeyboardIconPaths';
import { BrandTile } from './faces/BrandTile';
import { KeyboardLineIcon } from './faces/KeyboardLineIcon';
import { AdjustAvailability, AdjustButtonRow, AdjustFrame, AdjustHandles, AdjustRise, AdjustSpan, AdjustValues, KeyboardAdjustPolicy } from './KeyboardAdjustPolicy';
import { MiniKeyboardPreview, MoreSkinsTile } from './faces/MiniKeyboardPreview';
import { PagedTiles } from './faces/PagedTiles';
import { PanelTile } from './faces/PanelTile';
import { OneHandRail } from './faces/OneHandRail';
import { OneHandedMode, OneHandedPolicy, OneHandedSidePanel } from './OneHandedPolicy';
import { FloatingKeyboardPolicy } from './FloatingKeyboardPolicy';
import { SplitKeyboardPolicy, SplitKeyboardSettings, SplitRow, SplitSpace } from './SplitKeyboardPolicy';
import { PrivacyGate } from './PrivacyGate';
import { AiGenerationGate } from './AiGenerationGate';
import { PhraseList, PHRASE_LIST_ERROR, PHRASE_LIST_LOADING, PHRASE_LIST_READY } from './faces/PhraseList';
import { VoiceListening } from './faces/VoiceListening';
import { VOICE_MASCOT_FILE, VoiceListeningPolicy, VoiceListeningSession } from './faces/VoiceListeningPolicy';

import { TextPolicy } from './TextPolicy';
import { ToolbarTools, TouchToolbarPolicy } from './TouchToolbarPolicy';
import {
  CommaHold,
  CommaSwipeGesture,
  DesignRowEntry,
  DesignRowItem,
  DesignRowSlot,
  NumberRowFace,
  StyledFace,
  TouchKeyboard,
  TouchLayoutStylePolicy,
  TouchPanelShape
} from './TouchLayoutStylePolicy';
/**
 * The 26-key touch keyboard, laid out to the same metrics as the iOS extension in
 * platforms/ios/KeyboardExtension/Sources/KeyboardViewController.swift, which is the reference
 * implementation for how this keyboard is meant to look and behave.
 *
 * Keys are drawn as Text in a bordered container rather than with Button. Button carries a default
 * horizontal padding, and ten keys to a row do not leave room for it: the glyph loses its right-hand
 * third instead of shrinking. The iOS code zeroes the same insets for the same reason.
 */

/**
 * One parameter, passed by reference.
 *
 * A @Builder that takes its arguments by value is rendered once and never again, so a key built that
 * way keeps the face it was created with: Shift would not change the letters, and the return key
 * would still read 换行 while candidates were on screen. Passing a single object through $$ is what
 * makes ArkUI re-render the subtree when the state behind it moves.
 */
interface KeyOptions {
  face: string;
  /**
   * What a screen reader says, which is not the face.
   *
   * Optional only so a key whose face is already a whole word need not repeat it; every key that
   * draws a glyph, an abbreviation or a mode indicator sets it. Absent leaves ArkUI reading the
   * face, which for `⇧` or `123` is nothing useful.
   */
  label?: string;
  /** The double-pinyin unit under the letter, or empty when the key does not stand for one. */
  hint?: string;
  /** 触屏键盘字母键右上角的符号提示（`LetterHintTable`），滑动或长按会输入它；其他键上为空或不设。有提示的键按下时也会显示预览气泡。 */
  cornerHint?: string;
  /**
   * 字母键的小写字母。设了它，键面和读屏文字在绘制时按当前大小写现算，`face` 和 `label` 不再用：自动大写在每次文字变化后异步切换大小写，若把大小写放进 `ForEach` 的键标识，每次切换都会重建全部字母键，手指正按着的那个键也被换掉，这次按键就丢了。
   */
  letter?: string;
  /** 双拼 18 键上这一键的字母（`FourteenKey.letters`）。设了它，横滑选中这一键的某个字母时键面在绘制时改画那个字母，与 `letter` 同理不进 `ForEach` 的键标识，正按着的键不会被重建。 */
  swipeLetters?: string;
  /** 拼音九键字母层 1–9 键的输入字符（`NineKey.input`）。设了它，这一键滑动输入数字时键面在绘制时改画那个数字（`nineKeySwipeFace`），与 `swipeLetters` 同理不进 `ForEach` 的键标识。 */
  nineKeyInput?: string;
  size: number;
  /** 用操作色填充：2in1 上按住的 Shift，或有组合串时的回车键（在触屏键盘的设计行上则始终填充）。 */
  emphasized: boolean;
  /** Drawn in the theme's function-key colour rather than the letter-key colour. */
  special?: boolean;
  /** 固定键的宽度；设了 `share` 时忽略。 */
  width: Length;
  /** 触屏键盘的设计行按键在行中所占的份额而不是固定宽度来定宽：值即该键的 `layoutWeight`，固定宽度的键不设。 */
  share?: number;
  /** 代替键面绘制的 `KeyboardIconPaths` 线性图标（Shift、Caps Lock、回车）；读屏仍回退到键面文字。 */
  icon?: string;
  /** 图标用强调色而不是标签色：表示 Shift 已开启。 */
  accentIcon?: boolean;
  /** 键面字重；不设即常规的 400。 */
  fontWeight?: number;
  /** 触摸区；不设即 `keyHitRegion`，向四周空隙各扩出一半。紧挨的中/英键和回车键用它把两者的分界挪进回车键帽，见 `KeyboardGeometry.RETURN_YIELD_VP`。 */
  hitRegion?: Rectangle[];
  /** 分离式键盘右半边内侧重复的 G、V（`SplitKeyboardPolicy.duplicatesInnerKey`）：输入和左边那个键相同，但不记进滑行输入的键位，滑行仍按左边那个键算。 */
  duplicate?: boolean;
  action: () => void;
}

/** 「输入方式」选择器的一项：一个方案、两种英文模式之一，或末尾的「添加语言」磁贴。 */
interface SchemePickerEntry {
  kind: string;
  scheme: SchemeDefinition | null;
}

const LETTER_ROWS: string[][] = [
  ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
  ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
  ['z', 'x', 'c', 'v', 'b', 'n', 'm']
];

// The same three rows the iOS keyboard offers behind its 123 key.
const SYMBOL_ROWS: string[][] = [
  ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
  [',', '.', '?', '!', ';', ':', '\'', '"', '@', '/'],
  ['(', ')', '[', ']', '<', '>', '\\', '-', '_', '=']
];

/**
 * What these keys send while composing Chinese, taken from the Engine's punctuation contract.
 *
 * The keys are labelled with the ASCII that produces them, because nothing else on the keyboard says
 * that a backslash is how one types a 、 — which is the question people actually ask. Showing what
 * the key will produce answers it without spending a second key on it. Paired marks show their
 * opening half; the Engine alternates on its own.
 */
// The two narrow columns either side of the nine-key grid, as a share of the keyboard's width. The
// iOS layout ties both to the same fraction so the grid stays square-ish across phone widths.
const NINE_KEY_COLUMN_PERCENT: number = 14;
// 全拼九键展开面板的左栏（拼音或笔画）和右栏（功能键）占键盘宽度的份额；左栏要放得下 zhuang 这样的长拼音，比收起时的拼音栏宽一些。
const NINE_KEY_PANEL_LEFT_PERCENT: number = 18;
const NINE_KEY_PANEL_RIGHT_PERCENT: number = 16;

// 足够长，刻意的单击不会触发重复删除；又足够短，长按不会显得卡住：沿用 Android 的 `BackspaceRepeatPolicy`，首次重复前 420 ms，此后每 70 ms 重复一次。
const BACKSPACE_DELAY_MILLIS: number = 420;
const BACKSPACE_REPEAT_MILLIS: number = 70;
// How far the finger travels for one cursor step. Roughly a key width, which is what a step feels
// like it should cost.
const CURSOR_STEP_VP: number = 12;
const KEY_HINT_FONT_SIZE: number = 10;
const KEY_HINT_LINE_HEIGHT_VP: number = 12;
// 角标提示距键上边和右边的距离（Android 的 `KeyHintButton.CORNER_HINT_TOP_DP` / `CORNER_HINT_RIGHT_DP`）。
const CORNER_HINT_TOP_VP: number = 3;
const CORNER_HINT_RIGHT_VP: number = 5;
// 键面文字跟随系统字体放大的上限，与 Android 的 `KeyboardTypographyPolicy.MAX_KEYBOARD_FONT_SCALE` 相同。字号是 fp、会随系统字体放大，键高和给角标留的顶部（CORNER_HINT_TOP_VP + KEY_HINT_LINE_HEIGHT_VP）是 vp、不变；不设上限时系统字体一调大，字母就顶进右上角的角标，和它挤在一起。
const KEY_MAX_FONT_SCALE: number = 1.15;
// The glide trail's stroke, about as wide as a fingertip's line reads without hiding the key faces under it.
const GLIDE_TRAIL_WIDTH_VP: number = 4;
/** `.cursor { width: 1.5px }` in the source's candidate stylesheets. */
const PREEDIT_CARET_WIDTH_VP: number = 1.5;
// 触屏工具栏（设计稿里的鸿蒙工具栏）：七个等宽格，距工具条两侧 2vp，每格居中放一个 40vp、圆角 12 的按钮，按下时缩到 .92。
const TOOLBAR_SIDE_PADDING_VP: number = 2;
const TOOLBAR_BUTTON_VP: number = 40;
const TOOLBAR_BUTTON_RADIUS_VP: number = 12;
const TOOLBAR_PRESSED_SCALE: number = 0.92;
const TOOLBAR_PRESS_MS: number = 100;
// 触屏候选栏：12fp 的预编辑行缩进 10vp，下方隔 2vp 是 34vp 高的候选胶囊通道，再接一条 1x22 的细线和 40vp 的展开箭头。
const PREEDIT_FONT_SIZE: number = 12;
const PREEDIT_LETTER_SPACING: number = 0.24;
const PREEDIT_INDENT_VP: number = 10;
const PREEDIT_LINE_VP: number = 14;
// 14/18 键的拼音选择条与读音同一行：读音最多占这一行的一半，其余留给选择条，与 Android、iOS 相同；拼音比读音大一号、用强调色，两侧各留 6vp 当触摸余量。
const READING_WITH_SPELLINGS_MAX_WIDTH: string = '50%';
const READING_SPELLING_FONT_SIZE: number = 13;
const READING_SPELLING_PADDING_VP: number = 6;
const READING_SPELLING_GAP_VP: number = 8;
const CANDIDATE_BAR_GAP_VP: number = 2;
const CHIP_HEIGHT_VP: number = 34;
const CHIP_MIN_WIDTH_VP: number = 30;
const CHIP_PADDING_VP: number = 11;
const CHIP_RADIUS_VP: number = 9;
const CHIP_PRESSED_OPACITY: number = 0.6;
const CHIP_GLOSS_FONT_SIZE: number = 10;
const CHIP_GLOSS_LINE_VP: number = 11.5;
/** 34vp 胶囊里放得下的最大字号：分下方有注释行和没有注释行两种情况。 */
const CHIP_WORD_MAX_WITH_GLOSS: number = 18;
const CHIP_WORD_MAX_ALONE: number = 24;
/** 候选词行高与字号之比，紧到 18fp 的词加上 11.5vp 的注释行正好填满 34vp 胶囊而不溢出。 */
const CHIP_WORD_LINE_HEIGHT: number = 1.2;
const BAR_DIVIDER_HEIGHT_VP: number = 22;
const BAR_BUTTON_VP: number = 40;
const BAR_BUTTON_PRESSED_OPACITY: number = 0.55;
// 触屏按键：字母 22fp，Shift 和删除键占 1.4 份并用 22vp 线性图标，功能键标签 15fp、字重 500，按下时在 60ms 内变暗到 .86 并缩到 .96。
const TOUCH_LETTER_FONT_SIZE: number = 22;
const KEY_ICON_VP: number = 22;
const FUNCTION_LABEL_WEIGHT: number = 500;
const COMPOSING_RETURN_WEIGHT: number = 600;
const KEY_PRESSED_BRIGHTNESS: number = 0.86;
const KEY_PRESSED_SCALE: number = 0.96;
const KEY_PRESS_MS: number = 60;
const LETTER_EDGE_SHARE: number = 1.4;
const SECOND_ROW_INDENT_SHARE: number = 0.5;
// 分离式键盘一排字母里的占位符（`splitLetterItems`），不会和任何字母或 `;` 撞上。
const SPLIT_INDENT_START: string = 'split:indent-start';
const SPLIT_INDENT_END: string = 'split:indent-end';
const SPLIT_SHIFT: string = 'split:shift';
const SPLIT_DELETE: string = 'split:delete';
// 设计稿的底行（123 1.25 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9）和各底行样式的份额在 `TouchLayoutStylePolicy`。
const SPACE_LABEL_FONT_SIZE: number = 13;
const SPACE_LABEL_GAP_VP: number = 4;
// 触屏 123 和 #+= 层（`NumberSymbolLayout`）：数字行 20fp，其下各行 18fp，与 Android 上同样的键尺寸一致。
const LAYER_FIRST_ROW_FONT_SIZE: number = 20;
const LAYER_FONT_SIZE: number = 18;
// 触屏九键键面，按宽度份额分配：三列各占 2 份的字母格两侧各有一列 1.15 份的标点（`NineKeyLayout.SIDE_SHARE`、`GRID_SHARE`），标点和字母格都是 17fp，字母格字重 500，右上角有 10fp 的数字。底行的份额在 `NineKeyLayout.actionRow`。
const NINE_KEY_FONT_SIZE: number = 17;
const NINE_KEY_LETTER_WEIGHT: number = 500;
// 展开的候选网格：列宽至少 64vp、列间距 6vp，格高 44vp、圆角 8、词 17fp，最多 40 格，下方是 40vp 高、放「返回」和删除键的底栏。
// 44vp 高度和 17fp 文字是默认候选字号 100% 时的设计尺寸；网格通过 `CandidateScalePolicy.expandedFontSize` / `expandedCellHeight` 随候选字号和候选栏大小缩放（#10400）。
const EXPANDED_MIN_COLUMN_VP: number = 64;
const EXPANDED_GAP_VP: number = 6;
const EXPANDED_PADDING_VP: number = 2;
const EXPANDED_CELL_HEIGHT_VP: number = 44;
const EXPANDED_CELL_PADDING_VP: number = 8;
const EXPANDED_CELL_RADIUS_VP: number = 8;
const EXPANDED_FONT_SIZE: number = 17;
const EXPANDED_PRESSED_BRIGHTNESS: number = 0.88;
const EXPANDED_FOOTER_VP: number = 40;
// 皮肤选择器的分页：网格上方 4vp、行间 2vp（预览四周自带 4vp 的选中环余量，所以缩略图离面板上沿 8vp、离上一行的名称 6vp），列间至少 6vp；有多页需要显示圆点时，圆点占 8vp 间距加自身 6vp，下方再留 4vp。
const SKIN_TOP_PADDING_VP: number = 4;
const SKIN_ROWS_GAP_VP: number = 2;
const SKIN_MIN_COLUMNS_GAP_VP: number = 6;
const PANEL_DOTS_ROOM_VP: number = 14;
// 功能面板和方案选择的磁贴网格：上方 10vp，满尺寸行高 52vp（设计稿和 Android 的 `FunctionPanelView`）；行数、行距和磁贴缩放见 `FunctionPanelPolicy.pickerGrid`，放不下满尺寸时磁贴整体等比缩小；面板比网格高时各行平分多出来的高度，而不是在底下空着一块。
const TILE_TOP_PADDING_VP: number = 10;
const TILE_ROW_VP: number = 52;
const PANEL_BOTTOM_PADDING_VP: number = 4;
const PANEL_SIDE_PADDING_VP: number = 4;
const PANEL_COLUMNS: number = 4;
/** `MiniKeyboardPreview` 在缩略图之外额外占用的空间：四周各 4vp 选中环余量（2vp 间隙加 2vp 环），环下方 4vp 间距和 12fp 的名称。 */
const SKIN_NAME_ROOM_VP: number = 28;
const SKIN_RING_ROOM_VP: number = 8;
/** 预览缩略图的高宽比，即设计稿 `MiniKb` 的 292:390。 */
const SKIN_THUMB_ASPECT: number = 292 / 390;
// The surfaces that cover the key rows. Only one is ever showing.
const SURFACE_NONE: string = '';
const SURFACE_SCHEME: string = 'scheme';
const SURFACE_SKIN: string = 'skin';
const SURFACE_EMOJI: string = 'emoji';
const SURFACE_CLIPBOARD: string = 'clipboard';
const SURFACE_TOOLS: string = 'tools';
const SURFACE_MODES: string = 'modes';
const SURFACE_GEOMETRY: string = 'geometry';
/** 键盘调整遮罩上的柄和「移动」键，`adjustTouch` 按它决定拖动改哪几项。 */
const ADJUST_TOP: string = 'top';
const ADJUST_BOTTOM: string = 'bottom';
const ADJUST_LEFT: string = 'left';
const ADJUST_RIGHT: string = 'right';
const ADJUST_MOVE: string = 'move';
const ADJUST_RESET: string = 'reset';
const ADJUST_CONFIRM: string = 'confirm';
/** 键盘调整遮罩的尺寸（vp）：四角括号的边长和线宽，柄里横线、竖线的长度，柄的触摸区长边，三个按钮的图标、字号和图标与文字的间距。线的粗细和间距、柄的短边、按钮的宽和间距在 `KeyboardAdjustPolicy`，按钮那一排的换算要用到它们。 */
const ADJUST_CORNER_VP: number = 20;
const ADJUST_CORNER_STROKE_VP: number = 3;
const ADJUST_GRIP_LENGTH_VP: number = 24;
const ADJUST_HANDLE_LONG_VP: number = 96;
const ADJUST_BUTTON_ICON_VP: number = 30;
const ADJUST_BUTTON_FONT_SIZE: number = 14;
const ADJUST_BUTTON_GAP_VP: number = 6;
const SURFACE_EXPANDED: string = 'expanded';
const SURFACE_VARIANTS: string = 'variants';
const SURFACE_VOICE: string = 'voice';
const SURFACE_REPLY: string = 'reply';
const SURFACE_POLISH: string = 'polish';
const SURFACE_SYMBOLS: string = 'symbols-panel';
const SURFACE_NINE_KEY_HOLD: string = 'nine-key-hold';
/** 工具栏的「常用语」面板。 */
const SURFACE_PHRASES: string = 'phrases';

const TOOL_ICON_VP: number = 18;
// Third, which is where iOS puts the English card.
const ENGLISH_CARD_POSITION: number = 2;
// English has no entry in KeyboardScheme to take its name from, being a mode rather than a scheme.
const ENGLISH_TITLES: string[] = ['英文 26 键', '英文 9 键'];
// 「输入方式」选择器中方案以外的项：两种英文模式和「添加语言」。
const PICKER_SCHEME: string = 'scheme';
const PICKER_ENGLISH_26: string = 'english-26';
const PICKER_ENGLISH_9: string = 'english-9';
const PICKER_ADD_LANGUAGE: string = 'add-language';
const HANDWRITING_CANVAS_ID: string = 'msime-harmony-handwriting-canvas';
// 触屏手写底行 123 | ， | space | 。 | 中 | 重写 | ⌫ | return 的份额，iOS（`KeyboardViewController.handwriting*Weight`）与 Android（`KeyboardActionRow.HANDWRITING_*_WEIGHT`）相同：书写区铺满整行后「重写」和 ⌫ 搬到底行，空格从 4 让到 2.4、回车从 1.9 让到 1.6。
const HANDWRITING_LAYER_SHARE: number = 1.25;
const HANDWRITING_PUNCTUATION_SHARE: number = 1;
const HANDWRITING_SPACE_SHARE: number = 2.4;
const HANDWRITING_LANGUAGE_SHARE: number = 1.05;
const HANDWRITING_REWRITE_SHARE: number = 1.25;
const HANDWRITING_DELETE_SHARE: number = 1.25;
const HANDWRITING_RETURN_SHARE: number = 1.6;
const HANDWRITING_STATUS_FONT_SIZE: number = 13;
// 虚线辅助框距书写区边缘的距离；设计稿原为 14，三个移动端统一收窄到 8，让可见书写区更大。
const HANDWRITING_GUIDE_INSET_VP: number = 8;
const HANDWRITING_GUIDE_RADIUS_VP: number = 8;
const HANDWRITING_IDLE_STATUS: string = '在此手写，停笔后选字';
// Enough tint to mark the card without drowning the glyph sitting on it.
const SELECTED_CARD_ALPHA: number = 0.14;
const TRANSPARENT: string = '#00000000';
// Roughly a third of a key. Shorter than this is a tap on the centre kana, not a flick.
const FLICK_THRESHOLD_VP: number = 16;
// The Engine's own numbering for Japanese, which the ported policies are written against.
const JAPANESE_SCHEME: number = 3;

// A category runs to a few hundred entries; this is the ceiling that keeps a catalog which never
// reports itself complete from spinning here forever.
const EMOJI_MAX_PAGES: number = 16;
const EMOJI_COLUMNS: number = 8;
// 触屏表情面板（与设计稿和 Android 一致）：网格下方隔 6vp 是 40vp 高的底行，ABC 和 ⌫ 各占 1.3 份，夹着占 5 份的分类标签；标签 32vp、圆角 9vp、图标 18vp；表情 26vp，手指按住时在 80 ms 内放大四分之一；格间距 2vp。
const EMOJI_BAR_VP: number = 40;
const EMOJI_BAR_GAP_VP: number = 6;
const EMOJI_BAR_EDGE_WEIGHT: number = 1.3;
const EMOJI_BAR_TABS_WEIGHT: number = 5;
const EMOJI_TAB_VP: number = 32;
const EMOJI_TAB_RADIUS_VP: number = 9;
const EMOJI_TAB_FONT_SIZE: number = 18;
const EMOJI_PAGE_TAB_PADDING_VP: number = 8;
/** Android 中未选中分类标签的透明度。 */
const EMOJI_TAB_IDLE_OPACITY: number = 0.6;
const EMOJI_GLYPH_FONT_SIZE: number = 26;
const EMOJI_PRESSED_SCALE: number = 1.25;
const EMOJI_PRESS_MS: number = 80;
const EMOJI_GRID_GAP_VP: number = 2;
const EMOJI_GROUP_ROW_VP: number = 28;
// 触屏符号面板（与 Android 的 `SymbolPanelView` 一致）：左边 56vp 宽的分类栏，五类平分整列高度，每类不高于 40vp；右边五列符号键，键高 42vp、键距 6vp，这样三行之外还露出第四行的上沿，一眼看得出能往下滚，滚动条在网格右侧 6vp 的空隙里，不压在键上；底行与表情面板同高，是「返回」「锁定」和 ⌫ 三个键。
const SYMBOL_COLUMNS: number = 5;
const SYMBOL_RAIL_VP: number = 56;
const SYMBOL_TAB_MAX_VP: number = 40;
const SYMBOL_SCROLLBAR_GUTTER_VP: number = 6;
const SYMBOL_CELL_VP: number = 42;
const SYMBOL_GAP_VP: number = 6;
const SYMBOL_FONT_SIZE: number = 20;
// 「@gmail.com」这类网址片段放不下 20vp 时缩到这个字号，不折行也不截断。
const SYMBOL_MIN_FONT_SIZE: number = 8;
const EMOJI_PANEL: string = 'emoji';
const KAOMOJI_PANEL: string = 'kaomoji';
const SYMBOLS_PANEL: string = 'symbols';
// 随包的水杉娘表情包，读得到时排在所有页签最前、打开面板默认停在这一页（`MascotStickerPolicy.tabOrder` / `openingPanel`）；手机底栏上画第一张的缩略图而不是标题。
const STICKER_PANEL: string = MASCOT_STICKER_PANEL;
/** 底栏「水杉娘」页签缩略图四周的留白，让图不贴着选中态的圆角底色。 */
const STICKER_TAB_INSET_VP: number = 2;
// The Windows emoji panel's last page (`EmojiPanel::Page::Clipboard`). Only a 2in1 draws it as a tab: the phone reaches clipboard history from its own tools face.
const CLIPBOARD_PANEL: string = 'clipboard';
// The clipboard surface's two sections: this device's history, and the account's cloud clipboard.
const CLIPBOARD_TAB_LOCAL: string = 'local';
// 触屏剪贴板面板：先是 32vp 高的一行标签和操作，然后是间距 6vp、圆角 10vp、内边距 9/12vp 的卡片，正文 14.5vp、行高 21vp，下面是 11.5vp 的元信息行。
const CLIP_HEADER_VP: number = 32;
const CLIP_HEADER_SIDE_PADDING_VP: number = 4;
const CLIP_HEADER_ACTION_GAP_VP: number = 14;
const CLIP_IDLE_TAB_OPACITY: number = 0.55;
const CLIP_CARD_GAP_VP: number = 6;
const CLIP_CARD_RADIUS_VP: number = 10;
const CLIP_CARD_PADDING_V_VP: number = 9;
const CLIP_CARD_PADDING_H_VP: number = 12;
const CLIP_CARD_FONT_SIZE: number = 14.5;
const CLIP_CARD_LINE_HEIGHT_VP: number = 21;
const CLIP_CARD_LINE_GAP_VP: number = 2;
const CLIP_META_FONT_SIZE: number = 11.5;
// 多选时选中卡片右上角的序号 ①②③④（#10411），正文右边让出这么宽，长文字不压在角标底下。
const CLIP_BADGE_FONT_SIZE: number = 18;
const CLIP_BADGE_SPACE_VP: number = 20;
const CLIPBOARD_TAB_CLOUD: string = 'cloud';
// Enough results to scroll through; a one-letter search would otherwise lay out the whole catalog.
const EMOJI_SEARCH_LIMIT: number = 512;

/** The shared touch-scheme record this view writes when the picker changes scheme. */
interface TouchKeyboardSchemes {
  enabled: string[];
  selected: string;
}

/** One tab across the top of the emoji surface, in the order they are drawn. */
interface EmojiPanelTab {
  id: string;
  title: string;
}

const EMOJI_PANEL_TABS: EmojiPanelTab[] = [
  { id: EMOJI_PANEL, title: '表情' },
  { id: KAOMOJI_PANEL, title: '颜文字' },
  { id: SYMBOLS_PANEL, title: '符号' },
  { id: STICKER_PANEL, title: '水杉娘' }
];

const DESKTOP_EMOJI_PANEL_TABS: EmojiPanelTab[] = EMOJI_PANEL_TABS.concat([
  { id: CLIPBOARD_PANEL, title: '剪贴板' }
]);

// Said plainly rather than as an error, because nothing is broken: the platform reserves clipboard
// reads for system applications and this is not one.
const CLIPBOARD_NOT_PERMITTED: string =
  '系统未授予读取剪贴板的权限，第三方输入法无法保存剪贴板历史。';
// 不说「已在设置中关闭」：API 12 起键盘读的是自己沙箱里那份偏好，设置页的开关传不过来（platforms/harmony/README.md「输入法扩展的独立沙箱」），设置页显示开着时这句话就是错的。开关因此放在面板里，见 enableClipboardHistory。
const CLIPBOARD_HISTORY_OFF: string = '剪贴板历史未开启';

@Entry
@Component
struct KeyboardView {
  // 这几项决定工具条是不是空闲（`stripIdle`），变了就要看面板该不该因工具条出现或消失而改高度，见 `syncStripHeight`。
  @State @Watch('syncStripHeight') candidates: CandidateEntry[] = [];
  @State @Watch('syncStripHeight') editing: string = '';
  /** 按住空格说话时空格键上的提示：「松开结束」或「松开取消」；不在按住说话时为空串。 */
  @State spaceVoiceHint: string = '';
  /** 「本地输入」这一层列出的模式，每次打开时按设置重读（`LocalInputMode.touchModes`）。 */
  @State localInputModes: LocalInputModeDefinition[] = LocalInputMode.MODES;
  @State editingCaret: number = 0;
  /** The candidate page on screen, from zero, and how many there are, for the 2in1 page indicator. */
  @State candidatePage: number = 0;
  @State candidatePageCount: number = 1;
  @State phrasePrefix: string = '';
  /** 手机上拼写正写在输入框里，候选栏不画读音；见 `KeyboardSession.compositionInField`。 */
  @State compositionInField: boolean = false;
  @State spellings: string[] = [];
  @State engineScheme: number = 0;
  /** Whether the scheme's openable candidate list (the Korean Hanja list, the Zhuyin list) is on the strip, as the Engine last reported it. */
  @State candidateListOpen: boolean = false;
  /** 候选栏上是上屏后联想（#9433）而不是组字的候选：回车按空闲处理，长按不管理，候选栏按偏好放关闭键。 */
  @State prediction: boolean = false;
  /** Read-only English completions for the word being typed; empty whenever one is not. */
  @State @Watch('syncStripHeight') englishSuggestions: string[] = [];
  /** 光标前打到 `xxx@` 时给出的邮箱后缀，先于英文补全显示；没有时为空。 */
  @State @Watch('syncStripHeight') emailSuffixes: string[] = [];
  @State shuangpinProfile: string = 'xiaohe';
  /** 双拼触屏字母组码布局；26/14/18 键独立于当前双拼方案保存。 */
  @State shuangpinLayout: string = 'twenty_six_key';
  /** 五笔触屏字母键布局；26/14 键独立于 86 / 98 版保存。 */
  @State wubiLayout: string = 'twenty_six_key';
  // 方案、中英文、123 层、本地模式和横竖屏决定手写区要不要多一行（`handwritingExtraRows`），变了就重定面板高度，见 `syncStripHeight`。
  @State @Watch('syncStripHeight') localMode: string = 'none';
  @State @Watch('syncStripHeight') diagnostic: string = '';
  @State punctuation: boolean = false;
  /** The Japanese digit layer's bracket list, which shares the strip the other long presses use. */
  @State brackets: boolean = false;
  @State managing: number = -1;
  /** Touch offers only a translation; mouse right-click keeps desktop management actions. */
  @State touchCandidateMenu: boolean = false;
  @State feedback: FeedbackSettings = KeyboardFeedback.DEFAULTS;
  @State traditional: boolean = false;
  /** Chinese punctuation, mirrored from the session so the strip's pill and the panel's tile redraw when it flips. */
  @State chinesePunctuation: boolean = true;
  /** 固定标点锁定了中文或英文标点：这时切换不起作用，功能面板的「中文标点」磁贴置灰。 */
  @State punctuationLocked: boolean = false;
  /** View 的 `punctuation_marks`：引号与方头括号样式，符号键和 123 层的键面跟着它画。 */
  @State cornerQuotes: boolean = false;
  @State hollowBrackets: boolean = false;
  /** View 的 `punctuation_marks` 顿号开关：顿号键面跟它画。 */
  @State enumerationCommaBackslash: boolean = true;
  @State enumerationCommaSlash: boolean = false;
  /** Fullwidth ASCII, mirrored from the session for the panel's 全 tile. */
  @State fullWidth: boolean = false;
  /** Candidate translations, mirrored from the session for the panel's 译 tile and the strip's 译 pill. */
  @State translations: boolean = false;
  /** Fuzzy pinyin, mirrored from the session for the panel's 模糊音 tile. */
  @State fuzzyPinyin: boolean = false;
  @State voiceSkin: KeyboardSkin = KeyboardSkin.fromTheme('system', 'system', null, false);
  @State emojiSkin: KeyboardSkin = KeyboardSkin.fromTheme('system', 'system', null, false);
  @State handwritingSkin: KeyboardSkin = KeyboardSkin.fromTheme('system', 'system', null, false);
  /** The 2in1 candidate context menu's colours: the candidate palette, in the menu's own mode. */
  @State menuColors: CandidateColors = GlobalTheme.candidateColors(null, false);
  @State @Watch('syncStripHeight') english: boolean = false;
  /** Which surface the toolbar asked this window to carry, on a machine with no key faces. */
  @State desktop: DesktopSurface = DesktopSurface.NONE;
  @State @Watch('symbolLayerChanged') symbols: boolean = false;
  /** 在触屏键盘的符号层中，当前显示的是否是 #+= 层而不是 123 层；在字母层上始终为 false（Android 的 `ImeLetterRows.moreSymbols`）。 */
  @State @Watch('dropLetterGestures') moreSymbols: boolean = false;
  // 九键、14/18 键和假名网格决定字母上方的数字行画不画（`numberRowExtraRows`），变了就重定面板高度，见 `syncStripHeight`。
  @State @Watch('syncStripHeight') nineKey: boolean = false;
  @State nineKeyShuangpinFaces: NineKeyShuangpinFace[] = [];
  /** 全拼 14/18 键在这台设备上生效：方案是对应键面且不是 2in1；地址栏、密码框这类编辑框由 `applyEditorFace` 收回到 26 键。英文时仍为真，只是画 26 键（`drawsFourteenKey`）。 */
  @State @Watch('syncStripHeight') fourteenKey: boolean = false;
  /** 全拼 18 键在手机和平板上生效；英文、本地模式和特殊编辑框回落到 26 键。 */
  @State @Watch('syncStripHeight') eighteenKey: boolean = false;
  /** Scheme 3 is Japanese, which has its own grid rather than a pinyin one. */
  @State @Watch('syncStripHeight') japanese: boolean = false;
  /** The active kana key's live flick target; -1 means no finger is previewing a key. */
  @State flickKeyIndex: number = -1;
  @State flickDirection: number = DIRECTION_CENTRE;
  @State emojiGroup: string = EmojiCatalogModel.categories()[0].group;
  @State emojiItems: string[] = [];
  @State emojiRecents: string[] = [];
  @State emojiShowingRecents: boolean = false;
  @State emojiPanel: string = EMOJI_PANEL;
  @State emojiUnicodeGroup: string = EmojiCatalogModel.categories()[0].group;
  // 颜文字页和符号页的标签：内置组在前，符号集插件的组在后，见 PluginSymbolGroupPolicy。
  @State emojiKaomojiTabs: EmojiPanelGroupTab[] = [];
  @State emojiSymbolTabs: EmojiPanelGroupTab[] = [];
  // 颜文字页或符号页当前选中标签的 key。
  @State emojiPanelTab: string = '';
  // What the 2in1 panel's search reads and which item the arrows have reached; see EmojiPanelKeyPolicy.
  @State emojiQuery: string = '';
  @State emojiSelected: number = 0;
  @State emojiResults: string[] = [];
  /** 随包的水杉娘表情包，第一次打开表情面板时读入；包里没有时为空，也就没有这个页签。 */
  @State mascotStickers: MascotSticker[] = [];
  /** 贴纸网格扣掉左右内边距后的宽度，决定列数和格子边长；还没量出来时为 0。 */
  @State mascotStickerGridWidth: number = 0;
  // 贴纸网格的懒加载数据源，与 `mascotStickers` 同时设定一次，之后不再替换。
  private mascotStickerSource: MascotStickerSource = new MascotStickerSource([]);
  private emojiGridScroller: Scroller = new Scroller();
  private symbolGridScroller: Scroller = new Scroller();
  private clipListScroller: Scroller = new Scroller();
  /** Bumped by every cloud fetch and every drop, so only the newest answer is applied. */
  private cloudRequest: number = 0;
  /** Bumped when a cloud send is cancelled or its target/context changes, so late answers are dropped. */
  private cloudSendRequest: number = 0;
  /** The editor the cloud listing on screen was fetched for. */
  private cloudEditor: number = -1;
  /** Digest of the login that supplied the visible rows; never a credential. */
  private cloudBinding: string = '';
  // Every item of a panel with its keywords, read once per panel the first time it is searched.
  private emojiSearchIndex: Map<string, EmojiItem[]> = new Map<string, EmojiItem[]>();
  // The keywords of every item the panel has read, so a 2in1 hover can name it; recents that were never read show no name.
  private emojiKeywords: Map<string, string> = new Map<string, string>();
  // The 2in1 hover tooltip: which item it belongs to and what it says. See EmojiPanelTooltipPolicy.
  @State hoverTipKey: string = '';
  @State hoverTip: string = '';
  private hoverTimer: number = -1;
  @State symbolCategory: number = 0;
  @State symbolLocked: boolean = false;
  /** 长按九键字母格或 14/18 键双字母键时弹出的字母（九键还有数字），这些键面共用 `SURFACE_NINE_KEY_HOLD`。 */
  @State nineKeyHoldOptions: string[] = [];
  /** 拼音网格的 1 键（@#）放进候选条的常用标点；为空时候选条画 Engine 的候选。Engine 下一次回报（`onChanged`）、点选其中一个或打开别的界面时清空。 */
  @State @Watch('syncStripHeight') nineKeyQuickMarks: PunctuationEntry[] = [];
  /**
   * Which surface is covering the keys, if any.
   *
   * One field rather than a flag per panel. With a flag each, the render chain picked whichever came
   * first and the rest stayed set behind it: opening the skin picker and then tapping emoji set the
   * emoji flag, the chain still matched the picker, and the tap read as doing nothing while the two
   * were open at once. A single value cannot be in two states.
   */
  @State @Watch('dropLetterGestures') surface: string = SURFACE_NONE;
  @State clipboardEnabled: boolean = KeyboardSession.shared.clipboardHistoryAllowed();
  @State clips: ClipboardHistoryItem[] = [];
  @State clipboardNotice: string = '';
  @State confirmingClear: boolean = false;
  /** Which section of the clipboard surface is showing; the cloud one is never offered in a password field. */
  @State clipboardTab: string = CLIPBOARD_TAB_LOCAL;
  /** Whether the focused editor may see cloud items, mirrored from the session so the builders redraw when the field changes. */
  @State cloudVisible: boolean = false;
  @State cloudClips: CloudClipboardItem[] = [];
  /** What the last cloud listing said; meaningless until cloudStateKnown. */
  @State cloudState: CloudClipboardState = CloudClipboardState.FAILED;
  @State cloudStateKnown: boolean = false;
  @State cloudLoading: boolean = false;
  @State cloudSending: boolean = false;
  /** The local history entry a long press offered to send to the cloud clipboard, or empty. */
  @State clipSendTarget: string = '';
  /** 触屏剪贴板的多选（#10411）：点「多选」进入，点卡片按先后选中并在右上角标出 ①②③④，「插入」把选中的几条按序号一次插入，「取消」退出。只作用于本机历史，与 Android 的多选入口一致。 */
  @State clipSelecting: boolean = false;
  /** 按点选先后排列的选中文字（本机历史按文字去重，文字就是条目的标识）。 */
  @State clipSelection: string[] = [];
  @State expanded: ExpandedCandidate[] = [];
  /** 展开网格已经建了 `expanded` 开头的多少格，规则在 `ExpandedCandidateBatchPolicy`。 */
  @State expandedBuilt: number = 0;
  /** `expanded` 来自哪个会话的哪一代候选（`KeyboardSession.candidateListIdentity`）；面板关着时为空。 */
  private expandedListIdentity: string = '';
  private expandedScroller: Scroller = new Scroller();
  private expandedViewportHeight: number = 0;
  private expandedContentHeight: number = 0;
  /** Which row of the expanded panel is showing its gloss, or -1. Its own index space. */
  @State expandedGloss: number = -1;
  /** 全拼九键展开面板的左栏放拼音还是笔画；关闭面板时回到拼音。 */
  @State nineKeyPanelMode: NineKeyPanelMode = NineKeyPanelMode.SPELLING;
  /** 引擎报告的九宫格筛选状态（`View.nine_key_single_character`、`View.nine_key_strokes`）。 */
  @State nineKeySingleCharacter: boolean = false;
  @State nineKeyStrokes: string = '';
  @State availableWidth: number = 0;
  /** The candidate card's typing-effect flash: its opacity, animated back to 0 after each key. */
  @State effectFlash: number = 0;
  /** The flash colour an effect pack sets, `undefined` for the candidate accent. */
  @State effectColor: string | undefined = undefined;
  /** The combo badge's text, empty while there is no combo to show. */
  @State effectBadge: string = '';
  /** Power mode swells the badge on each counted key and lets it settle back. */
  @State effectBadgeScale: number = 1;
  /** The candidate card's height, so the flash covers exactly the card and not the decoration above it. */
  @State effectCardHeight: number = 0;
  @State caseMode: LetterCaseMode = LetterCaseMode.LOWERCASE;
  @State skin: KeyboardSkin = KeyboardSkin.fromTheme('system', 'system', null, false);
  @State @Watch('syncStripHeight') scheme: SchemeDefinition = KeyboardScheme.fallback();
  /** 五笔版本：方案仍是同一个五笔，只换卡片和工具栏上的「86」「98」字样；单独设成状态，只改版本时也会重绘。 */
  @State wubiProfile: string = KeyboardScheme.WUBI_86;
  @State handwritingStrokes: HandwritingStroke[] = [];
  @State handwritingRedo: HandwritingStroke[] = [];
  @State @Watch('syncStripHeight') handwritingCandidates: string[] = [];
  @State handwritingNotice: string = '';
  @State handwritingBusy: boolean = false;
  @State voiceText: string = '';
  @State voiceNotice: string = '';
  @State voiceBusy: boolean = false;
  @State voiceFinishing: boolean = false;
  /** 触屏上，直接上屏的识别结果内容，在聆听界面上以「已识别：…」显示 `VOICE_DONE_NOTICE_MS`；其他时候为空。 */
  @State voiceDone: string = '';
  /** 触屏上，识别器目前听到的文字；用户选择显示中间结果时，聆听界面在录音时把它显示在标题位，收尾识别时显示在提示行。 */
  @State voiceHeard: string = '';
  /** 麦克风电平 0–1，聆听面板的光圈跟着它缩放；只在录音中更新，收尾、完成和取消时回到 0。 */
  @State voiceLevel: number = 0;
  /** 聆听面板的指示区画水杉娘（非按住的会话）还是圆盘，由 `refreshVoiceMascot` 按两个按住标志算出。 */
  @State voiceMascot: boolean = true;
  @State voiceEnabled: boolean = KeyboardSession.shared.voiceInputEnabled();
  @State replySource: string = '';
  @State replyStyle: string = '高情商';
  @State replyPolish: boolean = false;
  @State replyResults: string[] = [];
  @State replyNotice: string = '';
  @State replyBusy: boolean = false;
  @State replyTemplates: CommunityReplyTemplate[] = [];
  @State replyTemplateMenu: boolean = false;
  @State polishSource: string = '';
  @State polishResult: string = '';
  @State polishNotice: string = '';
  @State polishBusy: boolean = false;
  @State polishConfigured: boolean = false;
  @State hoveredCandidate: number = -1;
  @State geometryKeySpacing: number = KeyboardSession.shared.preferences().keySpacingTenths;
  @State geometryRowSpacing: number = KeyboardSession.shared.preferences().rowSpacingTenths;
  /** 屏幕上按键行的高度调整值：已保存的值，或键盘调整打开期间正在预览的值。 */
  @State geometryHeight: number = KeyboardSession.shared.preferences().heightAdjustment;
  /** 键盘调整的底部抬高、宽度和水平位置（`KeyboardAdjustPolicy`，存在本机的按键反馈文件里）：已保存的值，或键盘调整打开期间正在预览的值。 */
  @State geometryLift: number = KeyboardSession.shared.feedbackSettings().bottomLift;
  @State geometryWidthPercent: number = KeyboardSession.shared.feedbackSettings().widthPercent;
  @State geometryPositionX: number = KeyboardSession.shared.feedbackSettings().positionX;
  /** 「常用语」面板的条目及其读取状态（`PhraseList` 的 `PHRASE_LIST_*` 状态）。 */
  @State phrases: string[] = [];
  @State phraseState: string = PHRASE_LIST_LOADING;
  /** 每次从标志打开功能面板时递增，让 `PagedTiles` 回到第一页。 */
  @State panelResetToken: number = 0;
  /** 手机横屏（`KeyboardFormFactorPolicy.phoneLandscape`）：功能面板和方案选择只排两行。转屏时随屏幕变化重读，开着的面板跟着按新行数重排。 */
  @State @Watch('syncStripHeight') phoneLandscape: boolean = false;
  /** 屏幕宽大于高（手机、平板都算）：横屏时手写区保持三行，不像竖屏那样多一行（`KeyboardMetrics.handwritingExtraRows`），与 iOS、Android 同一条规则。 */
  @State @Watch('syncStripHeight') landscape: boolean = false;
  /** 单手模式下侧栏与按键这一块的宽度（`OneHandedPolicy.blockWidthVp`）：0 表示手机，铺满整行；大屏上是手机大小。转屏时随屏幕重读。 */
  @State oneHandBlockVp: number = 0;
  /** 大屏单手时空出来那一侧的数字键盘宽度（`OneHandedPolicy.sidePadWidthVp`）：0 表示手机或放不下。转屏时随屏幕重读。 */
  @State oneHandSidePadVp: number = 0;
  /** 共享偏好 `touch_one_handed_side_panel`：大屏单手模式空出来的一侧放什么（#10203）。 */
  @State oneHandedSidePanel: OneHandedSidePanel = KeyboardSession.shared.preferences().oneHandedSidePanel;
  /** 共享偏好里分离式键盘的四项（#10224，`SplitKeyboardPolicy`）。 */
  @State splitKeyboard: SplitKeyboardSettings = KeyboardSession.shared.preferences().splitKeyboard;
  /** 屏幕此刻的宽和高（vp，转屏后互换）：分离式键盘按它判断方向和启用宽度，按它算中间空隙。转屏时随屏幕重读。 */
  @State screenWidthVp: number = 0;
  @State screenHeightVp: number = 0;
  private readonly displayChanged: (id: number) => void = (_id: number): void => this.readOrientation();
  /** 共享偏好 `touch_shuangpin_key_hints`：关掉时 26 键双拼的字母键不画声母/韵母提示。 */
  @State shuangpinKeyHints: boolean = KeyboardSession.shared.preferences().shuangpinKeyHints;
  /** 共享偏好 `touch_quanpin_shift`：打开时全拼 26 键组字保留 ⇧，点一下臂辅助码。 */
  @State quanpinShift: boolean = KeyboardSession.shared.preferences().quanpinShift;
  /** 共享偏好 `harmony_toolbar`：工具栏上画哪些工具，整行是否隐藏。 */
  @State @Watch('syncStripHeight') toolbarTools: ToolbarTools = KeyboardSession.shared.preferences().toolbarTools;
  /** 共享偏好 `touch_twenty_six_key_style`：中文 26 键的底行样式（`TouchLayoutStylePolicy.designRow`）；长空格样式让工具栏一直画着，所以变了要重定面板高度。 */
  @State @Watch('syncStripHeight') twentySixKeyStyle: string = KeyboardSession.shared.preferences().twentySixKeyStyle;
  /** 共享偏好 `touch_fourteen_key_style`：14 键与 18 键的底行样式。 */
  @State fourteenKeyStyle: string = KeyboardSession.shared.preferences().fourteenKeyStyle;
  /** 共享偏好 `touch_nine_key_style`：九键字母层的底行样式（`NineKeyLayout.actionRow`）。 */
  @State nineKeyStyle: string = KeyboardSession.shared.preferences().nineKeyStyle;
  /** 共享偏好 `touch_twenty_six_symbol_first`：26 键底行 符 排在 123 左边（`TouchLayoutStylePolicy.symbolFirst`）。 */
  @State twentySixSymbolFirst: boolean = KeyboardSession.shared.preferences().twentySixSymbolFirst;
  /** 共享偏好 `touch_number_pad_space`：九宫格数字键盘里空格的位置（`NineKeyLayout.actionRow` 的 `space`），鸿蒙缺省是 `before_zero`。 */
  @State numberPadSpace: string = KeyboardSession.shared.preferences().numberPadSpace;
  /** 共享偏好 `touch_nine_key_sidebar_symbols`（#10223）：中文九键一族字母层左列的表，空表画出厂的 ，。？（`NineKeyLayout.sideMarks`）。 */
  @State nineKeySidebarSymbols: string[] = KeyboardSession.shared.preferences().nineKeySidebarSymbols;
  /** 共享偏好 `touch_nine_key_digit_sidebar_symbols`：九宫格数字键盘左列 #+= 上面的表，空表画出厂的四则运算（`NineKeyLayout.digitSidebar`）。 */
  @State nineKeyDigitSidebarSymbols: string[] = KeyboardSession.shared.preferences().nineKeyDigitSidebarSymbols;
  /** 共享偏好 `touch_language_key_side` 为 `left`：设计底行、九键外框、数字九宫格和手写底行的 中/英 排到空格左边（`TouchLayoutStylePolicy.languageBeforeSpace`、`NineKeyLayout.actionRow` 的 `languageLeft`）。 */
  @State languageKeyLeft: boolean = KeyboardSession.shared.preferences().languageKeyLeft;
  /** 「右侧大回车」外框（`tallReturnNineKeyFrame`）量到的宽度，网格下方那一段底行按它定 123、中/英 和数字层空格的绝对宽度（`NineKeyLayout.tallReturnRow`）；量到之前和外框消失后为 0，按份额排。 */
  @State tallReturnFaceWidth: number = 0;
  /** 共享偏好 `touch_number_row`：26/14/18 键字母上方画 1 到 0，键盘高一行（`numberRowExtraRows`）。 */
  @State @Watch('syncStripHeight') numberRow: boolean = KeyboardSession.shared.preferences().numberRow;
  /** 当前编辑框是密码框或只收数字（`KeyboardSession.editorKeepsStandardRows`）：这时三种键面类型的底行样式都退回标准底行（`TouchLayoutStylePolicy.fieldRowStyle`），长空格样式让工具栏一直画着的规则也随之不算，所以变了要重定面板高度。 */
  @State @Watch('syncStripHeight') standardRowsField: boolean = false;
  /** 长空格样式下逗号的上滑或下滑（输入句号）。 */
  private readonly commaSwipe: CommaSwipeGesture = new CommaSwipeGesture();
  /** 面板上一次按工具条显示与否定下的高度，`syncStripHeight` 只在它和现在不一样时改面板高度。 */
  private stripWasShown: boolean =
    !TouchToolbarPolicy.rowHidden(KeyboardSession.shared.preferences().toolbarTools);
  /** 面板上一次定高时多出的行数（手写区、数字行），`syncStripHeight` 只在它和现在不一样时改面板高度。视图出现时换成扩展能力建面板时用的值（`KeyboardSession.firstTouchPanel`）。 */
  private extraRowsWere: number = 0;
  /** 面板上一次定高时算进去的底部抬高（vp），`syncStripHeight` 只在它和现在不一样时改面板高度（转屏、浮动开关会让它生效或不生效）。视图出现时换成扩展能力建面板时用的值（`KeyboardSession.firstTouchLift`）。 */
  private liftWas: number = 0;

  /** 按下的键变暗和缩小的程度：触屏上是设计稿的 .86 和 .96；2in1 上保持不变，它的设备形态在进程存续期间不会改变。 */
  private readonly keyPressedBrightness: number = KeyboardFormFactor.isDesktop() ? 1 : KEY_PRESSED_BRIGHTNESS;
  private readonly keyPressedScale: number = KeyboardFormFactor.isDesktop() ? 1 : KEY_PRESSED_SCALE;
  private readonly letterCase: EnglishLetterCaseState = new EnglishLetterCaseState();
  private settings: KeyboardPreferences = KeyboardSession.shared.preferences();
  private readonly cursorDrag: SpaceCursorMovement = new SpaceCursorMovement();
  /** 空格键的长按语音、拖动移光标与普通空格之间的判定。 */
  private readonly spaceGesture: SpaceVoiceGesture = new SpaceVoiceGesture();
  private spaceLongPressTimer: number = -1;
  private spaceDownAt: number = 0;
  /** 这次空格长按是否真的开始了录音；`startVoice` 可能因为语音会话不可用而没有开始。 */
  private spaceVoiceHeld: boolean = false;
  // A held Delete repeats, and letting go has to stop it, so the timer outlives the callback.
  private backspaceTimer: number = -1;
  private backspaceRepeated: boolean = false;
  private dragged: boolean = false;
  private diagnosticTimer: number = -1;
  /** Clears the combo badge once the combo has ended for want of keys. */
  private effectTimer: number = -1;
  private pendingRemoval: number = -1;
  private flickX: number = 0;
  private flickY: number = 0;
  private handwritingCurrent: HandwritingPoint[] = [];
  private handwritingTimer: number = -1;
  private readonly handwritingRecognition: HandwritingRecognitionQueue =
    new HandwritingRecognitionQueue();
  private handwritingContext: CanvasRenderingContext2D =
    new CanvasRenderingContext2D(new RenderingContextSettings(true));
  private skinPatternContext: CanvasRenderingContext2D =
    new CanvasRenderingContext2D(new RenderingContextSettings(true));
  private readonly voiceRecognizer: HarmonyVoiceRecognizer = new HarmonyVoiceRecognizer();
  private voiceGeneration: number = 0;
  /** 录音是否由语音快捷键按住；快捷键会自己结束录音，所以说话停顿不能结束它。 */
  private voiceHeldByKey: boolean = false;
  /** 停顿之后录音会不会自己结束（`VoiceRecordingBehaviourPolicy.endsOnPause`），用于提示停顿自动结束或点「说完了」；在 `voiceBusy` 之前更新，供手机聆听面板读取。 */
  private voiceEndsOnPause: boolean = true;
  /** 这次聆听由键盘的停顿计时结束：触屏上的本地识别和系统识别（`VoiceRecordingBehaviourPolicy.keyboardStopsOnPause`），开始聆听时固定。 */
  private voicePauseEnds: boolean = false;
  /** 这次聆听的停顿时间（`VoiceRecordingBehaviourPolicy.pauseMillis`），开始聆听时固定，改设置从下一次聆听生效。 */
  private voicePauseMs: number = VOICE_PAUSE_DEFAULT_MS;
  private voiceSilenceTimer: number = -1;
  /** 经过 VAD 的本地整段模型此刻是否听到说话；其他识别器从不报，始终为 false。 */
  private voiceSpeaking: boolean = false;
  private voiceDoneTimer: number = -1;
  private replyGeneration: number = 0;
  private polishGeneration: number = 0;
  /** 每次加载常用语和离开面板时递增，只绘制最新的结果。 */
  private phraseGeneration: number = 0;
  /** 键盘调整里正被拖动的柄或「移动」键（`ADJUST_*`），没有在拖时为空串。 */
  private adjustDragHandle: string = '';
  /** 按下那一刻手指的屏幕坐标（vp），之后的位移都相对它算：改高度、抬高时面板跟着变，柄在窗口里的位置会变，屏幕坐标不会。 */
  private adjustDragOrigin: [number, number] = [0, 0];
  /** 按下那一刻的四项和面板宽度（vp），整个拖动都从这里换算，慢速拖动不会被逐次取整抹掉。 */
  private adjustDragBase: AdjustValues = KeyboardAdjustPolicy.DEFAULTS;
  /** 打开键盘调整时的四项，离开时按它判断哪几项动过（`KeyboardAdjustPolicy.saved`）。 */
  private adjustOriginal: AdjustValues = KeyboardAdjustPolicy.DEFAULTS;
  private adjustDragWidthVp: number = 0;
  /** 「手写」磁贴切换前的方案，再次点按回到它；键盘不是由该磁贴切换过来时为 null。 */
  private schemeBeforeHandwriting: SchemeDefinition | null = null;
  /** 「手写」磁贴切换前键盘是否在英文（英文 26 键、英文 9 键），再次点按时一并回到英文（#9703）。 */
  private englishBeforeHandwriting: boolean = false;
  /** Glide typing (滑行输入): the letter keys a..z in window coordinates, as their area callbacks last reported them; null until a key has been laid out. */
  private glideKeys: (GlideKeyRect | null)[] = GlideTypingPolicy.emptyKeys();
  /** The finger being watched for a glide, from its touch-down on a letter key until it lifts; -1 while none is. */
  private glideFinger: number = -1;
  private glideDownKey: number = -1;
  private glideDownX: number = 0;
  /** When the watched finger went down, by Date.now(); every sample's time is measured from it. */
  private glideDownMillis: number = 0;
  /** Every sample of the watched finger since touch-down, in window coordinates. */
  private glidePoints: GlidePoint[] = [];
  /**
   * Set when a touch becomes a glide and cleared only by the next touch-down on the letter rows. A letter tapped in between is not typed: the key the stroke started on, whichever order ArkUI delivers its click and the lift in, and any other finger's key while the stroke ran.
   */
  private glideSwallowsTap: boolean = false;
  /** A glide is under way: its trail is drawn over the keys, and other fingers are ignored until it lifts. */
  @State glideActive: boolean = false;
  /** The keyboard's top-left in window coordinates, which the trail canvas is drawn from. */
  private glideOriginX: number = 0;
  private glideOriginY: number = 0;
  /** 键盘宽度（vp，`keyboardStack` 量到的面板宽），用于把按键预览气泡限制在键盘内；键盘调整的外框和左右沿拖动也按它换算（`adjustPanelWidthVp`），所以是状态，量到新宽度时外框跟着重画。 */
  @State rootWidth: number = 0;
  /** 按下字母键时其上方的按键预览气泡（按键弹出预览），以相对键盘左上角的 vp 计；未显示时为 null。 */
  @State keyPreview: KeyPreviewFrame | null = null;
  /** 气泡所属按键对应的手指，这样别处抬起的手指不会撤掉仍被按住的键的气泡；未显示时为 -1。 */
  private keyPreviewFinger: number = -1;
  /** 字母键上可能输入角标提示而不是字母的触摸。 */
  private letterHints: LetterHintGesture = new LetterHintGesture();
  /** 中文组字中的 26 键竖向滑动辅助码；与角标手势分开跟踪，长按仍保留角标。 */
  private helpcodeSwipes: HelpcodeSwipeGesture = new HelpcodeSwipeGesture();
  /** 26 键字母键和 `;` 的按下点，等这一键的点击取走（#10018，`TouchPointPolicy`）。 */
  private pendingTouches: PendingTouches = new PendingTouches();
  /** 微软双拼第二行末尾 `;` 键的位置（窗口坐标），同 `glideKeys`；没量到时为 null。 */
  private semicolonKey: GlideKeyRect | null = null;
  /** 每个被跟踪手指的长按计时器。 */
  private hintHoldTimers: Map<number, number> = new Map<number, number>();
  /** 双拼 18 键两个字母的键上的横滑。 */
  private eighteenSwipes: EighteenKeySwipeGesture = new EighteenKeySwipeGesture();
  /** 横滑正选中字母的那个 18 键（`FourteenKey.letters`）和选中的大写字母；没有时都为空串。 */
  @State eighteenSwipeKey: string = '';
  @State eighteenSwipeLetter: string = '';
  /** 拼音九键字母层 1–9 键的上滑或下滑（共享偏好 `touch_nine_key_swipe_digits`，`NineKeySwipePolicy`）。 */
  private nineKeySwipes: NineKeySwipeGesture = new NineKeySwipeGesture();
  /** 正滑动输入数字的那个九键（`NineKey.input`）和它的数字：键面临时画这个数字、角标藏起来；没有时都为空串。 */
  @State nineKeySwipeKey: string = '';
  @State nineKeySwipeDigit: string = '';
  private glideContext: CanvasRenderingContext2D =
    new CanvasRenderingContext2D(new RenderingContextSettings(true));
  /** 浮动键盘拖动条按下时手指在屏幕上的位置（vp）。 */
  private floatingTouchOrigin: [number, number] = [0, 0];

  aboutToAppear(): void {
    // 扩展能力按上次记下的键面定了第一帧的高度（`TouchLayoutStylePolicy.firstPanel`）；从那个高度出发核对，键面与记下的不同时 `syncStripHeight` 再改高度。要在下面任何一个带 @Watch 的状态被赋值之前定下。
    const opened: TouchPanelShape = KeyboardSession.shared.firstTouchPanel();
    this.stripWasShown = opened.strip;
    this.extraRowsWere = opened.extraRows;
    this.liftWas = KeyboardSession.shared.firstTouchLift();
    this.readOrientation();
    display.on('change', this.displayChanged);
    this.english = KeyboardSession.shared.isEnglish();
    this.clipboardEnabled = KeyboardSession.shared.clipboardHistoryAllowed();
    KeyboardSession.shared.onClipboardHistoryChanged = (enabled: boolean): void => {
      const wasEnabled: boolean = this.clipboardEnabled;
      this.clipboardEnabled = enabled;
      if (enabled && !wasEnabled) {
        this.clips = KeyboardSession.shared.clipboardHistory();
        this.clipboardNotice = '';
      }
      if (!enabled) {
        this.clips = [];
        this.confirmingClear = false;
        this.endClipSelection();
        this.clipboardNotice = '剪贴板历史已关闭，历史记录已清空';
        if (this.surface === SURFACE_CLIPBOARD) {
          this.show(SURFACE_NONE);
        }
      }
    };
    this.voiceEnabled = KeyboardSession.shared.voiceInputEnabled();
    // Polish shares the voice settings' provider, so it is re-read on the same notification: the
    // card would otherwise keep whatever the answer was when this view was built, and a user who
    // had just configured a provider would still see it greyed out.
    this.polishConfigured = KeyboardSession.shared.polishConfigured();
    KeyboardSession.shared.onVoiceInputChanged = (enabled: boolean): void => {
      this.voiceEnabled = enabled;
      this.polishConfigured = KeyboardSession.shared.polishConfigured();
      if (this.surface === SURFACE_POLISH && !this.polishConfigured) {
        this.show(SURFACE_NONE);
      }
      if (!enabled) {
        this.cancelVoice();
        if (this.surface === SURFACE_VOICE) {
          this.show(SURFACE_NONE);
        }
        if (KeyboardFormFactor.isDesktop()
          && KeyboardSession.shared.openSurface() === DesktopSurface.VOICE) {
          KeyboardSession.shared.setDesktopSurface(DesktopSurface.NONE);
        }
      }
    };
    KeyboardSession.shared.onEnglishSuggestions = (prefix: string, items: string[]): void => {
      this.englishSuggestions = items;
    };
    KeyboardSession.shared.onEmailSuffixes = (suffixes: string[]): void => {
      this.emailSuffixes = suffixes;
    };
    KeyboardSession.shared.onVoiceHotkey = (action: VoiceHotkeyAction): void => {
      // The panel has to be on screen before recording starts, or the first thing the user sees of
      // a shortcut they just pressed is nothing at all.
      if (action === VoiceHotkeyAction.START) {
        const desktopSurface: DesktopSurface | null =
          SurfaceRoutingPolicy.voiceSurface(KeyboardFormFactor.isDesktop());
        if (desktopSurface !== null) {
          KeyboardSession.shared.setDesktopSurface(desktopSurface);
        } else {
          this.show(SURFACE_VOICE);
        }
        this.startVoice(true);
        return;
      }
      if (action === VoiceHotkeyAction.STOP) {
        this.stopVoice();
        return;
      }
      if (action === VoiceHotkeyAction.CANCEL) {
        this.cancelVoice();
        if (SurfaceRoutingPolicy.voiceSurface(KeyboardFormFactor.isDesktop()) !== null) {
          KeyboardSession.shared.setDesktopSurface(DesktopSurface.NONE);
        } else {
          this.show(SURFACE_NONE);
        }
        return;
      }
      if (action === VoiceHotkeyAction.LOCK) {
        this.voiceNotice = '已锁定录音，松开按键不会结束。';
        // 锁定后松开按键不再结束录音，会话从此按点按开始对待：面板给「说完了」和「取消」；触屏上本地识别和系统识别停顿后自动结束（`voicePauseEnds`，见 `VoiceRecordingBehaviourPolicy.keyboardStopsOnPause`），豆包流式和上传式识别点「说完了」结束，指示区换成水杉娘。快捷键的 STOP、CANCEL 不看这个标志，照常生效。
        this.voiceHeldByKey = false;
        this.refreshVoiceMascot();
      }
    };
    KeyboardSession.shared.onPanelKey = (key: EmojiPanelKey): boolean => this.panelKey(key);
    KeyboardSession.shared.onSurfaceChanged = (surface: DesktopSurface): void => {
      if (SurfaceRoutingPolicy.endsVoice(this.desktop, surface) && this.voiceBusy) {
        this.cancelVoice();
      }
      if (SurfaceRoutingPolicy.endsHandwriting(this.desktop, surface) && this.hasHandwritingContext()) {
        this.clearHandwriting();
      }
      this.desktop = surface;
      if (surface === DesktopSurface.EMOJI) {
        // A search is for one visit to the panel, as it is on Windows, where the box starts empty each time the panel is shown.
        this.resetEmojiSearch();
        // 标签行要知道有没有「水杉娘」这一页。
        this.loadMascotStickers();
        if (this.emojiItems.length === 0) {
          this.loadEmoji(this.emojiGroup);
          // 第一次打开时默认选中「水杉娘」，之后与 Windows 的面板一样回到上次那一页；读不到表情包时停在表情页。
          this.emojiPanel = MascotStickerPolicy.openingPanel(this.emojiPanel, this.mascotStickers.length > 0);
        }
      }
    };
    this.applySettings();
    // The editor may already be attached by the time the view appears, in which case its override
    // never produced a callback and has to be read off the session here.
    this.applyEditorFace();
    // 扩展能力建面板时还不知道键面；开着手写的竖屏键盘、与记下的键面不同时，在这里补上或去掉多出的行。
    this.syncStripHeight();
    KeyboardSession.shared.onEditorChanged = (): void => {
      // A URI or password field opens in Latin whatever the user last chose, so the face has to
      // follow or it would claim a language the Engine is not composing in.
      this.english = KeyboardSession.shared.isEnglish();
      // 会话在每次获得焦点时重读按键反馈文件，设置应用所做的改动（字母键手势、滑行输入、手写板）就从这里进来，不带偏好修订号；视图的手势跟随这份副本。
      this.feedback = KeyboardSession.shared.feedbackSettings();
      // 带进这个编辑框的组字，要等它的属性说明是否接受预上屏文字后才知道拼写写不写得进输入框。
      this.compositionInField = KeyboardSession.shared.compositionInField();
      this.applyEditorFace();
      this.applyAutomaticCase();
      this.syncCloudClipboardEditor();
    };
    KeyboardSession.shared.onViewPreferencesReloaded = (): void => {
      // The settings page saved while this view was alive; draw what it saved instead of what the view read when it appeared.
      this.applySettings();
      this.applyEditorFace();
      if (!KeyboardFormFactor.isDesktop()) {
        this.resizeTouchPanel();
      }
    };
    KeyboardSession.shared.onReplyContextChanged = (): void => {
      this.replyResults = [];
      this.replyBusy = false;
      // Ink and OCR results belong to the caret and editor where they were produced. Keeping them
      // across a host text/selection change would let an old recognition result land in a new field.
      if (this.hasHandwritingContext()) {
        this.clearHandwriting();
      }
      if (this.surface === SURFACE_REPLY) {
        this.replyNotice = '输入位置已变化，请重新选择回复方式';
      }
    };
    KeyboardSession.shared.onEditorAttached = (): void => {
      // 换输入框时系统不一定先发 inputStop（那会经 `onPanelHidden` 离开），键盘也不一定收起，只来一次新的 inputStart：键盘调整开着就在这里离开，按确认保存（见 `show`），不把遮罩带进新的输入框。
      if (this.surface === SURFACE_GEOMETRY) {
        this.show(SURFACE_NONE);
      }
    };
    KeyboardSession.shared.onPanelHidden = (): void => {
      // 收起键盘、换输入框（detach）或切到别的应用时离开键盘调整，按确认保存（见 `show`）；不离开的话下次显示时遮罩还盖着。
      if (this.surface === SURFACE_GEOMETRY) {
        this.show(SURFACE_NONE);
      }
      // 面板收起时视图不会销毁（aboutToDisappear 不会执行）。还在走的空格长按计时要取消，否则收起后它照样到点开始录音。
      this.cancelSpaceLongPress();
      if (!VoiceRecognitionPolicy.cancelsOnHide(this.touchDesign(), this.voiceBusy, this.voiceFinishing)) {
        return;
      }
      // 按住空格说话时收起：松手事件可能再也收不到，空格键上的「松开结束」不能留到下次显示。
      this.spaceVoiceHeld = false;
      this.spaceVoiceHint = '';
      this.cancelVoice();
    };
    KeyboardSession.shared.onChanged = (state: KeyboardState): void => {
      const wasHandwriting: boolean = KeyboardScheme.usesHandwritingFace(
        this.scheme, this.english, this.symbols, this.localMode);
      // 这次回报的候选盖掉 1 键放进来的常用标点。
      this.nineKeyQuickMarks = [];
      this.candidates = state.candidates;
      this.editing = state.editing;
      this.editingCaret = state.editingCaret;
      this.candidatePage = state.page;
      this.candidatePageCount = state.pageCount;
      this.phrasePrefix = state.phrasePrefix;
      this.compositionInField = state.compositionInField;
      this.spellings = state.spellings;
      this.nineKeySingleCharacter = state.nineKeySingleCharacter;
      this.nineKeyStrokes = state.nineKeyStrokes;
      this.cornerQuotes = state.cornerQuotes;
      this.hollowBrackets = state.hollowBrackets;
      this.enumerationCommaBackslash = state.enumerationCommaBackslash;
      this.enumerationCommaSlash = state.enumerationCommaSlash;
      this.engineScheme = state.scheme;
      this.candidateListOpen = state.candidateListOpen;
      this.prediction = state.prediction;
      this.japanese = state.scheme === JAPANESE_SCHEME && !state.english
        && KeyboardScheme.isNineKeyLayout(this.scheme.touchKeyboardLayout);
      this.shuangpinProfile = state.shuangpinProfile;
      this.localMode = state.localMode;
      this.diagnostic = state.diagnostic;
      if (state.diagnostic.length > 0) {
        // Transient by design: it describes what just happened, and leaving it up would have it
        // still describing that long after the next thing.
        this.clearDiagnosticLater();
      }
      this.english = state.english;
      this.clearHandwritingAfterLeaving(wasHandwriting);
      // After the text changed, which is when the caret may have arrived at a new word or sentence.
      this.applyAutomaticCase();
      if (state.candidates.length === 0
        && !(this.surface === SURFACE_EXPANDED && this.nineKeyPanel())) {
        // 这份列表属于一个已不存在的组合；继续显示它，就是为用户已经完成或放弃的输入提供选项。功能面板和键盘调整与组合无关：用硬件键盘输入组合时面板会让开，组合结束后再回来；键盘调整不因组合结束而关上。全拼九键的三栏面板例外：筛选可能把候选滤空，组字还在，要留着面板让用户撤掉那一笔，组字结束时 nineKeyPanel() 为假再收起。
        if (!this.isTools() && this.surface !== SURFACE_GEOMETRY) {
          this.show(SURFACE_NONE);
        }
      } else if (this.surface === SURFACE_EXPANDED) {
        // 网格列出的是正在输入的组合，它刚刚发生了变化：面板开着时选拼音、退格或改筛选都会换掉候选。
        this.refreshExpanded();
      }
    };
    KeyboardSession.shared.onTypingEffect =
      (effect: TypingEffect, settings: TypingEffectSettings, key: boolean): void => {
        this.showTypingEffect(effect, settings, key);
      };
  }

  /** Draw one typing-effect answer: flash the candidate card for a key, swell the badge in power mode, and show the combo until it ends. */
  private showTypingEffect(effect: TypingEffect, settings: TypingEffectSettings, key: boolean): void {
    const intensity: number = settings.intensity;
    const peak: number = key ? TypingEffectPolicy.flashOpacity(effect, intensity) : 0;
    if (peak > 0) {
      this.effectColor = settings.color;
      this.effectFlash = peak;
      this.getUIContext().animateTo({ duration: settings.flashMillis, curve: Curve.EaseOut }, (): void => {
        this.effectFlash = 0;
      });
    }
    const scale: number = key ? TypingEffectPolicy.badgeScale(effect, intensity) : 1;
    if (scale > 1) {
      this.effectBadgeScale = scale;
      this.getUIContext().animateTo({ duration: settings.flashMillis * 2, curve: Curve.EaseOut }, (): void => {
        this.effectBadgeScale = 1;
      });
    }
    this.effectBadge = TypingEffectPolicy.badge(effect.count);
    // A commit counts nothing, so it neither starts nor extends the idle wait; host-api ends the combo after the same idle time since the last counted key, and the badge goes with it.
    if (!key) {
      return;
    }
    if (this.effectTimer !== -1) {
      clearTimeout(this.effectTimer);
      this.effectTimer = -1;
    }
    if (this.effectBadge.length > 0) {
      this.effectTimer = setTimeout((): void => {
        this.effectTimer = -1;
        this.effectBadge = '';
      }, COMBO_IDLE_RESET_MILLIS);
    }
  }

  /** Whether the combo badge sits on the composition line; without that line it is drawn over the card's corner instead. */
  private comboOnCompositionLine(): boolean {
    return KeyboardFormFactor.isDesktop() && this.settings.candidateShowPreedit;
  }

  aboutToDisappear(): void {
    display.off('change', this.displayChanged);
    KeyboardSession.shared.onChanged = undefined;
    KeyboardSession.shared.onClipboardHistoryChanged = undefined;
    KeyboardSession.shared.onVoiceInputChanged = undefined;
    KeyboardSession.shared.onTypingEffect = undefined;
    if (this.effectTimer !== -1) {
      clearTimeout(this.effectTimer);
      this.effectTimer = -1;
    }
    KeyboardSession.shared.onEditorChanged = undefined;
    KeyboardSession.shared.onViewPreferencesReloaded = undefined;
    KeyboardSession.shared.onReplyContextChanged = undefined;
    KeyboardSession.shared.onPanelHidden = undefined;
    KeyboardSession.shared.onEditorAttached = undefined;
    KeyboardSession.shared.onPanelKey = undefined;
    KeyboardSession.shared.invalidateReplyContext(false);
    this.cancelHoverTip();
    this.cancelBackspace();
    this.cancelHandwritingTimer();
    if (this.diagnosticTimer !== -1) {
      clearTimeout(this.diagnosticTimer);
      this.diagnosticTimer = -1;
    }
    this.cursorDrag.cancel();
    this.cancelSpaceLongPress();
    this.spaceGesture.cancel();
    this.spaceVoiceHeld = false;
    this.glideReset();
    this.dropLetterGestures();
    this.handwritingRecognition.changed();
    this.voiceRecognizer.shutdown();
    KeyboardSession.shared.voiceCancel();
    this.voiceGeneration = 0;
    this.cancelVoiceSilence();
    this.clearVoiceDone();
    // Replies and polish belong to the surface's current editor context. A view can disappear
    // while either request is awaiting the network; invalidate both generations so a late answer
    // cannot reappear when the same view is shown for a different field.
    this.replyGeneration += 1;
    this.replyBusy = false;
    this.replyResults = [];
    this.polishGeneration += 1;
    this.polishBusy = false;
    this.phraseGeneration += 1;
    this.voiceBusy = false;
    this.voiceFinishing = false;
    this.voiceLevel = 0;
    KeyboardSession.shared.setVoiceRecording(false, true);
  }

  /**
   * A tap deletes on release. A hold clears an unfinished composition once, or repeats through the
   * editor when there was no composition; it never crosses that boundary and eats committed text.
   */
  private startBackspace(): void {
    // Counted once per press here, on the way down, never per repeat of a hold.
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.BACKSPACE);
    this.cancelBackspace();
    this.backspaceRepeated = false;
    this.backspaceTimer = setTimeout((): void => {
      const action: BackspaceHoldAction =
        BackspaceHoldPolicy.firstRepeat(KeyboardSession.shared.composing());
      this.backspaceRepeated = true;
      if (action === BackspaceHoldAction.CANCEL_COMPOSITION) {
        this.backspaceTimer = -1;
        // The whole composition, a Korean syllable whose Hanja list is open included, which a single cancel would only close.
        KeyboardSession.shared.discardComposition();
        return;
      }
      KeyboardSession.shared.backspace();
      this.backspaceTimer = setInterval((): void => {
        KeyboardSession.shared.backspace();
      }, BACKSPACE_REPEAT_MILLIS);
    }, BACKSPACE_DELAY_MILLIS);
  }

  private clearDiagnosticLater(): void {
    if (this.diagnosticTimer !== -1) {
      clearTimeout(this.diagnosticTimer);
    }
    this.diagnosticTimer = setTimeout((): void => {
      this.diagnostic = '';
      this.diagnosticTimer = -1;
    }, InputDiagnosticPolicy.DISMISS_DELAY_MILLIS);
  }

  private finishBackspace(): void {
    const repeated: boolean = this.backspaceRepeated;
    this.cancelBackspace();
    if (!repeated) {
      KeyboardSession.shared.backspace();
    }
  }

  private cancelBackspace(): void {
    if (this.backspaceTimer !== -1) {
      clearTimeout(this.backspaceTimer);
      clearInterval(this.backspaceTimer);
      this.backspaceTimer = -1;
    }
    this.backspaceRepeated = false;
  }

  /**
   * 中文模式下中文方案的字母键面画大写，与 Android 和 iOS 一致，输入的仍是小写拼写；英文模式和日语罗马字平时画小写，只有下一键真的会输入大写时才画大写（`typesUppercase`）。规则在 `LetterKeyFacePolicy.displaysUppercase`。
   */
  private letterFace(letter: string): string {
    if (this.koreanFace()) {
      return DubeolsikLayout.jamo(letter, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    if (this.vietnameseFace() || this.tibetanFace()) {
      // 越南语和藏文（威利转写区分大小写）的字母按 Shift 给出的大小写输入，所以键面显示那个大小写，和英文一样。
      return LetterKeyFacePolicy.face(letter, false, false, false, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    // 日语罗马字读键盘自己选的方案，和 `koreanFace` 同理：引擎还没回报方案时键面也要画对。
    return LetterKeyFacePolicy.face(letter, !this.english, this.scheme.engineScheme === 'japanese',
      this.localMode !== 'none', this.typesUppercase());
  }

  /**
   * Shift 亮着时下一键是否真的输入大写，与 `tapLetter` 的判断一致：英文里是大写字母；中文里只有组字中的辅助码会以大写交给 Engine，没有组字时 Shift 加字母仍是小写拼音，键面画大写就是在骗人。读的都是视图的状态，组字开始或结束时键面跟着重绘。
   */
  private typesUppercase(): boolean {
    const held: boolean = this.caseMode !== LetterCaseMode.LOWERCASE;
    return held && (this.english
      || ChineseHelpcodePolicy.entersHelpcode(this.english, held, this.editing, this.engineScheme, this.localMode));
  }

  /** 提示跟随当前选中的方案，切换后立即更新，不等下一次按键的引擎回报。 */
  private letterHint(letter: string): string {
    if (this.scheme.engineScheme !== 'shuangpin') {
      return '';
    }
    return ShuangpinKeyHintPolicy.hint(this.scheme.shuangpinProfile ?? this.shuangpinProfile,
      letter, this.english, 1, this.localMode, this.shuangpinKeyHints);
  }

  /**
   * Whether the letter keys are the Dubeolsik jamo keys: the Korean scheme, with neither English nor a local utility mode taking the letters.
   *
   * Read from the keyboard's own scheme rather than from what the Engine last reported, for the reason the kana grid is: the face has to change the moment Korean is picked, before anything has been typed.
   */
  private koreanFace(): boolean {
    return KoreanCompositionPolicy.selected(this.scheme.engineScheme, this.english, this.localMode);
  }

  /** Whether the Engine last reported a Korean composition under the Korean rules, as opposed to the face the keyboard draws before it has. */
  private koreanComposition(): boolean {
    return KoreanCompositionPolicy.active(this.engineScheme, this.english, this.localMode);
  }

  /** Whether the strip holds the Hanja list of the composing Korean syllable. */
  private koreanHanjaListOpen(): boolean {
    return KoreanCompositionPolicy.hanjaListOpen(this.koreanComposition(), this.candidates.length);
  }

  /** Whether the keys are the Zhuyin Dachen keys: the Zhuyin scheme, with neither English nor a local utility mode taking them, read from the keyboard's own scheme for the reason `koreanFace` is. */
  private zhuyinFace(): boolean {
    return SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english,
      this.localMode) === SchemeTraits.ZHUYIN;
  }

  /** Whether letters go to the Vietnamese composer, which reads their case, read from the keyboard's own scheme for the reason `koreanFace` is. */
  private vietnameseFace(): boolean {
    return SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english,
      this.localMode) === SchemeTraits.VIETNAMESE;
  }

  /** 字母是否交给藏文威利转写组字（区分大小写），和 `koreanFace` 一样读键盘自己的方案。 */
  private tibetanFace(): boolean {
    return SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english,
      this.localMode) === SchemeTraits.TIBETAN;
  }

  /** Whether the Engine last reported a Zhuyin composition under the Zhuyin rules, as opposed to the face the keyboard draws before it has. */
  private zhuyinComposition(): boolean {
    return SchemeCompositionPolicy.rulesScheme(this.engineScheme, this.english, this.localMode)
      === SchemeTraits.ZHUYIN;
  }

  /** Whether the strip holds the list the button on the composition line opens: the Korean syllable's Hanja or the Zhuyin candidates. */
  private openableListShowing(): boolean {
    return this.zhuyinComposition() ? this.candidateListOpen : this.koreanHanjaListOpen();
  }

  /**
   * Whether the touch strip offers 漢 or 選: while a Korean syllable or a Zhuyin conversion composes, its list open or not. A 2in1 candidate window does not draw it, since that machine has the hardware Hanja key (or Down for Zhuyin) and the window is sized to its candidates; see HardwareKeyRouter.routeKorean and routeZhuyin.
   */
  private showsHanjaButton(): boolean {
    return !KeyboardFormFactor.isDesktop()
      && (KoreanCompositionPolicy.convertsHanja(this.koreanComposition(), this.editing)
        || (this.zhuyinComposition() && this.editing.length > 0));
  }

  /**
   * The 漢 button: list the Hanja of the composing syllable, or close the open list and keep the syllable composing. A lone jamo has no Hanja and the Engine leaves it alone, so the strip says why nothing opened, as on iOS. In Zhuyin the same command opens or closes the list over the conversion, the touch counterpart of Down on a hardware keyboard.
   */
  private convertKoreanSyllableToHanja(): void {
    if (!this.showsHanjaButton()) {
      return;
    }
    if (!KeyboardSession.shared.convertHanja()) {
      this.diagnostic = this.zhuyinComposition() ? '还没有可选的字' : '单个字母没有对应的汉字';
      this.clearDiagnosticLater();
    }
  }

  /**
   * A Dachen key. The Engine's editor reads the ASCII key, so that is what is sent; with the list open a digit 1-9 would pick a row instead, so the list is closed first and the key types what its face shows.
   */
  private tapZhuyinKey(key: string): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(key));
    if (ZhuyinLayout.selectsWhileListOpen(key) && KeyboardSession.shared.candidateListOpen()) {
      KeyboardSession.shared.convertHanja();
    }
    KeyboardSession.shared.press(key.charCodeAt(0), false);
  }

  /**
   * Zhuyin: a mark whose ASCII key is a Dachen phonetic or tone key would compose rather than be typed, so the conversion is finished and the mark typed after it, in Chinese while the punctuation switch says so.
   */
  private typeZhuyinMark(symbol: string): void {
    const chinese: string | undefined = ChineseSymbolFaces.face(
      symbol, this.cornerQuotes, this.hollowBrackets,
      this.enumerationCommaBackslash, this.enumerationCommaSlash);
    KeyboardSession.shared.commitThenInsert(
      this.chinesePunctuation && chinese !== undefined ? chinese : symbol);
  }

  private zhuyinKey(key: string): KeyOptions {
    return {
      face: ZhuyinLayout.face(key),
      label: ZhuyinLayout.label(key),
      size: KeyboardMetrics.KEY_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapZhuyinKey(key)
    };
  }

  /** Each of the four Dachen rows, which share the height of the three letter rows. */
  private dachenRowHeight(): number {
    return ZhuyinLayout.rowHeight(this.rowHeight(0) + this.rowHeight(1) + this.rowHeight(2),
      this.rowSpacing());
  }

  /** 是否画笔画键盘：笔画方案，且英文和本地工具模式都没有接管按键；与 zhuyinFace 一样按键盘自己选的方案判断，引擎还没回报时也画对。 */
  private strokeFace(): boolean {
    return SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english,
      this.localMode) === SchemeTraits.STROKE;
  }

  /** 笔画键发送它的字母，组字和候选都归引擎。空组合时的通配键不发送，见 StrokeLayout.sends。 */
  private tapStrokeKey(input: string): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(input));
    if (!StrokeLayout.sends(input, KeyboardSession.shared.composing())) {
      return;
    }
    KeyboardSession.shared.press(input.charCodeAt(0), false);
  }

  private strokeKey(key: StrokeKey): KeyOptions {
    return {
      face: key.face,
      label: key.label,
      hint: key.hint,
      size: KeyboardMetrics.KEY_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapStrokeKey(key.input)
    };
  }

  /** 底排是否用网格面板那一套（「符」代替逗号，逗号等标点在侧栏）：九键网格画着的时候（与 `typingFace` 的九键分支同一条件，英文九键的 123 页也是网格，0 只在这一排），或不在符号层的笔画键盘。 */
  private gridActionRow(): boolean {
    return this.numberPadShowing() || (KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)
      && (!this.symbols || !this.moreSymbols || !this.touchDesign()))
      || (this.strokeFace() && !this.symbols);
  }

  /** 触屏键盘的 123 一律打开数字九宫格（`NineKeyLayout.opensNumberPad`）：英文、本地模式、中文 26 键的长空格样式、日语（罗马字和假名网格）、韩文、越南语、藏文、注音、笔画、手写、14/18 键都在内；#+= 仍是完整的扩展符号页。数字九宫格底行有空格，长空格样式的用户在这一页照样有空格。2in1 的屏幕键盘不打开，理由见 `opensNumberPad`。 */
  private numberPadShowing(): boolean {
    return NineKeyLayout.opensNumberPad(this.symbols, this.moreSymbols, this.touchDesign());
  }

  /** 九宫格是这块键盘自己的字母层：拼音九键、九键双拼和英文九键，123 只是把网格重新标注成数字，返回键印「九键」，底行留着 符。假名网格虽然也是九键布局，数字九宫格对它是借来的。 */
  private ownsNineKeyGrid(): boolean {
    return KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode) && !this.japanese;
  }

  /** 此刻画的是假名网格：字母层，以及 2in1 上假名网格自己的数字层。触屏上 123 打开借来的数字九宫格、#+= 是扩展符号页，都不是假名网格。 */
  private kanaGridShowing(): boolean {
    return this.japanese && (!this.symbols || !this.touchDesign());
  }

  /** 借出数字九宫格的字母层（`NumberPadOrigin`），按 `typingFace` 的分支次序判断：假名网格最先，笔画先于 14/18 键。 */
  private numberPadOrigin(): NumberPadOrigin {
    if (this.japanese) {
      return NumberPadOrigin.KANA;
    }
    if (this.zhuyinFace()) {
      return NumberPadOrigin.ZHUYIN;
    }
    if (this.strokeFace()) {
      return NumberPadOrigin.STROKE;
    }
    if (this.drawsEighteenKey()) {
      return NumberPadOrigin.EIGHTEEN_KEY;
    }
    if (this.drawsFourteenKey()) {
      return NumberPadOrigin.FOURTEEN_KEY;
    }
    return this.koreanFace() ? NumberPadOrigin.KOREAN : NumberPadOrigin.LETTERS;
  }

  /** 数字九宫格底行返回键的键面：自己的九键网格印「九键」，借来的按借出它的字母层印（`NineKeyLayout.numberPadReturnTitle`）。26 键一族按 123 层的字母页读法印「拼音」或「ABC」（`designLayerChinese`），日语罗马字的字母是拉丁字母，印「ABC」。 */
  private numberPadReturnTitle(): string {
    if (this.ownsNineKeyGrid()) {
      return '九键';
    }
    return NineKeyLayout.numberPadReturnTitle(this.numberPadOrigin(),
      this.designLayerChinese() && this.scheme.engineScheme !== 'japanese');
  }

  /** `touch` 是这一键的按下点（键盘根区域坐标），没有时为 null：无障碍点击、没有被记下的触摸。 */
  private tapLetter(letter: string, touch: TouchPoint | null): void {
    if (this.glideSwallowsTap) {
      // The stroke already typed what it meant; see glideSwallowsTap.
      return;
    }
    if (this.helpcodeSwipes.swallows(letter)) {
      // 这次触摸已经上滑或下滑输入了辅助码，ArkUI 随后到达的点击不能再输入小写字母。
      return;
    }
    if (this.letterHints.swallows(letter)) {
      // 该键上的滑动或长按已在手指抬起时输入了角标提示。
      return;
    }
    // The key by its position, so Dubeolsik jamo and a shifted capital count on the same key as the plain letter.
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(letter));
    if (this.koreanFace()) {
      // The Engine reads the jamo from the letter's case, so Shift is sent as an upper-case letter rather than as helpcode.
      const shifted: boolean = this.caseMode !== LetterCaseMode.LOWERCASE;
      KeyboardSession.shared.press(DubeolsikLayout.input(letter, shifted).charCodeAt(0), shifted);
      if (this.caseMode === LetterCaseMode.SHIFTED) {
        this.letterCase.reset();
        this.caseMode = this.letterCase.mode();
      }
      return;
    }
    if (this.vietnameseFace() || this.tibetanFace()) {
      // 越南语组字和藏文威利转写都保留每个字母的大小写，所以 Shift 直接发大写字母本身，而不是当作辅助码。
      const automatic: boolean = this.letterCase.isAutomatic();
      const held: boolean = this.caseMode !== LetterCaseMode.LOWERCASE;
      KeyboardSession.shared.press(
        (held ? letter.toUpperCase() : letter.toLowerCase()).charCodeAt(0), false);
      if (this.caseMode === LetterCaseMode.SHIFTED && !automatic) {
        this.letterCase.reset();
        this.caseMode = this.letterCase.mode();
      }
      return;
    }
    // Shift means two different things depending on what is open. Over a pinyin composition an
    // uppercase letter is helpcode for the Engine, which is why the Engine is told the key was
    // shifted; with nothing composing it is only a capital, and in Chinese mode not even that.
    const held: boolean = this.caseMode !== LetterCaseMode.LOWERCASE;
    // Read before the press, which produces the state change that recomputes the automatic shift.
    const automatic: boolean = this.letterCase.isAutomatic();
    const shifted: boolean = KeyboardSession.shared.entersHelpcode(held)
      || (this.english && held);
    const input: string = ChineseHelpcodePolicy.letterInput(letter, shifted);
    const touchKey: TouchKey | null = this.touchKey(touch, shifted);
    if (touchKey !== null) {
      KeyboardSession.shared.pressTouch(input.charCodeAt(0), shifted, touchKey);
    } else {
      KeyboardSession.shared.press(input.charCodeAt(0), shifted);
    }
    // A one-shot Shift is spent by the letter that follows it; Caps Lock is not. Neither is an
    // automatic Shift: it is not spent but recomputed from the text, which the change this press
    // just caused has already done. Clearing it here would give a capitals-only field one capital.
    if (this.caseMode === LetterCaseMode.SHIFTED && !automatic) {
      this.letterCase.reset();
      this.caseMode = this.letterCase.mode();
    }
  }

  /** 这一键要不要带按下点交给引擎：只有触屏键盘上 26 键双拼的字母和 `;`，不是辅助码大写，键位都已量到（`TouchPointPolicy.usesTouch`）。 */
  private touchKey(touch: TouchPoint | null, shifted: boolean): TouchKey | null {
    if (touch === null || !TouchPointPolicy.usesTouch({
      touchDesign: this.touchDesign(),
      lettersLayer: this.qwertyLettersShowing(),
      engineScheme: this.engineScheme,
      english: this.english,
      localMode: this.localMode,
      shifted: shifted
    })) {
      return null;
    }
    const layout: string | null = this.touchLayout();
    if (layout === null) {
      return null;
    }
    const key: TouchKey = { point: touch, layout: layout };
    return key;
  }

  /** 当前 26 键的键位布局，相对键盘根区域；有键还没量到时为 null。微软双拼画着 `;` 时它也得量到。每键按当前键位现算，见 `TouchPointPolicy.geometryLayout`。 */
  private touchLayout(): string | null {
    return TouchPointPolicy.geometryLayout(this.glideKeys, this.secondRowSemicolon(), this.semicolonKey,
      this.glideOriginX, this.glideOriginY);
  }

  /** 点击时取走这一键的按下点，按这个键现在的位置换成键盘根区域坐标，与 `touchLayout` 同一帧的布局（`PendingTouches.take`）。 */
  private takeTouch(letter: string): TouchPoint | null {
    return this.pendingTouches.take(letter, Date.now(),
      TouchPointPolicy.keyRect(this.glideKeys, this.semicolonKey, letter), this.glideOriginX, this.glideOriginY);
  }

  /** 字母键和 `;` 的按下点，只在触屏键盘上记；取不取用到点击时再按 `touchKey` 定。一律取按下点，不取抬起点。按下时只记相对被按的键的位置，点击时由 `takeTouch` 按键当时的位置换回坐标，按下和点击之间重新排版也不会错位。 */
  private recordLetterTouch(letter: string | undefined, event: TouchEvent): void {
    if (letter === undefined || !this.touchDesign() || !TouchPointPolicy.tracksKey(letter)) {
      return;
    }
    if (event.type === TouchType.Down) {
      if (event.changedTouches.length === 0) {
        return;
      }
      const touch: TouchObject = event.changedTouches[0];
      const key: GlideKeyRect = {
        left: Number(event.target.area.globalPosition.x ?? 0),
        top: Number(event.target.area.globalPosition.y ?? 0),
        width: Number(event.target.area.width),
        height: Number(event.target.area.height)
      };
      this.pendingTouches.down(letter, touch.windowX, touch.windowY, key, Date.now());
    } else if (event.type === TouchType.Cancel) {
      this.pendingTouches.cancel(letter);
    }
  }

  /** Whether a touch on the letter keys may become a glide right now; see GlideTypingPolicy.armed. */
  private glideArmed(): boolean {
    const arming: GlideArming = {
      enabled: this.feedback.glideTyping,
      lettersLayer: this.qwertyLettersShowing(),
      engineScheme: this.engineScheme,
      english: this.english,
      localMode: this.localMode,
      composes: KeyboardSession.shared.composes()
    };
    return GlideTypingPolicy.armed(arming);
  }

  /** Whether typingFace() is drawing the letters layer of the standard 26-key keyboard, with nothing over it: every branch it takes before that one, and every face that relabels those keys. */
  private qwertyLettersShowing(): boolean {
    return this.surface === SURFACE_NONE && !this.symbols && !this.japanese
      && !KeyboardScheme.usesHandwritingFace(this.scheme, this.english, this.symbols, this.localMode)
      && !KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode) && !this.drawsFourteenKey()
      && !this.zhuyinFace() && !this.strokeFace() && !this.koreanFace()
      && !this.vietnameseFace() && !this.tibetanFace();
  }

  /** A letter key was laid out; anything else drawn by flexibleKey is not a glide key. */
  private recordGlideKey(letter: string | undefined, area: Area): void {
    if (letter === undefined) {
      return;
    }
    const index: number = GlideTypingPolicy.letterIndex(letter);
    if (index < 0 && letter !== ';') {
      return;
    }
    const rect: GlideKeyRect = {
      left: Number(area.globalPosition.x ?? 0),
      top: Number(area.globalPosition.y ?? 0),
      width: Number(area.width),
      height: Number(area.height)
    };
    // `;` 不是滑行键，只给按下点的几何用（`touchLayout`、`takeTouch`）。
    if (index < 0) {
      this.semicolonKey = rect;
      return;
    }
    this.glideKeys[index] = rect;
  }

  /** The letter key under a window point, reaching into the half gaps around it as its touch region does. */
  private glideKeyAt(x: number, y: number): number {
    return GlideTypingPolicy.keyAt(this.glideKeys, x, y, this.keySpacing() / 2, this.rowSpacing() / 2);
  }

  /** The touch that changed in this event with the given finger id, or null when this event is about another finger. */
  private changedTouch(event: TouchEvent, finger: number): TouchObject | null {
    for (const touch of event.changedTouches) {
      if (touch.id === finger) {
        return touch;
      }
    }
    return null;
  }

  /** Whether the finger is still among those on the screen. */
  private touching(event: TouchEvent, finger: number): boolean {
    for (const touch of event.touches) {
      if (touch.id === finger) {
        return true;
      }
    }
    return false;
  }

  /**
   * 单个字母键上的一次触摸，用于角标提示和预览气泡，处理方式同 Android 的 `ImeLetterRows.bindLetterGestures`：开启「按键弹出预览」时，按下即在气泡中显示键面；开启「滑动输入符号」时滑动超过阈值，或者不论该设置如何的长按，都会把气泡切换为提示（即使气泡关闭，长按也会显示它），并在手指抬起时输入提示而不是字母。滑行输入接管这次触摸时以它为准，改为输入滑行轨迹。
   */
  private letterHintTouch(letter: string | undefined, hint: string | undefined, event: TouchEvent): void {
    if (letter === undefined || hint === undefined || hint.length === 0) {
      return;
    }
    if (event.type === TouchType.Down) {
      if (event.changedTouches.length === 0) {
        return;
      }
      const touch: TouchObject = event.changedTouches[0];
      const rect: HintKeyRect = {
        left: Number(event.target.area.globalPosition.x ?? 0),
        top: Number(event.target.area.globalPosition.y ?? 0),
        width: Number(event.target.area.width),
        height: Number(event.target.area.height)
      };
      this.helpcodeSwipes.down(touch.id, letter, touch.windowX, touch.windowY,
        this.helpcodeSwipeEnabled());
      this.letterHints.down(touch.id, letter, hint, touch.windowY, rect);
      this.armHintHold(touch.id);
      if (this.feedback.keyPopup) {
        this.showKeyPreview(touch.id, this.letterFace(letter), rect);
      }
    } else if (event.type === TouchType.Move) {
      const moved: TouchObject[] = event.changedTouches.length > 0 ? event.changedTouches : event.touches;
      for (const touch of moved) {
        if (!this.letterHints.tracking(touch.id)) {
          continue;
        }
        if (this.glideActive) {
          this.abandonLetterHint(touch.id);
          continue;
        }
        if (this.helpcodeSwipes.tracking(touch.id)) {
          const helpcode: string = this.helpcodeSwipes.move(touch.id, touch.windowX, touch.windowY);
          if (helpcode.length > 0) {
            this.cancelHintHold(touch.id);
            this.letterHints.cancel(touch.id);
            this.relabelKeyPreview(touch.id, helpcode);
            continue;
          }
        }
        const fired: string | null = this.letterHints.move(touch.id, touch.windowX, touch.windowY,
          this.feedback.swipeSymbols, this.feedback.swipeSymbolsDirection);
        if (fired !== null) {
          this.cancelHintHold(touch.id);
          this.relabelKeyPreview(touch.id, fired);
        }
      }
    } else if (event.type === TouchType.Up) {
      if (event.changedTouches.length === 0) {
        return;
      }
      const finger: number = event.changedTouches[0].id;
      if (this.glideActive) {
        // 该行的 `glideTouch` 接下来会收到这次抬起并发送轨迹。
        this.abandonLetterHint(finger);
        return;
      }
      this.cancelHintHold(finger);
      this.hideKeyPreview(finger);
      const touch: TouchObject = event.changedTouches[0];
      const helpcode: string = this.helpcodeSwipes.up(finger, touch.windowX, touch.windowY);
      if (helpcode.length > 0) {
        this.letterHints.cancel(finger);
        // 抬手事件可能落到手指离开时所在的键；辅助码必须沿用按下时跟踪的原键。
        this.typeHelpcodeSwipe(helpcode);
        return;
      }
      const typed: string | null = this.letterHints.up(finger);
      if (typed !== null) {
        this.typeLetterHint(letter, typed);
      }
    } else if (event.type === TouchType.Cancel) {
      for (const touch of event.changedTouches) {
        this.helpcodeSwipes.cancel(touch.id);
        this.abandonLetterHint(touch.id);
      }
    }
  }

  /** 角标提示按原样输入，与 123 层的字符键输入键面的方式相同：已有组合先按原样上屏（Android 的 `commitNineKeyLiteral`，`COMMIT_RAW`）。它算作对来源键的一次按键。 */
  private typeLetterHint(letter: string, hint: string): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(letter));
    KeyboardSession.shared.commitRaw();
    KeyboardSession.shared.insert(hint);
  }

  /** 竖向滑动辅助码沿已有的 Shift+字母路径送入 Engine；引擎从大写字母本身读取辅助码。 */
  private typeHelpcodeSwipe(letter: string): void {
    const input: string = ChineseHelpcodePolicy.letterInput(letter, true);
    if (input.length === 0) {
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(letter));
    KeyboardSession.shared.press(input.charCodeAt(0), true);
    if (this.caseMode === LetterCaseMode.SHIFTED && !this.letterCase.isAutomatic()) {
      this.letterCase.reset();
      this.caseMode = this.letterCase.mode();
    }
  }

  private armHintHold(finger: number): void {
    this.cancelHintHold(finger);
    const timer: number = setTimeout((): void => {
      this.hintHoldTimers.delete(finger);
      if (this.glideActive) {
        return;
      }
      this.helpcodeSwipes.hold(finger);
      const hint: string | null = this.letterHints.hold(finger);
      const rect: HintKeyRect | null = this.letterHints.rect(finger);
      if (hint !== null && rect !== null) {
        // 无论「按键弹出预览」如何都显示：这是长按已生效的唯一标志。
        this.showKeyPreview(finger, hint, rect);
      }
    }, LetterHintGesture.HOLD_MILLIS);
    this.hintHoldTimers.set(finger, timer);
  }

  private cancelHintHold(finger: number): void {
    const timer: number | undefined = this.hintHoldTimers.get(finger);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.hintHoldTimers.delete(finger);
    }
  }

  /** 不会输入任何内容的被跟踪触摸：已取消，或被滑行输入接管。 */
  private abandonLetterHint(finger: number): void {
    this.cancelHintHold(finger);
    this.hideKeyPreview(finger);
    this.letterHints.cancel(finger);
    this.helpcodeSwipes.cancel(finger);
  }

  /** 只在鸿蒙手机 26 键全拼或双拼已有组字时启用；其它键面继续保留角标滑动。 */
  private helpcodeSwipeEnabled(): boolean {
    return this.touchDesign() && this.qwertyLettersShowing()
      && KeyboardSession.shared.entersHelpcode(true);
  }

  /** 为这根手指在 `rect`（窗口坐标）上方显示气泡；密码框除外，在那里它会把每个字符展示给任何看着屏幕的人。 */
  private showKeyPreview(finger: number, label: string, rect: HintKeyRect): void {
    if (!KeyboardSession.shared.keyPreviewAllowed()) {
      return;
    }
    // 有提示的键都是在行中平分宽度的字母键，所以权重为 1。
    this.keyPreview = KeyPreviewLayout.frame(label, rect.left - this.glideOriginX, rect.top - this.glideOriginY,
      rect.width, 1, this.rootWidth);
    this.keyPreviewFinger = finger;
  }

  /** 滑动触发时，同一个气泡改为显示提示；气泡属于其他手指或已关闭时什么也不做。 */
  private relabelKeyPreview(finger: number, label: string): void {
    const current: KeyPreviewFrame | null = this.keyPreview;
    if (current === null || this.keyPreviewFinger !== finger || current.label === label) {
      return;
    }
    this.keyPreview = {
      label: label,
      left: current.left,
      top: current.top,
      width: current.width
    };
  }

  /** 只在气泡属于这根手指时撤掉它，这样先抬起的手指不会撤掉另一根手指仍按住的键的气泡。 */
  private hideKeyPreview(finger: number): void {
    if (this.keyPreviewFinger === finger) {
      this.keyPreview = null;
      this.keyPreviewFinger = -1;
    }
  }

  /** 键盘上任何位置的每次按下都会在按键自身的处理器之后到达这里：若气泡对应的手指已不在屏幕上，说明它的抬起事件丢给了在其下方被重建的键，气泡随之撤掉。 */
  private pruneKeyPreview(event: TouchEvent): void {
    if (event.type === TouchType.Down && this.keyPreview !== null && !this.touching(event, this.keyPreviewFinger)) {
      this.keyPreview = null;
      this.keyPreviewFinger = -1;
    }
  }

  /** 被跟踪触摸下的键正在被替换（换了层或界面，或视图即将销毁）：丢弃这些触摸、它们的计时器和气泡，与 Android 的 `rebuildKeyRows` 一样。 */
  /** 切进或退出 123 层：丢掉进行中的字母手势；手写的 123 层不带多出的那一行，面板跟着改高度。 */
  private symbolLayerChanged(): void {
    this.dropLetterGestures();
    this.syncStripHeight();
  }

  private dropLetterGestures(): void {
    this.hintHoldTimers.forEach((timer: number): void => clearTimeout(timer));
    this.hintHoldTimers.clear();
    this.letterHints.clear();
    this.helpcodeSwipes.clear();
    this.pendingTouches.clear();
    this.eighteenSwipes.clear();
    this.eighteenSwipeKey = '';
    this.eighteenSwipeLetter = '';
    this.nineKeySwipes.clear();
    this.nineKeySwipeKey = '';
    this.nineKeySwipeDigit = '';
    this.keyPreview = null;
    this.keyPreviewFinger = -1;
  }

  /**
   * Every touch on the three letter rows passes through here, whichever key it lands on: watch a touch that went down on a letter, start a glide once it is over another letter far enough sideways, and send the stroke when it lifts.
   */
  private glideTouch(event: TouchEvent): void {
    if (event.type === TouchType.Down) {
      this.glideDown(event);
    } else if (event.type === TouchType.Move) {
      this.glideMove(event);
    } else if (event.type === TouchType.Up) {
      this.glideUp(event);
    } else if (event.type === TouchType.Cancel) {
      // Whatever took the touch away, a stroke the user did not finish is not sent.
      this.glideReset();
    }
  }

  private glideDown(event: TouchEvent): void {
    if (event.changedTouches.length === 0) {
      return;
    }
    const touch: TouchObject = event.changedTouches[0];
    if (this.glideActive && this.touching(event, this.glideFinger)) {
      // Another finger while the glide runs is ignored until the glide lifts.
      return;
    }
    if (this.glideFinger >= 0 && touch.id !== this.glideFinger && this.touching(event, this.glideFinger)) {
      // A second finger before the stroke became a glide: this is two-thumb typing, so the first touch stays a tap.
      this.glideReset();
      return;
    }
    // A fresh touch: whatever the last glide swallowed, this one is the user's next key.
    this.glideReset();
    this.glideSwallowsTap = false;
    if (event.touches.length > 1 || !this.glideArmed()) {
      return;
    }
    const key: number = this.glideKeyAt(touch.windowX, touch.windowY);
    if (key < 0) {
      return;
    }
    this.glideFinger = touch.id;
    this.glideDownKey = key;
    this.glideDownX = touch.windowX;
    this.glideDownMillis = Date.now();
    this.addGlidePoint(touch.windowX, touch.windowY, 0);
  }

  private glideMove(event: TouchEvent): void {
    if (this.glideFinger < 0) {
      return;
    }
    if (!this.glideActive && event.touches.length > 1) {
      this.glideReset();
      return;
    }
    let current: TouchObject | null = this.changedTouch(event, this.glideFinger);
    if (current === null) {
      for (const touch of event.touches) {
        if (touch.id === this.glideFinger) {
          current = touch;
        }
      }
    }
    if (current === null) {
      return;
    }
    // The samples the system coalesced into this one event, so a quick stroke keeps the corners it turned between frames.
    const now: number = Date.now() - this.glideDownMillis;
    const previous: number = this.glidePoints.length > 0 ? this.glidePoints[this.glidePoints.length - 1].t : 0;
    const coalesced: HistoricalPoint[] = event.getHistoricalPoints()
      .filter((historical: HistoricalPoint): boolean => historical.touchObject.id === this.glideFinger);
    for (let index: number = 0; index < coalesced.length; index++) {
      this.addGlidePoint(coalesced[index].touchObject.windowX, coalesced[index].touchObject.windowY,
        GlideTypingPolicy.coalescedMillis(previous, now, index, coalesced.length));
    }
    this.addGlidePoint(current.windowX, current.windowY, Math.max(previous, now));
    if (!this.glideActive && GlideTypingPolicy.starts(this.glideDownKey,
      this.glideKeyAt(current.windowX, current.windowY), this.glideDownX, current.windowX,
      GlideTypingPolicy.keyWidth(this.glideKeys))) {
      this.glideActive = true;
      this.glideSwallowsTap = true;
    }
    if (this.glideActive) {
      this.drawGlideTrail();
    }
  }

  private glideUp(event: TouchEvent): void {
    if (this.glideFinger < 0) {
      return;
    }
    const touch: TouchObject | null = this.changedTouch(event, this.glideFinger);
    if (touch === null) {
      // Some other finger lifted.
      return;
    }
    if (!this.glideActive) {
      // A tap, which the key's own click types.
      this.glideReset();
      return;
    }
    this.addGlidePoint(touch.windowX, touch.windowY, Date.now() - this.glideDownMillis);
    const request: string | null = GlideTypingPolicy.request(this.glideKeys, this.glidePoints);
    this.glideReset();
    if (request !== null) {
      // A stroke the Engine declines (handled false) is dropped: its keys are never typed in its place.
      KeyboardSession.shared.glide(request);
    }
  }

  /** One sample of the watched finger, `millis` after it went down. */
  private addGlidePoint(x: number, y: number, millis: number): void {
    if (this.glidePoints.length >= GlideTypingPolicy.BUFFER_POINTS) {
      this.glidePoints = GlideTypingPolicy.downsample(this.glidePoints, GlideTypingPolicy.BUFFER_POINTS / 2);
    }
    const point: GlidePoint = { x: x, y: y, t: millis };
    this.glidePoints.push(point);
  }

  /** Forget the watched touch and take the trail down; glideSwallowsTap is left for the next touch-down to clear. */
  private glideReset(): void {
    this.glideFinger = -1;
    this.glideDownKey = -1;
    this.glidePoints = [];
    this.glideActive = false;
  }

  /** The stroke so far as one smooth line in the accent colour: quadratic segments through the midpoints of the samples, so the corners between them do not show. */
  private drawGlideTrail(): void {
    const context: CanvasRenderingContext2D = this.glideContext;
    context.clearRect(0, 0, context.width, context.height);
    const points: GlidePoint[] = this.glidePoints;
    if (points.length < 2) {
      return;
    }
    const left: number = this.glideOriginX;
    const top: number = this.glideOriginY;
    context.strokeStyle = this.skin.accent;
    context.lineWidth = GLIDE_TRAIL_WIDTH_VP;
    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.beginPath();
    context.moveTo(points[0].x - left, points[0].y - top);
    for (let index: number = 1; index < points.length - 1; index++) {
      context.quadraticCurveTo(points[index].x - left, points[index].y - top,
        (points[index].x + points[index + 1].x) / 2 - left, (points[index].y + points[index + 1].y) / 2 - top);
    }
    context.lineTo(points[points.length - 1].x - left, points[points.length - 1].y - top);
    context.stroke();
  }

  /** 是否为触屏键盘，它绘制设计稿中的工具栏、候选栏、面板和按键；2in1 保留原有的候选窗和屏幕键盘。 */
  private touchDesign(): boolean {
    return !KeyboardFormFactor.isDesktop();
  }

  @Styles
  keyIdleStyle() {
    .brightness(1)
    .scale({ x: 1, y: 1 })
  }

  @Styles
  keyPressedStyle() {
    .brightness(this.keyPressedBrightness)
    .scale({ x: this.keyPressedScale, y: this.keyPressedScale })
  }

  private keyFaceHasCornerHint(options: KeyOptions): boolean {
    return options.cornerHint !== undefined && options.cornerHint.length > 0;
  }

  @Builder
  keyFace($$: KeyOptions) {
    Stack({ alignContent: Alignment.TopEnd }) {
      Column() {
        if ($$.icon !== undefined && $$.icon.length > 0) {
          KeyboardLineIcon({
            d: $$.icon ?? '',
            iconSize: KEY_ICON_VP,
            stroke: KeyboardIconPaths.KEY_STROKE,
            color: $$.accentIcon === true ? this.skin.accent
              : this.skin.keyLabelColor($$.emphasized, $$.special === true)
          })
        } else {
          Text($$.letter !== undefined ? this.letterFace($$.letter)
            : this.nineKeySwipeFace($$.nineKeyInput, this.eighteenSwipeFace($$.swipeLetters, $$.face)))
            .fontSize($$.size)
            .minFontSize(KEY_HINT_FONT_SIZE)
            .maxFontSize($$.size)
            .maxFontScale(KEY_MAX_FONT_SCALE)
            // 有角标时字母只能用角标下面剩的高度，按高度约束缩字（LAYOUT_CONSTRAINT_FIRST）。没有角标时按单行缩字（MAX_LINES_FIRST）：「123」「发送」这类功能键在收窄的键盘里比键帽宽，高度约束优先只看高度，不缩字，字会被截掉；按单行优先才会在 10 到原字号之间缩到放得下。
            .heightAdaptivePolicy(this.keyFaceHasCornerHint($$) ? TextHeightAdaptivePolicy.LAYOUT_CONSTRAINT_FIRST
              : TextHeightAdaptivePolicy.MAX_LINES_FIRST)
            .layoutWeight(this.keyFaceHasCornerHint($$) ? 1 : 0)
            .fontWeight($$.fontWeight ?? 400)
            .maxLines(1)
            .fontColor(this.skin.keyLabelColor($$.emphasized, $$.special === true))
            .fontFamily(this.skin.monospaced ? 'monospace' : undefined)
            .textAlign(TextAlign.Center)
            .width('100%')
        }
        if ($$.hint !== undefined && $$.hint.length > 0) {
          Text($$.hint)
            .fontSize(KEY_HINT_FONT_SIZE)
            .maxFontScale(KEY_MAX_FONT_SCALE)
            .minFontSize(6)
            .maxFontSize(KEY_HINT_FONT_SIZE)
            .heightAdaptivePolicy(TextHeightAdaptivePolicy.LAYOUT_CONSTRAINT_FIRST)
            .height(KEY_HINT_LINE_HEIGHT_VP)
            .maxLines(1)
            .fontColor(this.skin.accent)
            .textAlign(TextAlign.Center)
            .width('100%')
        }
      }
      .justifyContent(FlexAlign.Center)
      .width('100%')
      .height('100%')
      // 角标有独立的顶部区域，字母与双拼提示只在下方剩余高度内排版。
      .padding({ top: this.keyFaceHasCornerHint($$)
        ? CORNER_HINT_TOP_VP + KEY_HINT_LINE_HEIGHT_VP : 0 })
      if (this.keyFaceHasCornerHint($$)) {
        Text($$.cornerHint)
          .fontSize(KEY_HINT_FONT_SIZE)
          .maxFontScale(KEY_MAX_FONT_SCALE)
          .height(KEY_HINT_LINE_HEIGHT_VP)
          .maxLines(1)
          .fontColor(this.skin.secondary)
          .margin({ top: CORNER_HINT_TOP_VP, right: CORNER_HINT_RIGHT_VP })
          .hitTestBehavior(HitTestMode.None)
      }
    }
    .width('100%')
    .height('100%')
    // 键面只负责画，不接收触摸：触摸归外层的键，由它的 `responseRegion` 决定。
    .hitTestBehavior(HitTestMode.None)
  }

  /** A key that shares the row's spare width equally with its siblings. */
  @Builder
  flexibleKey($$: KeyOptions) {
    Column() {
      this.keyFace($$)
    }
    .accessibilityText($$.letter === undefined ? ($$.label ?? $$.face) : this.letterLabel($$.letter))
    .layoutWeight(1)
    .height('100%')
    .backgroundColor(this.skin.keySurfaceBackground($$.emphasized, $$.special === true))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onAreaChange((_old: Area, now: Area): void => {
      if ($$.duplicate !== true) {
        this.recordGlideKey($$.letter, now);
      }
    })
    .onTouch((event: TouchEvent): void => {
      this.recordLetterTouch($$.letter, event);
      this.letterHintTouch($$.letter, $$.cornerHint, event);
    })
    .onClick($$.action)
  }

  /** 宽度固定的键，这样 Shift 和删除键无论该行有什么都保持同样大小；在触屏键盘的设计行上则改为占该行固定的份额（`share`）。 */
  @Builder
  fixedKey($$: KeyOptions) {
    Column() {
      this.keyFace($$)
    }
    .accessibilityText($$.letter === undefined ? ($$.label ?? $$.face) : this.letterLabel($$.letter))
    .width($$.width)
    // 权重会覆盖沿行方向的宽度；为 0 时由宽度决定。
    .layoutWeight($$.share ?? 0)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .backgroundColor(this.skin.keySurfaceBackground($$.emphasized, $$.special === true))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion($$.hitRegion ?? this.keyHitRegion())
    .onClick($$.action)
  }

  /**
   * Loads one group of the catalog, a page at a time until the Engine says it is complete.
   *
   * The whole group is gathered before it is shown rather than paged in the panel: a category is a
   * few hundred entries at most, and a grid that grows while a finger is on it is worse than a
   * short wait. validatePage is the ported check that a page which advances nowhere while claiming
   * to be incomplete does not loop this forever.
   */
  private loadEmoji(group: string): void {
    this.emojiPanel = EMOJI_PANEL;
    this.emojiUnicodeGroup = group;
    this.loadEmojiCatalog(EMOJI_PANEL, group);
  }

  private loadEmojiCatalog(category: string, group: string, parent: string = ''): void {
    this.emojiGroup = group;
    this.emojiShowingRecents = false;
    this.resetEmojiSearch();
    this.emojiItems = this.collectEmojiCatalog(category, group, parent)
      .map((item: EmojiItem): string => item.text);
  }

  /** 打开颜文字页或符号页的一个标签：内置标签向 Engine 分页读取，插件标签直接显示清单里的条目。 */
  private loadEmojiTab(category: string, tab: EmojiPanelGroupTab): void {
    this.emojiPanelTab = tab.key;
    if (!tab.plugin) {
      this.loadEmojiCatalog(category, tab.catalogGroup, tab.catalogParent);
      return;
    }
    this.emojiShowingRecents = false;
    this.resetEmojiSearch();
    // 插件条目没有逐项的注释，组的关键词只用于搜索：这里不写 `emojiKeywords`，与插件条目同字的内置符号保留自己的名字。
    this.emojiItems = tab.items.map((item: EmojiItem): string => item.text);
  }

  /** 打开一页时选中它的第一个标签；一个标签都没有时照旧不带分组读取目录。 */
  private loadFirstEmojiTab(category: string, tabs: EmojiPanelGroupTab[]): void {
    if (tabs.length > 0) {
      this.loadEmojiTab(category, tabs[0]);
      return;
    }
    this.emojiPanelTab = '';
    this.loadEmojiCatalog(category, '', '');
  }

  /**
   * 已安装的符号集插件组。手机、平板和 2in1 的设置页都有插件页，`symbol_set_packs` 每种形态都声明，所以每种形态的面板都读。
   */
  private pluginSymbolGroups(): PluginSymbolGroup[] {
    return KeyboardSession.shared.pluginSymbolGroups();
  }

  private collectEmojiCatalog(category: string, group: string, parent: string): EmojiItem[] {
    const collected: EmojiItem[] = [];
    let offset: number = 0;
    for (let page: number = 0; page < EMOJI_MAX_PAGES; page++) {
      const result: EmojiPage | null =
        KeyboardSession.shared.emojiCatalogPage(category, group, offset, EMOJI_PAGE_SIZE, parent);
      if (result === null) {
        break;
      }
      // Each row is checked as it is read and the page as a whole before it is trusted; a row the
      // catalog cannot honour is not worth showing, and a bad page would loop this forever.
      const items: EmojiItem[] = [];
      try {
        for (const row of result.items) {
          items.push(EmojiCatalogModel.item(row.text, row.annotation, row.group));
        }
        EmojiCatalogModel.validatePage(items, offset, EMOJI_PAGE_SIZE, result.next_offset,
          result.complete);
      } catch (error) {
        break;
      }
      for (const item of items) {
        collected.push(item);
        if (item.annotation.length > 0) {
          this.emojiKeywords.set(item.text, item.annotation);
        }
      }
      offset = result.next_offset;
      if (result.complete) {
        break;
      }
    }
    return collected;
  }

  /** What the grid shows: the search results while something is typed, the chosen group otherwise. */
  private visibleEmojiItems(): string[] {
    return this.emojiQuery.length > 0 ? this.emojiResults : this.emojiItems;
  }

  /** The clipboard entries the search leaves; Windows matches the text without regard to case. */
  private visibleClips(): ClipboardHistoryItem[] {
    if (this.emojiQuery.length === 0) {
      return this.clips;
    }
    return this.clips.filter((entry: ClipboardHistoryItem): boolean =>
      EmojiPanelKeyPolicy.matches(entry.text, entry.text, this.emojiQuery));
  }

  private resetEmojiSearch(): void {
    this.emojiQuery = '';
    this.emojiResults = [];
    this.emojiSelected = 0;
    this.cancelHoverTip();
  }

  /**
   * The pointer entered or left an item. The tooltip waits until the pointer has rested for `EMOJI_TOOLTIP_DELAY_MS` and goes as soon as it leaves, like `ArmTooltip` and `CancelTooltip` on Windows. A phone has no pointer, so it never arms one.
   */
  private hoverItem(key: string, tip: string, isHover: boolean): void {
    if (!KeyboardFormFactor.isDesktop()) {
      return;
    }
    if (!isHover) {
      if (this.hoverTipKey === key) {
        this.cancelHoverTip();
      }
      return;
    }
    this.cancelHoverTip();
    if (tip.length === 0) {
      return;
    }
    this.hoverTimer = setTimeout((): void => {
      this.hoverTimer = -1;
      this.hoverTip = tip;
      this.hoverTipKey = key;
    }, EMOJI_TOOLTIP_DELAY_MS);
  }

  private cancelHoverTip(): void {
    if (this.hoverTimer >= 0) {
      clearTimeout(this.hoverTimer);
      this.hoverTimer = -1;
    }
    this.hoverTipKey = '';
    this.hoverTip = '';
  }

  /** An emoji cell's hover name; nothing when the only name there is would be the emoji itself. */
  private emojiHoverName(item: string): string {
    const name: string = EmojiPanelTooltipPolicy.displayName(this.emojiKeywords.get(item) ?? '', item);
    return name === item ? '' : name;
  }

  /**
   * Searches the open tab across all of its groups, the way a Windows detail page searches every group on it.
   *
   * The whole tab is read once and kept, so each key filters in memory rather than asking the Engine again.
   */
  private searchEmoji(query: string): void {
    this.emojiQuery = query;
    this.emojiSelected = 0;
    if (query.length === 0 || this.emojiPanel === CLIPBOARD_PANEL) {
      this.emojiResults = [];
      return;
    }
    let index: EmojiItem[] | undefined = this.emojiSearchIndex.get(this.emojiPanel);
    if (index === undefined) {
      let pages: EmojiItem[][] = [];
      if (this.emojiPanel === EMOJI_PANEL) {
        pages = EmojiCatalogModel.categories().map((category: EmojiCategory): EmojiItem[] =>
          this.collectEmojiCatalog(EMOJI_PANEL, category.group, ''));
      } else if (this.emojiPanel === KAOMOJI_PANEL) {
        pages = this.emojiKaomojiTabs.filter((tab: EmojiPanelGroupTab): boolean => !tab.plugin)
          .map((tab: EmojiPanelGroupTab): EmojiItem[] =>
            this.collectEmojiCatalog(KAOMOJI_PANEL, tab.catalogGroup, ''));
      } else {
        pages = this.emojiSymbolTabs.filter((tab: EmojiPanelGroupTab): boolean => !tab.plugin)
          .map((tab: EmojiPanelGroupTab): EmojiItem[] =>
            this.collectEmojiCatalog(SYMBOLS_PANEL, '', tab.catalogParent));
      }
      const flattened: EmojiItem[] = [];
      for (const page of pages) {
        for (const item of page) {
          flattened.push(item);
        }
      }
      index = flattened;
      this.emojiSearchIndex.set(this.emojiPanel, index);
    }
    // 缓存里只有内置目录；插件条目取自这次打开时读到的标签并接在后面，装上或卸下插件后重开面板即可搜到最新内容。
    const tabs: EmojiPanelGroupTab[] = this.emojiPanel === KAOMOJI_PANEL ? this.emojiKaomojiTabs
      : this.emojiPanel === SYMBOLS_PANEL ? this.emojiSymbolTabs : [];
    const searched: EmojiItem[] = index.concat(PluginSymbolGroupPolicy.pluginItems(tabs));
    const results: string[] = [];
    for (const item of searched) {
      if (EmojiPanelKeyPolicy.matches(item.text, item.annotation, query) && !results.includes(item.text)) {
        results.push(item.text);
        if (results.length === EMOJI_SEARCH_LIMIT) {
          break;
        }
      }
    }
    this.emojiResults = results;
  }

  /** A hardware key offered to the open 2in1 panel; false leaves it to the editor. */
  private panelKey(key: EmojiPanelKey): boolean {
    const clipboard: boolean = this.emojiPanel === CLIPBOARD_PANEL;
    const stickers: boolean = this.emojiPanel === STICKER_PANEL;
    const cloud: boolean = clipboard && this.cloudTabActive();
    const clips: string[] = !clipboard ? []
      : cloud ? this.visibleCloudClips().map((entry: CloudClipboardItem): string => entry.text)
        : this.visibleClips().map((entry: ClipboardHistoryItem): string => entry.text);
    const items: string[] = clipboard || stickers ? [] : this.visibleEmojiItems();
    const decision: EmojiPanelKeyDecision = EmojiPanelKeyPolicy.decide(key,
      clipboard ? clips.length : stickers ? this.mascotStickers.length : items.length,
      clipboard ? 1 : stickers ? MascotStickerPolicy.columns(this.mascotStickerGridWidth) : EMOJI_COLUMNS,
      this.emojiSelected, this.emojiQuery);
    if (decision.action === EmojiPanelKeyAction.NONE) {
      return false;
    }
    if (decision.action === EmojiPanelKeyAction.CLOSE) {
      this.leaveSurface(DesktopSurface.EMOJI);
    } else if (decision.action === EmojiPanelKeyAction.SEARCH) {
      // 表情包不做搜索；键照样由面板收下，不落到面板后面的输入框里。
      if (!stickers) {
        this.searchEmoji(decision.query);
        this.revealEmojiSelection(clipboard);
      }
    } else if (decision.action === EmojiPanelKeyAction.ACTIVATE) {
      if (cloud) {
        this.insertCloudClip(clips[decision.index]);
      } else if (clipboard) {
        KeyboardSession.shared.insert(clips[decision.index]);
      } else if (stickers) {
        this.sendMascotSticker(this.mascotStickers[decision.index]);
      } else {
        this.selectEmoji(items[decision.index]);
      }
    } else {
      this.emojiSelected = decision.index;
      this.revealEmojiSelection(clipboard);
    }
    return true;
  }

  private revealEmojiSelection(clipboard: boolean): void {
    try {
      (clipboard ? this.clipListScroller : this.emojiGridScroller)
        .scrollToIndex(this.emojiSelected, false, ScrollAlign.AUTO);
    } catch (error) {
      KeyboardLog.error(`emoji panel scroll unavailable: ${describeError(error)}`);
    }
  }

  private showEmojiPanel(panel: string): void {
    // 底栏和标签行都要知道有没有「水杉娘」这一页，所以无论打开哪一页都先读一次（读过就用缓存）。
    this.loadMascotStickers();
    if (panel === STICKER_PANEL && this.mascotStickers.length === 0) {
      // 没跑 stage-resources.sh 打出的包里没有表情包，上次停在这一页时回到表情页。
      this.showEmojiPanel(EMOJI_PANEL);
      return;
    }
    this.emojiPanel = panel;
    this.resetEmojiSearch();
    if (panel === STICKER_PANEL) {
      return;
    }
    if (panel === CLIPBOARD_PANEL) {
      this.clips = this.clipboardEnabled ? KeyboardSession.shared.clipboardHistory() : [];
      this.clipboardNotice = this.clipboardEnabled ? '' : CLIPBOARD_HISTORY_OFF;
      this.confirmingClear = false;
      this.openCloudClipboard();
      return;
    }
    if (panel === EMOJI_PANEL) {
      this.emojiRecents = KeyboardSession.shared.emojiRecents();
      if (this.emojiRecents.length > 0) {
        this.showEmojiRecents();
      } else {
        this.loadEmoji(this.emojiUnicodeGroup);
      }
      return;
    }
    if (panel === KAOMOJI_PANEL) {
      this.emojiKaomojiTabs = PluginSymbolGroupPolicy.kaomojiTabs(
        KeyboardSession.shared.emojiGroups(KAOMOJI_PANEL), this.pluginSymbolGroups());
      this.loadFirstEmojiTab(KAOMOJI_PANEL, this.emojiKaomojiTabs);
      return;
    }
    this.emojiSymbolTabs = PluginSymbolGroupPolicy.symbolTabs(
      KeyboardSession.shared.emojiSymbolGroups(), this.pluginSymbolGroups());
    this.loadFirstEmojiTab(SYMBOLS_PANEL, this.emojiSymbolTabs);
  }

  private showEmojiRecents(): void {
    this.emojiShowingRecents = true;
    this.resetEmojiSearch();
    this.emojiRecents = KeyboardSession.shared.emojiRecents();
    this.emojiItems = this.emojiRecents.slice();
  }

  private selectEmoji(text: string): void {
    KeyboardSession.shared.insert(text);
    this.emojiRecents = KeyboardSession.shared.recordEmojiRecent(text);
    if (this.emojiShowingRecents) {
      this.emojiItems = this.emojiRecents.slice();
    }
  }

  /** 读入随包的水杉娘表情包。`MascotStickerStore` 缓存读到的结果，读不到时缓存的是空表，所以每次打开面板调用它都不会重读文件。 */
  private loadMascotStickers(): void {
    if (this.mascotStickers.length > 0) {
      return;
    }
    const stickers: MascotSticker[] = KeyboardSession.shared.mascotStickers.list();
    if (stickers.length > 0) {
      this.mascotStickerSource = new MascotStickerSource(stickers);
      this.mascotStickers = stickers;
    }
  }

  /**
   * 点一张水杉娘：先按原样上屏正在拼写的内容（与打开表情面板时的 `commitRaw` 相同），再把这张图复制进系统剪贴板，并在候选栏提示怎么粘贴，或者为什么没复制成。输入法框架没有把图片交给输入框的接口，见 `MascotStickerStore`。不记入表情的最近使用。
   */
  private sendMascotSticker(sticker: MascotSticker): void {
    if (KeyboardSession.shared.composing()) {
      KeyboardSession.shared.commitRaw();
    }
    KeyboardSession.shared.mascotStickers.copy(sticker).then((notice: string): void => {
      this.diagnostic = notice;
      this.clearDiagnosticLater();
    });
  }

  private clearEmojiRecents(): void {
    if (KeyboardSession.shared.clearEmojiRecents()) {
      this.emojiRecents = [];
      if (this.emojiShowingRecents) {
        this.emojiItems = [];
      }
    }
  }

  private openEmojiSurface(): void {
    // 选中项直接进入编辑框，所以已有组合要先按原样上屏，否则表情会落在它前面（Android 的 `showEmojiPicker` 发送 `COMMIT_RAW`）。
    if (this.surface !== SURFACE_EMOJI && KeyboardSession.shared.composing()) {
      KeyboardSession.shared.commitRaw();
    }
    this.toggle(SURFACE_EMOJI);
    if (this.surface === SURFACE_EMOJI) {
      // 最近使用标签位于每一页的底行，所以无论重新打开的是哪一页都会读取它们。
      this.emojiRecents = KeyboardSession.shared.emojiRecents();
      // 与 iOS 每次新建的表情面板一样，有水杉娘表情包时每次打开都直接显示它；读不到时照旧回到上次那一页。
      this.loadMascotStickers();
      this.showEmojiPanel(MascotStickerPolicy.openingPanel(this.emojiPanel, this.mascotStickers.length > 0));
    }
  }

  /** 长按底行的 123：在字母层直接打开符号面板，与点「符」键相同；符号层里那个键是返回，不响应长按。 */
  private openSymbolsFromLayerKey(): void {
    if (this.symbols) {
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
    this.openSymbolSurface();
  }

  private openSymbolSurface(): void {
    if (KeyboardFormFactor.isDesktop()) {
      return;
    }
    // 与表情界面相同：符号直接进入编辑框，所以组合先按原样上屏（Android 的 `showSymbolPanel`）。
    if (KeyboardSession.shared.composing()) {
      KeyboardSession.shared.commitRaw();
    }
    this.symbolCategory = 0;
    this.symbolLocked = false;
    this.show(SURFACE_SYMBOLS);
  }

  /** 换分类时网格回到顶部，否则停在上一类滚到的位置，看到的是新分类的中间。 */
  private selectSymbolCategory(index: number): void {
    this.symbolCategory = index;
    this.symbolGridScroller.scrollToIndex(0);
  }

  private selectSymbol(symbol: string): void {
    if (!SymbolPanelPolicy.isValidSymbol(symbol)) {
      return;
    }
    KeyboardSession.shared.insert(symbol);
    const selection = SymbolPanelPolicy.select(this.symbolLocked);
    if (selection.returnToKeyboard) {
      this.show(SURFACE_NONE);
    }
  }

  /** 长按弹出的字母：九键格子的 `NineKeyLayout.holdOptions`，或 14/18 键的组码键表。选中后都是先结束组字再上屏（`selectNineKeyHoldOption`）。 */
  private openKeyHold(options: string[]): void {
    if (options.length === 0 || this.symbols) {
      return;
    }
    this.nineKeyHoldOptions = options;
    this.show(SURFACE_NINE_KEY_HOLD);
  }

  private selectNineKeyHoldOption(option: string): void {
    if (!this.nineKeyHoldOptions.includes(option)) {
      return;
    }
    this.nineKeyHoldOptions = [];
    this.show(SURFACE_NONE);
    KeyboardSession.shared.finishComposition();
    KeyboardSession.shared.insert(option);
  }

  private selectScheme(candidate: SchemeDefinition): boolean {
    // The composition belongs to the scheme that was current when it was typed; carrying it into a
    // different one would reinterpret letters that were never meant that way.
    this.leaveComposition();
    if (this.hasHandwritingContext()) {
      this.clearHandwriting();
    }
    // Every field one scheme decides moves together, including the record of the Chinese scheme a
    // Japanese choice replaces; writing the Engine's scheme alone is what loses it.
    const changes: Map<string, Object> = KeyboardSession.shared.schemeChanges(candidate);
    // 共享校验会拒绝未启用的已选方案。选择器只提供已启用的方案；但面板的「手写」磁贴可以在用户于选择器中关闭手写时选中它，所以选中时一并启用。
    const enabled: string[] = this.settings.enabledSchemes.includes(candidate.preferenceId)
      ? this.settings.enabledSchemes : this.settings.enabledSchemes.concat([candidate.preferenceId]);
    const schemes: TouchKeyboardSchemes = {
      enabled: enabled, selected: candidate.preferenceId
    };
    changes.set('touch_keyboard_schemes', schemes);
    if (!KeyboardSession.shared.changePreferences(changes)) {
      return false;
    }
    this.schemeBeforeHandwriting = null;
    if (enabled !== this.settings.enabledSchemes) {
      this.settings = KeyboardSession.shared.preferences();
    }
    KeyboardSession.shared.useScheme(KeyboardScheme.engineSchemeOf(candidate),
      candidate.shuangpinProfile);
    if (this.english) {
      // Picking a Chinese scheme is a request to compose Chinese with it.
      this.english = KeyboardSession.shared.toggleLanguage();
    }
    this.scheme = candidate;
    this.nineKey = KeyboardScheme.isNineKeyLayout(candidate.touchKeyboardLayout);
    this.nineKeyShuangpinFaces = candidate.preferenceId === 'nine_key_shuangpin'
      ? KeyboardSession.shared.nineKeyShuangpinFaces().slice() : [];
    this.fourteenKey = this.offersFourteenKey(candidate);
    this.eighteenKey = this.offersEighteenKey(candidate);
    // The kana grid belongs to the scheme, not to what the Engine last reported: the Engine only
    // says so once something has been typed, and the face has to change before that.
    this.japanese = candidate.engineScheme === 'japanese' && candidate.touchKeyboardLayout
      === 'nine_key';
    KeyboardSession.shared.setKeyGrid(this.keyGrid());
    this.show(SURFACE_NONE);
    return true;
  }

  /** 键盘是否处于手写状态：方案为「手写」，且英文没有接管按键。 */
  private handwritingSelected(): boolean {
    return !this.english && this.scheme.preferenceId === KeyboardScheme.HANDWRITING.preferenceId;
  }

  /**
   * 功能面板的「手写」磁贴：切换到手写板，或切回该磁贴切换前的方案和中英状态（#9703）。Android、iOS 的磁贴也这样来回切换；若键盘是经其他途径进入手写的，切回的是选择器提供的第一个其他方案（`KeyboardScheme.handwritingReturn`）。原先只记方案不记英文，从英文键盘进入手写再关掉会落到中文。
   */
  private toggleHandwritingScheme(): void {
    if (this.handwritingSelected()) {
      const before: SchemeDefinition | null = this.schemeBeforeHandwriting;
      const english: boolean = this.englishBeforeHandwriting;
      const back: SchemeDefinition = KeyboardScheme.handwritingReturn(before, this.offeredSchemes());
      if (this.selectScheme(back) && english && before !== null
        && back.preferenceId === before.preferenceId && !this.english) {
        this.toggleTouchLanguage();
      }
      return;
    }
    const previous: SchemeDefinition = this.scheme;
    const previousEnglish: boolean = this.english;
    const remembered: SchemeDefinition | null = this.schemeBeforeHandwriting;
    const rememberedEnglish: boolean = this.englishBeforeHandwriting;
    if (!this.selectScheme(KeyboardScheme.HANDWRITING)) {
      return;
    }
    if (previous.preferenceId !== KeyboardScheme.HANDWRITING.preferenceId) {
      this.schemeBeforeHandwriting = previous;
      this.englishBeforeHandwriting = previousEnglish;
    } else {
      // 英文盖在手写上时点磁贴只是退出英文，回到手写板；经磁贴进入手写前的那个方案仍然有效。
      this.schemeBeforeHandwriting = remembered;
      this.englishBeforeHandwriting = rememberedEnglish;
    }
  }

  /**
   * End the composition before the scheme or mode under it changes. A spelling is thrown away, since its letters mean nothing to the next scheme; an open Korean syllable, a Zhuyin conversion or a Vietnamese word is already the user's text (`commits_on_blur`), so it is committed instead.
   */
  private leaveComposition(): void {
    if (SchemeTraits.commitsOnBlur(KeyboardSession.shared.rulesScheme())) {
      KeyboardSession.shared.finishComposition();
    } else {
      KeyboardSession.shared.cancel();
    }
  }

  private toggleSymbolLayer(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LAYER);
    const wasHandwriting: boolean = KeyboardScheme.usesHandwritingFace(
      this.scheme, this.english, this.symbols, this.localMode);
    this.symbols = !this.symbols;
    // 进入符号时总是打开 123 层；回到字母层后也就没有需要记住的 #+=。
    this.moreSymbols = false;
    this.clearHandwritingAfterLeaving(wasHandwriting);
  }

  private toggleTouchLanguage(): void {
    const wasHandwriting: boolean = KeyboardScheme.usesHandwritingFace(
      this.scheme, this.english, this.symbols, this.localMode);
    this.english = KeyboardSession.shared.toggleLanguage();
    this.clearHandwritingAfterLeaving(wasHandwriting);
    this.letterCase.reset();
    this.caseMode = this.letterCase.mode();
  }

  /**
   * Shift the letter face when the editor asked for capitals and the caret is where they belong.
   *
   * A Shift the user pressed is theirs: EnglishLetterCaseState refuses to touch caps lock and
   * reports whether anything changed, so this only redraws when it did. Asked on attach and after
   * every text change, those being the two moments the answer can differ — a field that opens empty
   * wants a capital, and so does the character after a full stop.
   */
  private applyAutomaticCase(): void {
    if (!KeyboardSession.shared.capitalizes()) {
      return;
    }
    // A Shift the user pressed stands until a letter spends it; only one this rule applied is its
    // to recompute. Without this a candidate arriving between the Shift and the letter would clear
    // the capital they asked for.
    if (this.caseMode === LetterCaseMode.SHIFTED && !this.letterCase.isAutomatic()) {
      return;
    }
    if (this.letterCase.applyAutomatic(KeyboardSession.shared.shouldShiftNextLetter())) {
      this.caseMode = this.letterCase.mode();
    }
  }

  /**
   * Put the grid away for an editor that cannot use it, and bring it back for one that can.
   *
   * An address, a password or a one-time code is arbitrary text, and a grid spells by matching a
   * digit sequence against a dictionary that will not contain it, so those fields get the full
   * letter face however the user last set the layout. Recomputed from the scheme on every attach
   * rather than remembered, which is what makes leaving the field restore the grid: the override is
   * the editor's and never reaches the preference. A manual layout change made while the field is
   * still focused outranks it until the next attach, the user having asked for that face by name.
   */
  private applyEditorFace(): void {
    this.standardRowsField = KeyboardSession.shared.editorKeepsStandardRows();
    const fullFace: boolean = KeyboardSession.shared.prefersFullFace();
    const grid: boolean = KeyboardScheme.isNineKeyLayout(this.scheme.touchKeyboardLayout) && !fullFace;
    // 14 键同理：地址、密码这些编辑框画整块 26 键，离开后回到 14 键。
    const fourteen: boolean = this.offersFourteenKey(this.scheme) && !fullFace;
    const eighteen: boolean = this.offersEighteenKey(this.scheme) && !fullFace;
    if (grid === this.nineKey && fourteen === this.fourteenKey && eighteen === this.eighteenKey) {
      return;
    }
    this.nineKey = grid;
    this.fourteenKey = fourteen;
    this.eighteenKey = eighteen;
    // Same reason the scheme cards set this rather than waiting for the Engine: the Engine only
    // reports Japanese once something has been typed, and the face has to change before that.
    this.japanese = grid && this.scheme.engineScheme === 'japanese';
    KeyboardSession.shared.setKeyGrid(this.keyGrid());
  }

  /** 这个方案在这台设备上是否画 14 键：全拼看方案布局，双拼、五笔各看自己的独立布局偏好；2in1 有实体键盘，一律回落 26 键。 */
  private offersFourteenKey(scheme: SchemeDefinition): boolean {
    return KeyboardScheme.letterLayout(scheme, this.shuangpinLayout, this.wubiLayout) === 'fourteen_key'
      && !KeyboardFormFactor.isDesktop();
  }

  private offersEighteenKey(scheme: SchemeDefinition): boolean {
    return KeyboardScheme.letterLayout(scheme, this.shuangpinLayout, this.wubiLayout) === 'eighteen_key'
      && !KeyboardFormFactor.isDesktop();
  }

  /** 交给引擎的网格（`KeyGrid`），由当前键面决定；双拼 18 键使用独立的网格值 5。英文下网格照旧保留，引擎在专用英文里不受理组码，字母走 26 键。 */
  private keyGrid(): number {
    if (this.nineKey) {
      return KeyGrid.NINE_KEY;
    }
    if (this.fourteenKey) return KeyGrid.FOURTEEN_KEY;
    if (!this.eighteenKey) return KeyGrid.NONE;
    return this.scheme.engineScheme === 'shuangpin'
      ? KeyGrid.EIGHTEEN_KEY_SHUANGPIN : KeyGrid.EIGHTEEN_KEY;
  }

  /** 是否正在画 14 键或 18 键键面：英文和本地模式画 26 键。 */
  private drawsFourteenKey(): boolean {
    return KeyboardScheme.usesFourteenKeyFace(
      this.fourteenKey || this.eighteenKey, this.english, this.localMode);
  }

  private drawsEighteenKey(): boolean {
    return this.eighteenKey && !this.english && this.localMode === 'none';
  }

  /**
   * Switch to English on the grid the card names.
   *
   * The twenty-six key card leaves the scheme alone, because English there is only a text mode over
   * whichever scheme the typist will come back to. The nine-key card cannot: the Engine reads the
   * grid's digits as letters only under quanpin, that being the only scheme whose syllables the grid
   * knows, so picking it moves the scheme underneath as well and says so on the card.
   */
  private selectEnglish(nineKey: boolean): void {
    this.leaveComposition();
    if (this.hasHandwritingContext()) {
      this.clearHandwriting();
    }
    if (nineKey) {
      const changes: Map<string, Object> =
        KeyboardSession.shared.schemeChanges(KeyboardScheme.QUANPIN_NINE_KEY);
      if (!KeyboardSession.shared.changePreferences(changes)) {
        return;
      }
      KeyboardSession.shared.useScheme(KeyboardScheme.QUANPIN_NINE_KEY.engineScheme);
      this.scheme = KeyboardScheme.QUANPIN_NINE_KEY;
      this.japanese = false;
    } else if (this.nineKey) {
      // Leaving the grid for the letters is the same request the 26/九 key makes, and the scheme has
      // to follow the face or the candidate strip goes on naming a layout that is off screen.
      // 14 键不走这里：英文本来就画 26 键，切回中文要回到 14 键，所以不改写 `touch_keyboard_layout`。
      const changes: Map<string, Object> = new Map<string, Object>();
      changes.set('touch_keyboard_layout', KeyboardScheme.QUANPIN.touchKeyboardLayout);
      if (!KeyboardSession.shared.changePreferences(changes)) {
        return;
      }
      this.scheme = this.resolveScheme(KeyboardScheme.QUANPIN.touchKeyboardLayout);
      this.japanese = false;
    }
    this.nineKey = nineKey;
    if (nineKey) {
      this.fourteenKey = false;
      this.eighteenKey = false;
    }
    KeyboardSession.shared.setKeyGrid(this.keyGrid());
    if (!this.english) {
      this.english = KeyboardSession.shared.toggleLanguage();
      this.letterCase.reset();
      this.caseMode = this.letterCase.mode();
    }
    this.show(SURFACE_NONE);
  }

  private selectSkin(choice: KeyboardSkin): void {
    if (!KeyboardSession.shared.selectGlobalTheme(choice.id)) {
      return;
    }
    // A theme may fix its own mode, so every surface is read back rather than assumed from the card.
    this.settings = KeyboardSession.shared.preferences();
    this.applySkins();
    this.show(SURFACE_NONE);
  }

  /**
   * The scheme is what the candidate strip names, and the grid is part of its identity: quanpin on
   * nine keys is a different scheme from quanpin on twenty-six, so switching the face has to switch
   * the scheme with it or the strip goes on claiming a layout that is no longer on screen.
   */
  /** 仅提供用户启用且当前设备可操作的方案；手写由本机离线笔画模型识别。 */
  private offeredSchemes(): SchemeDefinition[] {
    return KeyboardScheme.SCHEMES.filter((candidate: SchemeDefinition): boolean =>
      this.settings.enabledSchemes.includes(candidate.preferenceId)
      // A grid is a way of getting twenty-six letters out of nine keys, which is a problem only a
      // touch screen has. On a machine with a physical keyboard every letter already has a key.
      && !(KeyboardFormFactor.isDesktop() && (KeyboardScheme.isNineKeyLayout(candidate.touchKeyboardLayout)
        || candidate.touchKeyboardLayout === 'fourteen_key'
        || candidate.touchKeyboardLayout === 'eighteen_key')));
  }

  private schemeAvailable(candidate: SchemeDefinition): boolean {
    return true;
  }

  /** The scheme the keyboard is on, which the nine-key toggle can move without the picker. */
  /**
   * How tall a surface is: everything the key rows and the action row occupy, plus the gaps between
   * them. A surface covers the whole keyboard, the way iOS covers it, rather than sitting above an
   * action row that stays put and clips the surface's last line.
   */
  private surfaceHeight(): number {
    let total: number = this.rowSpacing() * (KeyboardMetrics.KEY_ROWS - 1);
    for (let row: number = 0; row < KeyboardMetrics.KEY_ROWS; row++) {
      total += this.rowHeight(row);
    }
    // 竖屏手写多一行键高和一道行距；从工具栏打开的面板也盖这么高，打开表情等面板时键盘不跳。字母上方的数字行（`numberRowExtraRows`）同样多一行。
    return total + this.handwritingExtraRows() * (KeyboardMetrics.ROW_HEIGHT_VP + this.rowSpacing())
      + this.numberRowExtraHeight();
  }

  /** 面板多出的行：竖屏手写和数字行，两者不会同时出现。 */
  private extraRows(): number {
    return this.handwritingExtraRows() + this.numberRowExtraRows();
  }

  /** 手写区比其他键面多出的行数（`KeyboardMetrics.handwritingExtraRows`）：竖屏触屏手写为 1，其余为 0。 */
  private handwritingExtraRows(): number {
    return KeyboardMetrics.handwritingExtraRows(this.touchDesign()
      && KeyboardScheme.usesHandwritingFace(this.scheme, this.english, this.symbols, this.localMode), this.landscape);
  }

  /**
   * What is left of a surface once the action row and the gap above it are taken out.
   *
   * The faces that draw their own keys — the two grids and handwriting — end with actionRow(), so
   * their key area is this rather than the whole surface. Keeping the subtraction here is what makes
   * the keyboard the same height whichever layout is showing, which is what the Apple host asserts.
   */
  private gridHeight(): number {
    return this.faceHeight() - this.rowHeight(3) - this.rowSpacing();
  }

  /** 网格键面（九键、借用的数字九宫格）自己占的高度：数字行开着时借用的数字九宫格上方另有一排补充键（`symbolExtraKeysRow`），网格让出那一行。其余键面数字行不画，与 `surfaceHeight` 相同。 */
  private faceHeight(): number {
    return this.surfaceHeight() - this.numberRowExtraHeight();
  }

  private isPicker(): boolean {
    return this.surface === SURFACE_SCHEME || this.surface === SURFACE_SKIN;
  }

  private isTools(): boolean {
    return this.surface === SURFACE_TOOLS || this.surface === SURFACE_MODES;
  }

  /** Shows one surface, which closes whatever was showing. */
  private show(surface: string): void {
    const leavingExpanded: boolean = this.surface === SURFACE_EXPANDED && surface !== SURFACE_EXPANDED;
    if (surface !== SURFACE_NINE_KEY_HOLD) {
      this.nineKeyHoldOptions = [];
    }
    this.nineKeyQuickMarks = [];
    if (surface !== SURFACE_REPLY) {
      this.replyGeneration += 1;
      this.replyBusy = false;
      this.replyResults = [];
      if (this.surface === SURFACE_REPLY) {
        // The Apple model invalidates its context when the reply surface is cleared. Do the same
        // in the session as well as the view so a late HTTP answer cannot remain insertable.
        KeyboardSession.shared.invalidateReplyContext(false);
      }
    }
    if (surface !== SURFACE_POLISH) {
      // Abandons a rewrite the user has walked away from. The generation is what the reply the
      // model eventually sends is checked against, so bumping it here is what makes leaving the
      // panel mean "never mind" rather than "show me this later, over whatever I am typing then".
      this.polishGeneration += 1;
      this.polishBusy = false;
    }
    if (surface !== SURFACE_PHRASES) {
      // 仍在读取中的常用语列表属于发起请求的那次访问。
      this.phraseGeneration += 1;
    }
    if (this.surface === SURFACE_GEOMETRY && surface !== SURFACE_GEOMETRY) {
      // 以任何方式离开键盘调整（确认、切到别的面板、收起键盘、换输入框、切应用）都按确认保存，没有「取消」，与 Android 的决定相同（`2026-10-08-touch-keyboard-height-reverts`）。
      this.saveAdjust();
    }
    if (surface !== SURFACE_VOICE) {
      if (VoiceRecognitionPolicy.cancelsOnLeave(this.touchDesign(), this.voiceBusy,
        this.surface === SURFACE_VOICE)) {
        // 触屏上离开语音界面（点工具栏标志、表情、剪贴板等）即放弃这次录音，否则麦克风会在按键后面一直开着。
        this.cancelVoice();
      }
      this.clearVoiceDone();
    }
    this.surface = surface;
    this.syncStripHeight();
    if (surface === SURFACE_REPLY) {
      this.replyTemplates = KeyboardSession.shared.communityReplyTemplates();
      this.replyTemplateMenu = false;
    }
    if (surface !== SURFACE_EXPANDED) {
      this.expanded = [];
      this.expandedBuilt = 0;
      this.expandedListIdentity = '';
      this.expandedGloss = -1;
    }
    if (surface === SURFACE_TOOLS) {
      // 按键反馈的文件设置页也会写；工具面板的开关照它画，也照它改，读旧值会把设置页选的档位写回去。会话每次获得焦点都重读这个文件，读不到时保留上次的值，所以取会话持有的副本。
      this.feedback = KeyboardSession.shared.feedbackSettings();
    }
    // 放在 surface 已经换掉之后：清筛选会同步回调 onChanged，那时面板必须已经算作关闭，否则会再进来一次。
    if (leavingExpanded) {
      this.leaveNineKeyPanel();
    }
  }

  /** 候选栏的展开箭头：用全部候选代替按键显示，或收起网格。 */
  private toggleExpanded(): void {
    if (this.surface === SURFACE_EXPANDED) {
      this.show(SURFACE_NONE);
      return;
    }
    this.nineKeyPanelMode = NineKeyPanelMode.SPELLING;
    this.show(SURFACE_EXPANDED);
    this.refreshExpanded();
  }

  /** 重新取完整候选列表；长按释义指的那一行没了就收起释义条。同一代候选的重取建回已建的格子，换了一代从第一批建起并回到顶部。 */
  private refreshExpanded(): void {
    const previous: string = this.expandedListIdentity;
    this.expanded = KeyboardSession.shared.allCandidates();
    this.expandedListIdentity = KeyboardSession.shared.candidateListIdentity();
    const sameGeneration: boolean = previous.length > 0 && previous === this.expandedListIdentity;
    this.expandedBuilt = ExpandedCandidateBatchPolicy.initialCount(this.expanded.length, this.expandedBuilt,
      sameGeneration);
    if (previous.length > 0 && !sameGeneration) {
      // 面板开着时换了候选（选掉前一段、选拼音、退格、改筛选）：新列表从头建，留着旧的滚动位置会落在第一批之外。刚打开的面板本来就在顶部。
      this.expandedScroller.scrollTo({ xOffset: 0, yOffset: 0 });
    }
    if (this.expandedGloss >= this.expanded.length) {
      this.expandedGloss = -1;
    }
  }

  /** 滚到离已建格子底边不到一屏，或第一批没填够一屏多时，追加下一批。 */
  private appendExpandedIfNeeded(): void {
    if (this.surface !== SURFACE_EXPANDED || !ExpandedCandidateBatchPolicy.shouldAppend(this.expandedBuilt,
      this.expanded.length, this.expandedScroller.currentOffset().yOffset, this.expandedViewportHeight,
      this.expandedContentHeight)) {
      return;
    }
    this.expandedBuilt = ExpandedCandidateBatchPolicy.nextCount(this.expandedBuilt, this.expanded.length);
  }

  /**
   * 展开面板里点中一个候选。只选掉前面一段、组字还在时面板留着：选词的回报经 onChanged 同步走到 refreshExpanded，换上剩下组字的全部候选，用户接着在面板里往下选（#9193，规则在 `CandidatePanelSelectionPolicy`）。整句上屏、组字清空或引擎拒绝了这次选择就收起，组字还在的话 leaveNineKeyPanel 照旧清掉筛选。九键面板开着笔画筛选时顺带清掉笔画：它比的是刚被选掉的那个字。
   */
  private chooseExpanded(index: number): void {
    const nineKeyPanel: boolean = this.nineKeyPanel();
    const handled: boolean = KeyboardSession.shared.chooseAny(index);
    if (!CandidatePanelSelectionPolicy.keepsOpen(handled, KeyboardSession.shared.composing())) {
      if (this.surface === SURFACE_EXPANDED) {
        this.show(SURFACE_NONE);
      }
      return;
    }
    const strokes: string | null = nineKeyPanel && this.nineKeyPanel()
      ? CandidatePanelSelectionPolicy.strokesAfterPartialSelection(this.nineKeyStrokes) : null;
    if (strokes !== null) {
      KeyboardSession.shared.setNineKeyFilter(this.nineKeySingleCharacter, strokes);
    }
  }

  /** 展开面板关掉时：笔画模式复位，组字还在的话把筛选清掉，收起后的候选条不该还是筛过的。 */
  private leaveNineKeyPanel(): void {
    this.nineKeyPanelMode = NineKeyPanelMode.SPELLING;
    if (NineKeyPanelPolicy.clearsFilterOnClose(KeyboardSession.shared.composing(),
      this.nineKeySingleCharacter, this.nineKeyStrokes)) {
      KeyboardSession.shared.setNineKeyFilter(false, '');
    }
  }

  /** 读音行右侧是否画 14/18 键的拼音选择条：组码组字、有可选的拼音时。在 123 层和三栏面板开着时照样画，与 Android、iOS 相同：读音行一直在，选择条跟着它。 */
  private readingRowSpellings(): boolean {
    return this.drawsFourteenKey() && this.spellings.length > 0;
  }

  /** 展开面板是否画成全拼九键的三栏；14/18 键用同一个面板。 */
  private nineKeyPanel(): boolean {
    return NineKeyPanelPolicy.threeColumn(
      KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode) || this.drawsFourteenKey(),
      SchemeCompositionPolicy.rulesScheme(this.engineScheme, this.english, this.localMode),
      this.editing.length > 0);
  }

  /** 三栏面板里 ⌫ 的这一下：笔画模式下先撤一笔，否则同键盘上的退格。 */
  private nineKeyPanelBackspace(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.BACKSPACE);
    if (NineKeyPanelPolicy.backspace(this.nineKeyPanelMode, this.nineKeyStrokes)
      === NineKeyPanelBackspace.POP_STROKE) {
      KeyboardSession.shared.setNineKeyFilter(this.nineKeySingleCharacter,
        NineKeyPanelPolicy.popStroke(this.nineKeyStrokes));
      return;
    }
    KeyboardSession.shared.backspace();
  }

  /** 拼音栏和笔画栏互换；离开笔画模式时清掉笔画筛选。 */
  private toggleNineKeyPanelMode(): void {
    const strokes: string | null = NineKeyPanelPolicy.strokesAfterToggle(this.nineKeyPanelMode, this.nineKeyStrokes);
    this.nineKeyPanelMode = NineKeyPanelPolicy.toggledMode(this.nineKeyPanelMode);
    if (strokes !== null) {
      KeyboardSession.shared.setNineKeyFilter(this.nineKeySingleCharacter, strokes);
    }
  }

  private appendNineKeyStroke(code: string): void {
    const next: string = NineKeyPanelPolicy.appendStroke(this.nineKeyStrokes, code);
    if (next !== this.nineKeyStrokes) {
      KeyboardSession.shared.setNineKeyFilter(this.nineKeySingleCharacter, next);
    }
  }

  /** 中栏候选网格可用的宽度：三栏时去掉左右两栏和它们之间的两道键距。 */
  private expandedGridWidth(): number {
    if (!this.nineKeyPanel()) {
      return this.availableWidth;
    }
    return Math.max(0, this.availableWidth
      * (100 - NINE_KEY_PANEL_LEFT_PERCENT - NINE_KEY_PANEL_RIGHT_PERCENT) / 100
      - this.keySpacing() * 2);
  }

  /**
   * The back control of a face that both form factors draw.
   *
   * On a phone the face is one of the panel's own surfaces. On a 2in1 the toolbar may have opened it as the window's desktop surface, which show() never touches, so closing it there has to go through the session; otherwise the button does nothing and the only way out is the toolbar. The same face reached from inside the 2in1 screen keyboard is a panel surface again, and backing out of it returns to the keys rather than closing the keyboard.
   */
  private leaveSurface(owner: DesktopSurface): void {
    if (KeyboardFormFactor.isDesktop() && this.desktop === owner) {
      KeyboardSession.shared.setDesktopSurface(DesktopSurface.NONE);
      return;
    }
    this.show(SURFACE_NONE);
  }

  /** Tapping the icon of the surface already open closes it. */
  private toggle(surface: string): void {
    this.show(this.surface === surface ? SURFACE_NONE : surface);
  }

  /** 有任何东西盖住按键时，工具栏的标志会着色：功能面板、选择器、由工具栏或功能面板打开的面板，或展开的候选。 */
  private brandActive(): boolean {
    return this.surface !== SURFACE_NONE;
  }

  /** 标志：从其他任何界面回到按键；否则在第一页打开功能面板，或将其关闭。 */
  private tapBrand(): void {
    if (this.surface !== SURFACE_NONE) {
      this.show(SURFACE_NONE);
      return;
    }
    this.panelResetToken += 1;
    this.show(SURFACE_TOOLS);
  }

  /** 长按工具栏的任一按钮：给一下按键反馈，回到按键，再打开设置应用里放工具栏开关的「屏幕键盘」页。 */
  private openToolbarSettings(): void {
    KeyboardSession.shared.holdFeedback();
    this.show(SURFACE_NONE);
    KeyboardSession.shared.openSettings('screen-keyboard');
  }

  /** 工具栏的「剪贴板」：用剪贴板面板代替按键，或回到按键。 */
  private toggleClipboard(): void {
    if (this.surface === SURFACE_CLIPBOARD) {
      this.show(SURFACE_NONE);
      return;
    }
    this.openClipboard();
  }

  /** 工具栏的「常用语」：用常用语列表代替按键，或回到按键。打开时先把已有组合按原样上屏，与 Android 的 `showCommonPhrases` 一致，这样列表上方的工具条重新变回工具栏。 */
  private togglePhrases(): void {
    if (this.surface === SURFACE_PHRASES) {
      this.show(SURFACE_NONE);
      return;
    }
    if (KeyboardSession.shared.composing()) {
      KeyboardSession.shared.commitRaw();
    }
    this.show(SURFACE_PHRASES);
    const generation: number = ++this.phraseGeneration;
    this.phraseState = PHRASE_LIST_LOADING;
    KeyboardSession.shared.commonPhrases().then((phrases: string[]): void => {
      if (generation !== this.phraseGeneration) {
        return;
      }
      this.phrases = phrases;
      this.phraseState = PHRASE_LIST_READY;
    }).catch((error: Object): void => {
      if (generation !== this.phraseGeneration) {
        return;
      }
      KeyboardLog.error(`common phrases unavailable: ${describeError(error)}`);
      this.phrases = [];
      this.phraseState = PHRASE_LIST_ERROR;
    });
  }

  private pickPhrase(text: string): void {
    KeyboardSession.shared.commitPhrase(text);
    this.show(SURFACE_NONE);
  }

  /** 功能面板的「键盘调整」：整块键盘上盖一层遮罩（`adjustOverlay`），下面照常画作实时预览，从已保存的四项开始。 */
  private openAdjustMode(): void {
    this.adjustDragHandle = '';
    this.adjustOriginal = this.adjustValues();
    this.show(SURFACE_GEOMETRY);
  }

  /** 此刻的四项：已保存的值，或键盘调整打开期间正在预览的值。 */
  private adjustValues(): AdjustValues {
    return {
      height: this.geometryHeight,
      lift: this.geometryLift,
      widthPercent: this.geometryWidthPercent,
      positionX: this.geometryPositionX
    };
  }

  /** 此刻哪几项生效（`KeyboardAdjustPolicy.availability`）：浮动、手机横屏和 2in1 上只有高度；单手或分离时没有宽度和位置。 */
  private adjustAvailability(): AdjustAvailability {
    return KeyboardAdjustPolicy.availability(this.floatingDrawn(), this.phoneLandscape, KeyboardFormFactor.isDesktop(),
      this.oneHandActive(), this.splitDrawn());
  }

  /** 遮罩上画哪些柄和「移动」键。 */
  private adjustHandles(): AdjustHandles {
    return KeyboardAdjustPolicy.handles(this.adjustAvailability());
  }

  /** 键盘外框在面板里的位置：根节点和遮罩按它在两侧和底下让出键盘底色。 */
  private adjustFrame(): AdjustFrame {
    return KeyboardAdjustPolicy.frame(this.adjustValues(), this.adjustAvailability(), this.adjustPanelWidthVp());
  }

  /** 键盘调整换算宽度和位置用的面板宽（vp）：量到的 `rootWidth`，还没量到时退回屏幕宽。不直接用 `screenWidthVp`：停靠面板的宽度是扩展能力建面板时的屏幕宽（`fitTouchPanel` 一直按它 resize），平板转屏或折叠屏开合之后与此刻的屏幕宽不同，按屏幕宽换算会让出比面板还宽的两侧，拖左右沿时对边也不再不动。 */
  private adjustPanelWidthVp(): number {
    return this.rootWidth > 0 ? this.rootWidth : this.screenWidthVp;
  }

  /** 中间一排按钮的宽和间距（`KeyboardAdjustPolicy.buttonRow`）：外框窄时先收间距再收按钮，不压到左右柄上。 */
  private adjustButtonRow(): AdjustButtonRow {
    const frame: AdjustFrame = this.adjustFrame();
    const handles: AdjustHandles = this.adjustHandles();
    return KeyboardAdjustPolicy.buttonRow(this.adjustPanelWidthVp() - frame.left - frame.right,
      handles.move ? 3 : 2, handles.left && handles.right);
  }

  /** 此刻算进面板高度的底部抬高（vp），不生效时为 0。 */
  private bottomLiftVp(): number {
    return KeyboardAdjustPolicy.liftVp(this.geometryLift, this.adjustAvailability());
  }

  /** 按四项绘制但不保存：高度或抬高变了时面板随之改高度，所以预览就是真实大小。 */
  private previewAdjust(values: AdjustValues): void {
    const height: number = KeyboardGeometry.heightAdjustment(values.height);
    const lift: number = KeyboardAdjustPolicy.lift(values.lift);
    const resize: boolean = height !== this.geometryHeight || lift !== this.geometryLift;
    this.geometryHeight = height;
    this.geometryLift = lift;
    this.geometryWidthPercent = KeyboardAdjustPolicy.widthPercent(values.widthPercent);
    this.geometryPositionX = KeyboardAdjustPolicy.position(values.positionX);
    if (resize) {
      this.resizeTouchPanel();
    }
  }

  /**
   * 保存预览的四项：高度变了才写共享偏好（`applyGeometry`）；其余三项只取此刻生效且动过的（`KeyboardAdjustPolicy.saved`），写本机的按键反馈文件（`KeyboardSession.saveKeyboardAdjust`，没变时不写），不生效的项保留已存的值，与 Android 相同。之后把视图里的三项换成存下的值，回到它们生效的模式时按存下的画。
   */
  private saveAdjust(): void {
    this.adjustDragHandle = '';
    if (this.geometryHeight !== this.settings.heightAdjustment) {
      this.applyGeometry(this.geometryKeySpacing, this.geometryRowSpacing, this.geometryHeight);
    }
    const feedback: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    const stored: AdjustValues = {
      height: this.geometryHeight,
      lift: feedback.bottomLift,
      widthPercent: feedback.widthPercent,
      positionX: feedback.positionX
    };
    const saved: AdjustValues = KeyboardAdjustPolicy.saved(stored, this.adjustOriginal, this.adjustValues(),
      this.adjustAvailability());
    if (KeyboardSession.shared.saveKeyboardAdjust(saved.lift, saved.widthPercent, saved.positionX)) {
      this.feedback = KeyboardSession.shared.feedbackSettings();
    }
    this.previewAdjust(saved);
  }

  /** 从会话读回已保存的四项：高度来自共享偏好，其余三项来自会话持有的按键反馈副本。 */
  private readAdjustValues(): void {
    const feedback: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    this.geometryHeight = this.settings.heightAdjustment;
    this.geometryLift = feedback.bottomLift;
    this.geometryWidthPercent = feedback.widthPercent;
    this.geometryPositionX = feedback.positionX;
  }

  /**
   * 遮罩上一个柄或「移动」键的触摸：按下记下起点、四项和面板宽度，移动时按相对起点的屏幕位移换算（`KeyboardAdjustPolicy`）并预览，抬起或取消时结束。不在这里保存，离开键盘调整时才存。
   */
  private adjustTouch(handle: string, event: TouchEvent): void {
    const touch: TouchObject | undefined = event.touches.length > 0 ? event.touches[0] : undefined;
    if (event.type === TouchType.Down && touch !== undefined) {
      this.adjustDragHandle = handle;
      this.adjustDragOrigin = [touch.displayX, touch.displayY];
      this.adjustDragBase = this.adjustValues();
      this.adjustDragWidthVp = this.adjustPanelWidthVp();
    } else if (event.type === TouchType.Move && touch !== undefined && this.adjustDragHandle === handle) {
      this.dragAdjust(handle, touch.displayX - this.adjustDragOrigin[0], touch.displayY - this.adjustDragOrigin[1]);
    } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
      this.adjustDragHandle = '';
    }
    event.stopPropagation();
  }

  /** 一次拖动位移（vp，向右、向下为正）换成四项：每个柄只动它那条边，对边在屏幕上不动；「移动」只改抬高和位置，大小不变。 */
  private dragAdjust(handle: string, dx: number, dy: number): void {
    const base: AdjustValues = this.adjustDragBase;
    const width: number = this.adjustDragWidthVp;
    if (handle === ADJUST_TOP) {
      const next: AdjustValues = {
        height: KeyboardAdjustPolicy.dragTop(base.height, dy),
        lift: base.lift,
        widthPercent: base.widthPercent,
        positionX: base.positionX
      };
      this.previewAdjust(next);
    } else if (handle === ADJUST_BOTTOM) {
      const rise: AdjustRise = KeyboardAdjustPolicy.dragBottom(base.height, base.lift, dy);
      const next: AdjustValues = {
        height: rise.height,
        lift: rise.lift,
        widthPercent: base.widthPercent,
        positionX: base.positionX
      };
      this.previewAdjust(next);
    } else if (handle === ADJUST_LEFT || handle === ADJUST_RIGHT) {
      const span: AdjustSpan = handle === ADJUST_LEFT
        ? KeyboardAdjustPolicy.dragLeft(width, base.widthPercent, base.positionX, dx)
        : KeyboardAdjustPolicy.dragRight(width, base.widthPercent, base.positionX, dx);
      const next: AdjustValues = {
        height: base.height,
        lift: base.lift,
        widthPercent: span.widthPercent,
        positionX: span.positionX
      };
      this.previewAdjust(next);
    } else if (handle === ADJUST_MOVE) {
      const availability: AdjustAvailability = this.adjustAvailability();
      const next: AdjustValues = {
        height: base.height,
        lift: KeyboardAdjustPolicy.movedLift(base.lift, dy, availability),
        widthPercent: base.widthPercent,
        positionX: KeyboardAdjustPolicy.movedPosition(width, base.widthPercent, base.positionX, dx, availability)
      };
      this.previewAdjust(next);
    }
  }

  /** 「重置」和「确认」：重置把四项预览回默认、不保存；确认离开键盘调整，离开时保存（见 `show`）。 */
  private adjustButtonClick(kind: string): void {
    if (kind === ADJUST_RESET) {
      this.previewAdjust(KeyboardAdjustPolicy.DEFAULTS);
    } else if (kind === ADJUST_CONFIRM) {
      this.show(SURFACE_NONE);
    }
  }

  /** 柄的读屏名称，三端相同。 */
  private adjustHandleLabel(handle: string): string {
    return handle === ADJUST_TOP ? KeyboardAdjustPolicy.TOP_LABEL
      : handle === ADJUST_BOTTOM ? KeyboardAdjustPolicy.BOTTOM_LABEL
        : handle === ADJUST_LEFT ? KeyboardAdjustPolicy.LEFT_LABEL : KeyboardAdjustPolicy.RIGHT_LABEL;
  }

  /** 柄的读屏说明：当前值，再说怎么调。 */
  private adjustHandleDescription(handle: string): string {
    if (handle === ADJUST_TOP) {
      return `${KeyboardAdjustPolicy.heightValue(this.geometryHeight)}，${KeyboardAdjustPolicy.TOP_DESCRIPTION}`;
    }
    if (handle === ADJUST_BOTTOM) {
      return `${KeyboardAdjustPolicy.liftValue(this.geometryLift)}，${KeyboardAdjustPolicy.BOTTOM_DESCRIPTION}`;
    }
    return `${KeyboardAdjustPolicy.widthValue(this.geometryWidthPercent)}，${KeyboardAdjustPolicy.SIDE_DESCRIPTION}`;
  }

  private adjustButtonLabel(kind: string): string {
    return kind === ADJUST_RESET ? KeyboardAdjustPolicy.RESET_LABEL
      : kind === ADJUST_MOVE ? KeyboardAdjustPolicy.MOVE_LABEL : KeyboardAdjustPolicy.CONFIRM_LABEL;
  }

  private adjustButtonDescription(kind: string): string {
    return kind === ADJUST_RESET ? KeyboardAdjustPolicy.RESET_DESCRIPTION
      : kind === ADJUST_MOVE ? KeyboardAdjustPolicy.MOVE_DESCRIPTION : KeyboardAdjustPolicy.CONFIRM_DESCRIPTION;
  }

  private adjustButtonIcon(kind: string): string {
    return kind === ADJUST_RESET ? KeyboardIconPaths.ADJUST_RESET
      : kind === ADJUST_MOVE ? KeyboardIconPaths.ADJUST_MOVE : KeyboardIconPaths.ADJUST_CONFIRM;
  }

  private isCurrentScheme(candidate: SchemeDefinition): boolean {
    // Not while English is running: the scheme is still set underneath, but what the keyboard is
    // composing is English, and two cards showing as current would say otherwise.
    return !this.english && candidate.preferenceId === this.scheme.preferenceId;
  }

  /**
   * What a grid key says, which for one of them depends on what the digits are spelling.
   *
   * The apostrophe separates one syllable from the next, so its cap reads 分词 while the grid is
   * spelling pinyin. In English it is a letter of the words themselves — don't, it's — and what it
   * sends never changes, only what to call it.
   */
  private nineKeyFaceOf(key: NineKey): string {
    return this.english && key.input === '\'' ? '\'' : key.label;
  }

  private nineKeyLabel(key: NineKey): string {
    const options: string[] = NineKeyLayout.holdOptions(key);
    return options.length === 0 ? key.description
      : `${key.description}，长按输入 ${options.join('、')}`;
  }

  private activateReturn(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.ENTER);
    const dispatch: ReturnDispatch = ReturnKeyAction.dispatch(
      this.japanese, this.editing.length > 0, this.candidates.length,
      KeyboardSession.shared.japaneseConversionActive(), this.koreanFace(), this.vietnameseFace(),
      this.strokeFace(), this.prediction);
    if (dispatch === ReturnDispatch.COMMIT_RAW) {
      KeyboardSession.shared.commitRaw();
    } else if (dispatch === ReturnDispatch.FINISH_THEN_EDITOR) {
      KeyboardSession.shared.finishBeforeKey();
      KeyboardSession.shared.submit();
    } else if (dispatch === ReturnDispatch.COMMIT_HIGHLIGHTED) {
      KeyboardSession.shared.commitHighlighted();
    } else if (dispatch === ReturnDispatch.COMMIT_READING) {
      KeyboardSession.shared.commitReading();
    } else if (dispatch === ReturnDispatch.FINISH_COMPOSITION) {
      if (!KeyboardSession.shared.finishComposition()) {
        KeyboardSession.shared.submit();
      }
    } else {
      KeyboardSession.shared.submit();
    }
  }

  private activateSpace(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SPACE);
    if (this.koreanFace()) {
      // Space ends the open syllable and is still a space, so the keyboard types it after the commit. With the syllable's Hanja list open it chooses the highlighted Hanja instead, as Return does.
      if (this.koreanHanjaListOpen()) {
        KeyboardSession.shared.commitHighlighted();
        return;
      }
      KeyboardSession.shared.commitThenType(0x20);
      return;
    }
    if (this.vietnameseFace()) {
      // Space ends the word and is still a space, as for a Korean syllable. Zhuyin keeps the shared path: there Space is tone 1, opens the list over a conversion and picks the highlighted row of an open one, which the Engine decides.
      KeyboardSession.shared.commitThenType(0x20);
      return;
    }
    if (this.tibetanFace()) {
      // 藏文空格由引擎结束音节串并补上音节点；没有组字时引擎不处理，键盘照常输入空格。
      KeyboardSession.shared.pressThenType(0x20);
      return;
    }
    if (this.japanese && this.editing.length > 0
        && KeyboardSession.shared.convertJapanese()) {
      return;
    }
    KeyboardSession.shared.pressSpace();
  }

  /** Which of the two English cards is current is the grid, English itself being common to both. */
  private isCurrentEnglish(nineKey: boolean): boolean {
    return this.english && this.nineKey === nineKey;
  }

  /** Every surface's skin from the session's resolved global theme. */
  private applySkins(): void {
    this.skin = KeyboardSession.shared.keyboardSkin(this.settings.dark);
    this.voiceSkin = KeyboardSession.shared.surfaceSkin(this.settings.voiceTheme);
    this.emojiSkin = KeyboardSession.shared.surfaceSkin(this.settings.emojiTheme);
    this.handwritingSkin = KeyboardSession.shared.surfaceSkin(this.settings.handwritingTheme);
    this.menuColors = KeyboardSession.shared.menuColors();
  }

  /** Read the stored preferences into the view's skins, geometry and scheme. */
  private applySettings(): void {
    this.settings = KeyboardSession.shared.preferences();
    this.geometryKeySpacing = this.settings.keySpacingTenths;
    this.geometryRowSpacing = this.settings.rowSpacingTenths;
    this.shuangpinKeyHints = this.settings.shuangpinKeyHints;
    this.quanpinShift = this.settings.quanpinShift;
    this.toolbarTools = this.settings.toolbarTools;
    this.twentySixKeyStyle = this.settings.twentySixKeyStyle;
    this.fourteenKeyStyle = this.settings.fourteenKeyStyle;
    this.nineKeyStyle = this.settings.nineKeyStyle;
    this.numberRow = this.settings.numberRow;
    this.twentySixSymbolFirst = this.settings.twentySixSymbolFirst;
    this.numberPadSpace = this.settings.numberPadSpace;
    this.nineKeySidebarSymbols = this.settings.nineKeySidebarSymbols;
    this.nineKeyDigitSidebarSymbols = this.settings.nineKeyDigitSidebarSymbols;
    this.oneHandedSidePanel = this.settings.oneHandedSidePanel;
    this.splitKeyboard = this.settings.splitKeyboard;
    this.languageKeyLeft = this.settings.languageKeyLeft;
    // 取会话持有的副本而不是再读一次磁盘：会话在构建和每次获得焦点时读取，读不到时保留上次的值，视图跟着它就不会因一次读取失败把隐私模式等显示成关闭。
    this.feedback = KeyboardSession.shared.feedbackSettings();
    if (this.surface !== SURFACE_GEOMETRY) {
      // 键盘调整开着时保留正在预览的四项：设置页中途保存引起的重载不能把预览盖回旧值，离开时按确认存下（与 Android 相同）。
      this.readAdjustValues();
    }
    this.traditional = KeyboardSession.shared.usesTraditional();
    this.chinesePunctuation = KeyboardSession.shared.usesChinesePunctuation();
    this.punctuationLocked = !KeyboardSession.shared.punctuationToggles();
    this.fullWidth = KeyboardSession.shared.isFullWidth();
    this.translations = KeyboardSession.shared.showsCandidateTranslations();
    this.fuzzyPinyin = KeyboardSession.shared.usesFuzzyPinyin();
    this.applySkins();
    this.wubiProfile = this.settings.wubiProfile;
    this.shuangpinLayout = this.settings.shuangpinLayout;
    this.wubiLayout = this.settings.wubiLayout;
    this.scheme = this.enabledScheme();
    this.nineKey = KeyboardScheme.isNineKeyLayout(this.scheme.touchKeyboardLayout);
    this.nineKeyShuangpinFaces = this.scheme.preferenceId === 'nine_key_shuangpin'
      ? KeyboardSession.shared.nineKeyShuangpinFaces().slice() : [];
    this.fourteenKey = this.offersFourteenKey(this.scheme);
    this.eighteenKey = this.offersEighteenKey(this.scheme);
    this.japanese = this.scheme.engineScheme === 'japanese' && this.nineKey;
    KeyboardSession.shared.setKeyGrid(this.keyGrid());
  }

  /**
   * The scheme to open on, after the shared page's enabled list has had its say.
   *
   * `scheme` and `touch_keyboard_layout` are the Engine's view and resolve to whatever was last
   * applied, whether or not the picker still offers it. Turning off the current scheme from the
   * settings page therefore left the keyboard on it with no card to leave by. The resolution and
   * the write-back are both ported — this reads the list, that one moves the preference.
   */
  private enabledScheme(): SchemeDefinition {
    const applied: SchemeDefinition = this.resolveScheme(this.settings.layout);
    const selected: SchemeDefinition = KeyboardScheme.resolveEnabledSelection(applied,
      this.settings.selectedScheme,
      KeyboardScheme.enabledFromPreferenceIds(this.settings.enabledSchemes));
    // Reads false when nothing moved, and also when the write failed — in which case the keyboard
    // stays where the document says it is rather than drawing a scheme that was not persisted.
    return KeyboardSession.shared.applyRuntimeSelection(applied, selected) ? selected : applied;
  }

  private resolveScheme(layout: string): SchemeDefinition {
    return KeyboardScheme.fromPreferences(this.settings.scheme, this.settings.shuangpinProfile,
      layout);
  }

  /**
   * Kana or digits, the same eleven cells either way.
   *
   * The digit layer keeps the grid rather than handing over to the twenty-six key symbol rows, and
   * every one of its labels carries an empty stroke, which sendKana already treats as insert-as-is —
   * so a digit goes in directly instead of being read as romaji.
   */
  private japaneseKeys(): JapaneseKey[] {
    return this.symbols ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys();
  }

  /** A symbol wears the face of what it will insert, which in Chinese is not the ASCII on the cap. */
  private symbolFace(symbol: string): string {
    const chinese: string | undefined = ChineseSymbolFaces.face(
      symbol, this.cornerQuotes, this.hollowBrackets,
      this.enumerationCommaBackslash, this.enumerationCommaSlash);
    // 韩语、越南语和藏文的标点就是键帽上的 ASCII。
    return !this.english && !this.koreanFace() && !this.vietnameseFace() && !this.tibetanFace()
      && chinese !== undefined
      ? chinese : symbol;
  }

  /**
   * 符号层按键的行为。数字结束正在组的韩语音节，然后输入在它后面，标点路由做不到这一点，因为它只接受标点。越南语数字也是这样，除非它是组字中单词的 VNI 声调数字，那时它给单词加符号。藏文数字同样先上屏音节串再输入数字本身；藏文的拼写符号（`'` `+` `.` `-` `/`）走标点路由，由运行时按 spelling_symbols 交给引擎，其中 `+` 来自藏文方案下第三排替换 `=` 的键（KeyboardScheme.symbolRowKey）。注音的数字或标点如果是大千的注音键或声调键，由键盘自己输入，因为引擎会把它组进注音。
   */
  private tapSymbol(symbol: string): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(symbol));
    const character: number = symbol.charCodeAt(0);
    if (this.koreanFace() && KoreanCompositionPolicy.typesAfterCommit(character)) {
      KeyboardSession.shared.commitThenType(character);
      return;
    }
    if (this.zhuyinFace() && ZhuyinLayout.claimsSymbol(symbol)) {
      this.typeZhuyinMark(symbol);
      return;
    }
    if (this.tibetanFace() && character >= 0x30 && character <= 0x39) {
      KeyboardSession.shared.commitThenType(character);
      return;
    }
    if (this.vietnameseFace() && character >= 0x30 && character <= 0x39) {
      if (this.editing.length > 0
          && KeyboardSession.shared.hardwareSpelling().spellingSymbols.includes(symbol)) {
        KeyboardSession.shared.press(character, false);
      } else {
        KeyboardSession.shared.commitThenType(character);
      }
      return;
    }
    // 组字中 Engine 列为拼写的符号走字符路由；标点入口拒收数字，网址里的数字会被丢掉。
    if (HardwareKeyRouter.touchSpells(KeyboardSession.shared.hardwareSpelling(), character)) {
      KeyboardSession.shared.press(character, false);
      return;
    }
    KeyboardSession.shared.punctuation(character);
  }

  private symbolKey(symbol: string): KeyOptions {
    return {
      face: this.symbolFace(symbol),
      label: KeyAccessibilityPolicy.symbol(this.symbolFace(symbol)),
      size: KeyboardMetrics.KEY_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapSymbol(symbol)
    };
  }

  /**
   * What a key's appearance depends on, folded into its ForEach identity.
   *
   * ForEach with no key generator identifies an item by its own value, and a letter is always the
   * same letter, so its subtree is never rebuilt: the faces stayed lowercase after switching to
   * Chinese and the shuangpin hints never appeared, while the return key beside them updated fine
   * because it is not inside a ForEach at all.
   */
  /** Tenths of a vp in the store, whole vp on screen. */
  private keySpacing(): number {
    return this.geometryKeySpacing / 10;
  }

  private rowSpacing(): number {
    return this.geometryRowSpacing / 10;
  }

  /** 键的触摸区：左右各扩出半个键距、上下各扩出半个行距，相邻键在空隙中线相接，键与候选栏之间的空隙也只占一半，碰不到候选栏，见 `KeyboardGeometry.hitOffsets`。 */
  private keyHitRegion(): Rectangle[] {
    const halfKey: number = this.keySpacing() / 2;
    const halfRow: number = this.rowSpacing() / 2;
    return this.hitRegion(halfKey, halfRow, halfKey, halfRow);
  }

  /** 九键和笔画键盘左侧标点栏的一格：格与格之间没有空隙，只有首格向上、末格向下扩出半个行距，左右照键扩出半个键距。 */
  private railHitRegion(index: number, count: number): Rectangle[] {
    const halfKey: number = this.keySpacing() / 2;
    const halfRow: number = this.rowSpacing() / 2;
    return this.hitRegion(halfKey, index === 0 ? halfRow : 0, halfKey, index === count - 1 ? halfRow : 0);
  }

  /** 紧挨回车的中/英键：右侧伸过整个键距，再伸进回车键帽 `RETURN_YIELD_VP`，与 `returnKeyHitRegion` 正好相接。 */
  private languageKeyHitRegion(): Rectangle[] {
    const halfKey: number = this.keySpacing() / 2;
    const halfRow: number = this.rowSpacing() / 2;
    return this.hitRegion(halfKey, halfRow, this.keySpacing() + KeyboardGeometry.RETURN_YIELD_VP, halfRow);
  }

  /** 紧挨中/英键的回车键：触摸区整体右移 `RETURN_YIELD_VP`，左边让给中/英（`KeyboardGeometry.shiftedHitOffsets`）。 */
  private returnKeyHitRegion(): Rectangle[] {
    const halfRow: number = this.rowSpacing() / 2;
    return KeyboardGeometry.shiftedHitOffsets(KeyboardGeometry.RETURN_YIELD_VP, halfRow, halfRow)
      .map((offset: HitOffset): Rectangle => {
        const rectangle: Rectangle = { x: offset.x, y: offset.y, width: '100%', height: '100%' };
        return rectangle;
      });
  }

  private hitRegion(left: number, top: number, right: number, bottom: number): Rectangle[] {
    return KeyboardGeometry.hitOffsets(left, top, right, bottom).map((offset: HitOffset): Rectangle => {
      const rectangle: Rectangle = { x: offset.x, y: offset.y, width: '100%', height: '100%' };
      return rectangle;
    });
  }

  /** The adjustment is spread across the key rows so none of them keeps a stray pixel. */
  private rowHeight(index: number): number {
    return KeyboardGeometry.adjustedRowHeight(KeyboardMetrics.ROW_HEIGHT_VP,
      this.geometryHeight, KeyboardMetrics.KEY_ROWS, index);
  }

  /**
   * 空格键上显示的键盘名称，参照 Android 的 `SpaceKeyFace.schemeLabel`：全拼、双拼 · 小鹤、五笔 86 等，英文时为 space。
   *
   * 本地模式运行时显示它的名称，因为此时按键的作用与方案所说的不同，应当让用户知道。英文也出于同样的原因显示：方案仍在底下设着，但显示方案名会让人以为键盘在拼音节，而同样的键其实在拼单词。
   */
  private spaceLabel(): string {
    const mode: LocalInputModeDefinition | null =
      LocalInputMode.fromPreferenceKey(this.localMode);
    if (mode !== null) {
      return mode.title;
    }
    if (this.english) {
      return 'space';
    }
    switch (this.scheme.preferenceId) {
      case 'quanpin':
      case 'nine_key':
      case 'fourteen_key':
      case 'eighteen_key':
        return '全拼';
      case 'nine_key_shuangpin':
        return '九键双拼';
      case 'xiaohe':
        return '双拼 · 小鹤';
      case 'ziranma':
        return '双拼 · 自然码';
      case 'microsoft':
        return '双拼 · 微软';
      case 'shoudao':
        return '双拼 · 首道';
      case 'wubi':
        return KeyboardScheme.normalizedWubiProfile(this.wubiProfile) === KeyboardScheme.WUBI_98
          ? '五笔 98' : '五笔 86';
      case 'japanese':
      case 'japanese_nine_key':
        return '日语';
      case 'korean':
        return '韩语';
      case 'cantonese':
        return '粤拼';
      case 'zhuyin':
        return '注音';
      case 'vietnamese':
        return '越南语';
      case 'tibetan':
        return '藏文';
      case 'stroke':
        return '笔画';
      case 'handwriting':
        return '手写';
      default:
        return KeyboardScheme.title(this.scheme, this.wubiProfile);
    }
  }

  private faceKey(): string {
    // The layer belongs here because the grid keeps its cells across the switch: 2 through 9 send
    // the same characters on both layers, so without it ForEach would judge the subtrees identical
    // and the digit layer would go on reading ABC, DEF, GHI. The twenty-six key rows never needed it
    // because their two layers hold different characters.
    // 大小写不在这里：字母键的键面和读屏文字在绘制时按大小写现算（见 `KeyOptions.letter`），自动大写切换时键节点保持不变，正按着的键不会被重建。
    // 底行样式和数字行改变键的排列，与双拼提示开关一样放进键里，设置页改了之后已画好的键跟着重建。
    return `${this.english}:${this.localMode}:${this.shuangpinProfile}:${this.shuangpinKeyHints}:`
      + `${this.quanpinShift}:${this.engineScheme}:${this.scheme.preferenceId}:${this.symbols}:${this.moreSymbols}:`
      + `${this.twentySixKeyStyle}:${this.fourteenKeyStyle}:${this.nineKeyStyle}:${this.numberRow}:`
      + `${this.twentySixSymbolFirst}:${this.numberPadSpace}:${this.languageKeyLeft}:`
      + `${this.standardRowsField}:${this.skin.key()}`;
  }

  /**
   * Which candidates share a row, decided by the ported wrap policy rather than by Flex.
   *
   * Width is estimated rather than measured: ArkUI reports text extent only after layout and the
   * wrap decision has to precede it. A CJK candidate is about one font size per character, which is
   * close enough to keep a row from overflowing.
   */
  /**
   * How wide one chip on the strip is.
   *
   * Only while glosses are expected, and only on the strip. There the width has to stop depending
   * on whether the gloss has arrived: the networked ones come back over a few hundred milliseconds
   * and a chip that widens when its answer lands pushes every candidate to its right along with it.
   * The columns are decided from the visible width first and the answers fill them afterwards.
   *
   * The vertical list keeps the full width it always had — there each candidate is its own row, so
   * a gloss arriving beside it moves nothing.
   *
   * Width is estimated the way the wrap policy estimates it, one font size per code point, because
   * ArkUI reports text extent only after layout and this has to precede it.
   */
  private candidateChipWidth(candidate: CandidateEntry, index: number): Length {
    if (KeyboardFormFactor.isDesktop() && this.settings.candidateLayout === 'vertical') {
      return '100%';
    }
    const padding: number = KeyboardMetrics.CANDIDATE_PADDING_VP;
    // 桌面候选宽度与窗口估算保持一致，包含序号及其最小保留栏。
    const word: number = KeyboardFormFactor.isDesktop()
      ? CandidateWidthPolicy.chipContentVp(candidate, index, this.settings.candidateFontSize,
        this.candidateNumberSize()) + (this.settings.candidateSkinLayout == null ? 0
          : Math.max(0, 9 - (this.candidateNumberSize() > 0
            ? CandidateWidthPolicy.textWidthVp(`${index + 1}`, this.candidateNumberSize()) : -CandidateWidthPolicy.ORDINAL_GAP_VP)))
      : TextPolicy.characters(candidate.text).length * this.settings.candidateFontSize;
    if (!CandidateChipWidth.reservesColumn(KeyboardSession.shared.glossesExpected(), candidate.source,
      candidate.hunEum)) {
      return CandidateLayoutPolicy.chipWidth(this.candidateGeometry(), this.settings.candidateFontSize, word, padding);
    }
    const column: number = CandidateChipWidth.column(this.availableWidth, 0, padding);
    if (column <= 0) {
      return CandidateLayoutPolicy.chipWidth(this.candidateGeometry(), this.settings.candidateFontSize, word, padding);
    }
    return CandidateChipWidth.chip(word, true, column, padding);
  }

  /** Vertical desktop rows keep glosses beside the word; every horizontal strip reserves below. */
  private candidateUsesGlossLine(): boolean {
    return !(KeyboardFormFactor.isDesktop() && this.settings.candidateLayout === 'vertical')
      && KeyboardSession.shared.glossRows() > 0;
  }

  /** 画出来的组字，见 CandidatePreeditStylePolicy：2in1 的候选窗口照「候选窗口预编辑」；手机没有这个设置，拼写写在输入框里时候选栏不画读音，写不进去（14/18 键、日语、不接受预上屏文字的编辑框）时照旧画。 */
  private visibleComposition(): VisibleComposition {
    return CandidatePreeditStylePolicy.visible(KeyboardFormFactor.isDesktop()
      ? this.settings.candidateShowPreedit : !this.compositionInField, this.phrasePrefix, this.editing, this.localMode);
  }

  /** One candidate row without the reserved gloss line; see KeyboardMetrics.candidateRowHeightVp. */
  private candidateLineHeight(): number {
    return KeyboardMetrics.candidateRowHeightVp(this.settings.candidateFontSize, KeyboardFormFactor.isDesktop());
  }

  private candidateRowHeight(): number {
    return this.candidateLineHeight()
      + (this.candidateUsesGlossLine()
        ? KeyboardMetrics.glossHeightVp(KeyboardSession.shared.glossRows(),
          this.settings.candidateFontSize) : 0);
  }

  private letterKey(letter: string): KeyOptions {
    if (this.koreanFace()) {
      return {
        face: this.letterFace(letter),
        label: this.letterLabel(letter),
        letter: letter,
        size: this.letterFontSize(),
        emphasized: false,
        width: 0,
        action: () => this.tapLetter(letter, this.takeTouch(letter))
      };
    }
    return {
      face: this.letterFace(letter),
      label: this.letterLabel(letter),
      letter: letter,
      hint: this.letterHint(letter),
      cornerHint: this.letterCornerHint(letter),
      size: this.letterFontSize(),
      emphasized: false,
      width: 0,
      action: () => this.tapLetter(letter, this.takeTouch(letter))
    };
  }

  /**
   * 字母键角上的符号提示，与 Android 的 `ImeLetterRows` 在其标准字母键上的绘制一致：26 键字母行的每个键面都有，韩文字母除外（注音有自己的行）；无论「滑动输入符号」是否开启都绘制，因为长按总会输入它。仅限触屏键盘；2in1 的屏幕键盘保持原样。
   */
  private letterCornerHint(letter: string): string {
    if (!this.touchDesign() || this.koreanFace()) {
      return '';
    }
    return LetterHintTable.hint(letter) ?? '';
  }

  /** 触屏上用设计稿的 22fp 字母；2in1 屏幕键盘保持共享的键字号。 */
  private letterFontSize(): number {
    return this.touchDesign() ? TOUCH_LETTER_FONT_SIZE : KeyboardMetrics.KEY_FONT_SIZE;
  }

  /**
   * 第二行字母是否以 ; 键结尾。
   *
   * 微软双拼用分号拼 ing，所以这个键要能在字母行里直接按到，而不是藏在符号层后面。触屏上设计稿把这一行画成两端各缩进半个键的九个字母，; 键只在微软双拼正在组合时才出现，与 Android 一致（`MicrosoftShuangpinKeyPolicy`）。两笔式在那里没有字母，越南语和威利转写也不用它，所以韩语、越南语和藏文把这一行画成九个键。
   */
  private secondRowSemicolon(): boolean {
    if (this.symbols || this.koreanFace() || this.vietnameseFace() || this.tibetanFace()) {
      return false;
    }
    if (!this.touchDesign()) {
      return true;
    }
    return !this.english && this.localMode === 'none'
      && this.scheme.preferenceId === KeyboardScheme.MICROSOFT.preferenceId;
  }

  /** 第二行字母是否两端各缩进半个键：即触屏键盘的九字母行。 */
  private secondRowIndented(): boolean {
    return this.touchDesign() && !this.symbols && !this.secondRowSemicolon();
  }

  /** What a screen reader says for a letter key under the current case. */
  private letterLabel(letter: string): string {
    if (this.koreanFace()) {
      // The jamo itself, which is what a screen reader set to Korean reads.
      return this.letterFace(letter);
    }
    // Ported with the face policy and, until now, computed for nobody: the view attached no
    // accessibility text at all, so every key on this keyboard was silent to a screen reader.
    return LetterKeyFacePolicy.accessibilityLabel(letter,
      !this.english && !this.vietnameseFace() && !this.tibetanFace(),
      this.localMode !== 'none', this.vietnameseFace() || this.tibetanFace()
        ? this.caseMode !== LetterCaseMode.LOWERCASE : this.typesUppercase());
  }

  /** The shared English/primary/fallback order, as one CSS-style list for a Text modifier. */
  private candidateFontFamily(): string {
    return CandidateFontFamilyPolicy.families(this.settings.candidateFontFamily,
      this.settings.candidateEnglishFont, this.settings.candidateFallbackFonts);
  }

  @State private canvasReadySurfaces: string[] = [];
  @State private canvasColorKey: string = '';
  @State private canvasExpandedWidth: number = 0;
  @State private canvasColors: CandidateColors | null = null;

  private canvasReady(surface: string, key: string, ready: boolean): void {
    const identity: string = `${surface}:${key}`;
    this.canvasReadySurfaces = this.canvasReadySurfaces.filter((value: string): boolean => value !== identity);
    if (ready && this.settings.candidateCanvas?.key === key) {
      if (this.canvasColorKey !== key || this.canvasColors === null) {
        this.canvasColors = KeyboardSession.shared.canvasCandidateColors(key);
        this.canvasColorKey = key;
      }
      this.canvasReadySurfaces.push(identity);
    }
  }

  /** 2in1 与 canvas 使用候选区配色；普通触屏条沿用键盘配色。 */
  private candidateColors(): CandidateColors {
    const scene = this.settings.candidateCanvas;
    if (scene !== null && this.canvasColorKey === scene.key && this.canvasColors !== null
      && this.canvasReadySurfaces.some((value: string): boolean => value.endsWith(`:${scene.key}`))) {
      return this.canvasColors;
    }
    if (KeyboardFormFactor.isDesktop() || scene !== null) {
      return this.settings.candidateColors;
    }
    const colors: CandidateColors = {
      surface: this.skin.background,
      border: TRANSPARENT,
      text: this.skin.keyForeground,
      number: this.skin.secondary,
      secondary: this.skin.secondary,
      accent: this.skin.accent,
      selected: TRANSPARENT,
      // 首选画在字母键色的底片上，字用皮肤的首选字色（浅色季节配色加深到 WCAG AA）。
      selectedText: this.skin.candidateAccent,
      selectedNumber: this.skin.accent,
      hover: TRANSPARENT,
      showSelectedBar: false
    };
    return colors;
  }

  /** The 2in1 card's radius: the user's, then the package's, then the HarmonyOS PC window's 16. */
  private candidateCardRadius(): number {
    return this.settings.candidateCornerRadiusVp ?? KeyboardMetrics.CANDIDATE_CARD_CORNER_VP;
  }

  /** 优先使用皮肤行圆角；用户设置卡片圆角时，行圆角不超过卡片。 */
  private candidateItemRadius(): number {
    const radius: number = this.settings.candidateSkinLayout?.itemRadius ?? KeyboardMetrics.CANDIDATE_ITEM_CORNER_VP;
    return this.settings.candidateCornerRadiusUser
      ? Math.min(radius, this.candidateCardRadius())
      : radius;
  }

  /** A card fill or border with the user's window opacity applied; the touch strip is part of the keyboard and stays as it is. */
  private candidateCardColor(colour: string): string {
    return KeyboardFormFactor.isDesktop()
      ? GlobalTheme.withOpacity(colour, this.settings.candidateWindowOpacity) : colour;
  }

  private candidateTextColor(): string {
    return this.candidateColors().text;
  }

  private candidateAccentColor(): string {
    return this.candidateColors().accent;
  }

  private candidateSelectedTextColor(): string {
    return this.candidateColors().selectedText;
  }

  private candidateGeometry(): CandidateSkinGeometry | null {
    const layout = this.settings.candidateSkinLayout;
    if (layout === null || layout === undefined) return null;
    return KeyboardFormFactor.isDesktop() && this.settings.candidateLayout === 'vertical' ? layout.vertical : layout.horizontal;
  }

  private candidateNumberSize(): number {
    return CandidateLayoutPolicy.numberSize(this.candidateGeometry(), this.settings.candidateFontSize,
      CandidateNumberFontPolicy.size(this.settings.candidateFontSize));
  }

  private candidateGap(vertical: boolean): number {
    return CandidateLayoutPolicy.gap(this.settings.candidateSkinLayout, vertical, this.settings.candidateFontSize);
  }

  private candidateRightPadding(): number {
    return CandidateLayoutPolicy.dip(this.candidateGeometry()?.itemPaddingRight,
      this.settings.candidateFontSize, KeyboardMetrics.CANDIDATE_PADDING_VP);
  }

  private candidateBarWidth(): number {
    return Math.max(3, CandidateLayoutPolicy.dip(this.candidateGeometry()?.selectedBarWidth, this.settings.candidateFontSize, 3));
  }

  private candidateBarHeight(): number {
    const height: number = this.candidateGeometry()?.selectedBarHeightEm ?? 0.55;
    return Math.max(2 * this.candidateBarWidth(), height * this.settings.candidateFontSize);
  }

  private candidateNumberColor(highlighted: boolean): string {
    return highlighted ? this.candidateColors().selectedNumber : this.candidateColors().number;
  }

  /** The 2in1 page indicator sits at the end of the composition line, so it goes when that line is switched off; a touch strip scrolls rather than pages. */
  private showsCandidatePager(): boolean {
    return KeyboardFormFactor.isDesktop() && this.settings.candidateShowPreedit
      && this.candidates.length > 0 && CandidatePagerPolicy.visible(this.candidatePageCount);
  }

  /** A candidate window's composition line leads with the brand mark, as the macOS candidate window's top row does; see CandidateLogoPolicy for why a touch strip's does not. */
  private showsCandidateLogo(): boolean {
    return CandidateLogoPolicy.visible(KeyboardFormFactor.isDesktop(), this.settings.candidateShowPreedit);
  }

  /** The spelling: the accent in a candidate window, the keyboard's secondary text over a touch strip, where the candidates below carry the accent. */
  private compositionColor(): string {
    return KeyboardFormFactor.isDesktop() ? this.candidateAccentColor() : this.skin.secondary;
  }

  /** 2in1 卡片上的拼写用设计稿的强调色、字重 600；触屏工具条上的保持常规字重。 */
  private compositionWeight(): number {
    return KeyboardFormFactor.isDesktop() ? 600 : 400;
  }

  /** A touch strip marks the selected candidate by colour and weight alone; it has no fill to do it with. */
  private candidateWeight(candidate: CandidateEntry): number {
    return candidate.highlighted && !KeyboardFormFactor.isDesktop() ? 600 : 400;
  }

  private showsHandwritingCandidates(): boolean {
    return SurfaceRoutingPolicy.showsHandwritingCandidates(KeyboardFormFactor.isDesktop(), this.desktop,
      KeyboardScheme.usesHandwritingFace(this.scheme, this.english, this.symbols, this.localMode))
      && this.handwritingCandidates.length > 0;
  }

  /**
   * 触屏键盘这一刻画不画工具条那一行。工具栏设成隐藏或工具全关时，空闲的键盘不画它；组字时它是候选栏，照常画；有面板盖住按键时也画，标志键是回到按键的出口，否则从底行打开的表情面板就没有路回去。九键长按和日文变体只换掉按键行里的键面，不算面板。2in1 不走触屏框架，一律算画。
   */
  private stripShown(): boolean {
    if (!this.touchDesign()) {
      return true;
    }
    // 键盘调整盖在整块键盘上，下面画的是收起遮罩之后的样子，所以也不算面板：工具栏隐藏时预览里同样没有这一行，高度才对得上。
    const panelOpen: boolean = this.surface !== SURFACE_NONE && this.surface !== SURFACE_NINE_KEY_HOLD
      && this.surface !== SURFACE_VARIANTS && this.surface !== SURFACE_GEOMETRY;
    // 长空格样式把符号入口放在工具栏上，与 Android 一样这一行一直画着；看的是这块键盘而不是此刻的中/英（`longSpaceToolbarRow`），切到英文 26 键时也留着，面板高度不跳。
    return TouchToolbarPolicy.stripShown(TouchToolbarPolicy.rowHidden(this.toolbarTools) && !this.longSpaceToolbarRow(),
      this.stripIdle(), panelOpen);
  }

  /** 工具条出现或消失时让面板跟着改高度，没变时什么也不做：组字时每打一个字母都会走到这里。 */
  private syncStripHeight(): void {
    this.rememberPanelFace();
    if (this.stripShown() !== this.stripWasShown || this.extraRows() !== this.extraRowsWere
      || this.bottomLiftVp() !== this.liftWas) {
      this.resizeTouchPanel();
    }
  }

  /** 按当前的行距、高度调整、底部抬高和工具条显示与否给手机面板定高度；2in1 上会话不做任何事。 */
  private resizeTouchPanel(): void {
    this.rememberPanelFace();
    this.stripWasShown = this.stripShown();
    this.extraRowsWere = this.extraRows();
    this.liftWas = this.bottomLiftVp();
    KeyboardSession.shared.resizeTouchKeyboard(KeyboardMetrics.totalHeightVp(
      this.geometryRowSpacing, this.geometryHeight, KeyboardSession.shared.glossRows(),
      this.settings.candidateFontSize, this.settings.candidatePreeditFontSize, false, this.stripWasShown,
      this.extraRowsWere, this.settings.candidateScalePercent, this.liftWas));
  }

  /** 把这一刻的键面里决定第一帧高度的两件事交给会话记下（只在变了时写盘），下次冷启动时扩展能力照它建面板（`TouchLayoutStylePolicy.firstPanel`）。2in1 不走触屏面板，不记。 */
  private rememberPanelFace(): void {
    if (!this.touchDesign()) {
      return;
    }
    KeyboardSession.shared.rememberTouchPanelFace({
      lettersRow: TouchLayoutStylePolicy.lettersRowFace(this.numberRowFace()),
      chineseTwentySix: this.chineseTwentySixKeyboard()
    });
  }

  /** 候选条的读音行要不要占位。没有 canvas 时一直占位，候选不会因为它出现、消失而上下移动；有 canvas 时只在有东西显示时占位，见 `stripCanvasContentHeight`。 */
  private readingRowShown(): boolean {
    return this.settings.candidateCanvas === null || this.diagnostic.length > 0 || this.visibleComposition().caret
      || this.visibleComposition().text.length > 0 || this.readingRowSpellings();
  }

  /** canvas 在候选条里至少要让出的内容高度：34vp 的候选通道，读音行占位时再加它和间距，都按「候选栏大小」缩放。皮肤的留白按它缩放，高亮因此不会画到面板外面。 */
  private stripCanvasContentHeight(): number {
    return this.chipHeight() + (this.readingRowShown() ? this.stripSized(PREEDIT_LINE_VP) + this.stripSized(CANDIDATE_BAR_GAP_VP) : 0);
  }

  /** 触屏工具条的高度：设计的 50vp 乘以「候选栏大小」（`candidate_scale_percent`，#10137）。工具栏和候选栏共用这一行，组字开始、结束时按键不动。 */
  private stripHeight(): number {
    return KeyboardMetrics.touchStripHeightVp(this.settings.candidateScalePercent, KeyboardSession.shared.glossRows());
  }

  /** 候选胶囊的高度：34vp，释义和英文音标都打开时再加第二行注释（`KeyboardMetrics.touchExtraDetailVp`），按「候选栏大小」缩放。 */
  private chipHeight(): number {
    return this.stripSized(CHIP_HEIGHT_VP + KeyboardMetrics.touchExtraDetailVp(KeyboardSession.shared.glossRows()));
  }

  /** 候选栏里的一段长度或字号按「候选栏大小」缩放：读音行、候选通道、注释行和细线跟着工具条一起变矮或变高。 */
  private stripSized(value: number): number {
    return value * this.settings.candidateScalePercent / 100;
  }

  /** 工具栏按钮、图标和展开箭头只随工具条缩小、不随它放大（`KeyboardMetrics.toolbarScale`）：工具条矮于 50vp 时不会伸出这一行被裁掉。 */
  private toolbarSized(value: number): number {
    return value * KeyboardMetrics.toolbarScale(this.settings.candidateScalePercent);
  }

  /** Nothing to spell, choose or report on a touch keyboard: the strip is the logo bar. */
  private stripIdle(): boolean {
    return !KeyboardFormFactor.isDesktop() && this.editing.length === 0
      && this.diagnostic.length === 0 && this.candidates.length === 0
      && this.englishSuggestions.length === 0 && this.emailSuffixes.length === 0
      && !this.showsHandwritingCandidates()
      && this.nineKeyQuickMarks.length === 0;
  }

  /** "current / total" and the two arrows, in the palette's secondary colour, as the design's `pageInfo` and `pageBtn` draw them. The arrows page through the Engine the way PageUp and PageDown do. */
  @Builder
  candidatePager() {
    Row() {
      Text(CandidatePagerPolicy.label(this.candidatePage, this.candidatePageCount))
        .fontSize(CandidatePagerPolicy.FONT_SIZE)
        .fontColor(this.candidateColors().secondary)
        .fontFamily(this.candidateFontFamily())
        .maxLines(1)
      Text(CandidatePagerPolicy.PREVIOUS)
        .fontSize(CandidatePagerPolicy.FONT_SIZE)
        .fontColor(this.candidateColors().secondary)
        .padding({ left: CandidatePagerPolicy.ARROW_PADDING_VP, right: CandidatePagerPolicy.ARROW_PADDING_VP })
        .accessibilityText(CandidatePagerPolicy.arrowLabel(false))
        .onClick((): void => {
          KeyboardSession.shared.previousPage();
        })
      Text(CandidatePagerPolicy.NEXT)
        .fontSize(CandidatePagerPolicy.FONT_SIZE)
        .fontColor(this.candidateColors().secondary)
        .padding({ left: CandidatePagerPolicy.ARROW_PADDING_VP, right: CandidatePagerPolicy.ARROW_PADDING_VP })
        .accessibilityText(CandidatePagerPolicy.arrowLabel(true))
        .onClick((): void => {
          KeyboardSession.shared.nextPage();
        })
    }
    .margin({ left: CandidatePagerPolicy.LEADING_GAP_VP })
    .flexShrink(0)
  }

  /**
   * The composition gets a line of its own. Sharing the candidate row costs it width on the one row
   * where width is worth most, which is the reasoning the iOS implementation records.
   */
  /** The mascot over the card's top edge, placed as every host places it: see CandidateDecorationLayout. The card spans the strip's width, with its top edge below the band. */
  candidateDecorationRect(): CandidateDecorationRect | null {
    if (this.settings.candidateDecorationImage === null) {
      return null;
    }
    return CandidateDecorationLayout.rect(this.settings.candidateDecorationAlign, 0, this.availableWidth,
      this.settings.candidateDecorationTopVp, KeyboardMetrics.CANDIDATE_PADDING_VP,
      CandidateDecorationLayout.OVERLAP_VP, this.settings.candidateDecorationTopVp,
      this.settings.candidateDecorationWidthVp, this.settings.candidateDecorationAspectRatio);
  }

  /** The package background: over the surface colour, under the candidates, clipped with the card. */
  @Builder
  candidateBackground() {
    if (this.settings.candidateBackgroundImage !== null) {
      Image(this.settings.candidateBackgroundImage)
        .width('100%')
        .height('100%')
        .objectFit(this.settings.candidateBackgroundFit === 'contain' ? ImageFit.Contain
          : this.settings.candidateBackgroundFit === 'stretch' ? ImageFit.Fill : ImageFit.Cover)
        .opacity(this.settings.candidateBackgroundOpacity * this.settings.candidateWindowOpacity)
    }
  }

  /** 2in1 候选窗：组合行在候选上方，整体是一张卡片。触屏键盘改为绘制 `touchStrip()`。 */
  @Builder
  candidateStrip() {
    CandidateCanvasFrame({ scene: this.settings.candidateCanvas,
      surfaceOpacity: this.settings.candidateWindowOpacity,
      fallbackColor: this.settings.candidateColors.surface,
      onSize: (width: number): void => { this.availableWidth = width; },
      onPanelReady: (key: string, ready: boolean): void => this.canvasReady('strip', key, ready) }) {
    Stack({ alignContent: Alignment.Top }) {
      Column() {
      // The composition line, reserved whether or not anything is being composed so that nothing
      // below it shifts when one starts. Composing it shows the spelling, and a diagnostic takes it
      // over while it lasts.
      Row() {
        if (this.showsCandidateLogo()) {
          // The full-colour application mark rather than the keyboard_brand path the touch strip tints: the mark the macOS candidate window, toolbar and mode badge lead with.
          Image($r('app.media.app_icon'))
            .width(CandidateLogoPolicy.SIDE_VP)
            .height(CandidateLogoPolicy.SIDE_VP)
            .objectFit(ImageFit.Contain)
            .margin({ right: CandidateLogoPolicy.GAP_VP })
            .flexShrink(0)
            .accessibilityText('水杉输入法')
        }
        if (this.diagnostic.length > 0 || !KeyboardFormFactor.isDesktop()
            || this.settings.candidateShowPreedit) {
          // The caret the segment keys move. The source draws it as a bar inside the composition
          // (`.cursor`), and Ctrl+Left, Ctrl+Right and Ctrl+Backspace were already moving one here
          // that nothing drew — a segment editor whose caret is invisible cannot be used, because
          // there is no way to tell what the next key will affect. A diagnostic line is not a
          // composition and gets no caret.
          if (this.diagnostic.length > 0) {
            Text(this.diagnostic)
              .fontSize(this.settings.candidatePreeditFontSize)
              .fontColor(this.candidateAccentColor())
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
          } else {
            if (this.visibleComposition().caret) {
              Row() {
                Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).before)
                  .fontSize(this.settings.candidatePreeditFontSize)
                  .fontWeight(this.compositionWeight())
                  .fontColor(this.compositionColor())
                  .fontFamily(this.candidateFontFamily())
                  .maxLines(1)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
                Divider()
                  .vertical(true)
                  .strokeWidth(PREEDIT_CARET_WIDTH_VP)
                  .height(PreeditCaretPolicy.height(this.settings.candidatePreeditFontSize))
                  .color(this.candidateAccentColor())
                Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).after)
                  .fontSize(this.settings.candidatePreeditFontSize)
                  .fontWeight(this.compositionWeight())
                  .fontColor(this.compositionColor())
                  .fontFamily(this.candidateFontFamily())
                  .maxLines(1)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
              }
            } else {
              // 「候选栏预编辑：不显示」 on the phone: the line keeps its height and loses the spelling, but not the chosen part of a phrase or a local mode's trigger.
              Text(this.visibleComposition().text)
                .fontSize(this.settings.candidatePreeditFontSize)
                .fontWeight(this.compositionWeight())
                .fontColor(this.compositionColor())
                .fontFamily(this.candidateFontFamily())
                .maxLines(1)
                .textOverflow({ overflow: TextOverflow.Ellipsis })
            }
          }
        } else {
          Blank()
        }
        Blank()
        if (this.effectBadge.length > 0 && this.comboOnCompositionLine()) {
          this.comboBadge()
        }
        if (this.showsHanjaButton()) {
          this.hanjaButton()
        }
        if (this.showsCandidatePager()) {
          this.candidatePager()
        }
      }
      .width('100%')
      .height(KeyboardFormFactor.isDesktop() && !this.settings.candidateShowPreedit
        ? 0 : KeyboardMetrics.compositionRowHeightVp(this.settings.candidatePreeditFontSize))
      .margin({ bottom: this.settings.candidateShowPreedit ? this.settings.candidateCanvas?.lineGapDip ?? 0 : 0 })
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      // 候选窗没有需要填充的空闲状态；触屏工具栏打开的那些界面，在屏幕空间充足的机器上可以从应用里进入。
      if (this.showsHandwritingCandidates()) {
        // OCR results are candidates too. Putting them in this shared strip avoids stacking an
        // otherwise-idle shortcut row above a private row inside the canvas, and gives every result
        // a full touch target in a horizontally scrollable lane.
        this.handwritingCandidateRow()
      } else if (this.candidates.length > 0) {
        this.candidateRow()
      } else if (this.englishSuggestions.length > 0) {
        // The same row the candidates use. Direct English has no composition, so the strip is idle
        // and the completions are the only thing that would be on it.
        this.englishSuggestionRow()
      }
    }
      .width('100%')
      // 2in1 卡片内设计稿的 8vp，`KeyboardMetrics.candidateHeightVp` 和 `CandidateWidthPolicy.EXTRA_WIDTH_VP` 按此给窗口定尺寸。
      .padding(KeyboardFormFactor.isDesktop() ? KeyboardMetrics.CANDIDATE_CARD_PADDING_VP : 0)
      .margin({ top: this.settings.candidateDecorationTopVp })
      .backgroundColor(this.settings.candidateCanvas === null ? this.candidateCardColor(this.candidateColors().surface) : TRANSPARENT)
      .background(this.candidateBackground)
      .clip(this.settings.candidateBackgroundImage !== null)
      .borderRadius(KeyboardFormFactor.isDesktop()
        ? this.candidateCardRadius()
        : KeyboardMetrics.STRIP_CORNER_VP)
      .borderWidth(KeyboardFormFactor.isDesktop() && this.settings.candidateCanvas === null ? this.settings.candidateSkinLayout?.borderWidth ?? 1 : 0)
      .borderColor(this.candidateCardColor(this.candidateColors().border))
      .onAreaChange((_old: Area, now: Area): void => {
        if (this.settings.candidateCanvas === null) this.availableWidth = Number(now.width);
        this.effectCardHeight = Number(now.height);
      })
      // The typing-effect flash: an accent wash over the card that fades after each key. Always present on a 2in1 at opacity 0, so a fade is never cut short by the node going away; it takes no touches.
      if (KeyboardFormFactor.isDesktop()) {
        Column()
          .position({ x: 0, y: this.settings.candidateDecorationTopVp })
          .width('100%')
          .height(this.effectCardHeight)
          .backgroundColor(this.effectColor ?? this.candidateAccentColor())
          .borderRadius(this.settings.candidateCornerRadiusVp ?? KeyboardMetrics.CANDIDATE_CARD_CORNER_VP)
          .opacity(this.effectFlash)
          .hitTestBehavior(HitTestMode.Transparent)
        if (this.effectBadge.length > 0 && !this.comboOnCompositionLine()) {
          Row() {
            this.comboBadge()
          }
          .position({ x: 0, y: this.settings.candidateDecorationTopVp })
          .width('100%')
          .justifyContent(FlexAlign.End)
          .padding({ right: KeyboardMetrics.CANDIDATE_PADDING_VP })
          .hitTestBehavior(HitTestMode.Transparent)
        }
      }
      // 后画装饰组，覆盖卡片顶边；图片和文字都不接收点击。
      if (this.settings.candidateDecorationImage !== null && this.candidateDecorationRect() !== null) {
        ForEach([this.settings.candidateDecorationImage], (image: string) => {
          CandidateDecorationView({
            image: image, rect: this.candidateDecorationRect()!,
            boxWidth: this.settings.candidateDecorationWidthVp,
            boxHeight: this.settings.candidateDecorationTopVp,
            imageAlign: this.settings.candidateDecorationAlign,
            fontFamily: this.candidateFontFamily(), style: this.settings.candidateDecorationCandidate,
            candidates: this.candidates
          })
        }, (image: string): string => image)
      }
    }
    .width('100%')
    }
  }

  /**
   * 漢, the touch counterpart of a Korean keyboard's Hanja key, at the end of the composition line. It sits on that line rather than among the candidates so it stays put while the list it opens fills the row below, and its filled face says the list is open.
   */
  @Builder
  hanjaButton() {
    Text(this.zhuyinComposition() ? '選' : '漢')
      .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
      .fontColor(this.skin.accent)
      .fontFamily(this.candidateFontFamily())
      .maxLines(1)
      .height(this.stripSized(CHIP_HEIGHT_VP))
      .padding({ left: CHIP_PADDING_VP, right: CHIP_PADDING_VP })
      .borderRadius(CHIP_RADIUS_VP)
      .backgroundColor(this.openableListShowing() ? this.skin.keyBackground : TRANSPARENT)
      .flexShrink(0)
      .stateStyles({
        normal: {
          .opacity(1)
        },
        pressed: {
          .opacity(BAR_BUTTON_PRESSED_OPACITY)
        }
      })
      .accessibilityText(this.zhuyinComposition()
        ? KeyAccessibilityPolicy.zhuyinList(this.openableListShowing())
        : KeyAccessibilityPolicy.hanja(this.openableListShowing()))
      .onClick((): void => {
        this.convertKoreanSyllableToHanja();
      })
  }

  /** The typing combo: its count in the accent colour, swelling in power mode. */
  @Builder
  comboBadge() {
    Text(this.effectBadge)
      .fontSize(Math.max(this.settings.candidatePreeditFontSize - 2, 10))
      .fontColor(this.candidateAccentColor())
      .fontFamily(this.candidateFontFamily())
      .fontWeight(FontWeight.Medium)
      .maxLines(1)
      .flexShrink(0)
      .margin({ left: CandidateLogoPolicy.GAP_VP, right: CandidateLogoPolicy.GAP_VP })
      .scale({ x: this.effectBadgeScale, y: this.effectBadgeScale })
      .hitTestBehavior(HitTestMode.Transparent)
      .accessibilityText(this.effectBadge)
  }

  /** OCR results use the shared candidate lane, but select through the recognizer rather than Engine. */
  @Builder
  handwritingCandidateRow() {
    Scroll() {
      Row({ space: this.keySpacing() }) {
        ForEach(this.handwritingCandidates, (candidate: string) => {
          Text(candidate)
            .fontSize(this.settings.candidateFontSize)
            .fontFamily(this.candidateFontFamily())
            .fontColor(this.candidateTextColor())
            .accessibilityText(`手写候选：${candidate}`)
            .textAlign(TextAlign.Center)
            .width(this.candidateLineHeight())
            .height(this.candidateLineHeight())
            .backgroundColor(this.handwritingSkin.keyBackground)
            .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
            .onClick(() => this.chooseHandwriting(candidate))
        }, (candidate: string): string => candidate)
      }
      .height(this.candidateRowHeight())
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
    .height(this.candidateRowHeight())
    .align(Alignment.Start)
  }

  /** Read-only English completions, drawn where the candidates would be. */
  @Builder
  englishSuggestionRow() {
    Scroll() {
      Row() {
        ForEach(this.englishSuggestions, (word: string, index: number) => {
          Text(word)
            .fontSize(this.settings.candidateFontSize)
            .fontFamily(this.candidateFontFamily())
            .fontColor(this.candidateTextColor())
            .accessibilityText(KeyAccessibilityPolicy.englishSuggestion(index + 1, word))
            .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
              right: KeyboardMetrics.CANDIDATE_PADDING_VP })
            .height(this.candidateLineHeight())
            .onClick(() => KeyboardSession.shared.useEnglishSuggestion(index))
        }, (word: string, index: number) => `${index}:${word}`)
      }
      .height(this.candidateRowHeight())
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
    .height(this.candidateRowHeight())
    .align(Alignment.Start)
  }

  @Builder
  candidateRow() {
    if (KeyboardFormFactor.isDesktop() && this.settings.candidateLayout === 'vertical') {
      Column({ space: this.candidateGap(true) }) {
        ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
          this.candidateCell(candidate, index)
        }, (candidate: CandidateEntry, index: number) =>
          `${index}:${JSON.stringify(candidate)}`)
      }
      .width('100%')
      // One row per candidate here and exactly one there. The two used to share a ternary applied to
      // the if/else as a whole, which ArkUI cannot do: modifiers belong to a component, and a branch
      // is not one. Each branch states its own height, and the condition already decided which.
      .height(this.candidateRowHeight() * KeyboardMetrics.visibleCandidateRows(this.candidates.length)
        + this.candidateGap(true) * Math.max(0, KeyboardMetrics.visibleCandidateRows(this.candidates.length) - 1))
      .onAxisEvent((event?: AxisEvent): void => {
        if (!this.settings.navigation.mouseWheel || event === undefined) {
          return;
        }
        const delta: number = event.getVerticalAxisValue();
        if (CandidateWheelPolicy.previousPage(delta)) {
          KeyboardSession.shared.previousPage();
        } else if (CandidateWheelPolicy.nextPage(delta)) {
          KeyboardSession.shared.nextPage();
        }
      })
    } else {
      Row() {
      Scroll() {
        Row({ space: this.candidateGap(false) }) {
          ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
            this.candidateCell(candidate, index)
          }, (candidate: CandidateEntry, index: number) =>
            `${index}:${JSON.stringify(candidate)}`)
        }
        .height(this.candidateRowHeight())
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .layoutWeight(1)
      .height(this.candidateRowHeight())
      .align(Alignment.Start)
      // 候选窗显示整页，用按键和滚轮翻页，与 Windows 版一致；触屏候选栏的展开箭头是手机上查看放不下的候选的方式。
      }
      .width('100%')
      .height(this.candidateRowHeight())
      .onAxisEvent((event?: AxisEvent): void => {
        if (!KeyboardFormFactor.isDesktop() || !this.settings.navigation.mouseWheel
            || event === undefined) {
          return;
        }
        const delta: number = event.getVerticalAxisValue();
        if (CandidateWheelPolicy.previousPage(delta)) {
          KeyboardSession.shared.previousPage();
        } else if (CandidateWheelPolicy.nextPage(delta)) {
          KeyboardSession.shared.nextPage();
        }
      })
    }
  }

  /**
   * 释义、注释和编码提示的颜色：调色板的 `secondary`，它自带透明度，不再减淡。2in1 候选窗取候选区调色板，触屏条取键盘的，对应设计稿的 `candSelTr` 和 `kb.sub`。2in1 的选中行按 `GlobalTheme.candidateDetail` 取选中序号色（和 macOS 的 candSelTr 一样），默认浅色的次要色在选中淡底上只有约 4.0:1，选中序号色到 4.5:1；皮肤包自己给了释义色时照旧用它。
   */
  private candidateDetailColor(highlighted: boolean): string {
    const colors: CandidateColors = this.candidateColors();
    return KeyboardFormFactor.isDesktop() ? GlobalTheme.candidateDetail(colors, highlighted) : colors.secondary;
  }

  @Builder
  candidatePrimaryLine(candidate: CandidateEntry) {
    Row() {
      Text(candidate.text).fontSize(this.settings.candidateFontSize)
        .fontWeight(this.candidateWeight(candidate))
        .fontColor(this.candidateWordColor(candidate))
      if (candidate.badge.length > 0) {
        Text(candidate.badge).fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateAccentColor()).opacity(0.8).padding({ left: 2 })
      }
      if (candidate.hint.length > 0) {
        Text(candidate.hint).fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted))).padding({ left: 2 })
      }
      // Only where there is no gloss line, the vertical 2in1 list; everywhere else the 훈음 is on the line under the Hanja.
      if (candidate.hunEum.length > 0 && !this.candidateUsesGlossLine()) {
        Text(candidate.hunEum)
          .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted))).padding({ left: 4 })
      }
      // 只为音标预留了一行（`translationRows()` 为 0）时下面没有释义行，译文和竖排一样接在词旁边，和触屏胶囊的 `chipAnnotationBeside` 同一条规则。
      if (candidate.annotation.length > 0
        && (!this.candidateUsesGlossLine() || KeyboardSession.shared.translationRows() === 0)) {
        if (candidate.annotationIsTranslation) {
          Text(candidate.annotation)
            .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
            .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted)))
            .padding({ left: CandidateTranslationStyle.gap(this.settings.candidateFontSize) })
        } else {
          Text(candidate.annotation)
            .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
            .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted))).padding({ left: 4 })
        }
      } else if (candidate.annotation.length > 0 && !candidate.annotationIsTranslation) {
        Text(candidate.annotation)
          .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted))).padding({ left: 4 })
      }
      // 竖排 2in1 候选窗没有释义行，音标和释义一样接在旁边。
      if (candidate.pronunciation.length > 0 && !this.candidateUsesGlossLine()) {
        Text(candidate.pronunciation)
          .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
          .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted)))
          .padding({ left: CandidateTranslationStyle.gap(this.settings.candidateFontSize) })
      }
    }
    .alignItems(VerticalAlign.Center)
  }

  /**
   * What a screen reader says for one candidate.
   *
   * Computed here because a @Builder body may hold only the UI description, the same reason
   * `candidateDetailColor` is a method. The suffix goes through the ported gloss policy rather
   * than being written again: it already decides whether an annotation reads as 提示 or as
   * 英文释义, and that distinction is not one to re-derive at the call site.
   */
  private candidateLabel(candidate: CandidateEntry, index: number): string {
    const annotation: string | null =
      candidate.annotation.length > 0 ? candidate.annotation : null;
    const suffix: string = CandidateGlossPolicy.hunEumAccessibilitySuffix(candidate.hunEum)
      + CandidateGlossPolicy.accessibilitySuffix(
        candidate.annotationIsTranslation ? null : annotation,
        candidate.annotationIsTranslation ? annotation : null, true)
      + CandidatePronunciationPolicy.accessibilitySuffix(candidate.pronunciation)
      + CorrectionMarkPolicy.accessibilitySuffix(candidate.confidentCorrection);
    return KeyAccessibilityPolicy.candidate(index + 1, candidate.text, candidate.hint, suffix);
  }

  /** 候选窗与触屏胶囊画字所用的令牌，首选纠错行（`confidentCorrection`）的标记色由它经 `CorrectionMarkPolicy.markColor` 算出。 */
  private candidateSurfaceColors(): CorrectionSurfaceColors {
    const colors: CorrectionSurfaceColors = {
      text: this.candidateTextColor(),
      highlightedText: this.candidateSelectedTextColor(),
      accent: this.candidateAccentColor()
    };
    return colors;
  }

  /** 候选窗与触屏胶囊上候选字的颜色：在线占位行画次要色，首选纠错行画标记色，其余按固定词与高亮的规则。 */
  private candidateWordColor(candidate: CandidateEntry): string {
    if (CandidatePresentationPolicy.aiPlaceholder(candidate.source)) return this.candidateDetailColor(candidate.highlighted);
    return CorrectionMarkPolicy.tint(candidate.confidentCorrection, candidate.highlighted,
      CandidateSkinPolicy.rowTextColor(candidate.highlighted, candidate.fixedPosition > 0,
        this.candidateTextColor(), this.candidateSelectedTextColor()),
      this.candidateSurfaceColors());
  }

  /** 这一行注释、释义、编码提示的颜色：`base` 是这一处原来的次要色，首选纠错行换成标记色，原始按键注释因此在高亮的首选上也看得出标记。 */
  private candidateDetailTint(candidate: CandidateEntry, base: string): string {
    return CorrectionMarkPolicy.tint(candidate.confidentCorrection, candidate.highlighted, base,
      this.candidateSurfaceColors());
  }

  /** 展开面板格子的令牌：有候选包画布时用候选区配色，没有时格子是按键，高亮格用 `toggleForeground`，与格子的底色同源。 */
  private expandedSurfaceColors(): CorrectionSurfaceColors {
    const keyboard: CorrectionSurfaceColors = {
      text: this.skin.keyForeground,
      highlightedText: this.skin.toggleForeground,
      accent: this.skin.accent
    };
    return CorrectionMarkPolicy.expandedSurface(this.settings.candidateCanvas !== null,
      this.candidateSurfaceColors(), keyboard);
  }

  /** The line under a candidate: a Korean Hanja's 훈음 and then the translation. An Engine annotation is not on it; that stays beside the word. */
  private candidateGlossLine(candidate: CandidateEntry): string {
    return CandidateGlossPolicy.glossLine(candidate.hunEum,
      candidate.annotationIsTranslation ? candidate.annotation : '');
  }

  @Builder
  candidateCell(candidate: CandidateEntry, index: number) {
    Row() {
      if (candidate.highlighted && this.settings.candidateSelectedBar) {
        Blank().width(this.candidateBarWidth()).height(this.candidateBarHeight())
          .backgroundColor(this.candidateColors().accent).borderRadius(2)
      }
      if (CandidateNumberFontPolicy.visible(KeyboardFormFactor.isDesktop())) {
        Text(this.candidateNumberSize() === 0 ? '' : `${index + 1}`).fontSize(Math.max(1, this.candidateNumberSize()))
          .constraintSize({ minWidth: this.settings.candidateSkinLayout == null ? 0 : 9 })
          .fontColor(this.candidateNumberColor(candidate.highlighted)).padding({ right: 4 })
      }
      if (this.candidateUsesGlossLine()) {
        Column() {
          this.candidatePrimaryLine(candidate)
          if (KeyboardSession.shared.translationRows() > 0) {
            Text(this.candidateGlossLine(candidate).length > 0 ? this.candidateGlossLine(candidate) : ' ')
              .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
              .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted)))
              // The line is reserved whether or not this candidate has a gloss, so the row heights stay level.
              .opacity(this.candidateGlossLine(candidate).length > 0 ? 1 : 0)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .height(KeyboardMetrics.glossHeightVp(KeyboardSession.shared.translationRows(),
                this.settings.candidateFontSize))
          }
          // 音标在释义下面单独一行，和 macOS、Android 相同；这一行同样按设置预留，音标没到或这个词查不到时留空，行高不跳。
          if (KeyboardSession.shared.pronunciationRows() > 0) {
            Text(candidate.pronunciation.length > 0 ? candidate.pronunciation : ' ')
              .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
              .fontColor(this.candidateDetailTint(candidate, this.candidateDetailColor(candidate.highlighted)))
              .opacity(candidate.pronunciation.length > 0 ? 1 : 0)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .height(KeyboardMetrics.glossHeightVp(KeyboardSession.shared.pronunciationRows(),
                this.settings.candidateFontSize))
          }
        }
        .alignItems(HorizontalAlign.Start)
      } else {
        this.candidatePrimaryLine(candidate)
      }
    }.alignItems(VerticalAlign.Center)
      .accessibilityText(this.candidateLabel(candidate, index))
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP, right: this.candidateRightPadding() })
      .height(this.candidateRowHeight()).width(this.candidateChipWidth(candidate, index))
      .backgroundColor(index === this.hoveredCandidate ? this.candidateColors().hover
        : candidate.highlighted ? this.candidateColors().selected : TRANSPARENT)
      .borderRadius(KeyboardFormFactor.isDesktop() ? this.candidateItemRadius() : this.settings.candidateSkinLayout?.itemRadius ?? 0)
      .onClick(() => KeyboardSession.shared.choose(index))
      .onHover((isHover: boolean): void => { this.hoveredCandidate = isHover ? index : -1; })
      .gesture(LongPressGesture().onAction(() => this.openTouchCandidateMenu(index)))
      // The source opens this menu with a right click on the candidate window; it has no touch
      // screen to long-press on. A 2in1 has both pointers, so it gets both gestures rather than
      // being told to hold the left button down on a candidate and wait.
      .onMouse((event: MouseEvent): void => {
        if (!KeyboardFormFactor.isDesktop() || !CandidateContextMenuPolicy.opens(
          event.button as number as PointerButton,
          event.action as number as PointerAction)) {
          return;
        }
        if (!KeyboardSession.shared.candidateManagementAllowed()) {
          return;
        }
        event.stopPropagation();
        this.punctuation = false;
        this.touchCandidateMenu = false;
        this.managing = index;
      })
  }

  /**
   * 触屏键盘的工具条，始终为 `KeyboardMetrics.stripHeightVp` 高（设计的 50vp 乘以「候选栏大小」），按键因此从不移动：没有输入时是工具栏，其他时候是候选栏。键盘调整时它照常画在遮罩下面。
   */
  @Builder
  touchStrip() {
    CandidateCanvasFrame({ scene: this.stripIdle() || this.surface === SURFACE_GEOMETRY ? null : this.settings.candidateCanvas,
      fixedContent: true, frameHeight: this.stripHeight(), fallbackColor: this.settings.candidateColors.surface,
      minimumContentHeight: this.stripCanvasContentHeight(),
      onSize: (width: number): void => { this.availableWidth = width; },
      onPanelReady: (key: string, ready: boolean): void => this.canvasReady('strip', key, ready) }) {
    Column() {
      if (this.stripIdle()) {
        this.toolbarRow()
      } else {
        this.candidateBar()
      }
    }
    .width('100%')
    .height('100%')
    // 空闲时这里是工具栏，属于键盘：canvas 这时不画（见上面的 `scene`），底色也要用键盘的。用候选包的 `surface` 时，皮肤的白卡片配上按键前景色的图标，图标几乎看不见。
    .backgroundColor(this.stripIdle() ? this.skin.background
      : this.settings.candidateCanvas === null ? this.candidateCardColor(this.candidateColors().surface) : TRANSPARENT)
    .background(this.candidateBackground)
    .clip(this.settings.candidateBackgroundImage !== null)
    .borderRadius(KeyboardMetrics.STRIP_CORNER_VP)
    .onAreaChange((_old: Area, now: Area): void => {
      if (this.settings.candidateCanvas === null) this.availableWidth = Number(now.width);
    })
    }
  }

  /**
   * 空闲时的工具条，即设计稿中的鸿蒙工具栏：等宽格，依次是标志（功能面板）、「表情」「常用语」「剪贴板」「皮肤」「输入方式」和「收起」。中间五个工具按 `harmony_toolbar` 显示，关掉的不占格；标志和收起固定在两端。
   *
   * 面板已打开的工具显示为切换色底、强调色图标，再点一次收起面板。以前在这里以胶囊显示的语言、方案名、翻译和标点，现在和语音、回复面板、设置一样放在底行和功能面板里。长按这一行的任何按钮（含标志和收起）打开设置应用的「屏幕键盘」页，在那里选显示哪些工具，与 Android 长按工具栏打开「编辑工具栏」相同。
   */
  @Builder
  toolbarRow() {
    Row() {
      Column() {
        BrandTile({
          active: this.brandActive(),
          toggleBackground: this.skin.toggleBackground,
          framePath: KeyboardIconPaths.BRAND_FRAME,
          strokePath: KeyboardIconPaths.BRAND_STROKE,
          sizeScale: KeyboardMetrics.toolbarScale(this.settings.candidateScalePercent),
          spokenLabel: KeyAccessibilityPolicy.tools(),
          onTap: (): void => this.tapBrand(),
          onHold: (): void => this.openToolbarSettings()
        })
      }
      .layoutWeight(1)
      .height('100%')
      .justifyContent(FlexAlign.Center)
      if (this.longSpaceActive()) {
        // 中文 26 键的长空格样式：底行没有 符，符号面板入口在标志右边，与 Android 长空格布局的顶部符号键同位。
        this.toolbarTextButton('符', SURFACE_SYMBOLS, KeyAccessibilityPolicy.symbolPanel(), (): void => {
          if (this.surface === SURFACE_SYMBOLS) {
            this.show(SURFACE_NONE);
            return;
          }
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
          this.openSymbolSurface();
        });
      }
      if (this.toolbarToolShown(this.toolbarTools.emoji)) {
        this.toolbarButton(KeyboardIconPaths.TOOLBAR_EMOJI, SURFACE_EMOJI, KeyAccessibilityPolicy.emoji(),
          (): void => {
            KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_EMOJI);
            this.openEmojiSurface();
          });
      }
      if (this.toolbarToolShown(this.toolbarTools.phrase)) {
        this.toolbarButton(KeyboardIconPaths.TOOLBAR_PHRASE, SURFACE_PHRASES, KeyAccessibilityPolicy.phrase(),
          (): void => this.togglePhrases());
      }
      if (this.toolbarToolShown(this.toolbarTools.clipboard)) {
        this.toolbarButton(KeyboardIconPaths.TOOLBAR_CLIPBOARD, SURFACE_CLIPBOARD,
          KeyAccessibilityPolicy.clipboard(), (): void => this.toggleClipboard());
      }
      if (this.toolbarToolShown(this.toolbarTools.skin)) {
        this.toolbarButton(KeyboardIconPaths.TOOLBAR_SKIN, SURFACE_SKIN, KeyAccessibilityPolicy.skin(),
          (): void => this.toggle(SURFACE_SKIN));
      }
      if (this.toolbarToolShown(this.toolbarTools.scheme)) {
        this.toolbarButton(KeyboardIconPaths.TOOLBAR_SCHEME, SURFACE_SCHEME, KeyAccessibilityPolicy.scheme(),
          (): void => this.toggle(SURFACE_SCHEME));
      }
      Column() {
        Column() {
          KeyboardLineIcon({
            d: KeyboardIconPaths.COLLAPSE,
            iconSize: this.toolbarSized(KeyboardIconPaths.COLLAPSE_SIZE_VP),
            stroke: KeyboardIconPaths.CHEVRON_STROKE,
            color: this.skin.keyForeground
          })
        }
        .width(this.toolbarSized(TOOLBAR_BUTTON_VP))
        .height(this.toolbarSized(TOOLBAR_BUTTON_VP))
        .justifyContent(FlexAlign.Center)
        .borderRadius(this.toolbarSized(TOOLBAR_BUTTON_RADIUS_VP))
        .stateStyles({
          normal: {
            .scale({ x: 1, y: 1 })
          },
          pressed: {
            .scale({ x: TOOLBAR_PRESSED_SCALE, y: TOOLBAR_PRESSED_SCALE })
          }
        })
        .animation({ duration: TOOLBAR_PRESS_MS })
        .accessibilityText(KeyAccessibilityPolicy.collapse())
        .onClick((): void => {
          KeyboardSession.shared.dismiss();
        })
        .gesture(LongPressGesture().onAction((): void => this.openToolbarSettings()))
      }
      .layoutWeight(1)
      .height('100%')
      .justifyContent(FlexAlign.Center)
    }
    .width('100%')
    .height('100%')
    .padding({ left: TOOLBAR_SIDE_PADDING_VP, right: TOOLBAR_SIDE_PADDING_VP })
    .alignItems(VerticalAlign.Center)
  }

  /** 工具栏上的这个工具画不画：工具栏设成隐藏、又因为长空格样式一直画着这一行时，与 Android 一样只留标志、符号和收起（切到英文 26 键时 符 也不在，只剩标志和收起）。 */
  private toolbarToolShown(enabled: boolean): boolean {
    return enabled && !(this.toolbarTools.hidden && this.longSpaceToolbarRow());
  }

  /** 与 `toolbarButton` 同样的按钮，画一个字而不是线性图标（长空格样式的「符」）。 */
  @Builder
  toolbarTextButton(text: string, surface: string, label: string, action: () => void) {
    Column() {
      Column() {
        Text(text)
          .fontSize(this.toolbarSized(KeyboardMetrics.SMALL_KEY_FONT_SIZE + 2))
          .fontWeight(FUNCTION_LABEL_WEIGHT)
          .fontColor(this.surface === surface ? this.skin.toggleForeground : this.skin.secondary)
      }
      .width(this.toolbarSized(TOOLBAR_BUTTON_VP))
      .height(this.toolbarSized(TOOLBAR_BUTTON_VP))
      .justifyContent(FlexAlign.Center)
      .borderRadius(this.toolbarSized(TOOLBAR_BUTTON_RADIUS_VP))
      .backgroundColor(this.surface === surface ? this.skin.toggleBackground : TRANSPARENT)
      .stateStyles({
        normal: {
          .scale({ x: 1, y: 1 })
        },
        pressed: {
          .scale({ x: TOOLBAR_PRESSED_SCALE, y: TOOLBAR_PRESSED_SCALE })
        }
      })
      .animation({ duration: TOOLBAR_PRESS_MS })
      .accessibilityText(label)
      .accessibilitySelected(this.surface === surface)
      .onClick(action)
      .gesture(LongPressGesture().onAction((): void => this.openToolbarSettings()))
    }
    .layoutWeight(1)
    .height('100%')
    .justifyContent(FlexAlign.Center)
  }

  /** 工具栏的一个面板工具：40vp 按钮里的线框图标，平时为次要文字色（设计稿的 kbSub，即 Android 的 `KeyboardSkin.toolbarIcon()`；收起箭头仍用按键文字色），`surface` 为当前显示的界面时着色。标签不可省略，因为按钮只绘制图标。 */
  @Builder
  toolbarButton(path: string, surface: string, label: string, action: () => void) {
    Column() {
      Column() {
        KeyboardLineIcon({
          d: path,
          iconSize: this.toolbarSized(KeyboardIconPaths.TOOLBAR_SIZE_VP),
          stroke: KeyboardIconPaths.TOOLBAR_STROKE,
          color: this.surface === surface ? this.skin.toggleForeground : this.skin.secondary
        })
      }
      .width(this.toolbarSized(TOOLBAR_BUTTON_VP))
      .height(this.toolbarSized(TOOLBAR_BUTTON_VP))
      .justifyContent(FlexAlign.Center)
      .borderRadius(this.toolbarSized(TOOLBAR_BUTTON_RADIUS_VP))
      .backgroundColor(this.surface === surface ? this.skin.toggleBackground : TRANSPARENT)
      .stateStyles({
        normal: {
          .scale({ x: 1, y: 1 })
        },
        pressed: {
          .scale({ x: TOOLBAR_PRESSED_SCALE, y: TOOLBAR_PRESSED_SCALE })
        }
      })
      .animation({ duration: TOOLBAR_PRESS_MS })
      .accessibilityText(label)
      .accessibilitySelected(this.surface === surface)
      .onClick(action)
      // 长按被识别后这次触摸不再算点按，所以不会同时打开这个工具的面板。
      .gesture(LongPressGesture().onAction((): void => this.openToolbarSettings()))
    }
    .layoutWeight(1)
    .height('100%')
    .justifyContent(FlexAlign.Center)
  }

  /**
   * 正在输入时的工具条：预编辑行在候选胶囊通道上方，之后是一条细线和展开箭头，点它会用全部候选代替按键。
   *
   * 两行正好放进工具条自身的 50vp，所以注释不再需要单独一行，组合开始时下方也不会移动。手写结果和英文补全使用同一条通道；韩文或注音组合的「漢」或「選」按钮位于细线之前。
   */
  @Builder
  candidateBar() {
    Row() {
      Column({ space: this.stripSized(CANDIDATE_BAR_GAP_VP) }) {
        Row() {
          if (this.diagnostic.length > 0) {
            Text(this.diagnostic)
              .fontSize(this.stripSized(PREEDIT_FONT_SIZE))
              .fontColor(this.skin.accent)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
          } else if (this.visibleComposition().caret) {
            // 分段键移动的光标，与 2in1 组合行上的一样。
            Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).before)
              .fontSize(this.stripSized(PREEDIT_FONT_SIZE))
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
            Divider()
              .vertical(true)
              .strokeWidth(PREEDIT_CARET_WIDTH_VP)
              .height(PreeditCaretPolicy.height(this.stripSized(PREEDIT_FONT_SIZE)))
              .color(this.skin.accent)
            Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).after)
              .fontSize(this.stripSized(PREEDIT_FONT_SIZE))
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
          } else {
            // 拼写写在输入框里：这一行保留位置但不画拼写，候选不会因此上下移动；短语中已选定的部分和本地模式的触发符仍会显示。
            Text(this.visibleComposition().text)
              .fontSize(this.stripSized(PREEDIT_FONT_SIZE))
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .constraintSize({ maxWidth: this.readingRowSpellings() ? READING_WITH_SPELLINGS_MAX_WIDTH : '100%' })
          }
          if (this.readingRowSpellings()) {
            // 14 键的拼音选择条在读音右侧横向滚动，不盖住下面的候选，三端位置相同。内容与九键左列相同：先完整音节，再列下一键上能作声母的大写字母；点音节锁定，退格先撤销锁定。
            Scroll() {
              Row() {
                ForEach(this.spellings, (spelling: string, index: number) => {
                  Text(spelling)
                    .fontSize(this.stripSized(READING_SPELLING_FONT_SIZE))
                    .fontColor(this.skin.accent)
                    .fontFamily(this.candidateFontFamily())
                    .maxLines(1)
                    .height('100%')
                    .padding({ left: READING_SPELLING_PADDING_VP, right: READING_SPELLING_PADDING_VP })
                    .accessibilityText(KeyAccessibilityPolicy.spelling(spelling))
                    .onClick(() => KeyboardSession.shared.chooseSpelling(index))
                }, (spelling: string, index: number) => `${index}:${spelling}`)
              }
              .height('100%')
            }
            .scrollable(ScrollDirection.Horizontal)
            .scrollBar(BarState.Off)
            .align(Alignment.Start)
            .layoutWeight(1)
            .height('100%')
            .margin({ left: READING_SPELLING_GAP_VP })
          }
        }
        .width('100%')
        .height(this.stripSized(PREEDIT_LINE_VP))
        .padding({ left: PREEDIT_INDENT_VP })
        .clip(true)
        // canvas 在 50vp 的条里只留得出候选通道那么高，空着的读音行这时不占位（`None` 不参与布局）；有东西显示时照常占位，边框跟着再缩一些，见 `stripCanvasContentHeight`。
        .visibility(this.readingRowShown() ? Visibility.Visible : Visibility.None)

        if (this.showsHandwritingCandidates()) {
          // OCR 结果也是候选，通过识别器而不是 Engine 选择。
          this.chipLane(0)
        } else if (this.nineKeyQuickMarks.length > 0) {
          // 1 键放进来的常用标点；组字中也先于 Engine 的候选显示，直到下一次回报。
          this.chipLane(2)
        } else if (this.candidates.length > 0) {
          Scroll() {
            Row({ space: CandidateLayoutPolicy.gap(this.settings.candidateSkinLayout, false, this.chipFontSize()) }) {
              ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
                this.candidateChip(candidate, index)
              }, (candidate: CandidateEntry, index: number) =>
                `${index}:${JSON.stringify(candidate)}`)
            }
            .height(this.stripSized(CHIP_HEIGHT_VP))
          }
          .scrollable(ScrollDirection.Horizontal)
          .scrollBar(BarState.Off)
          .width('100%')
          .height(this.stripSized(CHIP_HEIGHT_VP))
          .align(Alignment.Start)
        } else if (this.emailSuffixes.length > 0) {
          // 光标前打到 `xxx@`：后缀先于英文补全，`@` 之后接着打的域名也会被英文补全当成单词。
          this.chipLane(3)
        } else if (this.englishSuggestions.length > 0) {
          // 直接英文输入没有组合，所以通道上只有补全。
          this.chipLane(1)
        } else {
          Column()
            .width('100%')
            .height(this.stripSized(CHIP_HEIGHT_VP))
        }
      }
      .layoutWeight(1)
      .height('100%')
      .justifyContent(FlexAlign.Center)

      if (this.showsHanjaButton()) {
        this.hanjaButton()
      }
      if (this.candidates.length > 0 && !this.showsHandwritingCandidates()) {
        // 上屏后联想按用户选的样式在展开键左边放关闭键（`next_word_prediction_buttons`）；两个键并排时分隔线只画在 × 左边，与 Android 相同。
        if (this.predictionCloseShown()) {
          Divider()
            .vertical(true)
            .strokeWidth(1)
            .height(this.stripSized(BAR_DIVIDER_HEIGHT_VP))
            .color(this.skin.hair)
          this.closePredictionButton()
        }
        if (this.expandChevronAllowed()) {
          if (!this.predictionCloseShown()) {
            Divider()
              .vertical(true)
              .strokeWidth(1)
              .height(this.stripSized(BAR_DIVIDER_HEIGHT_VP))
              .color(this.skin.hair)
          }
          this.expandChevron()
        }
      }
    }
    .width('100%')
    .height('100%')
    .alignItems(VerticalAlign.Center)
  }

  /** 胶囊绘制的词：字号不超过 34vp 胶囊能容纳的大小，分下方有无注释行两种情况。 */
  private chipFontSize(): number {
    return Math.min(this.settings.candidateFontSize,
      KeyboardSession.shared.glossesExpected() ? this.stripSized(CHIP_WORD_MAX_WITH_GLOSS) : this.stripSized(CHIP_WORD_MAX_ALONE));
  }

  /** 通道的可见宽度：工具条宽度减去细线和展开箭头（联想时还有关闭键）。 */
  private chipLaneWidth(): number {
    const buttons: number = (this.expandChevronAllowed() ? 1 : 0) + (this.predictionCloseShown() ? 1 : 0);
    return Math.max(0, this.availableWidth - buttons * (1 + BAR_BUTTON_VP));
  }

  /** 候选栏右端放不放关闭键：只在上屏后联想时，并且样式带 ×。 */
  private predictionCloseShown(): boolean {
    return NextWordPredictionPolicy.showsClose(KeyboardSession.shared.predictionButtons(), this.prediction);
  }

  /** 候选栏右端放不放展开键：联想时「只有关闭」不放，其余照旧。 */
  private expandChevronAllowed(): boolean {
    return NextWordPredictionPolicy.allowsExpand(KeyboardSession.shared.predictionButtons(), this.prediction);
  }

  /** 关闭上屏后联想：展开面板开着时先收起，再送 Cancel；引擎清掉建议、不往输入框写字，候选清空后工具条换回工具栏。 */
  private closePrediction(): void {
    if (this.surface === SURFACE_EXPANDED) {
      this.show(SURFACE_NONE);
    }
    KeyboardSession.shared.dismissPrediction();
  }

  /** 一个注释列的文字宽度（`CandidateChipWidth`：可见通道放三列），不预期出现注释时为 0。 */
  private chipGlossColumn(): number {
    return KeyboardSession.shared.glossesExpected()
      ? CandidateChipWidth.column(this.chipLaneWidth(), 0, CHIP_PADDING_VP) : 0;
  }

  /** 胶囊的最小宽度：设计稿的 30vp；预期有注释时为整个注释列宽，这样注释到来时不会撑宽胶囊、把后面的胶囊挤开。永远没有注释的来源（emoji、符号等，见 `CandidateChipWidth.reservesColumn`）不留这一列。皮肤的 `itemMinWidth` 只给 2in1 候选窗，触屏不用（`CandidateChipWidth.touchMinimum`）。 */
  private chipMinWidth(candidate: CandidateEntry): number {
    const column: number = CandidateChipWidth.reservesColumn(KeyboardSession.shared.glossesExpected(),
      candidate.source, candidate.hunEum) ? this.chipGlossColumn() : 0;
    return CandidateChipWidth.touchMinimum(column, CHIP_PADDING_VP);
  }

  /** 注释最多能画多宽：它的列宽，或上方的词更宽时取词宽。 */
  private chipGlossMaxWidth(candidate: CandidateEntry): number {
    return CandidateChipWidth.content(TextPolicy.characters(candidate.text).length * this.chipFontSize(), true,
      this.chipGlossColumn());
  }

  /** 胶囊在词旁绘制的内容：Engine 的注解，或在没有注释行承载时的翻译。 */
  private chipAnnotationBeside(candidate: CandidateEntry): boolean {
    return candidate.annotation.length > 0
      && (!candidate.annotationIsTranslation || KeyboardSession.shared.translationRows() === 0);
  }

  /** 一个候选：词在上、注释在下，底色为按键色；为高亮项时用强调色、字重 600。 */
  @Builder
  candidateChip(candidate: CandidateEntry, index: number) {
    Column() {
      Row() {
        Text(candidate.text)
          .fontSize(this.chipFontSize())
          .lineHeight(this.chipFontSize() * CHIP_WORD_LINE_HEIGHT)
          .fontWeight(this.candidateWeight(candidate))
          .fontColor(this.candidateWordColor(candidate))
          .fontFamily(this.candidateFontFamily())
          .maxLines(1)
        if (candidate.badge.length > 0) {
          Text(candidate.badge)
            .fontSize(this.stripSized(CHIP_GLOSS_FONT_SIZE))
            .fontColor(this.skin.accent)
            .opacity(0.8)
            .padding({ left: 2 })
            .maxLines(1)
        }
        if (candidate.hint.length > 0) {
          Text(candidate.hint)
            .fontSize(this.stripSized(CHIP_GLOSS_FONT_SIZE))
            .fontColor(this.candidateDetailTint(candidate, this.skin.secondary))
            .padding({ left: 2 })
            .maxLines(1)
        }
        if (this.chipAnnotationBeside(candidate)) {
          Text(candidate.annotation)
            .fontSize(this.stripSized(CHIP_GLOSS_FONT_SIZE))
            .fontColor(this.candidateDetailTint(candidate, this.skin.secondary))
            .padding({ left: 3 })
            .maxLines(1)
        }
      }
      .alignItems(VerticalAlign.Center)
      if (KeyboardSession.shared.translationRows() > 0) {
        // 无论该候选有无注释都预留位置，这样通道上的词保持齐平。
        Text(this.candidateGlossLine(candidate).length > 0 ? this.candidateGlossLine(candidate) : ' ')
          .fontSize(this.stripSized(CHIP_GLOSS_FONT_SIZE))
          .lineHeight(this.stripSized(CHIP_GLOSS_LINE_VP))
          .fontColor(this.candidateDetailTint(candidate, this.skin.secondary))
          .fontFamily(this.candidateFontFamily())
          .opacity(this.candidateGlossLine(candidate).length > 0 ? 1 : 0)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .constraintSize({ maxWidth: this.chipGlossMaxWidth(candidate) })
      }
      // 英文音标在释义下面单独一行，同样按设置预留，音标没到或这个词查不到时留空。
      if (KeyboardSession.shared.pronunciationRows() > 0) {
        Text(candidate.pronunciation.length > 0 ? candidate.pronunciation : ' ')
          .fontSize(this.stripSized(CHIP_GLOSS_FONT_SIZE))
          .lineHeight(this.stripSized(CHIP_GLOSS_LINE_VP))
          .fontColor(this.candidateDetailTint(candidate, this.skin.secondary))
          .opacity(candidate.pronunciation.length > 0 ? 1 : 0)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .constraintSize({ maxWidth: this.chipGlossMaxWidth(candidate) })
      }
    }
    .constraintSize({ minWidth: this.chipMinWidth(candidate) })
    .height(this.chipHeight())
    .padding({ left: CHIP_PADDING_VP, right: CandidateLayoutPolicy.dip(
      this.settings.candidateSkinLayout?.horizontal.itemPaddingRight, this.chipFontSize(), CHIP_PADDING_VP) })
    .justifyContent(FlexAlign.Center)
    .borderRadius(this.settings.candidateSkinLayout?.itemRadius ?? CHIP_RADIUS_VP)
    // canvas 下文字用候选包的配色，高亮也要用包的 `selected`；按键色配包的文字色会对不上，深色键盘配浅色模式的包时首选几乎看不见。
    .backgroundColor(candidate.highlighted
      ? this.settings.candidateCanvas === null ? this.skin.keyBackground : this.candidateColors().selected : TRANSPARENT)
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(CHIP_PRESSED_OPACITY)
      }
    })
    .accessibilityText(this.candidateLabel(candidate, index))
    .onClick(() => KeyboardSession.shared.choose(index))
    .gesture(LongPressGesture().onAction(() => this.openTouchCandidateMenu(index)))
  }

  /**
   * 用于非 Engine 候选的普通胶囊通道：手写结果（`kind` 0），通过识别器选择；英文补全（`kind` 1），是对正在输入的单词的只读建议；九键 1 键的常用标点（`kind` 2），点选后按标点键的路线输入；邮箱后缀（`kind` 3），点选后补全光标前的邮箱。列表在这里读取而不是传入，这样通道随列表变化而更新。
   */
  @Builder
  chipLane(kind: number) {
    Scroll() {
      Row() {
        ForEach(kind === 0 ? this.handwritingCandidates : kind === 1 ? this.englishSuggestions
          : kind === 3 ? this.emailSuffixes
          : this.nineKeyQuickMarks.map((mark: PunctuationEntry): string => mark.face), (item: string, index: number) => {
          Text(item)
            .fontSize(this.chipFontSize())
            .fontFamily(this.candidateFontFamily())
            .fontColor(this.skin.keyForeground)
            .maxLines(1)
            .textAlign(TextAlign.Center)
            .constraintSize({ minWidth: CHIP_MIN_WIDTH_VP })
            .height(this.stripSized(CHIP_HEIGHT_VP))
            .padding({ left: CHIP_PADDING_VP, right: CHIP_PADDING_VP })
            .borderRadius(CHIP_RADIUS_VP)
            .stateStyles({
              normal: {
                .opacity(1)
              },
              pressed: {
                .opacity(CHIP_PRESSED_OPACITY)
              }
            })
            .accessibilityText(kind === 0 ? `手写候选：${item}`
              : kind === 1 ? KeyAccessibilityPolicy.englishSuggestion(index + 1, item)
              : kind === 3 ? KeyAccessibilityPolicy.emailSuffix(index + 1, item)
              : KeyAccessibilityPolicy.symbol(item))
            .onClick(() => {
              if (kind === 0) {
                this.chooseHandwriting(item);
              } else if (kind === 1) {
                KeyboardSession.shared.useEnglishSuggestion(index);
              } else if (kind === 3) {
                KeyboardSession.shared.useEmailSuffix(index);
              } else {
                this.chooseNineKeyQuickMark(index);
              }
            })
        }, (item: string, index: number) => `${kind}:${index}:${item}`)
      }
      .height(this.stripSized(CHIP_HEIGHT_VP))
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
    .height(this.stripSized(CHIP_HEIGHT_VP))
    .align(Alignment.Start)
  }

  /** 上屏后联想的关闭键 ×，与展开键同宽同高。 */
  @Builder
  closePredictionButton() {
    Column() {
      KeyboardLineIcon({
        d: KeyboardIconPaths.CLOSE,
        iconSize: this.toolbarSized(KeyboardIconPaths.CLOSE_SIZE_VP),
        stroke: KeyboardIconPaths.CHEVRON_STROKE,
        color: this.skin.keyForeground
      })
    }
    .width(BAR_BUTTON_VP)
    .height(this.toolbarSized(BAR_BUTTON_VP))
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .flexShrink(0)
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(BAR_BUTTON_PRESSED_OPACITY)
      }
    })
    .accessibilityText(KeyAccessibilityPolicy.closePrediction())
    .onClick(() => this.closePrediction())
  }

  /** 候选栏的展开箭头，网格打开期间翻转显示。 */
  @Builder
  expandChevron() {
    Column() {
      Column() {
        KeyboardLineIcon({
          d: KeyboardIconPaths.EXPAND,
          iconSize: this.toolbarSized(KeyboardIconPaths.EXPAND_SIZE_VP),
          stroke: KeyboardIconPaths.CHEVRON_STROKE,
          color: this.skin.keyForeground
        })
      }
      .rotate({ angle: this.surface === SURFACE_EXPANDED ? 180 : 0 })
    }
    .width(BAR_BUTTON_VP)
    .height(this.toolbarSized(BAR_BUTTON_VP))
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .flexShrink(0)
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(BAR_BUTTON_PRESSED_OPACITY)
      }
    })
    .accessibilityText(KeyAccessibilityPolicy.expandCandidates(this.surface === SURFACE_EXPANDED))
    .onClick(() => this.toggleExpanded())
  }

  @Builder
  nineKeyFace() {
    if (this.tallReturnFrame()) {
      this.tallReturnNineKeyFrame()
    } else {
      Column({ space: this.rowSpacing() }) {
        if (this.touchDesign()) {
          this.touchNineKeyGrid()
        } else {
          this.nineKeyGrid()
        }
        this.actionRow()
      }
      .width('100%')
      .height(this.faceHeight())
    }
  }

  /**
   * 九键样式「右侧大回车」的外框（`tallReturnFrame`），拼音九键、九键双拼、笔画键盘和它们的数字层共用，三列与普通网格同宽（`NineKeyLayout.SIDE_SHARE`、`GRID_SHARE`）：
   *
   * - 左列：跨网格三行的标点栏（拼音九键是 ，。？！、组字时的读音、数字层的四则运算和 #+=；笔画是标点栏），栏下方底行放与它同宽的 符，数字层也在这里。
   * - 中间：3×3 网格（笔画是笔画键），格下方是恰好与网格同宽的底行 `nineKeyActionRow`：123 | 空格 | 中/英，数字层 切换键 | 0 | 空格 | 中/英（「中/英键放在空格左边」时是 123 | 中/英 | 空格、切换键 | 中/英 | 0 | 空格）。123、中/英 和数字层的空格与标准九键底行里同宽，空格（数字层是 0）占剩下的部分；外框量到自己的宽度（`tallReturnFaceWidth`）后按 `NineKeyLayout.tallReturnRow` 定宽，之前按 `NineKeyLayout.actionRow` 的份额排。
   * - 右列：与中间网格逐行对齐（`tallReturnGridRowHeight`），回车跨网格最后一行和底行、两倍高中间无缝。拼音九键 ⌫ 在第一行，第二行是拼音分词（数字层小数点），回车跨第三行和底行；原来第三行的 ！ 接到左列末尾。笔画网格只有两行（`StrokeLayout.ROWS`），⌫ 占第一行，回车跨第二行和底行。
   *
   * 中/英 与回车不在同一行，回车不让出键帽，两键都是普通的触摸区。
   */
  @Builder
  tallReturnNineKeyFrame() {
    Row({ space: this.keySpacing() }) {
      Column({ space: this.rowSpacing() }) {
        if (this.strokeFace() && !this.symbols) {
          Column() {
            this.strokeRailMarks()
          }
          .width('100%')
          .height(this.gridHeight())
          .backgroundColor(this.skin.keySurfaceBackground(false, true))
          .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
        } else {
          Column({ space: this.rowSpacing() }) {
            this.touchNineKeySidebar()
          }
          .width('100%')
          .height(this.gridHeight())
        }
        Column() {
          this.nineKeySymbolPanelKey('100%', 0)
        }
        .width('100%')
        .height(this.rowHeight(3))
      }
      .layoutWeight(NineKeyLayout.SIDE_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        Column({ space: this.rowSpacing() }) {
          if (this.strokeFace() && !this.symbols) {
            this.strokeCells()
          } else {
            this.touchNineKeyCells()
          }
        }
        .width('100%')
        .height(this.gridHeight())
        this.nineKeyActionRow()
      }
      .layoutWeight(NineKeyLayout.GRID_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        if (this.strokeFace() && !this.symbols) {
          Column() {
            this.deleteKey('100%', true, 0)
          }
          .width('100%')
          .height(this.tallReturnGridRowHeight())
        } else {
          Column() {
            this.deleteKey('100%', true, 0)
          }
          .width('100%')
          .height(this.tallReturnGridRowHeight())
          Column() {
            this.nineKeySeparatorKey()
          }
          .width('100%')
          .height(this.tallReturnGridRowHeight())
        }
        // 回车占右列剩下的高度：网格最后一行、一道行距和底行。
        this.tallReturnKey()
      }
      .layoutWeight(NineKeyLayout.SIDE_SHARE)
      .height('100%')
    }
    .width('100%')
    .height(this.faceHeight())
    .onAreaChange((_old: Area, now: Area): void => {
      this.tallReturnFaceWidth = Number(now.width);
    })
    // 外框不在时宽度可能变了（比如在 26 键上转了屏再切回九键），留着旧宽度会让新外框第一帧按旧宽度定出比网格还宽的固定键；清零后第一帧按份额排，量到新宽度再定宽。
    .onDisAppear((): void => {
      this.tallReturnFaceWidth = 0;
    })
  }

  /** 「右侧大回车」外框里网格一行的高度：按这块网格自己的行数（`tallReturnGridRows`）平分网格高度减去行距（`NineKeyLayout.tallReturnGridRowHeight`），右列的键与网格逐行对齐。 */
  private tallReturnGridRowHeight(): number {
    return NineKeyLayout.tallReturnGridRowHeight(this.gridHeight(), this.rowSpacing(), this.tallReturnGridRows());
  }

  /** 「右侧大回车」中间网格的行数，与 `tallReturnNineKeyFrame` 画的格子一致：笔画字母层是 `StrokeLayout.ROWS` 的两行，拼音九键、九键双拼和数字层是三行。 */
  private tallReturnGridRows(): number {
    if (this.strokeFace() && !this.symbols) {
      return StrokeLayout.ROWS.length;
    }
    return (this.symbols ? NineKeyLayout.digits() : NineKeyLayout.touchRows()).length;
  }

  /** 「右侧大回车」的回车：与设计底行的回车同一键面和动作，宽度是右列的宽度，高度占右列剩下的部分（`layoutWeight(1)`，在列里是竖向的）。左边没有紧挨的中/英，不让出键帽。 */
  @Builder
  tallReturnKey() {
    this.fixedKey({
      face: this.returnFace(),
      label: KeyAccessibilityPolicy.returnKey(this.returnFace()),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: this.returnComposing() ? COMPOSING_RETURN_WEIGHT : FUNCTION_LABEL_WEIGHT,
      emphasized: true,
      width: '100%',
      share: 1,
      icon: this.returnUsesIcon() ? KeyboardIconPaths.RETURN : '',
      action: () => this.activateReturn()
    });
  }

  /** 九键外框的 符：打开符号面板，按下记 `SoftSymbol`。普通底行里按份额定宽（`width` 为 0），「右侧大回车」里铺满左列（`share` 为 0）。 */
  @Builder
  nineKeySymbolPanelKey(width: Length, share: number) {
    this.fixedKey({
      face: '符',
      label: KeyAccessibilityPolicy.symbolPanel(),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      special: true,
      width: width,
      share: share,
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
        this.openSymbolSurface();
      }
    });
  }

  /**
   * 触屏网格左列：数字层是四则运算和 #+=，组字时是这串数字可能代表的读音，其余时候是标点（「右侧大回车」时末尾多一个 !，`NineKeyLayout.sideMarks`）。普通网格和「右侧大回车」外框共用。
   *
   * 用户自定义了左列（共享偏好 `touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols`，#10223）时字母层换成可滚动的等高栈（`customLetterSidebar`），数字层换成滚动区加最下面的 #+=（`customDigitSidebar`）；半角语境的出厂数字左列在四则运算后面多出 @ : ; ( ) ' " $（`NineKeyLayout.digitSidebar`），一屏放不下，也画成这个滚动区（`NineKeyLayout.digitSidebarScrolls`）；其余空表时照原来的等分画法，画面不变。长按任一项打开设置。
   */
  @Builder
  touchNineKeySidebar() {
    if (this.symbols && NineKeyLayout.digitSidebarScrolls(this.digitSidebarTable(), this.numberPadPunctuation())) {
      this.customDigitSidebar()
    } else if (this.symbols) {
      ForEach(NineKeyLayout.digitSidebar(), (symbol: string) => {
        Column() {
          this.fixedKey(this.numberPadMarkOptions(symbol));
        }
        .layoutWeight(1)
        .width('100%')
        .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(true)))
      }, (symbol: string) => symbol + this.faceKey())
      Column() {
        this.fixedKey(this.moreSymbolsOptions());
      }
      .layoutWeight(1)
      .width('100%')
    } else if (this.spellings.length > 0) {
      Scroll() {
        Column() {
          ForEach(this.spellings, (spelling: string, index: number) => {
            Text(spelling)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.skin.accent)
              .textAlign(TextAlign.Center)
              .width('100%')
              .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
              .accessibilityText(KeyAccessibilityPolicy.spelling(spelling))
              .onClick(() => KeyboardSession.shared.chooseSpelling(index))
          })
        }
        .width('100%')
      }
      .scrollBar(BarState.Off)
      .width('100%')
      .height('100%')
      .backgroundColor(this.skin.keySurfaceBackground(false, true))
      .borderRadius(this.skin.keyCornerRadius())
    } else if (NineKeyLayout.customSidebar(this.letterSidebarTable())) {
      this.customLetterSidebar()
    } else {
      ForEach(NineKeyLayout.sideMarks(this.tallReturnFrame()), (symbol: string) => {
        Column() {
          this.fixedKey(this.nineKeyMarkOptions(symbol));
        }
        .layoutWeight(1)
        .width('100%')
        .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(false)))
      }, (symbol: string) => symbol + this.faceKey())
    }
  }

  /** 字母层左列的自定义表：等高的键上下滚动，一屏最多 `LETTER_SIDEBAR_VISIBLE_ROWS` 项、每项不矮于 `SIDEBAR_MIN_ROW_VP`（`NineKeyLayout.sidebarItems`），项与项之间的行距与出厂表各格相同；右侧大回车不再接 !。 */
  @Builder
  customLetterSidebar() {
    Scroll() {
      Column({ space: this.rowSpacing() }) {
        ForEach(this.letterSidebarItems(), (item: NineKeySidebarItem) => {
          Column() {
            this.fixedKey(this.customSidebarOptions(item.symbol, false));
          }
          .width('100%')
          .height(item.height)
          .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(false)))
        }, (item: NineKeySidebarItem) => item.key)
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .width('100%')
    .height('100%')
  }

  /** 数字层左列的滚动画法：上面四格高的滚动区放用户的表，或半角语境的出厂项加半角符号（一屏最多 `DIGIT_SIDEBAR_VISIBLE_ROWS` 项），最下面一格仍是 #+=，与出厂五格等高时同一位置、同一高度（`NineKeyLayout.digitSidebarScrollHeight`、`digitSidebarCellHeight`）。每一项怎么输入见 `digitSidebarOptions`。 */
  @Builder
  customDigitSidebar() {
    Scroll() {
      Column({ space: this.rowSpacing() }) {
        ForEach(this.digitSidebarItems(), (item: NineKeySidebarItem) => {
          Column() {
            this.fixedKey(this.digitSidebarOptions(item.symbol));
          }
          .width('100%')
          .height(item.height)
          .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(true)))
        }, (item: NineKeySidebarItem) => item.key)
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .width('100%')
    .height(NineKeyLayout.digitSidebarScrollHeight(this.gridHeight(), this.rowSpacing()))
    Column() {
      this.fixedKey(this.moreSymbolsOptions());
    }
    .width('100%')
    .height(NineKeyLayout.digitSidebarCellHeight(this.gridHeight(), this.rowSpacing()))
  }

  /** 触屏网格中间的 3×3 格：字母层是 @# 和字母组，数字层是 1–9。普通网格和「右侧大回车」外框共用。 */
  @Builder
  touchNineKeyCells() {
    ForEach(this.symbols ? NineKeyLayout.digits() : NineKeyLayout.touchRows(), (row: NineKey[]) => {
      Row({ space: this.keySpacing() }) {
        ForEach(row, (key: NineKey) => {
          if (!this.symbols && !this.nineKeyShuangpinFaceActive()
            && NineKeyLayout.holdOptions(key).length > 0) {
            this.nineKeyHoldKey(key);
          } else {
            this.flexibleKey(this.nineKeyKeyOptions(key));
          }
        }, (key: NineKey) => key.input + this.faceKey())
      }
      .layoutWeight(1)
      .width('100%')
    }, (row: NineKey[]) => row[0].input + this.faceKey())
  }

  /** 触屏网格右列的第二格：字母层是拼音分词，数字层是字面小数点。普通网格和「右侧大回车」外框共用；键面直接写在这里而不是由方法返回，切层时键面才会跟着变（见 `KeyOptions`）。 */
  @Builder
  nineKeySeparatorKey() {
    this.fixedKey({
      face: this.symbols ? '.' : this.nineKeyFaceOf(NineKeyLayout.separator()),
      label: this.symbols ? '小数点' : this.nineKeyLabel(NineKeyLayout.separator()),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      special: true,
      width: '100%',
      action: () => {
        if (this.symbols) {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.character('.'));
          this.typeNumberPadText('.');
          return;
        }
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(NineKeyLayout.separator().input));
        KeyboardSession.shared.press(NineKeyLayout.separator().input.charCodeAt(0), false);
      }
    });
  }

  /**
   * 触屏键盘的九键网格，与设计稿和 Android（`ImeLayoutRows.rebuildNineKeyRows`）一致：一列 ，。？ 键，首格是 @# 的 3×3 网格，以及一列删除键、音节分隔符和 ！。@# 把常用标点放进候选条（`showNineKeyQuickMarks`），符号面板从底行最左边的 符 打开。数字层重新标注网格并保留两侧的列。
   *
   * 输入数字串时，左列改为列出这些数字可能代表的读音而不是标点，鸿蒙九键一直如此；设计稿里没有放它们的位置。
   */
  @Builder
  touchNineKeyGrid() {
    Row({ space: this.keySpacing() }) {
      Column({ space: this.rowSpacing() }) {
        this.touchNineKeySidebar()
      }
      .layoutWeight(NineKeyLayout.SIDE_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        this.touchNineKeyCells()
      }
      .layoutWeight(NineKeyLayout.GRID_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        Column() {
          this.deleteKey('100%', true, 0)
        }
        .layoutWeight(1)
        .width('100%')
        Column() {
          // 数字页的中间键是字面小数点；字母页保留拼音分词。
          this.nineKeySeparatorKey()
        }
        .layoutWeight(1)
        .width('100%')
        Column() {
          // 数字层选了「空格在右列」（`touch_number_pad_space`）时这一格是带语音和拖动光标手势的空格，0 占底行（`NineKeyLayout.actionRow`）；其余时候是 ！。
          if (this.symbols && this.numberPadSpace === TouchLayoutStylePolicy.RIGHT_COLUMN) {
            this.spaceKey(true, 0, '100%')
          } else {
            this.fixedKey(this.nineKeyMarkOptions(NineKeyLayout.closingMark()));
          }
        }
        .layoutWeight(1)
        .width('100%')
      }
      .layoutWeight(NineKeyLayout.SIDE_SHARE)
      .height('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }

  /** 触屏网格两侧的一个标点键：功能色的键，通过 Engine 的标点处理输入该标点，所以输出符合当前组合语言和标点开关，与它所取代的侧栏一致。 */
  private nineKeyMarkOptions(symbol: string): KeyOptions {
    return {
      face: this.symbolFace(symbol),
      label: KeyAccessibilityPolicy.symbol(this.symbolFace(symbol)),
      size: NINE_KEY_FONT_SIZE,
      emphasized: false,
      special: true,
      width: '100%',
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
        KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
      }
    };
  }

  /** 数字层左列最后一格的 #+=：打开扩展符号页。出厂表和自定义表的左列共用。 */
  private moreSymbolsOptions(): KeyOptions {
    return {
      face: '#+=',
      label: '更多符号',
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      special: true,
      width: '100%',
      action: () => this.toggleMoreSymbols()
    };
  }

  /** 中文九键一族字母层左列（拼音九键、九键双拼、笔画）读的表：英文九键不读，照出厂。 */
  private letterSidebarTable(): string[] {
    return this.english ? [] : this.nineKeySidebarSymbols;
  }

  /** 九宫格数字键盘左列读的表：九键一族的数字层和其余键面借来的数字九宫格都读，表里的标点按当前语言换写法（`NineKeyLayout.numberPadSymbols`）；英文下的数字九宫格（英文九键和英文 26 键借来的）不读，照出厂的 ASCII 运算符加半角符号（`NineKeyLayout.digitSidebar`）。读这张表的半角语境（本地模式、韩文、越南语、藏文、关掉中文标点的中文）自定义过就照用户的表，不接半角符号。 */
  private digitSidebarTable(): string[] {
    return this.english ? [] : NineKeyLayout.numberPadSymbols(this.nineKeyDigitSidebarSymbols,
      this.numberPadPunctuation());
  }

  /** 数字九宫格上的标点此刻按哪种写法出现（`NumberPadPunctuation`）：本地模式和英文是半角；日语方案（罗马字和假名网格）是日语写法；其余跟着中文标点的键面写法（`chinesePunctuationFaces`），韩文、越南语、藏文和关掉中文标点时是半角。 */
  private numberPadPunctuation(): NumberPadPunctuation {
    if (this.localMode !== 'none' || this.english) {
      return NumberPadPunctuation.ASCII;
    }
    if (this.scheme.engineScheme === 'japanese') {
      return NumberPadPunctuation.JAPANESE;
    }
    return this.chinesePunctuationFaces() ? NumberPadPunctuation.CHINESE : NumberPadPunctuation.ASCII;
  }

  /** 字母层自定义左列逐项的高度和 ForEach 键：左列与网格同高（`gridHeight`）。 */
  private letterSidebarItems(): NineKeySidebarItem[] {
    return NineKeyLayout.sidebarItems(this.letterSidebarTable(), this.gridHeight(),
      NineKeyLayout.LETTER_SIDEBAR_VISIBLE_ROWS, this.rowSpacing(), this.faceKey());
  }

  /** 数字层滚动左列逐项的高度和 ForEach 键：表是 `NineKeyLayout.digitSidebar`（用户的表原样；空表在半角语境是出厂运算符加半角符号），滚动区是 #+= 上面那四格加三道行距。 */
  private digitSidebarItems(): NineKeySidebarItem[] {
    return NineKeyLayout.sidebarItems(
      NineKeyLayout.digitSidebar(this.digitSidebarTable(), this.numberPadPunctuation()),
      NineKeyLayout.digitSidebarScrollHeight(this.gridHeight(), this.rowSpacing()),
      NineKeyLayout.DIGIT_SIDEBAR_VISIBLE_ROWS, this.rowSpacing(), this.faceKey());
  }

  /** 笔画标点栏的自定义表：格与格之间没有空隙。 */
  private strokeSidebarItems(): NineKeySidebarItem[] {
    return NineKeyLayout.sidebarItems(this.letterSidebarTable(), this.gridHeight(),
      NineKeyLayout.LETTER_SIDEBAR_VISIBLE_ROWS, 0, this.faceKey());
  }

  /** 自定义左列的一项：功能色的键，点按记 `SoftPunctuation`（与出厂表同一个统计格子），先结束组字再按字面上屏（`KeyboardSession.insertSidebarSymbol`）；`digits` 是数字层，组字与出厂运算符一样原样上屏。 */
  private customSidebarOptions(symbol: string, digits: boolean): KeyOptions {
    return {
      face: symbol,
      label: KeyAccessibilityPolicy.symbol(symbol),
      size: NINE_KEY_FONT_SIZE,
      emphasized: false,
      special: true,
      width: '100%',
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
        KeyboardSession.shared.insertSidebarSymbol(symbol, digits);
      }
    };
  }

  /** 长按九键左侧符号栏任一项（#10223）：给一下按键反馈，再打开设置应用「屏幕键盘」页的「按键布局」，直接进入这一张表的编辑页（`digits` 为真时是数字表，否则是字母表），与 Android 带 `edit_sidebar_key` 打开「按键布局」相同。子目标是那张表的共享偏好名（`SettingsPageLink.NINE_KEY_SIDEBAR_TARGET` / `NINE_KEY_DIGIT_SIDEBAR_TARGET`），设置页认不得时停在「屏幕键盘」页；英文九键不读这两张表，长按不做事。 */
  private openSidebarSettings(digits: boolean): void {
    if (this.english) {
      return;
    }
    KeyboardSession.shared.holdFeedback();
    KeyboardSession.shared.openSettings('screen-keyboard',
      digits ? SettingsPageLink.NINE_KEY_DIGIT_SIDEBAR_TARGET : SettingsPageLink.NINE_KEY_SIDEBAR_TARGET);
  }

  /**
   * 数字层滚动左列的一项：用户的表按自定义项上屏（`customSidebarOptions`，先结束组字再按字面上屏，藏文里也是）；出厂项和半角符号与出厂运算符同一条路（`numberPadMarkOptions` → `typeNumberPadText`），按 `NineKeyLayout.numberPadToSession` 交给会话或按字面上屏。藏文的 `'` 因此与原来藏文 123 层一样交给会话（achung，如 'od），不先上屏组合，也不按字面上屏。
   */
  private digitSidebarOptions(symbol: string): KeyOptions {
    return NineKeyLayout.customSidebar(this.digitSidebarTable())
      ? this.customSidebarOptions(symbol, true) : this.numberPadMarkOptions(symbol);
  }

  private numberPadMarkOptions(symbol: string): KeyOptions {
    return {
      face: symbol,
      label: KeyAccessibilityPolicy.symbol(symbol),
      size: NINE_KEY_FONT_SIZE,
      emphasized: false,
      special: true,
      width: '100%',
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
        this.typeNumberPadText(symbol);
      }
    };
  }

  /** 保留显式全角设置，跳过中文标点映射和候选数字选择。 */
  private insertNumberPadLiteral(text: string): void {
    KeyboardSession.shared.commitRaw();
    KeyboardSession.shared.insert(text);
  }

  /** 数字九宫格上的数字、0、小数点、左列出厂项和半角符号、顶部补充行怎么输入（键位已由调用方记过），规则与 Android、iOS 相同（`NineKeyLayout.numberPadToSession`）：本地模式、越南语和藏文里引擎此刻列为拼写符号的字符，以及藏文的 `'`，交给会话（`press`）；其余一律按字面上屏（`insertNumberPadLiteral`），日语、韩文和大千注音的数字和标点也是。 */
  private typeNumberPadText(text: string): void {
    if (this.typeHelpcodeTrigger(text)) {
      return;
    }
    if (NineKeyLayout.numberPadToSession(text, this.localMode !== 'none', this.vietnameseFace(), this.tibetanFace(),
      KeyboardSession.shared.hardwareSpelling())) {
      KeyboardSession.shared.press(text.charCodeAt(0), false);
      return;
    }
    this.insertNumberPadLiteral(text);
  }

  /** 到这里结束的键面是否用触屏九键外框的底行（`nineKeyActionRow`）：拼音九键网格、数字九宫格，或不在符号层的笔画键盘。假名网格的字母层也是九键布局，但保留共享底行；它借来的数字九宫格用九键外框的底行。 */
  private touchNineKeyRow(): boolean {
    return this.touchDesign() && !this.kanaGridShowing() && this.gridActionRow();
  }

  /**
   * 0 在数字层上与其他九个数字一样直接输入。字母层上（2in1 网格的控制列）它不参与拼写：九键的组码只有 2~9，Engine 不接 0，交给它就是按了没反应。所以和 Android 的 `commitNineKeyLiteral` 一样，正在打的拼音先按原样上屏，再输入 0。
   */
  private typeNineKeyZero(): void {
    if (this.symbols) {
      this.typeNineKeyDigit('0');
      return;
    }
    if (this.nineKeyShuangpinFaceActive()) {
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey('0'));
      KeyboardSession.shared.press('0'.charCodeAt(0), false);
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey('0'));
    KeyboardSession.shared.commitRaw();
    KeyboardSession.shared.insert('0');
  }

  /** 数字九宫格上的一个数字（`typeNumberPadText`）；借来的数字九宫格记普通数字键，自己的九键网格（`ownsNineKeyGrid`）记 Nine。 */
  private typeNineKeyDigit(digit: string): void {
    KeyboardSession.shared.softKeyPress(this.ownsNineKeyGrid() ? KeyIdPolicy.nineKey(digit) : KeyIdPolicy.character(digit));
    this.typeNumberPadText(digit);
  }

  /** 拼音网格的 1 键（@#）：候选条换成当前模式的常用标点（与逗号长按的「常用标点」同一组），组字中也一样，Engine 下一次回报时换回它的候选。 */
  private showNineKeyQuickMarks(): void {
    this.nineKeyQuickMarks = QuickPunctuationPolicy.entries(this.english, this.engineScheme, this.localMode);
  }

  /** 点选 1 键放进来的一个标点：先清掉候选条，再与侧栏标点一样交给 Engine 的标点处理，组字中先上屏首选。空闲时标点不一定经过 Engine 回报（智能标点的连按替换就不经过），所以不能等 `onChanged` 来清。 */
  private chooseNineKeyQuickMark(index: number): void {
    const mark: PunctuationEntry = this.nineKeyQuickMarks[index];
    this.nineKeyQuickMarks = [];
    KeyboardSession.shared.punctuation(mark.input.charCodeAt(0));
  }

  /** 触屏九键外框的底行：英文九键的字母层在空格两侧多出句点和逗号，中文九键的字母层按 `touch_nine_key_style` 排（`NineKeyLayout.actionRow`），「右侧大回车」时是网格下方那一段。拼音九键、九键双拼、英文九键和笔画的数字层带着 符，符 和切换键与字母层同一格（#10061）；其余键面（含假名网格）借来的数字九宫格不带。「中/英键放在空格左边」时每一种排法（含右侧大回车和数字层）的 中/英 都挪到空格左边（`NineKeyLayout.actionRow` 的 `languageLeft`）。 */
  private nineKeyActions(): NineKeyAction[] {
    // 「符」与「123」互换后 26 键字母层以 符 打头，从它借来的数字九宫格也按九键一族的次序排（符 | 返回），返回键仍在字母层 123 那一格（#10056）。
    const nineKeyDigits: boolean = this.symbols
      && (this.ownsNineKeyGrid() || this.strokeFace()
        || this.letterRowSymbolLeads());
    return NineKeyLayout.actionRow(this.symbols, this.english, this.nineKeyFrameStyle(), nineKeyDigits,
      this.numberPadSpace, this.languageKeyLeft);
  }

  /** 这块键面字母层的设计底行是否以 符 打头：只有打开「符」与「123」互换、字母层是 26 键一族、底行样式里有 符（标准、逗号句号在空格两侧）时才是。不看此刻在不在数字层，借来的数字九宫格按它排前两键。手写、14/18 键、大千注音的字母层底行没有 符，假名网格用共享底行、不跟这一项互换。 */
  private letterRowSymbolLeads(): boolean {
    if (!this.twentySixSymbolFirst || this.japanese
      || KeyboardScheme.usesHandwritingFace(this.scheme, this.english, false, this.localMode)) {
      return false;
    }
    const commaSecondary: boolean = KeyboardScheme.designRowUsesCommaSecondaryPeriod(
      SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english, this.localMode),
      this.drawsFourteenKey(), false);
    const styled: StyledFace = this.chineseTwentySixFace() && commaSecondary ? StyledFace.TWENTY_SIX : StyledFace.NONE;
    const row: DesignRowEntry[] = TouchLayoutStylePolicy.designRow(commaSecondary,
      KeyboardScheme.designRowHasEnglishPeriod(this.english, commaSecondary), styled, this.designRowStyle(styled));
    return TouchLayoutStylePolicy.symbolLeads(TouchLayoutStylePolicy.symbolFirst(row, true));
  }

  /**
   * 九键外框此刻按哪种样式排：密码框和只收数字的编辑框里是标准（`TouchLayoutStylePolicy.fieldRowStyle`）；「右侧大回车」只给中文九键一族的键面（拼音九键、九键双拼的网格，笔画键盘，以及它们的数字层），其他键面（含假名网格）借用数字九宫格时仍是原来的数字层。
   */
  private nineKeyFrameStyle(): string {
    const style: string = TouchLayoutStylePolicy.fieldRowStyle(this.nineKeyStyle, this.standardRowsField);
    if (style === TouchLayoutStylePolicy.TALL_RETURN
      && !this.ownsNineKeyGrid() && !this.strokeFace()) {
      return TouchLayoutStylePolicy.STANDARD;
    }
    return style;
  }

  /** 这块键面画「右侧大回车」外框（`tallReturnNineKeyFrame`）：触屏、不是假名网格、中文九键一族选了这个样式（`NineKeyLayout.tallReturn`）。 */
  private tallReturnFrame(): boolean {
    return this.touchDesign() && !this.japanese && NineKeyLayout.tallReturn(this.english, this.nineKeyFrameStyle());
  }

  /**
   * `nineKeyActionRow` 逐个画的键（`NineKeyLayout.actionItems`）：每次重建这一行，底行表（`nineKeyActions`）和「右侧大回车」量到外框宽度后的 `NineKeyLayout.tallReturnRow` 都只在这里算一次。没量到外框宽度或不是这个样式时没有固定宽度，全按份额排。
   *
   * 它是 ForEach 的数组，随它读到的状态（切层、中/英、样式、量到的外框宽度）重新求值；宽度、份额和让位按值传给每个键的 @Builder，都在 ForEach 的键里，变了就重建那个键，与 `designRowItems` 同一个道理。
   */
  private nineKeyActionItems(): NineKeyActionItem[] {
    const fixed: NineKeyAction[] = this.tallReturnFrame() && this.tallReturnFaceWidth > 0
      ? NineKeyLayout.tallReturnRow(this.symbols, this.tallReturnFaceWidth, this.keySpacing(), this.languageKeyLeft)
      : [];
    return NineKeyLayout.actionItems(this.nineKeyActions(), fixed, NineKeyLayout.tallReturnFill(this.symbols));
  }

  /** 九键外框底行的句点或句号，经 Engine 的标点路由输入。 */
  @Builder
  nineKeyPeriodKey(share: number) {
    this.fixedKey({
      face: this.chinesePunctuationFaces() ? '。' : '.',
      label: KeyAccessibilityPolicy.period(),
      size: TOUCH_LETTER_FONT_SIZE,
      emphasized: false,
      width: 0,
      share: share,
      action: () => this.typePeriod()
    });
  }

  /** 九键外框底行的逗号。中文九键「底栏带标点」的逗号长按打开常用标点列表，与 Android、iOS 相同（`NineKeyLayout.commaOpensQuickPunctuation`）；英文九键的逗号不接长按。 */
  @Builder
  nineKeyCommaKey(share: number) {
    if (NineKeyLayout.commaOpensQuickPunctuation(this.symbols, this.english, this.nineKeyFrameStyle())) {
      Column() {
        this.fixedKey({
          face: this.chinesePunctuationFaces() ? '，' : ',',
          label: '逗号；长按打开常用标点',
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: '100%',
          action: () => this.typeComma()
        });
      }
      .layoutWeight(share)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => {
        this.managing = -1;
        this.brackets = false;
        this.punctuation = true;
      }))
    } else {
      this.fixedKey({
        face: this.chinesePunctuationFaces() ? '，' : ',',
        label: '逗号',
        size: TOUCH_LETTER_FONT_SIZE,
        emphasized: false,
        width: 0,
        share: share,
        action: () => this.typeComma()
      });
    }
  }

  /** 九键外框数字层底行的 0：标准排在空格右边，「右侧大回车」排在空格左边、占剩余宽度。 */
  @Builder
  nineKeyZeroKey(width: number, share: number) {
    this.fixedKey({
      face: '0',
      label: '数字 0',
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      width: width,
      share: share,
      action: () => this.typeNineKeyZero()
    });
  }

  /**
   * 触屏九键外框（拼音九键和笔画键盘）的底行，键序和份额见 `NineKeyLayout.actionRow`：符 | 123 | space | 中 | return，中/英键紧挨回车，与字母键面的底行一致。中文时标点在网格两侧的列里，所以这里没有逗号和句号，与 Android 一致；英文九键的字母层在空格左边加句点、右边加逗号，与三端英文 26 键底行一致。123 把网格重新标注为数字，在数字层上该键显示「九键」，即 Android 中这个网格的 `KeyboardActionRow.layerTitle`；其他键面借用这个数字页时，该键按原键面印「拼音」「ABC」「14键」「18键」「笔」「注」「한」或「あいう」（`numberPadReturnTitle`），与 Android 和 iOS 相同。数字层的网格只有 1-9，0 排在底行空格右边：拼音九键、九键双拼、英文九键和笔画的数字层保留 符，符 和切换键与字母层同一格，0 的一份从空格让出（#10061）；借来的数字九宫格没有 符，0 占 符 让出的那一份。
   */
  @Builder
  nineKeyActionRow() {
    Row({ space: this.keySpacing() }) {
      ForEach(this.nineKeyActionItems(), (item: NineKeyActionItem) => {
        this.nineKeyActionKey(item)
      }, (item: NineKeyActionItem): string => NineKeyLayout.actionItemKey(item))
    }
    .width('100%')
    .height(this.rowHeight(3))
  }

  /** 九键外框底行上的一个键（`nineKeyActionItems` 的一项）：宽度、份额和让位按值收下，它们在 ForEach 的键里，变了这个键整个重建；其余内容都在属性里直接读状态。 */
  @Builder
  nineKeyActionKey(item: NineKeyActionItem) {
    if (item.kind === NineKeyActionKind.SYMBOL_PANEL) {
      this.nineKeySymbolPanelKey(0, item.share)
    } else if (item.kind === NineKeyActionKind.LAYER_TOGGLE) {
      // 长按 123 直接打开符号面板，不必再经过 #+= 层；符号层里这个键是返回，长按不做事。
      Column() {
        this.fixedKey({
          face: this.symbols
            ? this.numberPadReturnTitle()
            : '123',
          label: KeyAccessibilityPolicy.layoutToggle(this.symbols),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: '100%',
          action: () => this.toggleSymbolLayer()
        });
      }
      .width(item.width)
      .layoutWeight(item.share)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => this.openSymbolsFromLayerKey()))
    } else if (item.kind === NineKeyActionKind.EMOJI) {
      // 九键样式「表情键」：😀 紧挨 123，宽度从空格让出。
      this.emojiKey(item.share);
    } else if (item.kind === NineKeyActionKind.PERIOD) {
      // 英文九键的句点紧挨空格左边，与英文 26 键底行相同；「底栏带标点」的句号在空格右边。经 Engine 的标点路由输入。
      this.nineKeyPeriodKey(item.share)
    } else if (item.kind === NineKeyActionKind.COMMA) {
      // 「底栏带标点」的逗号紧挨空格左边；英文九键的逗号紧挨空格右边。
      this.nineKeyCommaKey(item.share)
    } else if (item.kind === NineKeyActionKind.ZERO) {
      // 标准数字层的 0 在空格右边；「右侧大回车」的数字层里 0 占空格原来的位置和剩余宽度，空格在它右边。
      this.nineKeyZeroKey(item.width, item.share)
    } else if (item.kind === NineKeyActionKind.SPACE) {
      this.spaceKey(true, item.share, item.width);
    } else if (item.kind === NineKeyActionKind.LANGUAGE) {
      this.fixedKey({
        face: this.languageFace(),
        label: KeyAccessibilityPolicy.language(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: item.width,
        share: item.share,
        // 「右侧大回车」的回车在右列、不在这一行，中/英 不伸进回车键帽（`NineKeyLayout.languageBesideReturn`）。
        hitRegion: item.besideReturn ? this.languageKeyHitRegion() : undefined,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LANGUAGE);
          this.toggleTouchLanguage();
        }
      });
    } else if (item.kind === NineKeyActionKind.RETURN) {
      this.designReturnKey(item.share, item.besideReturn);
    }
  }

  @Builder
  nineKeyGrid() {
    Row({ space: this.keySpacing() }) {
      Column() {
        // One column, two jobs, exactly as the iOS sidebar does it: the readings the digits could
        // stand for while something is being spelled, and the common punctuation when nothing is.
        if (this.symbols) {
          ForEach(NineKeyLayout.digitPunctuation(), (symbol: string, index: number) => {
            Column() {
              Text(symbol)
                .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                .fontColor(this.skin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
            }
            .layoutWeight(1)
            .justifyContent(FlexAlign.Center)
            .width('100%')
            .accessibilityText(KeyAccessibilityPolicy.symbol(symbol))
            .responseRegion(this.railHitRegion(index, NineKeyLayout.digitPunctuation().length))
            .onClick(() => {
              KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
              this.insertNumberPadLiteral(symbol);
            })
          }, (symbol: string) => symbol + this.faceKey())
        } else if (this.spellings.length > 0) {
          Scroll() {
            Column() {
              ForEach(this.spellings, (spelling: string, index: number) => {
                Text(spelling)
                  .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                  .fontColor(this.skin.accent)
                .textAlign(TextAlign.Center)
                .width('100%')
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
                .accessibilityText(KeyAccessibilityPolicy.spelling(spelling))
                .onClick(() => KeyboardSession.shared.chooseSpelling(index))
              })
            }
            .width('100%')
          }
          .scrollBar(BarState.Off)
          .width('100%')
          .height('100%')
        } else {
          ForEach(NineKeyLayout.punctuation(), (symbol: string, index: number) => {
            Column() {
              Text(this.symbolFace(symbol))
                .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                .fontColor(this.skin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
            }
            .layoutWeight(1)
            .justifyContent(FlexAlign.Center)
            .width('100%')
            .responseRegion(this.railHitRegion(index, NineKeyLayout.punctuation().length))
            .onClick(() => {
              KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
              KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
            })
          }, (symbol: string) => symbol + this.faceKey())
        }
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
      // The sidebar is a column of function keys, and takes their colour.
      .backgroundColor(this.skin.keySurfaceBackground(false, true))
      .borderRadius(KeyboardMetrics.KEY_CORNER_VP)

      Column({ space: this.rowSpacing() }) {
        ForEach(this.symbols ? NineKeyLayout.digits() : NineKeyLayout.rows(), (row: NineKey[]) => {
          Row({ space: this.keySpacing() }) {
            ForEach(row, (key: NineKey) => {
              if (!this.symbols && !this.nineKeyShuangpinFaceActive()
                && NineKeyLayout.holdOptions(key).length > 0) {
                this.nineKeyHoldKey(key);
              } else {
                this.flexibleKey(this.nineKeyKeyOptions(key));
              }
            }, (key: NineKey) => key.input + this.faceKey())
          }
          .layoutWeight(1)
          .width('100%')
        }, (row: NineKey[]) => row[0].input + this.faceKey())
      }
      .layoutWeight(1)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        Column() {
          this.deleteKey('100%', true, 0)
        }
        .layoutWeight(1)
        .width('100%')
        // 字母页按标点路由处理句点；数字页原样写小数点。
        this.nineKeyControl('.', '句点', () => {
          // 小数点记 Period，侧栏的符号才记 SoftPunctuation。
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.character('.'));
          if (this.symbols) this.insertNumberPadLiteral('.');
          else KeyboardSession.shared.punctuation(0x2e);
        });
        this.nineKeyControl('0', '数字 0', () => this.typeNineKeyZero());
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }

  private nineKeyKeyOptions(key: NineKey): KeyOptions {
    const shuangpin: boolean = this.nineKeyShuangpinFaceActive();
    const input: string = shuangpin && (key.input === '@' || key.input === "'") ? '1' : key.input;
    const face: NineKeyShuangpinFace | null = shuangpin
      ? nineKeyShuangpinFace(this.nineKeyShuangpinFaces, input) : null;
    return {
      face: this.symbols ? key.label : (face === null ? this.nineKeyFaceOf(key) : nineKeyShuangpinLabel(face)),
      label: this.symbols ? KeyAccessibilityPolicy.symbol(key.label)
        : (face === null ? this.nineKeyLabel(key) : nineKeyShuangpinLabel(face).replace(/\n/g, '，')),
      // 触屏网格：字母格 17fp、字重 500，数字用旁边 0 键的 22fp。
      size: !this.touchDesign() ? KeyboardMetrics.SMALL_KEY_FONT_SIZE
        : (this.symbols ? TOUCH_LETTER_FONT_SIZE : NINE_KEY_FONT_SIZE),
      fontWeight: this.touchDesign() && !this.symbols ? NINE_KEY_LETTER_WEIGHT : undefined,
      // 只有拼音九键字母层带这个标记，数字层和九键双拼的键不会被滑动输入数字换掉键面。
      nineKeyInput: this.symbols || shuangpin ? undefined : key.input,
      emphasized: false,
      width: 0,
      // A digit is inserted, not spelled: the same call the twenty-six key symbol row makes for
      // its own digit row, rather than handing it to the Engine as a candidate position.
      action: this.symbols
        ? () => this.typeNineKeyDigit(key.input)
        : shuangpin
        ? () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(input));
          KeyboardSession.shared.press(input.charCodeAt(0), false);
        }
        : NineKeyLayout.showsQuickMarks(key)
        ? () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
          this.showNineKeyQuickMarks();
        }
        : () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
          KeyboardSession.shared.press(key.input.charCodeAt(0), false);
        }
    };
  }

  private nineKeyShuangpinFaceActive(): boolean {
    return this.scheme.preferenceId === 'nine_key_shuangpin'
      && !this.symbols && !this.english && this.localMode === 'none';
  }

  @Builder
  nineKeyHoldKey(key: NineKey) {
    Stack({ alignContent: Alignment.TopEnd }) {
      this.keyFace(this.nineKeyKeyOptions(key))
      if (this.touchDesign()) {
        // 长按可输入的数字，包括常用标点键的 1；数字层不重复印。与 26 键字母键的角标同位同号：距顶部 3 vp、距右侧 5 vp，10fp。
        Text(NineKeyLayout.digitHint(key))
          .fontSize(KEY_HINT_FONT_SIZE)
          .fontColor(this.skin.secondary)
          .maxLines(1)
          .margin({ top: CORNER_HINT_TOP_VP, right: CORNER_HINT_RIGHT_VP })
          .hitTestBehavior(HitTestMode.Transparent)
          // 滑动输入数字时键面已经画着这个数字，角标暂时藏起来，松手或取消后回来。
          .visibility(this.nineKeySwipeKey === key.input ? Visibility.Hidden : Visibility.Visible)
      }
    }
    .accessibilityText(this.nineKeyLabel(key))
    .layoutWeight(1)
    .height('100%')
    .backgroundColor(this.skin.keySurfaceBackground(false))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onTouch((event: TouchEvent): void => this.nineKeySwipeTouch(key, event))
    .onClick(() => {
      if (this.nineKeySwipes.swallows(key.input)) {
        // 这次触摸已经滑出去，输入了数字或弹出了选项，点击不再输入字母。
        return;
      }
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
      if (NineKeyLayout.showsQuickMarks(key)) {
        this.showNineKeyQuickMarks();
      } else {
        KeyboardSession.shared.press(key.input.charCodeAt(0), false);
      }
    })
    .gesture(LongPressGesture().onAction(() => {
      if (!this.nineKeySwipes.hold(key.input)) {
        // 滑动已经接管这次触摸，长按不再弹出选项。
        return;
      }
      // 长按仍算这格的一次按键，选项通过字面输入路径上屏。
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
      this.openKeyHold(NineKeyLayout.holdOptions(key));
    }))
  }

  /**
   * 拼音九键字母层 1–9 键上的一次触摸（`NineKeySwipePolicy`，#10229，与 Android `ImeLayoutRows.bindNineKeySwipe` 相同）：按下时读共享偏好 `touch_nine_key_swipe_digits`，沿设定方向滑过 24 vp 的那一刻键面换成这个键的数字、角标藏起来，松手按长按选数字的同一条路径上屏（先结束组字再字面上屏）；反方向滑过 24 vp 的那一刻算这一格按了一次，弹出和长按相同的数字和字母。判定成立后点击和长按都不再做事；系统取消时还原键面，什么也不输入。2in1 没有这项设置，只在触屏键盘上判定。
   */
  private nineKeySwipeTouch(key: NineKey, event: TouchEvent): void {
    if (event.type === TouchType.Down) {
      if (event.changedTouches.length > 0) {
        const touch: TouchObject = event.changedTouches[0];
        // 每次按下读当前的设置：键盘开着时在设置页改了，下一次触摸就生效。
        const mode: string = this.touchDesign() ? this.settings.nineKeySwipeDigits : NineKeySwipePolicy.OFF;
        // 1 键（@#）长按只有 1，没有字母可弹。
        const hasLetters: boolean = NineKeyLayout.holdOptions(key).length > 1;
        this.endNineKeySwipeFace(key.input);
        this.nineKeySwipes.down(key.input, touch.id, touch.windowY, mode, hasLetters);
      }
    } else if (event.type === TouchType.Move) {
      const moved: TouchObject[] = event.changedTouches.length > 0 ? event.changedTouches : event.touches;
      for (const touch of moved) {
        const swipe: NineKeySwipe = this.nineKeySwipes.move(key.input, touch.id, touch.windowY);
        if (swipe === NineKeySwipe.DIGIT) {
          this.nineKeySwipeDigit = NineKeyLayout.digitHint(key);
          this.nineKeySwipeKey = key.input;
        } else if (swipe === NineKeySwipe.LETTERS) {
          // 与长按相同：这一格算按了一次，之后从弹窗里选的不再计。
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
          this.openKeyHold(NineKeyLayout.holdOptions(key));
        }
      }
    } else if (event.type === TouchType.Up) {
      if (event.changedTouches.length > 0) {
        const touch: TouchObject = event.changedTouches[0];
        if (this.nineKeySwipes.up(key.input, touch.id) === NineKeySwipe.DIGIT) {
          this.endNineKeySwipeFace(key.input);
          this.typeNineKeySwipeDigit(key);
        }
      }
    } else if (event.type === TouchType.Cancel) {
      this.nineKeySwipes.cancel(key.input);
      this.endNineKeySwipeFace(key.input);
    }
  }

  /** 滑出来的数字：按键音、振动和键位统计与点按这一格相同，上屏走长按选数字的同一条路径（`selectNineKeyHoldOption`：先结束组字再字面上屏）。 */
  private typeNineKeySwipeDigit(key: NineKey): void {
    const digit: string = NineKeyLayout.digitHint(key);
    if (digit.length === 0) {
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.nineKey(key.input));
    KeyboardSession.shared.holdFeedback();
    KeyboardSession.shared.finishComposition();
    KeyboardSession.shared.insert(digit);
  }

  private endNineKeySwipeFace(input: string): void {
    if (this.nineKeySwipeKey === input) {
      this.nineKeySwipeKey = '';
      this.nineKeySwipeDigit = '';
    }
  }

  /** 键面文字：这一键正滑动输入数字时画那个数字，否则画原来的键面。 */
  private nineKeySwipeFace(input: string | undefined, face: string): string {
    return input !== undefined && input === this.nineKeySwipeKey && this.nineKeySwipeDigit.length > 0
      ? this.nineKeySwipeDigit : face;
  }

  /**
   * 全拼 14 键的字母层，与 Android（`ImeLetterRows`）和 iOS（`FourteenKeyLayout`）同一张表：`QW ER TY UI OP` / `AS DF GH JK L`（不缩进）/ `[分词] ZX CV BN M [⌫]`，四行等高，总高度与 26 键相同，底行就是 26 键的 `designActionRow`。
   *
   * 键面写两个大字母，没有角标；不画 Shift，不接滑行、角标滑动和双拼提示。点一下把这一组的首字母交给引擎（`pressGridKey`），长按弹出这一键的两个字母。
   */
  @Builder
  fourteenKeyFace() {
    Row({ space: this.keySpacing() }) {
      ForEach(FourteenKeyLayout.rows()[0], (key: FourteenKey) => {
        this.fourteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
    }
    .width('100%')
    .height(this.rowHeight(0))

    Row({ space: this.keySpacing() }) {
      ForEach(FourteenKeyLayout.rows()[1], (key: FourteenKey) => {
        this.fourteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
    }
    .width('100%')
    .height(this.rowHeight(1))

    Row({ space: this.keySpacing() }) {
      // 左端键拼音组字中是「分词」，送 ' 在已打的键末尾定一个音节分界；没在组字时、以及五笔 14 键一直是「符」，打开符号面板。宽度同 26 键的 Shift。
      this.fixedKey({
        face: this.fourteenKeySeparates() ? '分词' : '符',
        label: this.fourteenKeySeparates() ? NineKeyLayout.separator().description
          : KeyAccessibilityPolicy.symbolPanel(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: LETTER_EDGE_SHARE,
        action: () => this.tapFourteenKeySeparator()
      });
      ForEach(FourteenKeyLayout.rows()[2], (key: FourteenKey) => {
        this.fourteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, LETTER_EDGE_SHARE);
    }
    .width('100%')
    .height(this.rowHeight(2))

    this.actionRow()
  }

  /** 全拼 18 键的三排组码键面；三排从同一左右边界铺开，第二排六个键平分整行（A 与 Q 左对齐、L 与 P 右对齐，与 Android、iOS 相同），第三排两端留给分词和删除。 */
  @Builder
  eighteenKeyFace() {
    Row({ space: this.keySpacing() }) {
      ForEach(EighteenKeyLayout.rows()[0], (key: FourteenKey) => {
        this.eighteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
    }
    .width('100%').height(this.rowHeight(0))

    Row({ space: this.keySpacing() }) {
      ForEach(EighteenKeyLayout.rows()[1], (key: FourteenKey) => {
        this.eighteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
    }
    .width('100%').height(this.rowHeight(1))

    Row({ space: this.keySpacing() }) {
      this.fixedKey({
        face: this.editing.length > 0 ? '分词' : '符',
        label: this.editing.length > 0 ? NineKeyLayout.separator().description
          : KeyAccessibilityPolicy.symbolPanel(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: LETTER_EDGE_SHARE,
        action: () => this.tapEighteenKeySeparator()
      });
      ForEach(EighteenKeyLayout.rows()[2], (key: FourteenKey) => {
        this.eighteenKeyButton(key)
      }, (key: FourteenKey) => key.letters + this.faceKey())
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, LETTER_EDGE_SHARE);
    }
    .width('100%').height(this.rowHeight(2))

    this.actionRow()
  }

  /** 14 键的一个字母键：两个字母的键可以长按，L、M 只有点按。 */
  @Builder
  fourteenKeyButton(key: FourteenKey) {
    if (FourteenKeyLayout.holdOptions(key).length > 0) {
      this.fourteenKeyHoldKey(key)
    } else {
      this.flexibleKey(this.fourteenKeyOptions(key))
    }
  }

  @Builder
  eighteenKeyButton(key: FourteenKey) {
    if (EighteenKeyLayout.holdOptions(key).length > 0) {
      this.eighteenKeyHoldKey(key)
    } else {
      this.flexibleKey(this.eighteenKeyOptions(key))
    }
  }

  private eighteenKeyOptions(key: FourteenKey): KeyOptions {
    return {
      face: this.compactKeyFace(key),
      label: this.compactKeyDescription(key),
      swipeLetters: key.letters,
      size: TOUCH_LETTER_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapEighteenKey(key)
    };
  }

  private tapEighteenKey(key: FourteenKey): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.eighteenKey(key.letters));
    KeyboardSession.shared.pressGridKey(key.input.charCodeAt(0));
  }

  private tapEighteenKeySeparator(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
    if (KeyboardSession.shared.composing()) {
      KeyboardSession.shared.press(NineKeyLayout.separator().input.charCodeAt(0), false);
      return;
    }
    this.openSymbolSurface();
  }

  @Builder
  eighteenKeyHoldKey(key: FourteenKey) {
    Column() {
      this.keyFace(this.eighteenKeyOptions(key))
    }
    .accessibilityText(this.compactKeyDescription(key))
    .layoutWeight(1)
    .height('100%')
    .backgroundColor(this.skin.keySurfaceBackground(false))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onTouch((event: TouchEvent): void => this.eighteenKeySwipeTouch(key, event))
    .onClick((event: ClickEvent): void => this.clickEighteenKey(key, event))
    .gesture(LongPressGesture().onAction(() => {
      if (this.swipesEighteenKey() && !this.eighteenSwipes.hold(key.letters)) {
        // 横滑已经接管这次按压，长按不再弹出字母。
        return;
      }
      this.endEighteenSwipeFace(key.letters);
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.eighteenKey(key.letters));
      this.openKeyHold(EighteenKeyLayout.holdOptions(key));
    }))
  }

  /** 两个字母的 18 键是否接横滑：只有双拼 18 键，全拼 18 键只点按。 */
  private swipesEighteenKey(): boolean {
    return this.scheme.engineScheme === 'shuangpin' && this.drawsEighteenKey();
  }

  /**
   * 双拼 18 键两个字母的键上的一次触摸（`EighteenKeySwipePolicy`）：拖动时键面画选中的字母，抬手把它以大写精确标记交给引擎；回到起点、同一键上的第二根手指和系统取消都不输入。没越过阈值的触摸交给这个键的点击。
   */
  private eighteenKeySwipeTouch(key: FourteenKey, event: TouchEvent): void {
    if (!this.swipesEighteenKey()) {
      return;
    }
    if (event.type === TouchType.Down) {
      if (event.changedTouches.length > 0) {
        const touch: TouchObject = event.changedTouches[0];
        this.eighteenSwipes.down(key.letters, touch.id, touch.windowX, touch.windowY);
        this.showEighteenSwipeFace(key.letters, '');
      }
    } else if (event.type === TouchType.Move) {
      const moved: TouchObject[] = event.changedTouches.length > 0 ? event.changedTouches : event.touches;
      for (const touch of moved) {
        const selected: string | null = this.eighteenSwipes.move(key.letters, touch.id, touch.windowX, touch.windowY);
        if (selected !== null) {
          this.showEighteenSwipeFace(key.letters, selected);
        }
      }
    } else if (event.type === TouchType.Up) {
      if (event.changedTouches.length > 0) {
        const touch: TouchObject = event.changedTouches[0];
        const letter: string = this.eighteenSwipes.up(key.letters, touch.id, touch.windowX, touch.windowY);
        this.endEighteenSwipeFace(key.letters);
        this.typeEighteenKeyLetter(key, letter);
      }
    } else if (event.type === TouchType.Cancel) {
      this.eighteenSwipes.cancel(key.letters);
      this.endEighteenSwipeFace(key.letters);
    }
  }

  /** 18 键的点击：全拼送整组；双拼由横滑判定这次按压是整组、精确字母还是已被横滑接管。 */
  private clickEighteenKey(key: FourteenKey, event: ClickEvent): void {
    if (!this.swipesEighteenKey()) {
      this.tapEighteenKey(key);
      return;
    }
    const tap: EighteenKeyTap = this.eighteenSwipes.click(key.letters, event.windowX, event.windowY);
    if (tap.group) {
      this.tapEighteenKey(key);
    } else {
      this.endEighteenSwipeFace(key.letters);
      this.typeEighteenKeyLetter(key, tap.letter);
    }
  }

  /** 横滑选中的字母以大写精确标记送入当前网格；键位与点按、长按同记在这一键上。 */
  private typeEighteenKeyLetter(key: FourteenKey, letter: string): void {
    if (letter.length === 0) {
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.eighteenKey(key.letters));
    KeyboardSession.shared.pressGridKey(letter.charCodeAt(0));
  }

  private showEighteenSwipeFace(letters: string, letter: string): void {
    if (letter.length === 0 && this.eighteenSwipeKey !== letters) {
      return;
    }
    if (this.eighteenSwipeKey !== letters) {
      this.eighteenSwipeKey = letters;
    }
    if (this.eighteenSwipeLetter !== letter) {
      this.eighteenSwipeLetter = letter;
    }
  }

  private endEighteenSwipeFace(letters: string): void {
    if (this.eighteenSwipeKey === letters) {
      this.eighteenSwipeKey = '';
      this.eighteenSwipeLetter = '';
    }
  }

  /** 键面文字：横滑正选中这一键的字母时画那个字母，否则画原来的键面。 */
  private eighteenSwipeFace(letters: string | undefined, face: string): string {
    return letters !== undefined && letters === this.eighteenSwipeKey && this.eighteenSwipeLetter.length > 0
      ? this.eighteenSwipeLetter : face;
  }

  private fourteenKeyOptions(key: FourteenKey): KeyOptions {
    return {
      face: this.compactKeyFace(key),
      label: this.compactKeyDescription(key),
      size: TOUCH_LETTER_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapFourteenKey(key)
    };
  }

  /** 双拼 14/18 键把微软双拼所需的分号并入 L 键；其他方案沿用共享键表的键面和读屏。 */
  private compactKeyFace(key: FourteenKey): string {
    return this.scheme.engineScheme === 'shuangpin' && key.letters === 'l' ? 'L;' : key.label;
  }

  private compactKeyDescription(key: FourteenKey): string {
    return this.scheme.engineScheme === 'shuangpin' && key.letters === 'l'
      ? '按键 L 分号' : key.description;
  }

  /** 点一下 14 键：记这一键的键位，把这一组的首字母交给引擎。 */
  private tapFourteenKey(key: FourteenKey): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.fourteenKey(key.letters));
    KeyboardSession.shared.pressGridKey(key.input.charCodeAt(0));
  }

  private usesQuanpinSeparator(): boolean {
    return !this.quanpinShift && KeyboardScheme.usesQuanpinSeparator(this.scheme, this.touchDesign(),
      this.english, this.symbols, this.localMode);
  }

  private tapLetterShift(): void {
    if (this.usesQuanpinSeparator()) {
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.character("'"));
      if (KeyboardSession.shared.composing()) {
        KeyboardSession.shared.press("'".charCodeAt(0), false);
      }
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SHIFT);
    this.letterCase.toggle(Date.now());
    this.caseMode = this.letterCase.mode();
  }

  /** 14 键第三排左端键此刻是不是「分词」：拼音组字中是；五笔不切音节，一直是「符」。 */
  private fourteenKeySeparates(): boolean {
    return KeyboardScheme.fourteenKeySeparates(this.scheme.engineScheme, this.editing.length > 0);
  }

  /** 第三行左端键：拼音组字中送分词符 '，空闲时和五笔 14 键打开符号面板（Android `ImeLayoutRows.symbolKey`）。 */
  private tapFourteenKeySeparator(): void {
    // 一个键一个键位 id：不论组字与否都记作符号键，与 Android、iOS 的 14 键相同。
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
    if (KeyboardScheme.fourteenKeySeparates(this.scheme.engineScheme, KeyboardSession.shared.composing())) {
      KeyboardSession.shared.press(NineKeyLayout.separator().input.charCodeAt(0), false);
      return;
    }
    this.openSymbolSurface();
  }

  /** 两个字母的 14 键：点按同 `fourteenKeyOptions`，长按弹出这两个字母。 */
  @Builder
  fourteenKeyHoldKey(key: FourteenKey) {
    Column() {
      this.keyFace(this.fourteenKeyOptions(key))
    }
    .accessibilityText(this.compactKeyDescription(key))
    .layoutWeight(1)
    .height('100%')
    .backgroundColor(this.skin.keySurfaceBackground(false))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onClick(() => this.tapFourteenKey(key))
    .gesture(LongPressGesture().onAction(() => {
      // 长按也是按了这一键一次，不管最后选哪个字母。
      KeyboardSession.shared.softKeyPress(KeyIdPolicy.fourteenKey(key.letters));
      this.openKeyHold(FourteenKeyLayout.holdOptions(key));
    }))
  }

  /**
   * The row every key face ends with: the layout toggle, a comma, space, the language key and Return.
   *
   * It used to be written inline at the end of the twenty-six key rows, which meant the grid faces —
   * quanpin nine-key, the Japanese kana grid, handwriting — rendered without it and had no space
   * key, no way to commit with Return and no way to leave English or reach the digits. The only exit
   * from those faces was tapping a candidate. It is a builder now so that every face ends the same
   * way, which is also how the Apple host is arranged.
   *
   * 触屏键盘的字母键面改用设计稿的底行（`designActionRow`），九键网格和笔画键盘用九键外框的底行（`nineKeyActionRow`）；触屏 123 和 #+= 层自己绘制底行（`designLayerFace`），触屏手写用 `handwritingActionRow`。假名网格和所有 2in1 屏幕键盘（含 2in1 的手写面板）仍用这一行。
   */
  @Builder
  actionRow() {
    if (this.designBottomRow()) {
      this.designActionRow()
    } else if (this.touchNineKeyRow()) {
      this.nineKeyActionRow()
    } else {
      this.classicActionRow()
    }
  }

  /** 到这里结束的键面是否使用设计稿的底行：触屏键盘的字母行（26 键、大千行以及大千键面保留的共享符号行），而不是带自己标点列的网格、假名网格或手写板（触屏手写板另有 `handwritingActionRow`）。 */
  private designBottomRow(): boolean {
    return this.touchDesign() && !this.gridActionRow() && !this.japanese
      && !KeyboardScheme.usesHandwritingFace(this.scheme, this.english, this.symbols, this.localMode);
  }

  /** 实际会输入的逗号和句号：开着中文标点组合中文时是中文标点；英文、关闭中文标点时，以及标点本就是 ASCII 的韩文、越南语和藏文，用 ASCII 标点。 */
  private chinesePunctuationFaces(): boolean {
    return !this.english && this.chinesePunctuation && !this.koreanFace() && !this.vietnameseFace()
      && !this.tibetanFace();
  }

  /** 只换行时回车键显示回车箭头；编辑框要求了动作时保留其动作词（搜索、发送、前往、下一项），这对用户有实际意义；有组合时显示「确认」。 */
  private returnUsesIcon(): boolean {
    return !this.returnComposing()
      && !ReturnKeyAction.performsEditorAction(KeyboardSession.shared.returnAction(), false);
  }

  /** 语言键显示的内容：当前模式「中」或「英」，有自己文字的方案则显示其文字。 */
  private languageFace(): string {
    if (this.english) {
      return '英';
    }
    if (this.koreanFace()) {
      return '한';
    }
    if (this.vietnameseFace()) {
      return '越';
    }
    return this.tibetanFace() ? '藏' : '中';
  }

  private typeComma(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(','));
    // 大千的逗号键是 ㄝ，所以注音直接输入标点本身。
    if (this.zhuyinFace()) {
      this.typeZhuyinMark(',');
    } else {
      KeyboardSession.shared.punctuation(0x2c);
    }
  }

  private typePeriod(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character('.'));
    // 大千的句号键是 ㄡ，所以注音与逗号一样直接输入标点本身。
    if (this.zhuyinFace()) {
      this.typeZhuyinMark('.');
    } else {
      KeyboardSession.shared.punctuation(0x2e);
    }
  }

  /** 英文 26 键字母层在空格左边另放独立句点，空格右边仍是逗号。 */
  private designEnglishPeriod(): boolean {
    return KeyboardScheme.designRowHasEnglishPeriod(this.english, this.designCommaSecondaryPeriod());
  }

  /** 26 键字母层使用独立逗号、长按句号，其余设计底行保留两个标点键。 */
  private designCommaSecondaryPeriod(): boolean {
    return KeyboardScheme.designRowUsesCommaSecondaryPeriod(
      SchemeCompositionPolicy.selectedRulesScheme(this.scheme.engineScheme, this.english, this.localMode),
      this.drawsFourteenKey(), this.symbols);
  }

  /** 判定数字行的键面状态（`TouchLayoutStylePolicy.lettersRowFace`）：与在不在 123 层无关，手写按方案本身算。 */
  private numberRowFace(): NumberRowFace {
    return this.numberRowFaceIn(this.english);
  }

  /** 这块键盘在中文（`english` 为假）或英文下的字母层键面（`TouchLayoutStylePolicy.letterFace`）；九键网格、14/18 键和假名网格跟着方案走，不随中/英变。 */
  private numberRowFaceIn(english: boolean): NumberRowFace {
    return TouchLayoutStylePolicy.letterFace(this.touchKeyboard(), english);
  }

  /** 用户选的方案和键面（`TouchKeyboard`），不随中/英变。 */
  private touchKeyboard(): TouchKeyboard {
    return {
      scheme: this.scheme,
      localMode: this.localMode,
      nineKey: this.nineKey,
      letterGrid: this.fourteenKey || this.eighteenKey,
      kanaGrid: this.japanese
    };
  }

  /** 字母层是中文 26 键（全拼、26 键双拼、五笔 26 键、粤拼等），底行样式 `touch_twenty_six_key_style` 作用于它；与在不在 123 层无关。英文 26 键、韩文、越南语、藏文、日语罗马字和 2in1 都不算。 */
  private chineseTwentySixFace(): boolean {
    return !this.english && this.chineseTwentySixKeyboard();
  }

  /** 这块键盘的中文键面是中文 26 键（`TouchLayoutStylePolicy.chineseTwentySixKeyboard`）：与 `chineseTwentySixFace` 同一套判断，但按中文算，同一块键盘切到英文 26 键时仍为真。长空格样式留不留工具条那一行只看它（`longSpaceToolbarRow`）；韩文、越南语、藏文、日语、14/18 键、九键、手写、笔画、大千注音的键盘和 2in1 都不算。 */
  private chineseTwentySixKeyboard(): boolean {
    return this.touchDesign() && TouchLayoutStylePolicy.chineseTwentySixKeyboard(this.touchKeyboard());
  }

  /** 长空格样式把工具条那一行一直留着（符 在上面）：看用户的 26 键样式和这块键盘的中文键面（`chineseTwentySixKeyboard`），不看此刻的中/英，与 Android `longSpaceToolbarRowKept` 相同；密码框和只收数字的编辑框里不留。英文 26 键画自己的标准底行，工具栏上没有 符（`longSpaceActive`），但这一行照样画着，中/英 来回切时面板高度不跳。 */
  private longSpaceToolbarRow(): boolean {
    return TouchLayoutStylePolicy.longSpaceActive(this.chineseTwentySixKeyboard(), this.twentySixKeyStyle,
      this.standardRowsField);
  }

  /** 中文 26 键处在长空格样式（`TouchLayoutStylePolicy.longSpaceActive`）：符号入口在工具栏，123 与其他键面一样打开底行带空格的数字九宫格；密码框和只收数字的编辑框里不算，与 Android 一致。 */
  private longSpaceActive(): boolean {
    return TouchLayoutStylePolicy.longSpaceActive(this.chineseTwentySixFace(), this.twentySixKeyStyle,
      this.standardRowsField);
  }

  /** 字母上方的数字行（或 123 层上的补充行）多出的行数，0 或 1（`TouchLayoutStylePolicy.numberRowExtraRows`）；2in1 没有。 */
  private numberRowExtraRows(): number {
    return this.touchDesign()
      ? TouchLayoutStylePolicy.numberRowExtraRows(this.numberRow, this.numberRowFace(), this.phoneLandscape) : 0;
  }

  /** 数字行或补充行连同它下面那道行距占的高度。 */
  private numberRowExtraHeight(): number {
    return this.numberRowExtraRows() * (KeyboardMetrics.ROW_HEIGHT_VP + this.rowSpacing());
  }

  /** 设计底行按哪种键面类型的样式排：中文 26 键的字母层、14/18 键的字母层，其余不受样式影响。 */
  private designRowStyledFace(): StyledFace {
    if (this.chineseTwentySixFace() && this.designCommaSecondaryPeriod()) {
      return StyledFace.TWENTY_SIX;
    }
    if (this.drawsFourteenKey() && !this.symbols) {
      return StyledFace.FOURTEEN;
    }
    return StyledFace.NONE;
  }

  /** 设计底行实际按哪种样式排：密码框和只收数字的编辑框里一律是标准底行（`TouchLayoutStylePolicy.fieldRowStyle`）。`styled` 是调用方算好的 `designRowStyledFace`，同一次求值里不重算。 */
  private designRowStyle(styled: StyledFace): string {
    if (styled === StyledFace.TWENTY_SIX) {
      return TouchLayoutStylePolicy.fieldRowStyle(this.twentySixKeyStyle, this.standardRowsField);
    }
    return styled === StyledFace.FOURTEEN
      ? TouchLayoutStylePolicy.fieldRowStyle(this.fourteenKeyStyle, this.standardRowsField)
      : TouchLayoutStylePolicy.STANDARD;
  }

  /** 设计底行的键序与份额（`TouchLayoutStylePolicy.designRow`）；打开「符」与「123」互换时 26 键一族的 符 排到 123 左边（`TouchLayoutStylePolicy.symbolFirst`），之后「中/英键放在空格左边」再把 中/英 挪到空格（和紧贴空格的标点）左边（`TouchLayoutStylePolicy.languageBeforeSpace`）。密码框和数字框里样式退回标准，这两项不退回。 */
  private designRowEntries(): DesignRowEntry[] {
    const styled: StyledFace = this.designRowStyledFace();
    const row: DesignRowEntry[] = TouchLayoutStylePolicy.symbolFirst(TouchLayoutStylePolicy.designRow(
      this.designCommaSecondaryPeriod(), this.designEnglishPeriod(), styled, this.designRowStyle(styled)),
      this.twentySixSymbolFirst);
    return TouchLayoutStylePolicy.languageBeforeSpace(row, this.languageKeyLeft);
  }

  /**
   * `designActionRow` 逐个画的键（`TouchLayoutStylePolicy.designRowItems`）：每次重建这一行，底行表只在这里算一次。
   *
   * 它是 ForEach 的数组，ForEach 的数组表达式会随它读到的状态（中/英、样式、编辑框等）重新求值；份额和让位按值传给每个键的 @Builder，编译后那次调用只在建键时执行一次，所以它们都在 ForEach 的键里，变了就重建那个键。不能像 `this.spaceKey(true, 某个随状态变的份额, 0)` 那样在一行里直接按值传：键建好之后那一处一直停在建键时的值。
   */
  private designRowItems(): DesignRowItem[] {
    return TouchLayoutStylePolicy.designRowItems(this.designRowEntries());
  }

  /** 底行逗号的长按；`styled` 是调用方算好的 `designRowStyledFace`。 */
  private designCommaHold(styled: StyledFace): CommaHold {
    return TouchLayoutStylePolicy.commaHold(this.designCommaSecondaryPeriod(), styled, this.designRowStyle(styled));
  }

  /** 底行逗号接不接滑动；`styled` 是调用方算好的 `designRowStyledFace`。 */
  private designCommaSwipes(styled: StyledFace): boolean {
    return TouchLayoutStylePolicy.commaSwipesPeriod(styled, this.designRowStyle(styled));
  }

  /** 底行逗号的读屏文字：默认的两种底行保持原来的说法，样式新增的说法写明滑动和长按做什么。`styled` 是调用方算好的 `designRowStyledFace`。 */
  private designCommaLabel(styled: StyledFace): string {
    const hold: CommaHold = this.designCommaHold(styled);
    if (this.designCommaSwipes(styled)) {
      return hold === CommaHold.PERIOD ? '逗号；上滑、下滑或长按输入句号' : '逗号；上滑或下滑输入句号，长按打开常用标点';
    }
    if (hold === CommaHold.PERIOD) {
      return '逗号；长按输入句号';
    }
    return styled === StyledFace.TWENTY_SIX ? '逗号；长按打开常用标点' : KeyAccessibilityPolicy.punctuation();
  }

  /** 长空格样式下逗号键（字母层底行和 123、#+= 层左下）的上滑或下滑：移动时判定是否滑过门槛、是否已经归长按，抬起时是一次滑动就输入句号，这次点按随之被吞掉，见 `CommaSwipeGesture`。 */
  private commaSwipeTouch(event: TouchEvent): void {
    if (event.changedTouches.length === 0) {
      return;
    }
    const touch: TouchObject = event.changedTouches[0];
    if (event.type === TouchType.Down) {
      this.commaSwipe.down(touch.windowX, touch.windowY, Date.now());
    } else if (event.type === TouchType.Move) {
      this.commaSwipe.move(touch.windowX, touch.windowY, Date.now());
    } else if (event.type === TouchType.Up) {
      if (!this.commaSwipeEnabled()) {
        this.commaSwipe.cancel();
        return;
      }
      if (this.commaSwipe.up(touch.windowX, touch.windowY, Date.now())) {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(','));
        KeyboardSession.shared.punctuation(0x2e);
      }
    } else if (event.type === TouchType.Cancel) {
      this.commaSwipe.cancel();
    }
  }

  /** 这一刻画着的逗号接不接滑动：字母层看底行样式，123、#+= 层左下的逗号只在长空格样式下出现。 */
  private commaSwipeEnabled(): boolean {
    return this.symbols ? this.longSpaceActive() : this.designCommaSwipes(this.designRowStyledFace());
  }

  /**
   * 数字行的数字，与 Android 数字行相同：没有组字时和 123 层的数字一样输入；组字中先交给引擎，当前页有对应候选的 1–9 由引擎选词，引擎不收（0，或没有对应候选，或韩语、越南语等引擎自己先上屏了组字）时先结束还在的组合再上屏这个数字（`KeyboardSession.pressElseCommitThenType`）。按下记 `Digit0`–`Digit9`。
   */
  private typeNumberRowDigit(digit: string): void {
    if (!KeyboardSession.shared.composing()) {
      this.tapSymbol(digit);
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(digit));
    KeyboardSession.shared.pressElseCommitThenType(digit.charCodeAt(0));
  }

  /**
   * 设计稿的底行，按宽度份额分配：123 1.25 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9，即 Android 的 `KeyboardActionRow.designEntries`。中/英键放在回车左边，与经典底行的位置一致。26 键一族的字母层改为 123 1.25 | 符 1.05 | space 4 | ， 1 | 中 1.05 | return 1.9（`designCommaSecondaryPeriod`）；英文时空格左边再加句点 1，空格让到 3；14 键、大千注音保留逗号句号。中文 26 键和 14/18 键按各自的底行样式排（`designRowStyledFace`），键序与份额都在 `TouchLayoutStylePolicy.designRow`。
   *
   * 键序由 `TouchLayoutStylePolicy.designRow` 给出，这里用 ForEach 按它逐个画键（`designRowItems`、`designRowKey`），每种样式都是 123，符 或 😀，空格左边的标点，空格，空格右边的标点，中/英，回车这一模板的子集。
   *
   * 功能键文字 15fp、字重 500。26 键逗号长按输入句号，其他布局保留长按常用标点列表（`TouchLayoutStylePolicy.commaHold`）；长空格样式的逗号上滑或下滑输入句号。回车键始终使用操作色。
   */
  @Builder
  designActionRow() {
    Row({ space: this.keySpacing() }) {
      if (this.splitDrawn() && this.designRowSpaceIndex() >= 0) {
        // 分离式键盘：空格拆成两个，左边那个的份额让空隙落在整行中间（`SplitKeyboardPolicy.leftSpaceWeight`），两边的键照原样排。
        ForEach(this.designRowItems().slice(0, this.designRowSpaceIndex()), (item: DesignRowItem) => {
          this.designRowKey(item)
        }, (item: DesignRowItem): string => TouchLayoutStylePolicy.designRowItemKey(item))
        this.spaceKey(true, this.designRowSplitSpace().left, 0);
        this.splitGap(this.designRowSplitSpace().gap)
        this.spaceKey(true, this.designRowSplitSpace().right, 0);
        ForEach(this.designRowItems().slice(this.designRowSpaceIndex() + 1), (item: DesignRowItem) => {
          this.designRowKey(item)
        }, (item: DesignRowItem): string => TouchLayoutStylePolicy.designRowItemKey(item))
      } else {
        ForEach(this.designRowItems(), (item: DesignRowItem) => {
          this.designRowKey(item)
        }, (item: DesignRowItem): string => TouchLayoutStylePolicy.designRowItemKey(item))
      }
    }
    .width('100%')
    .height(this.rowHeight(3))
  }

  /** 设计底行里空格的下标，没有空格时为 -1（那时不拆）。 */
  private designRowSpaceIndex(): number {
    return this.designRowItems().findIndex((item: DesignRowItem): boolean => item.slot === DesignRowSlot.SPACE);
  }

  /** 分离式键盘里设计底行拆开的两个空格和中间空隙。 */
  private designRowSplitSpace(): SplitSpace {
    return this.splitSpace(this.designRowItems().map((item: DesignRowItem): number => item.share),
      this.designRowSpaceIndex());
  }

  /** 设计底行上的一个键（`designRowItems` 的一项）：份额和让位按值收下，它们在 ForEach 的键里，变了这个键整个重建；其余内容都在属性里直接读状态。 */
  @Builder
  designRowKey(item: DesignRowItem) {
    if (item.slot === DesignRowSlot.LAYER) {
      // 长按 123 直接打开符号面板，不必再经过 #+= 层；符号层里这个键是返回，长按不做事。注音（鸿蒙只有大千一种注音键面）的 #+= 是共享符号行（`designLayerShowing`），那一页没有自己的切换键，这一格印 123、点按回到数字九宫格，与设计稿 #+= 层的 123 相同（`NumberSymbolLayout.layerKeyFace`）。
      Column() {
        this.fixedKey({
          face: NumberSymbolLayout.layerKeyFace(this.symbols, this.moreSymbols),
          label: KeyAccessibilityPolicy.layoutToggle(this.symbols && !this.moreSymbols),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: '100%',
          action: () => {
            if (this.moreSymbols) {
              this.toggleMoreSymbols();
              return;
            }
            this.toggleSymbolLayer();
          }
        });
      }
      .layoutWeight(item.share)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => this.openSymbolsFromLayerKey()))
    } else if (item.slot === DesignRowSlot.SYMBOL_PANEL) {
      // 符号入口位于 123 之后，沿用面板动作与按键统计。
      this.fixedKey({
        face: '符',
        label: KeyAccessibilityPolicy.symbolPanel(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: item.share,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
          this.openSymbolSurface();
        }
      });
    } else if (item.slot === DesignRowSlot.EMOJI) {
      // 表情键样式：😀 占 符（26 键）或逗号（14/18 键）原来的位置和宽度，打开表情面板；符号面板仍可长按 123 进入。
      this.emojiKey(item.share);
    } else if (item.slot === DesignRowSlot.COMMA) {
      this.designCommaKey(item.share)
    } else if (item.slot === DesignRowSlot.PERIOD) {
      // 英文 26 键的句点紧挨空格左边，逗号在空格右边；其余底行的句号在空格右边。
      this.designPeriodKey(item.share)
    } else if (item.slot === DesignRowSlot.SPACE) {
      this.spaceKey(true, item.share, 0);
    } else if (item.slot === DesignRowSlot.LANGUAGE) {
      this.fixedKey({
        face: this.languageFace(),
        label: KeyAccessibilityPolicy.language(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: item.share,
        hitRegion: item.besideReturn ? this.languageKeyHitRegion() : undefined,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LANGUAGE);
          this.toggleTouchLanguage();
        }
      });
    } else if (item.slot === DesignRowSlot.RETURN) {
      this.designReturnKey(item.share, item.besideReturn);
    }
  }

  /** 设计底行的逗号：点按输入逗号；长按按 `TouchLayoutStylePolicy.commaHold` 输入句号或打开常用标点列表；长空格样式下上滑或下滑输入句号（`commaSwipe`），已经滑出去时长按不再做事。 */
  @Builder
  designCommaKey(share: number) {
    Column() {
      this.fixedKey({
        face: this.chinesePunctuationFaces() ? '，' : ',',
        label: this.designCommaLabel(this.designRowStyledFace()),
        size: TOUCH_LETTER_FONT_SIZE,
        emphasized: false,
        width: '100%',
        action: () => {
          if (this.commaSwipe.swallows()) {
            return;
          }
          this.typeComma();
        }
      });
    }
    .layoutWeight(share)
    .height('100%')
    .onTouch((event: TouchEvent): void => this.commaSwipeTouch(event))
    .gesture(LongPressGesture().onAction(() => {
      if (!this.commaSwipeEnabled()) {
        this.commaSwipe.cancel();
      } else if (!this.commaSwipe.longPress()) {
        // 长空格样式下这次触摸已经滑出去了：长按不再做事，抬起时输入句号。
        return;
      }
      if (this.designCommaHold(this.designRowStyledFace()) === CommaHold.PERIOD) {
        // 统计实际按下的逗号键，长按识别后不再触发普通点按。
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(','));
        KeyboardSession.shared.punctuation(0x2e);
        return;
      }
      this.managing = -1;
      this.brackets = false;
      this.punctuation = true;
    }))
  }

  /** 设计底行的句点或句号键，点按输入，没有长按。 */
  @Builder
  designPeriodKey(share: number) {
    this.fixedKey({
      face: this.chinesePunctuationFaces() ? '。' : '.',
      label: KeyAccessibilityPolicy.period(),
      size: TOUCH_LETTER_FONT_SIZE,
      emphasized: false,
      width: 0,
      share: share,
      action: () => this.typePeriod()
    });
  }

  /** 底行样式的 😀 键：与 123 层左下的表情键同一图标和动作，按下记 `SoftEmoji`。 */
  @Builder
  emojiKey(share: number) {
    this.fixedKey({
      face: '😀',
      label: KeyAccessibilityPolicy.emoji(),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      emphasized: false,
      special: true,
      width: 0,
      share: share,
      icon: KeyboardIconPaths.TOOLBAR_EMOJI,
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_EMOJI);
        this.openEmojiSurface();
      }
    });
  }

  /**
   * 触屏手写的底行：123 1.25 | ， 1 | space 2.4 | 。 1 | 中 1.05 | 重写 1.25 | ⌫ 1.25 | return 1.6，即 Android 的 `KeyboardActionRow.designEntries(HANDWRITING_LAYOUT)` 和 iOS 的手写底行。书写区铺满整行，原来它右侧的「重写」和 ⌫ 排在中/英与回车之间；原来左侧的 ？ ！ 由 ， 和 。 的长按输入，两键都画角标。
   *
   * ⌫ 有笔迹时撤回一笔、没有时删除光标前的字符（`deleteHandwriting`），紧挨回车，所以回车不让出键帽（`besideLanguage` 为假）。
   *
   * 「中/英键放在空格左边」（`languageKeyLeft`）时 中/英 排到 ， 左边：123 | 中 | ， | 空格 | 。 | 重写 | ⌫ | 回车，与设计底行同一条规则（`TouchLayoutStylePolicy.languageBeforeSpace`），份额不变。右边时那一格照原样写在 。 后面，`scripts/test-mobile-handwriting-pad-parity.py` 按它读三端的默认键序；左边时画 `handwritingLeftLanguageKey`。
   */
  @Builder
  handwritingActionRow() {
    Row({ space: this.keySpacing() }) {
      Column() {
        this.fixedKey({
          face: '123',
          label: KeyAccessibilityPolicy.layoutToggle(false),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: '100%',
          action: () => this.toggleSymbolLayer()
        });
      }
      .layoutWeight(HANDWRITING_LAYER_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => this.openSymbolsFromLayerKey()))
      if (this.languageKeyLeft) {
        this.handwritingLeftLanguageKey()
      }
      Column() {
        this.fixedKey({
          face: this.chinesePunctuationFaces() ? '，' : ',',
          label: '逗号；长按输入问号',
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: '100%',
          cornerHint: this.chinesePunctuationFaces() ? '？' : '?',
          action: () => this.typeComma()
        });
      }
      .layoutWeight(HANDWRITING_PUNCTUATION_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => {
        // 统计实际按下的逗号键，长按识别后不再触发普通点按。
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(','));
        KeyboardSession.shared.punctuation(0x3f);
      }))
      this.spaceKey(true, HANDWRITING_SPACE_SHARE, 0);
      Column() {
        this.fixedKey({
          face: this.chinesePunctuationFaces() ? '。' : '.',
          label: '句号；长按输入叹号',
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: '100%',
          cornerHint: this.chinesePunctuationFaces() ? '！' : '!',
          action: () => this.typePeriod()
        });
      }
      .layoutWeight(HANDWRITING_PUNCTUATION_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character('.'));
        KeyboardSession.shared.punctuation(0x21);
      }))
      if (!this.languageKeyLeft) {
        this.fixedKey({
          face: this.languageFace(),
          label: KeyAccessibilityPolicy.language(),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: 0,
          share: HANDWRITING_LANGUAGE_SHARE,
          action: () => {
            KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LANGUAGE);
            this.toggleTouchLanguage();
          }
        });
      }
      this.fixedKey({
        face: '重写',
        label: '清空手写',
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: HANDWRITING_REWRITE_SHARE,
        action: () => this.clearHandwriting()
      });
      this.fixedKey({
        face: '⌫',
        label: KeyAccessibilityPolicy.delete(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        emphasized: false,
        special: true,
        width: 0,
        share: HANDWRITING_DELETE_SHARE,
        icon: KeyboardIconPaths.BACKSPACE,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.BACKSPACE);
          this.deleteHandwriting();
        }
      });
      this.designReturnKey(HANDWRITING_RETURN_SHARE, false);
    }
    .width('100%')
    .height(this.rowHeight(3))
  }

  /** 「中/英键放在空格左边」时手写底行排在 123 和 ， 之间的 中/英，与右边时 。 后面那一格同一个键：同样的份额、点按、读屏文字和统计 id（`handwritingActionRow`）。 */
  @Builder
  handwritingLeftLanguageKey() {
    this.fixedKey({
      face: this.languageFace(),
      label: KeyAccessibilityPolicy.language(),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      special: true,
      width: 0,
      share: HANDWRITING_LANGUAGE_SHARE,
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LANGUAGE);
        this.toggleTouchLanguage();
      }
    });
  }

  /** 触屏键盘的回车键，无论显示什么都用操作色：回车箭头、编辑框的动作词，或有组合时的「确认」。`besideLanguage` 为真时它左边紧挨中/英键，两者的触摸分界挪进回车键帽（`returnKeyHitRegion`）。 */
  @Builder
  designReturnKey(share: number, besideLanguage: boolean) {
    this.fixedKey({
      face: this.returnFace(),
      label: KeyAccessibilityPolicy.returnKey(this.returnFace()),
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: this.returnComposing() ? COMPOSING_RETURN_WEIGHT : FUNCTION_LABEL_WEIGHT,
      emphasized: true,
      width: 0,
      share: share,
      icon: this.returnUsesIcon() ? KeyboardIconPaths.RETURN : '',
      hitRegion: besideLanguage ? this.returnKeyHitRegion() : undefined,
      action: () => this.activateReturn()
    });
  }

  @Builder
  classicActionRow() {
    Row({ space: this.keySpacing() }) {
      this.fixedKey({
        face: this.symbols ? '返回' : '123',
        label: KeyAccessibilityPolicy.layoutToggle(this.symbols),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        emphasized: false,
        special: true,
        width: KeyboardMetrics.LAYOUT_TOGGLE_WIDTH_VP,
        action: () => this.toggleSymbolLayer()
      });
      if (this.gridActionRow()) {
        this.fixedKey({
          face: '符',
          label: KeyAccessibilityPolicy.symbolPanel(),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          emphasized: false,
          special: true,
          width: KeyboardMetrics.MODIFIER_WIDTH_VP,
          action: () => {
            KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
            this.openSymbolSurface();
          }
        });
      } else {
        Column() {
          this.fixedKey({
            face: this.koreanFace() || this.vietnameseFace() || this.tibetanFace() ? ',' : '，',
            label: KeyAccessibilityPolicy.punctuation(),
            size: KeyboardMetrics.KEY_FONT_SIZE,
            emphasized: false,
            width: KeyboardMetrics.MODIFIER_WIDTH_VP,
            action: () => this.typeComma()
          });
        }
        .width(KeyboardMetrics.MODIFIER_WIDTH_VP)
        .height('100%')
        .gesture(LongPressGesture().onAction(() => {
          this.managing = -1;
          this.brackets = false;
          this.punctuation = true;
        }))
      }
      this.spaceKey(false, 1, 0);
      this.fixedKey({
        // The face is the mode it is in, not the pair it toggles between: a key labelled 中/英
        // says nothing about which one is running.
        face: this.english ? 'En' : (this.koreanFace() ? '한'
          : (this.vietnameseFace() ? '越' : (this.tibetanFace() ? '藏' : '中'))),
        // Never the mode it is in: a label reading 中 would be the character, not the action.
        label: KeyAccessibilityPolicy.language(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        emphasized: false,
        special: true,
        width: KeyboardMetrics.LANGUAGE_WIDTH_VP,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LANGUAGE);
          this.toggleTouchLanguage();
        }
      });
      this.fixedKey({
        // A composition takes Return before any editor action does: the user is confirming what is being spelled, not submitting the field, and the key turns the platform accent to say so. Otherwise the key says what the field asked for, so it reads 搜索 in a search box rather than 换行 in a box that has no lines, and sits with the other function keys.
        face: this.returnFace(),
        // The face is already a whole word here — 搜索, 换行, 确认 — so the spoken name is the same word rather than a second description of it.
        label: KeyAccessibilityPolicy.returnKey(this.returnFace()),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        emphasized: this.returnComposing(),
        special: !this.returnComposing(),
        width: KeyboardMetrics.RETURN_WIDTH_VP,
        action: () => this.activateReturn()
      });
    }
    .width('100%')
    .height(this.rowHeight(3))
  }

  /** Whether Return currently confirms a composition rather than acting on the field. */
  private returnComposing(): boolean {
    return this.japanese ? this.editing.length > 0 : this.composingCandidates() > 0;
  }

  /** 回车要确认的候选数：上屏后联想的候选不算，联想时回车照常换行或执行编辑器动作。空格不同，联想时照样选高亮的一条（运行时接管），读屏名称仍是「选定」。 */
  private composingCandidates(): number {
    return NextWordPredictionPolicy.composingCandidates(this.candidates.length, this.prediction);
  }

  private returnFace(): string {
    if (this.japanese) {
      return JapaneseNineKeyActions.returnTitle(this.editing.length > 0);
    }
    return this.composingCandidates() > 0
      ? '确认' : ReturnKeyAction.title(KeyboardSession.shared.returnAction(), false);
  }

  /**
   * Delete is the one key that cares about the whole gesture rather than the tap, so it is built
   * separately: the press starts the repeat and any end of the touch stops it, including one that
   * ends with the finger somewhere else.
   */
  @Builder
  deleteKey(width: Length, intoGaps: boolean, share: number) {
    Column() {
      if (this.touchDesign()) {
        KeyboardLineIcon({
          d: KeyboardIconPaths.BACKSPACE,
          iconSize: KEY_ICON_VP,
          stroke: KeyboardIconPaths.KEY_STROKE,
          color: this.skin.keyLabelColor(false, true)
        })
      } else {
        Text('⌫')
          .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
          .fontColor(this.skin.keyLabelColor(false, true))
          .textAlign(TextAlign.Center)
          .width('100%')
      }
    }
    .width(width)
    // 在触屏键盘的字母行上占该行的份额；为 0 时由宽度决定。
    .layoutWeight(share)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .accessibilityText(KeyAccessibilityPolicy.delete())
    .backgroundColor(this.skin.keySurfaceBackground(false, true))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    // 表情面板标题栏里的删除键不在键盘的键距里，触摸区不扩。
    .responseRegion(intoGaps ? this.keyHitRegion() : this.hitRegion(0, 0, 0, 0))
    .onTouch((event: TouchEvent): void => {
      if (event.type === TouchType.Down) {
        this.startBackspace();
      } else if (event.type === TouchType.Up) {
        this.finishBackspace();
      } else if (event.type === TouchType.Cancel) {
        this.cancelBackspace();
      }
    })
  }

  /**
   * Space, which also moves the cursor when the finger slides along it.
   *
   * The accumulator is the ported SpaceCursorMovement: it carries the leftover distance between
   * events so a slow drag still moves, and abandons the gesture outright on an implausible jump
   * rather than clamping it, because a jump that large means the gesture was already lost. A drag
   * that moved the cursor must not also type a space, which is what dragged tracks.
   *
   * 触屏上按住 `SPACE_LONG_PRESS_MS` 不动即按住说话，与拖动移光标的先后判定在 `SpaceVoiceGesture`：松手结束录音，上滑进入「松开取消」后松手放弃。
   */
  @Builder
  spaceKey(design: boolean, share: number, width: Length) {
    Column() {
      if (design) {
        // 设计稿的空格键：开启语音输入时先是麦克风，然后是方案简称（Android 的 `SpaceKeyFace`）。
        Row({ space: SPACE_LABEL_GAP_VP }) {
          if (this.voiceEnabled) {
            KeyboardLineIcon({
              d: KeyboardIconPaths.MIC,
              iconSize: KEY_ICON_VP,
              stroke: KeyboardIconPaths.KEY_STROKE,
              color: this.skin.secondary
            })
          }
          Text(this.spaceVoiceHint.length > 0 ? this.spaceVoiceHint : this.spaceLabel())
            .fontSize(SPACE_LABEL_FONT_SIZE)
            .fontColor(this.skin.secondary)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .flexShrink(1)
        }
        .width('100%')
        .justifyContent(FlexAlign.Center)
        .padding({ left: 4, right: 4 })
      } else {
        Text(this.spaceVoiceHint.length > 0 ? this.spaceVoiceHint : (this.japanese
          ? JapaneseNineKeyActions.spaceTitle(this.editing.length > 0)
          : (this.english ? 'space' : '空格')))
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.secondary)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
    }
    .width(width)
    // 权重会覆盖沿行方向的宽度；为 0 时由宽度决定。
    .layoutWeight(share)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    // What it will do, not what it is: mid-composition this key selects the highlighted candidate,
    // and a user who cannot see the candidate row has no other way to know it changed meaning.
    .accessibilityText(KeyAccessibilityPolicy.space(this.candidates.length > 0))
    .backgroundColor(this.skin.keySurfaceBackground(false))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onTouch((event: TouchEvent): void => {
      const position: number = event.touches[0].windowX;
      if (event.type === TouchType.Down) {
        this.cursorDrag.begin(position, this);
        this.dragged = false;
        const armed: boolean = SpaceVoiceGesture.armed(this.touchDesign(), this.voiceEnabled, this.voiceBusy,
          this.editing.length > 0, this.localMode);
        this.spaceDownAt = Date.now();
        this.spaceGesture.down(this.spaceDownAt, position, event.touches[0].windowY, this.surfaceHeight(), armed);
        this.cancelSpaceLongPress();
        if (armed) {
          this.spaceLongPressTimer = setTimeout((): void => {
            this.spaceLongPressTimer = -1;
            // 计时器不会早于设定时长触发，取较大者只是防止两次读钟的毫秒取整让判定差一毫秒而错过。
            if (this.spaceGesture.tick(Math.max(Date.now(), this.spaceDownAt + SPACE_LONG_PRESS_MS))) {
              this.startSpaceVoice();
            }
          }, SPACE_LONG_PRESS_MS);
        }
      } else if (event.type === TouchType.Move) {
        const before: SpaceGestureState = this.spaceGesture.state();
        this.spaceGesture.move(Date.now(), position, event.touches[0].windowY);
        if (this.spaceGesture.state() === SpaceGestureState.VOICE) {
          if (before !== SpaceGestureState.VOICE) {
            this.startSpaceVoice();
          }
          this.showSpaceVoiceHint();
          return;
        }
        if (this.spaceGesture.state() === SpaceGestureState.CURSOR) {
          this.cancelSpaceLongPress();
        }
        const steps: number = this.cursorDrag.advance(position, this, CURSOR_STEP_VP);
        for (let step: number = 0; step < Math.abs(steps); step++) {
          if (steps < 0) {
            KeyboardSession.shared.moveLeft();
          } else {
            KeyboardSession.shared.moveRight();
          }
          this.dragged = true;
        }
      } else {
        this.cancelSpaceLongPress();
        this.cursorDrag.cancel();
        const outcome: SpaceGestureOutcome = event.type === TouchType.Up
          ? this.spaceGesture.up() : this.spaceGesture.cancel();
        if (outcome === SpaceGestureOutcome.VOICE_FINISH || outcome === SpaceGestureOutcome.VOICE_CANCEL) {
          this.endSpaceVoice(outcome === SpaceGestureOutcome.VOICE_FINISH);
        } else if (!this.dragged && event.type === TouchType.Up) {
          this.activateSpace();
        }
      }
    })
  }

  private cancelSpaceLongPress(): void {
    if (this.spaceLongPressTimer >= 0) {
      clearTimeout(this.spaceLongPressTimer);
      this.spaceLongPressTimer = -1;
    }
  }

  /** 空格长按进入语音：手指还按着，录音按「按住」处理，说话停顿不会结束它，松手才结束。 */
  private startSpaceVoice(): void {
    this.cancelSpaceLongPress();
    this.cursorDrag.cancel();
    this.startVoice(true);
    this.spaceVoiceHeld = this.voiceBusy;
    this.showSpaceVoiceHint();
  }

  private showSpaceVoiceHint(): void {
    this.spaceVoiceHint = this.spaceGesture.cancelling() ? '松开取消' : '松开结束';
  }

  /**
   * 按住空格说话的手指离开：`finish` 为 true 时结束录音并打开语音界面看结果（识别结果在那里直接上屏，与从工具栏开始的录音相同）；为 false 时放弃这次录音。录音没能开始时，结束仍打开语音界面，让用户看到原因。按住期间不切换界面，是为了让空格键一直在，松手事件才送得到这里。
   */
  private endSpaceVoice(finish: boolean): void {
    const held: boolean = this.spaceVoiceHeld;
    this.spaceVoiceHeld = false;
    this.spaceVoiceHint = '';
    // 与 Android 一样，长按语音那一下仍按空格键计数。
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SPACE);
    if (!finish) {
      if (held) {
        this.cancelVoice();
      }
      return;
    }
    this.show(SURFACE_VOICE);
    if (held) {
      this.stopVoice();
    }
  }

  private clearClipboard(): void {
    // Clearing takes every pinned entry with it, which is exactly what a pin was meant to prevent, so it is worth one confirmation.
    if (!this.confirmingClear) {
      this.confirmingClear = true;
      this.clipboardNotice = '再次点按将删除全部历史，包括已固定的。';
      return;
    }
    this.applyClipboardMutation(KeyboardSession.shared.clearClipboardHistory());
    this.confirmingClear = false;
  }

  /** 2in1 上候选管理条就是 Windows 画的那种候选右键菜单，明暗跟随颜色模式；其他条，以及手机上的每一条，都属于键盘。 */
  private overlayIsMenu(): boolean {
    return KeyboardFormFactor.isDesktop() && this.managing >= 0 && !this.punctuation && !this.brackets
      && this.expandedGloss < 0;
  }

  /** A menu over the candidate window wears the candidate palette, as the theme contract derives every menu from it: surface, text and accent, with the border as its outline. */
  private overlayBackground(): string {
    return this.overlayIsMenu() ? this.menuColors.surface : this.skin.keyBackground;
  }

  private overlayForeground(): string {
    return this.overlayIsMenu() ? this.menuColors.text : this.skin.keyForeground;
  }

  private overlayAccent(): string {
    return this.overlayIsMenu() ? this.menuColors.accent : this.skin.accent;
  }

  /** The phone draws clipboard history on the keyboard's own skin; a 2in1 draws it inside the emoji surface, on that surface's skin. */
  private clipboardSkin(): KeyboardSkin {
    return KeyboardFormFactor.isDesktop() ? this.emojiSkin : this.skin;
  }

  /**
   * 打开手机的剪贴板面板。本机剪贴板历史在设置中关闭时也照常打开，在「本机」一栏说明已关闭，「云端」一栏照常可用，与 Android 的 `renderClipboardHistory` 和 2in1 表情面板里的剪贴板页一致。以前这里拒绝打开面板，改在工具条上显示一句诊断：工具栏整个换成候选栏的空壳，提示挤在拼音预编辑那一行，几秒后才恢复；而且本机历史一关，云剪贴板也跟着打不开。
   */
  private openClipboard(): void {
    this.show(SURFACE_CLIPBOARD);
    this.clips = this.clipboardEnabled ? KeyboardSession.shared.clipboardHistory() : [];
    this.clipboardNotice = this.clipboardEnabled ? '' : CLIPBOARD_HISTORY_OFF;
    this.confirmingClear = false;
    this.endClipSelection();
    this.openCloudClipboard();
  }

  /** 进入多选：长按管理行和清空的确认先收起。只在本机历史开着、有记录时可进，与 Android 的「多选」入口一致。 */
  private beginClipSelection(): void {
    if (!this.clipboardEnabled || this.clips.length === 0 || this.cloudTabActive()) {
      return;
    }
    this.clipSendTarget = '';
    this.confirmingClear = false;
    this.clipSelection = [];
    this.clipSelecting = true;
    this.clipboardNotice = ClipboardSelectionPolicy.notice(0);
  }

  private endClipSelection(): void {
    if (!this.clipSelecting) {
      return;
    }
    this.clipSelecting = false;
    this.clipSelection = [];
    this.clipboardNotice = '';
  }

  /** 本机记录在多选里的序号（从 0 起），没选中时为 -1。 */
  private clipOrdinal(text: string): number {
    return ClipboardSelectionPolicy.ordinal(this.clipSelection, text);
  }

  /** 多选时点一张卡片：按先后选中或取消；到了 20 项上限时说明原因。 */
  private toggleClipSelection(text: string): void {
    const next: string[] = ClipboardSelectionPolicy.toggled(this.clipSelection, text);
    this.clipboardNotice = next === this.clipSelection
      ? `最多只能选择 ${ClipboardSelectionPolicy.MAX_SELECTIONS} 项` : ClipboardSelectionPolicy.notice(next.length);
    this.clipSelection = next;
  }

  /** 「插入」：按序号把选中的几条首尾相接，经和单条插入同一个 `KeyboardSession.insert` 一次上屏，然后退出多选。 */
  private insertClipSelection(): void {
    if (!this.clipSelecting || !ClipboardSelectionPolicy.canInsert(this.clipSelection.length)) {
      return;
    }
    const text: string = ClipboardSelectionPolicy.insertionText(this.clipSelection);
    this.endClipSelection();
    KeyboardSession.shared.insert(text);
  }

  /** Whether the clipboard surface is on screen, as the phone's own face or as the 2in1 emoji surface's clipboard tab. */
  private clipboardShowing(): boolean {
    return this.surface === SURFACE_CLIPBOARD
      || (this.desktop === DesktopSurface.EMOJI && this.emojiPanel === CLIPBOARD_PANEL);
  }

  private cloudTabActive(): boolean {
    return this.clipboardTab === CLIPBOARD_TAB_CLOUD && this.cloudVisible;
  }

  /**
   * Fetches the cloud clipboard as the panel opens.
   *
   * This and the refresh button are the only times the keyboard asks: there is no polling, nothing is uploaded on copy, and a signed-out keyboard answers from the session file without a request.
   */
  private openCloudClipboard(): void {
    this.invalidateCloudSend();
    this.clipSendTarget = '';
    this.cloudVisible = KeyboardSession.shared.cloudClipboardVisible();
    this.cloudEditor = KeyboardSession.shared.cloudClipboardEditor();
    if (!this.cloudVisible) {
      this.clipboardTab = CLIPBOARD_TAB_LOCAL;
      this.dropCloudClipboard();
      return;
    }
    this.refreshCloudClipboard();
  }

  private refreshCloudClipboard(): void {
    if (!this.cloudVisible) {
      return;
    }
    const request: number = ++this.cloudRequest;
    const previousBinding: string = this.cloudBinding;
    this.cloudEditor = KeyboardSession.shared.cloudClipboardEditor();
    this.cloudBinding = '';
    this.cloudClips = [];
    this.cloudStateKnown = false;
    this.cloudLoading = true;
    KeyboardSession.shared.cloudClipboardItems().then((listing: CloudClipboardBoundListing | null): void => {
      if (request !== this.cloudRequest) {
        return;
      }
      this.cloudLoading = false;
      if (listing === null) {
        // The editor or login changed while the request was out; never show its rows here.
        this.cloudClips = [];
        this.cloudState = CloudClipboardState.FAILED;
        this.cloudStateKnown = this.cloudEditor === KeyboardSession.shared.cloudClipboardEditor()
          && this.cloudVisible;
        if (previousBinding.length > 0) {
          this.closeCloudClipboard();
        }
        return;
      }
      if (previousBinding.length > 0 && listing.binding !== previousBinding) {
        this.closeCloudClipboard();
        return;
      }
      this.cloudClips = listing.items;
      this.cloudBinding = listing.binding ?? '';
      this.cloudState = listing.state;
      this.cloudStateKnown = true;
    });
  }

  /** Forgets the listing on screen and any answer still on its way. */
  private dropCloudClipboard(): void {
    this.cloudRequest++;
    this.invalidateCloudSend();
    this.cloudBinding = '';
    this.cloudClips = [];
    this.cloudStateKnown = false;
    this.cloudLoading = false;
  }

  /** 账号会话或编辑框绑定失效时关闭整个云剪贴板面板，避免把失效状态留在可操作界面里。 */
  private closeCloudClipboard(): void {
    this.clipSendTarget = '';
    this.clipboardTab = CLIPBOARD_TAB_LOCAL;
    this.dropCloudClipboard();
    if (KeyboardFormFactor.isDesktop() && this.desktop === DesktopSurface.EMOJI
      && this.emojiPanel === CLIPBOARD_PANEL) {
      // Reopening the desktop emoji surface must start on the ordinary emoji page rather than
      // immediately reopening the cloud page whose binding was just rejected.
      this.emojiPanel = EMOJI_PANEL;
      this.resetEmojiSearch();
    }
    this.leaveSurface(DesktopSurface.EMOJI);
  }

  /**
   * Follows a focus change into the clipboard surface.
   *
   * The editor callback also fires for every text edit, so only a different field, or one whose attributes have just said it is a password field, drops the listing. A listing fetched for the previous field is not this one's, and the cloud section disappears outright from a field that may not see it.
   */
  private syncCloudClipboardEditor(): void {
    const visible: boolean = KeyboardSession.shared.cloudClipboardVisible();
    const editor: number = KeyboardSession.shared.cloudClipboardEditor();
    if (visible === this.cloudVisible && editor === this.cloudEditor) {
      return;
    }
    this.cloudVisible = visible;
    this.cloudEditor = editor;
    this.clipSendTarget = '';
    this.dropCloudClipboard();
    if (!visible) {
      this.clipboardTab = CLIPBOARD_TAB_LOCAL;
      return;
    }
    if (this.clipboardShowing()) {
      this.refreshCloudClipboard();
    }
  }

  private selectClipboardTab(tab: string): void {
    if (tab === this.clipboardTab) {
      return;
    }
    this.invalidateCloudSend();
    this.clipboardTab = tab;
    this.clipSendTarget = '';
    this.confirmingClear = false;
    this.emojiSelected = 0;
    if (tab === CLIPBOARD_TAB_CLOUD && !this.cloudStateKnown && !this.cloudLoading) {
      this.refreshCloudClipboard();
    }
  }

  /** The cloud items the 2in1 search leaves, matched the way local entries are. */
  private visibleCloudClips(): CloudClipboardItem[] {
    if (this.emojiQuery.length === 0) {
      return this.cloudClips;
    }
    return this.cloudClips.filter((entry: CloudClipboardItem): boolean =>
      EmojiPanelKeyPolicy.matches(entry.text, entry.text, this.emojiQuery));
  }

  private cloudNotice(): string {
    return CloudClipboardPolicy.notice(this.cloudStateKnown ? this.cloudState : null, this.cloudLoading,
      this.cloudClips.length);
  }

  private insertCloudClip(text: string): void {
    const request: number = this.cloudRequest;
    const editor: number = this.cloudEditor;
    const binding: string = this.cloudBinding;
    KeyboardSession.shared.cloudClipboardBindingCurrent(binding).then((current: boolean): void => {
      if (request !== this.cloudRequest || !this.clipboardShowing() || binding !== this.cloudBinding) return;
      if (CloudClipboardPolicy.canUseRows(this.cloudVisible,
        this.cloudStateKnown ? this.cloudState : null, binding, current ? binding : '')
        && KeyboardSession.shared.insertCloudClipboard(text, editor)) return;
      // The editor or account changed while the row was visible; close the old listing instead of
      // leaving a stale panel that invites another tap.
      this.closeCloudClipboard();
    });
  }

  /** A long press on a local entry offers it to the cloud clipboard; nothing is sent until the user taps the action. */
  private offerCloudSend(text: string): void {
    if (!this.cloudVisible) {
      return;
    }
    this.invalidateCloudSend();
    this.clipSendTarget = text;
  }

  /** Cancels the visible send state without trying to cancel the request already on the wire. */
  private invalidateCloudSend(): void {
    this.cloudSendRequest++;
    this.cloudSending = false;
  }

  /** Why the offered entry cannot be sent right now, or null when it can. */
  private cloudSendBlock(): string | null {
    if (this.cloudLoading) {
      return CloudClipboardPolicy.sendBlock(null, this.clipSendTarget);
    }
    return CloudClipboardPolicy.sendBlock(this.cloudStateKnown ? this.cloudState : null, this.clipSendTarget);
  }

  private cloudSendHint(): string {
    const block: string | null = this.cloudSendBlock();
    return block === null ? this.clipSendTarget : block;
  }

  private sendClipToCloud(): void {
    const text: string = this.clipSendTarget;
    const binding: string = this.cloudBinding;
    if (this.cloudSending || text.length === 0 || binding.length === 0 || this.cloudSendBlock() !== null) {
      return;
    }
    const request: number = ++this.cloudSendRequest;
    this.cloudSending = true;
    KeyboardSession.shared.sendToCloudClipboard(text, binding).then((outcome: CloudClipboardSendOutcome): void => {
      if (!CloudClipboardPolicy.sendCurrent(request, this.cloudSendRequest)) {
        return;
      }
      this.cloudSending = false;
      if (binding !== this.cloudBinding) {
        this.closeCloudClipboard();
        return;
      }
      if (this.clipSendTarget === text) {
        this.clipSendTarget = '';
      }
      this.clipboardNotice = CloudClipboardPolicy.sendNotice(outcome);
      // Read the cloud again so its section shows the new item, or the state that refused it.
      if (this.clipboardShowing()) {
        this.refreshCloudClipboard();
      }
    });
  }

  /**
   * Saves whatever the system clipboard holds now.
   *
   * Nothing is captured in the background. An input method that quietly recorded every copy would be
   * collecting the user's text without being asked, so this happens only when the button is pressed.
   */
  private captureClipboard(): void {
    if (!this.clipboardEnabled) {
      this.clipboardNotice = CLIPBOARD_HISTORY_OFF;
      return;
    }
    KeyboardSession.shared.canReadClipboard().then((allowed: boolean): void => {
      if (!allowed) {
        this.clipboardNotice = CLIPBOARD_NOT_PERMITTED;
        return;
      }
      this.storeClipboardText();
    });
  }

  private loadReplySource(): void {
    const generation: number = ++this.replyGeneration;
    KeyboardSession.shared.systemClipboardText().then((text: string): void => {
      if (generation !== this.replyGeneration) return;
      this.replySource = text;
      this.replyResults = [];
      this.replyNotice = text.trim().length === 0 ? '剪贴板里没有文字' : '选择风格生成回复';
    });
  }

  private generateReply(style: string, extraPrompt: string = ''): void {
    if (this.replyBusy) return;
    if (this.replySource.trim().length === 0) {
      this.replyNotice = '先点粘贴，放入对方的话';
      return;
    }
    this.replyStyle = style;
    const generation: number = ++this.replyGeneration;
    this.replyBusy = true;
    this.replyResults = [];
    this.replyNotice = '正在生成…';
    KeyboardSession.shared.thoughtfulReplies(
      this.replySource, style, extraPrompt, this.replyPolish && extraPrompt.length === 0).then(
      (results: string[]): void => {
      if (generation !== this.replyGeneration) return;
      this.replyBusy = false;
      this.replyResults = results;
      this.replyNotice = results.length === 0 ? '生成失败，请检查 AI 设置或重试' : '点选回复插入输入框';
    });
  }

  private resetReplyResults(notice: string = ''): void {
    this.replyGeneration += 1;
    this.replyBusy = false;
    this.replyResults = [];
    if (notice.length > 0) this.replyNotice = notice;
    KeyboardSession.shared.invalidateReplyContext(false);
  }

  private clearReplySource(): void {
    this.resetReplyResults();
    this.replySource = '';
    this.replyNotice = '剪贴板里没有文字';
  }

  private deleteReplySourceCharacter(): void {
    if (this.replySource.length === 0) return;
    this.resetReplyResults();
    const characters: string[] = TextPolicy.characters(this.replySource);
    characters.pop();
    this.replySource = characters.join('');
    this.replyNotice = this.replySource.length === 0 ? '剪贴板里没有文字' : '选择风格生成回复';
  }

  private toggleReplyPolish(polish: boolean): void {
    if (this.replyPolish === polish) return;
    this.resetReplyResults();
    this.replyPolish = polish;
    this.replyNotice = this.replySource.length === 0
      ? '点击读取剪贴板中的对方文字' : '选择风格生成，内容仅在点击风格时发送';
  }

  @Builder
  replyFace() {
    Column({ space: this.rowSpacing() }) {
      Row() {
        Text('高情商回复').fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.skin.keyForeground).layoutWeight(1)
        Button('帮你回').enabled(!this.replyBusy).onClick(() => this.toggleReplyPolish(false))
          .backgroundColor(this.replyPolish ? this.skin.keyBackground : this.skin.accent)
          .fontColor(this.replyPolish ? this.skin.keyForeground : this.skin.keyBackground)
        Button('帮润色').enabled(!this.replyBusy).onClick(() => this.toggleReplyPolish(true))
          .backgroundColor(this.replyPolish ? this.skin.accent : this.skin.keyBackground)
          .fontColor(this.replyPolish ? this.skin.keyBackground : this.skin.keyForeground)
        if (this.replyTemplates.length > 0) {
          Text(this.replyTemplateMenu ? '风格' : '模板').fontColor(this.skin.accent)
            .onClick(() => this.replyTemplateMenu = !this.replyTemplateMenu)
        }
        Text('完成').fontColor(this.skin.accent).onClick(() => this.show(SURFACE_NONE))
      }.width('100%').height(28)
      Row({ space: this.keySpacing() }) {
        Text(this.replySource.length === 0 ? '点击读取剪贴板中的对方文字' : this.replySource)
          .fontSize(14).fontColor(this.skin.keyForeground).maxLines(2).layoutWeight(1)
        Button('删除').enabled(!this.replyBusy && this.replySource.length > 0)
          .onClick(() => this.deleteReplySourceCharacter())
        Button('清空').enabled(!this.replyBusy && this.replySource.length > 0)
          .onClick(() => this.clearReplySource())
        Button('粘贴').enabled(!this.replyBusy).onClick(() => this.loadReplySource())
      }.width('100%').padding(8).backgroundColor(this.skin.keyBackground)
      if (this.replyResults.length === 0 && this.replyTemplateMenu) {
        Scroll() {
          Column({ space: this.keySpacing() }) {
            ForEach(this.replyTemplates, (template: CommunityReplyTemplate) => {
              Button(template.name).enabled(!this.replyBusy)
                .onClick(() => {
                  this.replyTemplateMenu = false;
                  this.generateReply('自定义模板', template.prompt);
                })
                .width('100%').height(44).backgroundColor(this.skin.keyBackground)
                .fontColor(this.skin.keyForeground)
            }, (template: CommunityReplyTemplate) => template.id)
          }.width('100%')
        }.layoutWeight(1)
      } else if (this.replyResults.length === 0) {
        Grid() {
          ForEach(ReplyKeyboardPolicy.STYLES, (style: ReplyStyle) => {
            GridItem() {
              Button(style.title).enabled(!this.replyBusy)
                .onClick(() => this.generateReply(style.id))
                .width('100%').height(42).backgroundColor(this.skin.keyBackground)
                .fontColor(this.skin.keyForeground)
            }
          }, (style: ReplyStyle) => style.id)
        }.columnsTemplate('1fr 1fr 1fr').width('100%').layoutWeight(1)
      } else {
        Scroll() {
          Column({ space: this.keySpacing() }) {
            ForEach(this.replyResults, (reply: string) => {
              Button(reply).width('100%').height(48).fontColor(this.skin.keyForeground)
                .backgroundColor(this.skin.keyBackground)
                .onClick(() => {
                  if (KeyboardSession.shared.insertReply(reply)) {
                    this.replyResults = [];
                    this.replyNotice = '已插入，请确认发送';
                    this.show(SURFACE_NONE);
                  } else {
                    this.replyNotice = '输入位置已变化，请重新生成';
                  }
                })
            }, (reply: string) => reply)
          }.width('100%')
        }.layoutWeight(1)
        Row({ space: this.keySpacing() }) {
          Button('换一句').enabled(!this.replyBusy)
            .onClick(() => this.generateReply(this.replyStyle))
          Button('选风格').enabled(!this.replyBusy)
            .onClick(() => this.resetReplyResults())
        }.width('100%').height(42)
      }
      if (this.replyBusy) {
        Button('取消').onClick(() => this.resetReplyResults('已取消'))
          .width('100%').height(38)
      }
      Text(this.replyNotice).fontSize(12).fontColor(this.skin.keyForeground).opacity(0.72)
    }.width('100%').height(this.surfaceHeight())
  }

  /**
   * Opens the polish panel over the text the user has just typed.
   *
   * Read when the panel opens rather than when it was last drawn: the point of the panel is to act
   * on what is in the editor now, and a source captured earlier is the one thing here that would
   * be quietly wrong.
   */
  private openPolish(): void {
    this.polishGeneration += 1;
    this.polishResult = '';
    this.polishBusy = false;
    this.polishSource = KeyboardSession.shared.polishSource();
    this.polishNotice = this.polishSource.length === 0
      ? '先输入一段文字，再回来润色' : '发送给 AI 重写这段文字';
    this.show(SURFACE_POLISH);
  }

  private requestPolish(): void {
    if (this.polishBusy || this.polishSource.length === 0) return;
    const generation: number = ++this.polishGeneration;
    this.polishBusy = true;
    this.polishResult = '';
    this.polishNotice = '正在润色…';
    KeyboardSession.shared.polish(this.polishSource).then((result: string | null): void => {
      if (generation !== this.polishGeneration) return;
      this.polishBusy = false;
      this.polishResult = result ?? '';
      this.polishNotice = result === null
        ? '没有可用的结果，请检查语音设置里的润色服务或重试' : '确认后替换原文';
    });
  }

  @Builder
  polishFace() {
    Column({ space: this.keySpacing() }) {
      Row() {
        Text('AI 润色').fontSize(15).fontWeight(FontWeight.Medium)
          .fontColor(this.skin.keyForeground).layoutWeight(1)
        Text('完成').fontColor(this.skin.accent).onClick(() => this.show(SURFACE_NONE))
      }.width('100%').height(28)
      Scroll() {
        Column({ space: this.keySpacing() }) {
          Text(this.polishResult.length === 0 ? '待润色的文字' : '润色结果')
            .fontSize(12).fontColor(this.skin.keyForeground).opacity(0.72)
            .width('100%').textAlign(TextAlign.Start)
          Text(this.polishResult.length === 0 ? this.polishSource : this.polishResult)
            .fontSize(14).fontColor(this.skin.keyForeground).width('100%')
            .textAlign(TextAlign.Start).padding(8).backgroundColor(this.skin.keyBackground)
        }.width('100%')
      }.layoutWeight(1)
      Row({ space: this.keySpacing() }) {
        if (this.polishResult.length === 0) {
          Button('发送文字').enabled(!this.polishBusy && this.polishSource.length > 0)
            .onClick(() => this.requestPolish())
            .layoutWeight(1).height(42).backgroundColor(this.skin.keyBackground)
            .fontColor(this.skin.keyForeground)
        } else {
          // Replacing is refused rather than forced when the editor has moved: the rewrite belongs
          // to text that is no longer there, and putting it in anyway would delete something else.
          Button('替换原文').onClick(() => {
            if (KeyboardSession.shared.applyPolish(this.polishSource, this.polishResult)) {
              this.show(SURFACE_NONE);
            } else {
              this.polishResult = '';
              this.polishNotice = '输入位置已变化，请重新润色';
            }
          }).layoutWeight(1).height(42).backgroundColor(this.skin.accent)
            .fontColor(this.skin.keyBackground)
          Button('重新润色').enabled(!this.polishBusy).onClick(() => this.requestPolish())
            .layoutWeight(1).height(42).backgroundColor(this.skin.keyBackground)
            .fontColor(this.skin.keyForeground)
        }
      }.width('100%')
      Text(this.polishNotice).fontSize(12).fontColor(this.skin.keyForeground).opacity(0.72)
    }.width('100%').height(this.surfaceHeight())
  }

  /** 面板里的「开启」：写键盘自己的那份偏好。写成功后会话回调 onClipboardHistoryChanged，列表和提示在那里换上。 */
  private enableClipboardHistory(): void {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('clipboard_history', true);
    if (!KeyboardSession.shared.changePreferences(changes)) {
      this.clipboardNotice = '没能开启剪贴板历史，请稍后再试';
    }
  }

  private storeClipboardText(): void {
    if (!this.clipboardEnabled) {
      this.clipboardNotice = CLIPBOARD_HISTORY_OFF;
      return;
    }
    if (!KeyboardSession.shared.clipboardCaptureAllowed()) {
      this.clipboardNotice = PrivacyGate.CLIPBOARD_REFUSED;
      return;
    }
    KeyboardSession.shared.systemClipboardText().then((text: string): void => {
      this.applyClipboardMutation(KeyboardSession.shared.captureClipboardHistory(text));
    });
  }

  private sendKana(key: JapaneseKey, direction: number): void {
    const stroke: string = key.strokes[direction];
    if (stroke.length === 0) {
      // A label with no stroke is punctuation or a mark: it goes in as it stands.
      KeyboardSession.shared.insert(key.kana[direction]);
      return;
    }
    for (const character of stroke) {
      KeyboardSession.shared.press(character.charCodeAt(0), false);
    }
  }

  private manageCandidate(action: ManagementAction, index: number): void {
    if (action.available === false) {
      return;
    }
    if (action.confirmationRequired && this.pendingRemoval !== index) {
      // Deleting a dictionary entry is not undoable from here, so the first press only arms it.
      this.pendingRemoval = index;
      this.diagnostic = '再次选择“删除词条…”以确认';
      this.clearDiagnosticLater();
      return;
    }
    this.pendingRemoval = -1;
    this.managing = -1;
    this.touchCandidateMenu = false;
    if (action.id === 'READ_ALOUD') {
      // 和「加入生词本」一样先核对这一格还是打开菜单时的那个候选，再读打开菜单时共享层答的那段英文。
      if (index < 0 || index >= this.candidates.length || action.spoken === undefined ||
        this.candidates[index].text !== action.candidate) {
        return;
      }
      KeyboardSession.shared.readAloud(action.spoken).then((notice: string | null): void => {
        if (notice !== null) {
          this.diagnostic = notice;
          this.clearDiagnosticLater();
        }
      });
      return;
    }
    if (action.id === 'ADD_TO_PERSONAL_WORDBOOK') {
      // 和上屏释义一样先核对这一格还是打开菜单时的那个候选（原文和释义都要相同），再交出它的原文和释义；提示等共享层答了再显示。
      if (index < 0 || index >= this.candidates.length) {
        return;
      }
      const candidate: CandidateEntry = this.candidates[index];
      if (!CandidateManagementAction.samePersonalWordbookSubject(action,
        CandidateManagementAction.personalWordbookAction(candidate.annotation, candidate.annotationIsTranslation,
          candidate.text))) {
        return;
      }
      KeyboardSession.shared.addToPersonalWordbook(candidate.text, candidate.annotation.trim())
        .then((notice: string): void => {
          this.diagnostic = notice;
          this.clearDiagnosticLater();
        });
      return;
    }
    switch (action.id) {
      case 'INSERT_GLOSS': {
        // The strip can be rebuilt while the menu is open, and then this index is somebody else's
        // candidate. The source guards the same moment with a revision; here the gloss carried in
        // the action is compared with the one the candidate has now, which answers the same
        // question without a second counter to keep.
        if (index < 0 || index >= this.candidates.length) {
          return;
        }
        const candidate: CandidateEntry = this.candidates[index];
        if (!candidate.annotationIsTranslation) {
          return;
        }
        const current: ManagementAction | null =
          CandidateManagementAction.glossAction(candidate.annotation);
        if (current === null || current.announcement !== action.announcement) {
          return;
        }
        // The gloss first and the composition after, which is the order macOS uses: cancelling
        // first would let the editor see an empty composition and scroll away from the caret
        // before the text arrives.
        KeyboardSession.shared.insert(candidate.annotation.trim());
        KeyboardSession.shared.cancel();
        break;
      }
      case 'PROMOTE':
        KeyboardSession.shared.promoteCandidate(index);
        break;
      case 'FIX_FIRST':
        KeyboardSession.shared.fixCandidateFirst(index);
        break;
      case 'CLEAR_POSITION':
        KeyboardSession.shared.clearCandidatePosition(index);
        break;
      default:
        if (action.id.startsWith('FIX_') && action.position !== undefined) {
          KeyboardSession.shared.fixCandidatePosition(index, action.position);
        } else {
          KeyboardSession.shared.removeCandidate(index);
        }
        break;
    }
    this.diagnostic = action.announcement;
    this.clearDiagnosticLater();
  }

  private openTouchCandidateMenu(index: number): void {
    // 上屏后联想的候选只是建议，不能固定、调序或删除，长按什么也不做，与 Android 相同。
    if (index < 0 || index >= this.candidates.length || !KeyboardSession.shared.candidateManagementAllowed()) {
      return;
    }
    this.touchCandidateMenu = false;
    this.managing = -1;
    const candidate: CandidateEntry = this.candidates[index];
    // A 2in1 touch screen keeps the gloss alone, because its right click already opens management. A phone has no right click, so its long press is the only way to 优先显示, 固定排位 and 删除词条, and opens the whole menu the way Android's does.
    const glossOnly: boolean = KeyboardFormFactor.isDesktop();
    if (glossOnly && CandidateManagementAction.touchActions(
      candidate.annotation, candidate.annotationIsTranslation, candidate.text,
      KeyboardSession.shared.readAloudText(
        candidate.text, candidate.annotationIsTranslation ? candidate.annotation : '')).length === 0) {
      return;
    }
    this.punctuation = false;
    this.brackets = false;
    this.touchCandidateMenu = glossOnly;
    this.managing = index;
  }

  /** All three at once: they are one decision about how the keyboard is laid out. */
  private applyGeometry(keySpacing: number, rowSpacing: number, height: number): void {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('touch_key_spacing_tenths', KeyboardGeometry.keySpacing(keySpacing));
    changes.set('touch_row_spacing_tenths', KeyboardGeometry.rowSpacing(rowSpacing));
    changes.set('touch_keyboard_height_adjustment', KeyboardGeometry.heightAdjustment(height));
    if (!KeyboardSession.shared.changePreferences(changes)) {
      return;
    }
    this.settings = KeyboardSession.shared.preferences();
    this.geometryKeySpacing = this.settings.keySpacingTenths;
    this.geometryRowSpacing = this.settings.rowSpacingTenths;
    this.geometryHeight = this.settings.heightAdjustment;
    this.shuangpinKeyHints = this.settings.shuangpinKeyHints;
    this.quanpinShift = this.settings.quanpinShift;
    this.resizeTouchPanel();
  }

  private setFeedback(settings: FeedbackSettings): void {
    if (KeyboardSession.shared.saveFeedback(settings)) {
      this.feedback = settings;
    }
  }

  /** 设备本地设置，只改按键反馈，其余一切（包括滑行输入、手写板和字母键手势）保持原样。 */
  private feedbackWith(sound: boolean, haptics: boolean, strength: HapticStrength): FeedbackSettings {
    // 用会话的副本而不是视图的：会话在每次编辑框获得焦点时重读，而设置应用可能在视图上次读取文件后改过手势或手写板，这次写入不能把旧值写回去。
    const current: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    const next: FeedbackSettings = {
      sound: sound,
      haptics: haptics,
      strength: strength,
      glideTyping: current.glideTyping,
      handwritingDelayMs: current.handwritingDelayMs,
      handwritingStrokeColor: current.handwritingStrokeColor,
      handwritingStrokeWidth: current.handwritingStrokeWidth,
      keyPopup: current.keyPopup,
      swipeSymbols: current.swipeSymbols,
      swipeSymbolsDirection: current.swipeSymbolsDirection,
      oneHanded: current.oneHanded,
      incognito: current.incognito,
      floating: current.floating,
      floatingX: current.floatingX,
      floatingY: current.floatingY,
      bottomLift: current.bottomLift,
      widthPercent: current.widthPercent,
      positionX: current.positionX
    };
    return next;
  }

  /** 把「单手模式」设为 `mode`，其余保持原样。隐私模式从不在这里写入：它经由会话保存，会话会随之重建。 */
  private setOneHanded(mode: OneHandedMode): void {
    // 用会话的副本，原因见 `feedbackWith`。
    const current: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    const next: FeedbackSettings = {
      sound: current.sound,
      haptics: current.haptics,
      strength: current.strength,
      glideTyping: current.glideTyping,
      handwritingDelayMs: current.handwritingDelayMs,
      handwritingStrokeColor: current.handwritingStrokeColor,
      handwritingStrokeWidth: current.handwritingStrokeWidth,
      keyPopup: current.keyPopup,
      swipeSymbols: current.swipeSymbols,
      swipeSymbolsDirection: current.swipeSymbolsDirection,
      oneHanded: mode,
      incognito: current.incognito,
      floating: current.floating,
      floatingX: current.floatingX,
      floatingY: current.floatingY,
      bottomLift: current.bottomLift,
      widthPercent: current.widthPercent,
      positionX: current.positionX
    };
    this.setFeedback(next);
  }

  /** 通过会话切换「隐私模式」，会话负责保存并重建 Engine 会话，让学习和隐私标记随之生效。 */
  private togglePrivacyMode(): void {
    KeyboardSession.shared.setPrivacyMode(!this.feedback.incognito);
    this.feedback = KeyboardSession.shared.feedbackSettings();
  }

  /** 按键是否在单手侧栏旁收窄绘制。 */
  private oneHandActive(): boolean {
    // 分离式键盘画着或浮动键盘生效时单手模式让位，存着的值不变，回到不分离的方向或键面、停靠键盘时单手自动回来，与 Android 的 `ImeFrame.effectiveOneHanded` 相同。
    return OneHandedPolicy.active(this.feedback.oneHanded, KeyboardFormFactor.isDesktop()) && !this.splitDrawn() && !this.floatingDrawn();
  }

  /** 浮动键盘此刻是否生效（`FloatingKeyboardPolicy.active`）：面板缩窄悬在应用上面，顶部画拖动条。 */
  private floatingDrawn(): boolean {
    return FloatingKeyboardPolicy.active(this.feedback.floating, KeyboardFormFactor.isDesktop());
  }

  /** 功能面板「浮动键盘」磁贴和拖动条的停靠键：经会话存下开关，会话再让扩展能力换面板。 */
  private setFloating(on: boolean): void {
    KeyboardSession.shared.setFloatingKeyboard(on);
    this.feedback = KeyboardSession.shared.feedbackSettings();
    // 浮动时底部抬高不生效，停靠后回来：面板高度跟着变。
    this.syncStripHeight();
  }

  /**
   * 正在画的键面能不能分离：26 键一族（含英文、日文罗马字、粤拼、越南语、藏文）和韩文的字母层，以及它们的 #+= 层。123 打开的数字九宫格不分，不管是从哪个键面借来的；日文假名、手写、九键、注音、笔画、14/18 键也不分，它们的 #+= 页同样不分，免得同一个方案在两页之间来回变形；2in1 不画触摸按键。
   */
  private splitsFace(): boolean {
    return this.touchDesign() && !this.japanese && !this.numberPadShowing()
      && !KeyboardScheme.usesHandwritingFace(this.scheme, this.english, false, this.localMode)
      && !KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)
      && !this.zhuyinFace() && !this.strokeFace() && !this.drawsFourteenKey();
  }

  /** 此刻是否画分离式键盘（`SplitKeyboardPolicy.drawn`）：开关、大屏、方向和启用宽度都满足，正在画的键面可以分离，并且键盘没有浮动。 */
  private splitDrawn(): boolean {
    return SplitKeyboardPolicy.drawn(this.splitKeyboard, this.screenWidthVp, this.screenHeightVp,
      KeyboardFormFactor.isDesktop(), this.splitsFace(), this.floatingDrawn());
  }

  /** 按键这一行的宽度：屏幕宽减去键盘根的左右内边距，空隙比例按它算。 */
  private splitRowWidthVp(): number {
    return this.screenWidthVp - 2 * KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP;
  }

  /** 一行怎么断开；`letterRow` 是字母层的行下标，数字行和符号层传 -1，只断开不重复。 */
  private splitRow(weights: number[], letterRow: number): SplitRow {
    return SplitKeyboardPolicy.row(weights, letterRow, LETTER_ROWS.length, this.splitRowWidthVp(),
      this.splitKeyboard.gapPercent);
  }

  /** 拆开的底行空格；`weights` 是底行各键的份额，空格按整份计。 */
  private splitSpace(weights: number[], spaceIndex: number): SplitSpace {
    return SplitKeyboardPolicy.space(weights, spaceIndex, this.splitRowWidthVp(), this.splitKeyboard.gapPercent);
  }

  /** 每个键都占一份的行（数字行、补充行）怎么断开。 */
  private splitEvenRow(count: number): SplitRow {
    const weights: number[] = [];
    for (let index: number = 0; index < count; index++) {
      weights.push(1);
    }
    return this.splitRow(weights, -1);
  }

  /** Candidate translations on or off; the session re-derives the gloss line and the reload it announces resizes the keyboard. */
  private toggleTranslations(): void {
    this.translations = KeyboardSession.shared.toggleCandidateTranslations();
  }

  /** Traditional output on or off; the session applies it to the candidates on screen and reports the state it ended in. */
  private toggleTraditional(): void {
    this.traditional = KeyboardSession.shared.toggleCharacterSet();
  }

  private applyClipboardMutation(result: ClipboardHistoryMutation): void {
    if (!this.clipboardEnabled) {
      this.clips = [];
      this.clipboardNotice = CLIPBOARD_HISTORY_OFF;
      this.show(SURFACE_NONE);
      return;
    }
    if (result.failure === null) {
      this.clips = result.entries;
      this.clipboardNotice = '';
      return;
    }
    // A stale row means another writer changed the valid document. Refresh to that exact snapshot;
    // a corrupt/unreadable file has no trustworthy replacement, so keep what is already on screen.
    if (result.failure === ClipboardFailure.STALE) {
      this.clips = result.entries;
    }
    this.clipboardNotice = result.failure;
  }

  /** 设置笔迹的颜色、粗细和圆头圆角，返回这次用的墨色。整幅重画和书写中补画新的一段共用，两者画出的笔迹一致。 */
  private applyHandwritingInk(context: CanvasRenderingContext2D): string {
    const pad: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    // 「笔迹颜色」：「跟随皮肤」取手写板皮肤的文字颜色，这样笔迹在任何底色上都看得清；其余是 `KeyboardFeedback` 的固定墨色。
    const ink: string = KeyboardFeedback.strokeInk(pad.handwritingStrokeColor, this.handwritingSkin.keyForeground);
    context.strokeStyle = ink;
    // 「笔迹粗细」以 vp 为单位，这块画布也是：笔画按触摸的 vp 坐标记录，限制在画布自己的宽高之内（`HandwritingStrokePolicy.pointInCanvas`），绘制上下文用的也是同一单位，所以线宽无需缩放就与设置显示的一致。
    context.lineWidth = KeyboardFeedback.handwritingStrokeWidth(pad.handwritingStrokeWidth);
    context.lineCap = 'round';
    context.lineJoin = 'round';
    return ink;
  }

  private redrawHandwriting(): void {
    const context: CanvasRenderingContext2D = this.handwritingContext;
    // 书写区铺满键盘宽度，平板和折叠屏上会比 420vp 宽：按画布实际大小清，免得右侧留下上一笔。
    context.clearRect(0, 0, Math.max(context.width, HANDWRITING_CANVAS_SIZE),
      Math.max(context.height, HANDWRITING_CANVAS_SIZE));
    this.applyHandwritingInk(context);
    for (const stroke of this.handwritingStrokes) {
      this.strokeHandwritingPoints(context, stroke.points);
    }
    this.strokeHandwritingPoints(context, this.handwritingCurrent);
  }

  /** 一笔画成一条连续路径。按下标取点，不为每一笔复制一份点数组。 */
  private strokeHandwritingPoints(context: CanvasRenderingContext2D, points: HandwritingPoint[]): void {
    if (points.length === 0) {
      return;
    }
    context.beginPath();
    context.moveTo(points[0].x, points[0].y);
    for (let index: number = 1; index < points.length; index++) {
      context.lineTo(points[index].x, points[index].y);
    }
    context.stroke();
  }

  /**
   * 书写中每来一个触摸点，只在已有画面上补画最后一段，不清屏重画整个字：已画好的笔画在这期间不会变，整幅重画让每个移动事件的开销随已写的点数增长，写一个字是平方级。抬笔、取消、撤销、重做、清除和画布重建仍整幅重画，所以笔画定稿后的画面与原来逐点重画完全相同。半透明墨色的圆头线段在接点处会叠深，这时退回整幅重画。
   */
  private drawHandwritingSegment(): void {
    const context: CanvasRenderingContext2D = this.handwritingContext;
    const ink: string = this.applyHandwritingInk(context);
    const count: number = this.handwritingCurrent.length;
    if (count < 2 || !HandwritingStrokePolicy.opaqueInk(ink)) {
      this.redrawHandwriting();
      return;
    }
    const from: HandwritingPoint = this.handwritingCurrent[count - 2];
    const to: HandwritingPoint = this.handwritingCurrent[count - 1];
    context.beginPath();
    context.moveTo(from.x, from.y);
    context.lineTo(to.x, to.y);
    context.stroke();
  }

  private handwritingPoint(event: TouchEvent): HandwritingPoint {
    const touch = event.changedTouches[0];
    return HandwritingStrokePolicy.pointInCanvas(
      touch.x, touch.y, this.handwritingContext.width, this.handwritingContext.height);
  }

  private handwritingTouch(event: TouchEvent): void {
    if (event.type === TouchType.Cancel) {
      // 被系统接管的笔画不参与识别，取消事件不要求仍有触点。
      this.handwritingCurrent = [];
      this.redrawHandwriting();
      return;
    }
    if (event.changedTouches.length === 0) {
      return;
    }
    if (event.type === TouchType.Down) {
      this.handwritingChanged();
      this.handwritingCurrent = [this.handwritingPoint(event)];
      this.handwritingNotice = '书写中…';
      this.redrawHandwriting();
      return;
    }
    if (event.type === TouchType.Move) {
      if (this.handwritingCurrent.length > 0
        && this.handwritingCurrent.length < HANDWRITING_MAX_POINTS) {
        this.handwritingCurrent.push(this.handwritingPoint(event));
        this.drawHandwritingSegment();
      }
      return;
    }
    if (event.type === TouchType.Up) {
      if (this.handwritingCurrent.length > 0) {
        if (this.handwritingCurrent.length < HANDWRITING_MAX_POINTS) {
          this.handwritingCurrent.push(this.handwritingPoint(event));
        }
        if (this.handwritingStrokes.length < HANDWRITING_MAX_STROKES) {
          this.handwritingStrokes = [...this.handwritingStrokes,
            { points: this.handwritingCurrent.slice() }];
          this.handwritingRedo = [];
          this.handwritingNotice = '';
        } else {
          this.handwritingNotice = '笔画过多，请先清除或撤销后再识别';
        }
      }
      this.handwritingCurrent = [];
      this.redrawHandwriting();
      this.scheduleHandwritingRecognition();
      return;
    }
  }

  private cancelHandwritingTimer(): void {
    if (this.handwritingTimer >= 0) {
      clearTimeout(this.handwritingTimer);
      this.handwritingTimer = -1;
    }
  }

  /** Invalidates candidates and in-flight OCR as soon as the canvas changes. */
  private handwritingChanged(): void {
    this.cancelHandwritingTimer();
    this.handwritingRecognition.changed();
    this.handwritingCandidates = [];
  }

  /** 等待用户设定的停顿时长（「识别等待时间」，`KeyboardFeedback` 的 200–1500 ms，默认 600）后识别已完成的笔画。 */
  private scheduleHandwritingRecognition(): void {
    this.cancelHandwritingTimer();
    if (!HandwritingStrokePolicy.canRecognize(this.handwritingStrokes)) {
      this.handwritingNotice = this.handwritingStrokes.length === 0
        ? '在此手写，停笔后选字' : '笔迹过多，请撤销后重试';
      return;
    }
    this.handwritingNotice = '停笔后识别…';
    this.handwritingTimer = setTimeout((): void => {
      this.handwritingTimer = -1;
      const ticket: HandwritingRecognitionTicket | null = this.handwritingRecognition.request();
      if (ticket !== null) {
        this.recognizeHandwriting(ticket);
      }
    }, KeyboardFeedback.handwritingDelay(KeyboardSession.shared.feedbackSettings().handwritingDelayMs));
  }

  private clearHandwriting(): void {
    this.handwritingChanged();
    this.handwritingStrokes = [];
    this.handwritingRedo = [];
    this.handwritingCandidates = [];
    this.handwritingCurrent = [];
    this.handwritingNotice = '';
    this.redrawHandwriting();
  }

  private hasHandwritingContext(): boolean {
    return this.handwritingCurrent.length > 0 || this.handwritingStrokes.length > 0
      || this.handwritingRedo.length > 0
      || this.handwritingCandidates.length > 0;
  }

  private clearHandwritingAfterLeaving(wasHandwriting: boolean): void {
    if (wasHandwriting && !KeyboardScheme.usesHandwritingFace(
      this.scheme, this.english, this.symbols, this.localMode) && this.hasHandwritingContext()) {
      this.clearHandwriting();
    }
  }

  private undoHandwriting(): void {
    if (this.handwritingStrokes.length === 0) {
      return;
    }
    const removed: HandwritingStroke | undefined =
      this.handwritingStrokes[this.handwritingStrokes.length - 1];
    if (removed === undefined) {
      return;
    }
    this.handwritingChanged();
    this.handwritingStrokes = this.handwritingStrokes.slice(0, -1);
    this.handwritingRedo = [...this.handwritingRedo, removed];
    this.handwritingCandidates = [];
    this.handwritingNotice = '';
    this.redrawHandwriting();
    this.scheduleHandwritingRecognition();
  }

  private deleteHandwriting(): void {
    if (this.handwritingStrokes.length > 0) {
      this.undoHandwriting();
      return;
    }
    // Match the Apple handwriting panel: once the canvas is empty, its delete key is the
    // editor's backspace rather than a no-op. This is especially useful after choosing a
    // candidate and correcting the character immediately before the caret.
    KeyboardSession.shared.backspace();
  }

  private redoHandwriting(): void {
    if (this.handwritingRedo.length === 0
      || this.handwritingStrokes.length >= HANDWRITING_MAX_STROKES) {
      return;
    }
    const restored: HandwritingStroke | undefined = this.handwritingRedo[
      this.handwritingRedo.length - 1];
    if (restored === undefined) {
      return;
    }
    this.handwritingChanged();
    this.handwritingRedo = this.handwritingRedo.slice(0, -1);
    this.handwritingStrokes = [...this.handwritingStrokes, restored];
    this.handwritingCandidates = [];
    this.handwritingNotice = '';
    this.redrawHandwriting();
    this.scheduleHandwritingRecognition();
  }

  private finishHandwritingRecognition(): void {
    const next: HandwritingRecognitionTicket | null = this.handwritingRecognition.finish();
    if (next !== null && HandwritingStrokePolicy.canRecognize(this.handwritingStrokes)) {
      this.recognizeHandwriting(next);
      return;
    }
    this.handwritingBusy = false;
  }

  private recognizeHandwriting(ticket: HandwritingRecognitionTicket): void {
    if (!HandwritingStrokePolicy.canRecognize(this.handwritingStrokes)) {
      this.finishHandwritingRecognition();
      return;
    }
    this.handwritingBusy = true;
    this.handwritingNotice = '正在识别…';
    KeyboardSession.shared.recognizeHandwriting(
      this.handwritingStrokes, this.handwritingContext.width, this.handwritingContext.height)
      .then((candidates: string[]): void => {
      if (!this.handwritingRecognition.accepts(ticket)) {
        return;
      }
      this.handwritingCandidates = candidates;
      this.handwritingNotice = this.handwritingCandidates.length > 0
        ? '' : '没有识别到文字，请重写';
    }).catch((reason: Error): void => {
      if (this.handwritingRecognition.accepts(ticket)) {
        KeyboardLog.error(`handwriting recognition unavailable: ${describeError(reason)}`);
        this.handwritingCandidates = [];
        this.handwritingNotice = '手写识别服务不可用';
      }
    }).finally((): void => {
      this.finishHandwritingRecognition();
    });
  }

  private chooseHandwriting(candidate: string): void {
    if (this.handwritingBusy || candidate.length === 0) {
      return;
    }
    KeyboardSession.shared.insertWithSource(candidate, 'handwriting');
    this.clearHandwriting();
  }

  private startVoice(fromHotkey: boolean = false): void {
    if (this.voiceBusy) {
      return;
    }
    // 指示区按这次尝试的方式定下来：录音开不了（已关闭、没有麦克风权限、会话不可用）时面板显示原因，也用这一次的指示区，不沿用上一次会话的（#10313）。
    this.voiceMascot = VoiceListeningPolicy.showsMascot(this.spaceVoiceHeld, fromHotkey);
    if (!KeyboardSession.shared.voiceInputEnabled()) {
      this.voiceEnabled = false;
      this.voiceNotice = '语音输入已在设置中关闭';
      return;
    }
    if (!HarmonyMicrophonePermission.granted()) {
      this.voiceNotice = '请允许麦克风权限，返回后重新录音';
      KeyboardSession.shared.requestVoicePermission();
      return;
    }
    this.voiceGeneration = KeyboardSession.shared.voiceStart();
    if (this.voiceGeneration === 0) {
      this.voiceNotice = '语音会话不可用';
      return;
    }
    this.clearVoiceDone();
    this.cancelVoiceSilence();
    this.voiceSpeaking = false;
    this.voiceHeldByKey = fromHotkey;
    const voiceConfiguration = KeyboardSession.shared.voiceInputConfiguration();
    this.voiceEndsOnPause = VoiceRecordingBehaviourPolicy.endsOnPause(voiceConfiguration);
    // 停顿时间和判停灵敏度在开始聆听时固定（#10310）。本地识别用设置里的停顿时间，系统识别沿用固定的 1500 毫秒；判停灵敏度只给点按开始、按停顿结束的本地识别，按住说话的录音由松手结束，保留原来的 VAD 阈值。
    this.voicePauseEnds = this.touchDesign() && VoiceRecordingBehaviourPolicy.keyboardStopsOnPause(voiceConfiguration);
    this.voicePauseMs = VoiceRecordingBehaviourPolicy.pauseMillis(voiceConfiguration);
    voiceConfiguration.local_stop_sensitivity =
      VoiceRecordingBehaviourPolicy.stopSensitivity(voiceConfiguration, this.voicePauseEnds && !fromHotkey);
    this.voiceText = '';
    this.voiceHeard = '';
    this.voiceLevel = 0;
    this.voiceNotice = '正在聆听…';
    this.voiceBusy = true;
    this.voiceFinishing = false;
    // 结果要去的位置，这样只有结果到达时编辑框仍在那里才免审阅直接上屏（Android 的 `captureVoiceTarget`）。
    KeyboardSession.shared.captureVoiceTarget();
    KeyboardSession.shared.setVoiceRecording(true);
    this.voiceRecognizer.start(voiceConfiguration.language || 'zh-CN', voiceConfiguration,
      (text: string, final: boolean): void => {
      // 中间结果按「边说边显示」展示，最终结果始终进入上屏流程。
      if (text.length > 0
          && (final || VoiceRecordingBehaviourPolicy.showsInterimResults(voiceConfiguration))) {
        this.voiceText = VoiceRecognitionPolicy.result(text);
        if (!final && this.touchDesign()) {
          this.voiceHeard = this.voiceText;
        }
      }
      if (!final && text.length > 0 && this.voicePauseEnds) {
        this.armVoiceSilence();
      }
      if (final && this.voiceGeneration > 0) {
        const accepted: string | null = KeyboardSession.shared.voiceApply(
          this.voiceGeneration, this.voiceText);
        this.voiceGeneration = 0;
        if (accepted !== null) {
          this.voiceText = accepted;
        }
        this.cancelVoiceSilence();
        this.voiceFinishing = false;
        this.voiceBusy = false;
        this.voiceHeard = '';
        this.voiceLevel = 0;
        KeyboardSession.shared.setVoiceRecording(false);
        this.deliverVoice(accepted);
      }
      }, (message: string): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      this.voiceHeard = '';
      this.voiceLevel = 0;
      this.cancelVoiceSilence();
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = message;
      }, KeyboardSession.shared.voiceHotwordOptions(), (speaking: boolean): void => {
      this.voiceSpeechChanged(speaking);
      }, (level: number): void => {
      // 收尾识别时麦克风已经在关，晚到的电平不再让光圈涨回去。
      if (this.voiceBusy && !this.voiceFinishing) this.voiceLevel = level;
      }).catch((error: Object): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      this.voiceHeard = '';
      this.voiceLevel = 0;
      this.cancelVoiceSilence();
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = error instanceof Error ? error.message : '语音识别不可用';
    });
  }

  /**
   * 手机接受的最终转写直接进入开始录音时的输入位置，不按识别服务区分。输入位置变化时保留结果供显式插入；二合一沿用审阅面板。
   */
  private deliverVoice(accepted: string | null): void {
    if (accepted === null || accepted.length === 0) {
      this.voiceNotice = accepted !== null ? '没有识别到语音。' :
        (this.voiceText.length > 0 ? '已忽略过期的语音结果。' : '没有识别到语音。');
      return;
    }
    if (!VoiceRecognitionPolicy.skipsReview(this.touchDesign(), this.surface === SURFACE_VOICE)) {
      this.voiceNotice = '识别完成，可提交结果。';
      return;
    }
    if (!KeyboardSession.shared.voiceTargetMatches()) {
      this.voiceNotice = '输入位置已变化，结果已保留，可点插入文字。';
      return;
    }
    KeyboardSession.shared.insertVoiceResult(accepted);
    this.voiceText = '';
    this.voiceNotice = '';
    this.voiceDone = accepted;
    this.voiceDoneTimer = setTimeout((): void => {
      this.voiceDoneTimer = -1;
      if (this.voiceDone !== accepted) {
        return;
      }
      this.voiceDone = '';
      if (this.surface === SURFACE_VOICE) {
        this.show(SURFACE_NONE);
      }
    }, VOICE_DONE_NOTICE_MS);
  }

  private clearVoiceDone(): void {
    if (this.voiceDoneTimer >= 0) {
      clearTimeout(this.voiceDoneTimer);
      this.voiceDoneTimer = -1;
    }
    this.voiceDone = '';
  }

  /**
   * 本地识别和系统识别听到文字后停顿 `voicePauseMs` 自动结束（本地识别是用户设的停顿时间，系统识别固定 1500 毫秒）。豆包流式和上传式识别由用户点「说完了」结束，不经这里（`VoiceRecordingBehaviourPolicy.keyboardStopsOnPause`）。按住空格或快捷键的录音在松手时结束，VAD 仍听到说话时顺延静音计时。
   */
  private armVoiceSilence(): void {
    if (!this.touchDesign() || this.voiceHeldByKey || !this.voiceBusy || this.voiceFinishing) {
      return;
    }
    this.cancelVoiceSilence();
    this.voiceSilenceTimer = setTimeout((): void => {
      this.voiceSilenceTimer = -1;
      if (!VoiceRecognitionPolicy.silenceStops(this.voiceSpeaking)) {
        this.armVoiceSilence();
        return;
      }
      this.stopVoice();
    }, this.voicePauseMs);
  }

  /**
   * 经过 VAD 的本地整段模型报告说话状态变化。这类模型只在一段说完后才出文字，连续说话超过停顿时间时没有新文字可以续上静音计时，所以已经开始的计时在每次变化时从头再计：开始说话时推迟，说完时从这一刻起再等一个完整的停顿。还没听到文字时计时尚未开始，这里不启动它。
   */
  private voiceSpeechChanged(speaking: boolean): void {
    this.voiceSpeaking = speaking;
    if (this.voiceSilenceTimer >= 0) {
      this.armVoiceSilence();
    }
  }

  private cancelVoiceSilence(): void {
    if (this.voiceSilenceTimer >= 0) {
      clearTimeout(this.voiceSilenceTimer);
      this.voiceSilenceTimer = -1;
    }
  }

  private stopVoice(): void {
    if (!this.voiceBusy || this.voiceFinishing) {
      return;
    }
    this.cancelVoiceSilence();
    this.voiceFinishing = true;
    this.voiceLevel = 0;
    this.voiceNotice = '正在完成识别…';
    try {
      this.voiceRecognizer.stop();
    } catch (error) {
      this.voiceBusy = false;
      this.voiceFinishing = false;
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = '无法结束语音识别';
    }
  }

  private cancelVoice(): void {
    try {
      this.voiceRecognizer.cancel();
    } catch (error) {
      // Cancellation is best effort; the visible state still has to become idle.
    }
    this.voiceBusy = false;
    this.voiceFinishing = false;
    this.voiceHeard = '';
    this.voiceLevel = 0;
    this.cancelVoiceSilence();
    this.clearVoiceDone();
    // Cancelled rather than finished: nothing was recognised, so there is nothing to announce.
    KeyboardSession.shared.setVoiceRecording(false, true);
    this.voiceText = '';
    this.voiceNotice = '已取消语音识别';
    KeyboardSession.shared.voiceCancel();
    this.voiceGeneration = 0;
  }

  private submitVoice(): void {
    const text: string = VoiceRecognitionPolicy.result(this.voiceText);
    if (text.length === 0 || this.voiceBusy) {
      return;
    }
    KeyboardSession.shared.insertVoiceResult(text);
    this.voiceText = '';
    this.voiceNotice = '已提交';
  }

  private voiceDescription(): string {
    return VoiceInputConfigurationPolicy.description(
      KeyboardSession.shared.voiceInputConfiguration());
  }

  /** 聆听界面的标题：录音时听到了文字就显示它，否则为「正在聆听…」；结果上屏后为「识别完成」，即 Android 的 `VoiceListeningView` 文案。 */
  private voiceListeningTitle(): string {
    if (this.voiceListeningTitleTail()) return this.voiceHeard;
    return this.voiceDone.length > 0 ? '识别完成'
      : this.voiceFinishing ? '正在识别…' : this.voiceBusy ? '正在聆听…' : '语音输入';
  }

  /** 标题位放的是录音中听到的文字，面板放不下时从开头省略，留住最新的那段。 */
  private voiceListeningTitleTail(): boolean {
    return this.voiceDone.length === 0 && this.voiceBusy && !this.voiceFinishing && this.voiceHeard.length > 0;
  }

  /** 标题下方的一行：已上屏的内容、收尾识别时最后听到的文字，或如何结束；空闲时为提示或识别服务说明。 */
  private voiceListeningHint(): string {
    if (this.voiceDone.length > 0) {
      return this.voiceDone;
    }
    if (this.voiceFinishing) {
      return this.voiceHeard.length > 0 ? this.voiceHeard : '识别完成后自动上屏';
    }
    if (!this.voiceBusy) {
      return this.voiceNotice.length > 0 ? this.voiceNotice : this.voiceDescription();
    }
    if (this.voiceHeldByKey) {
      return '松开结束录音';
    }
    return this.voiceEndsOnPause ? '停顿后自动上屏，也可点「说完了」' : '说完后点「说完了」，识别后自动上屏';
  }

  /** 提示行放的是转写文字（已上屏的内容或收尾时最后听到的），放不下一行时从开头省略。 */
  private voiceListeningHintTail(): boolean {
    return this.voiceDone.length > 0 || (this.voiceFinishing && this.voiceHeard.length > 0);
  }

  /** 已上屏的内容前面加「已识别：」，裁剪时这几个字始终留着。 */
  private voiceListeningHintPrefix(): string {
    return this.voiceDone.length > 0 ? '已识别：' : '';
  }

  /**
   * 聆听面板按钮要看的会话状态，按钮文字由 `VoiceListeningPolicy.primaryLabel`、`cancelLabel` 决定。手机面板的主操作只结束采集；识别回调负责自动上屏，位置变化的结果才需显式插入。空格长按的面板在松手收尾时才出现；快捷键一按下就打开面板（`onVoiceHotkey` 的 START），按住期间没有按钮，结束和取消走快捷键自己的 STOP、CANCEL，LOCK 之后按点按开始对待。
   */
  private voiceListeningSession(): VoiceListeningSession {
    return {
      busy: this.voiceBusy,
      finishing: this.voiceFinishing,
      heldByKey: this.voiceHeldByKey,
      done: this.voiceDone.length > 0,
      hasText: this.voiceText.length > 0
    };
  }

  private voicePrimaryAction(): void {
    if (this.voiceBusy) {
      this.stopVoice();
    } else if (this.voiceText.length > 0) {
      this.submitVoice();
      this.show(SURFACE_NONE);
    } else {
      this.startVoice();
    }
  }

  /** 取消录音或返回按键，均不提交未完成的转写。 */
  private cancelListening(): void {
    this.cancelVoice();
    this.show(SURFACE_NONE);
  }

  /** 两个按住标志不是状态变量，快捷键 LOCK 清掉 `voiceHeldByKey` 之后调这里，让面板按新的会话方式换指示区（#10313）；开始录音时 `startVoice` 直接按这次尝试算。 */
  private refreshVoiceMascot(): void {
    this.voiceMascot = VoiceListeningPolicy.showsMascot(this.spaceVoiceHeld, this.voiceHeldByKey);
  }

  /** 聆听面板人物的图片地址：表情包里的 `01.webp`，与 Android、iOS 同一张；表情包读不到时为空，面板画圆盘。清单由 `MascotStickerStore` 缓存，只读一次。 */
  private voiceMascotUri(): string {
    const sticker: MascotSticker | undefined = KeyboardSession.shared.mascotStickers.list()
      .find((candidate: MascotSticker): boolean => candidate.file === VOICE_MASCOT_FILE);
    return sticker === undefined ? '' : KeyboardSession.shared.mascotStickers.uri(sticker);
  }

  @Builder
  voiceFace() {
    if (this.touchDesign()) {
      Column() {
        VoiceListening({
          accent: this.voiceSkin.accent,
          onAccent: this.voiceSkin.onAccent,
          foreground: this.voiceSkin.keyForeground,
          secondary: this.voiceSkin.secondary,
          micPath: KeyboardIconPaths.MIC,
          title: this.voiceListeningTitle(),
          titleTail: this.voiceListeningTitleTail(),
          hint: this.voiceListeningHint(),
          hintTail: this.voiceListeningHintTail(),
          hintPrefix: this.voiceListeningHintPrefix(),
          level: this.voiceLevel,
          mascot: this.voiceMascot,
          mascotUri: this.voiceMascot ? this.voiceMascotUri() : '',
          recording: this.voiceBusy && !this.voiceFinishing,
          primaryLabel: VoiceListeningPolicy.primaryLabel(this.voiceListeningSession()),
          primaryEnabled: !this.voiceFinishing,
          cancelLabel: VoiceListeningPolicy.cancelLabel(this.voiceListeningSession()),
          onPrimary: (): void => this.voicePrimaryAction(),
          onCancel: (): void => this.cancelListening()
        })
      }
      .width('100%')
      .height(this.surfaceHeight())
      .backgroundColor(this.voiceSkin.background)
    } else {
      this.voiceReviewFace()
    }
  }

  /** 待审阅和提交的识别结果，附带重新录音或取消的控件。 */
  @Builder
  voiceReviewFace() {
    Column({ space: KeyboardMetrics.KEY_SPACING_VP }) {
      Row() {
        Text('语音输入')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.voiceSkin.keyForeground)
          .layoutWeight(1)
        Text('返回')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.voiceSkin.accent)
          .onClick(() => this.leaveSurface(DesktopSurface.VOICE))
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      Text(this.voiceNotice.length > 0 ? this.voiceNotice : this.voiceDescription())
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.voiceSkin.secondary)
        .maxLines(2)
        .width('100%')
        .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
          right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      TextArea({ text: this.voiceText, placeholder: '识别结果会显示在这里' })
        .width('100%')
        .height(this.surfaceHeight() * 0.35)
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.voiceSkin.keyForeground)
        .backgroundColor(this.voiceSkin.keyBackground)
        .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
        .onChange((value: string): void => {
          this.voiceText = VoiceRecognitionPolicy.result(value);
        })

      Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
        Text(this.voiceFinishing ? '正在完成…' : this.voiceBusy ? '停止录音' : '开始录音')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.voiceSkin.actionForeground)
          .layoutWeight(1)
          .textAlign(TextAlign.Center)
          .padding({ top: 10, bottom: 10 })
          .backgroundColor(this.voiceSkin.actionBackground)
          .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
          .onClick(() => this.voiceBusy ? this.stopVoice() : this.startVoice())
        Text('取消')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.voiceSkin.keyForeground)
          .padding({ left: 16, right: 16, top: 10, bottom: 10 })
          .backgroundColor(this.voiceSkin.keyBackground)
          .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
          .onClick(() => this.cancelVoice())
        Text('提交')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.voiceSkin.keyForeground)
          .padding({ left: 16, right: 16, top: 10, bottom: 10 })
          .backgroundColor(this.voiceSkin.keyBackground)
          .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
          .opacity(this.voiceText.length > 0 && !this.voiceBusy ? 1 : 0.4)
          .onClick(() => this.submitVoice())
      }
      .width('100%')
    }
    .width('100%')
    .height(this.surfaceHeight())
    .backgroundColor(this.voiceSkin.background)
  }

  @Builder
  handwritingFace() {
    Column({ space: this.rowSpacing() }) {
      if (this.touchDesign()) {
        this.handwritingPad()
        this.handwritingActionRow()
      } else {
        this.handwritingPanel()
        // 手写 reaches the letters through the same language key every other face uses, which is what
        // HandwritingSchemeKeepsToolbarAndSwitchesBackToLetters asks of it.
        this.actionRow()
      }
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /**
   * 触屏手写板，即 Android 的 `rebuildHandwritingRows`：书写区铺满键区宽度，竖屏有四行键高（`KeyboardMetrics.handwritingExtraRows`），下方是 `handwritingActionRow`。⌫、「重写」和标点都在底行；⌫ 撤回最后一笔，手写板为空时删除光标前的字符；这里没有重做按钮，与 Android 一致。
   *
   * 参考线和状态行独立叠放；书写区外层统一接收触摸，识别只使用已完成的笔画。
   */
  @Builder
  handwritingPad() {
    Row() {
      Stack() {
        Canvas(this.handwritingContext)
          .id(HANDWRITING_CANVAS_ID)
          .accessibilityText('手写区域；用手指书写，停笔后选择候选文字')
          .width('100%')
          .height('100%')
          .backgroundColor(this.handwritingSkin.keyBackground)
          .borderRadius(this.handwritingSkin.keyCornerRadius())
          .borderWidth(this.handwritingSkin.borderWidth)
          .borderColor(this.handwritingSkin.borderColor)
          .onReady(() => this.redrawHandwriting())
        // 设计稿的参考线：距边缘 HANDWRITING_GUIDE_INSET_VP 的虚线框，以及穿过其中心的虚线十字，用细线颜色。
        Stack() {
          Column()
            .width('100%')
            .height('100%')
            .border({ width: 1, color: this.handwritingSkin.hair, style: BorderStyle.Dashed,
              radius: HANDWRITING_GUIDE_RADIUS_VP })
          Column()
            .width(1)
            .height('100%')
            .border({ width: { left: 1 }, color: this.handwritingSkin.hair, style: BorderStyle.Dashed })
          Column()
            .width('100%')
            .height(1)
            .border({ width: { top: 1 }, color: this.handwritingSkin.hair, style: BorderStyle.Dashed })
        }
        .width('100%')
        .height('100%')
        .padding(HANDWRITING_GUIDE_INSET_VP)
        .hitTestBehavior(HitTestMode.None)
        .accessibilityLevel('no')
        if (this.handwritingStatus().length > 0) {
          Text(this.handwritingStatus())
            .fontSize(HANDWRITING_STATUS_FONT_SIZE)
            .fontColor(this.handwritingSkin.secondary)
            .maxLines(2)
            .textAlign(TextAlign.Center)
            .padding({ left: HANDWRITING_GUIDE_INSET_VP, right: HANDWRITING_GUIDE_INSET_VP })
            .hitTestBehavior(HitTestMode.None)
        }
      }
      .layoutWeight(1)
      .height('100%')
      .onTouch((event: TouchEvent): void => this.handwritingTouch(event))
    }
    .width('100%')
    .height(this.gridHeight())
  }

  /** 手写板在书写区上显示的提示：识别器的进度或问题，或在手写板为空时的使用说明（Android 的手写状态行）。 */
  private handwritingStatus(): string {
    if (this.handwritingNotice.length > 0) {
      return this.handwritingNotice;
    }
    return this.handwritingStrokes.length === 0 && this.handwritingCurrent.length === 0
      ? HANDWRITING_IDLE_STATUS : '';
  }

  @Builder
  handwritingPanel() {
    Column({ space: KeyboardMetrics.KEY_SPACING_VP }) {
      Row() {
        Text('手写输入')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.handwritingSkin.keyForeground)
          .layoutWeight(1)
        Text('撤销')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.handwritingSkin.accent)
          .accessibilityText('撤销最后一笔')
          .onClick(() => this.undoHandwriting())
        Text('重做')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.handwritingSkin.accent)
          .padding({ left: 12 })
          .accessibilityText('重做最后一笔')
          .onClick(() => this.redoHandwriting())
        Text('清除')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.handwritingSkin.accent)
          .padding({ left: 12 })
          .accessibilityText('清除全部笔迹')
          .onClick(() => this.clearHandwriting())
        Text('⌫')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.handwritingSkin.accent)
          .padding({ left: 12 })
          .accessibilityText('删除')
          .onClick(() => this.deleteHandwriting())
        // Only the toolbar's pad needs a way out of its own; inside a handwriting scheme's keys the language key is that way.
        if (this.desktop === DesktopSurface.HANDWRITING) {
          Text('返回')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.handwritingSkin.accent)
            .padding({ left: 12 })
            .onClick(() => this.leaveSurface(DesktopSurface.HANDWRITING))
        }
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      Canvas(this.handwritingContext)
        .id(HANDWRITING_CANVAS_ID)
        .accessibilityText('手写区域，用手指书写')
        .width('100%')
        // Candidates now live in the shared strip above this face, so the canvas can take every bit
        // left after the tools and an occasional status line instead of keeping their old row empty.
        .layoutWeight(1)
        .backgroundColor(this.handwritingSkin.keyBackground)
        .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
        .borderWidth(this.handwritingSkin.borderWidth)
        .borderColor(this.handwritingSkin.borderColor)
        .onReady(() => this.redrawHandwriting())
        .onTouch((event: TouchEvent): void => this.handwritingTouch(event))

      if (this.handwritingNotice.length > 0) {
        Text(this.handwritingNotice)
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.handwritingSkin.secondary)
          .maxLines(1)
      }
    }
    .width('100%')
    .height(this.gridHeight())
    .backgroundColor(this.handwritingSkin.background)
  }

  /**
   * 「输入方式」选择器的条目顺序：用户启用的方案，两种英文模式排第三，与 iOS 的位置相同（启用的方案较少时排最后），末尾是「添加语言」。
   */
  private schemePickerEntries(): SchemePickerEntry[] {
    // 双拼只列用户设置的那一种（#6450）。
    const schemes: SchemeDefinition[] = KeyboardScheme.pickerSchemes(this.offeredSchemes(), this.scheme,
      this.settings.shuangpinProfile);
    const entries: SchemePickerEntry[] = [];
    const englishAt: number = Math.min(ENGLISH_CARD_POSITION, schemes.length);
    for (let index: number = 0; index <= schemes.length; index++) {
      if (index === englishAt) {
        entries.push({ kind: PICKER_ENGLISH_26, scheme: null });
        entries.push({ kind: PICKER_ENGLISH_9, scheme: null });
      }
      if (index < schemes.length) {
        entries.push({ kind: PICKER_SCHEME, scheme: schemes[index] });
      }
    }
    entries.push({ kind: PICKER_ADD_LANGUAGE, scheme: null });
    return entries;
  }

  /** 按屏幕此刻的宽高（转屏后两者互换）判断是不是手机横屏。 */
  private readOrientation(): void {
    try {
      const screen: display.Display = display.getDefaultDisplaySync();
      const widthVp: number = screen.width / screen.densityPixels;
      const heightVp: number = screen.height / screen.densityPixels;
      this.phoneLandscape = KeyboardFormFactorPolicy.phoneLandscape(widthVp, heightVp);
      this.landscape = screen.width > screen.height;
      this.screenWidthVp = widthVp;
      this.screenHeightVp = heightVp;
      const largeScreen: boolean = KeyboardFormFactorPolicy.largeScreen(widthVp, heightVp);
      this.oneHandBlockVp = OneHandedPolicy.blockWidthVp(widthVp, largeScreen);
      // 按键这一行的宽度是屏幕宽减去键盘根的左右内边距，侧边数字键盘按它算才不会把这一行撑出屏幕。
      this.oneHandSidePadVp = OneHandedPolicy.sidePadWidthVp(
        widthVp - 2 * KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP, largeScreen);
    } catch {
      // 显示服务暂不可用时保留上次方向，等待下一次窗口布局通知。
      KeyboardLog.error('keyboard orientation unavailable');
    }
  }

  /** 功能面板和方案选择的行数、行距和磁贴缩放（`FunctionPanelPolicy.pickerGrid`），按面板去掉上方留白、圆点和下方留白后的高度排；是否放得下按有圆点算。 */
  private tileGrid(): PickerGrid {
    return FunctionPanelPolicy.pickerGrid(this.phoneLandscape,
      this.surfaceHeight() - TILE_TOP_PADDING_VP - PANEL_DOTS_ROOM_VP - PANEL_BOTTOM_PADDING_VP, TILE_ROW_VP);
  }

  private tilePageSize(): number {
    return FunctionPanelPolicy.COLUMNS * this.tileGrid().rows;
  }

  /** 皮肤选择器的行高：两行预览，多于一页时再加圆点，在按键让出的高度内。 */
  /** 功能面板和方案选择每行的高度：各行填满面板去掉上下留白、行距和圆点后的高度，至少为 `pickerGrid` 给的缩放后行高。磁贴在行内居中。 */
  private tileRowHeight(count: number): number {
    const grid: PickerGrid = this.tileGrid();
    const dots: number = count > this.tilePageSize() ? PANEL_DOTS_ROOM_VP : 0;
    return Math.max(grid.rowHeight, Math.floor((this.surfaceHeight() - TILE_TOP_PADDING_VP
      - (grid.rows - 1) * grid.rowsGap - dots - PANEL_BOTTOM_PADDING_VP) / grid.rows));
  }

  private skinRowHeight(pageCount: number): number {
    const dots: number = pageCount > 1 ? PANEL_DOTS_ROOM_VP : 0;
    return Math.max(0, Math.floor((this.surfaceHeight() - SKIN_TOP_PADDING_VP - SKIN_ROWS_GAP_VP - dots
      - PANEL_BOTTOM_PADDING_VP) / 2));
  }

  /** 皮肤预览之间的间距：至少为设计稿的值，并且足够宽，使无论屏幕多宽，预览的缩略图加名称都放得进行高。 */
  private skinColumnsGap(pageCount: number): number {
    const thumbnail: number = (this.skinRowHeight(pageCount) - SKIN_NAME_ROOM_VP) / SKIN_THUMB_ASPECT;
    const tile: number = Math.max(0, thumbnail) + SKIN_RING_ROOM_VP;
    const inner: number = this.availableWidth - PANEL_SIDE_PADDING_VP * 2;
    return Math.max(SKIN_MIN_COLUMNS_GAP_VP, (inner - tile * PANEL_COLUMNS) / (PANEL_COLUMNS - 1));
  }

  private skinPageCount(): number {
    return Math.max(1, Math.ceil(this.skinTileCount() / FunctionPanelPolicy.PAGE_SIZE));
  }

  /** 皮肤选择器的格子数：各个皮肤，再加最后一格「更多皮肤」。 */
  private skinTileCount(): number {
    return KeyboardSession.shared.themeSkins(this.settings.dark).length + 1;
  }

  /**
   * 方案选择器和皮肤选择器，占据按键行的位置而不是浮在其上方，由工具栏的「输入方式」和「皮肤」打开和关闭。
   *
   * 扩展面板是固定的矩形，没有可以覆盖的地方，所以弹窗要么在面板边缘被裁切，要么迫使面板变大、把编辑框往屏幕上方顶。占用已有的空间没有代价，也不需要调整大小。
   */
  @Builder
  pickerFace() {
    Column() {
      if (this.surface === SURFACE_SCHEME) {
        PagedTiles({
          count: this.schemePickerEntries().length,
          pageSize: this.tilePageSize(),
          rowHeight: this.tileRowHeight(this.schemePickerEntries().length),
          rowsGap: this.tileGrid().rowsGap,
          topPadding: TILE_TOP_PADDING_VP,
          accent: this.skin.toggleForeground,
          hair: this.skin.hair,
          tile: (index: number): void => {
            this.schemeTile(this.schemePickerEntries()[index])
          }
        })
      } else {
        // 每个皮肤显示为按其配色绘制的键盘缩略图：配色就是全部的选择，直接展示比在色块旁写个名字更能说明问题。
        PagedTiles({
          count: this.skinTileCount(),
          pageSize: FunctionPanelPolicy.PAGE_SIZE,
          rowHeight: this.skinRowHeight(this.skinPageCount()),
          rowsGap: SKIN_ROWS_GAP_VP,
          columnsGap: this.skinColumnsGap(this.skinPageCount()),
          topPadding: SKIN_TOP_PADDING_VP,
          accent: this.skin.toggleForeground,
          hair: this.skin.hair,
          tile: (index: number): void => {
            if (index < this.skinTileCount() - 1) {
              this.skinTile(KeyboardSession.shared.themeSkins(this.settings.dark)[index])
            } else {
              this.moreSkinsTile()
            }
          }
        })
      }
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /**
   * 「输入方式」选择器的一项，显示为面板磁贴：方案的字形放在框里，角上带标记（9、26、双、86…），当前项用强调色并带勾选标记。
   *
   * 英文是平台文本模式，而不是另一个持久化的 Engine 方案，所以它的两个磁贴在 `KeyboardScheme` 中没有对应项；两者的区别只在于字母落在哪种网格上。
   */
  @Builder
  schemeTile(entry: SchemePickerEntry) {
    if (entry.kind === PICKER_ADD_LANGUAGE) {
      PanelTile({
        label: '添加语言',
        iconPath: KeyboardIconPaths.PLUS,
        contentScale: this.tileGrid().scale,
        foreground: this.skin.keyForeground,
        accent: this.skin.toggleForeground,
        ringColor: this.skin.background,
        checkPath: KeyboardIconPaths.CHECK,
        // 语言在「输入」页的「语言与方案」中管理，词库页只管理词条。
        onTap: (): void => {
          this.show(SURFACE_NONE);
          KeyboardSession.shared.openSettings('input');
        }
      })
    } else if (entry.kind === PICKER_SCHEME) {
      this.schemeChoiceTile(this.pickerScheme(entry))
    } else {
      PanelTile({
        label: ENGLISH_TITLES[entry.kind === PICKER_ENGLISH_9 ? 1 : 0],
        glyph: 'EN',
        subscript: entry.kind === PICKER_ENGLISH_9 ? '9' : '26',
        contentScale: this.tileGrid().scale,
        on: this.isCurrentEnglish(entry.kind === PICKER_ENGLISH_9),
        foreground: this.skin.keyForeground,
        accent: this.skin.toggleForeground,
        ringColor: this.skin.background,
        checkPath: KeyboardIconPaths.CHECK,
        accessibilityState: KeyAccessibilityPolicy.current(this.isCurrentEnglish(entry.kind === PICKER_ENGLISH_9)),
        onTap: (): void => this.selectEnglish(entry.kind === PICKER_ENGLISH_9)
      })
    }
  }

  /** `PICKER_SCHEME` 条目携带的方案。 */
  private pickerScheme(entry: SchemePickerEntry): SchemeDefinition {
    return entry.scheme ?? KeyboardScheme.fallback();
  }

  @Builder
  schemeChoiceTile(candidate: SchemeDefinition) {
    PanelTile({
      label: KeyboardScheme.title(candidate, this.wubiProfile),
      glyph: candidate.glyph,
      subscript: KeyboardScheme.badge(candidate, this.wubiProfile),
      contentScale: this.tileGrid().scale,
      on: this.isCurrentScheme(candidate),
      disabled: !this.schemeAvailable(candidate),
      foreground: this.skin.keyForeground,
      accent: this.skin.toggleForeground,
      ringColor: this.skin.background,
      checkPath: KeyboardIconPaths.CHECK,
      accessibilityState: KeyAccessibilityPolicy.current(this.isCurrentScheme(candidate)),
      onTap: (): void => {
        this.selectScheme(candidate);
      }
    })
  }

  /** 选择器最后一格：键盘读不到设置应用里下载的社区皮肤和保存的设计，这一格收起面板、打开设置应用的「皮肤」页，在那里能用全部皮肤。 */
  @Builder
  moreSkinsTile() {
    MoreSkinsTile({
      hair: this.skin.hair,
      foreground: this.skin.keyForeground,
      nameColor: this.skin.keyForeground,
      name: '更多皮肤',
      onTap: (): void => {
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings('skin');
      }
    })
  }

  /** 选择器中的一个皮肤：按其配色绘制的键盘缩略图，下方是名称；选中即应用并关闭选择器。 */
  @Builder
  skinTile(choice: KeyboardSkin) {
    MiniKeyboardPreview({
      keyboardBackground: choice.background,
      keyFill: choice.keyBackground,
      functionKey: choice.functionKeyBackground,
      accent: choice.actionBackground,
      hair: this.skin.hair,
      nameColor: this.skin.keyForeground,
      name: choice.title,
      selected: choice.id === this.skin.id,
      onTap: (): void => this.selectSkin(choice)
    })
  }

  @Builder
  japaneseFace() {
    Column({ space: this.rowSpacing() }) {
      this.japaneseGrid()
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  @Builder
  japaneseGrid() {
    Column({ space: this.rowSpacing() }) {
      ForEach([0, 1, 2], (row: number) => {
        Row({ space: this.keySpacing() }) {
          ForEach([0, 1, 2], (column: number) => {
            this.kanaKey(this.japaneseKeys()[row * 3 + column], row * 3 + column);
          }, (column: number) => `${row}:${column}:${this.faceKey()}`)
        }
        .layoutWeight(1)
        .width('100%')
      }, (row: number) => `${row}:${this.faceKey()}`)

      // The last row is two kana keys and the variant key, which is where the layout puts it: there
      // are eleven kana keys, not twelve, and the twelfth cell is what 小゛゜ occupies.
      //
      // Through a ForEach like the rows above rather than two direct calls: kanaKey takes its key
      // by reference, so handing it a different object does not by itself rebuild the cell, and on
      // the digit layer these two went on reading わ and 、 while the nine above them had become
      // digits. The identity carries faceKey, which is what actually changes.
      Row({ space: this.keySpacing() }) {
        this.variantKey();
        ForEach([9, 10], (index: number) => {
          this.kanaKey(this.japaneseKeys()[index], index);
        }, (index: number) => `${index}:${this.faceKey()}`)
      }
      .layoutWeight(1)
      .width('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }

  /** 小假名、浊音、半浊音. Only means anything once kana have been composed, so it says so. */
  @Builder
  variantKey() {
    Column() {
      Text(JapaneseVariantPolicy.brackets(this.japanese, this.symbols) ? '（）' : '小゛゜')
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.skin.keyForeground)
        .opacity(this.variantKeyLive() ? 1 : 0.35)
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .layoutWeight(1)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .backgroundColor(this.skin.keyBackground)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    // The dimming is the whole explanation of why this key does nothing yet, and dimming is not
    // something a screen reader conveys, so the disabled name says so in words.
    .accessibilityText(JapaneseVariantPolicy.brackets(this.japanese, this.symbols)
      ? JapaneseVariantPolicy.bracketsLabel()
      : JapaneseVariantPolicy.accessibilityLabel(
        JapaneseVariantPolicy.enabled(this.japanese, this.symbols, this.editing.length > 0)))
    .responseRegion(this.keyHitRegion())
    .onClick(() => {
      if (JapaneseVariantPolicy.brackets(this.japanese, this.symbols)) {
        // A strip rather than a menu: bindContextMenu does not show inside an input method panel,
        // which is why every other long-press list on this keyboard is drawn the same way.
        this.managing = -1;
        this.punctuation = false;
        this.brackets = true;
        return;
      }
      if (JapaneseVariantPolicy.enabled(this.japanese, this.symbols, this.editing.length > 0)) {
        this.show(SURFACE_VARIANTS);
      }
    })
  }

  /** Whether the twelfth cell does anything right now, under whichever of its two jobs it has. */
  private variantKeyLive(): boolean {
    return JapaneseVariantPolicy.brackets(this.japanese, this.symbols)
      || JapaneseVariantPolicy.enabled(this.japanese, this.symbols, this.editing.length > 0);
  }

  /**
   * One kana key. The direction is read from the same policy on every move so the user sees the
   * target before lifting, then read once more on release for the committed value. This is the
   * Harmony equivalent of Apple's floating flick guide without placing a popup outside the IME
   * panel, which the system is free to clip.
   */
  @Builder
  kanaKey($$: JapaneseKey, keyIndex: number) {
    Stack() {
      Text($$.kana[DIRECTION_CENTRE])
        .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
        .fontColor(this.skin.keyForeground)
        .textAlign(TextAlign.Center)
        .width('100%')
      if (this.flickKeyIndex === keyIndex && this.flickDirection !== DIRECTION_CENTRE) {
        Text($$.kana[this.flickDirection])
          .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .backgroundColor(this.skin.accent)
          .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
          .padding({ left: 10, right: 10, top: 5, bottom: 5 })
          .accessibilityText(`滑动目标：${$$.kana[this.flickDirection]}`)
      }
    }
    .layoutWeight(1)
    .height('100%')
    .alignContent(Alignment.Center)
    .backgroundColor(this.skin.keyBackground)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onTouch((event: TouchEvent): void => {
      if (event.type === TouchType.Down) {
        this.flickX = event.touches[0].windowX;
        this.flickY = event.touches[0].windowY;
        this.flickKeyIndex = keyIndex;
        this.flickDirection = DIRECTION_CENTRE;
      } else if (event.type === TouchType.Move) {
        this.flickDirection = JapaneseNineKeyLayout.direction(
          event.touches[0].windowX - this.flickX, event.touches[0].windowY - this.flickY,
          FLICK_THRESHOLD_VP);
      } else if (event.type === TouchType.Up) {
        const direction: number = JapaneseNineKeyLayout.direction(
          event.touches[0].windowX - this.flickX, event.touches[0].windowY - this.flickY,
          FLICK_THRESHOLD_VP);
        // One press of the cell per tap or flick, whichever direction it lands on; a direction with no label types nothing and counts nothing, as on iOS.
        if ($$.kana[direction].length > 0) {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.japaneseKana(keyIndex));
        }
        this.sendKana($$, direction);
        this.flickKeyIndex = -1;
        this.flickDirection = DIRECTION_CENTRE;
      } else if (event.type === TouchType.Cancel) {
        this.flickKeyIndex = -1;
        this.flickDirection = DIRECTION_CENTRE;
      }
    })
  }

  /** The post-kana variants: small kana, voiced and semi-voiced forms, grouped as the Engine has them. */
  @Builder
  variantFace() {
    Scroll() {
      Column({ space: KeyboardMetrics.KEY_SPACING_VP }) {
        ForEach(JapaneseNineKeyLayout.variants(), (group: VariantGroup) => {
          Column() {
            Text(group.title)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.skin.keyForeground)
              .opacity(0.55)
              .width('100%')
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP })
            Flex({ wrap: FlexWrap.Wrap }) {
              ForEach(group.kana, (kana: string, index: number) => {
                Text(kana)
                  .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE)
                  .fontColor(this.skin.keyForeground)
                  .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                    right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                  .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
                  .onClick(() => {
                    for (const character of group.strokes[index]) {
                      KeyboardSession.shared.press(character.charCodeAt(0), false);
                    }
                    this.show(SURFACE_NONE);
                  })
              })
            }
            .width('100%')
          }
          .width('100%')
        })
      }
      .width('100%')
    }
    .scrollBar(BarState.Off)
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** The line under an expanded-panel word, the strip's gloss line with a Korean Hanja's 훈음 ahead of the annotation. */
  private expandedGlossLine(index: number): string {
    const row: ExpandedCandidate = this.expanded[index];
    return CandidateGlossPolicy.glossLine(row.hunEum, row.annotation);
  }

  /**
   * What a long press offers, drawn in the panel rather than as a popup menu.
   *
   * bindContextMenu is the obvious way to write this and does not show inside an input method panel,
   * so these follow the same pattern as the pickers: a strip that takes the space above the keys,
   * which is a surface this window certainly owns.
   */
  /**
   * What the long-press strip offers for the candidate being managed.
   *
   * Resolved here because a @Builder body may hold only the UI description. The scheme reaches the
   * policy as the name it compares against rather than the Engine's runtime id: the two never
   * matched, so the rule that withholds dictionary actions from Japanese candidates never fired.
   */
  /** The one action the expanded panel's long press offers, or none when the row lost its gloss. */
  private expandedGlossActions(): ManagementAction[] {
    if (this.expandedGloss < 0 || this.expandedGloss >= this.expanded.length) {
      return [];
    }
    const row: ExpandedCandidate = this.expanded[this.expandedGloss];
    if (!row.annotationIsTranslation) {
      return [];
    }
    const action: ManagementAction | null =
      CandidateManagementAction.touchGlossAction(row.annotation);
    return action === null ? [] : [action];
  }

  /**
   * Type the gloss the expanded panel is offering, then put the panel and the composition away.
   *
   * The same order the strip's menu uses and the one macOS uses: the text first, the composition
   * after. Re-read rather than trusted, because the list can be replaced while the strip is open.
   */
  private insertExpandedGloss(): void {
    const actions: ManagementAction[] = this.expandedGlossActions();
    const row: ExpandedCandidate | null =
      this.expandedGloss >= 0 && this.expandedGloss < this.expanded.length
        ? this.expanded[this.expandedGloss] : null;
    this.expandedGloss = -1;
    if (actions.length === 0 || row === null) {
      return;
    }
    KeyboardSession.shared.insert(row.annotation.trim());
    KeyboardSession.shared.cancel();
    this.show(SURFACE_NONE);
    this.diagnostic = actions[0].announcement;
    this.clearDiagnosticLater();
  }

  private managementActions(): ManagementAction[] {
    const managed: CandidateEntry | null =
      this.managing >= 0 && this.managing < this.candidates.length
        ? this.candidates[this.managing] : null;
    // 「朗读」要读的英文：显示着的英文释义（Engine 的注释不算），或候选本身是英文。
    const spoken: string | null = managed === null ? null : KeyboardSession.shared.readAloudText(
      managed.text, managed.annotationIsTranslation ? managed.annotation : '');
    if (this.touchCandidateMenu) {
      return managed === null ? [] : CandidateManagementAction.touchActions(
        managed.annotation, managed.annotationIsTranslation, managed.text, spoken);
    }
    // The gloss is what the source offers here, and until now this host could show a translation without ever letting anyone type it: macOS has Option and a digit, and a touch keyboard has no modifiers to spare.
    return CandidateManagementAction.managementActions(
      managed === null ? '' : managed.annotation, managed !== null && managed.annotationIsTranslation,
      managed === null ? 0 : managed.fixedPosition,
      CandidateManagementAction.candidateActionsAvailable(
        KeyboardScheme.engineSchemeName(this.engineScheme), managed === null ? 0 : managed.source),
      CandidateManagementAction.hasMultipleCodePoints(managed === null ? '' : managed.text),
      managed === null ? '' : managed.text, spoken);
  }

  @Builder
  overlayStrip(title: string) {
    Row() {
      Text(title)
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.overlayForeground())
        .opacity(0.55)
        .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP })
      // Scrolls sideways: a phone's long press opens the whole management menu, which is wider than the screen, and the title and × stay put either side of it.
      Scroll() {
        Row() {
          if (this.expandedGloss >= 0) {
            // One action, drawn like the others rather than as a sentence: it is a thing to tap.
            ForEach(this.expandedGlossActions(), (item: ManagementAction) => {
              Text(item.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.overlayAccent())
                .padding({ left: 10, right: 10 })
                .onClick(() => this.insertExpandedGloss())
            }, (item: ManagementAction) => `${item.id}:${item.title}`)
          } else if (this.brackets) {
            ForEach(JapaneseNineKeyLayout.digitBrackets(), (bracket: string) => {
              Text(bracket)
                .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE)
                .fontColor(this.overlayForeground())
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .onClick(() => {
                  // Straight in, like every other label on this layer that carries no stroke.
                  KeyboardSession.shared.insert(bracket);
                  this.brackets = false;
                })
            }, (bracket: string) => bracket + this.faceKey())
          } else if (this.punctuation) {
            ForEach(QuickPunctuationPolicy.entries(this.english, this.engineScheme, this.localMode),
              (item: PunctuationEntry) => {
                Text(item.face)
                  .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE)
                  .fontColor(this.overlayForeground())
                  .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                    right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                  .onClick(() => {
                    // The face is what the Engine will print; the key is what it is told.
                    KeyboardSession.shared.punctuation(item.input.charCodeAt(0));
                    this.punctuation = false;
                  })
              }, (item: PunctuationEntry) => item.face + this.faceKey())
          } else {
            ForEach(this.managementActions(), (item: ManagementAction) => {
              Text(item.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(item.available === false ? this.overlayForeground() :
                  item.checked === true ? this.overlayForeground() : this.overlayAccent())
                .opacity(item.available === false ? 0.35 : 1)
                .padding({ left: 10, right: 10 })
                .onClick(() => this.manageCandidate(item, this.managing))
            }, (item: ManagementAction) => `${item.id}:${item.menuItemId}`)
          }
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .align(Alignment.Start)
      .layoutWeight(1)
      Text('×')
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.overlayForeground())
        .padding({ right: KeyboardMetrics.CANDIDATE_PADDING_VP })
        .onClick(() => {
          this.punctuation = false;
          this.brackets = false;
          this.expandedGloss = -1;
          this.managing = -1;
          this.touchCandidateMenu = false;
        })
    }
    .width('100%')
    .height(KeyboardMetrics.ROW_HEIGHT_VP)
    .backgroundColor(this.overlayBackground())
    .borderRadius(this.overlayIsMenu() ? KeyboardMetrics.CANDIDATE_CARD_CORNER_VP : KeyboardMetrics.KEY_CORNER_VP)
    .borderWidth(this.overlayIsMenu() ? 1 : 0)
    .borderColor(this.menuColors.border)
  }

  /** 绘制哪些展开格：引擎列出的候选按索引，已建到 `expandedBuilt`。触屏工具条只滚动当前页，所以网格是触屏键盘查看列表其余部分的唯一途径，往下滚会接着建，从不截断。 */
  private expandedIndices(): number[] {
    const indices: number[] = [];
    const built: number = Math.min(this.expandedBuilt, this.expanded.length);
    for (let index: number = 0; index < built; index++) {
      indices.push(index);
    }
    return indices;
  }

  /** 网格在内边距以内排布格子所用的宽度；全拼九键三栏面板里是中栏的宽度（`expandedGridWidth`）。 */
  private expandedInnerWidth(): number {
    const width: number = this.settings.candidateCanvas === null ? this.expandedGridWidth() : this.canvasExpandedWidth;
    return Math.max(0, width - EXPANDED_PADDING_VP * 2);
  }

  /** 键盘宽度能容纳的列数，每列至少 `EXPANDED_MIN_COLUMN_VP`。 */
  private expandedColumns(): number {
    return Math.max(1,
      Math.floor((this.expandedInnerWidth() + EXPANDED_GAP_VP) / (EXPANDED_MIN_COLUMN_VP + EXPANDED_GAP_VP)));
  }

  /** 各列平分剩余空间后单列的宽度，与设计稿中 `minmax(64px, 1fr)` 轨道的做法相同。 */
  private expandedColumnWidth(): number {
    const columns: number = this.expandedColumns();
    return Math.max(0, (this.expandedInnerWidth() - EXPANDED_GAP_VP * (columns - 1)) / columns);
  }

  /** 展开单元格文字大小：17fp 设计尺寸按候选栏实际字号缩放，该字号已经包含候选栏大小（#10400）。 */
  private expandedFontSize(): number {
    return CandidateScalePolicy.expandedFontSize(EXPANDED_FONT_SIZE, this.settings.candidateFontSize);
  }

  /** 展开单元格高度：44vp 设计高度随候选栏大小和字号比例增长，最低保持 44vp（#10400）。 */
  private expandedCellHeight(): number {
    return CandidateScalePolicy.expandedCellHeight(EXPANDED_CELL_HEIGHT_VP, this.settings.candidateFontSize,
      this.settings.candidateScalePercent);
  }

  /** 一个格子跨几列：普通词一列，长的（如整句或日文转换结果）多列，这样能完整读到，而不是只截到开头几个字。 */
  private expandedSpan(index: number): number {
    if (index < 0 || index >= this.expanded.length) return 1;
    const natural: number = ExpandedCandidateLayout.width(this.expanded[index].text, this.expandedFontSize(),
      EXPANDED_CELL_PADDING_VP, this.expandedInnerWidth());
    return ExpandedCandidateLayout.span(natural, this.expandedColumnWidth(), EXPANDED_GAP_VP, this.expandedColumns());
  }

  /** 格子的绘制宽度：实际占的列加上列间的间距，与分配其所在行时用的度量相同。 */
  private expandedCandidateWidth(span: number): number {
    return span * this.expandedColumnWidth() + (span - 1) * EXPANDED_GAP_VP;
  }

  /** 按行排列的绘制格子：`ExpandedCandidateLayout.rows` 把每格的跨度按 `expandedColumns()` 列装进各行，长候选另起一行而不是溢出，换行前那一格拉宽到行尾。 */
  private expandedRows(): ExpandedCandidateCell[][] {
    const indices: number[] = this.expandedIndices();
    const spans: number[] = [];
    for (let position: number = 0; position < indices.length; position++) {
      spans.push(this.expandedSpan(indices[position]));
    }
    return ExpandedCandidateLayout.rows(indices, spans, this.expandedColumns());
  }

  /** 供 `ForEach` 标识一个展开格的内容：它的位置、文字、是否高亮、是否标成首选纠错以及占几列（行尾那格的宽度会随后面的候选变）。 */
  private expandedCellKey(cell: ExpandedCandidateCell): string {
    return `${cell.index}:${this.expanded[cell.index].text}:${this.expandedHighlighted(cell.index)}:${this.expanded[cell.index].confidentCorrection}:${cell.span}`;
  }

  /** 当工具条显示的是网格所列列表的开头时，网格标出工具条高亮的那个候选。 */
  private expandedHighlighted(index: number): boolean {
    return this.candidatePage === 0 && index < this.candidates.length && this.candidates[index].highlighted
      && this.candidates[index].text === this.expanded[index].text;
  }

  /** 读屏对一个格子朗读的内容：序号、词，以及工具条会在其下方绘制的注释。 */
  private expandedLabel(index: number): string {
    const gloss: string = this.expandedGlossLine(index);
    return KeyAccessibilityPolicy.candidate(index + 1, this.expanded[index].text, '',
      (gloss.length > 0 ? `，${gloss}` : '')
        + CorrectionMarkPolicy.accessibilitySuffix(this.expanded[index].confidentCorrection));
  }

  /**
   * 用全部候选一次性代替按键显示，这是在工具条上滚动查看放不下的候选之外的另一种方式：由按键色格子组成的网格，高亮项着色，下方是「返回」和删除键。
   *
   * 上方的工具条保持原样，展开箭头翻转，所以组合仍然可见，再点箭头会收起网格。长按格子会提供它的注释，与工具条上一样。
   *
   * 全拼九键组字时画成三栏（`nineKeyExpandedFace`），中栏是同一个候选网格。
   */
  @Builder
  expandedFace() {
    if (this.nineKeyPanel()) {
      this.nineKeyExpandedFace()
    } else {
      Column({ space: EXPANDED_GAP_VP }) {
        this.expandedGrid()

        Row({ space: EXPANDED_GAP_VP }) {
          Text('返回')
            .layoutWeight(1)
            .height('100%')
            .borderRadius(EXPANDED_CELL_RADIUS_VP)
            .backgroundColor(this.skin.keySurfaceBackground(false, true))
            .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
            .fontWeight(FUNCTION_LABEL_WEIGHT)
            .fontColor(this.skin.keyLabelColor(false, true))
            .textAlign(TextAlign.Center)
            .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
            .accessibilityText(KeyAccessibilityPolicy.expandCandidates(true))
            .onClick(() => {
              this.show(SURFACE_NONE);
            })
          this.deleteKey('100%', false, 1)
        }
        .width('100%')
        .height(EXPANDED_FOOTER_VP)
      }
      .width('100%')
      .height(this.surfaceHeight())
    }
  }

  /** 展开面板的候选网格，按 `expandedInnerWidth` 排列；整块面板和全拼九键三栏面板的中栏共用。 */
  @Builder
  expandedGrid() {
    Column() {
    CandidateCanvasFrame({ scene: this.settings.candidateCanvas, fixedContent: true,
      fallbackColor: this.settings.candidateColors.surface,
      onPanelReady: (key: string, ready: boolean): void => this.canvasReady('expanded', key, ready) }) {
    Scroll(this.expandedScroller) {
      Column({ space: EXPANDED_GAP_VP }) {
        ForEach(this.expandedRows(), (row: ExpandedCandidateCell[]) => {
          Row({ space: EXPANDED_GAP_VP }) {
            ForEach(row, (cell: ExpandedCandidateCell) => {
              Text(this.expanded[cell.index].text)
                .width(this.expandedCandidateWidth(cell.span))
                .height(this.expandedCellHeight())
                .padding({ left: EXPANDED_CELL_PADDING_VP, right: EXPANDED_CELL_PADDING_VP })
                .borderRadius(EXPANDED_CELL_RADIUS_VP)
                .backgroundColor(this.settings.candidateCanvas !== null
                  ? (this.expandedHighlighted(cell.index) ? this.candidateColors().selected : TRANSPARENT)
                  : this.expandedHighlighted(cell.index) ? this.skin.toggleBackground : this.skin.keyBackground)
                .fontSize(this.expandedFontSize())
                .fontWeight(this.expandedHighlighted(cell.index) ? 600 : 400)
                .fontColor(CorrectionMarkPolicy.cellTextColor(this.expanded[cell.index].confidentCorrection,
                  this.expandedHighlighted(cell.index), this.expandedSurfaceColors()))
                .fontFamily(this.candidateFontFamily())
                .textAlign(TextAlign.Center)
                .maxLines(1)
                .textOverflow({ overflow: TextOverflow.Ellipsis })
                .stateStyles({
                  normal: {
                    .brightness(1)
                  },
                  pressed: {
                    .brightness(EXPANDED_PRESSED_BRIGHTNESS)
                  }
                })
                .accessibilityText(this.expandedLabel(cell.index))
                .onClick(() => {
                  this.chooseExpanded(cell.index);
                })
                // 原版对这里的长按给出与工具条上相同的注释。只给注释：管理操作按页面而不是按这个列表编号，所以这里没有对它们有意义的序号。
                .gesture(LongPressGesture().onAction(() => {
                  if (this.expanded[cell.index].annotationIsTranslation) {
                    this.expandedGloss = cell.index;
                  }
                }))
            }, (cell: ExpandedCandidateCell) => this.expandedCellKey(cell))
          }
          .width('100%')
          .justifyContent(FlexAlign.Start)
        }, (row: ExpandedCandidateCell[]) => row.map((cell: ExpandedCandidateCell): string => this.expandedCellKey(cell)).join('|'))
      }
      .width('100%')
      .padding(EXPANDED_PADDING_VP)
      .onAreaChange((_old: Area, now: Area): void => {
        this.expandedContentHeight = Number(now.height);
        this.appendExpandedIfNeeded();
      })
    }
    .scrollBar(BarState.Off)
    .edgeEffect(EdgeEffect.None)
    .align(Alignment.Top)
    .width('100%')
    .height('100%')
    .onDidScroll((): void => this.appendExpandedIfNeeded())
    .onAreaChange((_old: Area, now: Area): void => {
      this.canvasExpandedWidth = Number(now.width);
      this.expandedViewportHeight = Number(now.height);
      this.appendExpandedIfNeeded();
    })
    }
    }
    .layoutWeight(1)
  }

  /**
   * 全拼九键的展开面板，照常见九键输入法的样子分三栏占满键区：左栏是数字串的拼音（可切成横竖撇点折五个笔画键），中栏是可上下滚动的全部候选，右栏是返回、退格、重输、拼音/笔画、全部/单字。
   *
   * 选拼音、退格和改筛选都不关面板，候选在 onChanged 里重取；选中候选、点返回或组字结束才收起。
   */
  @Builder
  nineKeyExpandedFace() {
    Row({ space: this.keySpacing() }) {
      Column() {
        if (this.nineKeyPanelMode === NineKeyPanelMode.STROKE) {
          Text(this.nineKeyStrokes.length > 0 ? NineKeyPanelPolicy.glyphs(this.nineKeyStrokes) : '笔画')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .textAlign(TextAlign.Center)
            .width('100%')
            .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
          ForEach(NineKeyPanelPolicy.strokes(), (item: NineKeyStroke) => {
            Column() {
              Text(item.glyph)
                .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                .fontColor(this.skin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
            }
            .layoutWeight(1)
            .width('100%')
            .justifyContent(FlexAlign.Center)
            .accessibilityText(`笔画 ${item.name}`)
            .onClick(() => this.appendNineKeyStroke(item.code))
          }, (item: NineKeyStroke) => item.code)
        } else {
          Scroll() {
            Column() {
              ForEach(this.spellings, (spelling: string, index: number) => {
                Text(spelling)
                  .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                  .fontColor(this.skin.accent)
                  .textAlign(TextAlign.Center)
                  .width('100%')
                  .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
                  .accessibilityText(KeyAccessibilityPolicy.spelling(spelling))
                  .onClick(() => KeyboardSession.shared.chooseSpelling(index))
              }, (spelling: string, index: number) => `${index}:${spelling}`)
            }
            .width('100%')
          }
          .scrollBar(BarState.Off)
          .width('100%')
          .height('100%')
        }
      }
      .width(`${NINE_KEY_PANEL_LEFT_PERCENT}%`)
      .height('100%')
      .backgroundColor(this.skin.keySurfaceBackground(false, true))
      .borderRadius(KeyboardMetrics.KEY_CORNER_VP)

      Column() {
        this.expandedGrid()
      }
      .layoutWeight(1)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        this.nineKeyControl('返回', '收起候选面板', () => this.show(SURFACE_NONE));
        this.nineKeyControl('⌫', KeyAccessibilityPolicy.delete(), () => this.nineKeyPanelBackspace());
        this.nineKeyControl('重输', '清空输入', () => {
          KeyboardSession.shared.discardComposition();
          this.show(SURFACE_NONE);
        });
        this.nineKeyControl(NineKeyPanelPolicy.modeTitle(this.nineKeyPanelMode),
          NineKeyPanelPolicy.modeLabel(this.nineKeyPanelMode), () => this.toggleNineKeyPanelMode());
        this.nineKeyControl(NineKeyPanelPolicy.singleCharacterTitle(this.nineKeySingleCharacter),
          NineKeyPanelPolicy.singleCharacterLabel(this.nineKeySingleCharacter),
          () => KeyboardSession.shared.setNineKeyFilter(!this.nineKeySingleCharacter, this.nineKeyStrokes));
      }
      .width(`${NINE_KEY_PANEL_RIGHT_PERCENT}%`)
      .height('100%')
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** 功能面板开关的当前状态；操作类磁贴从不处于开启状态，除非其面板已打开。 */
  private functionTileOn(id: string): boolean {
    switch (id) {
      case FunctionPanelPolicy.FULL_WIDTH:
        return this.fullWidth;
      case FunctionPanelPolicy.CHINESE_PUNCTUATION:
        return this.chinesePunctuation;
      case FunctionPanelPolicy.FUZZY_PINYIN:
        return this.fuzzyPinyin;
      case FunctionPanelPolicy.TRADITIONAL:
        return this.traditional;
      case FunctionPanelPolicy.HANDWRITING:
        return this.handwritingSelected();
      case FunctionPanelPolicy.KEY_SOUND:
        return this.feedback.sound;
      case FunctionPanelPolicy.VIBRATION:
        return this.feedback.haptics;
      case FunctionPanelPolicy.ONE_HAND:
        return this.feedback.oneHanded !== OneHandedMode.OFF;
      case FunctionPanelPolicy.FLOATING:
        return this.feedback.floating;
      case FunctionPanelPolicy.PRIVACY:
        return this.feedback.incognito;
      case FunctionPanelPolicy.TRANSLATION:
        return this.translations;
      case FunctionPanelPolicy.VOICE:
        return this.voiceBusy;
      case FunctionPanelPolicy.AI_POLISH:
        return this.polishBusy;
      default:
        return false;
    }
  }

  /** 磁贴当前是否可用：固定标点锁定时中文标点切换不起作用，繁体输出对日文没有意义，手写需要带中文方案的版本，语音和 AI 润色需要各自的服务商，强度需要开启振动。 */
  private functionTileEnabled(id: string): boolean {
    switch (id) {
      case FunctionPanelPolicy.CHINESE_PUNCTUATION:
        // 锁定时 `togglePunctuation` 什么都不做，磁贴照常可点只会点了又弹回；置灰与 Windows 托盘在锁定时禁用「中文标点」相同，读屏也会报不可用。
        return !this.punctuationLocked;
      case FunctionPanelPolicy.TRADITIONAL:
        return !this.japanese;
      case FunctionPanelPolicy.HANDWRITING:
        return KeyboardScheme.offeredBy(KeyboardScheme.HANDWRITING);
      case FunctionPanelPolicy.VOICE:
        return this.voiceEnabled;
      case FunctionPanelPolicy.AI_POLISH:
        // 仅在配置了润色服务商时提供。改写会发到用户自己的服务，而一个总是失败的磁贴只会教会用户忽略这个面板。
        return this.polishConfigured;
      case FunctionPanelPolicy.VIBRATION_STRENGTH:
        return this.feedback.haptics;
      case FunctionPanelPolicy.ONE_HAND:
        // 2in1 不绘制可收窄的触屏按键，所以该磁贴在那里不起作用；分离式键盘画着或键盘浮动时单手模式也不生效，同样置灰，免得点了没有反应，与 Android 相同。
        return this.touchDesign() && !this.splitDrawn() && !this.floatingDrawn();
      case FunctionPanelPolicy.FLOATING:
        // 2in1 的面板是跟随光标的候选窗，不浮动。分离式键盘画着时照常可用：打开浮动后分离式键盘不再画（`splitDrawn`），与 Android 相同。
        return this.touchDesign();
      default:
        return true;
    }
  }

  /** 磁贴的作用。开关原地切换并保持面板打开；其余磁贴在前往其目标时关闭面板。 */
  private runFunctionTile(id: string): void {
    if (!this.functionTileEnabled(id)) {
      return;
    }
    switch (id) {
      case FunctionPanelPolicy.FULL_WIDTH:
        this.fullWidth = KeyboardSession.shared.toggleCharacterWidth();
        break;
      case FunctionPanelPolicy.CHINESE_PUNCTUATION:
        this.chinesePunctuation = KeyboardSession.shared.togglePunctuation();
        break;
      case FunctionPanelPolicy.FUZZY_PINYIN:
        this.fuzzyPinyin = KeyboardSession.shared.toggleFuzzyPinyin();
        break;
      case FunctionPanelPolicy.TRADITIONAL:
        this.toggleTraditional();
        break;
      case FunctionPanelPolicy.HANDWRITING:
        this.toggleHandwritingScheme();
        this.show(SURFACE_NONE);
        break;
      case FunctionPanelPolicy.DICTIONARY:
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings('dictionary');
        break;
      case FunctionPanelPolicy.KEYBOARD_HEIGHT:
        this.openAdjustMode();
        break;
      case FunctionPanelPolicy.SETTINGS:
        // 原版的托盘和工具栏让打字时一键就能打开整个设置应用；手机两者都没有，这就是它进入设置的途径。
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings();
        break;
      case FunctionPanelPolicy.KEY_SOUND:
        this.setFeedback(this.feedbackWith(!this.feedback.sound, this.feedback.haptics, this.feedback.strength));
        break;
      case FunctionPanelPolicy.VIBRATION:
        this.setFeedback(this.feedbackWith(this.feedback.sound, !this.feedback.haptics, this.feedback.strength));
        break;
      case FunctionPanelPolicy.ONE_HAND:
        this.setOneHanded(OneHandedPolicy.toggled(this.feedback.oneHanded));
        break;
      case FunctionPanelPolicy.FLOATING:
        this.setFloating(!this.feedback.floating);
        break;
      case FunctionPanelPolicy.PRIVACY:
        this.togglePrivacyMode();
        break;
      case FunctionPanelPolicy.FEEDBACK:
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings('feedback');
        break;
      case FunctionPanelPolicy.ABOUT:
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings('about');
        break;
      case FunctionPanelPolicy.PROMOTION:
        this.show(SURFACE_NONE);
        KeyboardSession.shared.commitPhrase(PROMOTION_TEXT);
        break;
      case FunctionPanelPolicy.TRANSLATION:
        this.toggleTranslations();
        break;
      case FunctionPanelPolicy.SYMBOLS:
        this.openSymbolSurface();
        break;
      case FunctionPanelPolicy.LOCAL_INPUT:
        // 进入下一层，而不是在键盘自己的按键行上放八种模式，这是 iOS 最终采用的布局。
        this.localInputModes = LocalInputMode.touchModes(KeyboardSession.shared.offersCommandMode());
        this.show(SURFACE_MODES);
        break;
      case FunctionPanelPolicy.VOICE:
        this.show(SURFACE_VOICE);
        if (!this.voiceBusy && this.voiceText.length === 0) {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_VOICE);
          this.startVoice();
        }
        break;
      case FunctionPanelPolicy.SMART_REPLY:
        // 高情商回复是工具而不是输入方案：任何方案下都从这里打开回复面板，点面板里的「完成」或工具栏的标志回到原来的键盘。
        this.show(SURFACE_REPLY);
        break;
      case FunctionPanelPolicy.AI_POLISH:
        this.openPolish();
        break;
      case FunctionPanelPolicy.VIBRATION_STRENGTH:
        // 一个磁贴显示级别并循环切换：做成三个会单独占一行。
        this.setFeedback(this.feedbackWith(this.feedback.sound, this.feedback.haptics,
          KeyboardFeedback.nextStrength(this.feedback.strength)));
        break;
      default:
        break;
    }
  }

  /** 一个功能面板磁贴，根据 `FunctionPanelPolicy` 的条目和视图中对应的状态绘制。 */
  @Builder
  functionTile(item: FunctionPanelItem) {
    PanelTile({
      label: FunctionPanelPolicy.label(item, this.feedback.strength),
      glyph: item.glyph ?? '',
      iconPath: item.icon ?? '',
      contentScale: this.tileGrid().scale,
      on: this.functionTileOn(item.id),
      disabled: !this.functionTileEnabled(item.id),
      foreground: this.skin.keyForeground,
      accent: this.skin.toggleForeground,
      ringColor: this.skin.background,
      checkPath: KeyboardIconPaths.CHECK,
      spokenLabel: item.accessibilityLabel,
      accessibilityState: FunctionPanelPolicy.state(item, this.functionTileOn(item.id),
        this.functionTileEnabled(item.id)),
      onTap: (): void => this.runFunctionTile(item.id),
      onHold: item.id === FunctionPanelPolicy.ONE_HAND ? (): void => this.swapOneHandedSide() : null
    })
  }

  /** 「单手模式」磁贴的长按和侧栏的换边按钮：按键移到另一侧边缘；模式原本关闭时则开启并靠左边缘，与 Android 的长按一致。 */
  private swapOneHandedSide(): void {
    if (!this.functionTileEnabled(FunctionPanelPolicy.ONE_HAND)) {
      return;
    }
    this.setOneHanded(OneHandedPolicy.swapped(this.feedback.oneHanded));
  }

  /**
   * 工具栏标志打开的功能面板：`FunctionPanelPolicy` 的磁贴每行四个、每页八个，横向翻页并在下方显示圆点，每次由标志打开时回到第一页。
   *
   * 「本地输入」进入下一层，到它自己的模式列表，而不是在键盘自己的按键行上放八种模式，这是 iOS 最终采用的布局。
   */
  @Builder
  toolsFace() {
    Column() {
      if (this.surface === SURFACE_MODES) {
        Row() {
          Text('‹ 返回')
            .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
            .fontColor(this.skin.accent)
            .onClick(() => {
              this.show(SURFACE_TOOLS);
            })
          Blank()
        }
        .width('100%')
        .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
        .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
          right: KeyboardMetrics.CANDIDATE_PADDING_VP })

        Scroll() {
          Column() {
            Grid() {
              ForEach(this.localInputModes, (mode: LocalInputModeDefinition) => {
                GridItem() {
                  this.toolCard(mode.title, mode.trigger, null, false, true, () => {
                    // 字母触发词是 Engine 认作进入那个模式的大写字母；「指令」的 / 按普通标点键送。
                    KeyboardSession.shared.press(mode.trigger.charCodeAt(0),
                      LocalInputMode.entersWithShift(mode));
                    this.show(SURFACE_NONE);
                  });
                }
              }, (mode: LocalInputModeDefinition) => mode.id)
            }
            .columnsTemplate(LocalInputMode.touchColumns(this.localInputModes.length) === 4
              ? '1fr 1fr 1fr 1fr' : '1fr 1fr 1fr')
            .width('100%')
          }
          .width('100%')
        }
        .scrollBar(BarState.Off)
        .width('100%')
        .layoutWeight(1)
      } else {
        PagedTiles({
          count: this.functionPanelItems().length,
          pageSize: this.tilePageSize(),
          rowHeight: this.tileRowHeight(this.functionPanelItems().length),
          rowsGap: this.tileGrid().rowsGap,
          topPadding: TILE_TOP_PADDING_VP,
          resetToken: this.panelResetToken,
          accent: this.skin.toggleForeground,
          hair: this.skin.hair,
          tile: (index: number): void => {
            this.functionTile(this.functionPanelItems()[index])
          }
        })
      }
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** 手机和平板不显示本地输入入口；2in1 保留完整的功能面板。 */
  private functionPanelItems(): FunctionPanelItem[] {
    const allItems: FunctionPanelItem[] = FunctionPanelPolicy.items(AiGenerationGate.enabled());
    return KeyboardFormFactor.isDesktop()
      ? allItems
      : FunctionPanelPolicy.items(AiGenerationGate.enabled(), false);
  }

  /** 「常用语」面板：用户的常用语，点一下即插入。 */
  @Builder
  phraseFace() {
    Column() {
      PhraseList({
        phrases: this.phrases,
        state: this.phraseState,
        foreground: this.skin.keyForeground,
        secondary: this.skin.secondary,
        hair: this.skin.hair,
        onPick: (text: string): void => this.pickPhrase(text)
      })
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** One card: an icon or a short label, a title, and a state line for the ones that toggle. */
  @Builder
  toolCard(title: string, label: string | null, icon: Resource | null, on: boolean,
           enabled: boolean, action: () => void) {
    Row() {
      if (icon !== null) {
        Image(icon)
          .width(TOOL_ICON_VP)
          .height(TOOL_ICON_VP)
          .fillColor(this.skin.keyForeground)
          .margin({ right: 8 })
      } else if (label !== null) {
        Text(label)
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .margin({ right: 8 })
      }
      Column() {
        Text(title)
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .width('100%')
        if (icon !== null && (title === '繁体输出' || title === '按键音' || title === '按键振动')) {
          Text(on ? '已开启' : '已关闭')
            .fontSize(KEY_HINT_FONT_SIZE)
            .fontColor(this.skin.keyForeground)
            .opacity(0.55)
            .width('100%')
        }
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .padding({ left: 10, right: 10, top: 7, bottom: 7 })
    .margin(3)
    .backgroundColor(on ? this.skin.tintedAccent(SELECTED_CARD_ALPHA) : this.skin.keyBackground)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .opacity(enabled ? 1 : 0.4)
    .onClick(() => {
      if (enabled) {
        action();
      }
    })
  }

  /**
   * 触屏剪贴板面板，与设计稿和 Android 一致：没有标题，也没有「返回」，因为工具栏的「剪贴板」会收起它。一个小行放「本机」/「云端」标签和当前分区的操作：本机历史是「保存当前」和「清空」，云端是「刷新」；本宿主保留「保存当前」，因为与 Android 不同，它不在后台记录任何内容（`captureClipboard`）。下面每条记录一张卡片。
   */
  @Builder
  clipboardFace() {
    Column({ space: CLIP_CARD_GAP_VP }) {
      Row({ space: CLIP_HEADER_ACTION_GAP_VP }) {
        if (this.clipSelecting) {
          // 多选时这一行换成计数、「取消」和「插入」，先后和 Android 多选顶行一致；「插入」只在选中 1 到 4 项时可点，不可点时标成禁用，读屏读得出它点不了。导出和删除是 Android 独有的，这里不提供。
          Text(`${this.clipSelection.length} 项已选择`)
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.keyForeground)
          Blank()
          Text('取消')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .accessibilityText('退出多选')
            .onClick(() => this.endClipSelection())
          Text('插入')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .enabled(ClipboardSelectionPolicy.canInsert(this.clipSelection.length))
            .opacity(ClipboardSelectionPolicy.canInsert(this.clipSelection.length) ? 1 : 0.4)
            .accessibilityText(this.clipSelection.length > ClipboardSelectionPolicy.MAX_INSERTIONS
              ? `插入不可用，${ClipboardSelectionPolicy.INSERTION_LIMIT_HINT}` : '按序号插入选中的剪贴板记录')
            .onClick(() => this.insertClipSelection())
        } else {
          // 密码框完全不显示标签：云端分区是唯一的另一个分区，而它在那里从不提供。
          if (this.cloudVisible) {
            Text('本机')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.cloudTabActive() ? this.skin.keyForeground : this.skin.accent)
              .opacity(this.cloudTabActive() ? CLIP_IDLE_TAB_OPACITY : 1)
              .accessibilityText(this.cloudTabActive() ? '剪贴板分类 本机' : '剪贴板分类 本机，已选中')
              .onClick(() => this.selectClipboardTab(CLIPBOARD_TAB_LOCAL))
            Text(CLOUD_CLIPBOARD_TAB)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.cloudTabActive() ? this.skin.accent : this.skin.keyForeground)
              .opacity(this.cloudTabActive() ? 1 : CLIP_IDLE_TAB_OPACITY)
              .accessibilityText(this.cloudTabActive() ? `剪贴板分类 ${CLOUD_CLIPBOARD_TAB}，已选中`
                : `剪贴板分类 ${CLOUD_CLIPBOARD_TAB}`)
              .onClick(() => this.selectClipboardTab(CLIPBOARD_TAB_CLOUD))
          }
          Blank()
          if (this.cloudTabActive()) {
            Text('刷新')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.skin.accent)
              .opacity(this.cloudLoading ? 0.4 : 1)
              .accessibilityText('刷新云剪贴板')
              .onClick(() => {
                if (!this.cloudLoading) {
                  this.refreshCloudClipboard();
                }
              })
          } else {
            // 保存、多选和清空作用于本机历史，所以云端分区不提供它们。
            Text('保存当前')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.skin.accent)
              .accessibilityText('保存剪贴板当前内容')
              .onClick(() => this.captureClipboard())
            if (this.clipboardEnabled && this.clips.length > 0) {
              Text('多选')
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.skin.accent)
                .accessibilityText('多选剪贴板记录')
                .onClick(() => this.beginClipSelection())
            }
            Text(this.confirmingClear ? '确认清空' : '清空')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.skin.accent)
              .onClick(() => this.clearClipboard())
          }
        }
      }
      .width('100%')
      .height(CLIP_HEADER_VP)
      .padding({ left: CLIP_HEADER_SIDE_PADDING_VP, right: CLIP_HEADER_SIDE_PADDING_VP })

      this.clipboardEntries()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** 触屏剪贴板卡片下方的一行：已固定 · 本机 · 2 分钟前（`ClipboardHistoryPolicy.localMeta`）。 */
  private clipMeta(entry: ClipboardHistoryItem): string {
    return ClipboardHistoryPolicy.localMeta(entry.pinned, entry.at, Date.now());
  }

  private cloudClipMeta(entry: CloudClipboardItem): string {
    return ClipboardHistoryPolicy.cloudMeta(entry.updatedAt, Date.now());
  }

  /** 触屏长按正在管理的条目，前提是它仍在历史中。 */
  private managedClip(): ClipboardHistoryItem | undefined {
    return this.clips.find((entry: ClipboardHistoryItem): boolean => entry.text === this.clipSendTarget);
  }

  /** 在本机卡片上触屏长按：管理行提供「固定」或「取消固定」、「删除」，以及在有云端分区时的「发到云剪贴板」，即 Android 的剪贴板条目菜单。 */
  private manageClip(text: string): void {
    this.clipSendTarget = text;
  }

  /** 管理行的「发到云剪贴板」：能发送时发送，不能时说明原因。 */
  private sendManagedClip(): void {
    const block: string | null = this.cloudSendBlock();
    if (block !== null) {
      this.clipboardNotice = block;
      return;
    }
    this.sendClipToCloud();
  }

  /** 管理行的「固定」或「取消固定」，之后收起该行。 */
  private pinManagedClip(): void {
    const entry: ClipboardHistoryItem | undefined = this.managedClip();
    this.clipSendTarget = '';
    if (entry !== undefined) {
      this.applyClipboardMutation(KeyboardSession.shared.setClipboardPinned(entry.text, !entry.pinned));
    }
  }

  /** 管理行的「删除」，之后收起该行。 */
  private removeManagedClip(): void {
    const entry: ClipboardHistoryItem | undefined = this.managedClip();
    this.clipSendTarget = '';
    if (entry !== undefined) {
      this.applyClipboardMutation(KeyboardSession.shared.removeClipboardHistory(entry.text));
    }
  }

  /** 触屏剪贴板卡片：正文最多两行，下方一小行说明是否已固定、来源和时间。`selected` 是多选中选中的那几张，卡片铺强调色，文字换成强调色上的文字色。 */
  @Builder
  clipCardBody(text: string, meta: string, selected: boolean) {
    Column({ space: CLIP_CARD_LINE_GAP_VP }) {
      Text(text)
        .fontSize(CLIP_CARD_FONT_SIZE)
        .lineHeight(CLIP_CARD_LINE_HEIGHT_VP)
        .fontColor(selected ? this.clipboardSkin().onAccent : this.clipboardSkin().keyForeground)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .width('100%')
      if (meta.length > 0) {
        Text(meta)
          .fontSize(CLIP_META_FONT_SIZE)
          .fontColor(selected ? this.clipboardSkin().onAccent : this.clipboardSkin().secondary)
          .opacity(selected ? 0.8 : 1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .width('100%')
      }
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
  }

  /** The section tabs, the notice and the entries, shared by the phone's clipboard surface and the 2in1 emoji surface's clipboard tab. */
  @Builder
  clipboardEntries() {
      // 「本机」和「云端」。密码框完全不显示标签：云端分区是唯一的另一个分区，而它在那里从不提供。触屏上它们在 `clipboardFace` 自己的那一行里。
      if (this.cloudVisible && !this.touchDesign()) {
        Row() {
          Text('本机')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.cloudTabActive() ? this.clipboardSkin().keyForeground : this.clipboardSkin().accent)
            .padding({ right: 16 })
            .onClick(() => this.selectClipboardTab(CLIPBOARD_TAB_LOCAL))
          Text(CLOUD_CLIPBOARD_TAB)
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.cloudTabActive() ? this.clipboardSkin().accent : this.clipboardSkin().keyForeground)
            .onClick(() => this.selectClipboardTab(CLIPBOARD_TAB_CLOUD))
          Blank()
          if (this.cloudTabActive()) {
            Text('刷新')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().accent)
              .opacity(this.cloudLoading ? 0.4 : 1)
              .onClick(() => {
                if (!this.cloudLoading) {
                  this.refreshCloudClipboard();
                }
              })
          }
        }
        .width('100%')
        .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
          right: KeyboardMetrics.CANDIDATE_PADDING_VP, top: 4, bottom: 4 })
      }

      if (this.cloudTabActive()) {
        if (this.cloudNotice().length > 0) {
          Text(this.cloudNotice())
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.clipboardSkin().secondary)
            .maxLines(2)
            .width('100%')
            .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
              right: KeyboardMetrics.CANDIDATE_PADDING_VP })
        }

        if (this.emojiQuery.length > 0 && this.cloudClips.length > 0 && this.visibleCloudClips().length === 0) {
          Text('没有匹配的剪贴板记录')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.clipboardSkin().secondary)
            .width('100%')
            .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
              right: KeyboardMetrics.CANDIDATE_PADDING_VP })
        }

        List({ space: this.touchDesign() ? CLIP_CARD_GAP_VP : KeyboardMetrics.KEY_SPACING_VP,
          scroller: this.clipListScroller }) {
          ForEach(this.visibleCloudClips(), (entry: CloudClipboardItem, index: number) => {
            ListItem() {
              if (this.touchDesign()) {
                Column() {
                  this.clipCardBody(entry.text, this.cloudClipMeta(entry), false)
                }
                .width('100%')
                .padding({ left: CLIP_CARD_PADDING_H_VP, right: CLIP_CARD_PADDING_H_VP,
                  top: CLIP_CARD_PADDING_V_VP, bottom: CLIP_CARD_PADDING_V_VP })
                .backgroundColor(this.clipboardSkin().keyBackground)
                .borderRadius(CLIP_CARD_RADIUS_VP)
                .accessibilityText(`点按插入云剪贴板记录：${entry.text}`)
                .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
                .animation({ duration: KEY_PRESS_MS })
                .onClick(() => this.insertCloudClip(entry.text))
              } else {
              Row() {
                Text(entry.text)
                  .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                  .fontColor(this.clipboardSkin().keyForeground)
                  .maxLines(2)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
                  .layoutWeight(1)
              }
              .width('100%')
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                right: KeyboardMetrics.CANDIDATE_PADDING_VP, top: 6, bottom: 6 })
              .backgroundColor(this.clipboardSkin().keyBackground)
              .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
              .border({ width: this.panelKeySelected(index) ? 2 : 0, color: this.clipboardSkin().accent,
                radius: KeyboardMetrics.KEY_CORNER_VP })
              .onClick(() => this.insertCloudClip(entry.text))
              }
            }
          }, (entry: CloudClipboardItem, index: number): string => `${entry.id}:${index}:${entry.updatedAt}`)
        }
        .scrollBar(BarState.Off)
        .width('100%')
        .layoutWeight(1)
      } else {
        if (this.clipboardNotice.length > 0) {
          Row({ space: CLIP_HEADER_ACTION_GAP_VP }) {
            Text(this.clipboardNotice)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().secondary)
              .maxLines(2)
              .layoutWeight(1)
            if (!this.clipboardEnabled) {
              Text('开启')
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.clipboardSkin().accent)
                .accessibilityText('开启剪贴板历史')
                .onClick(() => this.enableClipboardHistory())
            }
          }
          .width('100%')
          .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
            right: KeyboardMetrics.CANDIDATE_PADDING_VP })
        }

        if (this.emojiQuery.length > 0 && this.visibleClips().length === 0) {
          Text('没有匹配的剪贴板记录')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.clipboardSkin().secondary)
            .width('100%')
            .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
              right: KeyboardMetrics.CANDIDATE_PADDING_VP })
        }

        // 触屏上长按在面板内就地管理条目，因为 `bindContextMenu` 在输入法面板里不显示：即 Android 菜单中的「固定」或「取消固定」、「删除」，以及在有云端分区时的「发到云剪贴板」。
        if (this.touchDesign() && this.managedClip() !== undefined) {
          Row({ space: CLIP_HEADER_ACTION_GAP_VP }) {
            Text(this.managedClip()!.pinned ? '取消固定' : '固定')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().accent)
              .onClick(() => this.pinManagedClip())
            Text('删除')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().accent)
              .onClick(() => this.removeManagedClip())
            if (this.cloudVisible) {
              Text(CLOUD_CLIPBOARD_SEND)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.clipboardSkin().accent)
                .opacity(this.cloudSendBlock() === null && !this.cloudSending ? 1 : 0.4)
                .onClick(() => this.sendManagedClip())
            }
            Text(this.managedClip()!.text)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().secondary)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .layoutWeight(1)
            Text('取消')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().keyForeground)
              .opacity(0.55)
              .onClick(() => {
                this.clipSendTarget = '';
              })
          }
          .width('100%')
          .padding({ left: CLIP_HEADER_SIDE_PADDING_VP, right: CLIP_HEADER_SIDE_PADDING_VP })
        }

        // A long press on an entry offers it to the cloud clipboard here, in the panel: bindContextMenu does not show inside an input method panel.
        if (!this.touchDesign() && this.clipSendTarget.length > 0 && this.cloudVisible) {
          Row() {
            Text(CLOUD_CLIPBOARD_SEND)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().accent)
              .opacity(this.cloudSendBlock() === null && !this.cloudSending ? 1 : 0.4)
              .padding({ right: 12 })
              .onClick(() => this.sendClipToCloud())
            Text(this.cloudSendHint())
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().secondary)
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .layoutWeight(1)
            Text('取消')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.clipboardSkin().keyForeground)
              .opacity(0.55)
              .padding({ left: 12 })
              .onClick(() => {
                this.invalidateCloudSend();
                this.clipSendTarget = '';
              })
          }
          .width('100%')
          .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
            right: KeyboardMetrics.CANDIDATE_PADDING_VP, top: 4, bottom: 4 })
        }

        // A List rather than a Column in a Scroll so the 2in1 arrows can bring the selected entry into view.
        List({ space: this.touchDesign() ? CLIP_CARD_GAP_VP : KeyboardMetrics.KEY_SPACING_VP,
          scroller: this.clipListScroller }) {
            ForEach(this.visibleClips(), (entry: ClipboardHistoryItem, index: number) => {
              ListItem() {
              if (this.touchDesign() && this.clipSelecting) {
                // 多选：选中的卡片铺强调色，右上角是点选先后的序号；取消一项后，后面各项的序号随这次重画依次前移（#10411）。长按不再打开管理行。
                Stack({ alignContent: Alignment.TopEnd }) {
                  Column() {
                    this.clipCardBody(entry.text, this.clipMeta(entry), this.clipOrdinal(entry.text) >= 0)
                  }
                  .width('100%')
                  .padding({ left: CLIP_CARD_PADDING_H_VP, right: CLIP_CARD_PADDING_H_VP + CLIP_BADGE_SPACE_VP,
                    top: CLIP_CARD_PADDING_V_VP, bottom: CLIP_CARD_PADDING_V_VP })
                  if (this.clipOrdinal(entry.text) >= 0) {
                    Text(ClipboardSelectionPolicy.ordinalLabel(this.clipOrdinal(entry.text)))
                      .fontSize(CLIP_BADGE_FONT_SIZE)
                      .fontColor(this.clipboardSkin().onAccent)
                      .margin({ top: 4, right: 6 })
                      .accessibilityLevel('no')
                  }
                }
                .width('100%')
                .backgroundColor(this.clipOrdinal(entry.text) >= 0
                  ? this.clipboardSkin().accent : this.clipboardSkin().keyBackground)
                .borderRadius(CLIP_CARD_RADIUS_VP)
                .accessibilityText(`${ClipboardSelectionPolicy.selectedDescription(this.clipOrdinal(entry.text))}点按选择剪贴板记录：${entry.text}`)
                .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
                .animation({ duration: KEY_PRESS_MS })
                .onClick(() => this.toggleClipSelection(entry.text))
              } else if (this.touchDesign()) {
                Column() {
                  this.clipCardBody(entry.text, this.clipMeta(entry), false)
                }
                .width('100%')
                .padding({ left: CLIP_CARD_PADDING_H_VP, right: CLIP_CARD_PADDING_H_VP,
                  top: CLIP_CARD_PADDING_V_VP, bottom: CLIP_CARD_PADDING_V_VP })
                .backgroundColor(this.clipboardSkin().keyBackground)
                .borderRadius(CLIP_CARD_RADIUS_VP)
                .accessibilityText(`${entry.pinned ? '已固定；' : ''}点按插入剪贴板记录，长按管理：${entry.text}`)
                .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
                .animation({ duration: KEY_PRESS_MS })
                .onClick(() => KeyboardSession.shared.insert(entry.text))
                .gesture(LongPressGesture().onAction(() => this.manageClip(entry.text)))
              } else {
              Row() {
                Text(entry.text)
                  .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                  .fontColor(this.clipboardSkin().keyForeground)
                  .maxLines(2)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
                  .layoutWeight(1)
                  .onClick(() => KeyboardSession.shared.insert(entry.text))
                  .gesture(LongPressGesture().onAction(() => this.offerCloudSend(entry.text)))
                  .onHover((isHover: boolean): void => this.hoverItem(`clip:${entry.text}`,
                    EmojiPanelTooltipPolicy.clipboardText(entry.text), isHover))
                  .bindPopup(this.hoverTipKey === `clip:${entry.text}`, this.hoverTipOptions(`clip:${entry.text}`))
                Text(entry.pinned ? '已固定' : '固定')
                  .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                  .fontColor(entry.pinned ? this.clipboardSkin().accent : this.clipboardSkin().keyForeground)
                  .opacity(entry.pinned ? 1 : 0.55)
                  .padding({ left: 8, right: 8 })
                  .onClick(() => this.applyClipboardMutation(
                    KeyboardSession.shared.setClipboardPinned(entry.text, !entry.pinned)))
                Text('删除')
                  .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                  .fontColor(this.clipboardSkin().keyForeground)
                  .opacity(0.55)
                  .onClick(() => this.applyClipboardMutation(
                    KeyboardSession.shared.removeClipboardHistory(entry.text)))
              }
              .width('100%')
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                right: KeyboardMetrics.CANDIDATE_PADDING_VP, top: 6, bottom: 6 })
              .backgroundColor(this.clipboardSkin().keyBackground)
              .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
              .border({ width: this.panelKeySelected(index) ? 2 : 0, color: this.clipboardSkin().accent,
                radius: KeyboardMetrics.KEY_CORNER_VP })
              }
              }
            // 键里带上多选状态和序号：`clipCardBody` 的参数按值传入，不随状态刷新，序号或选中状态变了要让这一项整个重建。
            }, (entry: ClipboardHistoryItem): string =>
              `${entry.text}:${entry.pinned}:${entry.at}:${this.clipSelecting ? this.clipOrdinal(entry.text) : ''}`)
        }
        .scrollBar(BarState.Off)
        .width('100%')
        .layoutWeight(1)
      }
  }

  /** The hover tooltip itself, in the Windows tooltip's dark plate whatever the panel's theme. */
  @Builder
  hoverTipBody() {
    Text(this.hoverTip)
      .fontSize(13)
      .fontColor('#F5F5F7')
      .maxLines(CLIPBOARD_TOOLTIP_MAX_LINES)
      .textOverflow({ overflow: TextOverflow.Ellipsis })
      .constraintSize({ maxWidth: 360 })
      .padding({ left: 14, right: 14, top: 10, bottom: 10 })
  }

  private hoverTipOptions(key: string): CustomPopupOptions {
    return {
      builder: this.hoverTipBody,
      placement: Placement.Top,
      enableArrow: false,
      popupColor: '#F52B2B33',
      autoCancel: true,
      onStateChange: (event: PopupStateChangeParam): void => {
        // Only the item that owns the tooltip may clear it; every other item's popup reports hidden too.
        if (!event.isVisible && this.hoverTipKey === key) {
          this.hoverTipKey = '';
        }
      }
    };
  }

  /** Says what typing will search, the way the Windows box's placeholder follows the page. */
  private emojiSearchPlaceholder(): string {
    if (this.emojiPanel === CLIPBOARD_PANEL) {
      return '键入以搜索剪贴板';
    }
    if (this.emojiPanel === KAOMOJI_PANEL) {
      return '键入以搜索颜文字';
    }
    return this.emojiPanel === SYMBOLS_PANEL ? '键入以搜索符号' : '键入以搜索表情';
  }

  /** Whether an item carries the 2in1 keyboard selection; a phone panel is touched, so it never shows one. */
  private panelKeySelected(index: number): boolean {
    return KeyboardFormFactor.isDesktop() && this.desktop === DesktopSurface.EMOJI
      && index === this.emojiSelected;
  }

  /** 表情面板：触屏上是设计稿的网格，下方一行是 ABC / 分类 / ⌫；2in1 上是 Windows 风格的面板，带标签和搜索。 */
  @Builder
  emojiFace() {
    if (this.touchDesign()) {
      this.touchEmojiFace()
    } else {
      this.desktopEmojiFace()
    }
  }

  /** 表情页网格的行高：三行填满底行以外的空间，每行不低于 40（`EmojiCatalogModel.cellHeightVp`）。 */
  private emojiCellHeight(): number {
    return EmojiCatalogModel.cellHeightVp(this.surfaceHeight() - EMOJI_BAR_VP - EMOJI_BAR_GAP_VP,
      EMOJI_GRID_GAP_VP);
  }

  private emojiCategoryActive(group: string): boolean {
    return this.emojiPanel === EMOJI_PANEL && !this.emojiShowingRecents && group === this.emojiGroup;
  }

  private emojiRecentsActive(): boolean {
    return this.emojiPanel === EMOJI_PANEL && this.emojiShowingRecents;
  }

  /** 最近使用标签，无论当前打开哪一页。 */
  private openEmojiRecents(): void {
    this.emojiPanel = EMOJI_PANEL;
    this.showEmojiRecents();
  }

  /** 长按最近使用标签会清空它们，取代原来的「清除最近」按钮；之后网格移到面板上次显示的分类。 */
  private clearRecentsTab(): void {
    this.clearEmojiRecents();
    if (this.emojiRecents.length === 0 && this.emojiRecentsActive()) {
      this.loadEmoji(this.emojiUnicodeGroup);
    }
  }

  /**
   * 触屏表情面板，与设计稿和 Android 的 `buildEmojiPanel` 一致：没有标题栏，八列网格，底行是 ABC、分类标签和 ⌫。标签依次是画成缩略图的「水杉娘」（读不到表情包时没有这一个）、有最近使用时的 🕘、每个 Unicode 分类一个图标，然后是「颜文字」「符号」；「颜文字」「符号」两页放的是文字，各有自己的网格，并在网格上方一行列出分组名；「水杉娘」是贴纸网格，见 `mascotStickerGrid`。
   */
  @Builder
  touchEmojiFace() {
    Column({ space: EMOJI_BAR_GAP_VP }) {
      if (this.emojiPanel === KAOMOJI_PANEL || this.emojiPanel === SYMBOLS_PANEL) {
        Scroll() {
          Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
            ForEach(this.emojiPanel === KAOMOJI_PANEL ? this.emojiKaomojiTabs : this.emojiSymbolTabs,
              (tab: EmojiPanelGroupTab) => {
              Text(tab.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(tab.key === this.emojiPanelTab
                  ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(EMOJI_GROUP_ROW_VP)
                .onClick(() => this.loadEmojiTab(this.emojiPanel, tab))
            }, (tab: EmojiPanelGroupTab) => `${this.emojiPanel}:${tab.key}:${this.emojiPanelTab}`)
          }
        }
        .scrollable(ScrollDirection.Horizontal)
        .scrollBar(BarState.Off)
        .width('100%')
        .height(EMOJI_GROUP_ROW_VP)
        .align(Alignment.Start)

        Grid(this.emojiGridScroller) {
          ForEach(this.visibleEmojiItems(), (item: string, index: number) => {
            GridItem() {
              Text(item)
                .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE + 4)
                .fontColor(this.emojiSkin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
                .onClick(() => this.selectEmoji(item))
            }
          }, (item: string, index: number) => `${index}:${item}`)
        }
        .columnsTemplate('1fr '.repeat(EMOJI_COLUMNS).trim())
        .width('100%')
        .layoutWeight(1)
      } else if (this.emojiPanel === STICKER_PANEL) {
        this.mascotStickerGrid()
      } else {
        Grid(this.emojiGridScroller) {
          ForEach(this.visibleEmojiItems(), (item: string, index: number) => {
            GridItem() {
              Text(item)
                .fontSize(EMOJI_GLYPH_FONT_SIZE)
                .fontColor(this.emojiSkin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
                .height('100%')
                .accessibilityText(`表情 ${item}`)
                .stateStyles({
                  normal: {
                    .scale({ x: 1, y: 1 })
                  },
                  pressed: {
                    .scale({ x: EMOJI_PRESSED_SCALE, y: EMOJI_PRESSED_SCALE })
                  }
                })
                .animation({ duration: EMOJI_PRESS_MS })
                .onClick(() => this.selectEmoji(item))
            }
            .height(this.emojiCellHeight())
          }, (item: string, index: number) => `${index}:${item}`)
        }
        .columnsTemplate('1fr '.repeat(EMOJI_COLUMNS).trim())
        .columnsGap(EMOJI_GRID_GAP_VP)
        .rowsGap(EMOJI_GRID_GAP_VP)
        .padding({ left: EMOJI_GRID_GAP_VP, right: EMOJI_GRID_GAP_VP })
        .width('100%')
        .layoutWeight(1)
      }

      Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
        Text('ABC')
          .layoutWeight(EMOJI_BAR_EDGE_WEIGHT)
          .height('100%')
          .textAlign(TextAlign.Center)
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontWeight(FUNCTION_LABEL_WEIGHT)
          .fontColor(this.emojiSkin.keyLabelColor(false, true))
          .backgroundColor(this.emojiSkin.keySurfaceBackground(false, true))
          .borderRadius(this.emojiSkin.keyCornerRadius())
          .accessibilityText('返回键盘')
          .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
          .animation({ duration: KEY_PRESS_MS })
          .onClick(() => this.show(SURFACE_NONE))

        Scroll() {
          Row({ space: EMOJI_GRID_GAP_VP }) {
            ForEach(this.touchPageTabs(true), (panel: EmojiPanelTab) => {
              this.touchPageTab(panel)
            }, (panel: EmojiPanelTab) => `${panel.id}:${this.emojiPanel === panel.id}`)
            if (this.emojiRecents.length > 0) {
              Text(EMOJI_RECENTS_ICON)
                .fontSize(EMOJI_TAB_FONT_SIZE)
                .fontColor(this.emojiSkin.secondary)
                .textAlign(TextAlign.Center)
                .width(EMOJI_TAB_VP)
                .height(EMOJI_TAB_VP)
                .borderRadius(EMOJI_TAB_RADIUS_VP)
                .backgroundColor(this.emojiRecentsActive() ? this.emojiSkin.keyBackground : Color.Transparent)
                .opacity(this.emojiRecentsActive() ? 1 : EMOJI_TAB_IDLE_OPACITY)
                .accessibilityText(this.emojiRecentsActive() ? '最近使用的表情，已选中，长按清除'
                  : '最近使用的表情，长按清除')
                .onClick(() => this.openEmojiRecents())
                .gesture(LongPressGesture().onAction(() => this.clearRecentsTab()))
            }
            ForEach(EmojiCatalogModel.categories(), (category: EmojiCategory) => {
              Text(category.icon)
                .fontSize(EMOJI_TAB_FONT_SIZE)
                .fontColor(this.emojiSkin.secondary)
                .textAlign(TextAlign.Center)
                .width(EMOJI_TAB_VP)
                .height(EMOJI_TAB_VP)
                .borderRadius(EMOJI_TAB_RADIUS_VP)
                .backgroundColor(this.emojiCategoryActive(category.group)
                  ? this.emojiSkin.keyBackground : Color.Transparent)
                .opacity(this.emojiCategoryActive(category.group) ? 1 : EMOJI_TAB_IDLE_OPACITY)
                .accessibilityText(this.emojiCategoryActive(category.group)
                  ? `表情分类 ${category.title}，已选中` : `表情分类 ${category.title}`)
                .onClick(() => this.loadEmoji(category.group))
            }, (category: EmojiCategory) =>
              `${category.group}:${this.emojiCategoryActive(category.group)}`)
            ForEach(this.touchPageTabs(false), (panel: EmojiPanelTab) => {
              this.touchPageTab(panel)
            }, (panel: EmojiPanelTab) => `${panel.id}:${this.emojiPanel === panel.id}`)
          }
          .constraintSize({ minWidth: '100%' })
          .height('100%')
          .justifyContent(FlexAlign.SpaceAround)
        }
        .scrollable(ScrollDirection.Horizontal)
        .scrollBar(BarState.Off)
        .layoutWeight(EMOJI_BAR_TABS_WEIGHT)
        .height('100%')

        Column() {
          KeyboardLineIcon({
            d: KeyboardIconPaths.BACKSPACE,
            iconSize: KEY_ICON_VP,
            stroke: KeyboardIconPaths.KEY_STROKE,
            color: this.emojiSkin.keyLabelColor(false, true)
          })
        }
        .layoutWeight(EMOJI_BAR_EDGE_WEIGHT)
        .height('100%')
        .justifyContent(FlexAlign.Center)
        .alignItems(HorizontalAlign.Center)
        .backgroundColor(this.emojiSkin.keySurfaceBackground(false, true))
        .borderRadius(this.emojiSkin.keyCornerRadius())
        .accessibilityText(KeyAccessibilityPolicy.delete())
        .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
        .animation({ duration: KEY_PRESS_MS })
        .onTouch((event: TouchEvent): void => {
          if (event.type === TouchType.Down) {
            this.startBackspace();
          } else if (event.type === TouchType.Up) {
            this.finishBackspace();
          } else if (event.type === TouchType.Cancel) {
            this.cancelBackspace();
          }
        })
      }
      .width('100%')
      .height(EMOJI_BAR_VP)
    }
    .width('100%')
    .height(this.surfaceHeight())
    .backgroundColor(this.emojiSkin.background)
  }

  /** 要画的页签和顺序：读得到水杉娘表情包时「水杉娘」排第一，读不到时不画它，其余照常量里的顺序。 */
  private emojiPageTabs(tabs: EmojiPanelTab[]): EmojiPanelTab[] {
    return MascotStickerPolicy.tabOrder<EmojiPanelTab>(tabs, (panel: EmojiPanelTab): string => panel.id,
      this.mascotStickers.length > 0);
  }

  /**
   * 触屏底栏上「表情」一页展开成 🕘 和各分类图标，其他页签按 `emojiPageTabs` 的顺序分在它两边：`leading` 为真时取排在「表情」之前的（读得到表情包时就是「水杉娘」），否则取之后的「颜文字」「符号」。
   */
  private touchPageTabs(leading: boolean): EmojiPanelTab[] {
    const tabs: EmojiPanelTab[] = this.emojiPageTabs(EMOJI_PANEL_TABS);
    const emoji: number = tabs.findIndex((panel: EmojiPanelTab): boolean => panel.id === EMOJI_PANEL);
    return leading ? tabs.slice(0, emoji) : tabs.slice(emoji + 1);
  }

  /** 触屏底栏上的一个页签：「水杉娘」画清单第一张的缩略图，大小、圆角和选中态底色与分类图标一样；其余画标题。 */
  @Builder
  touchPageTab(panel: EmojiPanelTab) {
    if (panel.id === STICKER_PANEL) {
      Image(KeyboardSession.shared.mascotStickers.uri(this.mascotStickers[0]))
        .width(EMOJI_TAB_VP)
        .height(EMOJI_TAB_VP)
        .padding(STICKER_TAB_INSET_VP)
        .objectFit(ImageFit.Contain)
        .sourceSize({ width: EMOJI_TAB_VP, height: EMOJI_TAB_VP })
        .draggable(false)
        .borderRadius(EMOJI_TAB_RADIUS_VP)
        .backgroundColor(this.emojiPanel === panel.id ? this.emojiSkin.keyBackground : Color.Transparent)
        .opacity(this.emojiPanel === panel.id ? 1 : EMOJI_TAB_IDLE_OPACITY)
        .accessibilityText(MascotStickerPolicy.tabAccessibilityText(this.emojiPanel === panel.id))
        .onClick(() => this.showEmojiPanel(panel.id))
    } else {
      Text(panel.title)
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.emojiPanel === panel.id ? this.emojiSkin.keyForeground : this.emojiSkin.secondary)
        .textAlign(TextAlign.Center)
        .constraintSize({ minWidth: EMOJI_TAB_VP })
        .height(EMOJI_TAB_VP)
        .padding({ left: EMOJI_PAGE_TAB_PADDING_VP, right: EMOJI_PAGE_TAB_PADDING_VP })
        .borderRadius(EMOJI_TAB_RADIUS_VP)
        .backgroundColor(this.emojiPanel === panel.id ? this.emojiSkin.keyBackground : Color.Transparent)
        .accessibilityText(this.emojiPanel === panel.id ? `${panel.title}，已选中` : panel.title)
        .onClick(() => this.showEmojiPanel(panel.id))
    }
  }

  /** 贴纸格子的边长，按量出来的网格宽度和列数算。 */
  private mascotStickerSide(): number {
    return MascotStickerPolicy.cellSideVp(this.mascotStickerGridWidth,
      MascotStickerPolicy.columns(this.mascotStickerGridWidth), EMOJI_GRID_GAP_VP);
  }

  /**
   * 水杉娘表情包的网格，触屏与 2in1 共用：竖屏手机一行五个正方形格子，更宽时按每格约 72vp 加列（`MascotStickerPolicy.columns`）。每格等比缩放居中、四周留白，按显示大小下采样解码；按下时与表情格一样放大。宽度量出来之前不放格子，免得先按 1vp 解码一遍。
   */
  @Builder
  mascotStickerGrid() {
    Grid(this.emojiGridScroller) {
      if (this.mascotStickerGridWidth > 0) {
        LazyForEach(this.mascotStickerSource, (sticker: MascotSticker, index: number) => {
          GridItem() {
            Image(KeyboardSession.shared.mascotStickers.uri(sticker))
              .width('100%')
              .height('100%')
              .padding(MASCOT_STICKER_INSET_VP)
              .objectFit(ImageFit.Contain)
              .sourceSize({
                width: MascotStickerPolicy.decodeSideVp(this.mascotStickerSide()),
                height: MascotStickerPolicy.decodeSideVp(this.mascotStickerSide())
              })
              .draggable(false)
              .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
              .backgroundColor(this.panelKeySelected(index) ? this.emojiSkin.keyBackground : Color.Transparent)
              .accessibilityText(MascotStickerPolicy.accessibilityText(sticker))
              .stateStyles({
                normal: {
                  .scale({ x: 1, y: 1 })
                },
                pressed: {
                  .scale({ x: EMOJI_PRESSED_SCALE, y: EMOJI_PRESSED_SCALE })
                }
              })
              .animation({ duration: EMOJI_PRESS_MS })
              .onClick(() => this.sendMascotSticker(sticker))
          }
          .height(this.mascotStickerSide())
        }, (sticker: MascotSticker) => sticker.file)
      }
    }
    .columnsTemplate('1fr '.repeat(MascotStickerPolicy.columns(this.mascotStickerGridWidth)).trim())
    .columnsGap(EMOJI_GRID_GAP_VP)
    .rowsGap(EMOJI_GRID_GAP_VP)
    .padding({ left: EMOJI_GRID_GAP_VP, right: EMOJI_GRID_GAP_VP })
    .width('100%')
    .layoutWeight(1)
    .onAreaChange((_old: Area, now: Area): void => {
      this.mascotStickerGridWidth = Math.max(0, Number(now.width) - EMOJI_GRID_GAP_VP * 2);
    })
  }

  /** 2in1 表情面板，参照 Windows 版：「水杉娘」「表情」「颜文字」「符号」「剪贴板」标签（读不到表情包时没有「水杉娘」）、分组行、搜索行和网格。 */
  @Builder
  desktopEmojiFace() {
    Column() {
      // Back, title, delete: the panel covers the keys, so the delete key has to come with it or
      // there is no way to take back a tap without leaving.
      Row() {
        Text('\u2039')
          .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
          .fontColor(this.emojiSkin.accent)
          .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP, right: 10 })
          .onClick(() => this.leaveSurface(DesktopSurface.EMOJI))
        Blank()
        Text('表情')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.emojiSkin.keyForeground)
        if (this.emojiShowingRecents && this.emojiRecents.length > 0) {
          Text('清除最近')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.emojiSkin.accent)
            .padding({ left: 8, right: 8 })
            .onClick(() => this.clearEmojiRecents())
        }
        Blank()
        Column() {
          this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, false, 0)
        }
        .width(KeyboardMetrics.MODIFIER_WIDTH_VP)
        .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
        .padding({ right: KeyboardMetrics.CANDIDATE_PADDING_VP })
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)

      Scroll() {
        Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
          ForEach(this.emojiPageTabs(KeyboardFormFactor.isDesktop() ? DESKTOP_EMOJI_PANEL_TABS : EMOJI_PANEL_TABS),
            (panel: EmojiPanelTab) => {
            Text(panel.title)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.emojiPanel === panel.id
                ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                right: KeyboardMetrics.CANDIDATE_PADDING_VP })
              .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
              .onClick(() => this.showEmojiPanel(panel.id))
          }, (panel: EmojiPanelTab) => panel.id)
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
      .align(Alignment.Start)

      Scroll() {
        Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
          if (this.emojiPanel === EMOJI_PANEL && this.emojiRecents.length > 0) {
            Text('最近')
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(this.emojiShowingRecents ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                right: KeyboardMetrics.CANDIDATE_PADDING_VP })
              .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
              .onClick(() => this.showEmojiRecents())
          }
          if (this.emojiPanel === EMOJI_PANEL) {
            ForEach(EmojiCatalogModel.categories(), (category: EmojiCategory) => {
              Text(category.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(!this.emojiShowingRecents && category.group === this.emojiGroup
                  ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
                .onClick(() => this.loadEmoji(category.group))
            }, (category: EmojiCategory) => `${category.group}:${this.emojiGroup}:${this.emojiShowingRecents}`)
          } else if (this.emojiPanel === KAOMOJI_PANEL) {
            ForEach(this.emojiKaomojiTabs, (tab: EmojiPanelGroupTab) => {
              Text(tab.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(tab.key === this.emojiPanelTab
                  ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
                .onClick(() => this.loadEmojiTab(KAOMOJI_PANEL, tab))
            }, (tab: EmojiPanelGroupTab) => `${tab.key}:${this.emojiPanelTab}`)
          } else if (this.emojiPanel === CLIPBOARD_PANEL) {
            // Saving and clearing act on this device's history, so the cloud section does not offer them.
            if (!this.cloudTabActive()) {
              // Nothing is captured in the background (see captureClipboard), so saving is a button here too.
              Text('保存当前')
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.emojiSkin.accent)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
                .onClick(() => this.captureClipboard())
              Text(this.confirmingClear ? '确认清空' : '清空')
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(this.emojiSkin.accent)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
                .onClick(() => this.clearClipboard())
            }
          } else if (this.emojiPanel === SYMBOLS_PANEL) {
            // 符号页列出分组；「水杉娘」只有一组，不进任何分支，这一行留空。
            ForEach(this.emojiSymbolTabs, (tab: EmojiPanelGroupTab) => {
              Text(tab.title)
                .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
                .fontColor(tab.key === this.emojiPanelTab
                  ? this.emojiSkin.accent : this.emojiSkin.keyForeground)
                .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                  right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
                .onClick(() => this.loadEmojiTab(SYMBOLS_PANEL, tab))
            }, (tab: EmojiPanelGroupTab) => `${tab.key}:${this.emojiPanelTab}`)
          }
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
      .align(Alignment.Start)

      // The Windows panel's search box. Not a text field: that would ask the system for an input method inside the input method. The physical keys the panel receives spell it instead (EmojiPanelKeyPolicy), so only a 2in1, which routes them here, draws it.
      // 表情包不做搜索，没有搜索行。
      if (KeyboardFormFactor.isDesktop() && this.desktop === DesktopSurface.EMOJI && this.emojiPanel !== STICKER_PANEL) {
        Row() {
          Text(this.emojiQuery.length > 0 ? this.emojiQuery : this.emojiSearchPlaceholder())
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.emojiSkin.keyForeground)
            .opacity(this.emojiQuery.length > 0 ? 1 : 0.55)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .layoutWeight(1)
          if (this.emojiQuery.length > 0) {
            Text('\u00d7')
              .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
              .fontColor(this.emojiSkin.accent)
              .padding({ left: 8, right: 8 })
              .onClick(() => this.searchEmoji(''))
          }
        }
        .width('100%')
        .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
        .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
          right: KeyboardMetrics.CANDIDATE_PADDING_VP })
      }

      if (this.emojiPanel === CLIPBOARD_PANEL) {
        this.clipboardEntries()
      } else if (this.emojiPanel === STICKER_PANEL) {
        this.mascotStickerGrid()
      } else if (this.emojiQuery.length > 0 && this.emojiResults.length === 0) {
        Text('没有匹配的结果')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.emojiSkin.secondary)
          .width('100%')
          .layoutWeight(1)
          .textAlign(TextAlign.Center)
      } else {
        Grid(this.emojiGridScroller) {
          ForEach(this.visibleEmojiItems(), (item: string, index: number) => {
            GridItem() {
              Text(item)
                .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE + 4)
                .fontColor(this.emojiSkin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
                .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
                .backgroundColor(this.panelKeySelected(index) ? this.emojiSkin.keyBackground : Color.Transparent)
                .onClick(() => this.selectEmoji(item))
                .onHover((isHover: boolean): void => this.hoverItem(`emoji:${index}:${item}`,
                  this.emojiHoverName(item), isHover))
                .bindPopup(this.hoverTipKey === `emoji:${index}:${item}`, this.hoverTipOptions(`emoji:${index}:${item}`))
            }
          }, (item: string, index: number) => `${index}:${item}`)
        }
        .columnsTemplate('1fr '.repeat(EMOJI_COLUMNS).trim())
        .width('100%')
        .layoutWeight(1)
      }
    }
    .width('100%')
    .height(this.surfaceHeight())
    .backgroundColor(this.emojiSkin.background)
  }

  /**
   * 触屏符号面板，与 Android 的 `SymbolPanelView` 一致：左边竖排分类，右边五列可滚动的符号键，底行是「返回」「锁定」和 ⌫。点一个符号默认上屏后回到键盘；「锁定」打开时留在面板里连续输入，每次打开面板都重新关上。没有 Android 的标题行：这里面板上方一直有工具栏，标题只会再挤掉一行符号。
   */
  @Builder
  symbolPanelFace() {
    Column({ space: EMOJI_BAR_GAP_VP }) {
      Row({ space: SYMBOL_GAP_VP }) {
        Column({ space: EMOJI_GRID_GAP_VP }) {
          ForEach(SymbolPanelPolicy.categories(), (category: SymbolPanelCategory, index: number) => {
            Text(category.title)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontWeight(index === this.symbolCategory ? FUNCTION_LABEL_WEIGHT : 400)
              .fontColor(index === this.symbolCategory ? this.skin.keyForeground : this.skin.secondary)
              .textAlign(TextAlign.Center)
              .width('100%')
              .layoutWeight(1)
              .constraintSize({ maxHeight: SYMBOL_TAB_MAX_VP })
              .borderRadius(EMOJI_TAB_RADIUS_VP)
              .backgroundColor(index === this.symbolCategory ? this.skin.keyBackground : Color.Transparent)
              .opacity(index === this.symbolCategory ? 1 : EMOJI_TAB_IDLE_OPACITY)
              .accessibilityText(index === this.symbolCategory
                ? `符号分类 ${category.title}，已选中` : `符号分类 ${category.title}`)
              .onClick(() => this.selectSymbolCategory(index))
          }, (category: SymbolPanelCategory, index: number) =>
            `${category.id}:${index === this.symbolCategory}`)
        }
        .width(SYMBOL_RAIL_VP)
        .height('100%')

        Grid(this.symbolGridScroller) {
          // 成对符号的前半个落在行尾时留一格空，两半排在同一行（`SymbolPanelPolicy.cells`）；空格子只占位，不画键。
          ForEach(SymbolPanelPolicy.cells(SymbolPanelPolicy.category(this.symbolCategory).symbols
            .slice(0, SymbolPanelPolicy.maxSymbolsPerCategory()), SYMBOL_COLUMNS), (symbol: string) => {
            GridItem() {
              if (symbol.length > 0) {
                Text(symbol)
                  .fontSize(SYMBOL_FONT_SIZE)
                  .maxFontSize(SYMBOL_FONT_SIZE)
                  .minFontSize(SYMBOL_MIN_FONT_SIZE)
                  .maxLines(1)
                  .fontColor(this.skin.keyLabelColor(false))
                  .fontFamily(this.skin.monospaced ? 'monospace' : undefined)
                  .textAlign(TextAlign.Center)
                  .width('100%')
                  .height('100%')
                  .backgroundColor(this.skin.keySurfaceBackground(false))
                  .linearGradient({ angle: 180,
                    colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
                  .borderRadius(this.skin.keyCornerRadius())
                  .borderWidth(this.skin.borderWidth)
                  .borderColor(this.skin.borderColor)
                  .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
                    offsetX: 0, offsetY: this.skin.shadowOffset })
                  .accessibilityText(KeyAccessibilityPolicy.symbol(symbol))
                  .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
                  .animation({ duration: KEY_PRESS_MS })
                  .onClick(() => this.selectSymbol(symbol))
              }
            }
            .height(SYMBOL_CELL_VP)
          }, (symbol: string, index: number) => `${this.symbolCategory}:${index}:${symbol}:${this.faceKey()}`)
        }
        .columnsTemplate('1fr '.repeat(SYMBOL_COLUMNS).trim())
        .columnsGap(SYMBOL_GAP_VP)
        .rowsGap(SYMBOL_GAP_VP)
        .scrollBar(BarState.Auto)
        .edgeEffect(EdgeEffect.Spring)
        .padding({ right: SYMBOL_SCROLLBAR_GUTTER_VP })
        .layoutWeight(1)
        .height('100%')
      }
      .width('100%')
      .layoutWeight(1)

      Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
        Text('返回')
          .layoutWeight(1)
          .height('100%')
          .textAlign(TextAlign.Center)
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontWeight(FUNCTION_LABEL_WEIGHT)
          .fontColor(this.skin.keyLabelColor(false, true))
          .backgroundColor(this.skin.keySurfaceBackground(false, true))
          .borderRadius(this.skin.keyCornerRadius())
          .accessibilityText('返回键盘')
          .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
          .animation({ duration: KEY_PRESS_MS })
          .onClick(() => this.show(SURFACE_NONE))

        // 打开时用回车键的强调色，和「已锁定」的文字一起说明点符号不会回到键盘。
        Text(this.symbolLocked ? '已锁定' : '锁定')
          .layoutWeight(1)
          .height('100%')
          .textAlign(TextAlign.Center)
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontWeight(FUNCTION_LABEL_WEIGHT)
          .fontColor(this.skin.keyLabelColor(this.symbolLocked, true))
          .backgroundColor(this.skin.keySurfaceBackground(this.symbolLocked, true))
          .borderRadius(this.skin.keyCornerRadius())
          .accessibilityText(this.symbolLocked ? '已锁定，连续输入符号；点按解除' : '锁定，连续输入符号')
          .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
          .animation({ duration: KEY_PRESS_MS })
          .onClick(() => this.symbolLocked = !this.symbolLocked)

        Column() {
          KeyboardLineIcon({
            d: KeyboardIconPaths.BACKSPACE,
            iconSize: KEY_ICON_VP,
            stroke: KeyboardIconPaths.KEY_STROKE,
            color: this.skin.keyLabelColor(false, true)
          })
        }
        .layoutWeight(1)
        .height('100%')
        .justifyContent(FlexAlign.Center)
        .alignItems(HorizontalAlign.Center)
        .backgroundColor(this.skin.keySurfaceBackground(false, true))
        .borderRadius(this.skin.keyCornerRadius())
        .accessibilityText(KeyAccessibilityPolicy.delete())
        .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
        .animation({ duration: KEY_PRESS_MS })
        .onTouch((event: TouchEvent): void => {
          if (event.type === TouchType.Down) {
            this.startBackspace();
          } else if (event.type === TouchType.Up) {
            this.finishBackspace();
          } else if (event.type === TouchType.Cancel) {
            this.cancelBackspace();
          }
        })
      }
      .width('100%')
      .height(EMOJI_BAR_VP)
    }
    .width('100%')
    .height(this.surfaceHeight())
    .backgroundColor(this.skin.background)
  }

  /** Literal digit and letters offered by a long-pressed pinyin nine-key cell. */
  @Builder
  nineKeyHoldFace() {
    Column() {
      Text('长按输入')
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.skin.keyForeground)
        .opacity(0.65)
        .height(KeyboardMetrics.ROW_HEIGHT_VP)
      Row({ space: this.keySpacing() }) {
        ForEach(this.nineKeyHoldOptions, (option: string) => {
          Text(option)
            .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
            .fontColor(this.skin.keyForeground)
            .textAlign(TextAlign.Center)
            .layoutWeight(1)
            .height(KeyboardMetrics.ROW_HEIGHT_VP)
            .backgroundColor(this.skin.keyBackground)
            .borderRadius(this.skin.cornerRadius)
            .accessibilityText(`输入 ${option}`)
            .onClick(() => this.selectNineKeyHoldOption(option))
        }, (option: string) => option)
      }
      .width('100%')
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })
      Blank()
      Text('取消')
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.skin.accent)
        .textAlign(TextAlign.Center)
        .width('100%')
        .height(this.rowHeight(3))
        .accessibilityText('取消长按输入')
        .onClick(() => this.show(SURFACE_NONE))
    }
    .width('100%')
    .height(this.surfaceHeight())
    .backgroundColor(this.skin.background)
  }

  @Builder
  nineKeyControl(face: string, label: string, action: () => void) {
    Column() {
      Text(face)
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.skin.keyForeground)
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .layoutWeight(1)
    .width('100%')
    .justifyContent(FlexAlign.Center)
    .backgroundColor(this.skin.keyBackground)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .accessibilityText(label)
    .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
    .animation({ duration: KEY_PRESS_MS })
    .responseRegion(this.keyHitRegion())
    .onClick(action)
  }

  private drawSkinPattern(): void {
    const context: CanvasRenderingContext2D = this.skinPatternContext;
    const width: number = 1200;
    const height: number = 600;
    context.clearRect(0, 0, width, height);
    context.fillStyle = this.skin.patternColor();
    context.strokeStyle = this.skin.patternColor();
    if (this.skin.pattern === 1) {
      for (let y: number = 8; y < height; y += 16) {
        for (let x: number = 8; x < width; x += 16) context.fillRect(x, y, 2, 2);
      }
    } else if (this.skin.pattern === 2) {
      context.lineWidth = 0.5;
      context.beginPath();
      for (let x: number = 0; x < width; x += 20) {
        context.moveTo(x, 0); context.lineTo(x, height);
      }
      for (let y: number = 0; y < height; y += 20) {
        context.moveTo(0, y); context.lineTo(width, y);
      }
      context.stroke();
    } else if (this.skin.pattern === 3) {
      context.lineWidth = 2;
      context.beginPath();
      for (let offset: number = -100; offset < height + width; offset += 24) {
        context.moveTo(0, offset);
        context.bezierCurveTo(width * 0.35, offset - 90, width * 0.65, offset + 20,
          width, offset - 70);
      }
      context.stroke();
    }
  }

  @Builder
  skinBackdrop() {
    Stack() {
      if (this.skin.gradientEnd === null) {
        Column().width('100%').height('100%').backgroundColor(this.skin.background)
      } else {
        Column().width('100%').height('100%').linearGradient({
          angle: this.skin.gradientHorizontal ? 90 : 180,
          colors: [[this.skin.background, 0], [this.skin.gradientEnd, 1]]
        })
      }
      if (this.skin.photoSource !== null) {
        Image(this.skin.photoSource)
          .width('100%').height('100%').objectFit(ImageFit.Cover)
          .scale({ x: 1.15, y: 1.15 })
          .translate({ x: (this.skin.photoPosition - 0.5) * 40,
            y: (this.skin.photoPosition - 0.5) * 24 })
        Column().width('100%').height('100%').backgroundColor(this.skin.photoShadeColor())
      }
      if (this.skin.pattern !== 0) {
        Canvas(this.skinPatternContext)
          .id(`skin-pattern:${this.skin.key()}`)
          .width('100%').height('100%')
          .onReady(() => this.drawSkinPattern())
      }
    }
    .width('100%')
    .height('100%')
  }

  build() {
    Column() {
      if (this.floatingDrawn()) {
        this.floatingBar()
      }
      // 浮动时拖动条占去顶部 22vp，键盘这一层分到窗口剩下的高度；停靠时它就是整个窗口，与加拖动条之前相同。
      Column() {
        this.keyboardStack()
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(this.floatingDrawn() ? this.skin.background : Color.Transparent)
    .borderRadius(this.floatingDrawn() ? FloatingKeyboardPolicy.CORNER_RADIUS_VP : 0)
    .clip(this.floatingDrawn())
  }

  /**
   * 浮动键盘顶部的拖动条（#10225），移植自 Android 的 `FloatingKeyboardBar`：中间是 36×4 的横条，按住整条拖动移动键盘；右端是「停靠」键（maximize-2 图标，与单手侧栏的退出键同一个），点它回到停靠在底部的完整键盘。
   *
   * 拖动报告的是手指在屏幕上的位移（`displayX` / `displayY`），不是条内坐标：面板跟着手指移动，条内坐标会跟着变。面板归扩展能力，位置由它换算、钳制并移动。
   */
  @Builder
  floatingBar() {
    Row() {
      Stack() {
        Column()
          .width(FloatingKeyboardPolicy.GRIP_WIDTH_VP)
          .height(FloatingKeyboardPolicy.GRIP_HEIGHT_VP)
          .borderRadius(FloatingKeyboardPolicy.GRIP_HEIGHT_VP / 2)
          .backgroundColor(this.skin.keyForeground)
          .opacity(0.35)
      }
      .layoutWeight(1)
      .height('100%')
      .accessibilityText(FloatingKeyboardPolicy.DESCRIPTION)
      .onTouch((event: TouchEvent): void => this.floatingBarTouch(event))

      Stack() {
        KeyboardLineIcon({
          d: KeyboardIconPaths.EXIT_ONE_HAND,
          iconSize: FloatingKeyboardPolicy.DOCK_ICON_VP,
          stroke: OneHandedPolicy.ICON_STROKE,
          color: this.skin.keyForeground
        })
      }
      .width(FloatingKeyboardPolicy.DOCK_WIDTH_VP)
      .height('100%')
      .accessibilityText(FloatingKeyboardPolicy.DOCK_DESCRIPTION)
      .onClick((): void => this.setFloating(false))
    }
    .width('100%')
    .height(FloatingKeyboardPolicy.BAR_HEIGHT_VP)
  }

  /** 拖动条上的手指：按下记起点，移动时把相对起点的屏幕位移交给会话，抬起或取消时结束拖动。 */
  private floatingBarTouch(event: TouchEvent): void {
    const touch: TouchObject | undefined = event.touches.length > 0 ? event.touches[0] : undefined;
    if (event.type === TouchType.Down && touch !== undefined) {
      this.floatingTouchOrigin = [touch.displayX, touch.displayY];
      KeyboardSession.shared.beginFloatingDrag();
    } else if (event.type === TouchType.Move && touch !== undefined) {
      KeyboardSession.shared.dragFloating(touch.displayX - this.floatingTouchOrigin[0],
        touch.displayY - this.floatingTouchOrigin[1]);
    } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
      KeyboardSession.shared.endFloatingDrag();
    }
    event.stopPropagation();
  }

  /**
   * 键盘调整的遮罩：盖在键盘外框（`adjustFrame`，两侧和抬起的地方不盖）上的半透明黑色，四角是皮肤强调色的直角括号。上沿正中、下沿正中各一个横向把手，左右两沿垂直居中各一个竖向把手，中间一排「重置」「移动」「确认」。当前不生效的项不画对应的柄（`adjustHandles`）。整层挡住触摸，下面的键盘只是预览。
   */
  @Builder
  adjustOverlay() {
    Column() {
      Stack() {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(KeyboardAdjustPolicy.MASK_COLOR)
          .accessibilityLevel('no')
        this.adjustCorners()
        Column() {
          this.adjustEdge(ADJUST_TOP)
          Row() {
            // 左右柄一起画或一起不画；不画时按钮那一排用满外框宽（`adjustButtonRow` 也按此换算），不留占位。
            if (this.adjustHandles().left) {
              this.adjustEdge(ADJUST_LEFT)
            }
            Row({ space: this.adjustButtonRow().gap }) {
              this.adjustButton(ADJUST_RESET)
              if (this.adjustHandles().move) {
                this.adjustButton(ADJUST_MOVE)
              }
              this.adjustButton(ADJUST_CONFIRM)
            }
            .layoutWeight(1)
            .justifyContent(FlexAlign.Center)
            if (this.adjustHandles().right) {
              this.adjustEdge(ADJUST_RIGHT)
            }
          }
          .width('100%')
          .layoutWeight(1)
          .alignItems(VerticalAlign.Center)
          if (this.adjustHandles().bottom) {
            this.adjustEdge(ADJUST_BOTTOM)
          } else {
            Column().width(ADJUST_HANDLE_LONG_VP).height(KeyboardAdjustPolicy.HANDLE_SHORT_VP)
          }
        }
        .width('100%')
        .height('100%')
        .alignItems(HorizontalAlign.Center)
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .padding({ left: this.adjustFrame().left, right: this.adjustFrame().right, bottom: this.adjustFrame().lift })
    .hitTestBehavior(HitTestMode.Block)
  }

  /** 四角的直角括号，标出键盘外框；不接触摸，读屏跳过。 */
  @Builder
  adjustCorners() {
    Column() {
      Row() {
        this.adjustCorner(true, true)
        Blank()
        this.adjustCorner(true, false)
      }
      .width('100%')
      Blank()
      Row() {
        this.adjustCorner(false, true)
        Blank()
        this.adjustCorner(false, false)
      }
      .width('100%')
    }
    .width('100%')
    .height('100%')
    .hitTestBehavior(HitTestMode.None)
    .accessibilityLevel('no-hide-descendants')
  }

  /** 一角的括号。浮动时外框的下面两角就是卡片的圆角（`build` 按 `FloatingKeyboardPolicy.CORNER_RADIUS_VP` 裁剪，浮动时不抬高、不收窄），直角括号的拐角会被裁掉，只剩两截短线；这两角改画同样半径的圆角括号，贴着卡片边走。上面两角在拖动条下面，不是卡片的角，仍是直角。 */
  @Builder
  adjustCorner(top: boolean, left: boolean) {
    Column()
      .width(ADJUST_CORNER_VP)
      .height(ADJUST_CORNER_VP)
      .borderWidth({
        top: top ? ADJUST_CORNER_STROKE_VP : 0,
        bottom: top ? 0 : ADJUST_CORNER_STROKE_VP,
        left: left ? ADJUST_CORNER_STROKE_VP : 0,
        right: left ? 0 : ADJUST_CORNER_STROKE_VP
      })
      .borderRadius({
        bottomLeft: !top && left && this.floatingDrawn() ? FloatingKeyboardPolicy.CORNER_RADIUS_VP : 0,
        bottomRight: !top && !left && this.floatingDrawn() ? FloatingKeyboardPolicy.CORNER_RADIUS_VP : 0
      })
      .borderColor(this.skin.accent)
  }

  /**
   * 一条边上的把手：上下沿是三道横线，左右沿是两道竖线，触摸区比线大得多。按住拖动只改这条边（`dragAdjust`）。
   *
   * 上沿柄的线贴着外框上沿画（`KeyboardAdjustPolicy.TOP_GRIP_INSET_VP`），落在工具栏图标上方那一条空白里，与 Android 相同；居中的话线会画进工具栏那一行、压在图标上，收窄后图标挤得更密时尤其明显（iOS 验收时就是这样）。触摸区照旧 32vp 高，遮罩整层挡住触摸，盖到工具栏上不碍事。
   */
  @Builder
  adjustEdge(handle: string) {
    Stack({ alignContent: handle === ADJUST_TOP ? Alignment.Top : Alignment.Center }) {
      if (handle === ADJUST_TOP || handle === ADJUST_BOTTOM) {
        // 三道线连同间距的厚度取 gripSpanVp(3)：测试按它断言横线底边在工具栏图标之上，画出来的就是这个厚度。
        Column({ space: KeyboardAdjustPolicy.GRIP_GAP_VP }) {
          this.adjustGripLine(true)
          this.adjustGripLine(true)
          this.adjustGripLine(true)
        }
        .height(KeyboardAdjustPolicy.gripSpanVp(3))
      } else {
        Row({ space: KeyboardAdjustPolicy.GRIP_GAP_VP }) {
          this.adjustGripLine(false)
          this.adjustGripLine(false)
        }
        .width(KeyboardAdjustPolicy.gripSpanVp(2))
      }
    }
    .width(handle === ADJUST_TOP || handle === ADJUST_BOTTOM ? ADJUST_HANDLE_LONG_VP : KeyboardAdjustPolicy.HANDLE_SHORT_VP)
    .height(handle === ADJUST_TOP || handle === ADJUST_BOTTOM ? KeyboardAdjustPolicy.HANDLE_SHORT_VP : ADJUST_HANDLE_LONG_VP)
    .padding({ top: handle === ADJUST_TOP ? KeyboardAdjustPolicy.TOP_GRIP_INSET_VP : 0 })
    .accessibilityGroup(true)
    .accessibilityText(this.adjustHandleLabel(handle))
    .accessibilityDescription(this.adjustHandleDescription(handle))
    .onTouch((event: TouchEvent): void => this.adjustTouch(handle, event))
  }

  @Builder
  adjustGripLine(horizontal: boolean) {
    Column()
      .width(horizontal ? ADJUST_GRIP_LENGTH_VP : KeyboardAdjustPolicy.GRIP_THICKNESS_VP)
      .height(horizontal ? KeyboardAdjustPolicy.GRIP_THICKNESS_VP : ADJUST_GRIP_LENGTH_VP)
      .borderRadius(KeyboardAdjustPolicy.GRIP_THICKNESS_VP / 2)
      .backgroundColor(KeyboardAdjustPolicy.INK_COLOR)
  }

  /** 中间一排的按钮：白色线条图标加文字。「移动」按住拖动整块键盘，其余两个点按。 */
  @Builder
  adjustButton(kind: string) {
    Column({ space: ADJUST_BUTTON_GAP_VP }) {
      KeyboardLineIcon({
        d: this.adjustButtonIcon(kind),
        iconSize: ADJUST_BUTTON_ICON_VP,
        stroke: KeyboardIconPaths.ADJUST_STROKE,
        color: KeyboardAdjustPolicy.INK_COLOR
      })
      Text(this.adjustButtonLabel(kind))
        .fontSize(ADJUST_BUTTON_FONT_SIZE)
        .fontColor(KeyboardAdjustPolicy.INK_COLOR)
        .maxLines(1)
    }
    .width(this.adjustButtonRow().width)
    .padding({ top: ADJUST_BUTTON_GAP_VP, bottom: ADJUST_BUTTON_GAP_VP })
    .accessibilityGroup(true)
    .accessibilityText(this.adjustButtonLabel(kind))
    .accessibilityDescription(this.adjustButtonDescription(kind))
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(0.6)
      }
    })
    .onClick((): void => this.adjustButtonClick(kind))
    .onTouch((event: TouchEvent): void => {
      if (kind === ADJUST_MOVE) {
        this.adjustTouch(kind, event);
      }
    })
  }

  @Builder
  keyboardStack() {
    Stack({ alignContent: Alignment.Top }) {
      // On a desktop candidate window with a mascot the backdrop starts at the card's top edge: the band above it is transparent, so the mascot stands on the card rather than on a slab of keyboard colour.
      Column() {
        this.skinBackdrop()
      }
      .width('100%')
      .height('100%')
      .opacity(KeyboardFormFactor.isDesktop() && this.desktop === DesktopSurface.NONE
        && this.settings.candidateCanvas !== null ? 0 : 1)
      .padding({ top: CandidateDecorationLayout.backdropInsetVp(KeyboardFormFactor.isDesktop(),
        this.settings.candidateDecorationImage !== null, this.settings.candidateDecorationTopVp,
        KeyboardMetrics.ROOT_VERTICAL_PADDING_VP) })
      this.keyboard()
      if (this.glideActive) {
        // The glide trail, over the keys and blind to touches: the finger drawing it is already bound to the row it went down in.
        Canvas(this.glideContext)
          .width('100%')
          .height('100%')
          .hitTestBehavior(HitTestMode.None)
          .onReady(() => this.drawGlideTrail())
      }
      if (this.surface === SURFACE_GEOMETRY && this.touchDesign()) {
        this.adjustOverlay()
      }
      if (this.punctuation || this.brackets || this.managing >= 0
        || this.expandedGloss >= 0) {
        Column() {
          this.overlayStrip(this.punctuation ? '常用标点'
            : this.brackets ? '括弧'
              : this.expandedGloss >= 0 ? '释义' : '候选词管理')
        }
        .width('100%')
        .padding({
          left: this.overlayHorizontalInset() + this.adjustFrame().left,
          right: this.overlayHorizontalInset() + this.adjustFrame().right,
          top: this.overlayTopInset()
        })
      }
      if (this.keyPreview !== null) {
        // 按键预览气泡，在所有内容之上只绘制一次，且不接收触摸。第一行键的气泡会向上伸进工具条，两者在同一个窗口里。
        Text(this.keyPreview.label)
          .fontSize(KeyPreviewLayout.TEXT_FP)
          .fontColor(this.skin.keyForeground)
          .fontFamily(this.skin.monospaced ? 'monospace' : undefined)
          .textAlign(TextAlign.Center)
          .maxLines(1)
          .width(this.keyPreview.width)
          .height(KeyPreviewLayout.HEIGHT_VP)
          .backgroundColor(this.skin.keyBackground)
          .borderRadius({
            topLeft: KeyPreviewLayout.TOP_RADIUS_VP,
            topRight: KeyPreviewLayout.TOP_RADIUS_VP,
            bottomLeft: KeyPreviewLayout.BOTTOM_RADIUS_VP,
            bottomRight: KeyPreviewLayout.BOTTOM_RADIUS_VP
          })
          .border({ width: KeyPreviewLayout.RING_WIDTH_VP, color: KeyPreviewLayout.RING_COLOR })
          .shadow({ radius: KeyPreviewLayout.SHADOW_RADIUS_VP, color: KeyPreviewLayout.SHADOW_COLOR,
            offsetX: 0, offsetY: KeyPreviewLayout.SHADOW_OFFSET_VP })
          .position({ x: this.keyPreview.left, y: this.keyPreview.top })
          .hitTestBehavior(HitTestMode.None)
          .accessibilityLevel('no')
      }
    }
    .width('100%')
    .onAreaChange((_old: Area, now: Area): void => {
      this.glideOriginX = Number(now.globalPosition.x ?? 0);
      this.glideOriginY = Number(now.globalPosition.y ?? 0);
      this.rootWidth = Number(now.width);
    })
    .onTouch((event: TouchEvent): void => this.pruneKeyPreview(event))
  }

  /** 长按条所在位置：在第一行按键上方，工具条及其间隙下方；工具条不画时紧贴顶部内边距。 */
  private overlayTopInset(): number {
    if (this.touchDesign()) {
      return KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP
        + (this.stripShown() ? this.stripHeight() + KeyboardMetrics.STRIP_GAP_VP : 0);
    }
    return KeyboardMetrics.ROOT_VERTICAL_PADDING_VP
      + KeyboardMetrics.compositionRowHeightVp(this.settings.candidatePreeditFontSize)
      + this.candidateRowHeight() + KeyboardMetrics.CANDIDATE_CARD_PADDING_VP * 2
      + KeyboardMetrics.ROW_SPACING_VP;
  }

  private overlayHorizontalInset(): number {
    return this.touchDesign() ? KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP
      : KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP;
  }

  @Builder
  keyboard() {
    if (this.touchDesign()) {
      // 设计稿的框架：50vp 工具条上方 8vp，下方固定 10vp，按键行之间是用户设定的行距，底部 6vp（加上键盘调整的底部抬高），两侧 4vp。`KeyboardMetrics.totalHeightVp` 正是按此确定面板大小。工具栏隐藏或工具全关、空闲时不画工具条和它下面的间距（`stripShown`），面板同样按此变矮。
      Column() {
        if (this.stripShown()) {
          this.touchStrip()
        }
        Column({ space: this.rowSpacing() }) {
          this.keyFaces()
        }
        .width('100%')
        .margin({ top: this.stripShown() ? KeyboardMetrics.STRIP_GAP_VP : 0 })
      }
      .width('100%')
      // 键盘调整时遮罩下的工具条和按键只是预览：读屏整块跳过，免得双击落到遮罩下的「表情」或按键上，读屏只走遮罩上的柄和按钮（`adjustOverlay`），与 Android 的 IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS、iOS 的 accessibilityViewIsModal 相同。
      .accessibilityLevel(this.surface === SURFACE_GEOMETRY ? 'no-hide-descendants' : 'auto')
      // 键盘调整收窄或挪开时两侧多让出 `adjustFrame` 的左右，底部抬高时下面多让出抬高的那一段：让出的地方只有键盘底色，点了不落到按键上。
      .padding({
        left: KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP + this.adjustFrame().left,
        right: KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP + this.adjustFrame().right,
        top: KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP,
        bottom: KeyboardMetrics.TOUCH_ROOT_BOTTOM_PADDING_VP + this.adjustFrame().lift
      })
    } else {
      Column({ space: this.rowSpacing() }) {
        this.candidateStrip()

        // 机器自带按键时，工具条就是整个面板，除非工具栏请求打开某个在手机上要从键面进入的界面。不请自来地在实体键盘下再画一套按键，只会占用书写内容的屏幕空间；而在被请求时画出来，正是这个按钮的意义所在。
        if (this.desktop === DesktopSurface.EMOJI) {
          this.emojiFace()
        } else if (this.desktop === DesktopSurface.SCREEN_KEYBOARD) {
          this.keyFaces()
        } else if (this.desktop === DesktopSurface.VOICE) {
          this.voiceFace()
        } else if (this.desktop === DesktopSurface.HANDWRITING) {
          // 不带操作行的手写板：实体键盘已经有空格、回车和语言切换。
          this.handwritingPanel()
        }
      }
      .width('100%')
      .padding({
        left: KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP,
        right: KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP,
        top: KeyboardMetrics.ROOT_VERTICAL_PADDING_VP,
        bottom: KeyboardMetrics.ROOT_VERTICAL_PADDING_VP
      })
    }
  }

  /**
   * Everything below the strip: whichever surface is open, or the keys themselves.
   *
   * Separate from keyboard() because it is drawn only where the machine has no keys of its own.
   */
  @Builder
  keyFaces() {
      if (this.surface === SURFACE_GEOMETRY) {
        // 键盘调整时按键作实时预览；盖在整块键盘上的遮罩（`adjustOverlay`）接收所有触摸，所以调整时不会输入任何内容。
        Column({ space: this.rowSpacing() }) {
          this.handedKeys()
        }
        .width('100%')
        .height(this.surfaceHeight())
      } else if (this.isTools() && this.editing.length === 0 && this.candidates.length === 0) {
        // 硬件键盘组合输入时，候选占据工具条，按键回来；组合结束后面板恢复，因为界面保持原样。
        this.toolsFace()
      } else if (this.surface === SURFACE_EXPANDED) {
        this.expandedFace()
      } else if (this.surface === SURFACE_CLIPBOARD) {
        this.clipboardFace()
      } else if (this.surface === SURFACE_PHRASES) {
        this.phraseFace()
      } else if (this.surface === SURFACE_REPLY) {
        this.replyFace()
      } else if (this.surface === SURFACE_POLISH) {
        this.polishFace()
      } else if (this.surface === SURFACE_VOICE) {
        this.voiceFace()
      } else if (this.isPicker()) {
        this.pickerFace()
      } else if (this.surface === SURFACE_EMOJI) {
        this.emojiFace()
      } else if (this.surface === SURFACE_SYMBOLS) {
        this.symbolPanelFace()
      } else {
        this.handedKeys()
      }
  }

  /**
   * 按键，或按住某键时代替它们的键面（九键长按和日文变体），开启「单手模式」时在单手侧栏旁收窄绘制；大屏上侧栏和按键这一块缩到手机大小，贴到按键所在的那一侧（`oneHandBlockVp`）。关闭时与以前完全一样，直接画进调用方的列里。代替按键的面板不在这里绘制，保持全宽，与 Android 在没有按键行显示时隐藏侧栏一致。
   *
   * 大屏上空出来的那一侧可以画侧边数字键盘（`oneHandSidePadShown`，#10203）：键在右时排成 [面板, 空白, 块]，键在左时排成 [块, 空白, 面板]，面板贴屏幕边，中间的 `Blank` 是不接触摸的键盘底色。面板和按键在同一行里，代替按键的面板打开时这一行整个不画，侧边面板跟着不见。
   */
  @Builder
  handedKeys() {
    if (this.oneHandActive()) {
      Row() {
        if (this.oneHandSidePadShown() && OneHandedPolicy.railOnLeft(this.feedback.oneHanded)) {
          this.oneHandSidePad()
          Blank(OneHandedPolicy.RAIL_GAP_VP)
        }
        Row({ space: OneHandedPolicy.RAIL_GAP_VP }) {
          if (OneHandedPolicy.railOnLeft(this.feedback.oneHanded)) {
            this.oneHandRail()
          }
          Column({ space: this.rowSpacing() }) {
            this.keyArea()
          }
          .layoutWeight(1)
          if (!OneHandedPolicy.railOnLeft(this.feedback.oneHanded)) {
            this.oneHandRail()
          }
        }
        .width(this.oneHandBlockWidth())
        .alignItems(VerticalAlign.Center)
        if (this.oneHandSidePadShown() && !OneHandedPolicy.railOnLeft(this.feedback.oneHanded)) {
          Blank(OneHandedPolicy.RAIL_GAP_VP)
          this.oneHandSidePad()
        }
      }
      .width('100%')
      // 侧边面板与九键网格加底行一样高，底边对齐，数字行之类画在按键上方的行不影响它与网格逐行对齐。
      .alignItems(this.oneHandSidePadShown() ? VerticalAlign.Bottom : VerticalAlign.Center)
      .justifyContent(OneHandedPolicy.railOnLeft(this.feedback.oneHanded) ? FlexAlign.End : FlexAlign.Start)
    } else {
      this.keyArea()
    }
  }

  /** 侧边数字键盘此刻画不画（`OneHandedPolicy.sidePadShown`）。 */
  private oneHandSidePadShown(): boolean {
    return OneHandedPolicy.sidePadShown(this.oneHandedSidePanel, this.oneHandActive(), this.oneHandSidePadVp,
      this.nineKeyFamilyLettersShowing());
  }

  /** 主键区此刻画的是不是九键一族（拼音九键、九键双拼、笔画）的字母层：英文九键、假名九键、本地模式、123 层和借来的数字九宫格都不算。 */
  private nineKeyFamilyLettersShowing(): boolean {
    return this.touchDesign() && !this.symbols && !this.english && !this.japanese
      && this.scheme.engineScheme !== 'japanese' && this.localMode === 'none' && !this.numberPadShowing()
      && (KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode) || this.strokeFace());
  }

  /**
   * 大屏单手时空出来那一侧的数字键盘：3 列 × 4 行，`1 2 3 / 4 5 6 / 7 8 9 / . 0 ⌫`。前三行占九键网格的高度、与网格逐行同高，第四行与底行同高；键距、行距、键面和触摸区与主键区的键相同，键与键之间的空隙按 `keyHitRegion` 归最近的键。
   *
   * 数字和小数点走数字层的字面上屏（`insertNumberPadLiteral`）：组字中先把拼音原样上屏。⌫ 与九键右列的 ⌫ 同一个键，带连删。统计记普通数字键、`Period` 和 `Backspace`。
   */
  @Builder
  oneHandSidePad() {
    Column({ space: this.rowSpacing() }) {
      Column({ space: this.rowSpacing() }) {
        ForEach(OneHandedPolicy.sidePadDigitRows(), (row: string[]) => {
          Row({ space: this.keySpacing() }) {
            ForEach(row, (digit: string) => {
              this.fixedKey(this.sidePadDigitKey(digit))
            }, (digit: string) => `side-pad:${digit}:${this.faceKey()}`)
          }
          .layoutWeight(1)
          .width('100%')
        }, (row: string[]) => `side-pad-row:${row[0]}:${this.faceKey()}`)
      }
      .width('100%')
      .height(this.gridHeight())
      Row({ space: this.keySpacing() }) {
        this.fixedKey({
          face: '.',
          label: OneHandedPolicy.sidePadDecimalLabel(),
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: 0,
          share: 1,
          action: () => {
            KeyboardSession.shared.softKeyPress(KeyIdPolicy.character('.'));
            this.insertNumberPadLiteral('.');
          }
        })
        this.fixedKey(this.sidePadDigitKey('0'))
        this.deleteKey('100%', true, 1)
      }
      .width('100%')
      .height(this.rowHeight(3))
    }
    .width(this.oneHandSidePadVp)
    .id('oneHandSidePad')
  }

  /** 侧边数字键盘的一个数字键：字号与数字层的数字相同，读「数字 N」。 */
  private sidePadDigitKey(digit: string): KeyOptions {
    return {
      face: digit,
      label: OneHandedPolicy.sidePadDigitLabel(digit),
      size: TOUCH_LETTER_FONT_SIZE,
      emphasized: false,
      width: 0,
      share: 1,
      action: () => {
        KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(digit));
        this.insertNumberPadLiteral(digit);
      }
    };
  }

  /** 侧栏与按键这一块的宽度：手机铺满整行，大屏上是 `oneHandBlockVp`。 */
  private oneHandBlockWidth(): Length {
    return this.oneHandBlockVp > 0 ? this.oneHandBlockVp : '100%';
  }

  /** 当前显示的那个键面。 */
  @Builder
  keyArea() {
    if (this.surface === SURFACE_NINE_KEY_HOLD) {
      this.nineKeyHoldFace()
    } else if (this.japanese && this.surface === SURFACE_VARIANTS && !this.symbols) {
      this.variantFace()
    } else {
      this.typingFace()
    }
  }

  /** 单手侧栏：换边，或退出该模式。 */
  @Builder
  oneHandRail() {
    Column() {
      OneHandRail({
        keysOnRight: OneHandedPolicy.railOnLeft(this.feedback.oneHanded),
        keyBackground: this.skin.keyBackground,
        keyForeground: this.skin.keyForeground,
        secondary: this.skin.secondary,
        shadowRadius: this.skin.shadowRadius,
        shadowColor: this.skin.shadowColor(),
        shadowOffset: this.skin.shadowOffset,
        onSwap: (): void => this.swapOneHandedSide(),
        onExit: (): void => this.setOneHanded(OneHandedMode.OFF)
      })
    }
    .width(`${OneHandedPolicy.RAIL_PERCENT}%`)
  }

  /**
   * 字母上方的数字行（共享偏好 `touch_number_row`）：1 到 0 十个等宽键，20fp，与 123 层第一行同号。行高是不带高度调整的一行键高，正好是 `KeyboardMetrics.totalHeightVp` 为它多算的那一行。数字的输入见 `typeNumberRowDigit`。
   */
  @Builder
  numberKeysRow() {
    Row({ space: this.keySpacing() }) {
      if (this.splitDrawn()) {
        // 分离式键盘：12345 | 67890，中间是不接触摸的空隙。
        ForEach(TouchLayoutStylePolicy.NUMBER_ROW.slice(0, this.splitEvenRow(TouchLayoutStylePolicy.NUMBER_ROW.length).cut),
          (digit: string) => {
            this.numberRowKey(digit)
          }, (digit: string) => `number-row:${digit}:${this.faceKey()}`)
        this.splitGap(this.splitEvenRow(TouchLayoutStylePolicy.NUMBER_ROW.length).gapShare)
        ForEach(TouchLayoutStylePolicy.NUMBER_ROW.slice(this.splitEvenRow(TouchLayoutStylePolicy.NUMBER_ROW.length).cut),
          (digit: string) => {
            this.numberRowKey(digit)
          }, (digit: string) => `number-row:${digit}:${this.faceKey()}`)
      } else {
        ForEach(TouchLayoutStylePolicy.NUMBER_ROW, (digit: string) => {
          this.numberRowKey(digit)
        }, (digit: string) => `number-row:${digit}:${this.faceKey()}`)
      }
    }
    .width('100%')
    .height(KeyboardMetrics.ROW_HEIGHT_VP)
  }

  /** 数字行的一个键。 */
  @Builder
  numberRowKey(digit: string) {
    this.fixedKey({
      face: digit,
      label: digit,
      size: LAYER_FIRST_ROW_FONT_SIZE,
      emphasized: false,
      width: 0,
      share: 1,
      action: () => this.typeNumberRowDigit(digit)
    });
  }

  /** 分离式键盘中间的空隙：不是键、不接触摸，读屏也跳过它；份额由 `SplitKeyboardPolicy` 算，触摸落在这里什么也不按，与 Android 的空隙视图相同。 */
  @Builder
  splitGap(share: number) {
    Column()
      .layoutWeight(share)
      .height('100%')
      .accessibilityLevel('no')
  }

  /**
   * 数字行开着时 123、#+= 层和借用的数字九宫格顶部的补充行（`TouchLayoutStylePolicy.symbolExtraRow`，即 Android 的 `addSymbolExtraRow`）：与数字行同高，切到这几层时键盘不变矮，补的是这一层原来没有的字符。按键与该层其他字符键一样输入（`tapLayerCharacter`）。
   */
  @Builder
  symbolExtraKeysRow() {
    Row({ space: this.keySpacing() }) {
      if (this.splitDrawn()) {
        ForEach(this.symbolExtraKeys().slice(0, this.splitEvenRow(this.symbolExtraKeys().length).cut), (text: string) => {
          this.symbolExtraKey(text)
        }, (text: string) => `symbol-extra:${text}:${this.faceKey()}`)
        this.splitGap(this.splitEvenRow(this.symbolExtraKeys().length).gapShare)
        ForEach(this.symbolExtraKeys().slice(this.splitEvenRow(this.symbolExtraKeys().length).cut), (text: string) => {
          this.symbolExtraKey(text)
        }, (text: string) => `symbol-extra:${text}:${this.faceKey()}`)
      } else {
        ForEach(this.symbolExtraKeys(), (text: string) => {
          this.symbolExtraKey(text)
        }, (text: string) => `symbol-extra:${text}:${this.faceKey()}`)
      }
    }
    .width('100%')
    .height(KeyboardMetrics.ROW_HEIGHT_VP)
  }

  private symbolExtraKeys(): string[] {
    return TouchLayoutStylePolicy.symbolExtraRow(this.moreSymbols, this.numberPadShowing(),
      this.numberPadPunctuation() !== NumberPadPunctuation.ASCII);
  }

  /** 补充行的一个键：数字九宫格顶部那一排与数字九宫格上其他键同一条规则（`typeNumberPadText`），#+= 层的 1 到 0 照 123 层字符键（`tapLayerCharacter`）。 */
  @Builder
  symbolExtraKey(text: string) {
    this.fixedKey({
      face: text,
      label: KeyAccessibilityPolicy.symbol(text),
      size: LAYER_FIRST_ROW_FONT_SIZE,
      emphasized: false,
      width: 0,
      share: 1,
      action: () => {
        if (this.numberPadShowing()) {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(text));
          this.typeNumberPadText(text);
          return;
        }
        this.tapLayerCharacter(text);
      }
    });
  }

  /** The actual keys, shared by normal input and the transparent adjustment overlay. */
  @Builder
  typingFace() {
      if (this.numberRowExtraRows() > 0) {
        // 字母层上是 1 到 0 的数字行；123、#+= 层和借用的数字九宫格上是同样高的补充行，切层时键盘高度不变。
        if (this.symbols) {
          this.symbolExtraKeysRow()
        } else {
          this.numberKeysRow()
        }
      }
      if (this.kanaGridShowing()) {
        // 假名网格的字母层，以及 2in1 上它自己的数字层；触屏上 123 打开下面的数字九宫格，#+= 是设计稿的扩展符号页。
        this.japaneseFace()
      } else if (KeyboardScheme.usesHandwritingFace(
        this.scheme, this.english, this.symbols, this.localMode)) {
        this.handwritingFace()
      } else if (this.numberPadShowing() || (KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)
        && (!this.symbols || !this.moreSymbols || !this.touchDesign()))) {
        this.nineKeyFace()
      } else if (this.zhuyinFace() && !this.symbols) {
        this.dachenFace()
      } else if (this.strokeFace() && !this.symbols) {
        this.strokeKeypadFace()
      } else if (this.drawsFourteenKey() && !this.symbols) {
        // 字母组网格只在字母层画自己的键组，数字页复用前面的九宫格。
        if (this.drawsEighteenKey()) {
          this.eighteenKeyFace()
        } else {
          this.fourteenKeyFace()
        }
      } else if (this.designLayerShowing()) {
        this.designLayerFace()
      } else if (this.splitDrawn()) {
        this.splitLetterFace()
        this.actionRow()
      } else {
        Row({ space: this.keySpacing() }) {
          ForEach(this.symbols ? SYMBOL_ROWS[0] : LETTER_ROWS[0], (key: string) => {
            this.flexibleKey(this.symbols ? this.symbolKey(key) : this.letterKey(key));
          }, (key: string) => key + this.faceKey())
        }
        .width('100%')
        .height(this.rowHeight(0))
        // The rows rather than the keys watch for a glide: a touch keeps reporting to the row it went down in wherever the finger goes, and the keys themselves stay plain taps.
        .onTouch((event: TouchEvent): void => this.glideTouch(event))

        Row({ space: this.keySpacing() }) {
          if (this.secondRowIndented()) {
            Column()
              .layoutWeight(SECOND_ROW_INDENT_SHARE)
              .height('100%')
          }
          ForEach(this.symbols ? SYMBOL_ROWS[1] : LETTER_ROWS[1], (key: string) => {
            this.flexibleKey(this.symbols ? this.symbolKey(key) : this.letterKey(key));
          }, (key: string) => key + this.faceKey())
          if (this.secondRowSemicolon()) {
            this.flexibleKey(this.letterKey(';'));
          }
          if (this.secondRowIndented()) {
            Column()
              .layoutWeight(SECOND_ROW_INDENT_SHARE)
              .height('100%')
          }
        }
        .width('100%')
        .height(this.rowHeight(1))
        .onTouch((event: TouchEvent): void => this.glideTouch(event))

        Row({ space: this.keySpacing() }) {
          if (!this.symbols) {
            // 触屏上开启的 Shift 用字母键的填充色加强调色箭头，与设计稿一致；2in1 屏幕键盘仍用操作色填充。
            this.letterShiftKey()
          }
          ForEach(this.symbols ? SYMBOL_ROWS[2] : LETTER_ROWS[2], (key: string) => {
            this.flexibleKey(this.symbols
              ? this.symbolKey(KeyboardScheme.symbolRowKey(key, this.tibetanFace()))
              : this.letterKey(key));
          }, (key: string) => key + this.faceKey())
          this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, this.touchDesign() ? LETTER_EDGE_SHARE : 0);
        }
        .width('100%')
        .height(this.rowHeight(2))
        .onTouch((event: TouchEvent): void => this.glideTouch(event))

        this.actionRow()
      }
  }

  /** 第三排字母左端的 ⇧（全拼组字时是「分词」），分离与否画法相同。 */
  @Builder
  letterShiftKey() {
    this.fixedKey({
      face: this.usesQuanpinSeparator() ? '分词'
        : (this.caseMode === LetterCaseMode.CAPS_LOCK ? '⇪' : '⇧'),
      label: this.usesQuanpinSeparator() ? '拼音分词' : this.letterCase.accessibilityLabel(this.english),
      size: this.usesQuanpinSeparator() ? 14 : KeyboardMetrics.KEY_FONT_SIZE,
      emphasized: !this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE,
      special: this.usesQuanpinSeparator() || !(this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE),
      width: KeyboardMetrics.MODIFIER_WIDTH_VP,
      share: this.touchDesign() ? LETTER_EDGE_SHARE : 0,
      icon: !this.touchDesign() || this.usesQuanpinSeparator() ? ''
        : (this.caseMode === LetterCaseMode.CAPS_LOCK ? KeyboardIconPaths.CAPS_LOCK : KeyboardIconPaths.SHIFT),
      accentIcon: !this.usesQuanpinSeparator() && this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE,
      action: () => this.tapLetterShift()
    });
  }

  /**
   * 分离式键盘的三排字母（#10224，规则见 `SplitKeyboardPolicy`）：每排在键边界上断开，中间是不接触摸的空隙；第二、三排在右半边内侧重复左半边最内侧的字母（asdfg | ghjkl、⇧zxcv | vbnm⌫），重复的键从空隙借宽，三排键宽不变。键与不分离时同一套：字母、`;`、第二排的半键缩进、⇧ 和 ⌫ 都照原样画，只是排成两段。
   */
  @Builder
  splitLetterFace() {
    ForEach([0, 1, 2], (row: number) => {
      Row({ space: this.keySpacing() }) {
        ForEach(this.splitLetterItems(row).slice(0, this.splitLetterRow(row).cut), (item: string) => {
          this.splitLetterItem(item)
        }, (item: string) => `split-left:${item}:${this.faceKey()}`)
        this.splitGap(this.splitLetterRow(row).gapShare)
        if (this.splitLetterRow(row).duplicate) {
          this.flexibleKey(this.duplicateLetterKey(this.splitLetterItems(row)[this.splitLetterRow(row).cut - 1]))
        }
        ForEach(this.splitLetterItems(row).slice(this.splitLetterRow(row).cut), (item: string) => {
          this.splitLetterItem(item)
        }, (item: string) => `split-right:${item}:${this.faceKey()}`)
      }
      .width('100%')
      .height(this.rowHeight(row))
      .onTouch((event: TouchEvent): void => this.glideTouch(event))
    }, (row: number) => `split-row:${row}:${this.faceKey()}`)
  }

  /** 分离式键盘一排字母里依次画的东西：字母，以及占位符 `SPLIT_INDENT_START` / `SPLIT_INDENT_END`（第二排两端的半键缩进）、`SPLIT_SHIFT`、`SPLIT_DELETE`；微软双拼的 `;` 照原样写成 `;`。 */
  private splitLetterItems(row: number): string[] {
    const items: string[] = [];
    if (row === 1 && this.secondRowIndented()) {
      items.push(SPLIT_INDENT_START);
    }
    if (row === 2) {
      items.push(SPLIT_SHIFT);
    }
    for (const letter of LETTER_ROWS[row]) {
      items.push(letter);
    }
    if (row === 1 && this.secondRowSemicolon()) {
      items.push(';');
    }
    if (row === 1 && this.secondRowIndented()) {
      items.push(SPLIT_INDENT_END);
    }
    if (row === 2) {
      items.push(SPLIT_DELETE);
    }
    return items;
  }

  /** 分离式键盘一排怎么断开，份额与不分离时各子视图的 `layoutWeight` 相同。 */
  private splitLetterRow(row: number): SplitRow {
    const weights: number[] = this.splitLetterItems(row).map((item: string): number => {
      if (item === SPLIT_INDENT_START || item === SPLIT_INDENT_END) {
        return SECOND_ROW_INDENT_SHARE;
      }
      return item === SPLIT_SHIFT || item === SPLIT_DELETE ? LETTER_EDGE_SHARE : 1;
    });
    return this.splitRow(weights, row);
  }

  @Builder
  splitLetterItem(item: string) {
    if (item === SPLIT_INDENT_START || item === SPLIT_INDENT_END) {
      Column()
        .layoutWeight(SECOND_ROW_INDENT_SHARE)
        .height('100%')
    } else if (item === SPLIT_SHIFT) {
      this.letterShiftKey()
    } else if (item === SPLIT_DELETE) {
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, LETTER_EDGE_SHARE);
    } else {
      this.flexibleKey(this.letterKey(item));
    }
  }

  /** 右半边内侧重复的字母键：输入与左边那个相同，不记滑行键位。 */
  private duplicateLetterKey(letter: string): KeyOptions {
    const options: KeyOptions = this.letterKey(letter);
    options.duplicate = true;
    return options;
  }

  /**
   * 触屏键盘的符号层是否为设计稿的 #+= 层（`NumberSymbolLayout`）而不是共享符号行，规则见 `NumberSymbolLayout.drawsDesignLayer`。触屏的 123 一律是数字九宫格（`numberPadShowing`，`typingFace` 里先于这里判断），所以走到这里的只有 #+=：除注音外的每一种键面都用设计稿的 #+= 层。注音的 #+= 保留共享符号行，注音在那里输入原本会被其注音键占用的标点（， . / ; -），与 Android 一致，那一页底行的切换键印 123、回到数字九宫格（`designRowKey`）。`zhuyinFace` 只看方案的组字规则、不看键面，鸿蒙的注音也只有大千一种键面，所以每一种注音键面的 #+= 都走这里；2in1 屏幕键盘也保留共享符号行。
   */
  private designLayerShowing(): boolean {
    return NumberSymbolLayout.drawsDesignLayer(this.touchDesign(), this.symbols, this.zhuyinFace());
  }

  /** 该层是否带中文标点（全角，返回键为「拼音」）而不是 ASCII 标点（返回键为 ABC）：中文输入时是，英文和标点为 ASCII 的方案不是。 */
  private designLayerChinese(): boolean {
    return !this.english && !this.koreanFace() && !this.vietnameseFace() && !this.tibetanFace();
  }

  private designLayerRows(): LayerKey[][] {
    return NumberSymbolLayout.rows(this.moreSymbols, this.designLayerChinese(), this.cornerQuotes,
      this.stripIdle());
  }

  /** 123 / #+= 层第三排的字符键（不含两端的层切换键和 ⌫）。 */
  private designLayerCharacters(): LayerKey[] {
    return this.designLayerRows()[2].filter((key: LayerKey): boolean => key.kind === LayerKeyKind.CHARACTER);
  }

  /** 123 / #+= 层前两排怎么断开；不分离时 `cut` 是整排，全部画在第一个 ForEach 里。 */
  private splitLayerRow(row: number): SplitRow {
    const keys: LayerKey[] = this.designLayerRows()[row];
    if (!this.splitDrawn()) {
      return { cut: keys.length, duplicate: false, gapShare: 0 };
    }
    return this.splitRow(keys.map((key: LayerKey): number => key.weight), -1);
  }

  /** 123 / #+= 层第三排怎么断开：层切换键、字符键、⌫，`cut` 计入左端的层切换键。不分离时字符键全在第一个 ForEach 里。 */
  private splitLayerThirdRow(): SplitRow {
    const characters: LayerKey[] = this.designLayerCharacters();
    if (!this.splitDrawn()) {
      return { cut: characters.length + 1, duplicate: false, gapShare: 0 };
    }
    const weights: number[] = [NumberSymbolLayout.EDGE_WEIGHT];
    for (const key of characters) {
      weights.push(key.weight);
    }
    weights.push(NumberSymbolLayout.EDGE_WEIGHT);
    return this.splitRow(weights, -1);
  }

  /** 123 / #+= 层底行拆开的两个空格：返回、表情或符号、空格、回车。 */
  private splitLayerSpace(): SplitSpace {
    return this.splitSpace([NumberSymbolLayout.BACK_WEIGHT, NumberSymbolLayout.EMOJI_WEIGHT,
      NumberSymbolLayout.SPACE_WEIGHT, NumberSymbolLayout.RETURN_WEIGHT], 2);
  }

  /**
   * 组字中按到的辅助码触发键（123 层画成「；」的分号键、`/` 等，`ChineseHelpcodePolicy.layerTrigger`）交给 Engine 开一段辅助码，而不是按原样上屏再插入键面。Engine 收下时回到字母层，接下来要打的是辅助码字母；不收时返回 false，调用方照常输入键面。
   */
  private typeHelpcodeTrigger(text: string): boolean {
    const trigger: string | null = KeyboardSession.shared.helpcodeTrigger(text);
    if (trigger === null || !KeyboardSession.shared.press(trigger.charCodeAt(0), false)) {
      return false;
    }
    this.symbols = false;
    this.moreSymbols = false;
    return true;
  }

  /** 字符键输入它显示的内容；哪些键仍走符号行路径见 `NumberSymbolLayout.route`。 */
  private tapLayerCharacter(text: string): void {
    if (NumberSymbolLayout.route(text, this.designLayerChinese()) === LayerCharacterRoute.SYMBOL_KEY) {
      this.tapSymbol(text);
      return;
    }
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.character(text));
    if (this.typeHelpcodeTrigger(text)) {
      return;
    }
    // 已有组合按原样上屏，与 Android 的 `commitNineKeyLiteral` 一致。
    KeyboardSession.shared.commitRaw();
    KeyboardSession.shared.insert(text);
  }

  private layerCharacterKey(key: LayerKey, size: number): KeyOptions {
    return {
      face: key.text,
      label: KeyAccessibilityPolicy.symbol(key.description),
      size: size,
      emphasized: false,
      width: 0,
      share: key.weight,
      action: () => this.tapLayerCharacter(key.text)
    };
  }

  /** `#+=` 和 `123`，在两层之间切换。 */
  private toggleMoreSymbols(): void {
    KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_LAYER);
    this.moreSymbols = !this.moreSymbols;
  }

  /**
   * 触屏键盘的 123 和 #+= 层（Android 的 `ImeLetterRows.rebuildDesignLayer`）：两行各十个字符，层切换键、五个标点和删除键，然后用该层自己的底行代替操作行：「拼音」或 ABC 回到字母，123 层上是表情、#+= 层上是「符号」，以及空格和回车。
   */
  @Builder
  designLayerFace() {
    Row({ space: this.keySpacing() }) {
      // 分离式键盘时这一层也从中间断开（`splitLayerRow`），不重复键；不分离时 `cut` 是整排，空隙不画。
      ForEach(this.designLayerRows()[0].slice(0, this.splitLayerRow(0).cut), (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FIRST_ROW_FONT_SIZE));
      }, (key: LayerKey) => key.text + this.faceKey())
      if (this.splitDrawn()) {
        this.splitGap(this.splitLayerRow(0).gapShare)
      }
      ForEach(this.designLayerRows()[0].slice(this.splitLayerRow(0).cut), (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FIRST_ROW_FONT_SIZE));
      }, (key: LayerKey) => `right:${key.text}${this.faceKey()}`)
    }
    .width('100%')
    .height(this.rowHeight(0))

    Row({ space: this.keySpacing() }) {
      ForEach(this.designLayerRows()[1].slice(0, this.splitLayerRow(1).cut), (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
      }, (key: LayerKey) => key.text + this.faceKey())
      if (this.splitDrawn()) {
        this.splitGap(this.splitLayerRow(1).gapShare)
      }
      ForEach(this.designLayerRows()[1].slice(this.splitLayerRow(1).cut), (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
      }, (key: LayerKey) => `right:${key.text}${this.faceKey()}`)
    }
    .width('100%')
    .height(this.rowHeight(1))

    Row({ space: this.keySpacing() }) {
      this.fixedKey({
        face: this.designLayerRows()[2][0].text,
        label: this.designLayerRows()[2][0].description,
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: NumberSymbolLayout.EDGE_WEIGHT,
        action: () => this.toggleMoreSymbols()
      });
      // 左端的层切换键总在左半边，所以字符键按断点少一个切开。
      ForEach(this.designLayerCharacters().slice(0, this.splitLayerThirdRow().cut - 1),
        (key: LayerKey) => {
          this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
        }, (key: LayerKey) => key.text + this.faceKey())
      if (this.splitDrawn()) {
        this.splitGap(this.splitLayerThirdRow().gapShare)
      }
      ForEach(this.designLayerCharacters().slice(this.splitLayerThirdRow().cut - 1),
        (key: LayerKey) => {
          this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
        }, (key: LayerKey) => `right:${key.text}${this.faceKey()}`)
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, NumberSymbolLayout.EDGE_WEIGHT);
    }
    .width('100%')
    .height(this.rowHeight(2))

    Row({ space: this.keySpacing() }) {
      // 「符」与「123」互换后字母层是 符 | 123：这一层换成 😀/符号 | 返回，返回键留在字母层 123 那一格（`designLayerSymbolFirst`）。
      if (this.designLayerSymbolFirst()) {
        this.designLayerPanelKey()
        this.designLayerBackKey()
      } else {
        this.designLayerBackKey()
        this.designLayerPanelKey()
      }
      if (this.splitDrawn()) {
        this.spaceKey(true, this.splitLayerSpace().left, 0);
        this.splitGap(this.splitLayerSpace().gap)
        this.spaceKey(true, this.splitLayerSpace().right, 0);
      } else {
        this.spaceKey(true, NumberSymbolLayout.SPACE_WEIGHT, 0);
      }
      this.designReturnKey(NumberSymbolLayout.RETURN_WEIGHT, false);
    }
    .width('100%')
    .height(this.rowHeight(3))
  }

  /** 123 / #+= 层底行回到字母的「返回」。 */
  @Builder
  designLayerBackKey() {
    this.fixedKey({
      face: NumberSymbolLayout.lettersKeyTitle(this.designLayerChinese()),
      label: this.designLayerRows()[3][0].description,
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      fontWeight: FUNCTION_LABEL_WEIGHT,
      emphasized: false,
      special: true,
      width: 0,
      share: NumberSymbolLayout.BACK_WEIGHT,
      action: () => this.toggleSymbolLayer()
    });
  }

  /** 123 / #+= 层底行的第二个键：长空格样式是逗号，#+= 层是「符号」，123 层是表情。 */
  @Builder
  designLayerPanelKey() {
    if (this.longSpaceActive()) {
      // 中文 26 键的长空格样式：左下的 😀/符号 换成逗号，上滑或下滑输入句号，与 Android 长空格布局的数字层相同；表情和符号面板仍从工具栏进入。
      Column() {
        this.fixedKey({
          face: this.chinesePunctuationFaces() ? '，' : ',',
          label: '逗号；上滑或下滑输入句号',
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: '100%',
          action: () => {
            if (this.commaSwipe.swallows()) {
              return;
            }
            this.typeComma();
          }
        });
      }
      .layoutWeight(NumberSymbolLayout.EMOJI_WEIGHT)
      .height('100%')
      .onTouch((event: TouchEvent): void => this.commaSwipeTouch(event))
    } else if (this.moreSymbols) {
      this.fixedKey({
        face: this.designLayerRows()[3][1].text,
        label: this.designLayerRows()[3][1].description,
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: NumberSymbolLayout.EMOJI_WEIGHT,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_SYMBOL);
          this.openSymbolSurface();
        }
      });
    } else {
      this.fixedKey({
        face: this.designLayerRows()[3][1].text,
        label: this.designLayerRows()[3][1].description,
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        emphasized: false,
        special: true,
        width: 0,
        share: NumberSymbolLayout.EMOJI_WEIGHT,
        icon: KeyboardIconPaths.TOOLBAR_EMOJI,
        action: () => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_EMOJI);
          this.openEmojiSurface();
        }
      });
    }
  }

  /** 123 / #+= 层底行是否把返回键排到第二格：打开「符」与「123」互换、并且这块键面的字母层底行以 符 打头（`letterRowSymbolLeads`），返回键留在字母层 123 那一格（#10061 的规则）。九键和笔画的字母层本来就以 符 打头，它们的 #+= 层不跟着这一项变。 */
  private designLayerSymbolFirst(): boolean {
    return !KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode) && !this.strokeFace()
      && this.letterRowSymbolLeads();
  }

  /** The Zhuyin Dachen keys: four rows in the height of the three letter rows, Delete closing the last, then the shared action row. The symbol layer stays the shared one. */
  @Builder
  dachenFace() {
    Row({ space: this.keySpacing() }) {
      ForEach(ZhuyinLayout.ROWS[0], (key: string) => {
        this.flexibleKey(this.zhuyinKey(key));
      }, (key: string) => key + this.faceKey())
    }
    .width('100%')
    .height(this.dachenRowHeight())

    Row({ space: this.keySpacing() }) {
      ForEach(ZhuyinLayout.ROWS[1], (key: string) => {
        this.flexibleKey(this.zhuyinKey(key));
      }, (key: string) => key + this.faceKey())
    }
    .width('100%')
    .height(this.dachenRowHeight())

    Row({ space: this.keySpacing() }) {
      ForEach(ZhuyinLayout.ROWS[2], (key: string) => {
        this.flexibleKey(this.zhuyinKey(key));
      }, (key: string) => key + this.faceKey())
    }
    .width('100%')
    .height(this.dachenRowHeight())

    Row({ space: this.keySpacing() }) {
      ForEach(ZhuyinLayout.ROWS[3], (key: string) => {
        this.flexibleKey(this.zhuyinKey(key));
      }, (key: string) => key + this.faceKey())
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, 0);
    }
    .width('100%')
    .height(this.dachenRowHeight())

    this.actionRow()
  }

  /** 笔画键盘：九键的外框，左侧标点栏，中间 2×3 的笔画键，右侧删除键，底排与九键相同。符号层沿用共用的字母面，与大千注音一样。 */
  @Builder
  strokeKeypadFace() {
    if (this.tallReturnFrame()) {
      // 九键样式「右侧大回车」：笔画与拼音九键同一个外框（`tallReturnNineKeyFrame`）。
      this.tallReturnNineKeyFrame()
    } else {
      Column({ space: this.rowSpacing() }) {
        this.strokeGrid()
        this.actionRow()
      }
      .width('100%')
      .height(this.surfaceHeight())
    }
  }

  @Builder
  strokeGrid() {
    Row({ space: this.keySpacing() }) {
      Column() {
        this.strokeRailMarks()
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
      .backgroundColor(this.skin.keySurfaceBackground(false, true))
      .borderRadius(KeyboardMetrics.KEY_CORNER_VP)

      Column({ space: this.rowSpacing() }) {
        this.strokeCells()
      }
      .layoutWeight(1)
      .height('100%')

      Column() {
        this.deleteKey('100%', true, 0)
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }

  /** 笔画键盘左侧标点栏里的 ，。？！：普通外框和「右侧大回车」外框共用。用户自定义了字母层左列（`touch_nine_key_sidebar_symbols`）时换成可滚动的等高栈，一屏最多 `LETTER_SIDEBAR_VISIBLE_ROWS` 项，点按字面上屏；长按任一项打开设置。 */
  @Builder
  strokeRailMarks() {
    if (NineKeyLayout.customSidebar(this.letterSidebarTable())) {
      Scroll() {
        Column() {
          ForEach(this.strokeSidebarItems(), (item: NineKeySidebarItem, index: number) => {
            Column() {
              Text(item.symbol)
                .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
                .fontColor(this.skin.keyForeground)
                .textAlign(TextAlign.Center)
                .width('100%')
            }
            .height(item.height)
            .justifyContent(FlexAlign.Center)
            .width('100%')
            .accessibilityText(KeyAccessibilityPolicy.symbol(item.symbol))
            .responseRegion(this.railHitRegion(index, this.letterSidebarTable().length))
            .onClick(() => {
              KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
              KeyboardSession.shared.insertSidebarSymbol(item.symbol);
            })
            .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(false)))
          }, (item: NineKeySidebarItem) => item.key)
        }
        .width('100%')
      }
      .scrollBar(BarState.Off)
      .width('100%')
      .height('100%')
    } else {
      ForEach(NineKeyLayout.strokeSidebar(), (symbol: string, index: number) => {
        Column() {
          Text(this.symbolFace(symbol))
            .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
            .fontColor(this.skin.keyForeground)
            .textAlign(TextAlign.Center)
            .width('100%')
        }
        .layoutWeight(1)
        .justifyContent(FlexAlign.Center)
        .width('100%')
        .accessibilityText(KeyAccessibilityPolicy.symbol(symbol))
        .responseRegion(this.railHitRegion(index, NineKeyLayout.punctuation().length))
        .onClick(() => {
          KeyboardSession.shared.softKeyPress(KeyIdPolicy.SOFT_PUNCTUATION);
          KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
        })
        .gesture(LongPressGesture().onAction(() => this.openSidebarSettings(false)))
      }, (symbol: string) => symbol + this.faceKey())
    }
  }

  /** 笔画键盘中间的笔画键：普通外框和「右侧大回车」外框共用。 */
  @Builder
  strokeCells() {
    ForEach(StrokeLayout.ROWS, (row: StrokeKey[]) => {
      Row({ space: this.keySpacing() }) {
        ForEach(row, (key: StrokeKey) => {
          this.flexibleKey(this.strokeKey(key));
        }, (key: StrokeKey) => key.input + this.faceKey())
      }
      .layoutWeight(1)
      .width('100%')
    }, (row: StrokeKey[]) => row[0].input + this.faceKey())
  }
}
