import {
  KeyboardSession, KeyboardPreferences, KeyboardState, CandidateEntry, ExpandedCandidate,
  EmojiPage, EmojiRow, ClipboardHistoryMutation
} from './KeyboardSession';
import { KeyboardScheme, SchemeDefinition } from './KeyboardScheme';
import { KeyIdPolicy } from './KeyIdPolicy';
import { VoiceHotkeyAction } from './input/VoiceHotkeyPolicy';
import { BusinessErrorPolicy } from './BusinessErrorPolicy';
import { VoiceRecordingBehaviourPolicy } from './input/VoiceRecordingBehaviourPolicy';
import { KeyboardSkin } from './skin/KeyboardSkin';
import { CandidateColors, GlobalTheme } from './skin/GlobalTheme';
import { EnglishLetterCaseState, LetterCaseMode } from './input/EnglishLetterCaseState';
import { LetterKeyFacePolicy } from './input/LetterKeyFacePolicy';
import { ChineseHelpcodePolicy } from './input/ChineseHelpcodePolicy';
import { KeyAccessibilityPolicy } from './input/KeyAccessibilityPolicy';
import { CandidateGlossPolicy } from './candidate/CandidateGlossPolicy';
import { CandidateWrapPolicy } from './candidate/CandidateWrapPolicy';
import { KeyboardLayoutDragPolicy } from './input/KeyboardLayoutDragPolicy';
import { ExpandedCandidateLayout } from './candidate/ExpandedCandidateLayout';
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
import { CandidateSkinPolicy } from './candidate/CandidateSkinPolicy';
import { CandidateWheelPolicy } from './candidate/CandidateWheelPolicy';
import { KeyboardMetrics } from './KeyboardMetrics';
import {
  COMBO_IDLE_RESET_MILLIS, TypingEffect, TypingEffectPolicy, TypingEffectSettings
} from './TypingEffectPolicy';
import { HitOffset, KeyboardGeometry } from './KeyboardGeometry';
import { NineKeyLayout, NineKey } from './input/NineKeyLayout';
import { LayerCharacterRoute, LayerKey, LayerKeyKind, NumberSymbolLayout } from './input/NumberSymbolLayout';
import {
  NineKeyPanelBackspace, NineKeyPanelMode, NineKeyPanelPolicy, NineKeyStroke
} from './input/NineKeyPanelPolicy';
import { SpaceCursorMovement } from './input/SpaceCursorMovement';
import { GlideArming, GlideKeyRect, GlidePoint, GlideTypingPolicy } from './input/GlideTypingPolicy';
import { LetterHintTable } from './input/LetterHintTable';
import { HintKeyRect, LetterHintGesture } from './input/LetterHintGesture';
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
import { KeyboardLog, describeError } from './KeyboardLog';
import { KeyboardFormFactor } from './KeyboardFormFactor';
import { DesktopSurface } from './DesktopSurface';
import { SurfaceRoutingPolicy } from './SurfaceRoutingPolicy';
import { ClipboardHistoryItem, ClipboardFailure } from './clipboard/ClipboardHistoryStore';
import { ClipboardHistoryPolicy } from './clipboard/ClipboardHistoryPolicy';
import {
  CLOUD_CLIPBOARD_SEND, CLOUD_CLIPBOARD_TAB, CloudClipboardItem, CloudClipboardListing, CloudClipboardPolicy,
  CloudClipboardSendOutcome, CloudClipboardState
} from './clipboard/CloudClipboardPolicy';
import componentSnapshot from '@ohos.arkui.componentSnapshot';
import { image } from '@kit.ImageKit';
import { textRecognition } from '@kit.CoreVisionKit';
import {
  HandwritingPoint, HandwritingStroke, HandwritingStrokePolicy,
  HANDWRITING_CANVAS_SIZE, HANDWRITING_MAX_POINTS, HANDWRITING_MAX_STROKES
} from './input/HandwritingStrokePolicy';
import {
  HandwritingRecognitionQueue, HandwritingRecognitionTicket
} from './input/HandwritingRecognitionQueue';
import { HarmonyVoiceRecognizer } from './input/HarmonyVoiceRecognizer';
import { VoiceRecognitionPolicy, VOICE_DONE_NOTICE_MS, VOICE_SILENCE_STOP_MS } from './input/VoiceRecognitionPolicy';
import { VoiceInputConfigurationPolicy } from './input/VoiceInputConfiguration';
import { ReplyKeyboardPolicy, ReplyStyle } from './ReplyKeyboardPolicy';
import { CommunityReplyTemplate } from './CommunityReplyLibraryPolicy';
import { SymbolPanelCategory, SymbolPanelPolicy } from './input/SymbolPanelPolicy';
import { BackspaceHoldAction, BackspaceHoldPolicy } from './input/BackspaceHoldPolicy';
import { FunctionPanelItem, FunctionPanelPolicy } from './FunctionPanelPolicy';
import { KeyboardIconPaths } from './KeyboardIconPaths';
import { BrandTile } from './faces/BrandTile';
import { HeightBar } from './faces/HeightBar';
import { KeyboardLineIcon } from './faces/KeyboardLineIcon';
import { MiniKeyboardPreview } from './faces/MiniKeyboardPreview';
import { PagedTiles } from './faces/PagedTiles';
import { PanelTile } from './faces/PanelTile';
import { OneHandRail } from './faces/OneHandRail';
import { OneHandedMode, OneHandedPolicy } from './OneHandedPolicy';
import { PrivacyGate } from './PrivacyGate';
import { PhraseList, PHRASE_LIST_ERROR, PHRASE_LIST_LOADING, PHRASE_LIST_READY } from './faces/PhraseList';
import { VoiceListening } from './faces/VoiceListening';

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
// 角标提示距键上边和右边的距离（Android 的 `KeyHintButton.CORNER_HINT_TOP_DP` / `CORNER_HINT_RIGHT_DP`）。
const CORNER_HINT_TOP_VP: number = 3;
const CORNER_HINT_RIGHT_VP: number = 5;
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
const CHEVRON_TURN_MS: number = 200;
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
// 设计稿的底行：123 1.25 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9。
const LAYER_SHARE: number = 1.25;
const LANGUAGE_SHARE: number = 1.05;
const PUNCTUATION_SHARE: number = 1;
const SPACE_SHARE: number = 4;
const RETURN_SHARE: number = 1.9;
const SPACE_LABEL_FONT_SIZE: number = 13;
const SPACE_LABEL_GAP_VP: number = 4;
// 触屏 123 和 #+= 层（`NumberSymbolLayout`）：数字行 20fp，其下各行 18fp，与 Android 上同样的键尺寸一致。
const LAYER_FIRST_ROW_FONT_SIZE: number = 20;
const LAYER_FONT_SIZE: number = 18;
// 触屏九键键面，按宽度份额分配：三列各占 2 份的字母格两侧各有一列 1.15 份的标点，标点和字母格都是 17fp，字母格字重 500，右上角有 10fp 的数字。底行是 123 1.25 | space 4.2 | 0 1.05 | 中 1.05 | return 1.6。
const NINE_KEY_SIDE_SHARE: number = 1.15;
const NINE_KEY_GRID_SHARE: number = 6;
const NINE_KEY_FONT_SIZE: number = 17;
const NINE_KEY_LETTER_WEIGHT: number = 500;
const NINE_KEY_HINT_TOP_VP: number = 3;
const NINE_KEY_HINT_RIGHT_VP: number = 6;
const NINE_KEY_SPACE_SHARE: number = 4.2;
const NINE_KEY_ZERO_SHARE: number = 1.05;
const NINE_KEY_RETURN_SHARE: number = 1.6;
// 展开的候选网格：列宽至少 64vp、列间距 6vp，格高 44vp、圆角 8、词 17fp，最多 40 格，下方是 40vp 高、放「返回」和删除键的底栏。
const EXPANDED_MIN_COLUMN_VP: number = 64;
const EXPANDED_GAP_VP: number = 6;
const EXPANDED_PADDING_VP: number = 2;
const EXPANDED_CELL_HEIGHT_VP: number = 44;
const EXPANDED_CELL_PADDING_VP: number = 8;
const EXPANDED_CELL_RADIUS_VP: number = 8;
const EXPANDED_FONT_SIZE: number = 17;
const EXPANDED_PRESSED_BRIGHTNESS: number = 0.88;
const EXPANDED_FOOTER_VP: number = 40;
// 皮肤选择器的分页：网格上方 6vp、行间 4vp（预览自带 2vp 的描边余量），列间至少 6vp；有多页需要显示圆点时，圆点占 8vp 间距加自身 6vp，下方再留 4vp。
const SKIN_TOP_PADDING_VP: number = 6;
const SKIN_ROWS_GAP_VP: number = 4;
const SKIN_MIN_COLUMNS_GAP_VP: number = 6;
const PANEL_DOTS_ROOM_VP: number = 14;
// 功能面板和方案选择的磁贴网格：上方 10vp、行间 16vp，每行至少 52vp（设计稿和 Android 的 `FunctionPanelView`）；面板比这高时两行平分多出来的高度，而不是在底下空着一块。
const TILE_TOP_PADDING_VP: number = 10;
const TILE_ROWS_GAP_VP: number = 16;
const TILE_MIN_ROW_VP: number = 52;
const PANEL_BOTTOM_PADDING_VP: number = 4;
const PANEL_SIDE_PADDING_VP: number = 4;
const PANEL_COLUMNS: number = 4;
/** `MiniKeyboardPreview` 在缩略图下方和两侧额外占用的空间：上方和两侧各 2vp 描边余量，下方 6vp 间距和 12fp 的名称。 */
const SKIN_NAME_ROOM_VP: number = 24;
const SKIN_RING_ROOM_VP: number = 4;
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
// 触屏手写板的三列像 Android 的手写行一样按份额分配行宽：标点 .7，书写区 3，工具 .8。
const HANDWRITING_PUNCTUATION_WEIGHT: number = 0.7;
const HANDWRITING_AREA_WEIGHT: number = 3;
const HANDWRITING_TOOLS_WEIGHT: number = 0.8;
const HANDWRITING_SIDE_GAP_VP: number = 6;
const HANDWRITING_PUNCTUATION_FONT_SIZE: number = 17;
const HANDWRITING_REWRITE_FONT_SIZE: number = 14;
const HANDWRITING_STATUS_FONT_SIZE: number = 13;
const HANDWRITING_GUIDE_INSET_VP: number = 14;
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
  { id: SYMBOLS_PANEL, title: '符号' }
];

const DESKTOP_EMOJI_PANEL_TABS: EmojiPanelTab[] = EMOJI_PANEL_TABS.concat([
  { id: CLIPBOARD_PANEL, title: '剪贴板' }
]);

// Said plainly rather than as an error, because nothing is broken: the platform reserves clipboard
// reads for system applications and this is not one.
const CLIPBOARD_NOT_PERMITTED: string =
  '系统未授予读取剪贴板的权限，第三方输入法无法保存剪贴板历史。';

const CHINESE_SYMBOL_FACES: Map<string, string> = new Map<string, string>([
  [',', '，'], ['.', '。'], ['?', '？'], ['!', '！'], [';', '；'], [':', '：'],
  ['\'', '‘'], ['"', '“'], ['\\', '、'], ['(', '（'], [')', '）'],
  ['[', '【'], [']', '】'], ['<', '《'], ['>', '》']
]);

@Entry
@Component
struct KeyboardView {
  @State candidates: CandidateEntry[] = [];
  @State editing: string = '';
  @State editingCaret: number = 0;
  /** The candidate page on screen, from zero, and how many there are, for the 2in1 page indicator. */
  @State candidatePage: number = 0;
  @State candidatePageCount: number = 1;
  @State phrasePrefix: string = '';
  @State spellings: string[] = [];
  @State engineScheme: number = 0;
  /** Whether the scheme's openable candidate list (the Korean Hanja list, the Zhuyin list) is on the strip, as the Engine last reported it. */
  @State candidateListOpen: boolean = false;
  /** Read-only English completions for the word being typed; empty whenever one is not. */
  @State englishSuggestions: string[] = [];
  @State shuangpinProfile: string = 'xiaohe';
  @State localMode: string = 'none';
  @State diagnostic: string = '';
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
  @State english: boolean = false;
  /** Which surface the toolbar asked this window to carry, on a machine with no key faces. */
  @State desktop: DesktopSurface = DesktopSurface.NONE;
  @State @Watch('dropLetterGestures') symbols: boolean = false;
  /** 在触屏键盘的符号层中，当前显示的是否是 #+= 层而不是 123 层；在字母层上始终为 false（Android 的 `ImeLetterRows.moreSymbols`）。 */
  @State @Watch('dropLetterGestures') moreSymbols: boolean = false;
  @State nineKey: boolean = false;
  /** Scheme 3 is Japanese, which has its own grid rather than a pinyin one. */
  @State japanese: boolean = false;
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
  private emojiGridScroller: Scroller = new Scroller();
  private symbolGridScroller: Scroller = new Scroller();
  private clipListScroller: Scroller = new Scroller();
  /** Bumped by every cloud fetch and every drop, so only the newest answer is applied. */
  private cloudRequest: number = 0;
  /** Bumped when a cloud send is cancelled or its target/context changes, so late answers are dropped. */
  private cloudSendRequest: number = 0;
  /** The editor the cloud listing on screen was fetched for. */
  private cloudEditor: number = -1;
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
  @State nineKeyHoldOptions: string[] = [];
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
  @State expanded: ExpandedCandidate[] = [];
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
  @State scheme: SchemeDefinition = KeyboardScheme.fallback();
  /** 五笔版本：方案仍是同一个五笔，只换卡片和工具栏上的「86」「98」字样；单独设成状态，只改版本时也会重绘。 */
  @State wubiProfile: string = KeyboardScheme.WUBI_86;
  @State handwritingStrokes: HandwritingStroke[] = [];
  @State handwritingRedo: HandwritingStroke[] = [];
  @State handwritingCandidates: string[] = [];
  @State handwritingNotice: string = '';
  @State handwritingBusy: boolean = false;
  @State voiceText: string = '';
  @State voiceNotice: string = '';
  @State voiceBusy: boolean = false;
  @State voiceFinishing: boolean = false;
  /** 触屏上，直接上屏的识别结果内容，在聆听界面上以「已识别：…」显示 `VOICE_DONE_NOTICE_MS`；其他时候为空。 */
  @State voiceDone: string = '';
  /** 触屏上，识别器目前听到的文字；用户选择显示中间结果时，聆听界面把它显示在标题下方。 */
  @State voiceHeard: string = '';
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
  /** 屏幕上按键行的高度调整值：已保存的值，或高度条打开期间正在预览的值。 */
  @State geometryHeight: number = KeyboardSession.shared.preferences().heightAdjustment;
  /** 「常用语」面板的条目及其读取状态（`PhraseList` 的 `PHRASE_LIST_*` 状态）。 */
  @State phrases: string[] = [];
  @State phraseState: string = PHRASE_LIST_LOADING;
  /** 每次从标志打开功能面板时递增，让 `PagedTiles` 回到第一页。 */
  @State panelResetToken: number = 0;
  /** 九键数字层的排列，`NumberKeypadOrder` 的取值。 */
  @State numberKeypadOrder: string = KeyboardSession.shared.preferences().numberKeypadOrder;

