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
import { KeyAccessibilityPolicy } from './input/KeyAccessibilityPolicy';
import { CandidateGlossPolicy } from './candidate/CandidateGlossPolicy';
import { ShuangpinKeyHintPolicy } from './input/ShuangpinKeyHintPolicy';
import { ReturnDispatch, ReturnKeyAction } from './input/ReturnKeyAction';
import { InputDiagnosticPolicy } from './input/InputDiagnosticPolicy';
import { LocalInputMode, LocalInputModeDefinition } from './input/LocalInputMode';
import { CandidateWrapPolicy } from './candidate/CandidateWrapPolicy';
import { ExpandedCandidateLayout } from './candidate/ExpandedCandidateLayout';
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
import {
  KeyboardLayoutDragAxis, KeyboardLayoutDragPolicy
} from './input/KeyboardLayoutDragPolicy';
import { NineKeyLayout, NineKey } from './input/NineKeyLayout';
import { SpaceCursorMovement } from './input/SpaceCursorMovement';
import { GlideArming, GlideKeyRect, GlidePoint, GlideTypingPolicy } from './input/GlideTypingPolicy';
import {
  EmojiCatalogModel, EmojiCategory, EmojiItem, EMOJI_PAGE_SIZE
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
import { VoiceRecognitionPolicy } from './input/VoiceRecognitionPolicy';
import { VoiceInputConfigurationPolicy } from './input/VoiceInputConfiguration';
import { ReplyKeyboardPolicy, ReplyStyle } from './ReplyKeyboardPolicy';
import { CommunityReplyTemplate } from './CommunityReplyLibraryPolicy';
import { SymbolPanelCategory, SymbolPanelPolicy } from './input/SymbolPanelPolicy';
import { BackspaceHoldAction, BackspaceHoldPolicy } from './input/BackspaceHoldPolicy';

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
  /**
   * 字母键的小写字母。设了它，键面和读屏文字在绘制时按当前大小写现算，`face` 和 `label` 不再用：自动大写在每次文字变化后异步切换大小写，若把大小写放进 `ForEach` 的键标识，每次切换都会重建全部字母键，手指正按着的那个键也被换掉，这次按键就丢了。
   */
  letter?: string;
  size: number;
  /** Filled with the platform accent: a held Shift, or Return while a composition is open. */
  emphasized: boolean;
  /** Drawn in the theme's function-key colour rather than the letter-key colour. */
  special?: boolean;
  width: number;
  action: () => void;
}

/** One switch on the idle strip; passed by reference so a flip redraws it. */
interface StripPillOptions {
  face: string;
  label: string;
  /** Drawn on the key colour: the scheme pill, which names the keyboard rather than a mode. */
  filled: boolean;
  /** A switch drawn in the accent while on and in the secondary text colour while off; absent for a pill that names a mode rather than switching one. */
  on?: boolean;
  action: () => void;
}

/** One function-panel tile; passed by reference so a switch redraws when it flips. */
interface ToolTileOptions {
  /** The glyph drawn in the tile when there is no icon. */
  glyph: string;
  icon: Resource | null;
  title: string;
  /** What a screen reader says when the name under the tile is not the whole action; absent reads the name. */
  label?: string;
  on: boolean;
  /** A switch, whose state a screen reader is told along with its name. */
  toggles: boolean;
  enabled: boolean;
  action: () => void;
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

// Long enough that a deliberate tap never repeats, short enough that a hold does not feel stuck.
const BACKSPACE_DELAY_MILLIS: number = 400;
const BACKSPACE_REPEAT_MILLIS: number = 60;
// How far the finger travels for one cursor step. Roughly a key width, which is what a step feels
// like it should cost.
const CURSOR_STEP_VP: number = 12;
const KEY_HINT_FONT_SIZE: number = 10;
// The glide trail's stroke, about as wide as a fingertip's line reads without hiding the key faces under it.
const GLIDE_TRAIL_WIDTH_VP: number = 4;
/** `.cursor { width: 1.5px }` in the source's candidate stylesheets. */
const PREEDIT_CARET_WIDTH_VP: number = 1.5;
// The idle strip: a 34vp logo button with a 22vp mark, 13px pills, and trailing icons at 20 in 34vp targets.
const SHORTCUT_ICON_VP: number = 20;
const SHORTCUT_TARGET_VP: number = 34;
const LOGO_BUTTON_VP: number = 34;
const LOGO_MARK_VP: number = 22;
const LOGO_BUTTON_RADIUS_VP: number = 10;
const STRIP_PILL_FONT_SIZE: number = 13;
const STRIP_PILL_RADIUS_VP: number = 999;
// The expand chevron and the settings gear at the end of the strip.
const STRIP_TRAILING_FONT_SIZE: number = 16;
// The function panel's tiles: 52vp squares at radius 12 with a 19px/600 glyph over a 12px name, four to a row with 10vp between rows and 8vp between columns, inset 6vp.
const TILE_VP: number = 52;
const TILE_RADIUS_VP: number = 12;
const TILE_ICON_VP: number = 24;
const TILE_GLYPH_FONT_SIZE: number = 19;
const TILE_LABEL_FONT_SIZE: number = 12;
const TILE_ROW_GAP_VP: number = 10;
const TILE_COLUMN_GAP_VP: number = 8;
const TILE_PANEL_PADDING_VP: number = 6;
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

const TOOL_ICON_VP: number = 18;
// Third, which is where iOS puts the English card.
const ENGLISH_CARD_POSITION: number = 2;
// English has no entry in KeyboardScheme to take its name from, being a mode rather than a scheme.
const ENGLISH_TITLES: string[] = ['英文 26 键', '英文 9 键'];
const HANDWRITING_CANVAS_ID: string = 'msime-harmony-handwriting-canvas';
const HANDWRITING_RECOGNITION_DELAY_MS: number = 550;
const SCHEME_BADGE_VP: number = 26;
const SCHEME_GLYPH_SIZE: number = 15;
const SCHEME_BADGE_TEXT_SIZE: number = 9;
// Enough tint to mark the card without drowning the glyph sitting on it.
const SELECTED_CARD_ALPHA: number = 0.14;
const GEOMETRY_HINT: string = '拖把手改高度，键盘上左右拖改键距、上下拖改行间距';

// The skin preview is a keyboard reduced to its shape: four rows of blank caps, the last one ending
// in the action key, which is the only place a skin's accent shows among its keys.
const PREVIEW_ROWS: number[] = [0, 1, 2, 3];
const PREVIEW_KEY_VP: number = 7;
const PREVIEW_ACTION_WEIGHT: number = 3;

function previewKeys(row: number): number[] {
  if (row === PREVIEW_ROWS.length - 1) {
    return [2, 1, 5, PREVIEW_ACTION_WEIGHT];
  }
  const counts: number[] = [10, 9, 9];
  const cells: number[] = [];
  for (let index: number = 0; index < counts[row]; index++) {
    cells.push(1);
  }
  return cells;
}
const TRANSPARENT: string = '#00000000';
// Roughly a third of a key. Shorter than this is a tap on the centre kana, not a flick.
const FLICK_THRESHOLD_VP: number = 16;
// The Engine's own numbering for Japanese, which the ported policies are written against.
const JAPANESE_SCHEME: number = 3;

// A category runs to a few hundred entries; this is the ceiling that keeps a catalog which never
// reports itself complete from spinning here forever.
const EMOJI_MAX_PAGES: number = 16;
const EMOJI_COLUMNS: number = 8;
const EMOJI_PANEL: string = 'emoji';
const KAOMOJI_PANEL: string = 'kaomoji';
const SYMBOLS_PANEL: string = 'symbols';
// The Windows emoji panel's last page (`EmojiPanel::Page::Clipboard`). Only a 2in1 draws it as a tab: the phone reaches clipboard history from its own tools face.
const CLIPBOARD_PANEL: string = 'clipboard';
// The clipboard surface's two sections: this device's history, and the account's cloud clipboard.
const CLIPBOARD_TAB_LOCAL: string = 'local';
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
  @State symbols: boolean = false;
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
  private clipListScroller: Scroller = new Scroller();
  /** Bumped by every cloud fetch and every drop, so only the newest answer is applied. */
  private cloudRequest: number = 0;
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
  @State surface: string = SURFACE_NONE;
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
  @State geometryHint: string = GEOMETRY_HINT;
  @State geometryKeySpacing: number = KeyboardSession.shared.preferences().keySpacingTenths;
  @State geometryRowSpacing: number = KeyboardSession.shared.preferences().rowSpacingTenths;
  @State geometryHeight: number = KeyboardSession.shared.preferences().heightAdjustment;
  @State touchVoiceShortcut: boolean = KeyboardSession.shared.preferences().voiceShortcut;

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
  private replyGeneration: number = 0;
  private polishGeneration: number = 0;
  private geometryStartX: number = 0;
  private geometryStartY: number = 0;
  private geometryBaseKeySpacing: number = 0;
  private geometryBaseRowSpacing: number = 0;
  private geometryBaseHeight: number = 0;
  private geometryAxis: KeyboardLayoutDragAxis = KeyboardLayoutDragAxis.UNDECIDED;
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
        this.startVoice();
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
      if (state.candidates.length === 0) {
        // The list belonged to a composition that no longer exists; keeping it on screen would offer
        // choices for something the user has already finished or abandoned.
        this.show(SURFACE_NONE);
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
    this.handwritingRecognition.changed();
    this.voiceRecognizer.shutdown();
    KeyboardSession.shared.voiceCancel();
    this.voiceGeneration = 0;
    // Replies and polish belong to the surface's current editor context. A view can disappear
    // while either request is awaiting the network; invalidate both generations so a late answer
    // cannot reappear when the same view is shown for a different field.
    this.replyGeneration += 1;
    this.replyBusy = false;
    this.replyResults = [];
    this.polishGeneration += 1;
    this.polishBusy = false;
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
   * 拼音键面用大写,切到英文才回小写。Every Chinese keyboard draws its 26 keys in caps, and the case
   * there is only a typeface: the key still feeds a lowercase letter to the Engine. Shift belongs to
   * English, where the case decides what actually lands in the document.
   *
   * 本地模式除外 —— 那里敲进去的就是字面的小写字母,键面写大写就是在骗人。
   */
  private letterFace(letter: string): string {
    if (this.koreanFace()) {
      return DubeolsikLayout.jamo(letter, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    if (this.vietnameseFace() || this.tibetanFace()) {
      // 越南语和藏文（威利转写区分大小写）的字母按 Shift 给出的大小写输入，所以键面显示那个大小写，和英文一样。
      return LetterKeyFacePolicy.face(letter, false, false, this.caseMode !== LetterCaseMode.LOWERCASE);
    }
    return LetterKeyFacePolicy.face(letter, !this.english, this.localMode !== 'none',
      this.caseMode !== LetterCaseMode.LOWERCASE);
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

  @Builder
  keyFace($$: KeyOptions) {
    Column() {
      Text($$.letter === undefined ? $$.face : this.letterFace($$.letter))
        .fontSize($$.size)
        .maxLines(1)
        .fontColor(this.skin.keyLabelColor($$.emphasized, $$.special === true))
        .fontFamily(this.skin.monospaced ? 'monospace' : undefined)
        .textAlign(TextAlign.Center)
        .width('100%')
      if ($$.hint !== undefined && $$.hint.length > 0) {
        // Along the bottom edge, which is why the letter above it is not centred in the whole key.
        Text($$.hint)
          .fontSize(KEY_HINT_FONT_SIZE)
          .maxLines(1)
          .fontColor(this.skin.accent)
          .textAlign(TextAlign.Center)
          .width('100%')
      }
    }
    .justifyContent(FlexAlign.Center)
    .width('100%')
    .height('100%')
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
    .responseRegion(this.keyHitRegion())
    .onAreaChange((_old: Area, now: Area): void => this.recordGlideKey($$.letter, now))
    .onClick($$.action)
  }

  /** A key whose width is fixed, so Shift and Delete stay the same size whatever the row holds. */
  @Builder
  fixedKey($$: KeyOptions) {
    Column() {
      this.keyFace($$)
    }
    .accessibilityText($$.letter === undefined ? ($$.label ?? $$.face) : this.letterLabel($$.letter))
    .width($$.width)
    .height('100%')
    .backgroundColor(this.skin.keySurfaceBackground($$.emphasized, $$.special === true))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
    .responseRegion(this.keyHitRegion())
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
    this.toggle(SURFACE_EMOJI);
    if (this.surface === SURFACE_EMOJI) {
      this.showEmojiPanel(this.emojiPanel);
    }
  }

  private openSymbolSurface(): void {
    if (KeyboardFormFactor.isDesktop()) {
      return;
    }
    this.symbolCategory = 0;
    this.symbolLocked = false;
    this.show(SURFACE_SYMBOLS);
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

  private selectScheme(candidate: SchemeDefinition): void {
    // The composition belongs to the scheme that was current when it was typed; carrying it into a
    // different one would reinterpret letters that were never meant that way.
    this.leaveComposition();
    if (this.hasHandwritingContext()) {
      this.clearHandwriting();
    }
    // Every field one scheme decides moves together, including the record of the Chinese scheme a
    // Japanese choice replaces; writing the Engine's scheme alone is what loses it.
    const changes: Map<string, Object> = KeyboardSession.shared.schemeChanges(candidate);
    const schemes: TouchKeyboardSchemes = {
      enabled: this.settings.enabledSchemes, selected: candidate.preferenceId
    };
    changes.set('touch_keyboard_schemes', schemes);
    if (!KeyboardSession.shared.changePreferences(changes)) {
      return;
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
    this.surface = surface;
    if (surface === SURFACE_REPLY) {
      this.replyTemplates = KeyboardSession.shared.communityReplyTemplates();
      this.replyTemplateMenu = false;
    }
    if (surface !== SURFACE_EXPANDED) {
      this.expanded = [];
      this.expandedGloss = -1;
    }
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
    this.touchVoiceShortcut = this.settings.voiceShortcut;
    this.feedback = KeyboardSession.shared.feedback();
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
   * What the strip calls the keyboard when nothing is being composed.
   *
   * A local mode takes the name, because while one is running the keys do something other than what
   * the scheme says and the user is owed that. English takes it for the same reason: the scheme is
   * still set underneath, but naming it would say the keyboard is spelling syllables when the same
   * keys are spelling words.
   */
  private title(): string {
    const mode: LocalInputModeDefinition | null =
      LocalInputMode.fromPreferenceKey(this.localMode);
    if (mode !== null) {
      return mode.title;
    }
    return this.english ? ENGLISH_TITLES[this.nineKey ? 1 : 0]
      : KeyboardScheme.title(this.scheme, this.wubiProfile);
  }

  private faceKey(): string {
    // The layer belongs here because the grid keeps its cells across the switch: 2 through 9 send
    // the same characters on both layers, so without it ForEach would judge the subtrees identical
    // and the digit layer would go on reading ABC, DEF, GHI. The twenty-six key rows never needed it
    // because their two layers hold different characters.
    // 大小写不在这里：字母键的键面和读屏文字在绘制时按大小写现算（见 `KeyOptions.letter`），自动大写切换时键节点保持不变，正按着的键不会被重建。
    return `${this.english}:${this.localMode}:${this.shuangpinProfile}:`
      + `${this.engineScheme}:${this.scheme.preferenceId}:${this.symbols}:${this.skin.key()}`;
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

  private expandedRows(): number[][] {
    const widths: number[] = [];
    for (let index: number = 0; index < this.expanded.length; index++) {
      widths.push(this.expandedCandidateWidth(index));
    }
    const assignment: number[] = CandidateWrapPolicy.rows(this.availableWidth, 0, widths);
    const rows: number[][] = [];
    for (let index: number = 0; index < assignment.length; index++) {
      while (rows.length <= assignment[index]) {
        rows.push([]);
      }
      rows[assignment[index]].push(index);
    }
    return rows;
  }

  private expandedCandidateWidth(index: number): number {
    if (index < 0 || index >= this.expanded.length) return 0;
    return ExpandedCandidateLayout.width(this.expanded[index].text, index,
      this.settings.candidateFontSize, KeyboardMetrics.CANDIDATE_PADDING_VP,
      this.availableWidth);
  }

  private letterKey(letter: string): KeyOptions {
    if (this.koreanFace()) {
      return {
        face: this.letterFace(letter),
        label: this.letterLabel(letter),
        letter: letter,
        size: KeyboardMetrics.KEY_FONT_SIZE,
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
      size: KeyboardMetrics.KEY_FONT_SIZE,
      emphasized: false,
      width: 0,
      action: () => this.tapLetter(letter)
    };
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
      this.localMode !== 'none', this.caseMode !== LetterCaseMode.LOWERCASE);
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

  /** Idle, the bar takes the composition line's height as well as its own row's. */
  private shortcutBarHeight(): number {
    return this.candidateRowHeight() + (this.stripIdle()
      ? KeyboardMetrics.compositionRowHeightVp(this.settings.candidatePreeditFontSize) : 0);
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

  @Builder
  candidateStrip() {
    Stack({ alignContent: Alignment.Top }) {
      Column() {
      // Idle on a touch keyboard the strip is one bar over the height of both lines below: the logo, the switches and the tools. Nothing shifts when a composition starts, because the two lines it gives way to add up to the same height.
      if (this.stripIdle()) {
        this.shortcutBar()
      } else {
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
                  .fontColor(this.compositionColor())
                  .fontFamily(this.candidateFontFamily())
                  .maxLines(1)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
              }
            } else {
              // 「候选栏预编辑：不显示」 on the phone: the line keeps its height and loses the spelling, but not the chosen part of a phrase or a local mode's trigger.
              Text(this.visibleComposition().text)
                .fontSize(this.settings.candidatePreeditFontSize)
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

      // The shortcut bar stands in for the candidates rather than sitting beside them: one row, two
      // jobs, which is how iOS arranges it and why the composition line above stays reserved. A
      // candidate window has no idle state to fill, and the surfaces those icons open are reached
      // from the app on a machine that has room for it.
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
      } else if (!KeyboardFormFactor.isDesktop()) {
        this.shortcutBar()
      }
      }
    }
      .width('100%')
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
      .fontSize(this.settings.candidatePreeditFontSize)
      .fontColor(this.skin.accent)
      .fontFamily(this.candidateFontFamily())
      .maxLines(1)
      .height('100%')
      .padding({ left: 12, right: 12 })
      .borderRadius(STRIP_PILL_RADIUS_VP)
      .backgroundColor(this.openableListShowing() ? this.skin.keyBackground : TRANSPARENT)
      .flexShrink(0)
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
      // A candidate window shows its whole page and pages with the keys and the wheel, as the Windows one does; the expand chevron is the touch strip's way to reach what does not fit.
      if (!KeyboardFormFactor.isDesktop()) {
        Text('\u2228')
          .fontSize(STRIP_TRAILING_FONT_SIZE)
          .fontColor(this.candidateColors().secondary)
          .padding({ left: 10, right: 12 })
          .onClick(() => {
            this.expanded = KeyboardSession.shared.allCandidates();
          })
      }
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
   * The idle strip: the logo, the switches a user flips mid-sentence, and the tools.
   *
   * The logo opens the function panel and is tinted while it is open. The pills are the language, the scheme (which opens the picker, and is the one pill drawn on a key-coloured fill because it names the keyboard), candidate translations and the punctuation width. The icons after them are the surfaces with no tile of their own on a phone's key rows, then dismiss and the settings application.
   */
  @Builder
  shortcutBar() {
    Row({ space: 4 }) {
      Column() {
        Image($r('app.media.keyboard_brand'))
          .width(LOGO_MARK_VP)
          .height(LOGO_MARK_VP)
          .fillColor(this.skin.accent)
      }
      .width(LOGO_BUTTON_VP)
      .height(LOGO_BUTTON_VP)
      .justifyContent(FlexAlign.Center)
      .borderRadius(LOGO_BUTTON_RADIUS_VP)
      .backgroundColor(this.isTools() ? this.skin.toggleBackground : TRANSPARENT)
      .accessibilityText(KeyAccessibilityPolicy.tools())
      .onClick(() => {
        this.toggle(SURFACE_TOOLS);
      })
      Scroll() {
        Row({ space: 4 }) {
          this.stripPill({
            face: this.english ? '英' : '中', label: KeyAccessibilityPolicy.languageState(this.english), filled: false,
            action: () => {
              KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_LANGUAGE);
              this.toggleTouchLanguage();
            }
          });
          this.stripPill({
            face: this.title(), label: KeyAccessibilityPolicy.scheme(), filled: true,
            action: () => {
              this.toggle(SURFACE_SCHEME);
            }
          });
          this.stripPill({
            face: '译',
            label: KeyAccessibilityPolicy.tile(KeyAccessibilityPolicy.translations(), undefined, this.translations, true),
            filled: false, on: this.translations,
            action: () => {
              this.toggleTranslations();
            }
          });
          this.stripPill({
            face: this.chinesePunctuation ? '，。' : ',.',
            label: KeyAccessibilityPolicy.punctuationState(this.chinesePunctuation),
            filled: false,
            action: () => {
              this.chinesePunctuation = KeyboardSession.shared.togglePunctuation();
            }
          });
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .layoutWeight(1)
      .align(Alignment.Start)
      this.shortcutIcon($r('app.media.tool_emoji'), KeyAccessibilityPolicy.emoji(),
        this.surface === SURFACE_EMOJI, () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_EMOJI);
          this.openEmojiSurface();
        });
      if (this.touchVoiceShortcut && this.voiceEnabled) {
        this.shortcutText('🎙', KeyAccessibilityPolicy.voice(),
          this.surface === SURFACE_VOICE, () => {
            KeyboardSession.shared.recordKey(KeyIdPolicy.SOFT_VOICE);
            this.show(SURFACE_VOICE);
            if (!this.voiceBusy && this.voiceText.length === 0) {
              this.startVoice();
            }
          });
      }
      // 高情商回复是工具而不是输入方案：任何方案下都从这里打开回复面板，再点一次或点面板里的「完成」回到原来的键盘。
      this.shortcutText('💬', KeyAccessibilityPolicy.reply(),
        this.surface === SURFACE_REPLY, () => {
          this.toggle(SURFACE_REPLY);
        });
      this.shortcutIcon($r('app.media.tool_dismiss'), KeyAccessibilityPolicy.dismiss(), false, () => {
        KeyboardSession.shared.dismiss();
      });
      this.shortcutText('\u2699', KeyAccessibilityPolicy.settings(), false, () => {
        this.show(SURFACE_NONE);
        KeyboardSession.shared.openSettings();
      });
    }
    .width('100%')
    .height(this.shortcutBarHeight())
    .padding({ left: 4, right: 4 })
    .alignItems(VerticalAlign.Center)
  }

  /** A pill's text: the accent tint for a switch that is on, the secondary colour for one that is off, the key text for a pill that names a mode. */
  private pillForeground(on: boolean | undefined): string {
    if (on === undefined) {
      return this.skin.keyForeground;
    }
    return on ? this.skin.toggleForeground : this.skin.secondary;
  }

  /** One switch on the idle strip. Only the scheme pill is filled, with the key colour, because it names the keyboard rather than a mode. */
  @Builder
  stripPill($$: StripPillOptions) {
    Text($$.face)
      .fontSize(STRIP_PILL_FONT_SIZE)
      .fontColor(this.pillForeground($$.on))
      .maxLines(1)
      .padding({ left: 10, right: 10, top: 5, bottom: 5 })
      .borderRadius(STRIP_PILL_RADIUS_VP)
      .backgroundColor($$.filled ? this.skin.keyBackground : TRANSPARENT)
      .accessibilityText($$.label)
      .onClick($$.action)
  }

  /**
   * One tool in the shortcut bar. The label is not optional here the way it is on a key: a key at
   * least draws a character, and every one of these draws only an icon.
   */
  @Builder
  shortcutIcon(source: Resource, label: string, active: boolean, action: () => void) {
    Column() {
      Image(source)
        .width(SHORTCUT_ICON_VP)
        .height(SHORTCUT_ICON_VP)
        // Idle icons are the keyboard's own text colour, as the design's gear is; the one whose surface is open takes the accent.
        .fillColor(active ? this.skin.accent : this.skin.keyForeground)
    }
    .width(SHORTCUT_TARGET_VP)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .accessibilityText(label)
    .onClick(action)
  }

  @Builder
  shortcutText(face: string, label: string, active: boolean, action: () => void) {
    Column() {
      Text(face)
        .fontSize(STRIP_TRAILING_FONT_SIZE)
        .fontColor(active ? this.skin.accent : this.skin.keyForeground)
        .textAlign(TextAlign.Center)
    }
    .width(SHORTCUT_TARGET_VP)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    // A glyph is a picture with a name nobody wants read out; 🎙 would be announced as "microphone".
    .accessibilityText(label)
    .onClick(action)
  }

  @Builder
  nineKeyFace() {
    Column({ space: this.rowSpacing() }) {
      this.nineKeyGrid()
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
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
        ForEach(this.symbols ? NineKeyLayout.digits() : NineKeyLayout.rows(), (row: NineKey[]) => {
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
          this.deleteKey('100%', true)
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
      size: KeyboardMetrics.SMALL_KEY_FONT_SIZE,
      emphasized: false,
      width: 0,
      // A digit is inserted, not spelled: the same call the twenty-six key symbol row makes for
      // its own digit row, rather than handing it to the Engine as a candidate position.
      action: this.symbols
        ? () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
          KeyboardSession.shared.punctuation(key.input.charCodeAt(0));
        }
        : () => {
          KeyboardSession.shared.recordKey(KeyIdPolicy.nineKey(key.input));
          KeyboardSession.shared.press(key.input.charCodeAt(0), false);
        }
    };
  }

  @Builder
  nineKeyHoldKey(key: NineKey) {
    Column() {
      this.keyFace(this.nineKeyKeyOptions(key))
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
   */
  @Builder
  actionRow() {
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
            action: () => {
              KeyboardSession.shared.recordKey(KeyIdPolicy.character(','));
              // A Dachen comma is ㄝ, so Zhuyin types the mark itself.
              if (this.zhuyinFace()) {
                this.typeZhuyinMark(',');
              } else {
                KeyboardSession.shared.punctuation(0x2c);
              }
            }
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
      this.spaceKey();
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
  deleteKey(width: Length, intoGaps: boolean) {
    Column() {
      Text('⌫')
        .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
        .fontColor(this.skin.keyLabelColor(false, true))
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .width(width)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .accessibilityText(KeyAccessibilityPolicy.delete())
    .backgroundColor(this.skin.keySurfaceBackground(false, true))
    .linearGradient({ angle: 180,
      colors: [[this.skin.materialTop(), 0], [this.skin.materialBottom(), 1]] })
    .borderRadius(this.skin.keyCornerRadius())
    .borderWidth(this.skin.borderWidth)
    .borderColor(this.skin.borderColor)
    .shadow({ radius: this.skin.shadowRadius, color: this.skin.shadowColor(),
      offsetX: 0, offsetY: this.skin.shadowOffset })
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
  spaceKey() {
    Column() {
      Text(this.japanese
        ? JapaneseNineKeyActions.spaceTitle(this.editing.length > 0)
        : (this.english ? 'space' : '空格'))
        .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
        .fontColor(this.skin.secondary)
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .layoutWeight(1)
    .height('100%')
    .justifyContent(FlexAlign.Center)
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

  private openClipboard(): void {
    if (!this.clipboardEnabled) {
      this.clipboardNotice = '剪贴板历史已在设置中关闭';
      this.show(SURFACE_NONE);
      return;
    }
    this.show(SURFACE_CLIPBOARD);
    this.clips = KeyboardSession.shared.clipboardHistory();
    this.clipboardNotice = '';
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
    this.clipSendTarget = text;
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
    this.cloudSending = true;
    KeyboardSession.shared.sendToCloudClipboard(text).then((outcome: CloudClipboardSendOutcome): void => {
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
  private applyGeometry(keySpacing: number, rowSpacing: number, height: number,
                        resetVoice: boolean = false): void {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('touch_key_spacing_tenths', KeyboardGeometry.keySpacing(keySpacing));
    changes.set('touch_row_spacing_tenths', KeyboardGeometry.rowSpacing(rowSpacing));
    changes.set('touch_keyboard_height_adjustment', KeyboardGeometry.heightAdjustment(height));
    if (resetVoice) {
      changes.set('touch_voice_shortcut', false);
    }
    if (!KeyboardSession.shared.changePreferences(changes)) {
      return;
    }
    this.settings = KeyboardSession.shared.preferences();
    this.geometryKeySpacing = this.settings.keySpacingTenths;
    this.geometryRowSpacing = this.settings.rowSpacingTenths;
    this.geometryHeight = this.settings.heightAdjustment;
    this.touchVoiceShortcut = this.settings.voiceShortcut;
    KeyboardSession.shared.resizeTouchKeyboard(KeyboardMetrics.totalHeightVp(
      this.geometryRowSpacing, this.geometryHeight, KeyboardSession.shared.glossRows(),
      this.settings.candidateFontSize, this.settings.candidatePreeditFontSize));
  }

  private beginGeometryDrag(event: TouchEvent): void {
    this.geometryStartX = event.touches[0].windowX;
    this.geometryStartY = event.touches[0].windowY;
    this.geometryBaseKeySpacing = this.geometryKeySpacing;
    this.geometryBaseRowSpacing = this.geometryRowSpacing;
    this.geometryBaseHeight = this.geometryHeight;
    this.geometryAxis = KeyboardLayoutDragAxis.UNDECIDED;
  }

  /** Drag over the visible keys: first movement locks the gesture to one spacing dimension. */
  private geometrySpacingTouch(event: TouchEvent): void {
    if (event.type === TouchType.Down) {
      this.beginGeometryDrag(event);
      return;
    }
    if (event.type === TouchType.Move) {
      const offsetX: number = event.touches[0].windowX - this.geometryStartX;
      const offsetY: number = event.touches[0].windowY - this.geometryStartY;
      if (this.geometryAxis === KeyboardLayoutDragAxis.UNDECIDED) {
        this.geometryAxis = KeyboardLayoutDragPolicy.axis(offsetX, offsetY);
      }
      if (this.geometryAxis === KeyboardLayoutDragAxis.KEY_SPACING) {
        const spacing: number = KeyboardLayoutDragPolicy.keySpacing(
          this.geometryBaseKeySpacing, offsetX);
        if (spacing !== this.geometryKeySpacing) {
          this.applyGeometry(spacing, this.geometryRowSpacing, this.geometryHeight);
        }
        this.geometryHint = `按键间距 ${KeyboardGeometry.display(spacing)}`;
      } else {
        const spacing: number = KeyboardLayoutDragPolicy.rowSpacing(
          this.geometryBaseRowSpacing, offsetY);
        if (spacing !== this.geometryRowSpacing) {
          this.applyGeometry(this.geometryKeySpacing, spacing, this.geometryHeight);
        }
        this.geometryHint = `行间距 ${KeyboardGeometry.display(spacing)}`;
      }
      return;
    }
    this.geometryAxis = KeyboardLayoutDragAxis.UNDECIDED;
    this.geometryHint = GEOMETRY_HINT;
  }

  /** Drag the toolbar grip as the keyboard's top edge; upward movement makes the panel taller. */
  private geometryHeightTouch(event: TouchEvent): void {
    if (event.type === TouchType.Down) {
      this.beginGeometryDrag(event);
      return;
    }
    if (event.type === TouchType.Move) {
      const height: number = KeyboardLayoutDragPolicy.height(
        this.geometryBaseHeight, event.touches[0].windowY - this.geometryStartY);
      if (height !== this.geometryHeight) {
        this.applyGeometry(this.geometryKeySpacing, this.geometryRowSpacing, height);
      }
      this.geometryHint = `键盘高度 ${KeyboardGeometry.displayHeight(height)}`;
      return;
    }
    this.geometryHint = GEOMETRY_HINT;
  }

  /** Accessible discrete counterpart to the height grip on SDKs without adjustable actions. */
  private adjustGeometryHeight(delta: number): void {
    const height: number = KeyboardGeometry.heightAdjustment(
      this.geometryHeight + delta);
    this.applyGeometry(this.geometryKeySpacing, this.geometryRowSpacing, height);
    this.geometryHint = `键盘高度 ${KeyboardGeometry.displayHeight(height)}`;
  }

  private setFeedback(settings: FeedbackSettings): void {
    if (KeyboardSession.shared.saveFeedback(settings)) {
      this.feedback = settings;
    }
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
    context.clearRect(0, 0, HANDWRITING_CANVAS_SIZE, HANDWRITING_CANVAS_SIZE);
    context.strokeStyle = this.handwritingSkin.keyForeground;
    context.lineWidth = 4;
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

  /** Waits for the same 550 ms pause as the Apple source before snapshotting the whole character. */
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
    }, HANDWRITING_RECOGNITION_DELAY_MS);
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

  private startVoice(): void {
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
    this.voiceText = '';
    this.voiceNotice = '正在聆听…';
    this.voiceBusy = true;
    this.voiceFinishing = false;
    KeyboardSession.shared.setVoiceRecording(true);
    const voiceConfiguration = KeyboardSession.shared.voiceInputConfiguration();
    this.voiceRecognizer.start(voiceConfiguration.language || 'zh-CN', voiceConfiguration,
      (text: string, final: boolean): void => {
      // A partial result only reaches the panel when the user asked to watch the recognizer work.
      // The final one always does: it is the result.
      if (text.length > 0
          && (final || VoiceRecordingBehaviourPolicy.showsInterimResults(voiceConfiguration))) {
        this.voiceText = VoiceRecognitionPolicy.result(text);
      }
      if (final && this.voiceGeneration > 0) {
        const accepted: string | null = KeyboardSession.shared.voiceApply(
          this.voiceGeneration, this.voiceText);
        this.voiceGeneration = 0;
        if (accepted !== null) {
          this.voiceText = accepted;
        }
        this.voiceFinishing = false;
        this.voiceBusy = false;
        KeyboardSession.shared.setVoiceRecording(false);
        this.voiceNotice = accepted !== null ? '识别完成，可提交结果。' :
          (this.voiceText.length > 0 ? '已忽略过期的语音结果。' : '没有识别到语音。');
      }
      }, (message: string): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = message;
      }, KeyboardSession.shared.voiceHotwordOptions()).catch((error: Object): void => {
      KeyboardSession.shared.voiceCancel();
      this.voiceGeneration = 0;
      this.voiceBusy = false;
      this.voiceFinishing = false;
      KeyboardSession.shared.setVoiceRecording(false);
      this.voiceNotice = error instanceof Error ? error.message : '语音识别不可用';
    });
  }

  private stopVoice(): void {
    if (!this.voiceBusy || this.voiceFinishing) {
      return;
    }
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
    KeyboardSession.shared.insertWithSource(text, 'voice');
    this.voiceText = '';
    this.voiceNotice = '已提交';
  }

  private voiceDescription(): string {
    return VoiceInputConfigurationPolicy.description(
      KeyboardSession.shared.voiceInputConfiguration());
  }

  @Builder
  voiceFace() {
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
      this.handwritingPanel()
      // 手写 reaches the letters through the same language key every other face uses, which is what
      // HandwritingSchemeKeepsToolbarAndSwitchesBackToLetters asks of it.
      this.actionRow()
    }
    .width('100%')
    .height(this.surfaceHeight())
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
   * The scheme and skin pickers, which take over the key rows rather than floating above them.
   *
   * An extension panel is a fixed rectangle with nothing to overlay, so a popup would either be
   * clipped at the panel edge or force the panel to grow and shove the editor up the screen. Taking
   * the space that is already there costs nothing and needs no resize.
   */
  @Builder
  pickerFace() {
    Column() {
      Row() {
        Text(this.surface === SURFACE_SCHEME ? '输入方案' : '选择主题')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .layoutWeight(1)
        Text('完成')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .onClick(() => {
            this.show(SURFACE_NONE);
          })
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      Scroll() {
        if (this.surface === SURFACE_SCHEME) {
          // Four to a row, each a glyph badge over its name, which is the arrangement iOS uses and
          // the reason SchemeDefinition carries a glyph and a badge at all.
          Grid() {
            ForEach(this.offeredSchemes(), (candidate: SchemeDefinition, index: number) => {
              // English sits third, where iOS puts it, its two grids side by side the way the two
              // quanpin grids are: a platform text mode rather than a second persisted Engine
              // scheme, so neither card has an entry in KeyboardScheme to come from.
              if (index === ENGLISH_CARD_POSITION) {
                GridItem() {
                  this.englishCard(false);
                }
                GridItem() {
                  this.englishCard(true);
                }
              }
              GridItem() {
                this.schemeCard(candidate);
              }
            }, (candidate: SchemeDefinition) =>
              // 带上五笔版本：只改 86/98 时卡片也要重建，否则还显示旧的角标。
              `${candidate.id}:${this.scheme.id}:${this.english}:${this.nineKey}:${this.wubiProfile}`)
          }
          .columnsTemplate('1fr 1fr 1fr 1fr')
          .width('100%')
        } else {
          // Two to a row, each a miniature of the keyboard in that skin: the colours are the whole
          // choice, so a name beside a swatch says less than showing the thing.
          Grid() {
            ForEach(KeyboardSession.shared.themeSkins(this.settings.dark), (choice: KeyboardSkin) => {
              GridItem() {
                this.skinCard(choice);
              }
            }, (choice: KeyboardSkin) => choice.key() + this.skin.id)
          }
          .columnsTemplate('1fr 1fr')
          .width('100%')
        }
      }
      .scrollBar(BarState.Off)
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /**
   * The English modes, drawn as scheme cards because that is where a typist looks for them.
   *
   * English is a platform text mode rather than a second persisted Engine scheme, so neither card
   * has an entry in KeyboardScheme to come from; what distinguishes them is only the grid the
   * letters arrive on. The Engine spells words from the same digits the grid sends for syllables.
   */
  @Builder
  englishCard(nineKey: boolean) {
    Column() {
      Row() {
        Text('EN')
          .fontSize(SCHEME_GLYPH_SIZE - 1)
          .fontColor(this.isCurrentEnglish(nineKey) ? this.skin.accent : this.skin.keyForeground)
          .textAlign(TextAlign.Center)
          .width(SCHEME_BADGE_VP)
          .height(SCHEME_BADGE_VP)
          .borderWidth(1.4)
          .borderColor(this.isCurrentEnglish(nineKey) ? this.skin.accent : this.skin.keyForeground)
          .borderRadius(5)
        Text(nineKey ? '9' : '26')
          .fontSize(SCHEME_BADGE_TEXT_SIZE)
          .fontColor(this.isCurrentEnglish(nineKey) ? this.skin.accent : this.skin.keyForeground)
          .padding({ left: 1 })
      }
      .alignItems(VerticalAlign.Bottom)
      .height(SCHEME_BADGE_VP)

      Text(ENGLISH_TITLES[nineKey ? 1 : 0])
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.isCurrentEnglish(nineKey) ? this.skin.accent : this.skin.keyForeground)
        .maxLines(1)
        .margin({ top: 2 })
    }
    .width('100%')
    .padding(5)
    .backgroundColor(this.isCurrentEnglish(nineKey)
      ? this.skin.tintedAccent(SELECTED_CARD_ALPHA) : Color.Transparent)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .onClick(() => this.selectEnglish(nineKey))
  }

  /** The scheme's glyph in a bordered badge, its badge text tucked into the corner, name beneath. */
  @Builder
  schemeCard(candidate: SchemeDefinition) {
    Column() {
      // Bottom-aligned rather than offset: the badge's baseline sits on the frame's lower edge, to
      // the right of it. Nudging it into place with an offset put it a line lower than the frame and
      // reading as part of the title underneath.
      Row() {
        Text(candidate.glyph)
          .fontSize(SCHEME_GLYPH_SIZE)
          .fontColor(this.isCurrentScheme(candidate) ? this.skin.accent : this.skin.keyForeground)
          .textAlign(TextAlign.Center)
          .width(SCHEME_BADGE_VP)
          .height(SCHEME_BADGE_VP)
          .borderWidth(1.4)
          .borderColor(this.isCurrentScheme(candidate)
            ? this.skin.accent : this.skin.keyForeground)
          .borderRadius(5)
        Text(KeyboardScheme.badge(candidate, this.wubiProfile))
          .fontSize(SCHEME_BADGE_TEXT_SIZE)
          .fontColor(this.isCurrentScheme(candidate) ? this.skin.accent : this.skin.keyForeground)
          .padding({ left: 1 })
      }
      .alignItems(VerticalAlign.Bottom)
      .height(SCHEME_BADGE_VP)

      Text(KeyboardScheme.title(candidate, this.wubiProfile))
        .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
        .fontColor(this.isCurrentScheme(candidate) ? this.skin.accent : this.skin.keyForeground)
        .maxLines(1)
        .margin({ top: 2 })
    }
    .width('100%')
    .padding(5)
    .backgroundColor(this.isCurrentScheme(candidate)
      ? this.skin.tintedAccent(SELECTED_CARD_ALPHA) : Color.Transparent)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .opacity(this.schemeAvailable(candidate) ? 1 : 0.4)
    .onClick(() => this.selectScheme(candidate))
  }

  /** A miniature keyboard in the skin being offered, drawn from the same values the real one uses. */
  @Builder
  skinCard(choice: KeyboardSkin) {
    Column() {
      Row() {
        Text(choice.title)
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(choice.keyForeground)
          .layoutWeight(1)
        if (choice.id === this.skin.id) {
          Text('\u2713').fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE).fontColor(choice.accent)
        }
      }
      .width('100%')
      .padding({ left: 6, right: 6, bottom: 3 })

      ForEach(PREVIEW_ROWS, (row: number) => {
        Row({ space: 2 }) {
          ForEach(previewKeys(row), (cell: number) => {
            Row()
              .layoutWeight(cell)
              .height(PREVIEW_KEY_VP)
              .backgroundColor(row === PREVIEW_ROWS.length - 1 && cell === PREVIEW_ACTION_WEIGHT
                ? choice.actionBackground : choice.keyBackground)
              .borderRadius(2)
          }, (cell: number, index: number) => `${row}:${index}`)
        }
        .width('100%')
        .margin({ bottom: 2 })
      }, (row: number) => `${row}:${choice.key()}`)
    }
    .width('100%')
    .padding(6)
    .margin(3)
    .backgroundColor(choice.background)
    .borderRadius(KeyboardMetrics.KEY_CORNER_VP)
    .borderWidth(choice.id === this.skin.id ? 1.5 : 0)
    .borderColor(choice.accent)
    .onClick(() => this.selectSkin(choice))
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

  /** Every candidate at once, which is the alternative to paging past the ninth one by hand. */
  @Builder
  expandedFace() {
    Column() {
      Row() {
        Text(this.visibleComposition().text)
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .maxLines(1)
          .layoutWeight(1)
        Text('收起')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.skin.accent)
          .onClick(() => {
            this.show(SURFACE_NONE);
          })
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      Scroll() {
        Column() {
          ForEach(this.expandedRows(), (row: number[]) => {
            Row() {
              ForEach(row, (index: number) => {
                Row() {
                  Text(`${index + 1}`)
                    .fontSize(Math.max(KEY_HINT_FONT_SIZE, this.settings.candidateFontSize - 8))
                    .fontColor(this.candidateDetailColor())
                    .fontFamily(this.candidateFontFamily())
                    .padding({ right: 4 })
                  Column() {
                    Text(this.expanded[index].text)
                      .fontSize(this.settings.candidateFontSize)
                      .fontColor(this.candidateTextColor())
                      .fontFamily(this.candidateFontFamily())
                      .maxLines(1)
                      .textOverflow({ overflow: TextOverflow.Ellipsis })
                    // The same line the strip draws under a candidate. It used to be dropped on the
                    // way here, so the panel listed words while the strip beside it explained them.
                    if (this.expandedGlossLine(index).length > 0) {
                      Text(this.expandedGlossLine(index))
                        .fontSize(Math.max(KEY_HINT_FONT_SIZE,
                          this.settings.candidateFontSize - 8))
                        .fontColor(this.candidateDetailColor())
                        .fontFamily(this.candidateFontFamily())
                        .maxLines(1)
                    }
                  }
                  .alignItems(HorizontalAlign.Start)
                  .layoutWeight(1)
                }
                  .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                    right: KeyboardMetrics.CANDIDATE_PADDING_VP })
                  .width(this.expandedCandidateWidth(index))
                  .height(KeyboardMetrics.ROW_HEIGHT_VP)
                  .onClick(() => {
                    KeyboardSession.shared.chooseAny(index);
                    this.show(SURFACE_NONE);
                  })
                  // The source answers a long press here with the same gloss it offers on the
                  // strip. Only the gloss: the management operations index the page rather than
                  // this list, so there is no number here that would mean anything to them.
                  .gesture(LongPressGesture().onAction(() => {
                    if (this.expanded[index].annotationIsTranslation) {
                      this.expandedGloss = index;
                    }
                  }))
              }, (index: number) => `${index}:${this.expanded[index].text}`)
              Blank()
            }
            .width('100%')
          }, (row: number[]) => row.join(','))
        }
        .width('100%')
      }
      .scrollBar(BarState.Off)
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** Transparent controls over the live keys: the thing being adjusted remains visible. */
  @Builder
  geometryFace() {
    Column() {
      Row({ space: 8 }) {
        Text('恢复默认')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor('#C7473B')
          .onClick(() => {
            this.applyGeometry(KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
              KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
              KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP, true);
            this.geometryHint = GEOMETRY_HINT;
          })
        Text('−')
          .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .accessibilityText('降低键盘高度')
          .onClick(() => this.adjustGeometryHeight(-2))
        Column({ space: 4 }) {
          Text(this.geometryHint)
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.keyForeground)
            .opacity(0.75)
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
          Divider()
            .width(46)
            .strokeWidth(5)
            .color(this.skin.tintedAccent(0.45))
        }
        .layoutWeight(1)
        .height('100%')
        .justifyContent(FlexAlign.Center)
        .alignItems(HorizontalAlign.Center)
        .accessibilityText('键盘高度，向上或向下拖动')
        .onTouch((event: TouchEvent): void => this.geometryHeightTouch(event))
        Text('+')
          .fontSize(KeyboardMetrics.KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .accessibilityText('增加键盘高度')
          .onClick(() => this.adjustGeometryHeight(2))
        Text('完成')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .onClick(() => {
            this.show(SURFACE_NONE);
          })
      }
      .width('96%')
      .height(52)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })
      .backgroundColor(this.skin.keyBackground)
      .borderRadius(10)

      Blank()
        .width('100%')
        .layoutWeight(1)
        // A nearly transparent hit target blocks keys while preserving their exact appearance.
        .backgroundColor('#01000000')
        .accessibilityText('键盘布局调整区域')
        .onTouch((event: TouchEvent): void => this.geometrySpacingTouch(event))
    }
    .width('100%')
    .height(this.surfaceHeight())
    .alignItems(HorizontalAlign.Center)
  }

  /**
   * The function panel: tiles four to a row, the first eight where the design puts its eight.
   *
   * The eight are the design's: 拼, 译, 中/英, 全, 主题, 标点, 模糊音 and 设置. Everything the panel held before follows as more tiles, 繁体 and 表情 first, scrolled to: 本地输入 goes one level in rather than putting eight modes on the keyboard's own rows, which is the arrangement iOS settled on, and AI 润色 is offered only when the host has a configured provider, so an unavailable service is a disabled tile instead of a dead entry.
   */
  @Builder
  toolsFace() {
    Column() {
      if (this.surface === SURFACE_MODES) {
        Row() {
          Text('\u2039 返回')
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
      }

      Scroll() {
        Column() {
          if (this.surface === SURFACE_MODES) {
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
          } else {
            Grid() {
              GridItem() {
                this.toolTile({
                  glyph: '拼', icon: null, title: KeyboardScheme.title(this.scheme, this.wubiProfile), label: KeyAccessibilityPolicy.scheme(),
                  on: false, toggles: false,
                  enabled: true, action: () => {
                    this.show(SURFACE_SCHEME);
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '译', icon: null, title: KeyAccessibilityPolicy.translations(), on: this.translations,
                  toggles: true, enabled: true, action: () => {
                    this.toggleTranslations();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: this.english ? '英' : '中', icon: null, title: this.english ? '英文' : '中文',
                  label: KeyAccessibilityPolicy.languageState(this.english), on: !this.english, toggles: false,
                  enabled: true, action: () => {
                    this.toggleTouchLanguage();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '全', icon: null, title: '全角', on: this.fullWidth, toggles: true,
                  enabled: true, action: () => {
                    this.fullWidth = KeyboardSession.shared.toggleCharacterWidth();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '\u25D0', icon: null, title: `主题 · ${this.skin.title}`, label: KeyAccessibilityPolicy.theme(),
                  on: false, toggles: false,
                  enabled: true, action: () => {
                    this.show(SURFACE_SKIN);
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '，', icon: null, title: '中文标点', on: this.chinesePunctuation,
                  toggles: true, enabled: true, action: () => {
                    this.chinesePunctuation = KeyboardSession.shared.togglePunctuation();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '\u2248', icon: null, title: '模糊音', on: this.fuzzyPinyin, toggles: true,
                  enabled: true, action: () => {
                    this.fuzzyPinyin = KeyboardSession.shared.toggleFuzzyPinyin();
                  }
                });
              }
              GridItem() {
                // The source's tray and toolbar keep the whole settings application one click away while typing; this is the phone's way there, since it has neither.
                this.toolTile({
                  glyph: '\u2699', icon: null, title: '设置', label: KeyAccessibilityPolicy.settings(),
                  on: false, toggles: false,
                  enabled: true, action: () => {
                    this.show(SURFACE_NONE);
                    KeyboardSession.shared.openSettings();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '繁', icon: null, title: '繁体输出', on: this.traditional, toggles: true,
                  enabled: !this.japanese, action: () => {
                    this.toggleTraditional();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '', icon: $r('app.media.tool_emoji'), title: '表情', on: false,
                  toggles: false, enabled: true, action: () => {
                    this.show(SURFACE_NONE);
                    this.openEmojiSurface();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '\u2318', icon: null, title: '符号', on: this.surface === SURFACE_SYMBOLS,
                  toggles: false, enabled: true, action: () => {
                    this.show(SURFACE_NONE);
                    this.openSymbolSurface();
                  }
                });
              }
              if (this.clipboardEnabled) {
                GridItem() {
                  this.toolTile({
                    glyph: '', icon: $r('app.media.tool_clipboard'), title: '剪贴板', on: false,
                    toggles: false, enabled: true, action: () => {
                      this.show(SURFACE_NONE);
                      this.openClipboard();
                    }
                  });
                }
              }
              GridItem() {
                this.toolTile({
                  glyph: '123', icon: null, title: '本地输入', on: false, toggles: false,
                  enabled: true, action: () => {
                    this.show(SURFACE_MODES);
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '🎙', icon: null, title: '语音输入', on: this.voiceBusy, toggles: false,
                  enabled: this.voiceEnabled, action: () => {
                    this.show(SURFACE_VOICE);
                  }
                });
              }
              GridItem() {
                // Offered only when a polish provider is configured. The rewrite goes to the user's own service, and a tile that always failed would be a tile that taught the user to ignore this panel.
                this.toolTile({
                  glyph: '\u2728', icon: null, title: 'AI 润色', on: this.polishBusy, toggles: false,
                  enabled: this.polishConfigured, action: () => {
                    this.openPolish();
                  }
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '', icon: $r('app.media.tool_sound'), title: '按键音', on: this.feedback.sound,
                  toggles: true, enabled: true, action: () => this.setFeedback({
                    sound: !this.feedback.sound, haptics: this.feedback.haptics,
                    strength: this.feedback.strength, glideTyping: this.feedback.glideTyping
                  })
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '', icon: $r('app.media.tool_vibration'), title: '按键振动',
                  on: this.feedback.haptics, toggles: true, enabled: true, action: () => this.setFeedback({
                    sound: this.feedback.sound, haptics: !this.feedback.haptics,
                    strength: this.feedback.strength, glideTyping: this.feedback.glideTyping
                  })
                });
              }
              GridItem() {
                // One tile that shows the level and cycles it: three would take a row of their own.
                this.toolTile({
                  glyph: KeyboardFeedback.title(this.feedback.strength), icon: null, title: '振动强度',
                  on: false, toggles: false, enabled: this.feedback.haptics, action: () => this.setFeedback({
                    sound: this.feedback.sound, haptics: this.feedback.haptics,
                    strength: KeyboardFeedback.nextStrength(this.feedback.strength),
                    glideTyping: this.feedback.glideTyping
                  })
                });
              }
              GridItem() {
                this.toolTile({
                  glyph: '\u2194', icon: null, title: '键盘大小', label: KeyAccessibilityPolicy.geometry(),
                  on: false, toggles: false,
                  enabled: true, action: () => {
                    this.show(SURFACE_GEOMETRY);
                  }
                });
              }
            }
            .columnsTemplate('1fr 1fr 1fr 1fr')
            .rowsGap(TILE_ROW_GAP_VP)
            .columnsGap(TILE_COLUMN_GAP_VP)
            .padding({ left: TILE_PANEL_PADDING_VP, right: TILE_PANEL_PADDING_VP })
            .width('100%')
          }
        }
        .width('100%')
      }
      .scrollBar(BarState.Off)
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** One tile: a 52vp square in the key colour, or the accent tint while its switch is on, over a 12px name. */
  @Builder
  toolTile($$: ToolTileOptions) {
    Column({ space: 4 }) {
      Column() {
        if ($$.icon !== null) {
          Image($$.icon as Resource)
            .width(TILE_ICON_VP)
            .height(TILE_ICON_VP)
            .fillColor($$.on ? this.skin.toggleForeground : this.skin.keyForeground)
        } else {
          Text($$.glyph)
            .fontSize(TILE_GLYPH_FONT_SIZE)
            .fontWeight(600)
            .fontColor($$.on ? this.skin.toggleForeground : this.skin.keyForeground)
            .maxLines(1)
        }
      }
      .width(TILE_VP)
      .height(TILE_VP)
      .justifyContent(FlexAlign.Center)
      .borderRadius(TILE_RADIUS_VP)
      .backgroundColor($$.on ? this.skin.toggleBackground : this.skin.keySurfaceBackground(false))
      Text($$.title)
        .fontSize(TILE_LABEL_FONT_SIZE)
        .fontColor(this.skin.keyForeground)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .alignItems(HorizontalAlign.Center)
    .opacity($$.enabled ? 1 : 0.4)
    .accessibilityText(KeyAccessibilityPolicy.tile($$.title, $$.label, $$.on, $$.toggles))
    .onClick(() => {
      if ($$.enabled) {
        $$.action();
      }
    })
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

  /** The clipboard history: capture, pin, insert, and a clear that asks once before it happens. */
  @Builder
  clipboardFace() {
    Column() {
      Row() {
        Text('剪贴板历史')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .layoutWeight(1)
        // Saving and clearing act on this device's history, so the cloud section does not offer them.
        if (!this.cloudTabActive()) {
          Text('保存当前')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .padding({ right: 12 })
            .onClick(() => this.captureClipboard())
          Text(this.confirmingClear ? '确认清空' : '清空')
            .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
            .fontColor(this.skin.accent)
            .padding({ right: 12 })
            .onClick(() => this.clearClipboard())
        }
        Text('返回')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.skin.accent)
          .onClick(() => {
            this.show(SURFACE_NONE);
          })
      }
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 4)
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })

      this.clipboardEntries()
    }
    .width('100%')
    .height(this.surfaceHeight())
  }

  /** The section tabs, the notice and the entries, shared by the phone's clipboard surface and the 2in1 emoji surface's clipboard tab. */
  @Builder
  clipboardEntries() {
      // 本机 and 云端. A password field gets no tabs at all: the cloud section is the only other one, and it is never offered there.
      if (this.cloudVisible) {
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

        List({ space: KeyboardMetrics.KEY_SPACING_VP, scroller: this.clipListScroller }) {
          ForEach(this.visibleCloudClips(), (entry: CloudClipboardItem, index: number) => {
            ListItem() {
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
          }, (entry: CloudClipboardItem, index: number): string => `${entry.id}:${index}`)
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

        // A long press on an entry offers it to the cloud clipboard here, in the panel: bindContextMenu does not show inside an input method panel.
        if (this.clipSendTarget.length > 0 && this.cloudVisible) {
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
                this.clipSendTarget = '';
              })
          }
          .width('100%')
          .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
            right: KeyboardMetrics.CANDIDATE_PADDING_VP, top: 4, bottom: 4 })
        }

        // A List rather than a Column in a Scroll so the 2in1 arrows can bring the selected entry into view.
        List({ space: KeyboardMetrics.KEY_SPACING_VP, scroller: this.clipListScroller }) {
            ForEach(this.visibleClips(), (entry: ClipboardHistoryItem, index: number) => {
              ListItem() {
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
            })
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

  /** The emoji grid, with the Unicode categories down the top and the group's entries below. */
  @Builder
  emojiFace() {
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
          this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, false)
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

  /** The full categorized symbol keyboard; unlike emoji symbols it returns after one tap by default. */
  @Builder
  symbolPanelFace() {
    Column() {
      Scroll() {
        Row({ space: KeyboardMetrics.KEY_SPACING_VP }) {
          ForEach(SymbolPanelPolicy.categories(), (category: SymbolPanelCategory, index: number) => {
            Text(category.title)
              .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
              .fontColor(index === this.symbolCategory ? this.skin.accent : this.skin.keyForeground)
              .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
                right: KeyboardMetrics.CANDIDATE_PADDING_VP })
              .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)
              .accessibilityText(`${category.title}符号`)
              .onClick(() => this.symbolCategory = index)
          }, (category: SymbolPanelCategory) => category.id)
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .scrollBar(BarState.Off)
      .width('100%')
      .height(KeyboardMetrics.ROW_HEIGHT_VP * 3 / 5)

      Grid() {
        ForEach(SymbolPanelPolicy.category(this.symbolCategory).symbols
          .slice(0, SymbolPanelPolicy.maxSymbolsPerCategory()), (symbol: string) => {
          GridItem() {
            Text(symbol)
              .fontSize(KeyboardMetrics.CANDIDATE_FONT_SIZE + 2)
              .fontColor(this.skin.keyForeground)
              .textAlign(TextAlign.Center)
              .width('100%')
              .accessibilityText(symbol)
              .onClick(() => this.selectSymbol(symbol))
          }
          .height(46)
          .backgroundColor(this.skin.keyBackground)
          .borderRadius(this.skin.cornerRadius)
        }, (symbol: string, index: number) => `${this.symbolCategory}:${index}:${symbol}`)
      }
      .columnsTemplate('1fr '.repeat(5).trim())
      .columnsGap(1)
      .rowsGap(1)
      .width('100%')
      .layoutWeight(1)

      Row() {
        Text('返回')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.accent)
          .accessibilityText('返回键盘')
          .onClick(() => this.show(SURFACE_NONE))
        Blank()
        Text(this.symbolLocked ? '连续输入：开' : '连续输入：关')
          .fontSize(KeyboardMetrics.LANGUAGE_FONT_SIZE)
          .fontColor(this.symbolLocked ? this.skin.accent : this.skin.keyForeground)
          .accessibilityText(this.symbolLocked ? '关闭连续输入' : '开启连续输入')
          .onClick(() => this.symbolLocked = !this.symbolLocked)
        Blank()
        Text('删除')
          .fontSize(KeyboardMetrics.SMALL_KEY_FONT_SIZE)
          .fontColor(this.skin.keyForeground)
          .accessibilityText(KeyAccessibilityPolicy.delete())
          .onClick(() => KeyboardSession.shared.backspace())
      }
      .width('100%')
      .height(this.rowHeight(3))
      .padding({ left: KeyboardMetrics.CANDIDATE_PADDING_VP,
        right: KeyboardMetrics.CANDIDATE_PADDING_VP })
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
          left: KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP,
          right: KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP,
          top: KeyboardMetrics.ROOT_VERTICAL_PADDING_VP
            + KeyboardMetrics.compositionRowHeightVp(this.settings.candidatePreeditFontSize)
            + this.candidateRowHeight() + KeyboardMetrics.ROW_SPACING_VP
        })
      }
    }
    .width('100%')
    .onAreaChange((_old: Area, now: Area): void => {
      this.glideOriginX = Number(now.globalPosition.x ?? 0);
      this.glideOriginY = Number(now.globalPosition.y ?? 0);
    })
  }

  @Builder
  keyboard() {
    Column({ space: this.rowSpacing() }) {
      this.candidateStrip()

      // Where the machine has its own keys the strip is the whole panel, unless the toolbar asked
      // for one of the surfaces a phone would have reached from a key face. Drawing a second set of
      // keys under a physical keyboard uninvited would only take the screen away from what is being
      // written; drawing them when they were asked for is the whole point of the button.
      if (!KeyboardFormFactor.isDesktop()) {
        this.keyFaces()
      } else if (this.desktop === DesktopSurface.EMOJI) {
        this.emojiFace()
      } else if (this.desktop === DesktopSurface.SCREEN_KEYBOARD) {
        this.keyFaces()
      } else if (this.desktop === DesktopSurface.VOICE) {
        this.voiceFace()
      } else if (this.desktop === DesktopSurface.HANDWRITING) {
        // The pad without the action row: the physical keys already carry space, return and the language switch.
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

  /**
   * Everything below the strip: whichever surface is open, or the keys themselves.
   *
   * Separate from keyboard() because it is drawn only where the machine has no keys of its own.
   */
  @Builder
  keyFaces() {
      if (this.surface === SURFACE_GEOMETRY) {
        Stack() {
          this.typingFace()
          this.geometryFace()
        }
        .width('100%')
        .height(this.surfaceHeight())
      } else if (this.isTools()) {
        this.toolsFace()
      } else if (this.surface === SURFACE_EXPANDED) {
        this.expandedFace()
      } else if (this.surface === SURFACE_CLIPBOARD) {
        this.clipboardFace()
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
      } else if (this.surface === SURFACE_NINE_KEY_HOLD) {
        this.nineKeyHoldFace()
      } else if (this.japanese && this.surface === SURFACE_VARIANTS && !this.symbols) {
        this.variantFace()
      } else {
        this.typingFace()
      }
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
          ForEach(this.symbols ? SYMBOL_ROWS[1] : LETTER_ROWS[1], (key: string) => {
            this.flexibleKey(this.symbols ? this.symbolKey(key) : this.letterKey(key));
          }, (key: string) => key + this.faceKey())
          if (!this.symbols && !this.koreanFace() && !this.vietnameseFace() && !this.tibetanFace()) {
            // 微软双拼用分号拼 ing，所以这个键要能在字母行里直接按到，而不是藏在符号层后面。两笔式在那里没有字母，越南语和威利转写也不用它，所以韩语、越南语和藏文把这一行画成九个键。
            this.flexibleKey(this.letterKey(';'));
          }
        }
        .width('100%')
        .height(this.rowHeight(1))
        .onTouch((event: TouchEvent): void => this.glideTouch(event))

        Row({ space: this.keySpacing() }) {
          if (!this.symbols) {
            this.fixedKey({
              face: this.caseMode === LetterCaseMode.CAPS_LOCK ? '⇪' : '⇧',
              label: this.letterCase.accessibilityLabel(this.english),
              size: KeyboardMetrics.KEY_FONT_SIZE,
              emphasized: this.caseMode !== LetterCaseMode.LOWERCASE,
              special: true,
              width: KeyboardMetrics.MODIFIER_WIDTH_VP,
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
          this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true);
        }
        .width('100%')
        .height(this.rowHeight(2))
        .onTouch((event: TouchEvent): void => this.glideTouch(event))

        this.actionRow()
      }
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
      this.deleteKey(KeyboardMetrics.MODIFIER_WIDTH_VP, true);
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
        this.deleteKey('100%', true)
      }
      .width(`${NINE_KEY_COLUMN_PERCENT}%`)
      .height('100%')
    }
    .width('100%')
    .height(this.gridHeight())
  }
}