  /** 按下的键变暗和缩小的程度：触屏上是设计稿的 .86 和 .96；2in1 上保持不变，它的设备形态在进程存续期间不会改变。 */
  private readonly keyPressedBrightness: number = KeyboardFormFactor.isDesktop() ? 1 : KEY_PRESSED_BRIGHTNESS;
  private readonly keyPressedScale: number = KeyboardFormFactor.isDesktop() ? 1 : KEY_PRESSED_SCALE;
  private readonly letterCase: EnglishLetterCaseState = new EnglishLetterCaseState();
  private settings: KeyboardPreferences = KeyboardSession.shared.preferences();
  private readonly cursorDrag: SpaceCursorMovement = new SpaceCursorMovement();
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
  /** 当前录音的服务商是否边听边流式返回文字（`VoiceRecordingBehaviourPolicy.streamsPartialResults`）。只有这种录音才使用触屏聆听界面，点一下即取消，说话停顿即停止；上传式服务商仍用审阅界面及其「停止录音」。在 `voiceBusy` 之前设置，这样为忙碌状态绘制的界面读到的是新值。 */
  private voiceStreams: boolean = true;
  private voiceSilenceTimer: number = -1;
  private voiceDoneTimer: number = -1;
  private replyGeneration: number = 0;
  private polishGeneration: number = 0;
  /** 每次加载常用语和离开面板时递增，只绘制最新的结果。 */
  private phraseGeneration: number = 0;
  /** 高度条打开时已保存的高度调整值，「取消」及其他任何离开高度条的方式都会恢复到它。 */
  private heightSnapshot: number = 0;
  /** 高度条的拖柄是否正被拖动；手指抬起时拖动结束。 */
  private heightDragging: boolean = false;
  /** 本次拖动开始时的调整值。 */
  private heightDragBase: number = 0;
  /** 拖动开始以来手指的累计竖直位移，不取整，这样慢速拖动会逐步累积而不是被取整抹掉。 */
  private heightDragOffset: number = 0;
  /** 「手写」磁贴切换前的方案，再次点按回到它；键盘不是由该磁贴切换过来时为 null。 */
  private schemeBeforeHandwriting: SchemeDefinition | null = null;
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
  /** 键盘宽度，用于把按键预览气泡限制在键盘内。 */
  private rootWidth: number = 0;
  /** 按下字母键时其上方的按键预览气泡（按键弹出预览），以相对键盘左上角的 vp 计；未显示时为 null。 */
  @State keyPreview: KeyPreviewFrame | null = null;
  /** 气泡所属按键对应的手指，这样别处抬起的手指不会撤掉仍被按住的键的气泡；未显示时为 -1。 */
  private keyPreviewFinger: number = -1;
  /** 字母键上可能输入角标提示而不是字母的触摸。 */
  private letterHints: LetterHintGesture = new LetterHintGesture();
  /** 每个被跟踪手指的长按计时器。 */
  private hintHoldTimers: Map<number, number> = new Map<number, number>();
  private glideContext: CanvasRenderingContext2D =
    new CanvasRenderingContext2D(new RenderingContextSettings(true));

  aboutToAppear(): void {
    this.english = KeyboardSession.shared.isEnglish();
    this.clipboardEnabled = KeyboardSession.shared.clipboardHistoryAllowed();
    KeyboardSession.shared.onClipboardHistoryChanged = (enabled: boolean): void => {
      this.clipboardEnabled = enabled;
      if (!enabled) {
        this.clips = [];
        this.confirmingClear = false;
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
        if (this.emojiItems.length === 0) {
          this.loadEmoji(this.emojiGroup);
        }
      }
    };
    this.applySettings();
    // The editor may already be attached by the time the view appears, in which case its override
    // never produced a callback and has to be read off the session here.
    this.applyEditorFace();
    KeyboardSession.shared.onEditorChanged = (): void => {
      // A URI or password field opens in Latin whatever the user last chose, so the face has to
      // follow or it would claim a language the Engine is not composing in.
      this.english = KeyboardSession.shared.isEnglish();
      // 会话在每次获得焦点时重读按键反馈文件，设置应用所做的改动（字母键手势、滑行输入、手写板）就从这里进来，不带偏好修订号；视图的手势跟随这份副本。
      this.feedback = KeyboardSession.shared.feedbackSettings();
      this.applyEditorFace();
      this.applyAutomaticCase();
      this.syncCloudClipboardEditor();
    };
    KeyboardSession.shared.onViewPreferencesReloaded = (): void => {
      // The settings page saved while this view was alive; draw what it saved instead of what the view read when it appeared.
      this.applySettings();
      this.applyEditorFace();
      if (!KeyboardFormFactor.isDesktop()) {
        KeyboardSession.shared.resizeTouchKeyboard(KeyboardMetrics.totalHeightVp(
          this.geometryRowSpacing, this.geometryHeight, KeyboardSession.shared.glossRows(),
          this.settings.candidateFontSize, this.settings.candidatePreeditFontSize));
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
    KeyboardSession.shared.onChanged = (state: KeyboardState): void => {
      const wasHandwriting: boolean = KeyboardScheme.usesHandwritingFace(
        this.scheme, this.english, this.symbols, this.localMode);
      this.candidates = state.candidates;
      this.editing = state.editing;
      this.editingCaret = state.editingCaret;
      this.candidatePage = state.page;
      this.candidatePageCount = state.pageCount;
      this.phrasePrefix = state.phrasePrefix;
      this.spellings = state.spellings;
      this.nineKeySingleCharacter = state.nineKeySingleCharacter;
      this.nineKeyStrokes = state.nineKeyStrokes;
      this.engineScheme = state.scheme;
      this.candidateListOpen = state.candidateListOpen;
      this.japanese = state.scheme === JAPANESE_SCHEME && !state.english
        && this.scheme.touchKeyboardLayout === 'nine_key';
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
        // 这份列表属于一个已不存在的组合；继续显示它，就是为用户已经完成或放弃的输入提供选项。功能面板和高度条与组合无关：用硬件键盘输入组合时面板会让开，组合结束后再回来；高度条只能通过它自己的按钮离开。全拼九键的三栏面板例外：筛选可能把候选滤空，组字还在，要留着面板让用户撤掉那一笔，组字结束时 nineKeyPanel() 为假再收起。
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
    KeyboardSession.shared.setVoiceRecording(false, true);
  }

  /**
   * A tap deletes on release. A hold clears an unfinished composition once, or repeats through the
   * editor when there was no composition; it never crosses that boundary and eats committed text.
   */
  private startBackspace(): void {
    // Counted once per press here, on the way down, never per repeat of a hold.
    KeyboardSession.shared.recordKey(KeyIdPolicy.BACKSPACE);
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
   * 字母键平时画小写，与设计稿、Android 和 iOS 一致；只有下一键真的会输入大写时才画大写（`typesUppercase`）。
   */
  private letterFace(letter: string): string {
    if (this.koreanFace()) {
      return DubeolsikLayout.jamo(letter, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    if (this.vietnameseFace() || this.tibetanFace()) {
      // 越南语和藏文（威利转写区分大小写）的字母按 Shift 给出的大小写输入，所以键面显示那个大小写，和英文一样。
      return LetterKeyFacePolicy.face(letter, false, false, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    return LetterKeyFacePolicy.face(letter, !this.english, this.localMode !== 'none', this.typesUppercase());
  }

  /**
   * Shift 亮着时下一键是否真的输入大写，与 `tapLetter` 的判断一致：英文里是大写字母；中文里只有组字中的辅助码会以大写交给 Engine，没有组字时 Shift 加字母仍是小写拼音，键面画大写就是在骗人。读的都是视图的状态，组字开始或结束时键面跟着重绘。
   */
  private typesUppercase(): boolean {
    const held: boolean = this.caseMode !== LetterCaseMode.LOWERCASE;
    return held && (this.english
      || ChineseHelpcodePolicy.entersHelpcode(this.english, held, this.editing, this.engineScheme, this.localMode));
  }

  /** The double-pinyin unit a key stands for, printed along its bottom edge while it means one. */
  private letterHint(letter: string): string {
    return ShuangpinKeyHintPolicy.hint(this.shuangpinProfile, letter, this.english,
      this.engineScheme, this.localMode);
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(key));
    if (ZhuyinLayout.selectsWhileListOpen(key) && KeyboardSession.shared.candidateListOpen()) {
      KeyboardSession.shared.convertHanja();
    }
    KeyboardSession.shared.press(key.charCodeAt(0), false);
  }

  /**
   * Zhuyin: a mark whose ASCII key is a Dachen phonetic or tone key would compose rather than be typed, so the conversion is finished and the mark typed after it, in Chinese while the punctuation switch says so.
   */
  private typeZhuyinMark(symbol: string): void {
    const chinese: string | undefined = CHINESE_SYMBOL_FACES.get(symbol);
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(input));
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

  /** 底排是否用网格面板那一套（「符」代替逗号，逗号等标点在侧栏）：九键拼音，或不在符号层的笔画键盘。 */
  private gridActionRow(): boolean {
    return KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)
      || (this.strokeFace() && !this.symbols);
  }

  private tapLetter(letter: string): void {
    if (this.glideSwallowsTap) {
      // The stroke already typed what it meant; see glideSwallowsTap.
      return;
    }
    if (this.letterHints.swallows(letter)) {
      // 该键上的滑动或长按已在手指抬起时输入了角标提示。
      return;
    }
    // The key by its position, so Dubeolsik jamo and a shifted capital count on the same key as the plain letter.
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(letter));
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
    KeyboardSession.shared.press(letter.charCodeAt(0), shifted);
    // A one-shot Shift is spent by the letter that follows it; Caps Lock is not. Neither is an
    // automatic Shift: it is not spent but recomputed from the text, which the change this press
    // just caused has already done. Clearing it here would give a capitals-only field one capital.
    if (this.caseMode === LetterCaseMode.SHIFTED && !automatic) {
      this.letterCase.reset();
      this.caseMode = this.letterCase.mode();
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
      && !KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)
      && !this.zhuyinFace() && !this.strokeFace() && !this.koreanFace()
      && !this.vietnameseFace() && !this.tibetanFace();
  }

  /** A letter key was laid out; anything else drawn by flexibleKey is not a glide key. */
  private recordGlideKey(letter: string | undefined, area: Area): void {
    if (letter === undefined) {
      return;
    }
    const index: number = GlideTypingPolicy.letterIndex(letter);
    if (index < 0) {
      return;
    }
    const rect: GlideKeyRect = {
      left: Number(area.globalPosition.x ?? 0),
      top: Number(area.globalPosition.y ?? 0),
      width: Number(area.width),
      height: Number(area.height)
    };
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
      const typed: string | null = this.letterHints.up(finger);
      if (typed !== null) {
        this.typeLetterHint(letter, typed);
      }
    } else if (event.type === TouchType.Cancel) {
      for (const touch of event.changedTouches) {
        this.abandonLetterHint(touch.id);
      }
    }
  }

  /** 角标提示按原样输入，与 123 层的字符键输入键面的方式相同：已有组合先按原样上屏（Android 的 `commitNineKeyLiteral`，`COMMIT_RAW`）。它算作对来源键的一次按键。 */
  private typeLetterHint(letter: string, hint: string): void {
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(letter));
    KeyboardSession.shared.commitRaw();
    KeyboardSession.shared.insert(hint);
  }

  private armHintHold(finger: number): void {
    this.cancelHintHold(finger);
    const timer: number = setTimeout((): void => {
      this.hintHoldTimers.delete(finger);
      if (this.glideActive) {
        return;
      }
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
  private dropLetterGestures(): void {
    this.hintHoldTimers.forEach((timer: number): void => clearTimeout(timer));
    this.hintHoldTimers.clear();
    this.letterHints.clear();
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

  @Builder
  keyFace($$: KeyOptions) {
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
        Text($$.letter === undefined ? $$.face : this.letterFace($$.letter))
          .fontSize($$.size)
          .fontWeight($$.fontWeight ?? 400)
          .maxLines(1)
          .fontColor(this.skin.keyLabelColor($$.emphasized, $$.special === true))
          .fontFamily(this.skin.monospaced ? 'monospace' : undefined)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
      if ($$.hint !== undefined && $$.hint.length > 0) {
        // Along the bottom edge, which is why the letter above it is not centred in the whole key.
        Text($$.hint)
          .fontSize(KEY_HINT_FONT_SIZE)
          .maxLines(1)
          .fontColor(this.skin.accent)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
      if ($$.cornerHint !== undefined && $$.cornerHint.length > 0) {
        // 脱离列的布局流，按设计稿和 Android 的 `KeyHintButton` 的位置放在距顶部 3 vp、距右侧 5 vp 处，这样字母和双拼行保持原位。
        Text($$.cornerHint)
          .fontSize(KEY_HINT_FONT_SIZE)
          .maxLines(1)
          .fontColor(this.skin.secondary)
          .position({ top: CORNER_HINT_TOP_VP, right: CORNER_HINT_RIGHT_VP })
          .hitTestBehavior(HitTestMode.None)
      }
    }
    .justifyContent(FlexAlign.Center)
    .width('100%')
    .height('100%')
    // 键面只负责画，不接收触摸：触摸归外层的键，由它的 `responseRegion` 决定。否则键面这一层按自己的键帽大小被命中，挡住伸进这块键帽的邻键（紧挨回车的中/英键），而外层的回车又不在自己的触摸区里，这一下就落空了。
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
    .onAreaChange((_old: Area, now: Area): void => this.recordGlideKey($$.letter, now))
    .onTouch((event: TouchEvent): void => this.letterHintTouch($$.letter, $$.cornerHint, event))
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
   * 已安装的符号集插件组。插件只能在 2in1 的插件页安装，`symbol_set_packs` 能力也只在 2in1 声明，所以手机面板不去读。
   */
  private pluginSymbolGroups(): PluginSymbolGroup[] {
    return KeyboardFormFactor.isDesktop() ? KeyboardSession.shared.pluginSymbolGroups() : [];
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
    const cloud: boolean = clipboard && this.cloudTabActive();
    const clips: string[] = !clipboard ? []
      : cloud ? this.visibleCloudClips().map((entry: CloudClipboardItem): string => entry.text)
        : this.visibleClips().map((entry: ClipboardHistoryItem): string => entry.text);
    const items: string[] = clipboard ? [] : this.visibleEmojiItems();
    const decision: EmojiPanelKeyDecision = EmojiPanelKeyPolicy.decide(key,
      clipboard ? clips.length : items.length, clipboard ? 1 : EMOJI_COLUMNS, this.emojiSelected,
      this.emojiQuery);
    if (decision.action === EmojiPanelKeyAction.NONE) {
      return false;
    }
    if (decision.action === EmojiPanelKeyAction.CLOSE) {
      this.leaveSurface(DesktopSurface.EMOJI);
    } else if (decision.action === EmojiPanelKeyAction.SEARCH) {
      this.searchEmoji(decision.query);
      this.revealEmojiSelection(clipboard);
    } else if (decision.action === EmojiPanelKeyAction.ACTIVATE) {
      if (cloud) {
        this.insertCloudClip(clips[decision.index]);
      } else if (clipboard) {
        KeyboardSession.shared.insert(clips[decision.index]);
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
    this.emojiPanel = panel;
    this.resetEmojiSearch();
    if (panel === CLIPBOARD_PANEL) {
      this.clips = this.clipboardEnabled ? KeyboardSession.shared.clipboardHistory() : [];
      this.clipboardNotice = this.clipboardEnabled ? '' : '剪贴板历史已在设置中关闭';
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
      this.showEmojiPanel(this.emojiPanel);
    }
  }

  /** 长按底行的 123：在字母层直接打开符号面板，与点「符」键相同；符号层里那个键是返回，不响应长按。 */
  private openSymbolsFromLayerKey(): void {
    if (this.symbols) {
      return;
    }
    KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_SYMBOL);
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

  private openNineKeyHold(key: NineKey): void {
    const options: string[] = NineKeyLayout.holdOptions(key);
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
    this.nineKey = candidate.touchKeyboardLayout === 'nine_key';
    // The kana grid belongs to the scheme, not to what the Engine last reported: the Engine only
    // says so once something has been typed, and the face has to change before that.
    this.japanese = candidate.engineScheme === 'japanese' && candidate.touchKeyboardLayout
      === 'nine_key';
    KeyboardSession.shared.setNineKey(this.nineKey);
    this.show(SURFACE_NONE);
    return true;
  }

  /** 键盘是否处于手写状态：方案为「手写」，且英文没有接管按键。 */
  private handwritingSelected(): boolean {
    return !this.english && this.scheme.preferenceId === KeyboardScheme.HANDWRITING.preferenceId;
  }

  /**
   * 功能面板的「手写」磁贴：切换到手写板，或切回该磁贴切换前的方案。Android 的磁贴也这样来回切换；若键盘是经其他途径进入手写的，切回的是选择器提供的第一个其他方案，与 Android 的选法相同。
   */
  private toggleHandwritingScheme(): void {
    if (this.handwritingSelected()) {
      const back: SchemeDefinition = this.schemeBeforeHandwriting ?? this.firstNonHandwritingScheme();
      this.selectScheme(back);
      return;
    }
    const previous: SchemeDefinition = this.scheme;
    if (this.selectScheme(KeyboardScheme.HANDWRITING)
      && previous.preferenceId !== KeyboardScheme.HANDWRITING.preferenceId) {
      this.schemeBeforeHandwriting = previous;
    }
  }

  private firstNonHandwritingScheme(): SchemeDefinition {
    for (const candidate of this.offeredSchemes()) {
      if (candidate.preferenceId !== KeyboardScheme.HANDWRITING.preferenceId) {
        return candidate;
      }
    }
    return KeyboardScheme.fallback();
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LAYER);
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
    const grid: boolean = this.scheme.touchKeyboardLayout === 'nine_key'
      && !KeyboardSession.shared.prefersFullFace();
    if (grid === this.nineKey) {
      return;
    }
    this.nineKey = grid;
    // Same reason the scheme cards set this rather than waiting for the Engine: the Engine only
    // reports Japanese once something has been typed, and the face has to change before that.
    this.japanese = grid && this.scheme.engineScheme === 'japanese';
    KeyboardSession.shared.setNineKey(grid);
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
      const changes: Map<string, Object> = new Map<string, Object>();
      changes.set('touch_keyboard_layout', KeyboardScheme.QUANPIN.touchKeyboardLayout);
      if (!KeyboardSession.shared.changePreferences(changes)) {
        return;
      }
      this.scheme = this.resolveScheme(KeyboardScheme.QUANPIN.touchKeyboardLayout);
      this.japanese = false;
    }
    this.nineKey = nineKey;
    KeyboardSession.shared.setNineKey(nineKey);
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
  /**
   * The schemes this host can actually deliver.
   *
   * Handwriting and the thoughtful reply surface were both absent when this was written and both
   * exist now — handwriting through Core Vision Kit rather than an injected recognizer, and the
   * reply keyboard with its own surface — so the only thing filtered here is what the user turned
   * off, plus the grids a machine with real keys has no use for.
   *
   * Filtered here rather than in KeyboardScheme, whose declaration order is the fixed order every
   * host's pickers render in.
   */
  private offeredSchemes(): SchemeDefinition[] {
    return KeyboardScheme.SCHEMES.filter((candidate: SchemeDefinition): boolean =>
      this.settings.enabledSchemes.includes(candidate.preferenceId)
      // A grid is a way of getting twenty-six letters out of nine keys, which is a problem only a
      // touch screen has. On a machine with a physical keyboard every letter already has a key.
      && !(KeyboardFormFactor.isDesktop() && candidate.touchKeyboardLayout === 'nine_key'));
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
    return total;
  }

  /**
   * What is left of a surface once the action row and the gap above it are taken out.
   *
   * The faces that draw their own keys — the two grids and handwriting — end with actionRow(), so
   * their key area is this rather than the whole surface. Keeping the subtraction here is what makes
   * the keyboard the same height whichever layout is showing, which is what the Apple host asserts.
   */
  private gridHeight(): number {
    return this.surfaceHeight() - this.rowHeight(3) - this.rowSpacing();
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
    if (this.surface === SURFACE_GEOMETRY && surface !== SURFACE_GEOMETRY
      && this.geometryHeight !== this.heightSnapshot) {
      // 以「完成」以外的方式离开高度条不保留任何改动：预览恢复为已保存的高度。
      this.previewHeight(this.heightSnapshot);
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
    if (surface === SURFACE_REPLY) {
      this.replyTemplates = KeyboardSession.shared.communityReplyTemplates();
      this.replyTemplateMenu = false;
    }
    if (surface !== SURFACE_EXPANDED) {
      this.expanded = [];
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

  /** 重新取完整候选列表；长按释义指的那一行没了就收起释义条。 */
  private refreshExpanded(): void {
    this.expanded = KeyboardSession.shared.allCandidates();
    if (this.expandedGloss >= this.expanded.length) {
      this.expandedGloss = -1;
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

  /** 展开面板是否画成全拼九键的三栏。 */
  private nineKeyPanel(): boolean {
    return NineKeyPanelPolicy.threeColumn(KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode),
      SchemeCompositionPolicy.rulesScheme(this.engineScheme, this.english, this.localMode),
      this.editing.length > 0);
  }

  /** 三栏面板里 ⌫ 的这一下：笔画模式下先撤一笔，否则同键盘上的退格。 */
  private nineKeyPanelBackspace(): void {
    KeyboardSession.shared.recordKey(KeyIdPolicy.BACKSPACE);
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

  /** 功能面板的「键盘高度」：工具栏变成高度条，下方是实时按键，从已保存的高度开始。 */
  private openHeightMode(): void {
    this.heightSnapshot = this.geometryHeight;
    this.heightDragging = false;
    this.show(SURFACE_GEOMETRY);
  }

  /** 按某个调整值绘制按键但不保存：面板随之调整大小，所以预览就是真实高度。 */
  private previewHeight(adjustment: number): void {
    this.geometryHeight = KeyboardGeometry.heightAdjustment(adjustment);
    KeyboardSession.shared.resizeTouchKeyboard(KeyboardMetrics.totalHeightVp(
      this.geometryRowSpacing, this.geometryHeight, KeyboardSession.shared.glossRows(),
      this.settings.candidateFontSize, this.settings.candidatePreeditFontSize));
  }

  /** 高度条拖柄的一步：手指上移 1vp，按键行就高 1vp，范围以共享校验接受的为限。 */
  private dragHeight(deltaVp: number): void {
    if (!this.heightDragging) {
      this.heightDragging = true;
      this.heightDragBase = this.geometryHeight;
      this.heightDragOffset = 0;
    }
    this.heightDragOffset += deltaVp;
    const adjustment: number = KeyboardLayoutDragPolicy.height(this.heightDragBase, this.heightDragOffset);
    if (adjustment !== this.geometryHeight) {
      this.previewHeight(adjustment);
    }
  }

  /** 「完成」：保存预览的高度并回到工具栏。 */
  private finishHeightMode(): void {
    this.applyGeometry(this.geometryKeySpacing, this.geometryRowSpacing, this.geometryHeight);
    this.heightSnapshot = this.geometryHeight;
    this.show(SURFACE_NONE);
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.ENTER);
    const dispatch: ReturnDispatch = ReturnKeyAction.dispatch(
      this.japanese, this.editing.length > 0, this.candidates.length,
      KeyboardSession.shared.japaneseConversionActive(), this.koreanFace(), this.vietnameseFace(),
      this.strokeFace());
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.SPACE);
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
    KeyboardSession.shared.press(0x20, false);
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
    this.geometryHeight = this.settings.heightAdjustment;
    this.numberKeypadOrder = this.settings.numberKeypadOrder;
    // 取会话持有的副本而不是再读一次磁盘：会话在构建和每次获得焦点时读取，读不到时保留上次的值，视图跟着它就不会因一次读取失败把隐私模式等显示成关闭。
    this.feedback = KeyboardSession.shared.feedbackSettings();
    this.traditional = KeyboardSession.shared.usesTraditional();
    this.chinesePunctuation = KeyboardSession.shared.usesChinesePunctuation();
    this.fullWidth = KeyboardSession.shared.isFullWidth();
    this.translations = KeyboardSession.shared.showsCandidateTranslations();
    this.fuzzyPinyin = KeyboardSession.shared.usesFuzzyPinyin();
    this.applySkins();
    this.wubiProfile = this.settings.wubiProfile;
    this.scheme = this.enabledScheme();
    this.nineKey = this.scheme.touchKeyboardLayout === 'nine_key';
    this.japanese = this.scheme.engineScheme === 'japanese' && this.nineKey;
    KeyboardSession.shared.setNineKey(this.nineKey);
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
    const chinese: string | undefined = CHINESE_SYMBOL_FACES.get(symbol);
    // 韩语、越南语和藏文的标点就是键帽上的 ASCII。
    return !this.english && !this.koreanFace() && !this.vietnameseFace() && !this.tibetanFace()
      && chinese !== undefined
      ? chinese : symbol;
  }

  /**
   * 符号层按键的行为。数字结束正在组的韩语音节，然后输入在它后面，标点路由做不到这一点，因为它只接受标点。越南语数字也是这样，除非它是组字中单词的 VNI 声调数字，那时它给单词加符号。藏文数字同样先上屏音节串再输入数字本身；藏文的拼写符号（`'` `+` `.` `-` `/`）走标点路由，由运行时按 spelling_symbols 交给引擎，其中 `+` 来自藏文方案下第三排替换 `=` 的键（KeyboardScheme.symbolRowKey）。注音的数字或标点如果是大千的注音键或声调键，由键盘自己输入，因为引擎会把它组进注音。
   */
  private tapSymbol(symbol: string): void {
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(symbol));
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
        return '全拼';
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
    return `${this.english}:${this.localMode}:${this.shuangpinProfile}:`
      + `${this.engineScheme}:${this.scheme.preferenceId}:${this.symbols}:${this.moreSymbols}:`
      + `${this.skin.key()}`;
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
    // A desktop chip also carries its ordinal and its suffix, and the window is sized from the same estimate (CandidateWidthPolicy.rowWidthVp); counting the word alone made each chip narrower than what it draws.
    const word: number = KeyboardFormFactor.isDesktop()
      ? CandidateWidthPolicy.chipContentVp(candidate, index, this.settings.candidateFontSize,
        CandidateNumberFontPolicy.size(this.settings.candidateFontSize))
      : Array.from(candidate.text).length * this.settings.candidateFontSize;
    if (!KeyboardSession.shared.glossesExpected()) {
      return CandidateChipWidth.chip(word, false, 0, padding);
    }
    const column: number = CandidateChipWidth.column(this.availableWidth, 0, padding);
    if (column <= 0) {
      return CandidateChipWidth.chip(word, false, 0, padding);
    }
    return CandidateChipWidth.chip(word, true, column, padding);
  }

  /** Vertical desktop rows keep glosses beside the word; every horizontal strip reserves below. */
  private candidateUsesGlossLine(): boolean {
    return !(KeyboardFormFactor.isDesktop() && this.settings.candidateLayout === 'vertical')
      && KeyboardSession.shared.glossRows() > 0;
  }

  /** The composition as 「候选栏预编辑」 lets it be seen; see CandidatePreeditStylePolicy. */
  private visibleComposition(): VisibleComposition {
    return CandidatePreeditStylePolicy.visible(this.settings.candidateShowPreedit, this.phrasePrefix, this.editing,
      this.localMode);
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
        action: () => this.tapLetter(letter)
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
      action: () => this.tapLetter(letter)
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

  /**
   * The candidate colours for the surface drawing them.
   *
   * The 2in1 candidate window wears the theme's candidate palette, resolved in the candidate window's own mode. The touch strip sits on the keyboard, so it wears the keyboard palette instead: the keyboard's text, its secondary colour for hints, and the accent for the selected candidate with no fill behind it.
   */
  private candidateColors(): CandidateColors {
    if (KeyboardFormFactor.isDesktop()) {
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
      selectedText: this.skin.accent,
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

  /** A 2in1 row's radius: never rounder than a card radius the user chose; a package's radius leaves the rows as they were drawn before the setting existed. */
  private candidateItemRadius(): number {
    return this.settings.candidateCornerRadiusUser
      ? Math.min(KeyboardMetrics.CANDIDATE_ITEM_CORNER_VP, this.candidateCardRadius())
      : KeyboardMetrics.CANDIDATE_ITEM_CORNER_VP;
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

  /** Nothing to spell, choose or report on a touch keyboard: the strip is the logo bar. */
  private stripIdle(): boolean {
    return !KeyboardFormFactor.isDesktop() && this.editing.length === 0
      && this.diagnostic.length === 0 && this.candidates.length === 0
      && this.englishSuggestions.length === 0 && !this.showsHandwritingCandidates();
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
      .backgroundColor(this.candidateCardColor(this.candidateColors().surface))
      .background(this.candidateBackground)
      .clip(this.settings.candidateBackgroundImage !== null)
      .borderRadius(KeyboardFormFactor.isDesktop()
        ? this.candidateCardRadius()
        : KeyboardMetrics.STRIP_CORNER_VP)
      .borderWidth(KeyboardFormFactor.isDesktop() ? 1 : 0)
      .borderColor(this.candidateCardColor(this.candidateColors().border))
      .onAreaChange((_old: Area, now: Area): void => {
        this.availableWidth = Number(now.width);
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
      // After the card, so it is drawn over the card's top edge: that overlap is the whole point of the decoration.
      if (this.settings.candidateDecorationImage !== null && this.candidateDecorationRect() !== null) {
        Image(this.settings.candidateDecorationImage)
          .position({ x: this.candidateDecorationRect()!.x, y: this.candidateDecorationRect()!.y })
          .width(this.candidateDecorationRect()!.width)
          .height(this.candidateDecorationRect()!.height)
          .objectFit(ImageFit.Contain)
          .hitTestBehavior(HitTestMode.Transparent)
      }
    }
    .width('100%')
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
      .height(CHIP_HEIGHT_VP)
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
      Column() {
        ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
          this.candidateCell(candidate, index)
        }, (candidate: CandidateEntry, index: number) =>
          `${index}:${candidate.text}:${candidate.highlighted}`)
      }
      .width('100%')
      // One row per candidate here and exactly one there. The two used to share a ternary applied to
      // the if/else as a whole, which ArkUI cannot do: modifiers belong to a component, and a branch
      // is not one. Each branch states its own height, and the condition already decided which.
      .height(this.candidateRowHeight() * KeyboardMetrics.visibleCandidateRows(this.candidates.length))
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
        Row() {
          ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
            this.candidateCell(candidate, index)
          }, (candidate: CandidateEntry, index: number) =>
            `${index}:${candidate.text}:${candidate.highlighted}`)
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
   * Translations, comments and code hints, selected row included: the palette's `secondary`, which already carries its own transparency, so nothing dims it again. The 2in1 window takes it from the candidate palette and the touch strip from the keyboard's, as the design's `candSelTr` and `kb.sub` do.
   */
  private candidateDetailColor(): string {
    return this.candidateColors().secondary;
  }

  @Builder
  candidatePrimaryLine(candidate: CandidateEntry) {
    Row() {
      Text(candidate.text).fontSize(this.settings.candidateFontSize)
        .fontWeight(this.candidateWeight(candidate))
        .fontColor(CandidateSkinPolicy.rowTextColor(candidate.highlighted,
          candidate.fixedPosition > 0, this.candidateTextColor(), this.candidateSelectedTextColor()))
      if (candidate.badge.length > 0) {
        Text(candidate.badge).fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateAccentColor()).opacity(0.8).padding({ left: 2 })
      }
      if (candidate.hint.length > 0) {
        Text(candidate.hint).fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailColor()).padding({ left: 2 })
      }
      // Only where there is no gloss line, the vertical 2in1 list; everywhere else the 훈음 is on the line under the Hanja.
      if (candidate.hunEum.length > 0 && !this.candidateUsesGlossLine()) {
        Text(candidate.hunEum)
          .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailColor()).padding({ left: 4 })
      }
      if (candidate.annotation.length > 0 && !this.candidateUsesGlossLine()) {
        if (candidate.annotationIsTranslation) {
          Text(candidate.annotation)
            .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
            .fontColor(this.candidateDetailColor())
            .padding({ left: CandidateTranslationStyle.gap(this.settings.candidateFontSize) })
        } else {
          Text(candidate.annotation)
            .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
            .fontColor(this.candidateDetailColor()).padding({ left: 4 })
        }
      } else if (candidate.annotation.length > 0 && !candidate.annotationIsTranslation) {
        Text(candidate.annotation)
          .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
          .fontColor(this.candidateDetailColor()).padding({ left: 4 })
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
        candidate.annotationIsTranslation ? annotation : null, true);
    return KeyAccessibilityPolicy.candidate(index + 1, candidate.text, candidate.hint, suffix);
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
        Blank().width(3).height(this.candidateLineHeight() * 0.55)
          .backgroundColor(this.candidateColors().accent).borderRadius(2)
      }
      if (CandidateNumberFontPolicy.visible(KeyboardFormFactor.isDesktop())) {
        Text(`${index + 1}`).fontSize(CandidateNumberFontPolicy.size(this.settings.candidateFontSize))
          .fontColor(this.candidateNumberColor(candidate.highlighted)).padding({ right: 4 })
      }
      if (this.candidateUsesGlossLine()) {
        Column() {
          this.candidatePrimaryLine(candidate)
          Text(this.candidateGlossLine(candidate).length > 0 ? this.candidateGlossLine(candidate) : ' ')
            .fontSize(CandidateTranslationStyle.fontSize(this.settings.candidateFontSize))
            .fontColor(this.candidateDetailColor())
            // The line is reserved whether or not this candidate has a gloss, so the row heights stay level.
            .opacity(this.candidateGlossLine(candidate).length > 0 ? 1 : 0)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .height(KeyboardMetrics.glossHeightVp(KeyboardSession.shared.glossRows(),
              this.settings.candidateFontSize))
        }
        .alignItems(HorizontalAlign.Start)
      } else {
        this.candidatePrimaryLine(candidate)
      }
    }.alignItems(VerticalAlign.Center)
      .accessibilityText(this.candidateLabel(candidate, index))
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP, right: KeyboardMetrics.CANDIDATE_PADDING_VP })
      .height(this.candidateRowHeight()).width(this.candidateChipWidth(candidate, index))
      .backgroundColor(index === this.hoveredCandidate ? this.candidateColors().hover
        : candidate.highlighted ? this.candidateColors().selected : TRANSPARENT)
      .borderRadius(KeyboardFormFactor.isDesktop() ? this.candidateItemRadius() : 0)
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
        event.stopPropagation();
        this.punctuation = false;
        this.touchCandidateMenu = false;
        this.managing = index;
      })
  }

  /**
   * 触屏键盘的工具条，始终为 `KeyboardMetrics.STRIP_HEIGHT_VP` 高，按键因此从不移动：正在调高度时是高度条，没有输入时是工具栏，其他时候是候选栏。
   */
  @Builder
  touchStrip() {
    Column() {
      if (this.surface === SURFACE_GEOMETRY) {
        HeightBar({
          percent: KeyboardGeometry.heightPercent(this.geometryHeight),
          foreground: this.skin.keyForeground,
          secondary: this.skin.secondary,
          accent: this.skin.actionBackground,
          onAccent: this.skin.actionForeground,
          onDrag: (deltaVp: number): void => this.dragHeight(deltaVp),
          onRelease: (): void => {
            this.heightDragging = false;
          },
          // 离开高度条会恢复已保存的高度；见 `show()`。
          onCancel: (): void => this.show(SURFACE_NONE),
          onReset: (): void => this.previewHeight(KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP),
          onDone: (): void => this.finishHeightMode()
        })
      } else if (this.stripIdle()) {
        this.toolbarRow()
      } else {
        this.candidateBar()
      }
    }
    .width('100%')
    .height(KeyboardMetrics.STRIP_HEIGHT_VP)
    .backgroundColor(this.candidateCardColor(this.candidateColors().surface))
    .background(this.candidateBackground)
    .clip(this.settings.candidateBackgroundImage !== null)
    .borderRadius(KeyboardMetrics.STRIP_CORNER_VP)
    .onAreaChange((_old: Area, now: Area): void => {
      this.availableWidth = Number(now.width);
    })
  }

  /**
   * 空闲时的工具条，即设计稿中的鸿蒙工具栏：七个等宽格，依次是标志（功能面板）、「表情」「常用语」「剪贴板」「皮肤」「输入方式」和「收起」。
   *
   * 面板已打开的工具显示为切换色底、强调色图标，再点一次收起面板。以前在这里以胶囊显示的语言、方案名、翻译和标点，现在和语音、回复面板、设置一样放在底行和功能面板里。
   */
  @Builder
  toolbarRow() {
    Row() {
      Column() {
        BrandTile({
          active: this.brandActive(),
          toggleBackground: this.skin.toggleBackground,
          logoCirc: this.skin.logoCirc,
          logoBg: this.skin.logoBg,
          framePath: KeyboardIconPaths.BRAND_FRAME,
          strokePath: KeyboardIconPaths.BRAND_STROKE,
          spokenLabel: KeyAccessibilityPolicy.tools(),
          onTap: (): void => this.tapBrand()
        })
      }
      .layoutWeight(1)
      .height('100%')
      .justifyContent(FlexAlign.Center)
      this.toolbarButton(KeyboardIconPaths.TOOLBAR_EMOJI, SURFACE_EMOJI, KeyAccessibilityPolicy.emoji(),
        (): void => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_EMOJI);
          this.openEmojiSurface();
        });
      this.toolbarButton(KeyboardIconPaths.TOOLBAR_PHRASE, SURFACE_PHRASES, KeyAccessibilityPolicy.phrase(),
        (): void => this.togglePhrases());
      this.toolbarButton(KeyboardIconPaths.TOOLBAR_CLIPBOARD, SURFACE_CLIPBOARD,
        KeyAccessibilityPolicy.clipboard(), (): void => this.toggleClipboard());
      this.toolbarButton(KeyboardIconPaths.TOOLBAR_SKIN, SURFACE_SKIN, KeyAccessibilityPolicy.skin(),
        (): void => this.toggle(SURFACE_SKIN));
      this.toolbarButton(KeyboardIconPaths.TOOLBAR_SCHEME, SURFACE_SCHEME, KeyAccessibilityPolicy.scheme(),
        (): void => this.toggle(SURFACE_SCHEME));
      Column() {
        Column() {
          KeyboardLineIcon({
            d: KeyboardIconPaths.COLLAPSE,
            iconSize: KeyboardIconPaths.COLLAPSE_SIZE_VP,
            stroke: KeyboardIconPaths.CHEVRON_STROKE,
            color: this.skin.keyForeground
          })
        }
        .width(TOOLBAR_BUTTON_VP)
        .height(TOOLBAR_BUTTON_VP)
        .justifyContent(FlexAlign.Center)
        .borderRadius(TOOLBAR_BUTTON_RADIUS_VP)
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

  /** 工具栏的一个面板工具：40vp 按钮里的线框图标，平时为次要文字色（设计稿的 kbSub，即 Android 的 `KeyboardSkin.toolbarIcon()`；收起箭头仍用按键文字色），`surface` 为当前显示的界面时着色。标签不可省略，因为按钮只绘制图标。 */
  @Builder
  toolbarButton(path: string, surface: string, label: string, action: () => void) {
    Column() {
      Column() {
        KeyboardLineIcon({
          d: path,
          iconSize: KeyboardIconPaths.TOOLBAR_SIZE_VP,
          stroke: KeyboardIconPaths.TOOLBAR_STROKE,
          color: this.surface === surface ? this.skin.toggleForeground : this.skin.secondary
        })
      }
      .width(TOOLBAR_BUTTON_VP)
      .height(TOOLBAR_BUTTON_VP)
      .justifyContent(FlexAlign.Center)
      .borderRadius(TOOLBAR_BUTTON_RADIUS_VP)
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
      Column({ space: CANDIDATE_BAR_GAP_VP }) {
        Row() {
          if (this.diagnostic.length > 0) {
            Text(this.diagnostic)
              .fontSize(PREEDIT_FONT_SIZE)
              .fontColor(this.skin.accent)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
          } else if (this.visibleComposition().caret) {
            // 分段键移动的光标，与 2in1 组合行上的一样。
            Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).before)
              .fontSize(PREEDIT_FONT_SIZE)
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
            Divider()
              .vertical(true)
              .strokeWidth(PREEDIT_CARET_WIDTH_VP)
              .height(PreeditCaretPolicy.height(PREEDIT_FONT_SIZE))
              .color(this.skin.accent)
            Text(PreeditCaretPolicy.split(this.editing, this.editingCaret).after)
              .fontSize(PREEDIT_FONT_SIZE)
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
          } else {
            // 「候选栏预编辑：不显示」：这一行保留位置但不显示拼写，不过短语中已选定的部分和本地模式的触发符仍会显示。
            Text(this.visibleComposition().text)
              .fontSize(PREEDIT_FONT_SIZE)
              .letterSpacing(PREEDIT_LETTER_SPACING)
              .fontColor(this.skin.secondary)
              .fontFamily(this.candidateFontFamily())
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
          }
        }
        .width('100%')
        .height(PREEDIT_LINE_VP)
        .padding({ left: PREEDIT_INDENT_VP })
        .clip(true)

        if (this.showsHandwritingCandidates()) {
          // OCR 结果也是候选，通过识别器而不是 Engine 选择。
          this.chipLane(0)
        } else if (this.candidates.length > 0) {
          Scroll() {
            Row() {
              ForEach(this.candidates, (candidate: CandidateEntry, index: number) => {
                this.candidateChip(candidate, index)
              }, (candidate: CandidateEntry, index: number) =>
                `${index}:${candidate.text}:${candidate.highlighted}`)
            }
            .height(CHIP_HEIGHT_VP)
          }
          .scrollable(ScrollDirection.Horizontal)
          .scrollBar(BarState.Off)
          .width('100%')
          .height(CHIP_HEIGHT_VP)
          .align(Alignment.Start)
        } else if (this.englishSuggestions.length > 0) {
          // 直接英文输入没有组合，所以通道上只有补全。
          this.chipLane(1)
        } else {
          Column()
            .width('100%')
            .height(CHIP_HEIGHT_VP)
        }
      }
      .layoutWeight(1)
      .height('100%')
      .justifyContent(FlexAlign.Center)

      if (this.showsHanjaButton()) {
        this.hanjaButton()
      }
      if (this.candidates.length > 0 && !this.showsHandwritingCandidates()) {
        Divider()
          .vertical(true)
          .strokeWidth(1)
          .height(BAR_DIVIDER_HEIGHT_VP)
          .color(this.skin.hair)
        this.expandChevron()
      }
    }
    .width('100%')
    .height('100%')
    .alignItems(VerticalAlign.Center)
  }

  /** 胶囊绘制的词：字号不超过 34vp 胶囊能容纳的大小，分下方有无注释行两种情况。 */
  private chipFontSize(): number {
    return Math.min(this.settings.candidateFontSize,
      KeyboardSession.shared.glossesExpected() ? CHIP_WORD_MAX_WITH_GLOSS : CHIP_WORD_MAX_ALONE);
  }

  /** 通道的可见宽度：工具条宽度减去细线和展开箭头。 */
  private chipLaneWidth(): number {
    return Math.max(0, this.availableWidth - 1 - BAR_BUTTON_VP);
  }

  /** 一个注释列的文字宽度（`CandidateChipWidth`：可见通道放三列），不预期出现注释时为 0。 */
  private chipGlossColumn(): number {
    return KeyboardSession.shared.glossesExpected()
      ? CandidateChipWidth.column(this.chipLaneWidth(), 0, CHIP_PADDING_VP) : 0;
  }

  /** 胶囊的最小宽度：设计稿的 30vp；预期有注释时为整个注释列宽，这样注释到来时不会撑宽胶囊、把后面的胶囊挤开。 */
  private chipMinWidth(): number {
    const column: number = this.chipGlossColumn();
    return column > 0 ? Math.max(CHIP_MIN_WIDTH_VP, Math.ceil(column + CHIP_PADDING_VP * 2))
      : CHIP_MIN_WIDTH_VP;
  }

  /** 注释最多能画多宽：它的列宽，或上方的词更宽时取词宽。 */
  private chipGlossMaxWidth(candidate: CandidateEntry): number {
    return CandidateChipWidth.content(Array.from(candidate.text).length * this.chipFontSize(), true,
      this.chipGlossColumn());
  }

  /** 胶囊在词旁绘制的内容：Engine 的注解，或在没有注释行承载时的翻译。 */
  private chipAnnotationBeside(candidate: CandidateEntry): boolean {
    return candidate.annotation.length > 0
      && (!candidate.annotationIsTranslation || !KeyboardSession.shared.glossesExpected());
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
          .fontColor(CandidateSkinPolicy.rowTextColor(candidate.highlighted, candidate.fixedPosition > 0,
            this.candidateTextColor(), this.candidateSelectedTextColor()))
          .fontFamily(this.candidateFontFamily())
          .maxLines(1)
        if (candidate.badge.length > 0) {
          Text(candidate.badge)
            .fontSize(CHIP_GLOSS_FONT_SIZE)
            .fontColor(this.skin.accent)
            .opacity(0.8)
            .padding({ left: 2 })
            .maxLines(1)
        }
        if (candidate.hint.length > 0) {
          Text(candidate.hint)
            .fontSize(CHIP_GLOSS_FONT_SIZE)
            .fontColor(this.skin.secondary)
            .padding({ left: 2 })
            .maxLines(1)
        }
        if (this.chipAnnotationBeside(candidate)) {
          Text(candidate.annotation)
            .fontSize(CHIP_GLOSS_FONT_SIZE)
            .fontColor(this.skin.secondary)
            .padding({ left: 3 })
            .maxLines(1)
        }
      }
      .alignItems(VerticalAlign.Center)
      if (KeyboardSession.shared.glossesExpected()) {
        // 无论该候选有无注释都预留位置，这样通道上的词保持齐平。
        Text(this.candidateGlossLine(candidate).length > 0 ? this.candidateGlossLine(candidate) : ' ')
          .fontSize(CHIP_GLOSS_FONT_SIZE)
          .lineHeight(CHIP_GLOSS_LINE_VP)
          .fontColor(this.skin.secondary)
          .fontFamily(this.candidateFontFamily())
          .opacity(this.candidateGlossLine(candidate).length > 0 ? 1 : 0)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .constraintSize({ maxWidth: this.chipGlossMaxWidth(candidate) })
      }
    }
    .constraintSize({ minWidth: this.chipMinWidth() })
    .height(CHIP_HEIGHT_VP)
    .padding({ left: CHIP_PADDING_VP, right: CHIP_PADDING_VP })
    .justifyContent(FlexAlign.Center)
    .borderRadius(CHIP_RADIUS_VP)
    .backgroundColor(candidate.highlighted ? this.skin.keyBackground : TRANSPARENT)
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
   * 用于非 Engine 候选的普通胶囊通道：手写结果（`kind` 0），通过识别器选择；英文补全（`kind` 1），是对正在输入的单词的只读建议。列表在这里读取而不是传入，这样通道随列表变化而更新。
   */
  @Builder
  chipLane(kind: number) {
    Scroll() {
      Row() {
        ForEach(kind === 0 ? this.handwritingCandidates : this.englishSuggestions, (item: string, index: number) => {
          Text(item)
            .fontSize(this.chipFontSize())
            .fontFamily(this.candidateFontFamily())
            .fontColor(this.skin.keyForeground)
            .maxLines(1)
            .textAlign(TextAlign.Center)
            .constraintSize({ minWidth: CHIP_MIN_WIDTH_VP })
            .height(CHIP_HEIGHT_VP)
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
              : KeyAccessibilityPolicy.englishSuggestion(index + 1, item))
            .onClick(() => {
              if (kind === 0) {
                this.chooseHandwriting(item);
              } else {
                KeyboardSession.shared.useEnglishSuggestion(index);
              }
            })
        }, (item: string, index: number) => `${kind}:${index}:${item}`)
      }
      .height(CHIP_HEIGHT_VP)
    }
    .scrollable(ScrollDirection.Horizontal)
    .scrollBar(BarState.Off)
    .width('100%')
    .height(CHIP_HEIGHT_VP)
    .align(Alignment.Start)
  }

  /** 候选栏的展开箭头，网格打开期间翻转显示。 */
  @Builder
  expandChevron() {
    Column() {
      Column() {
        KeyboardLineIcon({
          d: KeyboardIconPaths.EXPAND,
          iconSize: KeyboardIconPaths.EXPAND_SIZE_VP,
          stroke: KeyboardIconPaths.CHEVRON_STROKE,
          color: this.skin.keyForeground
        })
      }
      .rotate({ angle: this.surface === SURFACE_EXPANDED ? 180 : 0 })
      .animation({ duration: CHEVRON_TURN_MS })
    }
    .width(BAR_BUTTON_VP)
    .height(BAR_BUTTON_VP)
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
    Column({ space: this.rowSpacing() }) {
      if (this.touchDesign()) {
        this.touchNineKeyGrid()
      } else {
        this.nineKeyGrid()
      }
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /**
   * 触屏键盘的九键网格，与设计稿和 Android（`ImeLayoutRows.rebuildNineKeyRows`）一致：一列 ，。？ 键，首格是 @# 的 3×3 网格，以及一列删除键、音节分隔符和 ！。数字层重新标注网格并保留两侧的列。
   *
   * 输入数字串时，左列改为列出这些数字可能代表的读音而不是标点，鸿蒙九键一直如此；设计稿里没有放它们的位置。
   */
  @Builder
  touchNineKeyGrid() {
    Row({ space: this.keySpacing() }) {
      Column({ space: this.rowSpacing() }) {
        if (!this.symbols && this.spellings.length > 0) {
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
        } else {
          ForEach(NineKeyLayout.sideMarks(), (symbol: string) => {
            Column() {
              this.fixedKey(this.nineKeyMarkOptions(symbol));
            }
            .layoutWeight(1)
            .width('100%')
          }, (symbol: string) => symbol + this.faceKey())
        }
      }
      .layoutWeight(NINE_KEY_SIDE_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        ForEach(this.symbols ? NineKeyLayout.digits(this.numberKeypadOrder) : NineKeyLayout.touchRows(), (row: NineKey[]) => {
          Row({ space: this.keySpacing() }) {
            ForEach(row, (key: NineKey) => {
              if (!this.symbols && NineKeyLayout.holdOptions(key).length > 0) {
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
      .layoutWeight(NINE_KEY_GRID_SHARE)
      .height('100%')

      Column({ space: this.rowSpacing() }) {
        Column() {
          this.deleteKey('100%', true, 0)
        }
        .layoutWeight(1)
        .width('100%')
        Column() {
          // 「重输」是删除键的长按（`BackspaceHoldPolicy`），所以这一格是分隔符，即 Android 的「拆分」。
          this.fixedKey({
            face: this.nineKeyFaceOf(NineKeyLayout.separator()),
            label: this.nineKeyLabel(NineKeyLayout.separator()),
            size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
            fontWeight: FUNCTION_LABEL_WEIGHT,
            emphasized: false,
            special: true,
            width: '100%',
            action: () => {
              KeyboardSession.shared.recordKey(KeyIdPolicy.character(NineKeyLayout.separator().input));
              KeyboardSession.shared.press(NineKeyLayout.separator().input.charCodeAt(0), false);
            }
          });
        }
        .layoutWeight(1)
        .width('100%')
        Column() {
          this.fixedKey(this.nineKeyMarkOptions(NineKeyLayout.closingMark()));
        }
        .layoutWeight(1)
        .width('100%')
      }
      .layoutWeight(NINE_KEY_SIDE_SHARE)
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
        KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_PUNCTUATION);
        KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
      }
    };
  }

  /** 到这里结束的键面是否为触屏键盘的九键网格，它有自己的底行（`nineKeyActionRow`）。假名网格也是九键布局，但保留共享底行。 */
  private touchNineKeyRow(): boolean {
    return this.touchDesign() && !this.japanese && KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode);
  }

  /** 0 在字母层上参与拼写，由 Engine 读取；在数字层上与其他九个数字一样直接输入。 */
  private typeNineKeyZero(): void {
    KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey('0'));
    if (this.symbols) {
      KeyboardSession.shared.punctuation(0x30);
    } else {
      KeyboardSession.shared.press(0x30, false);
    }
  }

  /**
   * 触屏九键键面的底行，按设计稿：123 1.25 | space 4.2 | 0 1.05 | 中 1.05 | return 1.6，中/英键紧挨回车，与字母键面的底行一致。标点在网格两侧的列里，所以这里没有逗号和句号，与 Android 一致；旧控制列上的 0 移到这里，即设计稿放它的位置。123 把网格重新标注为数字，在数字层上该键显示「九键」，即 Android 中这个网格的 `KeyboardActionRow.layerTitle`。
   */
  @Builder
  nineKeyActionRow() {
    Row({ space: this.keySpacing() }) {
      // 长按 123 直接打开符号面板，不必再经过 #+= 层；符号层里这个键是返回，长按不做事。
      Column() {
        this.fixedKey({
          face: this.symbols ? '九键' : '123',
          label: KeyAccessibilityPolicy.layoutToggle(this.symbols),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: '100%',
          action: () => this.toggleSymbolLayer()
        });
      }
      .layoutWeight(LAYER_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => this.openSymbolsFromLayerKey()))
      this.spaceKey(true, NINE_KEY_SPACE_SHARE);
      this.fixedKey({
        face: '0',
        label: '数字 0',
        size: TOUCH_LETTER_FONT_SIZE,
        emphasized: false,
        width: 0,
        share: NINE_KEY_ZERO_SHARE,
        action: () => this.typeNineKeyZero()
      });
      this.fixedKey({
        face: this.languageFace(),
        label: KeyAccessibilityPolicy.language(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: LANGUAGE_SHARE,
        hitRegion: this.languageKeyHitRegion(),
        action: () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LANGUAGE);
          this.toggleTouchLanguage();
        }
      });
      this.designReturnKey(NINE_KEY_RETURN_SHARE, true);
    }
    .width('100%')
    .height(this.rowHeight(3))
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
              KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_PUNCTUATION);
              KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
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
              KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_PUNCTUATION);
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
        ForEach(this.symbols ? NineKeyLayout.digits(this.numberKeypadOrder) : NineKeyLayout.rows(), (row: NineKey[]) => {
          Row({ space: this.keySpacing() }) {
            ForEach(row, (key: NineKey) => {
              if (!this.symbols && NineKeyLayout.holdOptions(key).length > 0) {
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
        // A period is punctuation while spelling and a decimal point beside the digit grid. Clear
        // is already available by holding Delete, so it no longer spends a permanent key here.
        this.nineKeyControl('.', '句点', () => {
          // The period key types '.', so it counts as Period as on iOS and Android; SoftPunctuation is for the side rail's cells.
          KeyboardSession.shared.recordKey(KeyIdPolicy.character('.'));
          KeyboardSession.shared.punctuation(0x2e);
        });
        // 0 is inserted on the digit layer for the same reason the other nine are, and spelled on
        // the letter layer, where the Engine reads it as a candidate position.
        this.nineKeyControl('0', '数字 0', this.symbols
          ? () => {
            KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey('0'));
            KeyboardSession.shared.punctuation(0x30);
          }
          : () => {
            KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey('0'));
            KeyboardSession.shared.press(0x30, false);
          });
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }

  private nineKeyKeyOptions(key: NineKey): KeyOptions {
    return {
      face: this.symbols ? key.label : this.nineKeyFaceOf(key),
      label: this.symbols ? KeyAccessibilityPolicy.symbol(key.label) : this.nineKeyLabel(key),
      // 触屏网格：字母格 17fp、字重 500，数字用旁边 0 键的 22fp。
      size: !this.touchDesign() ? KeyboardMetrics.SMALL_KEY_FONT_SIZE
        : (this.symbols ? TOUCH_LETTER_FONT_SIZE : NINE_KEY_FONT_SIZE),
      fontWeight: this.touchDesign() && !this.symbols ? NINE_KEY_LETTER_WEIGHT : undefined,
      emphasized: false,
      width: 0,
      // A digit is inserted, not spelled: the same call the twenty-six key symbol row makes for
      // its own digit row, rather than handing it to the Engine as a candidate position.
      action: this.symbols
        ? () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
          KeyboardSession.shared.punctuation(key.input.charCodeAt(0));
        }
        : NineKeyLayout.opensSymbols(key)
        ? () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
          this.openSymbolSurface();
        }
        : () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
          KeyboardSession.shared.press(key.input.charCodeAt(0), false);
        }
    };
  }

  @Builder
  nineKeyHoldKey(key: NineKey) {
    Stack({ alignContent: Alignment.TopEnd }) {
      this.keyFace(this.nineKeyKeyOptions(key))
      if (this.touchDesign()) {
        // 该格拼写的数字，印在右上角，与设计稿和 Android 的 `NineKeyDigitButton` 的印法一致。
        Text(NineKeyLayout.digitHint(key))
          .fontSize(KEY_HINT_FONT_SIZE)
          .fontColor(this.skin.secondary)
          .maxLines(1)
          .margin({ top: NINE_KEY_HINT_TOP_VP, right: NINE_KEY_HINT_RIGHT_VP })
          .hitTestBehavior(HitTestMode.Transparent)
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
    .onClick(() => {
      KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
      KeyboardSession.shared.press(key.input.charCodeAt(0), false);
    })
    .gesture(LongPressGesture().onAction(() => {
      // A hold is still one press of the cell, whichever of its options it ends in.
      KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
      this.openNineKeyHold(key);
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
   * 触屏键盘的字母键面改用设计稿的底行（`designActionRow`），九键网格用自己的底行（`nineKeyActionRow`）；触屏 123 和 #+= 层自己绘制底行（`designLayerFace`）。笔画键盘、假名网格、手写和所有 2in1 屏幕键盘仍用这一行。
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

  /** 到这里结束的键面是否使用设计稿的底行：触屏键盘的字母行（26 键、大千行以及大千键面保留的共享符号行），而不是带自己标点列的网格、假名网格或手写板。 */
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(','));
    // 大千的逗号键是 ㄝ，所以注音直接输入标点本身。
    if (this.zhuyinFace()) {
      this.typeZhuyinMark(',');
    } else {
      KeyboardSession.shared.punctuation(0x2c);
    }
  }

  private typePeriod(): void {
    KeyboardSession.shared.recordKey(KeyIdPolicy.character('.'));
    // 大千的句号键是 ㄡ，所以注音与逗号一样直接输入标点本身。
    if (this.zhuyinFace()) {
      this.typeZhuyinMark('.');
    } else {
      KeyboardSession.shared.punctuation(0x2e);
    }
  }

  /**
   * 设计稿的底行，按宽度份额分配：123 1.25 | ， 1 | space 4 | 。 1 | 中 1.05 | return 1.9，即 Android 的 `KeyboardActionRow.designEntries`。中/英键放在回车左边，与经典底行的位置一致。
   *
   * 功能键文字 15fp、字重 500。逗号保留长按弹出「常用标点」列表；句号是新加的。回车键始终使用操作色。
   */
  @Builder
  designActionRow() {
    Row({ space: this.keySpacing() }) {
      // 长按 123 直接打开符号面板，不必再经过 #+= 层；符号层里这个键是返回，长按不做事。
      Column() {
        this.fixedKey({
          face: this.symbols ? '返回' : '123',
          label: KeyAccessibilityPolicy.layoutToggle(this.symbols),
          size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
          fontWeight: FUNCTION_LABEL_WEIGHT,
          emphasized: false,
          special: true,
          width: '100%',
          action: () => this.toggleSymbolLayer()
        });
      }
      .layoutWeight(LAYER_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => this.openSymbolsFromLayerKey()))
      Column() {
        this.fixedKey({
          face: this.chinesePunctuationFaces() ? '，' : ',',
          label: KeyAccessibilityPolicy.punctuation(),
          size: TOUCH_LETTER_FONT_SIZE,
          emphasized: false,
          width: '100%',
          action: () => this.typeComma()
        });
      }
      .layoutWeight(PUNCTUATION_SHARE)
      .height('100%')
      .gesture(LongPressGesture().onAction(() => {
        this.managing = -1;
        this.brackets = false;
        this.punctuation = true;
      }))
      this.spaceKey(true, SPACE_SHARE);
      this.fixedKey({
        face: this.chinesePunctuationFaces() ? '。' : '.',
        label: KeyAccessibilityPolicy.period(),
        size: TOUCH_LETTER_FONT_SIZE,
        emphasized: false,
        width: 0,
        share: PUNCTUATION_SHARE,
        action: () => this.typePeriod()
      });
      this.fixedKey({
        face: this.languageFace(),
        label: KeyAccessibilityPolicy.language(),
        size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
        fontWeight: FUNCTION_LABEL_WEIGHT,
        emphasized: false,
        special: true,
        width: 0,
        share: LANGUAGE_SHARE,
        hitRegion: this.languageKeyHitRegion(),
        action: () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LANGUAGE);
          this.toggleTouchLanguage();
        }
      });
      this.designReturnKey(RETURN_SHARE, true);
    }
    .width('100%')
    .height(this.rowHeight(3))
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
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_SYMBOL);
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
      this.spaceKey(false, 1);
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
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LANGUAGE);
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
    return this.japanese ? this.editing.length > 0 : this.candidates.length > 0;
  }

  private returnFace(): string {
    if (this.japanese) {
      return JapaneseNineKeyActions.returnTitle(this.editing.length > 0);
    }
    return this.candidates.length > 0
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
   */
  @Builder
  spaceKey(design: boolean, share: number) {
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
          Text(this.spaceLabel())
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
        Text(this.japanese
          ? JapaneseNineKeyActions.spaceTitle(this.editing.length > 0)
          : (this.english ? 'space' : '空格'))
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.secondary)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
    }
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
      } else if (event.type === TouchType.Move) {
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
        this.cursorDrag.cancel();
        if (!this.dragged && event.type === TouchType.Up) {
          this.activateSpace();
        }
      }
    })
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

  /** On a 2in1 the candidate management strip is the candidate context menu Windows draws in `menu_theme`; the other strips, and every strip on a phone, belong to the keyboard. */
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
    this.clipboardNotice = this.clipboardEnabled ? '' : '剪贴板历史已在设置中关闭';
    this.confirmingClear = false;
    this.openCloudClipboard();
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
    this.cloudEditor = KeyboardSession.shared.cloudClipboardEditor();
    this.cloudLoading = true;
    KeyboardSession.shared.cloudClipboardItems().then((listing: CloudClipboardListing | null): void => {
      if (request !== this.cloudRequest) {
        return;
      }
      this.cloudLoading = false;
      if (listing === null) {
        // The field changed while the request was out; what came back was fetched for another one.
        this.cloudClips = [];
        this.cloudStateKnown = false;
        return;
      }
      this.cloudClips = listing.items;
      this.cloudState = listing.state;
      this.cloudStateKnown = true;
    });
  }

  /** Forgets the listing on screen and any answer still on its way. */
  private dropCloudClipboard(): void {
    this.cloudRequest++;
    this.invalidateCloudSend();
    this.cloudClips = [];
    this.cloudStateKnown = false;
    this.cloudLoading = false;
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
    if (KeyboardSession.shared.insertCloudClipboard(text, this.cloudEditor)) {
      return;
    }
    // The listing was fetched for a field that no longer has focus; fetch again for this one rather than paste into it.
    this.cloudVisible = KeyboardSession.shared.cloudClipboardVisible();
    this.dropCloudClipboard();
    if (this.cloudVisible) {
      this.refreshCloudClipboard();
    } else {
      this.clipboardTab = CLIPBOARD_TAB_LOCAL;
    }
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
    if (this.cloudSending || text.length === 0 || this.cloudSendBlock() !== null) {
      return;
    }
    const request: number = ++this.cloudSendRequest;
    this.cloudSending = true;
    KeyboardSession.shared.sendToCloudClipboard(text).then((outcome: CloudClipboardSendOutcome): void => {
      if (!CloudClipboardPolicy.sendCurrent(request, this.cloudSendRequest)) {
        return;
      }
      this.cloudSending = false;
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
      this.clipboardNotice = '剪贴板历史已在设置中关闭';
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
    const characters: string[] = Array.from(this.replySource);
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

  private storeClipboardText(): void {
    if (!this.clipboardEnabled) {
      this.clipboardNotice = '剪贴板历史已在设置中关闭';
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
    if (index < 0 || index >= this.candidates.length) {
      return;
    }
    this.touchCandidateMenu = false;
    this.managing = -1;
    const candidate: CandidateEntry = this.candidates[index];
    // A 2in1 touch screen keeps the gloss alone, because its right click already opens management. A phone has no right click, so its long press is the only way to 优先显示, 固定排位 and 删除词条, and opens the whole menu the way Android's does.
    const glossOnly: boolean = KeyboardFormFactor.isDesktop();
    if (glossOnly && CandidateManagementAction.touchActions(
      candidate.annotation, candidate.annotationIsTranslation).length === 0) {
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
    this.numberKeypadOrder = this.settings.numberKeypadOrder;
    KeyboardSession.shared.resizeTouchKeyboard(KeyboardMetrics.totalHeightVp(
      this.geometryRowSpacing, this.geometryHeight, KeyboardSession.shared.glossRows(),
      this.settings.candidateFontSize, this.settings.candidatePreeditFontSize));
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
      incognito: current.incognito
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
      incognito: current.incognito
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
    return OneHandedPolicy.active(this.feedback.oneHanded, KeyboardFormFactor.isDesktop());
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
      this.clipboardNotice = '剪贴板历史已在设置中关闭';
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

  private redrawHandwriting(): void {
    const context: CanvasRenderingContext2D = this.handwritingContext;
    const pad: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    context.clearRect(0, 0, HANDWRITING_CANVAS_SIZE, HANDWRITING_CANVAS_SIZE);
    // 「笔迹颜色」：「跟随皮肤」取手写板皮肤的文字颜色，这样笔迹在任何底色上都看得清；其余是 `KeyboardFeedback` 的固定墨色。
    context.strokeStyle = KeyboardFeedback.strokeInk(pad.handwritingStrokeColor, this.handwritingSkin.keyForeground);
    // 「笔迹粗细」以 vp 为单位，这块画布也是：笔画按触摸的 vp 坐标记录，限制在 `HANDWRITING_CANVAS_SIZE` 的正方形内，绘制上下文用的也是同一单位，所以线宽无需缩放就与设置显示的一致。
    context.lineWidth = KeyboardFeedback.handwritingStrokeWidth(pad.handwritingStrokeWidth);
    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (const stroke of this.handwritingStrokes) {
      if (stroke.points.length === 0) {
        continue;
      }
      context.beginPath();
      context.moveTo(stroke.points[0].x, stroke.points[0].y);
      for (const point of stroke.points.slice(1)) {
        context.lineTo(point.x, point.y);
      }
      context.stroke();
    }
    if (this.handwritingCurrent.length > 0) {
      context.beginPath();
      context.moveTo(this.handwritingCurrent[0].x, this.handwritingCurrent[0].y);
      for (const point of this.handwritingCurrent.slice(1)) {
        context.lineTo(point.x, point.y);
      }
      context.stroke();
    }
  }

  private handwritingPoint(event: TouchEvent): HandwritingPoint {
    const touch = event.touches[0];
    return HandwritingStrokePolicy.point(touch.x, touch.y);
  }

  private handwritingTouch(event: TouchEvent): void {
    if (event.touches.length === 0) {
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
        this.redrawHandwriting();
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
    if (event.type === TouchType.Cancel) {
      // A cancelled gesture must not become a real stroke. Harmony can cancel a Canvas touch
      // when the system takes the gesture (for example, a panel edge swipe); keeping the partial
      // stroke made the next pen lift recognise ink the user never finished.
      this.handwritingCurrent = [];
      this.redrawHandwriting();
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

  /** 等待用户设定的停顿时长（「识别等待时间」，`KeyboardFeedback` 的 200–1500 ms，默认 600）后再对整个字截图。 */
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
    componentSnapshot.get(HANDWRITING_CANVAS_ID, (error: Object, pixelMap: image.PixelMap): void => {
      // Testing the error object itself took this branch on every successful snapshot, so every
      // stroke answered 无法读取手写画布 and nothing was ever recognised.
      if (BusinessErrorPolicy.failed(error)) {
        if (this.handwritingRecognition.accepts(ticket)) {
          this.handwritingNotice = '无法读取手写画布';
        }
        this.finishHandwritingRecognition();
        return;
      }
      textRecognition.init().then((ready: boolean): Promise<textRecognition.TextRecognitionResult> => {
        if (!ready) {
          throw new Error('text recognition unavailable');
        }
        return textRecognition.recognizeText({ pixelMap: pixelMap });
      }).then((result: textRecognition.TextRecognitionResult): void => {
        if (!this.handwritingRecognition.accepts(ticket)) {
          return;
        }
        this.handwritingCandidates = HandwritingStrokePolicy.candidates(result.value);
        this.handwritingNotice = this.handwritingCandidates.length > 0
          ? '' : '没有识别到文字，请重写';
      }).catch((reason: Error): void => {
        if (this.handwritingRecognition.accepts(ticket)) {
          KeyboardLog.error(`handwriting recognition unavailable: ${describeError(reason)}`);
          this.handwritingCandidates = [];
          this.handwritingNotice = '手写识别服务不可用';
        }
      }).finally((): void => {
        void pixelMap.release();
        // The recognizer is process-global. Do not start the queued snapshot until its previous
        // invocation has actually released, or init() can race the outstanding release promise.
        void textRecognition.release().then((): void => {
          this.finishHandwritingRecognition();
        }, (_reason: Object): void => {
          this.finishHandwritingRecognition();
        });
      });
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
    if (!KeyboardSession.shared.voiceInputEnabled()) {
      this.voiceEnabled = false;
      this.voiceNotice = '语音输入已在设置中关闭';
      return;
    }
    this.voiceGeneration = KeyboardSession.shared.voiceStart();
    if (this.voiceGeneration === 0) {
      this.voiceNotice = '语音会话不可用';
      return;
    }
    this.clearVoiceDone();
    this.cancelVoiceSilence();
    this.voiceHeldByKey = fromHotkey;
    const voiceConfiguration = KeyboardSession.shared.voiceInputConfiguration();
    this.voiceStreams = VoiceRecordingBehaviourPolicy.streamsPartialResults(voiceConfiguration);
    this.voiceText = '';
    this.voiceHeard = '';
    this.voiceNotice = '正在聆听…';
    this.voiceBusy = true;
    this.voiceFinishing = false;
    // 结果要去的位置，这样只有结果到达时编辑框仍在那里才免审阅直接上屏（Android 的 `captureVoiceTarget`）。
    KeyboardSession.shared.captureVoiceTarget();
    KeyboardSession.shared.setVoiceRecording(true);
    this.voiceRecognizer.start(voiceConfiguration.language || 'zh-CN', voiceConfiguration,
      (text: string, final: boolean): void => {
      // A partial result only reaches the panel when the user asked to watch the recognizer work.
      // The final one always does: it is the result.
      if (text.length > 0
          && (final || VoiceRecordingBehaviourPolicy.showsInterimResults(voiceConfiguration))) {
        this.voiceText = VoiceRecognitionPolicy.result(text);
        if (!final && this.touchDesign()) {
          this.voiceHeard = this.voiceText;
        }
      }
      if (!final && text.length > 0) {
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
        KeyboardSession.shared.setVoiceRecording(false);
        this.deliverVoice(accepted);
      }
      }, (message: string): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      this.voiceHeard = '';
      this.cancelVoiceSilence();
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = message;
      }, KeyboardSession.shared.voiceHotwordOptions()).catch((error: Object): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      this.voiceHeard = '';
      this.cancelVoiceSilence();
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = error instanceof Error ? error.message : '语音识别不可用';
    });
  }

  /**
   * 会话接受后的最终结果。在触屏上，若这次是流式录音（聆听界面仍在）、编辑框也还是开始聆听时那个，就直接进入编辑框，与 Android 键盘内 LOCAL / STREAMING 识别器的做法一致；随后界面短暂显示「已识别：…」并把按键交还。上传式服务商的结果、编辑框已变化的结果以及 2in1 上的结果都留在审阅界面等待「提交」。
   */
  private deliverVoice(accepted: string | null): void {
    if (accepted === null || accepted.length === 0) {
      this.voiceNotice = accepted !== null ? '识别完成，可提交结果。' :
        (this.voiceText.length > 0 ? '已忽略过期的语音结果。' : '没有识别到语音。');
      return;
    }
    if (!VoiceRecognitionPolicy.skipsReview(this.touchDesign(), this.voiceStreams,
      this.surface === SURFACE_VOICE)) {
      this.voiceNotice = '识别完成，可提交结果。';
      return;
    }
    if (!KeyboardSession.shared.voiceTargetMatches()) {
      this.voiceNotice = '输入位置已变化，结果已保留，可点提交插入。';
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
   * 听到文字后，停顿 `VOICE_SILENCE_STOP_MS` 即结束录音并给出结果，即 Android 的静音停止：触屏聆听界面没有停止按钮，因为点击任意位置即取消。由快捷键按住的录音在松开按键时结束。
   */
  private armVoiceSilence(): void {
    if (!this.touchDesign() || this.voiceHeldByKey || !this.voiceBusy || this.voiceFinishing) {
      return;
    }
    this.cancelVoiceSilence();
    this.voiceSilenceTimer = setTimeout((): void => {
      this.voiceSilenceTimer = -1;
      this.stopVoice();
    }, VOICE_SILENCE_STOP_MS);
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

  /** 触屏上，流式识别器运行期间以及结果刚直接上屏后的片刻，用聆听界面代替审阅界面；审阅界面用于上传式服务商的录音（需要它的「停止录音」）、需要等待的结果、失败，以及 2in1。 */
  private voiceListening(): boolean {
    return this.touchDesign() && ((this.voiceBusy && this.voiceStreams) || this.voiceDone.length > 0);
  }

  /** 聆听界面的标题：结果上屏后为「识别完成」，即 Android 的 `VoiceListeningView` 文案。 */
  private voiceListeningTitle(): string {
    return this.voiceDone.length > 0 ? '识别完成' : '正在聆听…';
  }

  /** 标题下方的一行：已上屏的内容、录音正在收尾、目前听到的文字，或如何取消。 */
  private voiceListeningHint(): string {
    if (this.voiceDone.length > 0) {
      return `已识别：${this.voiceDone}`;
    }
    if (this.voiceFinishing) {
      return '正在识别…';
    }
    return this.voiceHeard.length > 0 ? this.voiceHeard : '点任意处取消';
  }

  /** 点按聆听界面：停止聆听并回到按键。 */
  private cancelListening(): void {
    this.cancelVoice();
    this.show(SURFACE_NONE);
  }

  @Builder
  voiceFace() {
    if (this.voiceListening()) {
      Column() {
        VoiceListening({
          accent: this.voiceSkin.accent,
          foreground: this.voiceSkin.keyForeground,
          secondary: this.voiceSkin.secondary,
          micPath: KeyboardIconPaths.MIC,
          title: this.voiceListeningTitle(),
          hint: this.voiceListeningHint(),
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
      } else {
        this.handwritingPanel()
      }
      // 手写 reaches the letters through the same language key every other face uses, which is what
      // HandwritingSchemeKeepsToolbarAndSwitchesBackToLetters asks of it.
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /**
   * 触屏手写板，即 Android 的 `rebuildHandwritingRows`：左侧竖排 ，。？！，中间是书写区，右侧竖排 ⌫ 和「重写」，下方是 `handwritingFace` 里的共享底行。⌫ 撤回最后一笔，手写板为空时删除光标前的字符；这里没有重做按钮，与 Android 一致。
   *
   * 参考线和状态行画在画布上层而不是画进画布：识别通过 `componentSnapshot` 读取画布，画进去的任何东西都会被当成笔迹。
   */
  @Builder
  handwritingPad() {
    Row({ space: this.keySpacing() }) {
      Column({ space: HANDWRITING_SIDE_GAP_VP }) {
        ForEach(NineKeyLayout.punctuation(), (symbol: string) => {
          Column() {
            Text(this.symbolFace(symbol))
              .fontSize(HANDWRITING_PUNCTUATION_FONT_SIZE)
              .fontColor(this.handwritingSkin.keyLabelColor(false, true))
              .textAlign(TextAlign.Center)
              .width('100%')
          }
          .layoutWeight(1)
          .width('100%')
          .justifyContent(FlexAlign.Center)
          .backgroundColor(this.handwritingSkin.keySurfaceBackground(false, true))
          .borderRadius(this.handwritingSkin.keyCornerRadius())
          .accessibilityText(KeyAccessibilityPolicy.symbol(symbol))
          .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
          .animation({ duration: KEY_PRESS_MS })
          .onClick(() => {
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_PUNCTUATION);
            KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
          })
        }, (symbol: string) => symbol + this.faceKey())
      }
      .layoutWeight(HANDWRITING_PUNCTUATION_WEIGHT)
      .height('100%')

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
          .onTouch((event: TouchEvent): void => this.handwritingTouch(event))
        // 设计稿的参考线：距边缘 14vp 的虚线框，以及穿过其中心的虚线十字，用细线颜色。
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
      .layoutWeight(HANDWRITING_AREA_WEIGHT)
      .height('100%')

      Column({ space: HANDWRITING_SIDE_GAP_VP }) {
        Column() {
          KeyboardLineIcon({
            d: KeyboardIconPaths.BACKSPACE,
            iconSize: KEY_ICON_VP,
            stroke: KeyboardIconPaths.KEY_STROKE,
            color: this.handwritingSkin.keyLabelColor(false, true)
          })
        }
        .layoutWeight(1)
        .width('100%')
        .justifyContent(FlexAlign.Center)
        .alignItems(HorizontalAlign.Center)
        .backgroundColor(this.handwritingSkin.keySurfaceBackground(false, true))
        .borderRadius(this.handwritingSkin.keyCornerRadius())
        .accessibilityText(KeyAccessibilityPolicy.delete())
        .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
        .animation({ duration: KEY_PRESS_MS })
        .onClick(() => this.deleteHandwriting())
        Text('重写')
          .fontSize(HANDWRITING_REWRITE_FONT_SIZE)
          .fontColor(this.handwritingSkin.keyLabelColor(false, true))
          .textAlign(TextAlign.Center)
          .layoutWeight(1)
          .width('100%')
          .backgroundColor(this.handwritingSkin.keySurfaceBackground(false, true))
          .borderRadius(this.handwritingSkin.keyCornerRadius())
          .accessibilityText('清空手写')
          .stateStyles({ normal: this.keyIdleStyle, pressed: this.keyPressedStyle })
          .animation({ duration: KEY_PRESS_MS })
          .onClick(() => this.clearHandwriting())
      }
      .layoutWeight(HANDWRITING_TOOLS_WEIGHT)
      .height('100%')
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
    const schemes: SchemeDefinition[] = this.offeredSchemes();
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

  /** 皮肤选择器的行高：两行预览，多于一页时再加圆点，在按键让出的高度内。 */
  /** 功能面板和方案选择每行的高度：两行填满面板去掉上下留白、行距和圆点后的高度，至少 `TILE_MIN_ROW_VP`。磁贴在行内居中。 */
  private tileRowHeight(count: number): number {
    const dots: number = count > FunctionPanelPolicy.PAGE_SIZE ? PANEL_DOTS_ROOM_VP : 0;
    return Math.max(TILE_MIN_ROW_VP, Math.floor((this.surfaceHeight() - TILE_TOP_PADDING_VP - TILE_ROWS_GAP_VP
      - dots - PANEL_BOTTOM_PADDING_VP) / 2));
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
    return Math.max(1, Math.ceil(KeyboardSession.shared.themeSkins(this.settings.dark).length
      / FunctionPanelPolicy.PAGE_SIZE));
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
          pageSize: FunctionPanelPolicy.PAGE_SIZE,
          rowHeight: this.tileRowHeight(this.schemePickerEntries().length),
          rowsGap: TILE_ROWS_GAP_VP,
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
          count: KeyboardSession.shared.themeSkins(this.settings.dark).length,
          pageSize: FunctionPanelPolicy.PAGE_SIZE,
          rowHeight: this.skinRowHeight(this.skinPageCount()),
          rowsGap: SKIN_ROWS_GAP_VP,
          columnsGap: this.skinColumnsGap(this.skinPageCount()),
          topPadding: SKIN_TOP_PADDING_VP,
          accent: this.skin.toggleForeground,
          hair: this.skin.hair,
          tile: (index: number): void => {
            this.skinTile(KeyboardSession.shared.themeSkins(this.settings.dark)[index])
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
        foreground: this.skin.keyForeground,
        accent: this.skin.toggleForeground,
        ringColor: this.skin.background,
        checkPath: KeyboardIconPaths.CHECK,
        // 语言随词库一起提供，所以该磁贴打开「词库」（`dc.html` 的添加项打开的是词库页），而不是设置首页。
        onTap: (): void => {
          this.show(SURFACE_NONE);
          KeyboardSession.shared.openSettings('dictionary');
        }
      })
    } else if (entry.kind === PICKER_SCHEME) {
      this.schemeChoiceTile(this.pickerScheme(entry))
    } else {
      PanelTile({
        label: ENGLISH_TITLES[entry.kind === PICKER_ENGLISH_9 ? 1 : 0],
        glyph: 'EN',
        subscript: entry.kind === PICKER_ENGLISH_9 ? '9' : '26',
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
          KeyboardSession.shared.recordKey(KeyIdPolicy.japaneseKana(keyIndex));
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
    if (this.touchCandidateMenu) {
      return managed === null ? [] : CandidateManagementAction.touchActions(
        managed.annotation, managed.annotationIsTranslation);
    }
    // The gloss is what the source offers here, and until now this host could show a translation without ever letting anyone type it: macOS has Option and a digit, and a touch keyboard has no modifiers to spare.
    return CandidateManagementAction.managementActions(
      managed === null ? '' : managed.annotation, managed !== null && managed.annotationIsTranslation,
      managed === null ? 0 : managed.fixedPosition,
      CandidateManagementAction.candidateActionsAvailable(
        KeyboardScheme.engineSchemeName(this.engineScheme), managed === null ? 0 : managed.source),
      CandidateManagementAction.hasMultipleCodePoints(managed === null ? '' : managed.text));
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

  /** 绘制哪些展开格：引擎列出的每个候选，按索引。触屏工具条只滚动当前页，所以网格是触屏键盘查看列表其余部分的唯一途径，从不截断。 */
  private expandedIndices(): number[] {
    const indices: number[] = [];
    for (let index: number = 0; index < this.expanded.length; index++) {
      indices.push(index);
    }
    return indices;
  }

  /** 网格在内边距以内排布格子所用的宽度；全拼九键三栏面板里是中栏的宽度（`expandedGridWidth`）。 */
  private expandedInnerWidth(): number {
    return Math.max(0, this.expandedGridWidth() - EXPANDED_PADDING_VP * 2);
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

  /** 一个格子跨几列：普通词一列，长的（如整句或日文转换结果）多列，这样能完整读到，而不是只截到开头几个字。 */
  private expandedSpan(index: number): number {
    if (index < 0 || index >= this.expanded.length) return 1;
    const natural: number = ExpandedCandidateLayout.width(this.expanded[index].text, EXPANDED_FONT_SIZE,
      EXPANDED_CELL_PADDING_VP, this.expandedInnerWidth());
    return ExpandedCandidateLayout.span(natural, this.expandedColumnWidth(), EXPANDED_GAP_VP, this.expandedColumns());
  }

  /** 格子的绘制宽度：所跨的列加上列间的间距，与分配其所在行时用的度量相同。 */
  private expandedCandidateWidth(index: number): number {
    const span: number = this.expandedSpan(index);
    return span * this.expandedColumnWidth() + (span - 1) * EXPANDED_GAP_VP;
  }

  /** 按行排列的绘制格子：`CandidateWrapPolicy` 把每格的跨度按 `expandedColumns()` 列装进各行，所以长候选会另起一行而不是溢出。 */
  private expandedRows(): number[][] {
    const indices: number[] = this.expandedIndices();
    const spans: number[] = [];
    for (let position: number = 0; position < indices.length; position++) {
      spans.push(this.expandedSpan(indices[position]));
    }
    const assignment: number[] = CandidateWrapPolicy.rows(this.expandedColumns(), 0, spans);
    const rows: number[][] = [];
    for (let position: number = 0; position < assignment.length; position++) {
      while (rows.length <= assignment[position]) {
        rows.push([]);
      }
      rows[assignment[position]].push(indices[position]);
    }
    return rows;
  }

  /** 供 `ForEach` 标识一个展开格的内容：它的位置、文字以及是否高亮。 */
  private expandedCellKey(index: number): string {
    return `${index}:${this.expanded[index].text}:${this.expandedHighlighted(index)}`;
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
      gloss.length > 0 ? `，${gloss}` : '');
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
            .animation({ duration: KEY_PRESS_MS })
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
    Scroll() {
      Column({ space: EXPANDED_GAP_VP }) {
        ForEach(this.expandedRows(), (row: number[]) => {
          Row({ space: EXPANDED_GAP_VP }) {
            ForEach(row, (index: number) => {
              Text(this.expanded[index].text)
                .width(this.expandedCandidateWidth(index))
                .height(EXPANDED_CELL_HEIGHT_VP)
                .padding({ left: EXPANDED_CELL_PADDING_VP, right: EXPANDED_CELL_PADDING_VP })
                .borderRadius(EXPANDED_CELL_RADIUS_VP)
                .backgroundColor(this.expandedHighlighted(index) ? this.skin.toggleBackground
                  : this.skin.keyBackground)
                .fontSize(EXPANDED_FONT_SIZE)
                .fontWeight(this.expandedHighlighted(index) ? 600 : 400)
                .fontColor(this.expandedHighlighted(index) ? this.skin.toggleForeground : this.skin.keyForeground)
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
                .accessibilityText(this.expandedLabel(index))
                .onClick(() => {
                  KeyboardSession.shared.chooseAny(index);
                  this.show(SURFACE_NONE);
                })
                // 原版对这里的长按给出与工具条上相同的注释。只给注释：管理操作按页面而不是按这个列表编号，所以这里没有对它们有意义的序号。
                .gesture(LongPressGesture().onAction(() => {
                  if (this.expanded[index].annotationIsTranslation) {
                    this.expandedGloss = index;
                  }
                }))
            }, (index: number) => this.expandedCellKey(index))
          }
          .width('100%')
          .justifyContent(FlexAlign.Start)
        }, (row: number[]) => row.map((index: number): string => this.expandedCellKey(index)).join('|'))
      }
      .width('100%')
      .padding(EXPANDED_PADDING_VP)
    }
    .scrollBar(BarState.Off)
    .edgeEffect(EdgeEffect.None)
    .align(Alignment.Top)
    .width('100%')
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

  /** 磁贴当前是否可用：繁体输出对日文没有意义，手写需要带中文方案的版本，语音和 AI 润色需要各自的服务商，强度需要开启振动。 */
  private functionTileEnabled(id: string): boolean {
    switch (id) {
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
        // 2in1 不绘制可收窄的触屏按键，所以该磁贴在那里不起作用；Android 在绘制分体键盘时也同样把它置灰。
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
        this.openHeightMode();
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
      case FunctionPanelPolicy.TRANSLATION:
        this.toggleTranslations();
        break;
      case FunctionPanelPolicy.SYMBOLS:
        this.openSymbolSurface();
        break;
      case FunctionPanelPolicy.LOCAL_INPUT:
        // 进入下一层，而不是在键盘自己的按键行上放八种模式，这是 iOS 最终采用的布局。
        this.show(SURFACE_MODES);
        break;
      case FunctionPanelPolicy.VOICE:
        this.show(SURFACE_VOICE);
        if (!this.voiceBusy && this.voiceText.length === 0) {
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_VOICE);
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
              ForEach(LocalInputMode.MODES, (mode: LocalInputModeDefinition) => {
                GridItem() {
                  this.toolCard(mode.title, mode.trigger, null, false, true, () => {
                    // The trigger is a capital the Engine reads as the way into that mode.
                    KeyboardSession.shared.press(mode.trigger.charCodeAt(0), true);
                    this.show(SURFACE_NONE);
                  });
                }
              }, (mode: LocalInputModeDefinition) => mode.id)
            }
            .columnsTemplate('1fr 1fr')
            .width('100%')
          }
          .width('100%')
        }
        .scrollBar(BarState.Off)
        .width('100%')
        .layoutWeight(1)
      } else {
        PagedTiles({
          count: FunctionPanelPolicy.items().length,
          pageSize: FunctionPanelPolicy.PAGE_SIZE,
          rowHeight: this.tileRowHeight(FunctionPanelPolicy.items().length),
          rowsGap: TILE_ROWS_GAP_VP,
          topPadding: TILE_TOP_PADDING_VP,
          resetToken: this.panelResetToken,
          accent: this.skin.toggleForeground,
          hair: this.skin.hair,
          tile: (index: number): void => {
            this.functionTile(FunctionPanelPolicy.items()[index])
          }
        })
      }
    }
    .width('100%')
    .height(this.surfaceHeight())
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
          // 保存和清空作用于本机历史，所以云端分区不提供它们。
          Text('保存当前')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .accessibilityText('保存剪贴板当前内容')
            .onClick(() => this.captureClipboard())
          Text(this.confirmingClear ? '确认清空' : '清空')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .onClick(() => this.clearClipboard())
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

  /** 触屏剪贴板卡片：正文最多两行，下方一小行说明是否已固定、来源和时间。 */
  @Builder
  clipCardBody(text: string, meta: string) {
    Column({ space: CLIP_CARD_LINE_GAP_VP }) {
      Text(text)
        .fontSize(CLIP_CARD_FONT_SIZE)
        .lineHeight(CLIP_CARD_LINE_HEIGHT_VP)
        .fontColor(this.clipboardSkin().keyForeground)
        .maxLines(2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .width('100%')
      if (meta.length > 0) {
        Text(meta)
          .fontSize(CLIP_META_FONT_SIZE)
          .fontColor(this.clipboardSkin().secondary)
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
                  this.clipCardBody(entry.text, this.cloudClipMeta(entry))
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
          Text(this.clipboardNotice)
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.clipboardSkin().secondary)
            .maxLines(2)
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
              if (this.touchDesign()) {
                Column() {
                  this.clipCardBody(entry.text, this.clipMeta(entry))
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
            }, (entry: ClipboardHistoryItem): string => `${entry.text}:${entry.pinned}:${entry.at}`)
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
   * 触屏表情面板，与设计稿和 Android 的 `buildEmojiPanel` 一致：没有标题栏，八列网格，底行是 ABC、分类标签和 ⌫。标签依次是有最近使用时的 🕘、每个 Unicode 分类一个图标，然后是「颜文字」和「符号」；后两页放的是文字，各有自己的网格，并在网格上方一行列出分组名。
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
            ForEach(EMOJI_PANEL_TABS.filter((panel: EmojiPanelTab): boolean => panel.id !== EMOJI_PANEL),
              (panel: EmojiPanelTab) => {
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

  /** 2in1 表情面板，参照 Windows 版：「表情」「颜文字」「符号」「剪贴板」标签、分组行、搜索行和网格。 */
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
          ForEach(KeyboardFormFactor.isDesktop() ? DESKTOP_EMOJI_PANEL_TABS : EMOJI_PANEL_TABS,
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
          } else {
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
      if (KeyboardFormFactor.isDesktop() && this.desktop === DesktopSurface.EMOJI) {
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
          ForEach(SymbolPanelPolicy.category(this.symbolCategory).symbols
            .slice(0, SymbolPanelPolicy.maxSymbolsPerCategory()), (symbol: string) => {
            GridItem() {
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
    Stack({ alignContent: Alignment.Top }) {
      // On a desktop candidate window with a mascot the backdrop starts at the card's top edge: the band above it is transparent, so the mascot stands on the card rather than on a slab of keyboard colour.
      Column() {
        this.skinBackdrop()
      }
      .width('100%')
      .height('100%')
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
      if (this.punctuation || this.brackets || this.managing >= 0
        || this.expandedGloss >= 0) {
        Column() {
          this.overlayStrip(this.punctuation ? '常用标点'
            : this.brackets ? '括弧'
              : this.expandedGloss >= 0 ? '释义' : '候选词管理')
        }
        .width('100%')
        .padding({
          left: this.overlayHorizontalInset(),
          right: this.overlayHorizontalInset(),
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

  /** 长按条所在位置：在第一行按键上方，工具条及其间隙下方。 */
  private overlayTopInset(): number {
    if (this.touchDesign()) {
      return KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP + KeyboardMetrics.STRIP_HEIGHT_VP
        + KeyboardMetrics.STRIP_GAP_VP;
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
      // 设计稿的框架：50vp 工具条上方 8vp，下方固定 10vp，按键行之间是用户设定的行距，底部 6vp，两侧 4vp。`KeyboardMetrics.totalHeightVp` 正是按此确定面板大小。
      Column() {
        this.touchStrip()
        Column({ space: this.rowSpacing() }) {
          this.keyFaces()
        }
        .width('100%')
        .margin({ top: KeyboardMetrics.STRIP_GAP_VP })
      }
      .width('100%')
      .padding({
        left: KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP,
        right: KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP,
        top: KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP,
        bottom: KeyboardMetrics.TOUCH_ROOT_BOTTOM_PADDING_VP
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
        // 按键作为上方工具条中正在拖动的高度的实时预览；覆盖其上的图层接收所有触摸，所以设置高度时不会输入任何内容。
        Stack() {
          Column({ space: this.rowSpacing() }) {
            this.handedKeys()
          }
          .width('100%')
          .height(this.surfaceHeight())
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor('#01000000')
            .hitTestBehavior(HitTestMode.Block)
            .accessibilityText('键盘高度预览')
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
   * 按键，或按住某键时代替它们的键面（九键长按和日文变体），开启「单手模式」时在单手侧栏旁收窄绘制。关闭时与以前完全一样，直接画进调用方的列里。代替按键的面板不在这里绘制，保持全宽，与 Android 在没有按键行显示时隐藏侧栏一致。
   */
  @Builder
  handedKeys() {
    if (this.oneHandActive()) {
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
      .width('100%')
      .alignItems(VerticalAlign.Center)
    } else {
      this.keyArea()
    }
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

  /** The actual keys, shared by normal input and the transparent adjustment overlay. */
  @Builder
  typingFace() {
      if (this.japanese) {
        // Including the digit layer: a grid face draws its own digits so the three columns survive.
        this.japaneseFace()
      } else if (KeyboardScheme.usesHandwritingFace(
        this.scheme, this.english, this.symbols, this.localMode)) {
        this.handwritingFace()
      } else if (KeyboardScheme.usesNineKeyFace(this.nineKey, this.localMode)) {
        this.nineKeyFace()
      } else if (this.zhuyinFace() && !this.symbols) {
        this.dachenFace()
      } else if (this.strokeFace() && !this.symbols) {
        this.strokeKeypadFace()
      } else if (this.designLayerShowing()) {
        this.designLayerFace()
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
            this.fixedKey({
              face: this.caseMode === LetterCaseMode.CAPS_LOCK ? '⇪' : '⇧',
              label: this.letterCase.accessibilityLabel(this.english),
              size: KeyboardMetrics.KEY_FONT_SIZE,
              emphasized: !this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE,
              special: !(this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE),
              width: KeyboardMetrics.MODIFIER_WIDTH_VP,
              share: this.touchDesign() ? LETTER_EDGE_SHARE : 0,
              icon: !this.touchDesign() ? ''
                : (this.caseMode === LetterCaseMode.CAPS_LOCK ? KeyboardIconPaths.CAPS_LOCK : KeyboardIconPaths.SHIFT),
              accentIcon: this.touchDesign() && this.caseMode !== LetterCaseMode.LOWERCASE,
              action: () => {
                KeyboardSession.shared.recordKey(KeyIdPolicy.SHIFT);
                this.letterCase.toggle(Date.now());
                this.caseMode = this.letterCase.mode();
              }
            });
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

  /**
   * 触屏键盘的符号层是否为设计稿的 123 / #+= 双层（`NumberSymbolLayout`）而不是共享符号行：所有把符号层交给字母行的键面，即 Android 的 `ImeLetterRows.drawsDesignLayer` 列出的那些，也就是 26 键、韩文、越南语、藏文、手写和笔画键盘。大千行保留共享符号行，注音在那里输入原本会被其注音键占用的标点，与 Android 一致；2in1 屏幕键盘也保留。
   */
  private designLayerShowing(): boolean {
    return this.touchDesign() && this.symbols && !this.zhuyinFace();
  }

  /** 该层是否带中文标点（全角，返回键为「拼音」）而不是 ASCII 标点（返回键为 ABC）：中文输入时是，英文和标点为 ASCII 的方案不是。 */
  private designLayerChinese(): boolean {
    return !this.english && !this.koreanFace() && !this.vietnameseFace() && !this.tibetanFace();
  }

  private designLayerRows(): LayerKey[][] {
    return NumberSymbolLayout.rows(this.moreSymbols, this.designLayerChinese());
  }

  /** 字符键输入它显示的内容；哪些键仍走符号行路径见 `NumberSymbolLayout.route`。 */
  private tapLayerCharacter(text: string): void {
    if (NumberSymbolLayout.route(text, this.designLayerChinese()) === LayerCharacterRoute.SYMBOL_KEY) {
      this.tapSymbol(text);
      return;
    }
    KeyboardSession.shared.recordKey(KeyIdPolicy.character(text));
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
    KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LAYER);
    this.moreSymbols = !this.moreSymbols;
  }

  /**
   * 触屏键盘的 123 和 #+= 层（Android 的 `ImeLetterRows.rebuildDesignLayer`）：两行各十个字符，层切换键、五个标点和删除键，然后用该层自己的底行代替操作行：「拼音」或 ABC 回到字母，123 层上是表情、#+= 层上是「符号」，以及空格和回车。
   */
  @Builder
  designLayerFace() {
    Row({ space: this.keySpacing() }) {
      ForEach(this.designLayerRows()[0], (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FIRST_ROW_FONT_SIZE));
      }, (key: LayerKey) => key.text + this.faceKey())
    }
    .width('100%')
    .height(this.rowHeight(0))

    Row({ space: this.keySpacing() }) {
      ForEach(this.designLayerRows()[1], (key: LayerKey) => {
        this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
      }, (key: LayerKey) => key.text + this.faceKey())
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
      ForEach(this.designLayerRows()[2].filter((key: LayerKey): boolean => key.kind === LayerKeyKind.CHARACTER),
        (key: LayerKey) => {
          this.fixedKey(this.layerCharacterKey(key, LAYER_FONT_SIZE));
        }, (key: LayerKey) => key.text + this.faceKey())
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true, NumberSymbolLayout.EDGE_WEIGHT);
    }
    .width('100%')
    .height(this.rowHeight(2))

    Row({ space: this.keySpacing() }) {
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
      if (this.moreSymbols) {
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
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_SYMBOL);
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
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_EMOJI);
            this.openEmojiSurface();
          }
        });
      }
      this.spaceKey(true, NumberSymbolLayout.SPACE_WEIGHT);
      this.designReturnKey(NumberSymbolLayout.RETURN_WEIGHT, false);
    }
    .width('100%')
    .height(this.rowHeight(3))
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
    Column({ space: this.rowSpacing() }) {
      this.strokeGrid()
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  @Builder
  strokeGrid() {
    Row({ space: this.keySpacing() }) {
      Column() {
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
          .accessibilityText(KeyAccessibilityPolicy.symbol(symbol))
          .responseRegion(this.railHitRegion(index, NineKeyLayout.punctuation().length))
          .onClick(() => {
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_PUNCTUATION);
            KeyboardSession.shared.punctuation(symbol.charCodeAt(0));
          })
        }, (symbol: string) => symbol + this.faceKey())
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
      .backgroundColor(this.skin.keySurfaceBackground(false, true))
      .borderRadius(KeyboardMetrics.KEY_CORNER_VP)

      Column({ space: this.rowSpacing() }) {
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
}
