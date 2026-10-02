import inputMethodEngine from '@ohos.inputMethodEngine';
import fs from '@ohos.file.fs';
import client from 'libmsimeclient.so';
import { utf8Length } from './Utf8';
import { TextPolicy } from './TextPolicy';
import { KeyboardLog, describeError } from './KeyboardLog';
import { DictionaryMaintenancePolicy } from './DictionaryMaintenancePolicy';
import { TypingStatisticsPolicy } from './TypingStatisticsPolicy';
import { KeyPressBatch, KeyPressFlush } from './KeyIdPolicy';
import { readTypingStatisticsEnabled, writeKeyPresses } from './TypingStatisticsWriter';
import taskpool from '@ohos.taskpool';
import { KeyboardSkin } from './skin/KeyboardSkin';
import { CustomKeyboardSkin, CustomSkinDocument } from './skin/CustomKeyboardSkin';
import {
  CandidateColors, CustomThemeDocument, GlobalTheme, ResolvedTheme, ResolveThemeRequest, ThemeCatalogEntry
} from './skin/GlobalTheme';
import { KeyboardFormFactor } from './KeyboardFormFactor';
import { KeyboardScheme, PreferenceMapping, SchemeDefinition } from './KeyboardScheme';
import { EditorPolicy, EditorTraits } from './input/EditorPolicy';
import { JapaneseSpacePolicy } from './input/JapaneseSpacePolicy';
import { KoreanCompositionPolicy } from './input/KoreanCompositionPolicy';
import { SchemeCompositionPolicy } from './input/SchemeCompositionPolicy';
import { SchemeTraits } from './SchemeTraits';
import {
  CapitalizationMode, EnglishCapitalizationPolicy
} from './input/EnglishCapitalizationPolicy';
import { WubiCodeHintPolicy } from './input/WubiCodeHintPolicy';
import { KeyboardGeometry } from './KeyboardGeometry';
import { FullWidthInputPolicy } from './input/FullWidthInputPolicy';
import { ChineseHelpcodePolicy } from './input/ChineseHelpcodePolicy';
import { ChineseOutputPolicy } from './input/ChineseOutputPolicy';
import { KeyboardFeedback, FeedbackSettings, HapticStrength } from './KeyboardFeedback';
import { HardwareKeyRouter, HardwareNavigationPreferences, HardwareSpelling } from './HardwareKeyRouter';
import vibrator from '@ohos.vibrator';
import common from '@ohos.app.ability.common';
import { InputDiagnosticPolicy } from './input/InputDiagnosticPolicy';
import { InputModeHudPolicy } from './InputModeHudPolicy';
import { HardwareKeyTarget } from './HardwareKeyDispatch';
import { EmojiPanelKey } from './emoji/EmojiPanelKeyPolicy';
import { DEFAULT_VOICE_HOTKEY_BINDINGS, VoiceHotkeyAction, VoiceHotkeyBindings, VoiceHotkeyPolicy,
  VoiceKey } from './input/VoiceHotkeyPolicy';
import { HarmonyVoiceRecordingBehaviour } from './input/HarmonyVoiceRecordingBehaviour';
import { KeySoundPlayer } from './KeySoundPlayer';
import { KEY_SOUNDS_OFF, KeySoundPolicy, PluginPreferenceDocument } from './KeySoundPolicy';
import { MusicPlayer } from './MusicPlayer';
import { MUSIC_OFF, MusicPolicy } from './MusicPolicy';
import {
  ResolvedTypingEffectDocument, TYPING_EFFECT_COMMIT, TYPING_EFFECTS_OFF, TypingEffect, TypingEffectPolicy,
  TypingEffectSettings
} from './TypingEffectPolicy';
import { EnglishCompletions, EnglishReplacement, EnglishSuggestionPolicy }
  from './input/EnglishSuggestionPolicy';
import { ImeModeScopePolicy } from './input/ImeModeScopePolicy';
import { ModeBindings, DEFAULT_MODE_BINDINGS } from './InputModeRouting';
import { FloatingToolbarLayout, ToolbarButton, ToolbarComponents, ToolbarState }
  from './FloatingToolbarLayout';
import { ToolbarSkin, ToolbarSkinPolicy } from './ToolbarSkinPolicy';
import { DesktopSurface } from './DesktopSurface';
import { CandidateAnnotationPreferencePolicy } from './candidate/CandidateAnnotationPreferencePolicy';
import { CandidateManagementAction } from './candidate/CandidateManagementAction';
import { CandidateGlossPolicy } from './candidate/CandidateGlossPolicy';
import {
  CandidateGlossLayoutPolicy, CandidateGlossProviderState
} from './candidate/CandidateGlossLayoutPolicy';
import { CandidatePresentationPolicy } from './candidate/CandidatePresentationPolicy';
import {
  CandidateSkinCatalogPolicy, CandidateSkinPackage, CandidateSkinDecoration, CandidateSkinBackground,
  CandidateSkinAlign, CandidateSkinFit
} from './candidate/CandidateSkinCatalogPolicy';
import { CandidateWidthPolicy } from './candidate/CandidateWidthPolicy';
import { CandidatePagerPolicy } from './candidate/CandidatePagerPolicy';
import { CandidateLogoPolicy } from './candidate/CandidateLogoPolicy';
import { CandidateNumberFontPolicy } from './candidate/CandidateNumberFontPolicy';
import { KeyboardMetrics } from './KeyboardMetrics';
import { OnlineCandidatePolicy, OnlineQuery } from './candidate/OnlineCandidatePolicy';
import {
  TranslationPolicy, TranslationQuery, TranslationPlanItem, TranslationEntry,
  TranslationProviderConfig, TencentTranslationConfig, NiuTransTranslationConfig, TranslationCandidate
} from './candidate/TranslationPolicy';
import { TranslationSensePolicy } from './candidate/TranslationSensePolicy';
import pasteboard from '@ohos.pasteboard';
import abilityAccessCtrl from '@ohos.abilityAccessCtrl';
import bundleManager from '@ohos.bundle.bundleManager';
import http from '@ohos.net.http';
import {
  ClipboardHistoryStore, ClipboardHistoryItem, ClipboardHistoryError, ClipboardFailure
} from './clipboard/ClipboardHistoryStore';
import { ClipboardHistoryPreferencePolicy } from './clipboard/ClipboardHistoryPreferencePolicy';
import { CloudClipboardClient } from './clipboard/CloudClipboardClient';
import {
  CloudClipboardListing, CloudClipboardPolicy, CloudClipboardSendOutcome
} from './clipboard/CloudClipboardPolicy';
import {
  DEFAULT_VOICE_INPUT_CONFIGURATION, VoiceInputConfiguration, VoiceInputConfigurationPolicy
} from './input/VoiceInputConfiguration';
import {
  AiPolishPolicy, MAX_POLISH_SOURCE_CHARACTERS, PolishReplacement
} from './input/AiPolishPolicy';
import { HarmonyVoicePolisher } from './input/HarmonyVoicePolisher';
import { SmartPunctuationSpacePolicy, SmartPunctuationSpaceSnapshot, SpaceConvertDecision }
  from './input/SmartPunctuationSpacePolicy';
import { SmartPunctuationContext } from './input/SmartPunctuationContext';
import { PairedPunctuationPolicy, PairedPunctuationCompletion } from
  './input/PairedPunctuationPolicy';
import { CandidateTextPolicy, CandidateTextEdge } from './input/CandidateTextPolicy';
import { ReplyKeyboardPolicy, ReplyRequest } from './ReplyKeyboardPolicy';
import { ReplyContextPolicy } from './ReplyContextPolicy';
import {
  CommunityReplyLibraryPolicy, CommunityReplyTemplate
} from './CommunityReplyLibraryPolicy';
import {
  EmojiCatalogModel, EMOJI_RECENTS_MAX_BYTES, EmojiSymbolGroup,
  normalizeGroups, normalizeSymbolGroups
} from './emoji/EmojiCatalogModel';
import {
  PluginSymbolGroup, PluginSymbolGroupDocument, PluginSymbolGroupPolicy
} from './emoji/PluginSymbolGroupPolicy';
import {
  SmartPunctuationRepeatPolicy, SmartPunctuationRepeatSnapshot
} from './input/SmartPunctuationRepeatPolicy';
import { BackspaceHoldPolicy } from './input/BackspaceHoldPolicy';
import { InlinePreeditPolicy, InlinePreeditStyle } from './input/InlinePreeditPolicy';
import {
  CompositionBoundary, CompositionBoundaryAction, CompositionBoundaryPolicy
} from './input/CompositionBoundaryPolicy';

// Shapes of the JSON the shared C ABI answers with. Only the fields this minimal surface reads are
// declared; the responses carry considerably more.
interface Candidate {
  text: string;
  highlighted: boolean;
  /** Which part of the Engine produced it; the gloss lookup treats sources differently. */
  source: number;
  /** Zero when unpinned, otherwise the fixed candidate slot (1..5). */
  fixed_position: number;
  /** The Engine input code that produced this candidate, used for scheme-specific hints. */
  code: string;
  /** An Engine-derived display suffix, never part of what gets committed. */
  annotation: string;
  /** Optional translation attached by the shared Engine for this generation. */
  translation?: string | null;
}

/** A candidate as the view draws it: the text, and whatever is printed beside it. */
export interface CandidateEntry {
  readonly text: string;
  readonly hint: string;
  readonly annotation: string;
  /** The annotation slot is a translation rather than an Engine annotation; they are styled
   * differently because the source styles only one of them. */
  readonly annotationIsTranslation: boolean;
  /** A Korean Hanja row's 훈음, drawn on the gloss line whatever the translation switches say and never committed; "" for any other row. */
  readonly hunEum: string;
  /** Engine-selected candidate; selection is not necessarily the first row. */
  readonly highlighted: boolean;
  readonly badge: string;
  readonly fixedPosition: number;
  readonly source: number;
  /** Translation attached to this exact Engine candidate, when one has been resolved. */
  readonly translation: string | null;
}

interface EngineView {
  candidates: Candidate[];
  editing_text: string;
  /**
   * 已经选中的那一段，运行时替这个宿主留在组字里而不是立刻上屏。
   *
   * 候选只吃掉部分输入时，引擎会继续组字并把选中的那一段交回来。留住它的宿主必须把它画出来：
   * 请求了却不画，用户已经选中的字既不在文档里也不在屏幕上，而组字在他取消之前不会结束
   * （scripts/test-phrase-preedit-hosts.py 守的就是这一半状态）。
   *
   * Absent rather than empty when nothing is held: the runtime skips serializing an empty prefix, so reading it bare throws on every ordinary keystroke.
   */
  phrase_prefix?: string;
  /** Byte offset into `editing_text`, which for this ASCII text is also the character index. */
  caret_position: number;
  /** The candidate page on screen, from zero, and how many pages the whole list makes. */
  page: number;
  page_count: number;
  generation: number;
  /** Which spellings the digits typed so far could be, offered while the nine-key grid is up. */
  nine_key_spellings: string[];
  /**
   * What the Engine is committing its own output as.
   *
   * 'Fullwidth' or 'Halfwidth', capitalised: this is the Rust enum's own name, not the lowercase
   * spelling the preference document uses for the same idea. Comparing it against the preference
   * vocabulary matches nothing, which is a toggle that reports success and changes no face.
   */
  character_width: string;
  /** What has been typed so far, which is the prefix a Wubi code hint is measured against. */
  preedit: string;
  /** True when the answer came from pinyin rather than the scheme's own code. */
  answered_by_pinyin_fallback: boolean;
  /** The Engine's own numbering, which the ported display policies are written against. */
  scheme: number;
  shuangpin_profile: string;
  /** Whether `;` can be the `ing` final, which only the Microsoft layout has. */
  microsoft_shuangpin?: boolean;
  /** 'none' unless a local utility mode is consuming the keys. */
  local_mode: string;
  /** The non-letter keys the Engine takes as input now: the active local mode's digits and operators, or with nothing composed `/` and `@` for the modes they open. Absent from a runtime older than those modes. */
  spelling_symbols?: string;
  dedicated_english: boolean;
  /** The scheme's openable candidate list is showing: the Korean Hanja list, the Zhuyin list. Absent from a runtime older than the flag. */
  candidate_list_open?: boolean;
}

// The stored preferences, read and written whole. Only the fields the keyboard changes are named;
// the rest travel through untouched as a parsed value, because rewriting a document one does not
// fully understand is how settings get silently dropped.
interface PreferencesSnapshot {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
}

interface SnapshotResponse {
  ok: boolean;
  value: PreferencesSnapshot;
  error: string;
}

interface TypingStatisticsRecordAction {
  operation: string;
  text: string;
  source: string;
  day: string;
  hour: number;
}

interface TypingStatisticsRecordRequest {
  directory: string;
  action: TypingStatisticsRecordAction;
}

interface TypingStatisticsRecordValue {
  /** Characters counted from this text. */
  recorded: number;
  /** The commit count just passed, while achievements are switched on; otherwise null. */
  milestone?: number | null;
}

interface TypingStatisticsRecordResponse {
  ok: boolean;
  value?: TypingStatisticsRecordValue;
  error?: string;
}

interface TypingStatisticsKeysAction {
  operation: string;
  day: string;
  keys: Record<string, number>;
}

interface TypingStatisticsKeysRequest {
  directory: string;
  action: TypingStatisticsKeysAction;
}

interface MobileClipboardAction {
  operation: string;
  text?: string;
  pinned?: boolean;
}

interface MobileClipboardRequest {
  directory: string;
  legacy: string;
  action: MobileClipboardAction;
}

interface NativeClipboardEntry {
  text: string;
  timestampMs: number;
  pinned: boolean;
}

interface MobileClipboardValue {
  entries: NativeClipboardEntry[];
  captured?: boolean;
  reason?: string | null;
  updated?: boolean;
  removed?: boolean;
  cleared?: boolean;
}

interface MobileClipboardReply {
  ok: boolean;
  value: MobileClipboardValue;
  error: string;
}

export interface ClipboardHistoryMutation {
  readonly entries: ClipboardHistoryItem[];
  readonly failure: ClipboardFailure | null;
}

// The local emoji catalog lives in others.db beside the dictionaries, so the query carries the
// resource directory rather than going through a session.
/** `{prefix,limit}` as the shared ABI reads it; the prefix is ASCII letters and the limit 1..32. */
interface EnglishCompletionRequest {
  prefix: string;
  limit: number;
}

interface EmojiQuery {
  category: string;
  limit: number;
  offset: number;
  group: string;
  cursor: boolean;
  list_groups: boolean;
  list_symbol_groups: boolean;
  parent: string;
  search: string;
  // 只有列符号集插件组时才带插件目录；其余请求传 null，native 按未提供处理。
  list_plugin_symbol_groups: boolean;
  plugins: string | null;
}

export interface EmojiRow {
  text: string;
  annotation: string;
  group: string;
}

export interface EmojiPage {
  items: EmojiRow[];
  next_offset: number;
  complete: boolean;
}

interface EmojiResponse {
  ok: boolean;
  value: EmojiPage;
  error: string;
}

interface EmojiGroups {
  groups: string[];
}

interface EmojiGroupsResponse {
  ok: boolean;
  value: EmojiGroups;
  error: string;
}

interface EmojiSymbolGroups {
  symbol_groups: EmojiSymbolGroup[];
}

interface EmojiSymbolGroupsResponse {
  ok: boolean;
  value: EmojiSymbolGroups;
  error: string;
}

interface PluginSymbolGroups {
  plugin_symbol_groups: PluginSymbolGroupDocument[];
}

interface PluginSymbolGroupsResponse {
  ok: boolean;
  value: PluginSymbolGroups;
  error: string;
}

/**
 * One row of the expanded panel: the word and whatever is written under it.
 *
 * Deliberately narrower than CandidateEntry. The panel draws a text and a gloss and selects through
 * selectAnyCandidate, which takes a position in this list; the fixed slot and the source belong to
 * the page the management operations act on, and carrying them here would invite indexing one list
 * with the other's number.
 */
export interface ExpandedCandidate {
  readonly text: string;
  readonly annotation: string;
  readonly annotationIsTranslation: boolean;
  /** As CandidateEntry.hunEum. */
  readonly hunEum: string;
}

interface CandidateSnapshot {
  generation: number;
  candidates: Candidate[];
}

interface SnapshotCandidates {
  ok: boolean;
  value: CandidateSnapshot;
  error: string;
}

interface NativeReply<T> {
  ok: boolean;
  value: T;
  error: string;
}

/** What the shared ABI is asked to turn into a provider request. */
interface AiHttpRequestInput {
  segmented_pinyin: string[];
  context: string;
  candidate_limit: number;
}

interface AiHttpRequest {
  config: PreparedAiAssistant;
  input: AiHttpRequestInput;
}

interface AiHttpDescriptor {
  url: string;
  method: string;
  headers: Record<string, string>;
  body: Object;
  timeout_ms: number;
  connect_timeout_ms: number;
  max_response_bytes: number;
}

interface PreparedAiAssistant {
  enabled: boolean;
  provider: string;
  model: string;
  token: string;
  tokens: Record<string, string>;
  endpoint: string;
  candidate_limit: number;
  prompt_id: string;
  prompt: string;
  prompt_custom_1: string;
  prompt_custom_2: string;
  prompt_custom_3: string;
}

interface TranslationHttpDescriptor {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: Object;
  body_utf8?: string;
  timeout_ms: number;
  max_response_bytes: number;
  expected_count?: number;
}

interface TranslationFetchResult {
  current: boolean;
  providerComplete: boolean;
}

interface TranslationProviderResult {
  value: string | null;
  completed: boolean;
}

interface TranslationResponseValue {
  generation: number;
  translations: TranslationEntry[];
}

interface GlossRequestCandidate {
  text: string;
  source: number;
}

interface GlossRequestDocument {
  generation: number;
  user_data: string;
  candidates: GlossRequestCandidate[];
  target_language?: string;
}

interface TranslationPlanDocument {
  target_language: string;
  candidates: GlossRequestCandidate[];
}

interface TencentTranslationRequest {
  config: TencentTranslationConfig;
  texts: string[];
  source_language: string;
  target_language: string;
  timestamp: number;
}

interface NiuTransTranslationRequest {
  config: NiuTransTranslationConfig;
  text: string;
  source_language: string;
  target_language: string;
  timestamp: string;
}

interface CustomTranslationRequest {
  config: TranslationProviderConfig;
  text: string;
  source_language: string;
  target_language: string;
}

interface TranslationGlossSaveRequest {
  target_language: string;
  translations: TranslationEntry[];
}

interface OnlineApplyValue {
  applied: boolean;
  view: EngineView;
}

/** What the view draws. Grouped so a new field does not change every signature on the way there. */
export interface KeyboardState {
  readonly candidates: CandidateEntry[];
  readonly editing: string;
  readonly editingCaret: number;
  /** The already-chosen part of a phrase leading `editing`, which the preedit style never hides. */
  readonly phrasePrefix: string;
  readonly spellings: string[];
  readonly scheme: number;
  readonly shuangpinProfile: string;
  readonly localMode: string;
  readonly english: boolean;
  readonly diagnostic: string;
  /** The candidate page on screen, from zero, and how many there are; a translation page is always the one page of one. */
  readonly page: number;
  readonly pageCount: number;
  /** Whether the scheme's openable candidate list (the Korean Hanja list, the Zhuyin list) is on the strip; see `candidateListOpen`. */
  readonly candidateListOpen: boolean;
}

interface EngineValue {
  /** False means the platform editor, rather than the Engine composition, owns the command. */
  handled: boolean;
  // The Rust side sends Option<String>, but an absent commit reaches ArkTS as undefined rather than
  // the null the JSON names, so both have to be spelled out here. Treating one and not the other
  // threw on every focus, which tore the session down before a single key could be pressed.
  commit: string | null | undefined;
  /** What the commit came from; present with a commit. */
  commit_context?: CommitContext | null;
  /** Something the Engine wants the user told, such as a dictionary that could not be opened. */
  diagnostic: string | null | undefined;
  view: EngineView;
}

interface CommitContext {
  /** False for text the expression, command and mention modes generated, which the user did not type out and typing statistics do not count. */
  typing_statistics: boolean;
}

interface CreateValue {
  session: number;
}

interface Response {
  ok: boolean;
  value: EngineValue;
  error: string;
}

// Not every entry point answers with a transition. setEnglishMode and its siblings return the view on
// its own, with no commit to apply and no wrapper around it, so they are read through their own shape
// rather than pushed through the transition one where the missing fields would throw.
interface ViewResponse {
  ok: boolean;
  value: EngineView;
  error: string;
}

interface CreateResponse {
  ok: boolean;
  value: CreateValue;
  error: string;
}

interface VoiceGenerationResponse {
  ok: boolean;
  value: number;
  error: string;
}

interface VoiceTextResponse {
  ok: boolean;
  value: string | null;
  error: string;
}

interface SkinCatalogValue {
  packages: CandidateSkinPackage[];
}

interface SkinCatalogResponse {
  ok: boolean;
  value: SkinCatalogValue;
  error: string;
}

/** One file inside an external skin package, named the way the shared ABI names it. */
interface SkinResourceRequest {
  directory: string;
  id: string;
  relative: string;
  kind: string;
}

interface SkinResourceValue {
  contentType: string;
  bytes: number[];
}

/** A candidate decoration image, already encoded for the component that draws it. */
interface CandidateSkinImage {
  image: string | null;
  aspectRatio: number;
}

/** What every path that has no decoration to draw returns, rather than a fresh literal each time. */
const NO_CANDIDATE_SKIN_IMAGE: CandidateSkinImage = { image: null, aspectRatio: 1 };

// Enough for the sentence rule to walk back over trailing spaces and closing quotes to the mark that
// ended the last sentence. A caret further than this from one is in the middle of a sentence by any
// reading, and the call is made on every text change, so it stays bounded.
const MAX_CAPITALIZATION_CONTEXT_CHARACTERS: number = 64;
// setPreviewText's range for "whatever preview text is shown now, or the caret when there is none".
const PREVIEW_REPLACE_CURRENT: inputMethodEngine.Range = { start: -1, end: -1 };

interface SkinResourceResponse {
  ok: boolean;
  value: SkinResourceValue;
  error: string;
}

interface SkinToolbarStylesheetResponse {
  ok: boolean;
  value: string | null;
  error: string;
}

// ArkTS rejects untyped object literals, so the request prepareHost takes gets a shape too.
interface PrepareRequest {
  resources: string;
  state_root: string;
}

interface SnapshotQueueProcessAction {
  operation: string;
  directory: string;
  staging_root: string;
  options: Object;
}

interface SnapshotQueueProcessReply {
  ok: boolean;
  error?: string;
}

interface SnapshotQueueRequestState {
  status: string;
}

interface SnapshotQueueStateValue {
  request?: SnapshotQueueRequestState | null;
}

interface SnapshotQueueStateReply {
  ok: boolean;
  value?: SnapshotQueueStateValue;
  error?: string;
}

interface SnapshotQueueStateAction {
  operation: string;
  directory: string;
  options: Object;
  acknowledge: boolean;
}

// Only the fields the keyboard acts on are declared. The rest of the document is forwarded to create
// untouched, so naming it here would only invite the two shapes to drift apart.
interface PreparedPreferences {
  default_ime_mode: string;
  /** `app` remembers the mode per application; `global` shares one across all of them. */
  ime_mode_scope?: string;
  scheme: string;
  /** Optional in the shared document; absent means the Wubi remaining-code hint stays on. */
  wubi_code_hint?: boolean;
  /** Whether the mode badge is drawn after a Chinese/English switch. Defaults to on. */
  input_mode_hud?: boolean;
  /** Read-only English word completions while typing directly in English. Defaults to on. */
  english_suggestions?: boolean;
  shuangpin_profile: string;
  touch_keyboard_layout: string;
  /** The shared global theme id; the ids and their palettes come from the theme catalog. */
  global_theme: string;
  /** What the `custom` theme is made of. Absent means the shared default: over `system`, with nothing set. */
  custom_theme?: CustomThemeDocument;
  candidate_theme: string;
  candidate_selected_bar: boolean | null;
  candidate_follow_cursor: boolean;
  number_row_selection: boolean;
  navigation: PreparedNavigationPreferences;
  candidate_layout: string;
  candidate_preedit_style: string;
  /** Inline spelling in the document; the 2in1 draws it as editor preview text. */
  tsf_preedit_style?: string;
  candidate_font_size: number;
  candidate_preedit_font_size: number;
  /** 50..100; absent in documents that never changed it, which means an opaque card. */
  candidate_opacity_percent?: number;
  /** 0..32 points; absent or null leaves the card to the skin package, then the host. */
  candidate_corner_radius?: number | null;
  candidate_font_family: string;
  /** Named first so Latin comes from it and Han falls through; absent means one family answers. */
  candidate_english_font?: string | null;
  candidate_fallback_fonts: string[];
  // 'follow', 'dark' or 'light' for the keyboard specifically; theme is the app mode it falls back
  // to, and only if both defer does the system decide. A global theme with a fixed appearance
  // overrides all of them.
  screen_keyboard_theme: string;
  /** 'follow', 'dark' or 'light' for the floating toolbar; it falls back to the app mode. */
  toolbar_theme?: string;
  /** 'follow', 'dark' or 'light' for the voice panel; it falls back to the app mode. */
  voice_theme?: string;
  /** 'follow', 'dark' or 'light' for the emoji panel; it falls back to the app mode. */
  emoji_theme?: string;
  /** 'follow', 'dark' or 'light' for the handwriting panel; it falls back to the app mode. */
  handwriting_theme?: string;
  /** 'follow', 'dark' or 'light' for the candidate context menu; it falls back to the app mode. */
  menu_theme?: string;
  theme: string;
  traditional_chinese_output: boolean;
  chinese_punctuation: boolean;
  smart_punctuation: boolean;
  smart_punctuation_repeat: boolean;
  smart_punctuation_space_convert: boolean;
  paired_punctuation: boolean;
  punctuation_lock: string;
  word_character: PreparedWordCharacterPreferences;
  floating_toolbar: PreparedFloatingToolbar;
  candidate_english_gloss: boolean;
  candidate_translations: boolean;
  /** Always written by the shared store; optional so a hand-edited document without it still prepares. */
  fuzzy_pinyin?: PreparedFuzzyPinyin;
  translation_target_language: string;
  translation_secondary_language?: string | null;
  custom_translation: TranslationProviderConfig;
  tencent_tmt: TencentTranslationConfig;
  niutrans: NiuTransTranslationConfig;
  /** Tenths of a vp, which is the unit the shared settings store these in. */
  touch_key_spacing_tenths: number;
  touch_row_spacing_tenths: number;
  /** Added to the total height of the key rows, between -12 and 48 vp. */
  touch_keyboard_height_adjustment: number;
  /** Show the native Harmony voice entry in the top touch-keyboard shortcut bar. */
  touch_voice_shortcut: boolean;
  /** Shared privacy switch for the private clipboard history file and keyboard entry point. */
  clipboard_history: boolean;
  voice_input: VoiceInputConfiguration;
  ai_assistant: PreparedAiAssistant;
  touch_keyboard_schemes: PreparedTouchKeyboardSchemes;
  keybindings: PreparedKeybindings;
  /** Sound packs and the rest of the plugin settings; absent while every part is at its default. */
  plugins?: PluginPreferenceDocument;
}

/** The shared fuzzy pinyin record. Turning it off keeps the rules, so the next on restores the same set. */
interface PreparedFuzzyPinyin {
  enabled: boolean;
  rules: string[];
  /** The store's marker that the first enable already opted every rule in; never shown. */
  seeded?: boolean;
}

/** The shared touch-scheme record: which schemes the picker offers, and which one is current. */
interface PreparedTouchKeyboardSchemes {
  enabled?: string[];
  selected: string | null;
}

interface PreparedNavigationPreferences {
  minus_equal: boolean;
  comma_period: boolean;
  brackets: boolean;
  tab: boolean;
  page_up_down: boolean;
  mouse_wheel: boolean;
  arrows: boolean;
}

interface PreparedWordCharacterPreferences {
  enabled: boolean;
  keys: string;
}

/** The shared floating toolbar record. Only what a HarmonyOS toolbar can honour is read from it. */
interface PreparedFloatingToolbar {
  enabled: boolean;
  english_mode: boolean;
  punctuation: boolean;
  fullwidth: boolean;
  character_set: boolean;
  emoji: boolean;
  /** Absent in documents written before the 2in1 toolbar carried it; shown there, as the shared default is. */
  handwriting?: boolean;
  screen_keyboard: boolean;
  /** Absent in documents written before the 2in1 toolbar carried it; shown there, as the shared default is. */
  voice?: boolean;
  /** Shared switch for the settings button; absent in older documents, where it was always shown. */
  settings?: boolean;
  scale_percent: number;
  font_size: number;
}

/** The shared mode-switch bindings, which a host with a real keyboard is the one to consume. */
interface PreparedKeybindings {
  switch_language_shift: boolean;
  switch_language_ctrl: boolean;
  switch_language_ctrl_alt_space: boolean;
  toggle_character_set_ctrl_shift_f: boolean;
}

/** What the view needs to draw itself, resolved once from the prepared preferences. */
export interface KeyboardPreferences {
  /** The keyboard's mode: its own theme, the app mode and the system, unless the global theme fixes it. */
  readonly dark: boolean;
  /** The shared global theme id the keyboard and candidate window are drawn in. */
  readonly globalThemeId: string;
  /** Manifest minimum width of the drawn external package, clamped to the same bounds as the native estimate. */
  readonly candidateMinWidthVp: number;
  /** Image-only data URL for a compatible external candidate decoration. */
  readonly candidateDecorationImage: string | null;
  readonly candidateDecorationTopVp: number;
  readonly candidateDecorationWidthVp: number;
  readonly candidateDecorationAspectRatio: number;
  /** Where the decoration sits along the card's top edge. */
  readonly candidateDecorationAlign: CandidateSkinAlign;
  /** The card radius: the user's, then the drawn package's; null keeps the host's. */
  readonly candidateCornerRadiusVp: number | null;
  /** Whether that radius is the user's own: only then do the rows round down with a card tighter than their own corners. */
  readonly candidateCornerRadiusUser: boolean;
  /** Image-only data URL drawn over the card surface, clipped to the card. */
  readonly candidateBackgroundImage: string | null;
  readonly candidateBackgroundFit: CandidateSkinFit;
  readonly candidateBackgroundOpacity: number;
  /** The user's 0.5..1 factor on the card's fill, border and background image; text and the selection stay opaque. */
  readonly candidateWindowOpacity: number;
  readonly candidateDark: boolean;
  /** The shared switch when it is set, otherwise what the theme says; the Harmony window marks the row with a fill, not a bar. */
  readonly candidateSelectedBar: boolean;
  readonly pairedPunctuation: boolean;
  readonly keyboardTheme: string;
  readonly toolbarTheme: string;
  readonly voiceTheme: string;
  readonly emojiTheme: string;
  readonly handwritingTheme: string;
  readonly menuTheme: string;
  /** The `theme` preference: the light/dark app mode, which is not the global theme. */
  readonly appMode: string;
  readonly scheme: string;
  readonly shuangpinProfile: string;
  readonly layout: string;
  /** Shared touch-scheme ids enabled in the native picker, in shared declaration order. */
  readonly enabledSchemes: string[];
  /**
   * Which of them the shared page last chose, or null when it has never said.
   *
   * Read here rather than inferred from `scheme` and `layout`: those two are the Engine's view and
   * say nothing about whether the picker still offers what they resolve to.
   */
  readonly selectedScheme: string | null;
  readonly candidateFollowCursor: boolean;
  readonly numberRowSelection: boolean;
  readonly navigation: HardwareNavigationPreferences;
  /** Whether the desktop candidate panel shows the spelling above its candidates. */
  readonly candidateShowPreedit: boolean;
  readonly candidateLayout: string;
  readonly candidateFontSize: number;
  readonly candidatePreeditFontSize: number;
  readonly candidateFontFamily: string;
  readonly candidateEnglishFont: string | null;
  readonly candidateFallbackFonts: string[];
  /** The 2in1 candidate window's colours in its mode, from the resolved theme with the native tokens behind it. */
  readonly candidateColors: CandidateColors;
  readonly keySpacingTenths: number;
  readonly rowSpacingTenths: number;
  readonly heightAdjustment: number;
  readonly voiceShortcut: boolean;
  readonly thoughtfulReply: boolean;
}

interface PreparedOptions {
  preferences: PreparedPreferences;
  /**
   * 让运行时把已选的那一段留在组字里而不是立刻上屏。
   *
   * 准备宿主时不带这个字段，是这个宿主自己要求的：它会把那一段画在候选条的读音前面。请求与绘制
   * 是一个决定，只做一半就等于把用户已经选中的字藏起来。
   */
  phrase_preedit?: boolean;
}

interface TypingEffectSettingsResponse {
  ok: boolean;
  value: ResolvedTypingEffectDocument;
  error: string;
}

interface PrepareResponse {
  ok: boolean;
  value: PreparedOptions;
  error: string;
}

interface DictionaryAction {
  operation: string;
}

interface DictionaryRequest {
  options: PreparedOptions;
  action: DictionaryAction;
}

interface PersonalDictionarySyncValue {
  pending_count: number;
}

interface PersonalDictionarySyncResponse {
  ok: boolean;
  value: PersonalDictionarySyncValue;
  error: string;
}

/**
 * The shared command table from msime_client_command. Spelling the numbers out here keeps the call
 * sites readable and keeps the table in one place instead of scattered behind comments.
 */
enum InputCommand {
  BACKSPACE = 0,
  SELECT_HIGHLIGHTED = 1,
  COMMIT_RAW = 2,
  CANCEL = 3,
  MOVE_LEFT = 4,
  MOVE_RIGHT = 5,
  MOVE_HOME = 6,
  MOVE_END = 7,
  DELETE_FORWARD = 8,
  FINISH_COMPOSITION = 9,
  COMMIT_READING = 11,
  BACKSPACE_SEGMENT = 12,
  MOVE_LEFT_SEGMENT = 13,
  MOVE_RIGHT_SEGMENT = 14,
  COMMIT_RAW_WITHOUT_LEARNING = 15,
  CONVERT_HANJA = 16,
  NEXT_PAGE = 100,
  PREVIOUS_PAGE = 101,
  NEXT_CANDIDATE = 102,
  PREVIOUS_CANDIDATE = 103,
  FIRST_CANDIDATE = 104,
  LAST_CANDIDATE = 105
}

const JAPANESE_SCHEME: number = 3;
const IDLE_DICTIONARY_RETRY_MS: number = 2000;

/**
 * Owns the native session for the lifetime of the extension ability and turns key presses into
 * committed text. The ability creates it; the panel's UI reads state back through onChanged.
 *
 * Deliberately thin: the Engine decides what a key means, this only forwards and applies the result.
 */
export class KeyboardSession implements HardwareKeyTarget {
  static readonly shared: KeyboardSession = new KeyboardSession();

  private handle: number = 0;
  private inputClient: inputMethodEngine.InputClient | undefined = undefined;
  private personalDictionarySettled: boolean = true;
  private personalDictionaryTimer: number = -1;
  private snapshotOptions: Object | undefined = undefined;
  private snapshotQueueTimer: number = -1;
  // Key press counts for the key heatmap. Nothing is held while statistics are off, which is read from the store on every focus; until the editor's attribute has said what kind of field it is, a press could be a password's, so none is counted.
  private keyStatisticsEnabled: boolean = false;
  private keyStatisticsEditorKnown: boolean = false;
  private readonly keyPresses: KeyPressBatch = new KeyPressBatch();
  private keyFlushTimer: number = -1;
  private editorGeneration: number = 0;
  // Bumped only by attach/detach: an idle Engine restart (which bumps editorGeneration) must not
  // drop the focused editor's attribute.
  private attributeGeneration: number = 0;
  // Editor mutations are echoed asynchronously through KeyboardDelegate.textChange. Keep the
  // count raised until those callbacks arrive so our own commit cannot be mistaken for a host edit
  // and finish a residual composition that the Engine deliberately left active.
  private pendingOwnEdits: number = 0;
  private inlinePreeditStyle: InlinePreeditStyle = 'raw';
  /** Whether the focused editor said it takes preview text; learned from its attribute. */
  private previewSupported: boolean = false;
  /** The preview text the editor holds now, empty when it holds none. */
  private previewText: string = '';
  private replyContextGeneration: number = 0;
  private replyResultEditorGeneration: number = -1;
  private replyResultContextGeneration: number = -1;
  private replyResultTexts: string[] = [];
  private generation: number = 0;
  // The Engine opens a session in whichever mode the preferences name, and the shared default is
  // English. Remembered here so every focus reasserts it rather than inheriting the previous editor.
  private englishMode: boolean = false;
  /** The editor generation in which the user explicitly chose Chinese or English. */
  private manualLanguageGeneration: number = -1;
  // Which application the mode belongs to. Windows and macOS both keep this per application by
  // default; this host used to keep one mode for every editor it was ever attached to.
  private englishSuggestionsEnabled: boolean = true;
  // The completion list currently on the strip, the prefix it was asked for, and a counter that
  // makes every earlier answer stale. A reply that arrives after the word moved on is discarded.
  private englishSuggestions: string[] = [];
  private englishSuggestionPrefix: string = '';
  private englishSuggestionEpoch: number = 0;
  private englishSuggestionRequested: string = '';
  // The Windows voice shortcuts, which only a machine with a hardware keyboard can press.
  private voiceHotkeys: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  // Whether the recognizer is running. The view owns it and says so; the shortcuts need to know,
  // because Space and Escape belong to a composition at every other moment.
  private voiceRecording: boolean = false;
  private voiceBehaviour: HarmonyVoiceRecordingBehaviour = new HarmonyVoiceRecordingBehaviour();
  // Key, melody, commit and achievement sounds. Configured off on a phone, which keeps its own key feedback.
  private readonly soundPlayer: KeySoundPlayer = new KeySoundPlayer();
  private readonly musicPlayer: MusicPlayer = new MusicPlayer();
  /** The typing effect and combo settings the preferences ask for; off on a phone, as the sounds are. */
  private typingEffectPreferences: TypingEffectSettings = TYPING_EFFECTS_OFF;
  /** `typingEffectPreferences` with the selected effect pack resolved by host-api: what is drawn. */
  private typingEffects: TypingEffectSettings = TYPING_EFFECTS_OFF;
  /** The focused editor's attributes have arrived, so whether it is a password field is known; background music waits for that. */
  private editorAttributesKnown: boolean = false;
  private modeScope: ImeModeScopePolicy = new ImeModeScopePolicy();
  private modeScopeSetting: string = 'app';
  private defaultEnglish: boolean = false;
  // Hardware events report this independently of the Engine's dedicated-English mode. The
  // floating toolbar uses it as the same high-priority status shown by the Windows presenter.
  private capsLock: boolean = false;
  // The grid is the Engine's business too: it decides what a 2 means, and it only reads them as
  // nine-key spellings once it has been told the grid is up.
  private nineKeyMode: boolean = false;
  private schemeName: string = 'quanpin';
  private profileName: string = 'xiaohe';
  private clipboardHistoryEnabled: boolean = false;
  // The two optional candidate annotations. Held here rather than in KeyboardPreferences because the
  // session builds the candidate entries itself; the view only draws what it is handed.
  private wubiCodeHint: boolean = true;
  private candidateEnglishGloss: boolean = false;
  private candidateGlossEnabled: boolean = false;
  private candidateGlossRows: number = 0;
  // Mirrored for the function panel's 模糊音 tile; the rules themselves stay in the document.
  private fuzzyPinyin: boolean = false;
  // Read before the panels are built, so a badge the user turned off is never created at all.
  private modeBadge: boolean = true;
  private resourceDirectory: string = '';
  private stateDirectory: string = '';
  private cloudClipboardClient: CloudClipboardClient | undefined = undefined;
  private systemDark: boolean = false;
  // What the editor wants its return key to do. Sending this rather than a newline is what makes the
  // key search in a search field and send in a message field, which is the whole point of having it.
  private enterKey: number = inputMethodEngine.ENTER_KEY_TYPE_UNSPECIFIED;
  private traits: EditorTraits = {
    text: true, password: false, uri: false, email: false, noSuggestions: false
  };
  /** What the attached editor asked for, already reconciled with the kind of field it is. */
  private capitalization: CapitalizationMode = CapitalizationMode.NONE;
  private appearance: KeyboardPreferences = {
    dark: false,
    globalThemeId: 'system',
    candidateMinWidthVp: CandidateWidthPolicy.MIN_WIDTH_VP,
    candidateDecorationImage: null,
    candidateDecorationTopVp: 0,
    candidateDecorationWidthVp: 0,
    candidateDecorationAspectRatio: 1,
    candidateDecorationAlign: 'right',
    candidateCornerRadiusVp: null,
    candidateCornerRadiusUser: false,
    candidateBackgroundImage: null,
    candidateBackgroundFit: 'cover',
    candidateBackgroundOpacity: 1,
    candidateWindowOpacity: 1,
    candidateDark: false,
    candidateSelectedBar: false,
    pairedPunctuation: true,
    keyboardTheme: 'follow',
    toolbarTheme: 'follow',
    appMode: 'system',
    emojiTheme: 'follow',
    handwritingTheme: 'follow',
    menuTheme: 'follow',
    voiceTheme: 'follow',
    // What an absent shared list means; the prepared record narrows it.
    enabledSchemes: KeyboardScheme.DEFAULT_ENABLED.map(
      (candidate: SchemeDefinition): string => candidate.preferenceId),
    selectedScheme: null,
    scheme: 'quanpin',
    shuangpinProfile: 'xiaohe',
    layout: 'twenty_six_key',
    candidateFollowCursor: true,
    // The shared default (`number_row_selection` in client-core preferences.rs): 1 through 9 pick a candidate.
    numberRowSelection: true,
    candidateShowPreedit: true,
    candidateLayout: 'vertical',
    navigation: {
      minusEqual: true, commaPeriod: true, brackets: false,
      tab: true, pageUpDown: true, mouseWheel: false, arrows: true
    },
    candidateFontSize: 18,
    candidatePreeditFontSize: 15,
    candidateFontFamily: 'Noto Sans SC',
    candidateEnglishFont: null,
    candidateFallbackFonts: ['Noto Sans SC', 'Microsoft YaHei'],
    candidateColors: GlobalTheme.candidateColors(null, false),
    keySpacingTenths: KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
    rowSpacingTenths: KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
    heightAdjustment: KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP,
    voiceShortcut: false,
    thoughtfulReply: false
  };
  private voiceInput: VoiceInputConfiguration = DEFAULT_VOICE_INPUT_CONFIGURATION;
  /**
   * The session's own polisher, separate from the recognizer's.
   *
   * The recognizer cancels its polisher whenever recognition starts, stops or is abandoned, which
   * is right for a transcript and wrong for a rewrite the user is waiting on: sharing one would let
   * tapping the microphone silently discard a polish request that was about to answer.
   */
  private readonly polisher: HarmonyVoicePolisher = new HarmonyVoicePolisher();
  /** The one in-flight reply request; invalidating its editor context must cancel the HTTP wait. */
  private replyRequest: http.HttpRequest | undefined = undefined;
  private aiAssistant: PreparedAiAssistant = {
    enabled: false, provider: 'deepseek', model: '', token: '', tokens: {}, endpoint: '',
    candidate_limit: 3, prompt_id: 'custom_1', prompt: '', prompt_custom_1: '',
    prompt_custom_2: '', prompt_custom_3: ''
  };

  onChanged: ((state: KeyboardState) => void) | undefined = undefined;
  /** The view removes clipboard history immediately when the shared privacy switch is turned off. */
  onClipboardHistoryChanged: ((enabled: boolean) => void) | undefined = undefined;
  /** Read-only English completions for the strip: the prefix they answer and the words offered. */
  onEnglishSuggestions: ((prefix: string, items: string[]) => void) | undefined = undefined;
  /** What a voice shortcut asked for, for the view that owns the recognizer. */
  onVoiceHotkey: ((action: VoiceHotkeyAction) => void) | undefined = undefined;
  /** The view removes the voice surface immediately when shared voice input is turned off. */
  onVoiceInputChanged: ((enabled: boolean) => void) | undefined = undefined;
  /** Lets a desktop panel resize when a vertical candidate page changes height. */
  onCandidateCount: ((count: number) => void) | undefined = undefined;
  /** Lets a desktop panel flash its candidate card and show the combo for each key and commit handed to the typing effect, with the settings it is drawn with (intensity, flash length and colour); `key` is false for a commit, which counts nothing and only reports the combo. */
  onTypingEffect: ((effect: TypingEffect, settings: TypingEffectSettings, key: boolean) => void) | undefined = undefined;
  /** Lets a desktop panel follow the widest current candidate without measuring ArkUI nodes. */
  onCandidateWidth: ((widthVp: number) => void) | undefined = undefined;
  /** The native host owns panel geometry; the touch view reports its newly requested total height. */
  onTouchHeight: ((heightVp: number) => void) | undefined = undefined;
  /** The ability's half of a preference reload: status panels and key bindings. */
  onPreferencesReloaded: (() => void) | undefined = undefined;
  /** The view's half of a preference reload: skins, geometry and the scheme face. */
  onViewPreferencesReloaded: (() => void) | undefined = undefined;
  /**
   * Whether a composition is open, for the host that owns the panel.
   *
   * A candidate window has nothing to say when nothing is being spelled, and the framework does not
   * show or hide one on its own, so the ability needs to hear about the transitions. The view has
   * onChanged; this is the same news for the other reader, and fires only when the answer changes.
   */
  onComposition: ((active: boolean) => void) | undefined = undefined;
  /** The mode badge's face, set by whichever window is currently drawing it: the mode, the toolbar's font size and scale it is sized by, and the toolbar palette it wears. */
  onModeBadge: ((english: boolean, fontSize: number, scale: number, toolbarSkin: ToolbarSkin) => void)
    | undefined = undefined;
  /** The toolbar's face, set by whichever window is currently drawing it. */
  onToolbar: ((state: ToolbarState, buttons: ToolbarButton[], fontSize: number, scale: number,
               toolbarSkin: ToolbarSkin) => void) | undefined = undefined;
  /** The status-bar panel owns the window; the ArkUI handle only reports its pan gesture. */
  onToolbarDragStart: (() => void) | undefined = undefined;
  onToolbarDrag: ((offsetX: number, offsetY: number) => void) | undefined = undefined;
  /** The settings application, which only the host can open: it owns the context that starts it. The 2in1 toolbar gear and the phone's 更多设置 card both reach it here. */
  onOpenSettings: (() => void) | undefined = undefined;
  /**
   * A surface the toolbar asked for, which two readers need and for different reasons: the host
   * sizes and shows the window, the view draws what goes in it. Two slots rather than one, the way
   * onChanged and onComposition are already split, because a single slot silently keeps whichever
   * listener registered last — and the window then resizes around a face that was never drawn.
   */
  onSurfaceRequested: ((surface: DesktopSurface) => void) | undefined = undefined;
  onSurfaceChanged: ((surface: DesktopSurface) => void) | undefined = undefined;
  /** The emoji panel's answer to a hardware key, set by the view that draws the panel; see EmojiPanelKeyPolicy. */
  onPanelKey: ((key: EmojiPanelKey) => boolean) | undefined = undefined;
  private desktopSurface: DesktopSurface = DesktopSurface.NONE;
  private composedLast: boolean = false;
  /** The editor's traits arrive after inputStart, so the view is told when they do. */
  onEditorChanged: (() => void) | undefined = undefined;
  /** Clears reply results when the focused editor or its cursor context changes. */
  onReplyContextChanged: (() => void) | undefined = undefined;
  /** Putting the keyboard away belongs to the panel, which the view has no handle on. */
  onDismiss: (() => void) | undefined = undefined;

  /**
   * True once the native ABI matches and a session exists.
   *
   * The options are not written here. prepareHost verifies the staged dictionary against the pinned
   * lock, lays out the derived state under the state root, and returns the document create expects;
   * a hand-written one would drift from the lock the moment the dictionary is republished.
   */
  private readCandidateSkinCatalog(stateRoot: string): CandidateSkinPackage[] {
    try {
      const response: SkinCatalogResponse =
        JSON.parse(client.skinCatalog(`${stateRoot}/skins`)) as SkinCatalogResponse;
      if (!response.ok || response.value === undefined || !Array.isArray(response.value.packages)) {
        return [];
      }
      return response.value.packages;
    } catch (error) {
      KeyboardLog.error(`candidate skin catalog unavailable: ${describeError(error)}`);
      return [];
    }
  }

  private readCandidateSkinImage(stateRoot: string, id: string,
                                 relative: string | null): CandidateSkinImage {
    if (relative === null) {
      return NO_CANDIDATE_SKIN_IMAGE;
    }
    try {
      const request: SkinResourceRequest = {
        directory: `${stateRoot}/skins`, id: id, relative: relative, kind: 'image'
      };
      const response: SkinResourceResponse =
        JSON.parse(client.skinResource(JSON.stringify(request))) as SkinResourceResponse;
      if (!response.ok || response.value === undefined) {
        return NO_CANDIDATE_SKIN_IMAGE;
      }
      const image: CandidateSkinImage = {
        image: CandidateSkinCatalogPolicy.imageDataUrl(response.value.contentType,
          response.value.bytes),
        aspectRatio: CandidateSkinCatalogPolicy.imageAspectRatio(response.value.contentType,
          response.value.bytes)
      };
      return image;
    } catch (error) {
      KeyboardLog.error(`candidate skin decoration unavailable: ${describeError(error)}`);
      return NO_CANDIDATE_SKIN_IMAGE;
    }
  }

  private readCandidateSkinToolbar(stateRoot: string, id: string,
                                    stylesheet: string | null): string | null {
    if (stylesheet === null || !KeyboardFormFactor.isDesktop()) {
      return null;
    }
    try {
      const response: SkinToolbarStylesheetResponse = JSON.parse(client.skinToolbarStylesheet(
        JSON.stringify({ directory: `${stateRoot}/skins`, id: id }))) as SkinToolbarStylesheetResponse;
      return response.ok && typeof response.value === 'string' ? response.value : null;
    } catch (error) {
      KeyboardLog.error(`candidate skin toolbar unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /**
   * Resolves the global theme for both modes and draws every surface's appearance from the answer.
   *
   * The resolve reads the skin root, so it runs when the preferences are read or the theme is chosen, never while drawing. Both modes are resolved up front because each surface keeps its own light/dark rule: the keyboard, the candidate window and the toolbar can sit in different modes at once, and a theme over `system` (a package, say) answers differently in each.
   */
  private applyAppearance(preferences: PreparedPreferences): void {
    if (this.themeCatalog.length === 0) {
      this.themeCatalog = this.readThemeCatalog();
    }
    this.globalThemeId = preferences.global_theme;
    this.customThemeBase = preferences.custom_theme?.base ?? 'system';
    const design: CustomSkinDocument | undefined = preferences.custom_theme?.keyboard;
    this.customDesign = design === undefined ? null : CustomKeyboardSkin.from(design);
    this.themeLight = this.resolveTheme(preferences, false);
    this.themeDark = this.resolveTheme(preferences, true);
    const keyboardDark: boolean = this.themeDarkFor(preferences.screen_keyboard_theme, preferences.theme);
    const candidateDark: boolean = this.themeDarkFor(preferences.candidate_theme, preferences.theme);
    const candidateTheme: ResolvedTheme | null = candidateDark ? this.themeDark : this.themeLight;
    const candidateColors: CandidateColors = GlobalTheme.candidateColors(
      candidateTheme === null ? null : candidateTheme.candidate, candidateDark);
    const toolbarDark: boolean = this.themeDarkFor(preferences.toolbar_theme ?? 'follow', preferences.theme);
    const toolbarTheme: ResolvedTheme | null = toolbarDark ? this.themeDark : this.themeLight;
    // `candidate_skin` is the one signal that a package was drawn in this layout and mode; the package catalog is only read for what the theme does not carry.
    const packageId: string | null = candidateTheme === null ? null : candidateTheme.candidate_skin;
    const toolbarPackageId: string | null = toolbarTheme === null ? null : toolbarTheme.candidate_skin;
    const skinPackages: CandidateSkinPackage[] = packageId === null && toolbarPackageId === null
      ? [] : this.readCandidateSkinCatalog(this.stateDirectory);
    const skinMinWidth: number | null = packageId === null
      ? null : CandidateSkinCatalogPolicy.minWidthVp(skinPackages, packageId);
    const skinDecoration: CandidateSkinDecoration | null = packageId === null
      ? null : CandidateSkinCatalogPolicy.decoration(skinPackages, packageId);
    const skinToolbar: string | null = toolbarPackageId === null ? null
      : this.readCandidateSkinToolbar(this.stateDirectory, toolbarPackageId,
        CandidateSkinCatalogPolicy.toolbarStylesheet(skinPackages, toolbarPackageId));
    const skinImage: CandidateSkinImage = KeyboardFormFactor.isDesktop() && packageId !== null
      ? this.readCandidateSkinImage(this.stateDirectory, packageId,
        skinDecoration === null ? null : skinDecoration.relative)
      : NO_CANDIDATE_SKIN_IMAGE;
    const skinBackground: CandidateSkinBackground | null = packageId === null
      ? null : CandidateSkinCatalogPolicy.background(skinPackages, packageId);
    // Like the decoration, the background is a desktop candidate window's; the touch strip keeps the keyboard's look.
    const backgroundImage: CandidateSkinImage = KeyboardFormFactor.isDesktop() && packageId !== null
      ? this.readCandidateSkinImage(this.stateDirectory, packageId,
        skinBackground === null ? null : skinBackground.relative)
      : NO_CANDIDATE_SKIN_IMAGE;
    // The store validates both to the shared bounds; an out-of-range value from an older or foreign document is ignored rather than drawn.
    const userRadius: number | null | undefined = preferences.candidate_corner_radius;
    const userRadiusValid: boolean =
      typeof userRadius === 'number' && Number.isFinite(userRadius) && userRadius >= 0 && userRadius <= 32;
    const candidateRadius: number | null = userRadiusValid
      ? userRadius as number
      : packageId === null ? null : CandidateSkinCatalogPolicy.cornerRadiusVp(skinPackages, packageId);
    const opacityPercent: number | undefined = preferences.candidate_opacity_percent;
    const windowOpacity: number = typeof opacityPercent === 'number' && Number.isFinite(opacityPercent)
      ? Math.max(50, Math.min(100, opacityPercent)) / 100 : 1;
    this.appearance = {
      dark: keyboardDark,
      globalThemeId: preferences.global_theme,
      candidateMinWidthVp: skinMinWidth === null
        ? CandidateWidthPolicy.MIN_WIDTH_VP
        : Math.max(CandidateWidthPolicy.MIN_WIDTH_VP,
          Math.min(CandidateWidthPolicy.MAX_WIDTH_VP, skinMinWidth)),
      candidateDecorationImage: skinImage.image,
      candidateDecorationTopVp: skinImage.image === null ? 0 : (skinDecoration?.topVp ?? 0),
      candidateDecorationWidthVp: skinImage.image === null ? 0 : (skinDecoration?.widthVp ?? 0),
      candidateDecorationAspectRatio: skinImage.aspectRatio,
      candidateDecorationAlign: skinDecoration === null ? 'right' : skinDecoration.align,
      candidateCornerRadiusVp: candidateRadius,
      candidateCornerRadiusUser: userRadiusValid,
      candidateBackgroundImage: backgroundImage.image,
      candidateBackgroundFit: skinBackground === null ? 'cover' : skinBackground.fit,
      candidateBackgroundOpacity: skinBackground === null ? 1 : skinBackground.opacity,
      candidateWindowOpacity: windowOpacity,
      candidateDark: candidateDark,
      candidateSelectedBar: preferences.candidate_selected_bar ?? candidateColors.showSelectedBar,
      pairedPunctuation: preferences.paired_punctuation !== false,
      keyboardTheme: preferences.screen_keyboard_theme,
      toolbarTheme: preferences.toolbar_theme ?? 'follow',
      voiceTheme: preferences.voice_theme ?? 'follow',
      emojiTheme: preferences.emoji_theme ?? 'follow',
      handwritingTheme: preferences.handwriting_theme ?? 'follow',
      menuTheme: preferences.menu_theme ?? 'follow',
      appMode: preferences.theme,
      scheme: preferences.scheme,
      shuangpinProfile: preferences.shuangpin_profile,
      layout: preferences.touch_keyboard_layout,
      selectedScheme: preferences.touch_keyboard_schemes?.selected ?? null,
      // A Cantonese or Zhuyin card whose dictionary this package did not stage stays hidden even when enabled: the Engine would type 全拼 under it.
      enabledSchemes: KeyboardScheme.withInstalledDictionaries(
        KeyboardScheme.enabledFromPreferenceIds(preferences.touch_keyboard_schemes?.enabled ?? null),
        (file: string): boolean => this.languageDictionaryInstalled(file))
        .map((candidate: SchemeDefinition): string => candidate.preferenceId),
      candidateFollowCursor: preferences.candidate_follow_cursor,
      numberRowSelection: preferences.number_row_selection,
      candidateShowPreedit: preferences.candidate_preedit_style !== 'empty',
      candidateLayout: preferences.candidate_layout === 'horizontal' ? 'horizontal' : 'vertical',
      navigation: {
        minusEqual: preferences.navigation?.minus_equal ?? true,
        commaPeriod: preferences.navigation?.comma_period ?? true,
        brackets: preferences.navigation?.brackets ?? false,
        tab: preferences.navigation?.tab ?? true,
        pageUpDown: preferences.navigation?.page_up_down ?? true,
        mouseWheel: preferences.navigation?.mouse_wheel ?? false,
        arrows: preferences.navigation?.arrows ?? true
      },
      // The shared store validates these to 12..32 and strips control
      // characters from family names. Clamp again at the host boundary so
      // an older staged document cannot produce an unusable panel.
      candidateFontSize: Math.max(12, Math.min(32, preferences.candidate_font_size)),
      candidatePreeditFontSize:
        Math.max(12, Math.min(32, preferences.candidate_preedit_font_size)),
      candidateFontFamily: preferences.candidate_font_family.trim().length > 0
        ? preferences.candidate_font_family : 'Noto Sans SC',
      candidateEnglishFont: preferences.candidate_english_font ?? null,
      candidateFallbackFonts: preferences.candidate_fallback_fonts.filter(
        (font: string): boolean => font.trim().length > 0),
      candidateColors: candidateColors,
      // Clamped rather than trusted: the store validates, but an older document or a host that
      // never learned the bounds would otherwise draw a keyboard nobody can hit.
      keySpacingTenths: KeyboardGeometry.keySpacing(preferences.touch_key_spacing_tenths),
      rowSpacingTenths: KeyboardGeometry.rowSpacing(preferences.touch_row_spacing_tenths),
      heightAdjustment:
        KeyboardGeometry.heightAdjustment(preferences.touch_keyboard_height_adjustment),
      voiceShortcut: preferences.touch_voice_shortcut,
      thoughtfulReply: preferences.touch_keyboard_schemes?.selected === 'thoughtful_reply'
    };
    // The theme's bar, then the package's `[toolbar]` colours for that mode, then its stylesheet.
    const toolbarColors: CandidateColors = GlobalTheme.candidateColors(
      toolbarTheme === null ? null : toolbarTheme.candidate, toolbarDark);
    this.toolbarSkin = ToolbarSkinPolicy.fromCss(toolbarColors, skinToolbar,
      ToolbarSkinPolicy.withPackage(toolbarColors,
        CandidateSkinCatalogPolicy.toolbar(skinPackages, toolbarPackageId), toolbarDark));
  }

  /** A surface's mode: fixed by a theme with an appearance, otherwise its own setting, then the app mode, then the system. */
  private themeDarkFor(surfaceTheme: string, appMode: string): boolean {
    return GlobalTheme.surfaceDark(this.fixedAppearance(),
      KeyboardSkin.resolveDark(surfaceTheme, appMode, this.systemDark));
  }

  /** `light` or `dark` when the global theme fixes the mode; the answer is the same for both requested modes. */
  private fixedAppearance(): string | null {
    return this.themeLight === null ? null : this.themeLight.appearance;
  }

  private readThemeCatalog(): ThemeCatalogEntry[] {
    try {
      return GlobalTheme.parseCatalog(client.themeCatalog());
    } catch (error) {
      KeyboardLog.error(`theme catalog unavailable: ${describeError(error)}`);
      return [];
    }
  }

  /** One mode of the global theme. A refused request draws the native tokens rather than failing the session. */
  private resolveTheme(preferences: PreparedPreferences, dark: boolean): ResolvedTheme | null {
    const request: ResolveThemeRequest = {
      global_theme: preferences.global_theme,
      custom_theme: preferences.custom_theme,
      dark: dark,
      layout: preferences.candidate_layout === 'horizontal' ? 'horizontal' : 'vertical',
      skins_directory: `${this.stateDirectory}/skins`
    };
    try {
      const resolved: ResolvedTheme | null =
        GlobalTheme.parseResolved(client.resolveTheme(JSON.stringify(request)));
      if (resolved === null) {
        KeyboardLog.error(`theme ${preferences.global_theme} refused; drawing the native tokens`);
      }
      return resolved;
    } catch (error) {
      KeyboardLog.error(`theme unavailable: ${describeError(error)}`);
      return null;
    }
  }

  start(resources: string, stateRoot: string, systemDark: boolean): boolean {
    let abi: number;
    try {
      abi = client.abiVersion();
    } catch (error) {
      KeyboardLog.error(`native module unavailable: ${describeError(error)}`);
      return false;
    }
    if (abi !== 3) {
      KeyboardLog.error(`native ABI is ${abi}, expected 3`);
      return false;
    }
    this.resourceDirectory = resources;
    this.stateDirectory = stateRoot;
    this.systemDark = systemDark;
    this.feedbackSettings = this.feedback();
    this.preferencesRevision = this.storedRevision(stateRoot);
    const request: PrepareRequest = { resources: resources, state_root: stateRoot };
    let options: string;
    try {
      const prepared: PrepareResponse =
        JSON.parse(client.prepareHost(JSON.stringify(request))) as PrepareResponse;
      if (!prepared.ok) {
        KeyboardLog.error(`host preparation refused: ${prepared.error}`);
        return false;
      }
      const requested: PreparedOptions = prepared.value;
      requested.phrase_preedit = true;
      options = JSON.stringify(requested);
      this.snapshotOptions = requested;
      this.processDictionarySnapshot(requested, stateRoot);
      const preferences: PreparedPreferences = prepared.value.preferences;
      this.clipboardHistoryEnabled = ClipboardHistoryPreferencePolicy.enabled(
        preferences.clipboard_history);
      this.inlinePreeditStyle = InlinePreeditPolicy.style(preferences.tsf_preedit_style);
      this.wubiCodeHint = CandidateAnnotationPreferencePolicy.wubiCodeHint(
        preferences.wubi_code_hint);
      this.candidateEnglishGloss = CandidateAnnotationPreferencePolicy.englishGloss(
        preferences.candidate_english_gloss);
      this.applyCandidateGloss(preferences);
      this.fuzzyPinyin = preferences.fuzzy_pinyin !== undefined && preferences.fuzzy_pinyin.enabled;
      this.modeBadge = InputModeHudPolicy.enabled(preferences.input_mode_hud);
      this.englishSuggestionsEnabled = preferences.english_suggestions !== false;
      if (ClipboardHistoryPreferencePolicy.shouldClearHistory(this.clipboardHistoryEnabled)) {
        this.clearClipboardHistoryFile();
      }
      if (this.onClipboardHistoryChanged !== undefined) {
        this.onClipboardHistoryChanged(this.clipboardHistoryEnabled);
      }
      this.voiceInput = preferences.voice_input;
      this.voiceHotkeys.use(KeyboardSession.hotkeyBindings(preferences.voice_input));
      if (this.onVoiceInputChanged !== undefined) {
        this.onVoiceInputChanged(this.voiceInputEnabled());
      }
      this.aiAssistant = preferences.ai_assistant;
      this.defaultEnglish = preferences.default_ime_mode === 'english';
      this.modeScopeSetting = preferences.ime_mode_scope === 'global' ? 'global' : 'app';
      this.englishMode = this.defaultEnglish;
      this.bindings = {
        switchLanguageShift: preferences.keybindings.switch_language_shift,
        switchLanguageCtrl: preferences.keybindings.switch_language_ctrl,
        switchLanguageCtrlAltSpace: preferences.keybindings.switch_language_ctrl_alt_space,
        toggleCharacterSetCtrlShiftF: preferences.keybindings.toggle_character_set_ctrl_shift_f
      };
      this.preparedPreferences = preferences;
      this.applyAppearance(preferences);
      this.schemeName = preferences.scheme;
      this.profileName = preferences.shuangpin_profile;
      this.traditional = preferences.traditional_chinese_output;
      this.chinesePunctuation = preferences.chinese_punctuation;
      this.smartPunctuation = preferences.smart_punctuation !== false;
      this.smartPunctuationRepeat = preferences.smart_punctuation_repeat !== false;
      this.smartPunctuationSpaceConvert = preferences.smart_punctuation_space_convert === true;
      this.punctuationLock = preferences.punctuation_lock;
      this.wordCharacter = preferences.word_character.enabled
        ? preferences.word_character.keys : 'disabled';
      this.clearSmartPunctuationRepeat();
      this.toolbar = preferences.floating_toolbar;
      this.nineKeyMode = preferences.touch_keyboard_layout === 'nine_key'
        && preferences.scheme === 'quanpin';
      this.soundPlayer.configure(KeyboardFormFactor.isDesktop()
        ? KeySoundPolicy.settings(preferences.plugins) : KEY_SOUNDS_OFF, stateRoot);
      this.musicPlayer.configure(KeyboardFormFactor.isDesktop()
        ? MusicPolicy.settings(preferences.plugins) : MUSIC_OFF, stateRoot);
      this.typingEffectPreferences = KeyboardFormFactor.isDesktop()
        ? TypingEffectPolicy.settings(preferences.plugins) : TYPING_EFFECTS_OFF;
      this.typingEffects = this.typingEffectPreferences;
    } catch (error) {
      KeyboardLog.error(`host preparation unreadable: ${describeError(error)}`);
      return false;
    }
    // Before the session exists and after the options are known: dictionary maintenance takes an
    // exclusive lock that a live session's shared lock would block, so this is the one moment in
    // the keyboard's life when queued personal-dictionary edits can actually be applied. A failure
    // is not a reason to refuse to start — the queue keeps its entries and the next start retries.
    this.personalDictionarySettled = this.drainPersonalDictionary(options);
    try {
      const created: CreateResponse = JSON.parse(client.create(options)) as CreateResponse;
      if (!created.ok) {
        KeyboardLog.error(`session refused: ${created.error}`);
        return false;
      }
      this.handle = created.value.session;
      KeyboardLog.info(`session ${this.handle} created`);
      this.refreshTypingEffects();
      this.schedulePersonalDictionaryDrain();
      this.scheduleDictionarySnapshotPoll();
      return true;
    } catch (error) {
      // A response that will not parse means the ABI moved underneath us. Refusing to start is
      // better than driving a session whose replies we cannot read.
      KeyboardLog.error(`session reply unreadable: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Applies whatever the settings window queued for the personal dictionary.
   *
   * Words imported from a file cannot be written when they are chosen: the settings window has no
   * way to take the Engine's maintenance lock while this process is holding a session, and telling
   * the user "busy" would make importing a file depend on whether they happened to have the
   * keyboard open. They are queued instead, and this is where the queue is paid.
   *
   * Everything here is best effort. The store keeps entries it could not apply, so a failure costs
   * a delay rather than the import; refusing to start the keyboard over it would turn a dictionary
   * problem into no keyboard at all.
   */
  private drainPersonalDictionary(options: string): boolean {
    try {
      const reply: PersonalDictionarySyncResponse =
        JSON.parse(client.personalDictionarySync(options)) as PersonalDictionarySyncResponse;
      if (!reply.ok) {
        KeyboardLog.error(`personal dictionary queue not applied; it will be retried: ${reply.error}`);
        return false;
      }
      return reply.value.pending_count === 0;
    } catch (error) {
      KeyboardLog.error(`personal dictionary queue unavailable: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Continue a large import in four-entry turns while the editor is idle.
   *
   * The native queue deliberately limits one synchronization to four edits. Releasing the event
   * loop between batches keeps a 128-entry import from monopolising the input method, while the
   * composition/local-mode guards make the brief Engine-session rebuild invisible to typed text.
   */
  private schedulePersonalDictionaryDrain(): void {
    if (this.personalDictionarySettled || this.personalDictionaryTimer >= 0 || this.handle === 0) {
      return;
    }
    this.personalDictionaryTimer = setTimeout((): void => {
      this.personalDictionaryTimer = -1;
      if (this.composing() || this.localMode !== 'none') {
        this.schedulePersonalDictionaryDrain();
        return;
      }
      this.restartIdleSession('personal dictionary queue');
    }, IDLE_DICTIONARY_RETRY_MS);
  }

  /** Poll the cross-process snapshot queue just as the Apple worker timer does. */
  private scheduleDictionarySnapshotPoll(): void {
    if (this.snapshotQueueTimer >= 0 || this.handle === 0) return;
    this.snapshotQueueTimer = setTimeout((): void => {
      this.snapshotQueueTimer = -1;
      if (this.composing() || this.localMode !== 'none') {
        this.scheduleDictionarySnapshotPoll();
        return;
      }
      if (this.dictionarySnapshotNeedsWork()) {
        this.restartIdleSession('snapshot queue');
        return;
      }
      this.scheduleDictionarySnapshotPoll();
    }, IDLE_DICTIONARY_RETRY_MS);
  }

  /** Querying queue state takes no Engine maintenance lock and never acknowledges terminal state. */
  private dictionarySnapshotNeedsWork(): boolean {
    if (this.snapshotOptions === undefined || this.stateDirectory.length === 0) return false;
    try {
      const action: SnapshotQueueStateAction = {
        operation: 'state', directory: `${this.stateDirectory}/dictionary-snapshots`,
        options: this.snapshotOptions, acknowledge: false
      };
      const reply: SnapshotQueueStateReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueStateReply;
      if (!reply.ok) {
        KeyboardLog.error(`snapshot queue poll refused: ${reply.error ?? 'snapshot_unavailable'}`);
        return false;
      }
      const request: SnapshotQueueRequestState | null | undefined = reply.value?.request;
      return request !== undefined && request !== null
        && (request.status === 'queued' || request.status === 'preparing');
    } catch (error) {
      KeyboardLog.error(`snapshot queue poll unavailable: ${describeError(error)}`);
      return false;
    }
  }

  /** Rebuild an idle Engine session without changing the editor-visible input mode. */
  private restartIdleSession(reason: string): void {
    const attached: inputMethodEngine.InputClient | undefined = this.inputClient;
    const english: boolean = this.englishMode;
    const nineKey: boolean = this.nineKeyMode;
    const revision: number = this.preferencesRevision;
    this.stop();
    if (!this.start(this.resourceDirectory, this.stateDirectory, this.systemDark)) {
      KeyboardLog.error(`${reason} left the session unavailable`);
      return;
    }
    this.englishMode = english;
    this.nineKeyMode = nineKey;
    if (attached !== undefined && this.inputClient === attached) {
      this.apply(client.focus(this.handle, true));
      this.refreshTypingEffects();
      this.applyView(client.setEnglishMode(this.handle, english));
      this.applyView(client.setNineKeyMode(this.handle, nineKey));
    }
    // Any rebuild reads the document afresh, whatever it was for, so the surfaces follow whenever the stored preferences moved underneath it.
    if (this.preferencesRevision !== revision) {
      this.announcePreferencesReloaded();
    }
  }

  /** Tell the view and the ability that what they derive from the preferences has to be read again. */
  private announcePreferencesReloaded(): void {
    if (this.onViewPreferencesReloaded !== undefined) {
      this.onViewPreferencesReloaded();
    }
    if (this.onPreferencesReloaded !== undefined) {
      this.onPreferencesReloaded();
    }
  }

  /**
   * Pick up what the settings page saved since this session was built.
   *
   * The settings page is another process writing the same document. Windows pushes a change to the running server and polls as a fallback (`WM_APPLY_IME_CONFIG`, `TIMER_ID_CONFIG_SYNC`); here the keyboard compares the stored revision each time an editor takes focus, which is the first moment anyone could type with the new settings, and costs one small read rather than a timer. A composition in progress is left alone and the next focus picks the change up.
   */
  reloadChangedPreferences(): void {
    if (this.handle === 0 || this.composing() || this.localMode !== 'none') {
      return;
    }
    const revision: number = this.storedRevision(this.stateDirectory);
    if (revision < 0 || revision === this.preferencesRevision) {
      return;
    }
    KeyboardLog.info(`preferences moved from revision ${this.preferencesRevision} to ${revision}`);
    this.restartIdleSession('preferences reload');
  }

  /** The revision of the stored preference document, or -1 when it cannot be read. */
  private storedRevision(stateRoot: string): number {
    try {
      const loaded: SnapshotResponse = JSON.parse(client.loadPreferences(stateRoot)) as SnapshotResponse;
      return loaded.ok ? loaded.value.revision : -1;
    } catch (error) {
      KeyboardLog.error(`preferences revision unreadable: ${describeError(error)}`);
      return -1;
    }
  }

  /** Whether the Engine's output is being converted to traditional. */
  usesTraditional(): boolean {
    return this.traditional;
  }

  /** What the user chose for the keyboard's appearance and scheme. */
  preferences(): KeyboardPreferences {
    return this.appearance;
  }

  /** The keyboard palette of the global theme in one mode: the custom design when the custom theme carries one, else the resolved palette, else the native tokens. */
  keyboardSkin(dark: boolean): KeyboardSkin {
    const title: string = this.themeTitle(this.globalThemeId);
    if (this.globalThemeId === 'custom' && this.customDesign !== null) {
      return KeyboardSkin.fromDesign(title, this.customDesign, dark);
    }
    const theme: ResolvedTheme | null = dark ? this.themeDark : this.themeLight;
    return KeyboardSkin.fromTheme(this.globalThemeId, title, theme === null ? null : theme.keyboard, dark);
  }

  /** The skin a keyboard-side surface (voice, emoji, handwriting, menus) wears: its own setting, then the app mode, then whatever the keyboard is in; a theme with a fixed appearance overrides all three. */
  surfaceSkin(surfaceTheme: string): KeyboardSkin {
    return this.keyboardSkin(GlobalTheme.surfaceDark(this.fixedAppearance(),
      KeyboardSkin.resolveDark(surfaceTheme, this.appearance.appMode, this.appearance.dark)));
  }

  /** The 2in1 candidate context menu's colours. Menus derive from the candidate palette rather than the keyboard's, in the mode `menu_theme` picks the way `surfaceSkin` resolves the keyboard-side surfaces. */
  menuColors(): CandidateColors {
    const dark: boolean = GlobalTheme.surfaceDark(this.fixedAppearance(),
      KeyboardSkin.resolveDark(this.appearance.menuTheme, this.appearance.appMode, this.appearance.dark));
    const theme: ResolvedTheme | null = dark ? this.themeDark : this.themeLight;
    return GlobalTheme.candidateColors(theme === null ? null : theme.candidate, dark);
  }

  /**
   * One card per global theme for the native picker, in the shared catalog order.
   *
   * A built-in theme is shown in the mode it fixes, `system` in the current mode, and `custom` as the user's design, or its base when there is no design. Ids, titles and colours all come from the catalog; nothing here keeps a copy.
   */
  themeSkins(dark: boolean): KeyboardSkin[] {
    if (this.themeCatalog.length === 0) {
      return [this.keyboardSkin(dark)];
    }
    return this.themeCatalog.map((entry: ThemeCatalogEntry): KeyboardSkin => {
      if (entry.id === this.globalThemeId) {
        return this.keyboardSkin(GlobalTheme.surfaceDark(entry.appearance, dark));
      }
      if (entry.id === 'custom') {
        if (this.customDesign !== null) {
          return KeyboardSkin.fromDesign(entry.title, this.customDesign, dark);
        }
        const base: ThemeCatalogEntry | null = GlobalTheme.entry(this.themeCatalog, this.customThemeBase);
        return base === null ? KeyboardSkin.fromTheme(entry.id, entry.title, null, dark)
          : KeyboardSkin.fromTheme(entry.id, entry.title, base.keyboard,
            GlobalTheme.surfaceDark(base.appearance, dark));
      }
      return KeyboardSkin.fromTheme(entry.id, entry.title, entry.keyboard,
        GlobalTheme.surfaceDark(entry.appearance, dark));
    });
  }

  /** Store a global theme picked on the keyboard and redraw every surface from it. */
  selectGlobalTheme(id: string): boolean {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('global_theme', id);
    if (!this.changePreferences(changes)) {
      return false;
    }
    if (this.preparedPreferences !== undefined) {
      this.preparedPreferences.global_theme = id;
      this.applyAppearance(this.preparedPreferences);
    }
    this.refreshToolbar();
    return true;
  }

  /** Whether candidates carry translations, which the function panel's 译 tile and the strip's 译 pill show as a switch. */
  showsCandidateTranslations(): boolean {
    return this.preparedPreferences !== undefined && this.preparedPreferences.candidate_translations;
  }

  /**
   * Candidate translations on or off from the keyboard, through the same document the settings page writes.
   *
   * The session takes the change at once, since the translation query reads its preferences. What the host derived from the switch is derived again here: whether a gloss is drawn, and the line reserved for it, which moves the keyboard's height, so the view and the ability are told the way a settings reload tells them.
   */
  toggleCandidateTranslations(): boolean {
    const prepared: PreparedPreferences | undefined = this.preparedPreferences;
    if (prepared === undefined) {
      return false;
    }
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('candidate_translations', !prepared.candidate_translations);
    if (!this.changePreferences(changes)) {
      return prepared.candidate_translations;
    }
    prepared.candidate_translations = !prepared.candidate_translations;
    this.applyCandidateGloss(prepared);
    this.publishCurrentCandidates();
    this.announcePreferencesReloaded();
    return prepared.candidate_translations;
  }

  /** Whether fuzzy pinyin is on, which the function panel's 模糊音 tile shows as a switch. */
  usesFuzzyPinyin(): boolean {
    return this.fuzzyPinyin;
  }

  /**
   * Fuzzy pinyin on or off from the keyboard, with the selected rules kept.
   *
   * Saved before it is applied, the reverse of changePreferences: the store opts every rule in on the first enable, and the session has to be given that seeded set rather than the empty one that was sent, or the switch would read on while matching exactly until the next rebuild. The saved snapshot carries the revision the session is then given, so the two never disagree about what that revision holds.
   */
  toggleFuzzyPinyin(): boolean {
    if (this.handle === 0) {
      return this.fuzzyPinyin;
    }
    try {
      const loaded: SnapshotResponse =
        JSON.parse(client.loadPreferences(this.stateDirectory)) as SnapshotResponse;
      if (!loaded.ok) {
        KeyboardLog.error(`preferences unreadable: ${loaded.error}`);
        return this.fuzzyPinyin;
      }
      const snapshot: PreferencesSnapshot = loaded.value;
      const expected: number = snapshot.revision;
      const stored: Object | undefined = snapshot.preferences['fuzzy_pinyin'];
      const absent: PreparedFuzzyPinyin = { enabled: false, rules: [] };
      const current: PreparedFuzzyPinyin = stored === undefined ? absent : stored as PreparedFuzzyPinyin;
      // The record just parsed from disk with only the switch flipped: the rules and the seeding marker go back as they were read.
      current.enabled = !current.enabled;
      snapshot.preferences['fuzzy_pinyin'] = current;
      snapshot.revision = expected + 1;
      const saved: SnapshotResponse = JSON.parse(client.savePreferences(this.stateDirectory, expected,
        JSON.stringify(snapshot))) as SnapshotResponse;
      if (!saved.ok) {
        KeyboardLog.error(`preferences not persisted: ${saved.error}`);
        return this.fuzzyPinyin;
      }
      const applied: PrepareResponse =
        JSON.parse(client.updatePreferences(this.handle, JSON.stringify(saved.value))) as PrepareResponse;
      if (!applied.ok) {
        // On disk but not in the session: leaving the revision behind makes the next focus rebuild from the document.
        KeyboardLog.error(`preferences refused: ${applied.error}`);
      } else {
        this.preferencesRevision = saved.value.revision;
      }
      const fuzzy: PreparedFuzzyPinyin = saved.value.preferences['fuzzy_pinyin'] as PreparedFuzzyPinyin;
      this.fuzzyPinyin = fuzzy.enabled;
      if (this.preparedPreferences !== undefined) {
        this.preparedPreferences.fuzzy_pinyin = fuzzy;
      }
    } catch (error) {
      KeyboardLog.error(`preferences failed: ${describeError(error)}`);
    }
    return this.fuzzyPinyin;
  }

  /** Whether a gloss is drawn under the candidates and how many lines are kept for it, from the two switches and the providers that can answer. */
  private applyCandidateGloss(preferences: PreparedPreferences): void {
    this.candidateGlossEnabled = CandidateGlossLayoutPolicy.displays(
      this.candidateEnglishGloss, preferences.candidate_translations);
    const glossTargets: string[] = [preferences.translation_target_language];
    if (preferences.translation_secondary_language !== undefined
        && preferences.translation_secondary_language !== null
        && !glossTargets.includes(preferences.translation_secondary_language)) {
      glossTargets.push(preferences.translation_secondary_language);
    }
    const glossProviders: CandidateGlossProviderState = {
      customEnabled: preferences.custom_translation.enabled,
      customEndpoint: preferences.custom_translation.endpoint,
      niuTransEnabled: preferences.niutrans.enabled,
      niuTransAppId: preferences.niutrans.app_id,
      niuTransApiKey: preferences.niutrans.apikey,
      tencentEnabled: preferences.tencent_tmt.enabled,
      tencentSecretId: preferences.tencent_tmt.secret_id,
      tencentSecretKey: preferences.tencent_tmt.secret_key
    };
    this.candidateGlossRows = CandidateGlossLayoutPolicy.rows(this.candidateEnglishGloss,
      glossTargets, preferences.candidate_translations, glossProviders,
      this.installedOfflineGlosses(glossTargets));
  }

  private themeTitle(id: string): string {
    const entry: ThemeCatalogEntry | null = GlobalTheme.entry(this.themeCatalog, id);
    return entry === null ? id : entry.title;
  }

  /** Whether the shared privacy preference permits this host's private history file. */
  clipboardHistoryAllowed(): boolean {
    return ClipboardHistoryPreferencePolicy.canReadOrWrite(this.clipboardHistoryEnabled);
  }

  /**
   * Turns the platform's inputPattern into the traits EditorPolicy reasons about.
   *
   * The port keeps the platform constants out of EditorPolicy on purpose: they are declared in
   * @ohos.inputMethodEngine without values, so the numbers belong to the runtime and must not be
   * guessed. Translating here is what lets the policy stay plain TypeScript and testable.
   */
  private static traitsFor(pattern: number): EditorTraits {
    const password: boolean = pattern === inputMethodEngine.PATTERN_PASSWORD
      || pattern === inputMethodEngine.PATTERN_PASSWORD_SCREEN_LOCK
      || pattern === inputMethodEngine.PATTERN_PASSWORD_NUMBER
      || pattern === inputMethodEngine.PATTERN_NEW_PASSWORD;
    return {
      text: pattern === inputMethodEngine.PATTERN_TEXT
        || pattern === inputMethodEngine.PATTERN_NULL
        || pattern === inputMethodEngine.PATTERN_EMAIL
        || pattern === inputMethodEngine.PATTERN_URI
        || pattern === inputMethodEngine.PATTERN_USER_NAME,
      password: password,
      uri: pattern === inputMethodEngine.PATTERN_URI,
      email: pattern === inputMethodEngine.PATTERN_EMAIL,
      // A one-time code is typed, not composed, and a user name is not a sentence.
      noSuggestions: pattern === inputMethodEngine.PATTERN_ONE_TIME_CODE
        || pattern === inputMethodEngine.PATTERN_USER_NAME
    };
  }

  /**
   * Turns the platform's capitalize mode into the one the policy reasons about.
   *
   * Same reason traitsFor exists: the enumeration belongs to the runtime. It is optional in the
   * attribute, and an editor that does not declare one is asking for nothing, which is also what a
   * value this build has no name for means — the keyboard leaves the case alone rather than guessing.
   */
  private static capitalizationFor(
    mode: inputMethodEngine.CapitalizeMode | undefined): CapitalizationMode {
    if (mode === inputMethodEngine.CapitalizeMode.SENTENCES) {
      return CapitalizationMode.SENTENCES;
    }
    if (mode === inputMethodEngine.CapitalizeMode.WORDS) {
      return CapitalizationMode.WORDS;
    }
    if (mode === inputMethodEngine.CapitalizeMode.CHARACTERS) {
      return CapitalizationMode.ALL_CHARACTERS;
    }
    return CapitalizationMode.NONE;
  }

  /**
   * Whether the next English letter should arrive shifted.
   *
   * Two questions, both already ported and neither previously asked: what the editor wants, which
   * EditorPolicy reconciles with the kind of field it is — an address is never capitalized whatever
   * the platform says — and whether the text already there puts the caret at the start of a word or
   * a sentence. Only English has a case to shift; Chinese composition has none, and asking while the
   * Engine is composing would capitalize the pinyin rather than the result.
   */
  /**
   * Whether this editor asks for automatic capitals at all.
   *
   * Separate from the question below because false here means do nothing, not shift nothing: an
   * editor that wants no capitals has said nothing about the Shift the user pressed, and clearing it
   * on their behalf would drop a capital they asked for.
   */
  capitalizes(): boolean {
    return this.englishMode && this.capitalization !== CapitalizationMode.NONE;
  }

  shouldShiftNextLetter(): boolean {
    if (!this.englishMode || this.capitalization === CapitalizationMode.NONE) {
      return false;
    }
    if (this.capitalization === CapitalizationMode.ALL_CHARACTERS) {
      return true;
    }
    if (this.inputClient === undefined || this.editing.length > 0) {
      return false;
    }
    let context: string = '';
    try {
      context = this.inputClient.getForwardSync(MAX_CAPITALIZATION_CONTEXT_CHARACTERS);
    } catch (error) {
      // Without the text before the caret there is no way to tell a sentence start from the middle
      // of one, and shifting on a guess is the more visible mistake.
      KeyboardLog.error(`editor capitalization context unavailable: ${describeError(error)}`);
      return false;
    }
    return EnglishCapitalizationPolicy.shouldShift(this.capitalization, context);
  }

  /** Whether this editor should be composed through the Engine at all. */
  composes(): boolean {
    return EditorPolicy.useEngine(this.traits);
  }

  /**
   * Whether a shift-modified letter should reach the Engine as composition helpcode rather than as
   * an uppercase letter. It means something only while a quanpin or shuangpin composition is open.
   */
  entersHelpcode(shifted: boolean): boolean {
    return ChineseHelpcodePolicy.entersHelpcode(this.englishMode, shifted, this.editing,
      this.scheme, this.localMode);
  }

  /**
   * Whether a gloss is expected under the candidates, which is a setting rather than an arrival.
   *
   * The strip reserves room by this and not by whether an answer has come back: the networked ones
   * land over a few hundred milliseconds, and a width that follows them widens one chip at a time
   * and pushes the rest along. Off means no room is kept and the chips are as wide as their words.
   */
  glossesExpected(): boolean {
    return this.glossRows() > 0;
  }

  /** Fixed horizontal rows reserved before asynchronous candidate glosses arrive; Korean always keeps one for the 훈음 of its Hanja rows. */
  glossRows(): number {
    return CandidateGlossLayoutPolicy.schemeRows(this.candidateGlossRows, this.schemeName === 'korean');
  }

  /** True when the Engine is composing English rather than Chinese. */
  isEnglish(): boolean {
    return this.englishMode;
  }

  /**
   * True in the Windows Ctrl+Shift+E mode: English, but a hardware letter spells a word from the English candidate list instead of going straight to the editor.
   *
   * The Engine side is the same dedicated-English flag either way; what differs is only whether a hardware key reaches it. The touch keyboard always sends its letters through, so on a phone the two are one mode.
   */
  isEnglishCandidates(): boolean {
    return this.englishMode && this.englishCandidates;
  }

  /** Whether punctuation is locked to Chinese, which the Windows host keeps even in English mode (`ResolvePunctuationOpen`). */
  punctuationLockedChinese(): boolean {
    return this.punctuationLock === 'chinese';
  }

  /**
   * Ctrl+Shift+E, as `IsEnglishModeToggleKey` → `SetEnglishInputMode` + `ClearState` on Windows.
   *
   * Unlike the Shift tap it commits nothing: the TSF side answers this chord with `FUNCTION_CANCEL`, and the Engine drops its composition when the dedicated-English flag flips. Leaving the mode goes back to Chinese, as the macOS host's `toggleDedicatedEnglishMode:` does.
   */
  toggleEnglishCandidates(): boolean {
    if (this.handle === 0) {
      return this.isEnglishCandidates();
    }
    const enabled: boolean = !this.isEnglishCandidates();
    if (this.composing() && this.englishMode === enabled) {
      // Direct English with a local-mode composition open: the flag does not flip, so the Engine would keep it.
      this.cancel();
    }
    this.englishCandidates = enabled;
    this.englishMode = enabled;
    this.manualLanguageGeneration = this.editorGeneration;
    this.modeScope.remember(this.englishMode);
    this.applyView(client.setEnglishMode(this.handle, this.englishMode));
    return enabled;
  }

  /** True when the editor now attached needs the full letter face instead of a nine-key grid. */
  prefersFullFace(): boolean {
    return EditorPolicy.prefersFullFace(this.traits);
  }

  /** Japanese romaji owns the unmodified hardware minus key; temporary R mode does not. */
  isJapanese(): boolean {
    return !this.englishMode && this.schemeName === 'japanese';
  }

  /** Whether letters go to the Korean Hangul automaton: the Korean scheme with neither English nor a local utility mode taking the keys. */
  isKorean(): boolean {
    return KoreanCompositionPolicy.selected(this.schemeName, this.englishMode, this.localMode);
  }

  /** Whether the scheme in force writes ASCII marks whatever the punctuation switch says: Korean and Vietnamese (`uses_chinese_punctuation`). */
  writesAsciiPunctuation(): boolean {
    const scheme: number = this.rulesScheme();
    return scheme >= 0 && !SchemeTraits.usesChinesePunctuation(scheme);
  }

  /** Whether the scheme in force writes half-width whatever the width switch says: Korean and Vietnamese (`widens_full_width`). */
  private keepsHalfWidth(): boolean {
    const scheme: number = this.rulesScheme();
    return scheme >= 0 && !SchemeTraits.widensFullWidth(scheme);
  }

  /** The Engine scheme number whose own key rules are in force, or -1 while English or a local utility mode takes the keys. */
  rulesScheme(): number {
    return SchemeCompositionPolicy.selectedRulesScheme(this.schemeName, this.englishMode, this.localMode);
  }

  /** Whether the scheme's openable candidate list (the composing Korean syllable's Hanja, the Zhuyin list) is on the strip, read from what the Engine last reported. */
  candidateListOpen(): boolean {
    const view: EngineView | undefined = this.lastView;
    return view !== undefined && SchemeCompositionPolicy.listOpen(
      SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode),
      view.candidate_list_open, view.candidates.length);
  }

  /** Which mode-switch gestures the user left bound, as the shared document has them. */
  modeBindings(): ModeBindings {
    return this.bindings;
  }

  /** Which unmodified hardware pair selects the first/last Han character. */
  wordCharacterBinding(): string {
    return this.wordCharacter;
  }

  /** Whether punctuation keys write Chinese punctuation, which the touch strip and the function panel show as a switch. */
  usesChinesePunctuation(): boolean {
    return this.chinesePunctuation;
  }

  /**
   * Chinese punctuation or ASCII, which the toolbar button and the Windows Ctrl+. binding share.
   *
   * Written as a preference rather than as a live session mode because that is how this host has always changed it: the same value drives the toolbar face and the settings page, and two mechanisms for one switch is how they come to disagree.
   *
   * The switch moves only once the store has taken the write: the reload that follows skips this session's own revision, so a refused write that still flipped the flag would write punctuation the settings page says is off. Returns the state now in effect.
   */
  togglePunctuation(): boolean {
    if (!this.writePreference('chinese_punctuation', !this.chinesePunctuation)) {
      return this.chinesePunctuation;
    }
    this.chinesePunctuation = !this.chinesePunctuation;
    this.clearSmartPunctuationRepeat();
    return this.chinesePunctuation;
  }

  /**
   * Insert a hardware key's printable ASCII character as its fullwidth form, the Windows double-byte mode. Returns false with fullwidth off, so the key goes back to the application.
   */
  widen(ascii: number): boolean {
    if (!this.fullWidth || ascii < 0x20 || ascii > 0x7e || this.inputClient === undefined) {
      return false;
    }
    this.clearSmartPunctuationRepeat();
    this.insert(String.fromCharCode(ascii));
    return true;
  }

  /** Whether direct ASCII is written in its fullwidth form. */
  isFullWidth(): boolean {
    return this.fullWidth;
  }

  /**
   * Count a hardware character the keyboard handed to the application, as Windows counts the keys it does not eat (`stats_passthrough.h`). Only for an editor this keyboard composes for: a password field is not the user's typing to keep a record of.
   */
  recordPassthrough(character: number, ctrl: boolean, alt: boolean, logo: boolean): void {
    if (!this.composes() || !TypingStatisticsPolicy.countsPassthrough(character, ctrl, alt, logo)) {
      return;
    }
    this.recordTypingStatistics(String.fromCodePoint(character), this.typingSource());
  }

  /**
   * Count one press of a key for the key heatmap: a soft key's tap or a physical key going down, by its id from KeyIdPolicy, null for a key without one.
   *
   * Every key counts while statistics are on, the ones that compose and the ones handed to the application alike, in a number, phone or date field as much as in prose, except in a password field: its keys are not the user's typing to keep a record of. Only counts are kept, per key and local day, never the order or what was typed. The press is filed under the day it happened on, and the write waits for a batch, a new day, the timer or the editor letting go.
   */
  recordKey(id: string | null): void {
    if (id === null || !this.keyStatisticsEnabled || !this.keyStatisticsEditorKnown
      || EditorPolicy.excludesKeyStatistics(this.traits) || this.stateDirectory.length === 0) {
      return;
    }
    const due: KeyPressFlush[] = this.keyPresses.add(id, TypingStatisticsPolicy.day(new Date()));
    for (const flush of due) {
      this.sendKeyPresses(flush);
    }
    if (this.keyPresses.pending() === 0) {
      this.cancelKeyFlushTimer();
    } else if (this.keyFlushTimer < 0) {
      this.keyFlushTimer = setTimeout((): void => {
        this.keyFlushTimer = -1;
        this.flushKeyPresses();
      }, KeyPressBatch.FLUSH_INTERVAL_MS);
    }
  }

  /** Write whatever key presses are held, now: the editor let go, or the keyboard is going away. */
  private flushKeyPresses(): void {
    this.cancelKeyFlushTimer();
    const flush: KeyPressFlush | null = this.keyPresses.drain();
    if (flush !== null) {
      this.sendKeyPresses(flush);
    }
  }

  private cancelKeyFlushTimer(): void {
    if (this.keyFlushTimer >= 0) {
      clearTimeout(this.keyFlushTimer);
      this.keyFlushTimer = -1;
    }
  }

  private sendKeyPresses(flush: KeyPressFlush): void {
    if (this.stateDirectory.length === 0) return;
    const action: TypingStatisticsKeysAction = {
      operation: 'record_keys', day: flush.day, keys: flush.keys
    };
    const requestDocument: TypingStatisticsKeysRequest = {
      directory: this.stateDirectory, action: action
    };
    const generation: number = this.attributeGeneration;
    taskpool.execute(writeKeyPresses, JSON.stringify(requestDocument)).then((response: Object): void => {
      const value: Record<string, Object> = JSON.parse(response as string) as Record<string, Object>;
      if (value.ok !== true) {
        KeyboardLog.error('typing statistics key counts refused');
        return;
      }
      // The store drops a batch it is not counting and says so with recorded 0: statistics were turned off in settings while this editor kept focus. Stop holding presses until the next attach reads the switch again. A reply from an editor that has since let go says nothing about the one now focused, whose attach has already re-read the switch.
      const result: Record<string, Object> = value.value as Record<string, Object>;
      if ((result.recorded as number) === 0 && Object.keys(flush.keys).length > 0
        && generation === this.attributeGeneration) {
        this.keyStatisticsEnabled = false;
        this.cancelKeyFlushTimer();
        this.keyPresses.clear();
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`typing statistics key counts unavailable: ${describeError(error)}`);
    });
  }

  /**
   * Read the statistics switch again, off the UI thread. Asked on every focus, which is how a switch flipped in the settings application reaches a keyboard that is already running; turning it off drops whatever was held rather than writing it.
   */
  private refreshKeyStatistics(): void {
    if (this.stateDirectory.length === 0) return;
    taskpool.execute(readTypingStatisticsEnabled, this.stateDirectory).then((value: Object): void => {
      this.keyStatisticsEnabled = (value as number) === 1;
      if (!this.keyStatisticsEnabled) {
        this.cancelKeyFlushTimer();
        this.keyPresses.clear();
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`typing statistics switch unavailable: ${describeError(error)}`);
    });
  }

  /** Halfwidth ASCII or its fullwidth twin, which Ctrl+Shift+F toggles. */
  toggleCharacterWidth(): boolean {
    if (this.handle === 0) {
      return this.fullWidth;
    }
    this.applyView(client.setCharacterWidth(this.handle, !this.fullWidth));
    return this.fullWidth;
  }

  /** Whether the user wants a toolbar at all, and how big. */
  toolbarEnabled(): boolean {
    return this.toolbar.enabled;
  }

  toolbarScale(): number {
    return FloatingToolbarLayout.scale(this.toolbar.scale_percent / 100);
  }

  toolbarFontSize(): number {
    return FloatingToolbarLayout.fontSize(this.toolbar.font_size);
  }

  /** Whether the shared preference permits the badge shown after a Chinese/English switch. */
  modeBadgeEnabled(): boolean {
    return this.modeBadge;
  }

  toolbarComponents(): ToolbarComponents {
    return {
      englishMode: this.toolbar.english_mode, punctuation: this.toolbar.punctuation,
      fullwidth: this.toolbar.fullwidth, characterSet: this.toolbar.character_set,
      emoji: this.toolbar.emoji, handwriting: this.toolbar.handwriting !== false,
      screenKeyboard: this.toolbar.screen_keyboard, voice: this.toolbar.voice !== false,
      settings: this.toolbar.settings !== false
    };
  }

  /** Push the toolbar's face to whichever window is drawing it. */
  refreshToolbar(): void {
    if (this.onToolbar === undefined) {
      return;
    }
    const state: ToolbarState = {
      english: this.englishMode,
      temporaryEnglish: this.localMode === 'temporary_english' || this.isEnglishCandidates(),
      japanese: !this.englishMode && this.schemeName === 'japanese',
      korean: !this.englishMode && this.schemeName === 'korean',
      vietnamese: !this.englishMode && this.schemeName === 'vietnamese',
      capsLock: this.capsLock,
      // Korean and Vietnamese write ASCII punctuation whatever the switch says (`uses_chinese_punctuation`), and the face reports what is written.
      chinesePunctuation: this.chinesePunctuation && !this.writesAsciiPunctuation(),
      fullWidth: this.fullWidth, traditional: this.traditional
    };
    this.onToolbar(state, FloatingToolbarLayout.buttons(this.toolbarComponents()),
      this.toolbarFontSize(), this.toolbarScale(), this.toolbarSkin);
  }

  beginToolbarDrag(): void {
    if (this.onToolbarDragStart !== undefined) {
      this.onToolbarDragStart();
    }
  }

  setCapsLock(enabled: boolean): void {
    if (this.capsLock === enabled) {
      return;
    }
    this.capsLock = enabled;
    this.refreshToolbar();
  }

  dragToolbar(offsetX: number, offsetY: number): void {
    if (this.onToolbarDrag !== undefined) {
      this.onToolbarDrag(offsetX, offsetY);
    }
  }

  /**
   * What a toolbar button does.
   *
   * The three that are preferences are written back to the document as well as applied, because they
   * are the same three the settings page shows: a toolbar that changed them only for this session
   * would disagree with the page the moment either was looked at.
   */
  runToolbarButton(button: ToolbarButton): void {
    switch (button) {
      case ToolbarButton.INPUT_MODE:
        this.toggleLanguage();
        break;
      case ToolbarButton.PUNCTUATION:
        this.togglePunctuation();
        break;
      case ToolbarButton.FULL_WIDTH:
        this.toggleCharacterWidth();
        break;
      case ToolbarButton.CHARACTER_SET:
        this.toggleCharacterSet();
        break;
      case ToolbarButton.EMOJI:
        this.requestSurface(DesktopSurface.EMOJI);
        break;
      case ToolbarButton.SCREEN_KEYBOARD:
        this.requestSurface(DesktopSurface.SCREEN_KEYBOARD);
        break;
      case ToolbarButton.HANDWRITING:
        this.requestSurface(DesktopSurface.HANDWRITING);
        break;
      case ToolbarButton.VOICE:
        this.requestSurface(DesktopSurface.VOICE);
        break;
      default:
        this.openSettings();
        break;
    }
    this.refreshToolbar();
  }

  /**
   * Simplified output becomes traditional and back, saved as the preference the settings page shows. Shared by the toolbar button, Ctrl+Shift+F and the function panel's 繁 tile.
   *
   * The switch moves only once the store has taken the write: the reload that follows skips this session's own revision, so a refused write that still flipped the flag would convert output the settings page says is off. Returns the state now in effect.
   */
  toggleCharacterSet(): boolean {
    if (!this.writePreference('traditional_chinese_output', !this.traditional)) {
      return this.traditional;
    }
    this.traditional = !this.traditional;
    // The candidate bar shows converted text, so the list on screen follows the switch at once.
    this.publishCurrentCandidates();
    return this.traditional;
  }

  /** One value, through the same store the settings page writes, revision check and all. False when the store refused it. */
  private writePreference(key: string, value: Object): boolean {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set(key, value);
    return this.changePreferences(changes);
  }

  /**
   * Ask for a surface that is not the candidate list.
   *
   * On a phone these are opened from the keyboard's own key faces. A desktop has none, so the
   * toolbar asks instead, and the host decides which window it goes in and how big it has to be.
   * Asking for the one already open closes it, the way tapping its icon does on the keyboard.
   */
  private requestSurface(surface: DesktopSurface): void {
    // Toolbar surfaces belong to the 2-in-1 candidate window. Keep this invariant in the session as
    // well as at the toolbar/host boundary: a phone may have an attached physical keyboard, but it
    // must never acquire the desktop surface state or ask its fixed touch panel to become one.
    if (!KeyboardFormFactor.isDesktop()) {
      return;
    }
    this.setDesktopSurface(this.desktopSurface === surface ? DesktopSurface.NONE : surface);
  }

  /** Set a desktop face without toggle semantics, for hotkeys that open a face repeatedly. */
  setDesktopSurface(surface: DesktopSurface): void {
    if (!KeyboardFormFactor.isDesktop()) {
      return;
    }
    this.desktopSurface = surface;
    // The view first: the window is sized around what it drew, so drawing it second would leave one
    // frame of a keyboard-sized window with a candidate strip in it.
    if (this.onSurfaceChanged !== undefined) {
      this.onSurfaceChanged(this.desktopSurface);
    }
    if (this.onSurfaceRequested !== undefined) {
      this.onSurfaceRequested(this.desktopSurface);
    }
  }

  /** Which of them is open, for the view that has to draw it. */
  openSurface(): DesktopSurface {
    return this.desktopSurface;
  }

  /** Offers a hardware key to the open 2in1 emoji panel, which the Windows panel receives as its focused window. False when no panel is open to take it. */
  routePanelKey(key: EmojiPanelKey): boolean {
    if (!KeyboardFormFactor.isDesktop() || this.desktopSurface !== DesktopSurface.EMOJI
        || this.onPanelKey === undefined) {
      return false;
    }
    return this.onPanelKey(key);
  }

  /** Tell whichever window is drawing the mode badge what to put on it, at the floating toolbar's size and in the toolbar theme's palette (toolbar_theme, then theme, then the system), as the macOS badge wears them. */
  setModeBadge(english: boolean): void {
    if (this.onModeBadge !== undefined) {
      this.onModeBadge(english, this.toolbarFontSize(), this.toolbarScale(), this.toolbarSkin);
    }
  }

  /** Whether something is being spelled right now, which decides who a physical key belongs to. */
  composing(): boolean {
    return HardwareKeyRouter.composing(this.editing, this.phrasePrefix);
  }

  /** Tells the Engine which grid the user is typing on, so a 2 means ABC rather than a digit. */
  /** The scheme now in force, which decides whether the nine-key flag means anything. */
  useScheme(name: string, profileName: string | null = null): void {
    const glossRows: number = this.glossRows();
    this.schemeName = name;
    if (profileName !== null) {
      this.profileName = profileName;
    }
    // Scheme changes happen from the keyboard's own picker as well as the desktop toolbar. Keep
    // the persistent toolbar's language face in sync before the next key or button action.
    this.refreshToolbar();
    // Korean keeps a gloss line for the 훈음 whatever the switches say, so entering or leaving it can move the keyboard's height; the view and the ability are told the way the 译 switch tells them.
    if (this.glossRows() !== glossRows) {
      this.announcePreferencesReloaded();
    }
  }

  setNineKey(enabled: boolean): void {
    // The Engine's nine-key flag belongs to quanpin alone — Japanese has its own kana grid and the
    // other schemes have none, and it answers "nine-key mode requires the quanpin scheme" for any
    // of them. Asking anyway put a refusal in the log on every Japanese focus.
    const wanted: boolean = enabled && this.schemeName === 'quanpin';
    if (this.handle === 0 || this.nineKeyMode === wanted) {
      return;
    }
    this.nineKeyMode = wanted;
    this.applyView(client.setNineKeyMode(this.handle, wanted));
  }

  /** Flips the composing language and reports the mode now in force. */
  toggleLanguage(): boolean {
    if (this.handle === 0) {
      return this.englishMode;
    }
    this.finishCompositionBoundary(CompositionBoundary.MODE_SWITCH);
    this.englishMode = !this.englishMode;
    // The Shift tap and the toolbar button go between Chinese and direct English; out of the Ctrl+Shift+E mode they go back to Chinese.
    this.englishCandidates = false;
    this.manualLanguageGeneration = this.editorGeneration;
    // The user asked for this one, so it is the choice this application is remembered by.
    this.modeScope.remember(this.englishMode);
    this.applyView(client.setEnglishMode(this.handle, this.englishMode));
    return this.englishMode;
  }

  /** What this editor asked its return key to do, in the shared editor-action numbering. */
  returnAction(): number {
    return this.enterKey;
  }

  /** Ask the host to put the keyboard away, committing whatever was being composed first. */
  dismiss(): void {
    this.finishCompositionBoundary(CompositionBoundary.DEACTIVATE);
    if (this.onDismiss !== undefined) {
      this.onDismiss();
    }
  }

  private finishCompositionBoundary(boundary: CompositionBoundary): void {
    const action: CompositionBoundaryAction = CompositionBoundaryPolicy.action(
      this.composing(), this.schemeName === 'japanese', boundary,
      SchemeTraits.commitsOnBlur(this.rulesScheme()));
    if (action === CompositionBoundaryAction.FINISH_COMPOSITION) {
      this.finishComposition();
    } else if (action === CompositionBoundaryAction.COMMIT_RAW) {
      // As typed, but not learned: Windows learns an entered English word only on Enter, and its mode switch commits the keystroke buffer without learning (`_HandleToogleIMEMode`).
      this.command(InputCommand.COMMIT_RAW_WITHOUT_LEARNING);
    }
  }

  /**
   * What Return does when no composition took it first.
   *
   * One call covers both halves here. Android splits them — performEditorAction for GO, SEARCH and
   * the rest, a committed newline otherwise — and the ported ReturnKeyAction is that split. On this
   * platform sendKeyFunction takes the enter key type itself and the framework does the right thing
   * with every value, ENTER_KEY_TYPE_NEW_LINE included. Measured, not assumed: replacing this with
   * the split put insertText("\n") into a WebView textarea that ignored it, and the same key with
   * the same editor (enter=8) had produced a newline the moment before.
   */
  submit(): void {
    if (this.inputClient !== undefined) {
      this.inputClient.sendKeyFunction(this.enterKey);
    }
  }

  /** Begin one host-independent voice generation, so late platform callbacks can be rejected. */
  voiceStart(): number {
    if (this.handle === 0 || !this.voiceInputEnabled()) {
      return 0;
    }
    try {
      const response: VoiceGenerationResponse = JSON.parse(client.voiceStart(this.handle)) as VoiceGenerationResponse;
      return response.ok && Number.isSafeInteger(response.value) && response.value > 0
        ? response.value : 0;
    } catch (error) {
      KeyboardLog.error(`voice session unavailable: ${describeError(error)}`);
      return 0;
    }
  }

  voiceInputConfiguration(): VoiceInputConfiguration {
    return this.voiceInput;
  }

  /** The prepared host options the on-device recognizer reads the user's own words through, or null before the host has been prepared. */
  voiceHotwordOptions(): Object | null {
    return this.snapshotOptions ?? null;
  }

  /**
   * The text before the caret, as the polish panel's starting point.
   *
   * Apple polishes the selection; HarmonyOS gives an input method no way to read one, so this host
   * offers what it can read instead. See `AiPolishPolicy` for why that is the same intent rather
   * than a lesser one.
   */
  polishSource(): string {
    if (this.inputClient === undefined || this.composing()) return '';
    try {
      return AiPolishPolicy.source(this.inputClient.getForwardSync(MAX_POLISH_SOURCE_CHARACTERS));
    } catch (error) {
      KeyboardLog.error(`polish source unavailable: ${describeError(error)}`);
      return '';
    }
  }

  /** Whether a polish provider is configured, which is what decides if the entry is offered. */
  polishConfigured(): boolean {
    const config: VoiceInputConfiguration = this.voiceInput;
    const provider: string = (config.polish_provider ?? '').trim();
    return provider.length > 0;
  }

  /**
   * Rewrites one piece of text through this host's polisher.
   *
   * The polisher is the one the voice path already uses. It is the right one and not merely a
   * convenient one: it allows the newlines a rewritten paragraph contains, where the AI candidate
   * parser beside it rejects every control character — that parser is for candidate words, and
   * sending a rewritten message through it would silently drop every multi-line answer.
   *
   * Returns null rather than throwing. Every failure here is "no rewrite to show", and the panel
   * has one sentence for that; distinguishing a refused endpoint from a timed-out one would give
   * the user nothing they could act on differently.
   */
  async polish(text: string): Promise<string | null> {
    if (text.length === 0) return null;
    const generation: number = this.editorGeneration;
    const result: string | null = await this.polisher.polish(this.voiceInput, text);
    // The editor may have moved while the model was writing. A rewrite of text that is no longer
    // there is not a result, and offering it would invite the user to replace the wrong thing.
    if (generation !== this.editorGeneration) return null;
    return result !== null && AiPolishPolicy.usable(text, result) ? result : null;
  }

  /**
   * Puts a rewrite in place of the text it was made from.
   *
   * The text before the caret is read again here rather than trusted from when the panel opened:
   * a polish request takes seconds, and anything typed in the meantime would otherwise be deleted
   * and replaced by the rewrite of something else. A mismatch is refused and reported.
   */
  applyPolish(source: string, result: string): boolean {
    if (this.inputClient === undefined || this.composing()) return false;
    let current: string;
    try {
      current = this.inputClient.getForwardSync(MAX_POLISH_SOURCE_CHARACTERS);
    } catch (error) {
      KeyboardLog.error(`polish target unreadable: ${describeError(error)}`);
      return false;
    }
    const replacement: PolishReplacement | null =
      AiPolishPolicy.replacement(source, result, current);
    if (replacement === null) return false;
    try {
      this.deleteEditorBackwardSync(replacement.deleteCount);
    } catch (error) {
      KeyboardLog.error(`polish could not replace the text: ${describeError(error)}`);
      return false;
    }
    this.insertWithSource(replacement.insert, 'ai');
    return true;
  }

  voiceInputEnabled(): boolean {
    return VoiceInputConfigurationPolicy.enabled(this.voiceInput.enabled);
  }

  voiceCancel(): void {
    if (this.handle === 0) {
      return;
    }
    try {
      client.voiceCancel(this.handle);
    } catch (error) {
      KeyboardLog.error(`voice cancellation unavailable: ${describeError(error)}`);
    }
  }

  /** Accept only the active generation and never log the recognized text. */
  voiceApply(generation: number, text: string): string | null {
    if (this.handle === 0 || generation <= 0 || text.length === 0) {
      return null;
    }
    try {
      const response: VoiceTextResponse = JSON.parse(client.voiceApply(this.handle, generation, text)) as VoiceTextResponse;
      return response.ok ? response.value : null;
    } catch (error) {
      KeyboardLog.error(`voice result unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /** The framework hands over a fresh client on every inputStart. */
  attach(inputClient: inputMethodEngine.InputClient): void {
    const editorGeneration: number = ++this.editorGeneration;
    const attributeGeneration: number = ++this.attributeGeneration;
    this.pendingOwnEdits = 0;
    this.previewSupported = false;
    this.previewText = '';
    this.invalidateReplyContext();
    this.clearSmartPunctuationRepeat();
    this.inputClient = inputClient;
    this.editorAttributesKnown = false;
    this.updateMusic();
    this.keyStatisticsEditorKnown = false;
    this.refreshKeyStatistics();
    inputClient.getEditorAttribute().then((attribute: inputMethodEngine.EditorAttribute): void => {
      if (attributeGeneration !== this.attributeGeneration || this.inputClient !== inputClient) {
        return;
      }
      this.enterKey = attribute.enterKeyType;
      this.previewSupported = attribute.isTextPreviewSupported === true;
      this.traits = KeyboardSession.traitsFor(attribute.inputPattern);
      this.editorAttributesKnown = true;
      // A pack imported again under the same id, or removed, while another editor had focus is picked up here, as host-api re-stamps its packs on focus.
      this.soundPlayer.recheck();
      this.musicPlayer.recheck();
      this.updateMusic();
      this.keyStatisticsEditorKnown = true;
      this.capitalization = EditorPolicy.capitalizationMode(this.traits,
        KeyboardSession.capitalizationFor(attribute.capitalizeMode));
      // Says the keyboard reached a real editor, which nothing else in the log does. Verifying that
      // on a device meant reading the window manager's avoid-area traffic to infer it.
      KeyboardLog.info(`attached to editor: pattern=${attribute.inputPattern} `
        + `enter=${attribute.enterKeyType} capitalize=${this.capitalization}`);
      // Which application this editor belongs to, so the mode can be the one last chosen there.
      // Resolved with the attribute rather than at attach: the bundle name arrives with it.
      this.modeScope.activate(attribute.bundleName ?? null, this.modeScopeSetting);
      // A password field must never see a composition buffer, and a field that asked for no
      // suggestions has said it does not want one. Latin for an address, where Chinese would only
      // be in the way. That override is the editor's, not the user's, so it is never remembered.
      const english: boolean =
        this.modeScope.mode(this.defaultEnglish) || EditorPolicy.prefersLatin(this.traits);
      if (this.handle !== 0 && EditorPolicy.appliesDelayedLanguage(
        this.composing(), this.englishMode, english,
        this.manualLanguageGeneration >= editorGeneration)) {
        this.englishMode = english;
        this.applyView(client.setEnglishMode(this.handle, english));
      }
      if (EditorPolicy.prefersLatin(this.traits)) {
        // A password or address field takes its letters as typed, never through a candidate list.
        this.englishCandidates = false;
      }
      // A composition carried into this editor was rendered before its attribute said whether it takes preview text.
      this.syncPreview();
      if (this.onEditorChanged !== undefined) {
        this.onEditorChanged();
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`editor attribute unavailable: ${describeError(error)}`);
    });
    if (this.handle !== 0) {
      this.apply(client.focus(this.handle, true));
      this.refreshTypingEffects();
      this.applyView(client.setEnglishMode(this.handle, this.englishMode));
      this.applyView(client.setNineKeyMode(this.handle, this.nineKeyMode));
    }
    this.reloadChangedPreferences();
  }

  detach(): void {
    this.editorGeneration++;
    this.attributeGeneration++;
    this.manualLanguageGeneration = -1;
    this.pendingOwnEdits = 0;
    this.modeScope.deactivate();
    this.clearEnglishSuggestions();
    this.invalidateReplyContext();
    this.clearSmartPunctuationRepeat();
    this.cancelOnlineProviders();
    if (this.handle !== 0) {
      // Runtime focus(false) cancels whatever remains. Finish first while the old editor client is
      // still attached, so changing fields cannot silently discard what the user just typed.
      this.finishCompositionBoundary(CompositionBoundary.DEACTIVATE);
      this.apply(client.focus(this.handle, false));
    }
    this.previewSupported = false;
    this.previewText = '';
    this.inputClient = undefined;
    this.editorAttributesKnown = false;
    this.updateMusic();
    this.keyStatisticsEditorKnown = false;
    this.flushKeyPresses();
  }

  stop(): void {
    this.editorGeneration++;
    this.pendingOwnEdits = 0;
    this.invalidateReplyContext();
    this.clearSmartPunctuationRepeat();
    this.cancelOnlineProviders();
    // A candidate-window face belongs to the editor/session that opened it. Do not carry a 2-in-1
    // emoji, voice or screen-keyboard surface into the next focus or an idle Engine restart; on a
    // phone this is a no-op because desktop surfaces are fail-closed at setDesktopSurface().
    if (this.desktopSurface !== DesktopSurface.NONE) {
      this.setDesktopSurface(DesktopSurface.NONE);
    }
    if (this.personalDictionaryTimer >= 0) {
      clearTimeout(this.personalDictionaryTimer);
      this.personalDictionaryTimer = -1;
    }
    if (this.snapshotQueueTimer >= 0) {
      clearTimeout(this.snapshotQueueTimer);
      this.snapshotQueueTimer = -1;
    }
    if (this.handle !== 0) {
      client.destroy(this.handle);
      this.handle = 0;
    }
  }

  /** Final ability teardown: invalidate editor callbacks that may still be awaiting attributes. */
  shutdown(): void {
    this.stop();
    this.soundPlayer.shutdown();
    this.musicPlayer.shutdown();
    this.attributeGeneration++;
    this.inputClient = undefined;
    this.previewSupported = false;
    this.previewText = '';
    this.keyStatisticsEditorKnown = false;
    this.flushKeyPresses();
  }

  /** Invalidate host-side network work before the focused editor changes or the session dies. */
  private cancelOnlineProviders(): void {
    this.onlineEpoch++;
    this.onlineSignature = '';
    for (const request of this.onlineRequests) request.destroy();
    this.onlineRequests.clear();
    if (this.onlineTimer >= 0) {
      clearTimeout(this.onlineTimer);
      this.onlineTimer = -1;
    }
    this.translationEpoch++;
    this.translationSignature = '';
    for (const request of this.translationRequests) request.destroy();
    this.translationRequests.clear();
    if (this.translationTimer >= 0) {
      clearTimeout(this.translationTimer);
      this.translationTimer = -1;
    }
  }

  /** Invalidate asynchronous reply work without logging or retaining editor text. */
  invalidateReplyContext(notifyEditorChange: boolean = true): void {
    this.replyContextGeneration++;
    const request: http.HttpRequest | undefined = this.replyRequest;
    this.replyRequest = undefined;
    if (request !== undefined) request.destroy();
    this.replyResultEditorGeneration = -1;
    this.replyResultContextGeneration = -1;
    this.replyResultTexts = [];
    if (notifyEditorChange && this.onReplyContextChanged !== undefined) {
      this.onReplyContextChanged();
    }
  }

  /** Resize only the phone/tablet soft-keyboard panel; desktop surfaces have separate geometry. */
  resizeTouchKeyboard(heightVp: number): void {
    if (!KeyboardFormFactor.isDesktop() && this.onTouchHeight !== undefined) {
      this.onTouchHeight(heightVp);
    }
  }

  /** Reconcile a document mutation reported by the editor with keyboard-owned edit echoes. */
  editorTextChanged(): void {
    this.invalidateReplyContext();
    if (!EditorPolicy.isExternalTextChange(this.pendingOwnEdits)) {
      this.pendingOwnEdits--;
      if (this.onEditorChanged !== undefined) {
        this.onEditorChanged();
      }
      return;
    }
    // A host edit may clear the field, replace the document or move text around the active
    // composition. Commit what the user typed before the Engine's state can drift from the editor.
    this.finishCompositionBoundary(CompositionBoundary.DEACTIVATE);
    this.clearEnglishSuggestions();
    this.clearSmartPunctuationRepeat();
    this.clearSmartPunctuationSpace();
    if (this.onEditorChanged !== undefined) {
      this.onEditorChanged();
    }
  }

  /** A selection move changes contextual UI but does not itself discard an active composition. */
  editorSelectionChanged(): void {
    this.invalidateReplyContext();
    this.clearEnglishSuggestions();
    if (this.onEditorChanged !== undefined) {
      this.onEditorChanged();
    }
  }

  /** Insert through the editor while reserving the asynchronous text-change echo it produces. */
  private insertEditorText(text: string): void {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    if (this.previewText.length > 0 || this.commitSynchronously) {
      // The insertion takes the preview text's place. It has to land before the next preview update, which is synchronous, and an asynchronous insert gives no such order.
      try {
        this.insertEditorTextSync(text);
      } catch (error) {
        KeyboardLog.error(`editor insertion unavailable: ${describeError(error)}`);
      }
      return;
    }
    const generation: number = this.editorGeneration;
    this.pendingOwnEdits++;
    inputClient.insertText(text).catch((error: Object): void => {
      if (generation === this.editorGeneration && this.inputClient === inputClient
          && this.pendingOwnEdits > 0) {
        this.pendingOwnEdits--;
      }
      KeyboardLog.error(`editor insertion unavailable: ${describeError(error)}`);
    });
  }

  /** Synchronous insert variant for an atomic read/replace editor operation. */
  private insertEditorTextSync(text: string): void {
    if (this.inputClient === undefined) return;
    this.pendingOwnEdits++;
    try {
      this.inputClient.insertTextSync(text);
    } catch (error) {
      this.pendingOwnEdits--;
      throw new Error(describeError(error));
    }
    // An insertion replaces preview text and ends the preview.
    this.previewText = '';
  }

  /**
   * Bring the editor's preview text in line with the composition the Engine last reported.
   *
   * Every change is a document edit the editor echoes back like an insertion, so it is reserved the same way; otherwise the echo would read as the user editing the field and end the composition it came from.
   */
  private syncPreview(): void {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    const view: EngineView | undefined = this.lastView;
    const wanted: string = view === undefined ? '' : InlinePreeditPolicy.text(this.inlinePreeditStyle,
      KeyboardFormFactor.isDesktop(), this.previewSupported, view.editing_text, view.preedit,
      view.phrase_prefix ?? '', SchemeCompositionPolicy.drawsPreedit(SchemeCompositionPolicy.rulesScheme(
        view.scheme, view.dedicated_english, view.local_mode)));
    if (inputClient === undefined || wanted === this.previewText) return;
    const shown: string = this.previewText;
    this.pendingOwnEdits++;
    try {
      inputClient.setPreviewTextSync(wanted, PREVIEW_REPLACE_CURRENT);
      if (wanted.length === 0) inputClient.finishTextPreviewSync();
      this.previewText = wanted;
      return;
    } catch (error) {
      this.pendingOwnEdits--;
      KeyboardLog.error(`editor preview unavailable: ${describeError(error)}`);
    }
    // The editor refused preview text after saying it took it; stop offering it for this focus.
    this.previewSupported = false;
    this.previewText = '';
    if (shown.length === 0) return;
    // Letters of a composition must not stay behind in the document: end the preview and take them out, the way a cancelled composition leaves nothing.
    try {
      inputClient.finishTextPreviewSync();
      // Only when the letters are still there as plain text: an editor that dropped them with the preview must not lose what precedes them.
      if (inputClient.getForwardSync(shown.length) === shown) {
        this.deleteEditorBackwardSync(shown.length);
      }
    } catch (error) {
      KeyboardLog.error(`editor preview cleanup unavailable: ${describeError(error)}`);
    }
  }

  /** Delete through the editor while reserving the asynchronous text-change echo it produces. */
  private deleteEditorBackwardSync(count: number): void {
    if (this.inputClient === undefined) return;
    this.pendingOwnEdits++;
    try {
      this.inputClient.deleteBackwardSync(count);
    } catch (error) {
      this.pendingOwnEdits--;
      throw new Error(describeError(error));
    }
  }

  /** Resume one durable cloud snapshot before a shared Engine session takes the read lock. */
  private processDictionarySnapshot(options: Object, stateRoot: string): void {
    const staging: string = `${stateRoot}/dictionary-snapshot-staging`;
    try {
      if (!fs.accessSync(staging)) fs.mkdirSync(staging, true);
      const action: SnapshotQueueProcessAction = {
        operation: 'process', directory: `${stateRoot}/dictionary-snapshots`,
        staging_root: staging, options: options
      };
      const reply: SnapshotQueueProcessReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueProcessReply;
      if (!reply.ok) {
        KeyboardLog.error(`snapshot queue refused: ${reply.error ?? 'snapshot_unavailable'}`);
      }
    } catch (error) {
      // The durable request remains queued/preparing. The idle poll retries the same UUID.
      KeyboardLog.error(`snapshot queue unavailable: ${describeError(error)}`);
    }
  }

  /**
   * Run one shared dictionary-management action without violating the Engine maintenance lock.
   *
   * Reads can coexist with the live session. A write briefly recreates an idle session so the
   * Engine has exclusive dictionary access, but never cancels a composition to obtain it.
   */
  dictionary(actionDocument: string, resources: string, stateRoot: string): string {
    let action: DictionaryAction;
    try {
      const decoded: DictionaryAction | null = JSON.parse(actionDocument) as DictionaryAction | null;
      if (decoded === null || typeof decoded.operation !== 'string') {
        return JSON.stringify({ ok: false, error: 'invalid dictionary action' });
      }
      action = decoded;
    } catch (error) {
      return JSON.stringify({ ok: false, error: 'invalid dictionary action' });
    }
    const decision = DictionaryMaintenancePolicy.decide(action.operation, this.composing());
    if (!decision.allowed) {
      return JSON.stringify({ ok: false, error: decision.error });
    }
    const maintenance: boolean = decision.maintenance;
    const hadSession: boolean = this.handle !== 0;
    const english: boolean = this.englishMode;
    const nineKey: boolean = this.nineKeyMode;
    if (maintenance && hadSession) {
      this.stop();
    }
    let result: string;
    try {
      const bootstrap: PrepareRequest = { resources: resources, state_root: stateRoot };
      const prepared: PrepareResponse =
        JSON.parse(client.prepareHost(JSON.stringify(bootstrap))) as PrepareResponse;
      if (!prepared.ok) {
        return JSON.stringify(prepared);
      }
      const request: DictionaryRequest = { options: prepared.value, action: action };
      // Same request shape, different store. The queued route writes the personal-dictionary file
      // beside the preferences and is drained in bounded idle turns; the Engine route needs the
      // maintenance lock this method has just taken.
      if (decision.queued) {
        result = client.personalDictionaryRequest(JSON.stringify(request));
        this.personalDictionarySettled = false;
        this.schedulePersonalDictionaryDrain();
      } else {
        result = client.dictionary(JSON.stringify(request));
      }
    } catch (error) {
      KeyboardLog.error(`dictionary request unavailable: ${describeError(error)}`);
      result = JSON.stringify({ ok: false, error: 'dictionary request failed' });
    } finally {
      if (maintenance && hadSession) {
        if (this.start(resources, stateRoot, this.systemDark)) {
          this.englishMode = english;
          this.nineKeyMode = nineKey;
          if (this.inputClient !== undefined) {
            this.apply(client.focus(this.handle, true));
            this.refreshTypingEffects();
            this.applyView(client.setEnglishMode(this.handle, english));
            this.applyView(client.setNineKeyMode(this.handle, nineKey));
          }
        } else {
          KeyboardLog.error('dictionary maintenance left the session unavailable');
        }
      }
    }
    return result;
  }

  /** Whether the key did anything. The touch keyboard ignores the answer; the hardware path hands a declined letter back to the application. */
  press(ascii: number, shift: boolean): boolean {
    // Checked before the repeat arming is cleared: the space that converts is the key immediately
    // after the mark, so this is the one press where both armings are live at once.
    if (this.convertPunctuationSpace(ascii)) {
      return true;
    }
    this.clearSmartPunctuationRepeat();
    if (!this.composes()) {
      // Straight to the editor, with no composition and nothing learned. A password or a one-time
      // code is not something an input method should be holding, let alone remembering.
      this.insert(shift ? String.fromCharCode(ascii).toUpperCase() : String.fromCharCode(ascii));
      return true;
    }
    if (this.handle === 0) {
      // Deliberately without the character: hilog is readable by tooling, and what the user types
      // does not belong in it. The refusal itself is the diagnostic.
      KeyboardLog.error('key ignored: no session');
      return false;
    }
    this.tapFeedback();
    const before: string = this.editing;
    const handled: boolean = this.apply(client.character(this.handle, ascii, shift));
    // A letter that leaves the composition exactly as it was is the one failure this path cannot
    // otherwise report: the key was claimed, so the application never sees it, and the Engine
    // answered `ok`, so apply() has nothing to complain about. The result is a key that silently
    // does nothing — which is what happened on a 2in1, where hardware keys compose nothing while
    // the same press() from the touch keyboard composes normally.
    //
    // What is logged is the shape of the failure, never the key: a character in hilog is the user's
    // text. The scheme and the local mode are the two host-side inputs that decide whether a letter
    // should have composed at all, and both are already public state.
    const consumed: boolean = handled || this.editing !== before;
    // A capital with nothing composed is declined by design and handed back to the application, so only a lower-case letter that composed nothing is a failure worth reporting.
    const capital: boolean = ascii >= 0x41 && ascii <= 0x5a;
    if (!consumed && before.length === 0 && !capital) {
      KeyboardLog.error(`composing key produced no composition: scheme=${this.schemeName}`
        + ` local=${this.localMode} english=${this.englishMode} candidates=${this.currentCandidates.length}`
        + ` shift=${shift} caps=${this.capsLock}`);
    }
    return consumed;
  }

  /** Route an ASCII punctuation key through the shared editor-context policy. */
  punctuation(ascii: number): void {
    if (this.handle === 0 || this.inputClient === undefined) return;
    let preceding: number = 0;
    try {
      // Harmony's getForwardSync returns the bounded text immediately before the cursor.
      preceding = SmartPunctuationContext.precedingCodePoint(InlinePreeditPolicy.beforePreview(
        this.inputClient.getForwardSync(2 + this.previewText.length), this.previewText));
    } catch (error) {
      // Editor context is optional; zero deliberately falls back to Engine punctuation.
      KeyboardLog.error(`editor punctuation context unavailable: ${describeError(error)}`);
    }
    const now: number = Date.now();
    if (SmartPunctuationRepeatPolicy.shouldReplace(
      this.smartPunctuationSnapshot, ascii, preceding, now, this.editorGeneration,
      this.smartPunctuation, this.smartPunctuationRepeat, this.composing(),
      this.currentCandidates.length)) {
      const chinese: string | null = SmartPunctuationRepeatPolicy.chineseMark(ascii);
      if (chinese !== null) {
        try {
          // deleteBackwardSync(1) removes the one scalar just before the caret. The preceding
          // scalar was checked above, so a replacement cannot consume unrelated text.
          this.deleteEditorBackwardSync(1);
          this.insertEditorTextSync(chinese);
          this.recordTypingStatistics(chinese, this.typingSource());
          this.clearSmartPunctuationRepeat();
          this.refreshEnglishSuggestions();
          return;
        } catch (error) {
          KeyboardLog.error(`repeated punctuation replacement unavailable: ${describeError(error)}`);
          this.clearSmartPunctuationRepeat();
        }
      }
    }

    let raw: string = client.punctuationWithContext(this.handle, ascii, preceding);
    let completion: PairedPunctuationCompletion | null = null;
    let committed: string | null | undefined = null;
    try {
      const response: Response = JSON.parse(raw) as Response;
      if (response.ok) {
        // Rewritten before apply() commits it, so the editor receives the opening quote and the completion below closes it.
        const reopened: string | null | undefined = PairedPunctuationPolicy.reopenQuote(
          response.value.commit, ascii, this.appearance.pairedPunctuation);
        if (reopened !== response.value.commit) {
          response.value.commit = reopened;
          raw = JSON.stringify(response);
        }
        committed = response.value.commit;
      }
      completion = response.ok
        ? PairedPunctuationPolicy.completion(response.value.commit,
          this.appearance.pairedPunctuation) : null;
    } catch (error) {
      KeyboardLog.error(`paired punctuation response unavailable: ${describeError(error)}`);
    }
    this.apply(raw);
    // PunctuationAscii intentionally has no Engine commit while idle. In that route the host must
    // write the literal mark itself; this is also the state from which a repeated smart mark can
    // be replaced on the next press.
    if ((committed === null || committed === undefined || committed.length === 0)
        && this.inputClient !== undefined && !this.composing()
        && this.currentCandidates.length === 0) {
      try {
        // Korean and Vietnamese punctuation is half-width ASCII in either character width (`widens_full_width`).
        const literal: string = FullWidthInputPolicy.output(String.fromCharCode(ascii),
          this.fullWidth && !this.keepsHalfWidth()) ?? String.fromCharCode(ascii);
        this.insertEditorTextSync(literal);
        this.recordTypingStatistics(literal, this.typingSource());
        this.refreshEnglishSuggestions();
        if (this.canArmSmartPunctuationRepeat(ascii, preceding)) {
          this.smartPunctuationSnapshot = SmartPunctuationRepeatPolicy.snapshot(
            ascii, literal, now, this.editorGeneration);
        } else {
          this.clearSmartPunctuationRepeat();
        }
      } catch (error) {
        KeyboardLog.error(`ASCII punctuation insertion unavailable: ${describeError(error)}`);
        this.clearSmartPunctuationRepeat();
      }
    } else {
      this.clearSmartPunctuationRepeat();
    }
    // Armed off the Engine's own commit rather than off the key that was pressed: what reaches the
    // editor is what the space would be correcting, and the two differ whenever the punctuation
    // policy rewrote the mark.
    this.smartPunctuationSpaceSnapshot = SmartPunctuationSpacePolicy.arm(
      committed, completion !== null, this.smartPunctuation, this.smartPunctuationSpaceConvert,
      this.editorGeneration);
    if (completion === null || this.inputClient === undefined) return;
    try {
      this.insertEditorTextSync(completion.closing);
      this.recordTypingStatistics(completion.closing, this.typingSource());
      this.refreshEnglishSuggestions();
      if (completion.opening === 0x3c) {
        const balanced: string = client.balancePairedPunctuationAfterAutoClose(
          this.handle, completion.opening);
        const balanceResponse: NativeReply<Object> = JSON.parse(balanced) as NativeReply<Object>;
        if (!balanceResponse.ok) {
          KeyboardLog.error(`book-title punctuation balance refused: ${balanceResponse.error}`);
        }
      }
      this.inputClient.moveCursorSync(inputMethodEngine.Direction.CURSOR_LEFT);
    } catch (error) {
      KeyboardLog.error(`paired punctuation completion unavailable: ${describeError(error)}`);
    }
  }

  private command(command: InputCommand): boolean {
    if (this.handle === 0) {
      return false;
    }
    return this.apply(client.command(this.handle, command));
  }

  commitHighlighted(): void {
    if (this.translationPage !== null) {
      this.choose(this.translationHighlight);
      return;
    }
    this.command(InputCommand.SELECT_HIGHLIGHTED);
  }

  hasHighlightedTranslation(): boolean {
    if (this.translationPage !== null) return false;
    return this.currentCandidates.some((candidate: Candidate): boolean =>
      candidate.highlighted && typeof candidate.translation === 'string'
      && candidate.translation.length > 0);
  }

  /** What a hardware `'`, `;`, digit or `+` is being typed into; see `HardwareSpelling`. */
  hardwareSpelling(): HardwareSpelling {
    return {
      localMode: this.localMode,
      editing: this.editing,
      caret: this.editingCaret,
      wubi: this.schemeName === 'wubi',
      microsoftShuangpin: this.lastView?.microsoft_shuangpin === true,
      englishCandidates: this.isEnglishCandidates(),
      spellingSymbols: this.lastView?.spelling_symbols ?? '',
    };
  }

  hasHighlightedCandidate(): boolean {
    return this.currentCandidates.some((candidate: Candidate): boolean => candidate.highlighted);
  }

  /** Ctrl+Enter commits the resolved translation without changing the user's composition text. */
  commitTranslation(): void {
    if (this.translationPage !== null) return;
    const candidate: Candidate | undefined = this.currentCandidates.find(
      (entry: Candidate): boolean => entry.highlighted && typeof entry.translation === 'string'
        && entry.translation.length > 0);
    if (candidate === undefined || candidate.translation === undefined
        || candidate.translation === null) {
      return;
    }
    const translations: string[] = TranslationSensePolicy.split(candidate.translation);
    if (translations.length === 0) return;
    if (translations.length > 1) {
      this.translationPage = translations;
      this.translationHighlight = 0;
      this.publishTranslationPage();
      return;
    }
    this.cancel();
    this.insertWithSource(translations[0], 'translation');
  }

  backspace(): void {
    const handled: boolean = this.command(InputCommand.BACKSPACE);
    if (!BackspaceHoldPolicy.deletesEditor(handled) || this.inputClient === undefined) {
      return;
    }
    try {
      this.deleteEditorBackwardSync(1);
      this.refreshEnglishSuggestions();
    } catch (error) {
      KeyboardLog.error(`editor backspace unavailable: ${describeError(error)}`);
    }
  }

  /** Throw the composition away without committing it, which is what 重输 means. */
  cancel(): void {
    if (this.translationPage !== null) {
      this.exitTranslationPage();
      return;
    }
    this.command(InputCommand.CANCEL);
  }

  /**
   * Throw the whole composition away, for a gesture that means to clear it rather than step back. In a scheme whose first MSIME_CANCEL keeps the composition (`cancel_keeps_composition`: it closes a Korean Hanja or Zhuyin list, or takes a Vietnamese word back to its keys; msime_client.h) a second drops it too; anywhere else this is cancel.
   */
  discardComposition(): void {
    const keeps: boolean = this.translationPage === null
      && SchemeTraits.cancelKeepsComposition(this.rulesScheme());
    this.cancel();
    if (keeps && this.composing()) {
      this.cancel();
    }
  }

  /**
   * MSIME_CONVERT_HANJA: list the Hanja of the composing Korean syllable, or close the open list and keep the syllable composing; in Zhuyin the same command opens or closes the list over the conversion. False when the Engine declined, which it does for a lone jamo and with nothing composing.
   */
  convertHanja(): boolean {
    return this.command(InputCommand.CONVERT_HANJA);
  }

  /** Commit the spelling as typed rather than any candidate. */
  commitRaw(): void {
    this.command(InputCommand.COMMIT_RAW);
  }

  /** Commit the highlighted candidate, or the reading when the scheme has no converted candidate. */
  finishComposition(): boolean {
    return this.command(InputCommand.FINISH_COMPOSITION);
  }

  /**
   * A hardware key the composition has no use for: finish the composition, then type the key after it.
   *
   * The Windows host finalizes and then lets the application insert the key, so the character is typed as the key made it, halfwidth and unconverted. Here the keyboard inserts it itself, after the commit, because releasing the key would leave its order against the commit to the editor.
   */
  commitThenType(ascii: number): void {
    this.commitThenInsert(String.fromCharCode(ascii));
  }

  /** `commitThenType` for text the keyboard chose rather than a key's own character: a Zhuyin symbol-layer mark whose ASCII key the Dachen editor would take as a phonetic or tone key. */
  commitThenInsert(text: string): void {
    this.finishComposition();
    if (this.inputClient === undefined) return;
    this.insertEditorText(text);
    this.recordTypingStatistics(text, this.typingSource());
  }

  /**
   * Korean, Zhuyin and Vietnamese: commit the open composition ahead of a key that goes on to do its own work (Return, a caret key, Delete).
   *
   * The commit is written synchronously, so it is in the document before the key that ended it reaches the editor or the application; left to the asynchronous insert, a Return could break the line ahead of the syllable it was meant to follow.
   */
  finishBeforeKey(): void {
    this.commitSynchronously = true;
    this.finishComposition();
    this.commitSynchronously = false;
  }

  /** Commit Japanese kana as entered, without forcing the first converted candidate. */
  commitReading(): boolean {
    return this.command(InputCommand.COMMIT_READING);
  }

  /** Space starts Japanese conversion, then advances and wraps without committing. False leaves Space to its normal commit, which is also what a lone Fallback row (the raw composition) needs. */
  convertJapanese(): boolean {
    const firstSource: number = this.currentCandidates.length > 0 ? this.currentCandidates[0].source : -1;
    if (this.scheme !== JAPANESE_SCHEME || this.editing.length === 0
        || !JapaneseSpacePolicy.converts(this.currentCandidates.length, firstSource)) {
      return false;
    }
    if (this.japaneseConversionIndex < 0
        || this.japaneseConversionReading !== this.editing) {
      this.japaneseConversionIndex = 0;
      this.japaneseConversionReading = this.editing;
      return true;
    }
    const next: number = this.japaneseConversionIndex + 1;
    const wraps: boolean = next >= this.currentCandidates.length;
    const wanted: number = wraps ? 0 : next;
    if (!this.command(wraps ? InputCommand.FIRST_CANDIDATE : InputCommand.NEXT_CANDIDATE)) {
      return false;
    }
    this.japaneseConversionIndex = wanted;
    return true;
  }

  /** Return commits the conversion only after Space; otherwise it preserves the kana reading. */
  commitJapanese(): boolean {
    if (this.scheme !== JAPANESE_SCHEME || this.editing.length === 0) {
      return false;
    }
    const converted: boolean = this.japaneseConversionActive();
    this.japaneseConversionIndex = -1;
    this.japaneseConversionReading = '';
    return converted ? this.command(InputCommand.SELECT_HIGHLIGHTED) : this.commitReading();
  }

  japaneseConversionActive(): boolean {
    return this.japaneseConversionIndex >= 0
      && this.japaneseConversionReading === this.editing;
  }

  moveLeft(): void {
    this.command(InputCommand.MOVE_LEFT);
  }

  moveRight(): void {
    this.command(InputCommand.MOVE_RIGHT);
  }

  /**
   * Home and End go to the ends of the whole candidate list while there are candidates, as the Windows source's `FUNCTION_MOVE_PAGE_TOP`/`BOTTOM` do and the other hosts' shared `FirstCandidate`/`LastCandidate` actions now do; with none on screen they move the caret in the spelling instead.
   */
  moveHome(): void {
    if (this.moveTranslationHighlight(-TranslationSensePolicy.MAX_SENSES)) return;
    this.command(this.currentCandidates.length > 0 ? InputCommand.FIRST_CANDIDATE : InputCommand.MOVE_HOME);
  }

  moveEnd(): void {
    if (this.moveTranslationHighlight(TranslationSensePolicy.MAX_SENSES)) return;
    this.command(this.currentCandidates.length > 0 ? InputCommand.LAST_CANDIDATE : InputCommand.MOVE_END);
  }

  deleteForward(): void {
    this.command(InputCommand.DELETE_FORWARD);
  }

  backspaceSegment(): void {
    this.command(InputCommand.BACKSPACE_SEGMENT);
  }

  moveLeftSegment(): void {
    this.command(InputCommand.MOVE_LEFT_SEGMENT);
  }

  moveRightSegment(): void {
    this.command(InputCommand.MOVE_RIGHT_SEGMENT);
  }

  nextPage(): void {
    // A translation page is never more than one page: it holds at most MAX_SENSES, which is the largest page size.
    if (this.translationPage !== null) return;
    this.command(InputCommand.NEXT_PAGE);
  }

  previousPage(): void {
    if (this.translationPage !== null) return;
    this.command(InputCommand.PREVIOUS_PAGE);
  }

  nextCandidate(): void {
    if (this.moveTranslationHighlight(1)) return;
    this.command(InputCommand.NEXT_CANDIDATE);
  }

  previousCandidate(): void {
    if (this.moveTranslationHighlight(-1)) return;
    this.command(InputCommand.PREVIOUS_CANDIDATE);
  }

  /** Moves the highlight on the Ctrl+Enter translation page, if that page is up; false otherwise, so the key goes on to the Engine. */
  private moveTranslationHighlight(delta: number): boolean {
    const translations: string[] | null = this.translationPage;
    if (translations === null) return false;
    this.translationHighlight = KeyboardGeometry.bounded(
      this.translationHighlight + delta, 0, translations.length - 1);
    this.publishTranslationPage();
    return true;
  }

  /**
   * Puts the candidates back as they were before Ctrl+Enter, as the Windows host's `ExitTranslationCandidateMode` does. The composition was not touched while the page was up, so the Engine's own list is still the one to show.
   */
  private exitTranslationPage(): void {
    if (this.translationPage === null) return;
    this.translationPage = null;
    this.translationHighlight = 0;
    this.publishCurrentCandidates();
  }

  choose(index: number): void {
    if (this.translationPage !== null) {
      const translation: string | undefined = this.translationPage[index];
      if (translation === undefined) return;
      this.translationPage = null;
      this.command(InputCommand.CANCEL);
      this.insertWithSource(translation, 'translation');
      return;
    }
    if (this.handle === 0) {
      return;
    }
    this.apply(client.select(this.handle, this.generation, index));
  }

  /** Commit the first or last Han scalar of the highlighted candidate. */
  selectEdge(edge: CandidateTextEdge): void {
    if (this.handle === 0) return;
    const candidate: Candidate | undefined = this.currentCandidates.find(
      (entry: Candidate): boolean => entry.highlighted);
    if (candidate === undefined) return;
    const raw: string = client.selectEdge(this.handle, this.generation,
      this.currentCandidates.indexOf(candidate), edge === CandidateTextEdge.FIRST ? 0 : 1);
    let response: Response;
    try {
      response = JSON.parse(raw) as Response;
    } catch (error) {
      KeyboardLog.error(`candidate edge response unavailable: ${describeError(error)}`);
      return;
    }
    if (!response.ok) {
      KeyboardLog.error(`candidate edge refused: ${response.error}`);
      return;
    }
    const commit: string | null | undefined = response.value.commit;
    this.apply(raw);
    if (commit !== null && commit !== undefined && commit.length > 0) return;
    const fallback: string | null = CandidateTextPolicy.extractHanCharacter(candidate.text, edge);
    if (fallback === null) return;
    this.apply(client.command(this.handle, InputCommand.CANCEL));
    this.insertWithSource(fallback, this.typingSource());
  }

  private apply(raw: string): boolean {
    // Every key the translation page does not answer itself ends up here, and on Windows every such key first puts the candidates back and then does what it would have done without the page (`ExitTranslationCandidateMode`). Left up, the page swallowed every later render, so a letter lengthened a composition the strip no longer showed and Enter committed under translations that stayed on screen.
    this.exitTranslationPage();
    let response: Response;
    try {
      response = JSON.parse(raw) as Response;
    } catch (error) {
      // Silence here would look exactly like a key that did nothing, which is the hardest kind of
      // failure to chase on a device. The reply itself stays out of the log: it carries candidates,
      // and candidates are the user's text.
      KeyboardLog.error(`unreadable engine reply: ${describeError(error)}`);
      return false;
    }
    if (!response.ok) {
      KeyboardLog.error(`engine refused: ${response.error}`);
      return false;
    }
    const value: EngineValue = response.value;
    // Bounded before it reaches the surface: a diagnostic is a message, not a place to print a
    // stack trace into a keyboard four rows tall.
    this.editing = value.view.editing_text;
    this.phrasePrefix = value.view.phrase_prefix ?? '';
    this.editingCaret = value.view.caret_position;
    this.scheme = value.view.scheme;
    this.diagnostic = InputDiagnosticPolicy.normalize(
      value.diagnostic === undefined ? null : value.diagnostic);
    const commit: string | null | undefined = value.commit;
    if (commit !== null && commit !== undefined && commit.length > 0
        && this.inputClient !== undefined) {
      // The Engine commits simplified; whether the user wanted traditional is a host decision, the
      // same boundary the Apple and Android hosts draw.
      const output: string = this.asTraditional(commit);
      this.insertEditorText(output);
      // What the V, / and @ modes generated is not the user's typing: an evaluated sum or an expanded command template would count characters nobody typed.
      if (value.commit_context?.typing_statistics !== false) {
        this.recordTypingStatistics(output, this.typingSource());
      }
      if (!this.soundsSilencedByEditor()) {
        this.soundPlayer.commit();
        this.typingEffect(TYPING_EFFECT_COMMIT);
      }
    }
    this.render(value.view);
    return value.handled === true;
  }

  /** For the entry points whose reply is a bare view. */
  private applyView(raw: string): void {
    this.exitTranslationPage();
    let response: ViewResponse;
    try {
      response = JSON.parse(raw) as ViewResponse;
    } catch (error) {
      KeyboardLog.error(`unreadable engine view: ${describeError(error)}`);
      return;
    }
    if (!response.ok) {
      KeyboardLog.error(`engine refused: ${response.error}`);
      return;
    }
    this.render(response.value);
  }

  private diagnostic: string = '';
  private fullWidth: boolean = false;
  private bindings: ModeBindings = DEFAULT_MODE_BINDINGS;
  private feedbackSettings: FeedbackSettings = KeyboardFeedback.DEFAULTS;
  // The global theme as last resolved, one answer per mode; null draws the native tokens.
  private preparedPreferences: PreparedPreferences | undefined = undefined;
  private themeCatalog: ThemeCatalogEntry[] = [];
  private themeLight: ResolvedTheme | null = null;
  private themeDark: ResolvedTheme | null = null;
  private globalThemeId: string = 'system';
  private customThemeBase: string = 'system';
  private customDesign: CustomKeyboardSkin | null = null;
  private vibratorFailed: boolean = false;
  private traditional: boolean = false;
  private chinesePunctuation: boolean = true;
  private smartPunctuation: boolean = true;
  private smartPunctuationRepeat: boolean = true;
  // Off by default, like the rest of the family on the Windows baseline: it changes a
  // character the user already saw land.
  private smartPunctuationSpaceConvert: boolean = false;
  private smartPunctuationSpaceSnapshot: SmartPunctuationSpaceSnapshot | null = null;
  private punctuationLock: string = 'follow';
  private wordCharacter: string = 'disabled';
  private smartPunctuationSnapshot: SmartPunctuationRepeatSnapshot | null = null;
  private toolbar: PreparedFloatingToolbar = {
    enabled: true, english_mode: true, punctuation: true, fullwidth: true, character_set: true,
    emoji: true, screen_keyboard: false, scale_percent: 100, font_size: 20
  };
  private toolbarSkin: ToolbarSkin = ToolbarSkinPolicy.base(GlobalTheme.candidateColors(null, false));
  private onlineTimer: number = -1;
  private onlineEpoch: number = 0;
  private readonly onlineRequests: Set<http.HttpRequest> = new Set<http.HttpRequest>();
  private onlineSignature: string = '';
  private translationTimer: number = -1;
  private translationEpoch: number = 0;
  private readonly translationRequests: Set<http.HttpRequest> = new Set<http.HttpRequest>();
  private translationSignature: string = '';
  private readonly translationCache: Map<string, string> = new Map<string, string>();
  private readonly translationNegativeCache: Map<string, number> = new Map<string, number>();
  private editing: string = '';
  /** The phrase piece the runtime holds ahead of the reading; it can outlive the reading, see HardwareKeyRouter.composing. */
  private phrasePrefix: string = '';
  private japaneseConversionIndex: number = -1;
  private japaneseConversionReading: string = '';
  /** Set by finishBeforeKey while its commit must reach the editor before the key that follows it. */
  private commitSynchronously: boolean = false;
  // Where the segment keys have put the caret. Drawn by the candidate window, which is the
  // only place it is visible: the editor sees a committed string, not a composition.
  private editingCaret: number = 0;
  private currentCandidates: Candidate[] = [];
  private translationPage: string[] | null = null;
  private translationHighlight: number = 0;
  private lastView: EngineView | undefined = undefined;
  private scheme: number = 0;
  private localMode: string = 'none';
  /** Revision of the preference document the session was last built from; -1 before the first. */
  private preferencesRevision: number = -1;
  /** Set by Ctrl+Shift+E; only meaningful while `englishMode` is on. */
  private englishCandidates: boolean = false;

  private render(view: EngineView): void {
    this.generation = view.generation;
    this.currentCandidates = view.candidates.slice();
    this.lastView = view;
    this.syncPreview();
    if (this.japaneseConversionIndex >= 0
        && view.editing_text !== this.japaneseConversionReading) {
      this.japaneseConversionIndex = -1;
      this.japaneseConversionReading = '';
    }
    if (this.translationPage !== null) return;
    const localModeChanged: boolean = this.localMode !== view.local_mode;
    this.localMode = view.local_mode;
    // Here rather than in apply(), because both paths end up here and only one of them went through
    // apply(): a reply that is a bare view — which is what every mode switch answers with — left
    // this behind, so the toolbar's 半 never became 全 and the next tap asked for the same thing
    // again.
    this.fullWidth = view.character_width === 'Fullwidth';
    const active: boolean = HardwareKeyRouter.composing(view.editing_text, view.phrase_prefix ?? '');
    if (active !== this.composedLast) {
      this.composedLast = active;
      if (this.onComposition !== undefined) {
        this.onComposition(active);
      }
    }
    const reading: string = KeyboardSession.drawnReading(view);
    // Korean's only candidates are the Hanja of the composing syllable, and their annotation is the 훈음 (msime_client.h).
    const hanja: boolean = KoreanCompositionPolicy.active(view.scheme, view.dedicated_english, view.local_mode);
    const entries: CandidateEntry[] = [];
    for (const candidate of view.candidates) {
      entries.push({
        text: this.asTraditional(candidate.text),
        highlighted: candidate.highlighted,
        badge: CandidatePresentationPolicy.badge(candidate.source),
        fixedPosition: candidate.fixed_position,
        source: candidate.source,
        translation: candidate.translation ?? null,
        // Only the part of the code still untyped, and only where that means something. The policy
        // decides; this just supplies what it reasons about.
        hint: WubiCodeHintPolicy.hint(candidate.code, view.preedit, this.wubiCodeHint, view.scheme,
          view.local_mode, view.answered_by_pinyin_fallback),
        annotation: CandidateGlossPolicy.annotation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          candidate.translation ?? null, this.candidateGlossEnabled),
        annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          candidate.translation ?? null, this.candidateGlossEnabled),
        hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation)
      });
    }
    if (this.onChanged !== undefined) {
      this.onChanged({
        candidates: entries,
        // 已选的那一段领在读音前面，与来源把 word_for_creating_word 拼在读音前面是同一件事。
        editing: (view.phrase_prefix ?? '') + reading,
        editingCaret: KeyboardSession.drawnCaret(view, reading) + (view.phrase_prefix ?? '').length,
        phrasePrefix: view.phrase_prefix ?? '',
        spellings: view.nine_key_spellings,
        scheme: view.scheme,
        shuangpinProfile: view.shuangpin_profile,
        localMode: view.local_mode,
        english: view.dedicated_english,
        diagnostic: this.diagnostic,
        page: view.page,
        pageCount: view.page_count,
        candidateListOpen: this.candidateListOpen()
      });
    }
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, reading, view.page, view.page_count));
    }
    if (this.onCandidateCount !== undefined) {
      this.onCandidateCount(entries.length);
    }
    if (localModeChanged) {
      this.refreshToolbar();
    }
    // Candidate annotations and online providers are display-only work. Both run after the Engine
    // view is published, and native result application rechecks the exact generation.
    this.scheduleOnlineProviders();
    this.scheduleTranslations();
    // Every typed letter and every backspace arrives here, which is the one place that sees the
    // editor change in direct English as well as in a composition.
    this.refreshEnglishSuggestions();
  }

  private publishCurrentCandidates(): void {
    const view: EngineView | undefined = this.lastView;
    if (view === undefined || this.onChanged === undefined) return;
    const hanja: boolean = KoreanCompositionPolicy.active(view.scheme, view.dedicated_english, view.local_mode);
    const entries: CandidateEntry[] = view.candidates.map((candidate: Candidate): CandidateEntry => ({
      text: this.asTraditional(candidate.text), highlighted: candidate.highlighted,
      badge: CandidatePresentationPolicy.badge(candidate.source),
      fixedPosition: candidate.fixed_position, source: candidate.source,
      translation: candidate.translation ?? null,
      hint: WubiCodeHintPolicy.hint(candidate.code, view.preedit, this.wubiCodeHint, view.scheme,
        view.local_mode, view.answered_by_pinyin_fallback),
      annotation: CandidateGlossPolicy.annotation(
        CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
        candidate.translation ?? null, this.candidateGlossEnabled),
      annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
        CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
        candidate.translation ?? null, this.candidateGlossEnabled),
      hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation)
    }));
    const reading: string = KeyboardSession.drawnReading(view);
    this.onChanged({
      candidates: entries, editing: (view.phrase_prefix ?? '') + reading,
      editingCaret: KeyboardSession.drawnCaret(view, reading) + (view.phrase_prefix ?? '').length,
      phrasePrefix: view.phrase_prefix ?? '',
      spellings: view.nine_key_spellings,
      scheme: view.scheme, shuangpinProfile: view.shuangpin_profile, localMode: view.local_mode,
      english: view.dedicated_english, diagnostic: this.diagnostic,
      page: view.page, pageCount: view.page_count, candidateListOpen: this.candidateListOpen()
    });
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, reading, view.page, view.page_count));
    }
    if (this.onCandidateCount !== undefined) this.onCandidateCount(entries.length);
  }

  /** The composition the strip draws: for Korean, Zhuyin and Vietnamese editing_text is only the keys behind the composition, so there the written text itself is drawn. */
  private static drawnReading(view: EngineView): string {
    return SchemeCompositionPolicy.reading(
      SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode),
      view.editing_text, view.preedit);
  }

  /** The caret within drawnReading: the Engine's own for a spelling, the end of a composition the scheme keeps the caret at the end of (a Vietnamese caret_position is a byte offset into a word that is not ASCII, so it is never used as an index). */
  private static drawnCaret(view: EngineView, reading: string): number {
    return SchemeCompositionPolicy.caret(
      SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode),
      view.caret_position, reading);
  }

  /** A horizontal candidate window on a machine with its own keys is as wide as its whole page; a vertical list is as wide as its widest row. A 2in1 window that shows the composition line also makes room on it for the brand mark and the page indicator. */
  private candidateWidthVp(entries: CandidateEntry[], editing: string, page: number, pageCount: number): number {
    const settings: KeyboardPreferences = this.preferences();
    if (!KeyboardFormFactor.isDesktop()) {
      return CandidateWidthPolicy.widthVp(entries, editing, settings.candidateFontSize,
        settings.candidatePreeditFontSize, settings.candidateMinWidthVp);
    }
    const base: number = settings.candidateLayout !== 'vertical'
      ? CandidateWidthPolicy.rowWidthVp(entries, editing, settings.candidateFontSize,
        settings.candidatePreeditFontSize, KeyboardMetrics.CANDIDATE_PADDING_VP,
        CandidateNumberFontPolicy.size(settings.candidateFontSize), settings.candidateMinWidthVp)
      : CandidateWidthPolicy.widthVp(entries, editing, settings.candidateFontSize,
        settings.candidatePreeditFontSize, settings.candidateMinWidthVp);
    return settings.candidateShowPreedit
      ? CandidatePagerPolicy.panelWidthVp(base, editing, settings.candidatePreeditFontSize, page, pageCount,
        CandidateWidthPolicy.MAX_WIDTH_VP, CandidateLogoPolicy.slotWidthVp(
          CandidateLogoPolicy.visible(KeyboardFormFactor.isDesktop(), settings.candidateShowPreedit)))
      : base;
  }

  private publishTranslationPage(): void {
    const translations: string[] | null = this.translationPage;
    if (translations === null || this.onChanged === undefined) return;
    const view: EngineView | undefined = this.lastView;
    const entries: CandidateEntry[] = translations.map((text: string, index: number): CandidateEntry => ({
      text: text, hint: '', annotation: '', annotationIsTranslation: false, hunEum: '',
      highlighted: index === this.translationHighlight, badge: '',
      fixedPosition: 0, source: 0, translation: null
    }));
    this.onChanged({
      candidates: entries,
      editing: view?.editing_text ?? this.editing,
      editingCaret: view?.caret_position ?? this.editingCaret, phrasePrefix: '', spellings: [],
      scheme: view?.scheme ?? this.scheme, shuangpinProfile: view?.shuangpin_profile ?? this.profileName,
      localMode: view?.local_mode ?? this.localMode,
      english: view?.dedicated_english ?? this.englishMode, diagnostic: this.diagnostic,
      page: 0, pageCount: 1, candidateListOpen: false
    });
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, view?.editing_text ?? this.editing, 0, 1));
    }
    if (this.onCandidateCount !== undefined) this.onCandidateCount(translations.length);
  }

  private scheduleOnlineProviders(): void {
    if (this.handle === 0) return;
    let reply: NativeReply<OnlineQuery | null>;
    try {
      reply = JSON.parse(client.onlineQuery(this.handle)) as NativeReply<OnlineQuery | null>;
    } catch (error) {
      KeyboardLog.error(`online query unavailable: ${describeError(error)}`);
      return;
    }
    if (!reply.ok || reply.value === null) {
      this.onlineSignature = '';
      return;
    }
    const query: OnlineQuery = reply.value;
    const signature: string = OnlineCandidatePolicy.signature(query);
    if (signature === this.onlineSignature) return;
    this.onlineSignature = signature;
    if (this.onlineTimer >= 0) clearTimeout(this.onlineTimer);
    const epoch: number = ++this.onlineEpoch;
    const handle: number = this.handle;
    const requestSignature: string = signature;
    this.onlineTimer = setTimeout((): void => {
      this.onlineTimer = -1;
      void this.fetchOnline(handle, JSON.stringify(query), query, epoch, requestSignature);
    }, OnlineCandidatePolicy.QUIET_INTERVAL_MS);
  }

  private async fetchOnline(handle: number, queryDocument: string, query: OnlineQuery,
    epoch: number, requestSignature: string): Promise<void> {
    if (epoch !== this.onlineEpoch || handle !== this.handle) return;
    let complete: boolean = true;
    let aiQuery: OnlineQuery = query;
    let aiDocument: string = queryDocument;
    if (query.cloud_candidates && query.cloud_eligible) {
      const applied: boolean = await this.fetchCloud(handle, queryDocument, epoch);
      complete = applied;
      // Applying a cloud result advances Engine's generation. Re-read the exact query before
      // sending AI so both optional providers can contribute without one becoming stale.
      if (applied && handle === this.handle && epoch === this.onlineEpoch) {
        try {
          const current: NativeReply<OnlineQuery | null> = JSON.parse(
            client.onlineQuery(handle)) as NativeReply<OnlineQuery | null>;
          if (current.ok && current.value !== null) {
            aiQuery = current.value;
            aiDocument = JSON.stringify(current.value);
          } else {
            complete = false;
            this.releaseOnlineSignatureIfCurrent(requestSignature, epoch, handle);
            return;
          }
        } catch {
          complete = false;
          this.releaseOnlineSignatureIfCurrent(requestSignature, epoch, handle);
          return;
        }
      }
    }
    if (epoch !== this.onlineEpoch || handle !== this.handle) return;
    const assistant = aiQuery.ai_assistant;
    if (assistant?.enabled === true && aiQuery.ai_eligible) {
      complete = await this.fetchAi(handle, aiDocument, assistant.candidate_limit, epoch) && complete;
    }
    if (!complete) this.releaseOnlineSignatureIfCurrent(requestSignature, epoch, handle);
  }

  private releaseOnlineSignatureIfCurrent(requestSignature: string, requestEpoch: number,
    requestHandle: number): void {
    if (OnlineCandidatePolicy.shouldReleaseAfterFailure(requestSignature, this.onlineSignature,
      requestEpoch, this.onlineEpoch, requestHandle, this.handle)) this.onlineSignature = '';
  }

  private async fetchCloud(handle: number, queryDocument: string, epoch: number): Promise<boolean> {
    let url: string;
    try {
      const reply: NativeReply<string> = JSON.parse(
        client.cloudRequestUrl(queryDocument)) as NativeReply<string>;
      if (!reply.ok || typeof reply.value !== 'string') return false;
      url = reply.value;
    } catch (error) {
      KeyboardLog.error(`cloud query unavailable: ${describeError(error)}`);
      return false;
    }
    const request = http.createHttp();
    this.onlineRequests.add(request);
    try {
      const response: http.HttpResponse = await request.request(url, {
        method: http.RequestMethod.GET,
        header: { Accept: 'application/json' },
        connectTimeout: 2500,
        readTimeout: 8000
      });
      if (epoch !== this.onlineEpoch || handle !== this.handle || response.responseCode < 200
        || response.responseCode >= 300 || typeof response.result !== 'string'
        || !OnlineCandidatePolicy.acceptsCloudBody(response.result)) return false;
      const applied: NativeReply<OnlineApplyValue> = JSON.parse(client.applyCloudResponse(
        handle, queryDocument, response.result)) as NativeReply<OnlineApplyValue>;
      if (applied.ok && applied.value.applied) this.render(applied.value.view);
      return applied.ok && applied.value.applied;
    } catch (error) {
      KeyboardLog.error(`cloud candidate unavailable: ${describeError(error)}`);
      return false;
    } finally {
      this.onlineRequests.delete(request);
      request.destroy();
    }
  }

  private async fetchAi(handle: number, queryDocument: string, limit: number,
    epoch: number): Promise<boolean> {
    let descriptor: AiHttpDescriptor | null;
    try {
      const reply: NativeReply<AiHttpDescriptor | null> = JSON.parse(
        client.aiRequestForQuery(handle, queryDocument)) as NativeReply<AiHttpDescriptor | null>;
      if (!reply.ok || reply.value === null) return false;
      descriptor = reply.value;
    } catch (error) {
      KeyboardLog.error(`AI query unavailable: ${describeError(error)}`);
      return false;
    }
    const request = http.createHttp();
    this.onlineRequests.add(request);
    try {
      const response: http.HttpResponse = await request.request(descriptor.url, {
        method: http.RequestMethod.POST,
        header: descriptor.headers,
        connectTimeout: KeyboardGeometry.bounded(descriptor.connect_timeout_ms, 1000, 8000),
        readTimeout: KeyboardGeometry.bounded(descriptor.timeout_ms, 1000, 10000),
        extraData: JSON.stringify(descriptor.body)
      });
      if (epoch !== this.onlineEpoch || handle !== this.handle || response.responseCode < 200
        || response.responseCode >= 300 || typeof response.result !== 'string') return false;
      const candidates: string[] | null = OnlineCandidatePolicy.aiCandidates(response.result, limit);
      if (candidates === null || candidates.length === 0) return false;
      const applied: NativeReply<OnlineApplyValue> = JSON.parse(client.applyOnlineCandidates(
        handle, queryDocument, JSON.stringify(candidates), 1)) as NativeReply<OnlineApplyValue>;
      if (applied.ok && applied.value.applied) this.render(applied.value.view);
      return applied.ok && applied.value.applied;
    } catch (error) {
      KeyboardLog.error(`AI candidate unavailable: ${describeError(error)}`);
      return false;
    } finally {
      this.onlineRequests.delete(request);
      request.destroy();
    }
  }

  private scheduleTranslations(): void {
    if (this.handle === 0) return;
    let reply: NativeReply<TranslationQuery | null>;
    try {
      reply = JSON.parse(client.translationQuery(this.handle)) as NativeReply<TranslationQuery | null>;
    } catch (error) {
      KeyboardLog.error(`translation query unavailable: ${describeError(error)}`);
      return;
    }
    if (!reply.ok || reply.value === null) {
      this.translationSignature = '';
      return;
    }
    const query: TranslationQuery = reply.value;
    const signature: string = TranslationPolicy.signature(query);
    if (signature === this.translationSignature) return;
    this.translationSignature = signature;
    if (this.translationTimer >= 0) clearTimeout(this.translationTimer);
    const epoch: number = ++this.translationEpoch;
    const handle: number = this.handle;
    const requestSignature: string = signature;
    this.translationTimer = setTimeout((): void => {
      this.translationTimer = -1;
      void this.fetchTranslations(handle, JSON.stringify(query), query, epoch, requestSignature);
    }, TranslationPolicy.QUIET_INTERVAL_MS);
  }

  private async fetchTranslations(handle: number, queryDocument: string,
    query: TranslationQuery, epoch: number, requestSignature: string): Promise<void> {
    if (!this.translationCurrent(handle, epoch)) return;
    const entries: TranslationEntry[] = [];
    let providerComplete: boolean = true;
    if (query.english_gloss && typeof query.resources === 'string' && query.resources.length > 0) {
      this.appendOfflineGloss(query, entries);
    }
    const offline: Map<string, TranslationEntry[]> = this.offlineTargetGlosses(query);
    const deadline: number = Date.now() + 6000;
    const provider: string = TranslationPolicy.provider(query);
    for (const target of TranslationPolicy.targets(query)) {
      if (!this.translationCurrent(handle, epoch)) return;
      if (Date.now() >= deadline) {
        this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
        return;
      }
      // This target's answers are gathered apart, so the offline dictionary can fill only the candidates the online provider left unanswered.
      const answered: TranslationEntry[] = [];
      const result: TranslationFetchResult = await this.fetchTargetTranslations(query, target,
        provider, entries, answered, epoch, handle, deadline);
      if (!result.current) {
        this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
        return;
      }
      providerComplete = providerComplete && result.providerComplete;
      TranslationPolicy.fill(answered, offline.get(target) ?? []);
      for (const entry of answered) TranslationPolicy.append(entries, entry.text, entry.translation);
    }
    if (!this.translationCurrent(handle, epoch)) return;
    if (entries.length === 0) {
      if (provider.length > 0) this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
      return;
    }
    let appliedCurrent: boolean = false;
    try {
      const applied: NativeReply<OnlineApplyValue> = JSON.parse(client.applyTranslations(
        handle, query.generation, JSON.stringify(entries))) as NativeReply<OnlineApplyValue>;
      if (applied.ok && applied.value !== undefined && applied.value.view !== undefined
        && applied.value.applied !== false) {
        appliedCurrent = applied.value.applied;
        this.render(applied.value.view);
      }
      if (appliedCurrent && typeof query.user_data === 'string' && query.user_data.length > 0
        && TranslationPolicy.targets(query).includes('en')) {
        const saveRequest: TranslationGlossSaveRequest = {
          target_language: 'en', translations: entries
        };
        client.translationGlossSave(JSON.stringify(saveRequest), query.user_data);
      }
      if (appliedCurrent && TranslationPolicy.shouldReleaseAfterProviderFailure(provider,
        providerComplete, entries.length > 0)) {
        this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
      }
      if (!appliedCurrent) this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
    } catch (error) {
      KeyboardLog.error(`candidate translations unavailable: ${describeError(error)}`);
      this.releaseTranslationSignatureIfCurrent(requestSignature, epoch, handle);
    }
  }

  private releaseTranslationSignatureIfCurrent(requestSignature: string, requestEpoch: number,
    requestHandle: number): void {
    if (TranslationPolicy.shouldReleaseAfterFailure(requestSignature, this.translationSignature,
      requestEpoch, this.translationEpoch, requestHandle, this.handle)) this.translationSignature = '';
  }

  /** One target's online answers, added to `answered`. `entries` is read only for the English candidates the offline gloss already covers. False when the work went stale or ran out of time. */
  private async fetchTargetTranslations(query: TranslationQuery, target: string, provider: string,
    entries: TranslationEntry[], answered: TranslationEntry[], epoch: number, handle: number,
    deadline: number): Promise<TranslationFetchResult> {
    // `/fy` is asked as its own item, past the plan and the caches (TranslationPolicy.commandItem).
    const command: TranslationPlanItem | null = TranslationPolicy.commandItem(query);
    let plan: TranslationPlanItem[] = command !== null ? [command] : [];
    if (command === null) {
      try {
        const planDocument: string = TranslationPolicy.planRequest(target, query.candidates);
        const planReply: NativeReply<TranslationPlanItem[]> =
          JSON.parse(client.translationPlan(planDocument)) as NativeReply<TranslationPlanItem[]>;
        if (planReply.ok && Array.isArray(planReply.value)) plan = planReply.value;
      } catch (error) {
        KeyboardLog.error(`translation plan unavailable: ${describeError(error)}`);
      }
    }
    if (plan.length === 0 || provider.length === 0) {
      return {
        current: this.translationCurrent(handle, epoch),
        providerComplete: provider.length === 0 || plan.length > 0,
      };
    }
    const pending: TranslationPlanItem[] = [];
    for (const item of plan) {
      if (command !== null) {
        pending.push(item);
        continue;
      }
      if (target === 'en' && query.english_gloss
        && entries.some((entry: TranslationEntry): boolean => entry.text === item.text)) {
        continue;
      }
      const key: string = TranslationPolicy.cacheKey(query, target, item);
      const cached: string | undefined = this.translationCache.get(key);
      if (cached !== undefined) {
        TranslationPolicy.append(answered, item.text, cached);
        continue;
      }
      const negativeUntil: number | undefined = this.translationNegativeCache.get(key);
      if (negativeUntil !== undefined) {
        if (negativeUntil > Date.now()) continue;
        this.translationNegativeCache.delete(key);
      }
      pending.push(item);
    }
    if (pending.length === 0) {
      return { current: this.translationCurrent(handle, epoch), providerComplete: true };
    }
    if (provider === 'tencent' && query.tencent_tmt !== null
      && query.tencent_tmt !== undefined) {
      const providerComplete: boolean = await this.fetchTencentTranslations(
        query, pending, answered, epoch, handle, deadline);
      return {
        current: this.translationCurrent(handle, epoch),
        providerComplete: providerComplete,
      };
    } else {
      let providerComplete: boolean = true;
      for (const item of pending) {
        if (!this.translationCurrent(handle, epoch) || Date.now() >= deadline) {
          return { current: false, providerComplete: false };
        }
        let value: string | null = null;
        let completed: boolean = false;
        if (provider === 'niutrans' && query.niutrans !== null
          && query.niutrans !== undefined) {
          const result: TranslationProviderResult = await this.fetchNiuTranslation(
            query.niutrans, item, epoch, handle);
          value = result.value;
          completed = result.completed;
        } else if (provider === 'custom' && query.custom_translation !== null
          && query.custom_translation !== undefined) {
          const result: TranslationProviderResult = await this.fetchCustomTranslation(
            query.custom_translation, item, epoch, handle);
          value = result.value;
          completed = result.completed;
        }
        if (!completed) providerComplete = false;
        this.recordTranslation(query, target, item, value, answered);
      }
      return {
        current: this.translationCurrent(handle, epoch),
        providerComplete: providerComplete,
      };
    }
  }

  /** Each installed non-English target's offline glosses, read from the dictionary beside the resources. */
  private offlineTargetGlosses(query: TranslationQuery): Map<string, TranslationEntry[]> {
    const result: Map<string, TranslationEntry[]> = new Map<string, TranslationEntry[]>();
    const resources: string = query.resources ?? '';
    if (resources.length === 0) return result;
    for (const language of TranslationPolicy.offlineTargets(query)) {
      try {
        const request: GlossRequestDocument = {
          generation: query.generation,
          user_data: '',
          candidates: query.candidates.map((candidate: TranslationCandidate): GlossRequestCandidate => ({
            text: candidate.text, source: 0
          })),
          target_language: language
        };
        const reply: NativeReply<TranslationResponseValue> = JSON.parse(
          client.candidateGlosses(JSON.stringify(request), resources)) as NativeReply<TranslationResponseValue>;
        if (!reply.ok || reply.value === undefined || reply.value.generation !== query.generation
          || !Array.isArray(reply.value.translations)) continue;
        const glosses: TranslationEntry[] = [];
        for (const entry of reply.value.translations) {
          TranslationPolicy.append(glosses, entry.text, entry.translation);
        }
        result.set(language, glosses);
      } catch (error) {
        KeyboardLog.error(`offline ${language} candidate glosses unavailable: ${describeError(error)}`);
      }
    }
    return result;
  }

  /** Whether the Cantonese or Zhuyin dictionary `file` is staged in `language-dictionaries` beside the resources, which is where host preparation looks for it. */
  private languageDictionaryInstalled(file: string): boolean {
    const parent: string = this.resourceDirectory.substring(0, this.resourceDirectory.lastIndexOf('/'));
    return parent.length > 0 && fs.accessSync(`${parent}/language-dictionaries/${file}`);
  }

  /** The non-English targets with an offline dictionary staged beside the resources, known before any query so the gloss line is reserved up front. */
  private installedOfflineGlosses(targets: string[]): string[] {
    const parent: string = this.resourceDirectory.substring(0, this.resourceDirectory.lastIndexOf('/'));
    if (parent.length === 0) return [];
    return targets.filter((target: string): boolean => target !== 'en'
      && fs.accessSync(`${parent}/offline-glosses/zh-${target}.db`));
  }

  private appendOfflineGloss(query: TranslationQuery, entries: TranslationEntry[]): void {
    try {
      const request: GlossRequestDocument = {
        generation: query.generation,
        user_data: query.user_data ?? '',
        candidates: query.candidates.map((candidate: TranslationCandidate): GlossRequestCandidate => ({
          text: candidate.text, source: 0
        }))
      };
      const glossResponse: string = client.candidateGlosses(
        JSON.stringify(request), query.resources ?? '');
      const reply: NativeReply<TranslationResponseValue> =
        JSON.parse(glossResponse) as NativeReply<TranslationResponseValue>;
      if (!reply.ok || reply.value === undefined || reply.value.generation !== query.generation
        || !Array.isArray(reply.value.translations)) return;
      for (const entry of reply.value.translations) {
        TranslationPolicy.append(entries, entry.text, entry.translation);
      }
    } catch (error) {
      KeyboardLog.error(`offline candidate glosses unavailable: ${describeError(error)}`);
    }
  }

  private async fetchTencentTranslations(query: TranslationQuery, items: TranslationPlanItem[],
    entries: TranslationEntry[], epoch: number, handle: number, deadline: number): Promise<boolean> {
    let providerComplete: boolean = true;
    const groups: Map<string, TranslationPlanItem[]> = new Map<string, TranslationPlanItem[]>();
    for (const item of items) {
      const group: string = `${item.source_language}\n${item.target_language}`;
      const values: TranslationPlanItem[] = groups.get(group) ?? [];
      values.push(item);
      groups.set(group, values);
    }
    for (const group of groups.values()) {
      if (!this.translationCurrent(handle, epoch) || Date.now() >= deadline) return false;
      const config: TencentTranslationConfig | null | undefined = query.tencent_tmt;
      if (config === null || config === undefined) {
        providerComplete = false;
        continue;
      }
      const request: TencentTranslationRequest = {
        config: config,
        texts: group.map((item: TranslationPlanItem): string => item.key),
        source_language: group[0].source_language,
        target_language: group[0].target_language,
        timestamp: Math.floor(Date.now() / 1000)
      };
      let descriptor: TranslationHttpDescriptor | null = null;
      try {
        const descriptorResponse: string = client.tencentTranslationHttpRequest(
          JSON.stringify(request));
        const reply: NativeReply<TranslationHttpDescriptor | null> =
          JSON.parse(descriptorResponse) as NativeReply<TranslationHttpDescriptor | null>;
        if (reply.ok) descriptor = reply.value;
      } catch (error) {
        KeyboardLog.error(`Tencent translation descriptor unavailable: ${describeError(error)}`);
      }
      if (descriptor === null) {
        providerComplete = false;
        continue;
      }
      const body: string | null = await this.translationHttp(descriptor, false, epoch, handle);
      if (body === null) {
        providerComplete = false;
        continue;
      }
      try {
        const parsed: NativeReply<string[] | null> = JSON.parse(client.parseTencentTranslationResponse(
          body, group.length)) as NativeReply<string[] | null>;
        if (!parsed.ok || parsed.value === null || parsed.value.length !== group.length) {
          providerComplete = false;
          continue;
        }
        for (let index: number = 0; index < group.length; index++) {
          this.recordTranslation(query, group[0].target_language, group[index], parsed.value[index],
            entries);
        }
      } catch (error) {
        KeyboardLog.error(`Tencent translation response unavailable: ${describeError(error)}`);
        providerComplete = false;
      }
    }
    return providerComplete;
  }

  private async fetchNiuTranslation(config: NiuTransTranslationConfig, item: TranslationPlanItem,
    epoch: number, handle: number): Promise<TranslationProviderResult> {
    const request: NiuTransTranslationRequest = {
        config: config,
        text: item.key,
        source_language: item.source_language,
        target_language: item.target_language,
        timestamp: String(Date.now())
    };
    const descriptor: TranslationHttpDescriptor | null = this.translationDescriptor(
      client.niuTransTranslationHttpRequest(JSON.stringify(request)));
    if (descriptor === null) return { value: null, completed: false };
    const body: string | null = await this.translationHttp(descriptor, false, epoch, handle);
    if (body === null) return { value: null, completed: false };
    try {
      const reply: NativeReply<string | null> = JSON.parse(
        client.parseNiuTransTranslationResponse(body)) as NativeReply<string | null>;
      return { value: reply.ok ? reply.value : null, completed: reply.ok };
    } catch {
      return { value: null, completed: false };
    }
  }

  private async fetchCustomTranslation(config: TranslationProviderConfig, item: TranslationPlanItem,
    epoch: number, handle: number): Promise<TranslationProviderResult> {
    const request: CustomTranslationRequest = {
        config: config,
        text: item.key,
        source_language: item.source_language,
        target_language: item.target_language
    };
    const descriptor: TranslationHttpDescriptor | null = this.translationDescriptor(
      client.customTranslationHttpRequest(JSON.stringify(request)));
    if (descriptor === null) return { value: null, completed: false };
    const body: string | null = await this.translationHttp(descriptor, true, epoch, handle);
    if (body === null) return { value: null, completed: false };
    try {
      const reply: NativeReply<string | null> = JSON.parse(
        client.parseCustomTranslationResponse(body)) as NativeReply<string | null>;
      return { value: reply.ok ? reply.value : null, completed: reply.ok };
    } catch {
      return { value: null, completed: false };
    }
  }

  private recordTranslation(query: TranslationQuery, target: string, item: TranslationPlanItem,
    value: string | null, entries: TranslationEntry[]): void {
    // A `/fy` answer is the command's row, not a candidate gloss: it is neither cached nor remembered as a failure.
    if (query.sentence === true) {
      if (value !== null) TranslationPolicy.append(entries, item.text, value);
      return;
    }
    if (value === null) {
      this.translationNegativeCache.set(TranslationPolicy.cacheKey(query, target, item),
        Date.now() + TranslationPolicy.NEGATIVE_CACHE_MS);
      return;
    }
    const normalised: string = TranslationPolicy.normalise(value);
    if (normalised.length === 0) {
      this.translationNegativeCache.set(TranslationPolicy.cacheKey(query, target, item),
        Date.now() + TranslationPolicy.NEGATIVE_CACHE_MS);
      return;
    }
    this.translationCache.set(TranslationPolicy.cacheKey(query, target, item), normalised);
    this.translationNegativeCache.delete(TranslationPolicy.cacheKey(query, target, item));
    TranslationPolicy.append(entries, item.text, normalised);
  }

  private translationDescriptor(raw: string): TranslationHttpDescriptor | null {
    try {
      const reply: NativeReply<TranslationHttpDescriptor | null> =
        JSON.parse(raw) as NativeReply<TranslationHttpDescriptor | null>;
      return reply.ok && reply.value !== null ? reply.value : null;
    } catch {
      return null;
    }
  }

  private async translationHttp(descriptor: TranslationHttpDescriptor, allowHttp: boolean,
    epoch: number, handle: number): Promise<string | null> {
    if (!this.translationCurrent(handle, epoch) || typeof descriptor.url !== 'string'
      || descriptor.url.length > 2048 || descriptor.url.includes('\n')
      || descriptor.url.includes('\r')
      || !TextPolicy.validSecureAuthority(descriptor.url, allowHttp)) return null;
    if (descriptor.max_response_bytes > TranslationPolicy.MAX_RESPONSE_BYTES) return null;
    const request = http.createHttp();
    this.translationRequests.add(request);
    try {
      const response: http.HttpResponse = await request.request(descriptor.url, {
        method: http.RequestMethod.POST,
        header: descriptor.headers,
        connectTimeout: KeyboardGeometry.bounded(descriptor.timeout_ms, 1000, 3000),
        readTimeout: KeyboardGeometry.bounded(descriptor.timeout_ms, 1000, 8000),
        extraData: descriptor.body_utf8 !== undefined
          ? descriptor.body_utf8 : JSON.stringify(descriptor.body ?? {})
      });
      if (!this.translationCurrent(handle, epoch) || response.responseCode < 200
        || response.responseCode >= 300 || typeof response.result !== 'string'
        || utf8Length(response.result) > TranslationPolicy.MAX_RESPONSE_BYTES) return null;
      return response.result;
    } catch (error) {
      KeyboardLog.error(`candidate translation request unavailable: ${describeError(error)}`);
      return null;
    } finally {
      this.translationRequests.delete(request);
      request.destroy();
    }
  }

  private translationCurrent(handle: number, epoch: number): boolean {
    return handle !== 0 && handle === this.handle && epoch === this.translationEpoch;
  }

  private feedbackFile(): string {
    return KeyboardSession.feedbackFileIn(this.stateDirectory);
  }

  private static feedbackFileIn(stateRoot: string): string {
    return `${stateRoot}/key-feedback.json`;
  }

  /**
   * The same file, named by a caller that has no live session.
   *
   * The settings page runs in the application rather than in the keyboard extension, so it has a
   * state directory but no session to ask. Both processes read and write the one file.
   */
  static feedbackIn(stateRoot: string): FeedbackSettings {
    try {
      const path: string = KeyboardSession.feedbackFileIn(stateRoot);
      if (!fs.accessSync(path) || fs.statSync(path).size > KeyboardFeedback.MAX_BYTES) {
        return KeyboardFeedback.DEFAULTS;
      }
      return KeyboardFeedback.parse(fs.readTextSync(path));
    } catch (error) {
      KeyboardLog.error(`key feedback unreadable: ${describeError(error)}`);
      return KeyboardFeedback.DEFAULTS;
    }
  }

  static saveFeedbackIn(stateRoot: string, settings: FeedbackSettings): boolean {
    try {
      const handle = fs.openSync(KeyboardSession.feedbackFileIn(stateRoot),
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, KeyboardFeedback.serialize(settings));
      } finally {
        fs.closeSync(handle);
      }
      return true;
    } catch (error) {
      KeyboardLog.error(`key feedback not saved: ${describeError(error)}`);
      return false;
    }
  }

  /** How keys should feel on this device, which is not something the shared preferences carry. */
  feedback(): FeedbackSettings {
    try {
      if (!fs.accessSync(this.feedbackFile())
        || fs.statSync(this.feedbackFile()).size > KeyboardFeedback.MAX_BYTES) {
        return KeyboardFeedback.DEFAULTS;
      }
      return KeyboardFeedback.parse(fs.readTextSync(this.feedbackFile()));
    } catch (error) {
      KeyboardLog.error(`key feedback unreadable: ${describeError(error)}`);
      return KeyboardFeedback.DEFAULTS;
    }
  }

  saveFeedback(settings: FeedbackSettings): boolean {
    try {
      const handle = fs.openSync(this.feedbackFile(),
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, KeyboardFeedback.serialize(settings));
      } finally {
        fs.closeSync(handle);
      }
      this.feedbackSettings = settings;
      return true;
    } catch (error) {
      KeyboardLog.error(`key feedback not saved: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Fires the feedback a key press asks for.
   *
   * Failures are logged once rather than per keystroke: a device without a vibrator would otherwise
   * fill the log with the same line at typing speed.
   */
  private tapFeedback(): void {
    const settings: FeedbackSettings = this.feedbackSettings;
    if (!settings.haptics || this.vibratorFailed) {
      return;
    }
    try {
      vibrator.startVibration(
        { type: 'time', duration: KeyboardFeedback.duration(settings.strength) },
        { usage: 'physicalFeedback' });
    } catch (error) {
      this.vibratorFailed = true;
      KeyboardLog.error(`vibration unavailable: ${describeError(error)}`);
    }
  }

  /** `stateRoot` is `<files>/state`; the shared mobile store owns `<files>/MSIME`. */
  private clipboardRoot(): string {
    const separator: number = this.stateDirectory.lastIndexOf('/');
    return separator > 0 ? this.stateDirectory.substring(0, separator) : '';
  }

  private clearClipboardHistoryFile(): void {
    if (this.stateDirectory.length === 0) {
      return;
    }
    const value: MobileClipboardValue | null = this.mobileClipboard({ operation: 'clear' });
    if (value === null || value.cleared !== true) {
      KeyboardLog.error('clipboard history not cleared');
    }
  }

  /** One locked native transaction; errors never expose clipboard contents in the log. */
  private mobileClipboard(action: MobileClipboardAction): MobileClipboardValue | null {
    const directory: string = this.clipboardRoot();
    if (directory.length === 0) return null;
    const request: MobileClipboardRequest = {
      directory: directory, legacy: 'harmony_state', action: action
    };
    try {
      const reply: MobileClipboardReply = JSON.parse(
        client.mobileClipboardHistory(JSON.stringify(request))) as MobileClipboardReply;
      if (!reply.ok || reply.value === undefined || reply.value === null
        || !Array.isArray(reply.value.entries)) return null;
      return reply.value;
    } catch (error) {
      KeyboardLog.error(`clipboard history unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /** Validate and translate the native camel-case record before it reaches ArkUI. */
  private clipboardEntries(value: MobileClipboardValue): ClipboardHistoryItem[] {
    const rows: ClipboardHistoryItem[] = value.entries.map(
      (entry: NativeClipboardEntry): ClipboardHistoryItem => ({
        text: entry.text, at: entry.timestampMs, pinned: entry.pinned
      }));
    return ClipboardHistoryStore.parse(JSON.stringify(rows));
  }

  /**
   * The saved clipboard history.
   *
   * A file too large to be a history of fifty bounded entries is not read at all: the bound exists
   * so that a wrong or corrupted path cannot turn into an unbounded read.
   */
  clipboardHistory(): ClipboardHistoryItem[] {
    if (!this.clipboardHistoryAllowed()) {
      return [];
    }
    try {
      const value: MobileClipboardValue | null = this.mobileClipboard({ operation: 'load' });
      return value === null ? [] : this.clipboardEntries(value);
    } catch (error) {
      KeyboardLog.error(`clipboard history unreadable: ${describeError(error)}`);
      return [];
    }
  }

  captureClipboardHistory(text: string): ClipboardHistoryMutation {
    if (!this.clipboardHistoryAllowed()) {
      return { entries: [], failure: ClipboardFailure.INVALID_FILE };
    }
    try {
      // Reuse the tested public validation and its specific user-facing failures before crossing
      // the ABI. The native transaction repeats these checks against the persisted document.
      ClipboardHistoryStore.add([], text, Date.now());
      const value: MobileClipboardValue | null = this.mobileClipboard(
        { operation: 'capture', text: text });
      if (value === null) return { entries: [], failure: ClipboardFailure.INVALID_FILE };
      const entries: ClipboardHistoryItem[] = this.clipboardEntries(value);
      if (value.captured !== true) {
        return { entries: entries, failure: value.reason === 'full'
          ? ClipboardFailure.FULL : value.reason === 'invalid'
            ? ClipboardFailure.TOO_LONG : ClipboardFailure.INVALID_FILE };
      }
      return { entries: entries, failure: null };
    } catch (error) {
      return { entries: [], failure: error instanceof ClipboardHistoryError
        ? error.failure : ClipboardFailure.INVALID_FILE };
    }
  }

  setClipboardPinned(text: string, pinned: boolean): ClipboardHistoryMutation {
    return this.mutateClipboard({ operation: 'set_pinned', text: text, pinned: pinned }, 'updated');
  }

  removeClipboardHistory(text: string): ClipboardHistoryMutation {
    return this.mutateClipboard({ operation: 'remove', text: text }, 'removed');
  }

  clearClipboardHistory(): ClipboardHistoryMutation {
    return this.mutateClipboard({ operation: 'clear' }, 'cleared');
  }

  /**
   * Which editor a cloud clipboard listing was fetched for.
   *
   * The attribute generation rather than the editor one: only attach and detach move it, so an idle Engine restart does not throw away a listing that still belongs to the focused field.
   */
  cloudClipboardEditor(): number {
    return this.attributeGeneration;
  }

  /** Whether the focused editor may see cloud clipboard items: never a password field, nor one whose attributes are still unknown. */
  cloudClipboardVisible(): boolean {
    return this.inputClient !== undefined
      && CloudClipboardPolicy.editorAllows(this.editorAttributesKnown, this.traits.password);
  }

  /**
   * The account's cloud clipboard, fetched now.
   *
   * Null when the listing no longer belongs to the focused editor: the field changed while the request was out, or it is one that may not see cloud items. A late answer for another field is dropped rather than shown, since tapping one of its items would paste into a field it was never fetched for.
   */
  async cloudClipboardItems(): Promise<CloudClipboardListing | null> {
    const editor: number = this.attributeGeneration;
    if (!this.cloudClipboardVisible()) return null;
    const listing: CloudClipboardListing = await this.cloudClipboard().list();
    if (!CloudClipboardPolicy.current(editor, this.attributeGeneration) || !this.cloudClipboardVisible()) {
      return null;
    }
    return listing;
  }

  /** Inserts a cloud item the way a local history entry is inserted, but only into the editor its listing was fetched for. */
  insertCloudClipboard(text: string, editor: number): boolean {
    if (!CloudClipboardPolicy.current(editor, this.attributeGeneration) || !this.cloudClipboardVisible()) {
      return false;
    }
    this.insert(text);
    return true;
  }

  /** Uploads one local history entry the user chose; nothing reaches the cloud clipboard any other way. */
  async sendToCloudClipboard(text: string): Promise<CloudClipboardSendOutcome> {
    return await this.cloudClipboard().add(text);
  }

  private cloudClipboard(): CloudClipboardClient {
    // Kept only once start() has named the state directory; a client made before that has no session file to read.
    const cached: CloudClipboardClient | undefined = this.cloudClipboardClient;
    if (cached !== undefined) return cached;
    const made: CloudClipboardClient = new CloudClipboardClient(this.stateDirectory);
    if (this.stateDirectory.length > 0) this.cloudClipboardClient = made;
    return made;
  }

  private mutateClipboard(action: MobileClipboardAction,
                          flag: string): ClipboardHistoryMutation {
    if (!this.clipboardHistoryAllowed()) {
      return { entries: [], failure: ClipboardFailure.INVALID_FILE };
    }
    try {
      const value: MobileClipboardValue | null = this.mobileClipboard(action);
      if (value === null) return { entries: [], failure: ClipboardFailure.INVALID_FILE };
      const succeeded: boolean = flag === 'updated' ? value.updated === true
        : flag === 'removed' ? value.removed === true : value.cleared === true;
      return { entries: this.clipboardEntries(value),
        failure: succeeded ? null : ClipboardFailure.STALE };
    } catch (error) {
      return { entries: [], failure: ClipboardFailure.INVALID_FILE };
    }
  }

  private emojiRecentsFile(): string {
    return `${this.stateDirectory}/emoji_recents.json`;
  }

  /** Recent emoji belong to this keyboard installation, not to the shared preference schema. */
  emojiRecents(): string[] {
    if (this.stateDirectory.length === 0) {
      return [];
    }
    const file: string = this.emojiRecentsFile();
    try {
      if (!fs.accessSync(file) || fs.statSync(file).size > EMOJI_RECENTS_MAX_BYTES) {
        return [];
      }
      return EmojiCatalogModel.parseRecents(fs.readTextSync(file));
    } catch (error) {
      KeyboardLog.error(`emoji recents unavailable: ${describeError(error)}`);
      return [];
    }
  }

  recordEmojiRecent(text: string): string[] {
    let recents: string[];
    try {
      recents = EmojiCatalogModel.recordRecent(this.emojiRecents(), text);
    } catch (error) {
      KeyboardLog.error(`emoji recent rejected: ${describeError(error)}`);
      return this.emojiRecents();
    }
    if (this.stateDirectory.length === 0) {
      return recents;
    }
    try {
      const handle = fs.openSync(this.emojiRecentsFile(),
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, EmojiCatalogModel.serializeRecents(recents));
      } finally {
        fs.closeSync(handle);
      }
    } catch (error) {
      KeyboardLog.error(`emoji recents not persisted: ${describeError(error)}`);
    }
    return recents;
  }

  clearEmojiRecents(): boolean {
    if (this.stateDirectory.length === 0) {
      return false;
    }
    try {
      const handle = fs.openSync(this.emojiRecentsFile(),
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, '[]');
      } finally {
        fs.closeSync(handle);
      }
      return true;
    } catch (error) {
      KeyboardLog.error(`emoji recents not cleared: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Whether this build may read the system clipboard at all.
   *
   * READ_PASTEBOARD is system_basic on OpenHarmony, so an ordinary third-party input method is
   * refused it however it asks; only a build signed with a provisioning profile carrying the ACL
   * gets it. Without this check a denied read is indistinguishable from an empty clipboard, and the
   * keyboard would tell the user there was nothing to save when in fact it was not allowed to look.
   */
  async canReadClipboard(): Promise<boolean> {
    try {
      const info: bundleManager.BundleInfo = await bundleManager.getBundleInfoForSelf(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION);
      const granted: abilityAccessCtrl.GrantStatus =
        await abilityAccessCtrl.createAtManager().checkAccessToken(
          info.appInfo.accessTokenId, 'ohos.permission.READ_PASTEBOARD');
      return granted === abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED;
    } catch (error) {
      KeyboardLog.error(`clipboard permission unknown: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Whatever plain text the system clipboard holds, or an empty string when it holds none.
   *
   * The plain-text record is asked for only once the clipboard says it has one. getPrimaryText is
   * declared as returning a string, but on a clipboard holding an image or nothing at all it hands
   * back undefined, and the caller then has a string that is not one. This is a system boundary, so
   * the check belongs here rather than at every use.
   */
  async systemClipboardText(): Promise<string> {
    try {
      const data: pasteboard.PasteData = await pasteboard.getSystemPasteboard().getData();
      if (data.getRecordCount() === 0 || !data.hasType(pasteboard.MIMETYPE_TEXT_PLAIN)) {
        return '';
      }
      const text: string | undefined = data.getPrimaryText();
      return text === undefined ? '' : text;
    } catch (error) {
      // A clipboard the user has not granted, or one holding something that is not text.
      KeyboardLog.error(`clipboard unavailable: ${describeError(error)}`);
      return '';
    }
  }

  /** Reads only the bounded, app-private reply templates explicitly shared by the settings app. */
  communityReplyTemplates(): CommunityReplyTemplate[] {
    // KeyboardExtensionAbility passes files/state as stateDirectory; the settings process shares
    // its explicit library one level above, at files/CommunityLibrary.json.
    const file: string = `${this.stateDirectory}/../CommunityLibrary.json`;
    try {
      if (!fs.accessSync(file) || fs.statSync(file).size > CommunityReplyLibraryPolicy.MAX_BYTES) {
        return [];
      }
      return CommunityReplyLibraryPolicy.parse(fs.readTextSync(file));
    } catch (error) {
      KeyboardLog.error(`community reply library unavailable: ${describeError(error)}`);
      return [];
    }
  }

  /** Generates bounded reply candidates through the shared AI ABI without involving Engine state. */
  async thoughtfulReplies(source: string, style: string, extraPrompt: string = '', polish: boolean = false): Promise<string[]> {
    const request: ReplyRequest | null = ReplyKeyboardPolicy.request(source, style, extraPrompt, polish);
    if (request === null || !this.aiAssistant.enabled || this.aiAssistant.endpoint.length === 0) {
      return [];
    }
    const editorGeneration: number = this.editorGeneration;
    const contextGeneration: number = this.replyContextGeneration;
    const config: PreparedAiAssistant = {
      enabled: this.aiAssistant.enabled, provider: this.aiAssistant.provider,
      model: this.aiAssistant.model, token: this.aiAssistant.token, tokens: this.aiAssistant.tokens,
      endpoint: this.aiAssistant.endpoint, candidate_limit: 3, prompt_id: this.aiAssistant.prompt_id,
      prompt: request.prompt, prompt_custom_1: this.aiAssistant.prompt_custom_1,
      prompt_custom_2: this.aiAssistant.prompt_custom_2, prompt_custom_3: this.aiAssistant.prompt_custom_3
    };
    const input: AiHttpRequestInput = {
      segmented_pinyin: ['reply'], context: request.source, candidate_limit: 3
    };
    const payload: AiHttpRequest = { config: config, input: input };
    const document: string = JSON.stringify(payload);
    let descriptor: AiHttpDescriptor | null;
    try {
      const raw: NativeReply<AiHttpDescriptor | null> = JSON.parse(
        client.aiHttpRequest(document)) as NativeReply<AiHttpDescriptor | null>;
      if (!raw.ok || raw.value === null) return [];
      descriptor = raw.value;
    } catch (error) {
      KeyboardLog.error(`thoughtful reply request unavailable: ${describeError(error)}`);
      return [];
    }
    const requestHttp = http.createHttp();
    this.replyRequest = requestHttp;
    try {
      const response: http.HttpResponse = await requestHttp.request(descriptor.url, {
        method: http.RequestMethod.POST, header: descriptor.headers,
        connectTimeout: KeyboardGeometry.bounded(descriptor.connect_timeout_ms, 1000, 8000),
        readTimeout: KeyboardGeometry.bounded(descriptor.timeout_ms, 1000, 10000),
        extraData: JSON.stringify(descriptor.body)
      });
      if (!ReplyContextPolicy.matches(editorGeneration, this.editorGeneration,
        contextGeneration, this.replyContextGeneration)
        || this.inputClient === undefined
        || response.responseCode < 200 || response.responseCode >= 300
        || typeof response.result !== 'string'
        || utf8Length(response.result) > descriptor.max_response_bytes) return [];
      const parsed: NativeReply<string[] | null> = JSON.parse(client.parseAiResponse(
        response.result, 3)) as NativeReply<string[] | null>;
      const results: string[] = parsed.ok ? ReplyKeyboardPolicy.results(parsed.value) : [];
      if (!ReplyContextPolicy.matches(editorGeneration, this.editorGeneration,
        contextGeneration, this.replyContextGeneration) || results.length === 0) return [];
      this.replyResultEditorGeneration = editorGeneration;
      this.replyResultContextGeneration = contextGeneration;
      this.replyResultTexts = results;
      return results;
    } catch (error) {
      KeyboardLog.error(`thoughtful reply unavailable: ${describeError(error)}`);
      return [];
    } finally {
      if (this.replyRequest === requestHttp) this.replyRequest = undefined;
      requestHttp.destroy();
    }
  }

  /** Insert a reply only if it was generated for the still-current editor context. */
  insertReply(text: string): boolean {
    if (!ReplyContextPolicy.matches(this.replyResultEditorGeneration, this.editorGeneration,
      this.replyResultContextGeneration, this.replyContextGeneration)
      || !this.replyResultTexts.includes(text)) return false;
    this.insertWithSource(text, 'reply');
    this.invalidateReplyContext();
    return true;
  }

  /**
   * Applies a set of stored preferences together, to the live session and to disk.
   *
   * All of them at once, not one call each. A scheme, its shuangpin profile and its keyboard layout
   * are a single decision, and writing them separately fails on the second write: the session
   * refuses a revision it has already seen, and the store compares against what is on disk, which
   * the first write has already moved.
   *
   * The document is read, edited and written whole rather than patched. The store rejects anything
   * it cannot parse in full, and everything this keyboard does not understand travels across
   * untouched, so changing a skin here cannot quietly drop a setting made on another host.
   */
  /**
   * The four preference values one scheme choice decides, read against the document on disk.
   *
   * Picking Japanese has to leave a record of the Chinese scheme it replaced, or the shared settings
   * page has nothing to restore when the user asks for 中文 again and falls back to quanpin — which
   * on a typist who was on 五笔 looks like the keyboard forgot. The Engine's own scheme field cannot
   * hold it, since it is the field being replaced.
   *
   * Read here rather than from the prepared record the view holds: the settings page is a second
   * writer, and the value that matters is the one currently on disk. The write that follows does its
   * own load and compares revisions, so a genuine conflict is still refused rather than overwritten.
   */
  /**
   * Write back a scheme the enabled list moved the keyboard to, and say whether it moved.
   *
   * The shared settings page owns which schemes are offered. Turn off the one the keyboard is on
   * and it vanishes from the picker while the keyboard goes on using it — there is then no way to
   * see or leave it short of choosing something else. MSIME-Apple normalizes the stored scheme the
   * moment the enabled list is written (DisabledSchemesAreHiddenAndCurrentSchemeFallsBack); this
   * host reads the list at attach instead, because the page and the keyboard are separate processes
   * and the write the page makes is to the document, not to a running keyboard.
   *
   * The decision stays in mappingForRuntimeSelection: whether the pair is a move at all, and that
   * the reply keyboard is never a destination, are the two things it answers.
   */
  applyRuntimeSelection(applied: SchemeDefinition, selected: SchemeDefinition): boolean {
    const current: Record<string, Object> | undefined = this.loadPreferenceDocument();
    const mapping: PreferenceMapping | null = KeyboardScheme.mappingForRuntimeSelection(
      applied, selected,
      current === undefined ? null : this.text(current['last_chinese_scheme']),
      current === undefined ? null : this.text(current['shuangpin_profile']));
    if (mapping === null) {
      return false;
    }
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('scheme', mapping.scheme);
    changes.set('last_chinese_scheme', mapping.lastChineseScheme);
    changes.set('shuangpin_profile', mapping.shuangpinProfile);
    changes.set('touch_keyboard_layout', mapping.touchKeyboardLayout);
    if (!this.changePreferences(changes)) {
      return false;
    }
    this.useScheme(selected.engineScheme, selected.shuangpinProfile);
    KeyboardLog.info(`scheme fell back to ${selected.preferenceId}: `
      + `${applied.preferenceId} is no longer enabled`);
    return true;
  }

  schemeChanges(candidate: SchemeDefinition): Map<string, Object> {
    const current: Record<string, Object> | undefined = this.loadPreferenceDocument();
    const mapping: PreferenceMapping = KeyboardScheme.mapping(candidate,
      current === undefined ? null : this.text(current['last_chinese_scheme']),
      current === undefined ? null : this.text(current['shuangpin_profile']));
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('scheme', mapping.scheme);
    changes.set('last_chinese_scheme', mapping.lastChineseScheme);
    changes.set('shuangpin_profile', mapping.shuangpinProfile);
    changes.set('touch_keyboard_layout', mapping.touchKeyboardLayout);
    return changes;
  }

  /** The stored document, or undefined when it cannot be read; the caller then uses the defaults. */
  private loadPreferenceDocument(): Record<string, Object> | undefined {
    try {
      const loaded: SnapshotResponse =
        JSON.parse(client.loadPreferences(this.stateDirectory)) as SnapshotResponse;
      if (!loaded.ok) {
        KeyboardLog.error(`preferences unreadable: ${loaded.error}`);
        return undefined;
      }
      return loaded.value.preferences;
    } catch (error) {
      KeyboardLog.error(`preferences unreadable: ${describeError(error)}`);
      return undefined;
    }
  }

  /** A stored value only counts as a scheme name if it is actually a string. */
  private text(value: Object | undefined): string | null {
    return typeof value === 'string' ? value as string : null;
  }

  changePreferences(changes: Map<string, Object>): boolean {
    if (this.handle === 0) {
      return false;
    }
    try {
      const loaded: SnapshotResponse =
        JSON.parse(client.loadPreferences(this.stateDirectory)) as SnapshotResponse;
      if (!loaded.ok) {
        KeyboardLog.error(`preferences unreadable: ${loaded.error}`);
        return false;
      }
      const snapshot: PreferencesSnapshot = loaded.value;
      // What is on disk now, which is what the store compares against before it overwrites.
      const expected: number = snapshot.revision;
      changes.forEach((value: Object, key: string): void => {
        snapshot.preferences[key] = value;
      });
      const nextClipboardHistory: boolean = ClipboardHistoryPreferencePolicy.enabled(
        snapshot.preferences['clipboard_history']);
      snapshot.revision = expected + 1;
      const document: string = JSON.stringify(snapshot);
      const applied: PrepareResponse =
        JSON.parse(client.updatePreferences(this.handle, document)) as PrepareResponse;
      if (!applied.ok) {
        KeyboardLog.error(`preferences refused: ${applied.error}`);
        return false;
      }
      const saved: SnapshotResponse =
        JSON.parse(client.savePreferences(this.stateDirectory, expected, document)) as
          SnapshotResponse;
      if (!saved.ok) {
        // The session already has them; only the write failed. Say so rather than pretending the
        // change did not happen, because on screen it plainly did.
        KeyboardLog.error(`preferences not persisted: ${saved.error}`);
      } else {
        // Our own write, already applied; the next focus must not rebuild the session for it. The revision is the store's answer rather than expected + 1: a save that changes nothing keeps the stored revision, and assuming the next one would make the next real change made elsewhere look like this one.
        this.preferencesRevision = saved.value.revision;
      }
      this.clipboardHistoryEnabled = nextClipboardHistory;
      if (changes.has('voice_input')) {
        this.voiceInput = snapshot.preferences.voice_input as VoiceInputConfiguration;
        if (this.onVoiceInputChanged !== undefined) {
          this.onVoiceInputChanged(this.voiceInputEnabled());
        }
      }
      if (ClipboardHistoryPreferencePolicy.shouldClearHistory(nextClipboardHistory)) {
        this.clearClipboardHistoryFile();
      }
      if (this.onClipboardHistoryChanged !== undefined) {
        this.onClipboardHistoryChanged(nextClipboardHistory);
      }
      return true;
    } catch (error) {
      KeyboardLog.error(`preferences failed: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * One page of the local emoji catalog.
   *
   * Reads others.db directly rather than through the session: the catalog is a property of the
   * installed dictionary, not of what is being typed, and nothing about it changes with focus.
   */
  emojiPage(group: string, offset: number, limit: number): EmojiPage | null {
    return this.emojiCatalogPage('emoji', group, offset, limit);
  }

  emojiCatalogPage(category: string, group: string, offset: number, limit: number,
                  parent: string = '', search: string = ''): EmojiPage | null {
    const query: EmojiQuery = {
      category: category,
      limit: limit,
      offset: offset,
      group: group,
      cursor: true,
      list_groups: false,
      list_symbol_groups: false,
      parent: parent,
      search: search,
      list_plugin_symbol_groups: false,
      plugins: null
    };
    try {
      const response: EmojiResponse =
        JSON.parse(client.emojiCatalog(JSON.stringify(query), this.resourceDirectory)) as
          EmojiResponse;
      if (!response.ok) {
        KeyboardLog.error(`emoji catalog refused: ${response.error}`);
        return null;
      }
      return response.value;
    } catch (error) {
      KeyboardLog.error(`emoji catalog unreadable: ${describeError(error)}`);
      return null;
    }
  }

  emojiGroups(category: string): string[] {
    const query: EmojiQuery = {
      category: category,
      limit: 1,
      offset: 0,
      group: '',
      cursor: false,
      list_groups: true,
      list_symbol_groups: false,
      parent: '',
      search: '',
      list_plugin_symbol_groups: false,
      plugins: null
    };
    try {
      const raw: string = client.emojiCatalog(JSON.stringify(query), this.resourceDirectory);
      const response: EmojiGroupsResponse = JSON.parse(raw) as EmojiGroupsResponse;
      if (!response.ok || !Array.isArray(response.value.groups)) {
        return [];
      }
      return normalizeGroups(response.value.groups);
    } catch (error) {
      KeyboardLog.error(`emoji groups unavailable: ${describeError(error)}`);
      return [];
    }
  }

  emojiSymbolGroups(): EmojiSymbolGroup[] {
    const query: EmojiQuery = {
      category: 'symbols',
      limit: 1,
      offset: 0,
      group: '',
      cursor: false,
      list_groups: false,
      list_symbol_groups: true,
      parent: '',
      search: '',
      list_plugin_symbol_groups: false,
      plugins: null
    };
    try {
      const raw: string = client.emojiCatalog(JSON.stringify(query), this.resourceDirectory);
      const response: EmojiSymbolGroupsResponse = JSON.parse(raw) as EmojiSymbolGroupsResponse;
      if (!response.ok || !Array.isArray(response.value.symbol_groups)) {
        return [];
      }
      return normalizeSymbolGroups(response.value.symbol_groups);
    } catch (error) {
      KeyboardLog.error(`emoji symbol groups unavailable: ${describeError(error)}`);
      return [];
    }
  }

  /**
   * 已安装符号集插件的组，读自 `<stateDirectory>/plugins`，与设置页交给插件管理的是同一个目录。
   *
   * 不依赖 `others.db`；读不出来时返回空表，表情面板照旧只显示内置内容。
   */
  pluginSymbolGroups(): PluginSymbolGroup[] {
    if (this.stateDirectory.length === 0 || this.resourceDirectory.length === 0) {
      return [];
    }
    const query: EmojiQuery = {
      category: 'symbols',
      limit: 1,
      offset: 0,
      group: '',
      cursor: false,
      list_groups: false,
      list_symbol_groups: false,
      parent: '',
      search: '',
      list_plugin_symbol_groups: true,
      plugins: `${this.stateDirectory}/plugins`
    };
    try {
      const raw: string = client.emojiCatalog(JSON.stringify(query), this.resourceDirectory);
      const response: PluginSymbolGroupsResponse = JSON.parse(raw) as PluginSymbolGroupsResponse;
      if (!response.ok) {
        KeyboardLog.error(`plugin symbol groups refused: ${response.error}`);
        return [];
      }
      return PluginSymbolGroupPolicy.parse(response.value.plugin_symbol_groups);
    } catch (error) {
      KeyboardLog.error(`plugin symbol groups unavailable: ${describeError(error)}`);
      return [];
    }
  }

  /**
   * Simplified to Traditional, where the user asked for it and the text is Chinese to begin with.
   *
   * The conversion is the shared phrase-level OpenCC s2t tables compiled into the native library, the same one the Windows, macOS, Linux and Android hosts call, so 头发 becomes 頭髮 and 发展 becomes 發展 here exactly as there. A converter that fails must not lose the text, so anything it cannot do falls back to what came in. The policy decides whether it applies at all; dedicated English, Japanese and the temporary Japanese mode are not Chinese output however the setting reads.
   */
  private asTraditional(text: string): string {
    return ChineseOutputPolicy.output(text, this.traditional,
      ChineseOutputPolicy.applies(this.englishMode, this.scheme, this.localMode),
      (value: string): string | null => client.simplifiedToTraditional(value));
  }

  /**
   * Put text into the editor directly. It is text, not a composition, so it bypasses the Engine.
   *
   * Fullwidth is applied here rather than left to the Engine: the Engine converts what it commits,
   * and this is the path for everything it does not see — an emoji, a clipboard entry, a letter
   * typed in a password field. Without it the same setting would apply to some of the user's text
   * and not the rest.
   */
  insert(text: string): void {
    if (this.inputClient !== undefined) {
      const widened: string | null = FullWidthInputPolicy.output(text, this.fullWidth);
      const output: string = this.asTraditional(widened === null ? text : widened);
      this.insertEditorText(output);
      this.recordTypingStatistics(output, this.typingSource());
      this.refreshEnglishSuggestions();
    }
  }

  /**
   * Offer completions for the English word being typed.
   *
   * Only in direct English with nothing composing: a pinyin composition already has a candidate
   * strip of its own, and the two would be competing for it. The query is a packaged-dictionary
   * lookup that creates no session, so it runs off the UI thread and its answer is checked against
   * the word still in the editor before anything is drawn.
   */
  private refreshEnglishSuggestions(): void {
    if (!this.englishSuggestionsEnabled || !this.englishMode || this.composing()
        || this.inputClient === undefined || this.resourceDirectory.length === 0) {
      this.clearEnglishSuggestions();
      return;
    }
    const prefix: string = this.englishWordBeforeCursor();
    if (!EnglishSuggestionPolicy.suggestible(prefix)) {
      this.clearEnglishSuggestions();
      return;
    }
    if (prefix === this.englishSuggestionRequested) {
      return;
    }
    const epoch: number = ++this.englishSuggestionEpoch;
    this.englishSuggestionRequested = prefix;
    const request: EnglishCompletionRequest = { prefix: prefix, limit: EnglishSuggestionPolicy.LIMIT };
    const resources: string = this.resourceDirectory;
    // A promise rather than a direct call: the lookup opens a packaged dictionary, and the keyboard
    // has to keep drawing while it happens.
    Promise.resolve().then((): string =>
      client.englishCompletions(JSON.stringify(request), resources)
    ).then((raw: string): void => {
      const completions: EnglishCompletions | null = EnglishSuggestionPolicy.decode(raw);
      this.applyEnglishSuggestions(epoch, prefix, completions);
    }).catch((error: Object): void => {
      // Completions are optional display state; a failed lookup never blocks input.
      KeyboardLog.error(`english completions unavailable: ${describeError(error)}`);
    });
  }

  private applyEnglishSuggestions(epoch: number, prefix: string,
                                  completions: EnglishCompletions | null): void {
    if (epoch !== this.englishSuggestionEpoch || completions === null
        || completions.prefix !== prefix || prefix !== this.englishWordBeforeCursor()) {
      return;
    }
    this.englishSuggestions = completions.items;
    this.englishSuggestionPrefix = prefix;
    this.publishEnglishSuggestions();
  }

  private clearEnglishSuggestions(): void {
    this.englishSuggestionEpoch++;
    this.englishSuggestionRequested = '';
    if (this.englishSuggestions.length === 0 && this.englishSuggestionPrefix.length === 0) {
      return;
    }
    this.englishSuggestions = [];
    this.englishSuggestionPrefix = '';
    this.publishEnglishSuggestions();
  }

  private publishEnglishSuggestions(): void {
    if (this.onEnglishSuggestions !== undefined) {
      this.onEnglishSuggestions(this.englishSuggestionPrefix, this.englishSuggestions.slice());
    }
  }

  /** What the editor says is immediately before the caret, as far back as a word can run. */
  private englishWordBeforeCursor(): string {
    if (this.inputClient === undefined) {
      return '';
    }
    try {
      return EnglishSuggestionPolicy.currentWord(
        this.inputClient.getForwardSync(EnglishSuggestionPolicy.CONTEXT_CHARACTERS));
    } catch (error) {
      return '';
    }
  }

  /**
   * Accept a completion, replacing the letters it completes.
   *
   * The word is re-read rather than trusted: the list was drawn some keystrokes ago, and if the
   * editor has moved on since then the deletion would eat text the completion was never about.
   */
  useEnglishSuggestion(index: number): void {
    if (index < 0 || index >= this.englishSuggestions.length || this.inputClient === undefined) {
      return;
    }
    const typed: string = this.englishWordBeforeCursor();
    if (typed !== this.englishSuggestionPrefix) {
      this.clearEnglishSuggestions();
      return;
    }
    const replacement: EnglishReplacement | null = EnglishSuggestionPolicy.replacement(
      typed, this.englishSuggestions[index],
      EnglishSuggestionPolicy.startedCapitalized(typed));
    if (replacement === null) {
      this.clearEnglishSuggestions();
      return;
    }
    try {
      this.deleteEditorBackwardSync(replacement.deleteCount);
    } catch (error) {
      KeyboardLog.error(`english completion could not replace the word: ${describeError(error)}`);
      this.clearEnglishSuggestions();
      return;
    }
    this.insert(replacement.insert);
    this.clearEnglishSuggestions();
  }

  /** The five switches as the policy reads them; an older document keeps the shared defaults. */
  private static hotkeyBindings(voice: VoiceInputConfiguration): VoiceHotkeyBindings {
    return {
      ralt: voice.hotkey_ralt ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ralt,
      ctrlWin: voice.hotkey_ctrl_win ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ctrlWin,
      rctrlRalt: voice.hotkey_rctrl_ralt ?? DEFAULT_VOICE_HOTKEY_BINDINGS.rctrlRalt,
      holdSpaceLock: voice.hotkey_hold_space_lock ?? DEFAULT_VOICE_HOTKEY_BINDINGS.holdSpaceLock,
      ctrlF9: voice.hotkey_ctrl_f9 ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ctrlF9
    };
  }

  /**
   * Offer a hardware key to the voice shortcuts.
   *
   * Returns whether the key was claimed, so the caller knows not to hand it on.
   */
  /** Opens the settings application: dictionary, quick phrases, cloud and voice providers live there rather than on the keyboard. */
  openSettings(): void {
    if (this.onOpenSettings !== undefined) {
      this.onOpenSettings();
    }
  }

  routeVoiceHotkey(key: VoiceKey): boolean {
    if (!this.voiceInputEnabled()) {
      return false;
    }
    const action: VoiceHotkeyAction = this.voiceHotkeys.accept(key, this.voiceRecording);
    if (action === VoiceHotkeyAction.NONE) {
      return false;
    }
    if (this.onVoiceHotkey !== undefined) {
      this.onVoiceHotkey(action);
    }
    return true;
  }

  /**
   * The view says when the recognizer starts and stops.
   *
   * A recording that ended by itself — a final result, a provider failure — leaves no hold or lock
   * behind, or the next press of the hold key would be read as the release of a recording that is
   * no longer running.
   */
  setVoiceRecording(active: boolean, cancelled: boolean = false): void {
    const changed: boolean = this.voiceRecording !== active;
    this.voiceRecording = active;
    this.updateMusic();
    if (!active) {
      this.voiceHotkeys.reset();
    }
    if (!changed) {
      return;
    }
    // Tones and quiet are display-only work around the recording, never in its way: a failure here
    // is logged inside the behaviour and the recording carries on regardless.
    if (active) {
      this.voiceBehaviour.starting(this.voiceInput).catch((error: Object): void => {
        KeyboardLog.error(`voice start behaviour failed: ${describeError(error)}`);
      });
      return;
    }
    const done: Promise<void> = cancelled
      ? this.voiceBehaviour.cancelled() : this.voiceBehaviour.finished(this.voiceInput);
    done.catch((error: Object): void => {
      KeyboardLog.error(`voice end behaviour failed: ${describeError(error)}`);
    });
  }

  /** The extension's context, which is what turns a packaged tone into something playable. */
  useVoiceContext(context: common.Context): void {
    this.voiceBehaviour.use(context);
  }

  /** The extension's context, which says where the built-in sound packs are and where rendered notes may go. */
  useSoundContext(context: common.Context): void {
    this.soundPlayer.use(context);
    this.musicPlayer.use(context);
  }

  /** Background music plays on a 2in1 while an editor whose attributes are known has focus, is not a password field, and nothing is being recorded; the desktop hosts tell their player the same with `msime_client_music_set_active`. */
  private updateMusic(): void {
    this.musicPlayer.setActive(
      MusicPolicy.active(this.editorAttributesKnown, this.traits.password, this.voiceRecording));
  }

  /** Whether the editor forbids sounds: a password field, or one whose attributes have not arrived yet, since `traits` still describes the previous field until they do (as music waits for them). */
  private soundsSilencedByEditor(): boolean {
    return !this.editorAttributesKnown || this.traits.password;
  }

  /**
   * The sound of a hardware key-down, on a 2in1 with key sounds on. Never in a password field: the rhythm of a password is what a key sound would give away.
   */
  hardwareKeySound(keyCode: number, unicodeChar: number, ctrl: boolean, alt: boolean, logo: boolean): void {
    if (KeySoundPolicy.keysSilent(this.soundsSilencedByEditor(), this.isEnglish(), this.isEnglishCandidates())) {
      return;
    }
    const keyClass: number = KeySoundPolicy.keyClass(keyCode, unicodeChar, ctrl, alt, logo);
    if (keyClass >= 0) {
      this.soundPlayer.key(keyClass);
      // HarmonyOS key events do not say whether a key is an auto-repeat, so every key-down counts.
      this.typingEffect(keyClass);
    }
  }

  /**
   * Hand one key or commit to the session's typing effect, alongside its sound, while a style is drawn or the combo is counted. host-api links no audio stack here, so a tier-up sound it reports due is played by this host; the view draws the rest.
   *
   * @param event a `msime_client_typing_effect` event: a key sound class, or `TYPING_EFFECT_COMMIT`
   */
  private typingEffect(event: number): void {
    if (this.handle === 0 || !TypingEffectPolicy.active(this.typingEffects)) {
      return;
    }
    const effect: TypingEffect = TypingEffectPolicy.decode(client.typingEffect(this.handle, event));
    if (effect.tierSound) {
      this.soundPlayer.tierUp(TypingEffectPolicy.tier(effect.count));
    }
    const listener: ((effect: TypingEffect, settings: TypingEffectSettings, key: boolean) => void) | undefined =
      this.onTypingEffect;
    if (listener !== undefined) {
      listener(effect, this.typingEffects, event !== TYPING_EFFECT_COMMIT);
    }
  }

  /** Resolve the selected effect pack through host-api: after the session is created and after each focus-in, which is when host-api looks at the pack again, never per key. Without a pack nothing is asked. */
  private refreshTypingEffects(): void {
    const preferences: TypingEffectSettings = this.typingEffectPreferences;
    if (this.handle === 0 || preferences.pack === '') {
      this.typingEffects = preferences;
      return;
    }
    try {
      const answer: TypingEffectSettingsResponse =
        JSON.parse(client.typingEffectSettings(this.handle)) as TypingEffectSettingsResponse;
      if (!answer.ok) {
        KeyboardLog.error(`typing effect settings refused: ${answer.error}`);
        this.typingEffects = preferences;
        return;
      }
      if (answer.value.issue !== null) {
        KeyboardLog.error(`effect pack ${preferences.pack} unavailable: ${answer.value.issue}`);
      }
      this.typingEffects = TypingEffectPolicy.resolve(preferences, answer.value);
    } catch (error) {
      KeyboardLog.error(`typing effect settings unreadable: ${describeError(error)}`);
      this.typingEffects = preferences;
    }
  }

  /** Whether the current recording outlives its hold key, for the panel that says so. */
  voiceHotkeyLocked(): boolean {
    return this.voiceHotkeys.isLocked();
  }

  /** Insert a platform-produced result and attribute it to its actual source. */
  insertWithSource(text: string, source: string): void {
    if (this.inputClient === undefined || text.length === 0) return;
    const output: string = this.asTraditional(text);
    this.insertEditorText(output);
    this.recordTypingStatistics(output, source);
  }

  private typingSource(): string {
    return TypingStatisticsPolicy.source(this.schemeName, this.profileName, this.englishMode,
      this.nineKeyMode, this.localMode);
  }

  private clearSmartPunctuationRepeat(): void {
    this.smartPunctuationSnapshot = null;
  }

  /**
   * Rewrites a just-committed Chinese mark as ASCII when the next key is a space.
   *
   * Returns whether it did, because the caller must then not also deliver the space: the user is
   * correcting the mark they just typed, not typing a mark and then a space.
   *
   * What is actually before the caret is re-read here rather than trusted from the arming. The
   * arming records what was committed; anything may have happened since, and the source declines
   * in exactly the same way when the two disagree.
   */
  convertPunctuationSpace(character: number): boolean {
    const snapshot: SmartPunctuationSpaceSnapshot | null = this.smartPunctuationSpaceSnapshot;
    if (snapshot === null || character !== 0x20 || this.inputClient === undefined) {
      return false;
    }
    let preceding: number = 0;
    try {
      preceding = SmartPunctuationContext.precedingCodePoint(InlinePreeditPolicy.beforePreview(
        this.inputClient.getForwardSync(2 + this.previewText.length), this.previewText));
    } catch (error) {
      KeyboardLog.error(`punctuation space context unavailable: ${describeError(error)}`);
      this.clearSmartPunctuationSpace();
      return false;
    }
    const decision: SpaceConvertDecision = SmartPunctuationSpacePolicy.decide(
      snapshot, character, preceding, this.composing(), this.editorGeneration);
    if (decision !== SpaceConvertDecision.CONVERT) {
      this.clearSmartPunctuationSpace();
      return false;
    }
    const ascii: string = String.fromCharCode(snapshot.ascii);
    try {
      // The preceding scalar was just checked to be the armed mark, so this cannot eat other text.
      this.deleteEditorBackwardSync(1);
      this.insertEditorTextSync(ascii);
      this.recordTypingStatistics(ascii, this.typingSource());
    } catch (error) {
      KeyboardLog.error(`punctuation space conversion unavailable: ${describeError(error)}`);
      this.clearSmartPunctuationSpace();
      return false;
    }
    this.clearSmartPunctuationSpace();
    // Typing the same mark again turns it back, which is the existing repeat behaviour and what the
    // source arms at this point too: the conversion has to be undoable without reaching for
    // backspace, or a mis-fire costs more than it saves.
    this.smartPunctuationSnapshot = SmartPunctuationRepeatPolicy.snapshot(
      snapshot.ascii, ascii, Date.now(), this.editorGeneration);
    return true;
  }

  private clearSmartPunctuationSpace(): void {
    this.smartPunctuationSpaceSnapshot = null;
  }

  private canArmSmartPunctuationRepeat(ascii: number, preceding: number): boolean {
    const view: EngineView | undefined = this.lastView;
    const dedicatedEnglish: boolean = view?.dedicated_english ?? this.englishMode;
    return this.smartPunctuation && this.smartPunctuationRepeat
      && this.chinesePunctuation && this.punctuationLock === 'follow'
      && !dedicatedEnglish && this.localMode === 'none'
      && SchemeTraits.hostSmartPunctuation(SchemeTraits.fromName(this.schemeName))
      && !this.composing() && this.currentCandidates.length === 0
      && SmartPunctuationRepeatPolicy.chineseMark(ascii) !== null
      && SmartPunctuationRepeatPolicy.isAsciiAlphanumeric(preceding);
  }

  private recordTypingStatistics(text: string, source: string): void {
    if (text.length === 0 || this.stateDirectory.length === 0) return;
    // One Date for both fields: a day and an hour read from separate instants either side of
    // midnight would file the commit under one day and the other day's hour.
    const instant: Date = new Date();
    const action: TypingStatisticsRecordAction = {
      operation: 'record', text: text, source: source,
      day: TypingStatisticsPolicy.day(instant), hour: TypingStatisticsPolicy.hour(instant)
    };
    const requestDocument: TypingStatisticsRecordRequest = {
      directory: this.stateDirectory, action: action
    };
    const request: string = JSON.stringify(requestDocument);
    // Native storage takes a file lock. Defer it off the key event so a slow filesystem never
    // changes the latency of the next key, while retaining the same bounded text and source.
    setTimeout((): void => {
      try {
        const response: string = client.typingStatistics(request);
        const value: TypingStatisticsRecordResponse = JSON.parse(response) as TypingStatisticsRecordResponse;
        if (value.ok !== true) {
          KeyboardLog.error('typing statistics write refused');
        } else if (typeof value.value?.milestone === 'number' && !this.soundsSilencedByEditor()) {
          // host-api reports the milestone only while achievements are on; it plays the jingle itself on the desktop, and here the player does. Not over a password field, like every other sound.
          this.soundPlayer.achievement();
        }
      } catch (error) {
        KeyboardLog.error(`typing statistics unavailable: ${describeError(error)}`);
      }
    }, 0);
  }

  /**
   * Every candidate for what is being spelled, not just the page on screen.
   *
   * Paging was the only way past the ninth candidate, which put the tail of a long answer dozens of
   * taps away. Showing the whole list at once is the arrangement the iOS keyboard settled on for the
   * same reason.
   */
  /**
   * Every candidate the composition has, not just the page the strip shows.
   *
   * Returns the entries rather than their text. The native record has carried the annotation and
   * the translation all along and this threw both away, so the expanded panel could draw a list of
   * words with no glosses under them while the strip beside it had them — and the gloss the long
   * press offers had nothing to read. Only the fields the panel uses are filled: the management
   * operations index the page rather than this list, so the source does not offer them here either.
   */
  allCandidates(): ExpandedCandidate[] {
    if (this.handle === 0) {
      return [];
    }
    try {
      const response: SnapshotCandidates =
        JSON.parse(client.allCandidates(this.handle)) as SnapshotCandidates;
      if (!response.ok) {
        KeyboardLog.error(`candidate list refused: ${response.error}`);
        return [];
      }
      // The generation identifies the composition these belong to; selecting against a stale one
      // would act on a list the user is no longer looking at.
      this.generation = response.value.generation;
      // The snapshot carries no scheme; the list being the Hanja one is read from the view it expands.
      const hanja: boolean = this.candidateListOpen() && this.lastView?.scheme === SchemeTraits.KOREAN;
      const entries: ExpandedCandidate[] = [];
      for (const candidate of response.value.candidates) {
        entries.push({
          text: this.asTraditional(candidate.text),
          annotation: CandidateGlossPolicy.annotation(
            CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
            candidate.translation ?? null, this.candidateGlossEnabled),
          annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
            CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
            candidate.translation ?? null, this.candidateGlossEnabled),
          hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation)
        });
      }
      return entries;
    } catch (error) {
      KeyboardLog.error(`candidate list unreadable: ${describeError(error)}`);
      return [];
    }
  }

  /**
   * The candidate-management operations behind a long press: promote it, pin it to the front, let go
   * of a pinned position, or take the entry out of the dictionary altogether.
   */
  promoteCandidate(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.pinCandidate(this.handle, this.generation, index));
  }

  /** Position 1 is the front of the list, which is what 固定到首位 means. */
  fixCandidateFirst(index: number): void {
    this.fixCandidatePosition(index, 1);
  }

  fixCandidatePosition(index: number, position: number): void {
    if (this.handle === 0) {
      return;
    }
    // Checked here rather than trusted from the menu. Today the menu is the only caller and it only
    // ever builds slots 1 to 5, so this throws for nobody; it is here because this is the last point
    // before the slot becomes an argument to the Engine, and a session method is callable by
    // anything in the host.
    this.apply(client.fixCandidatePosition(this.handle, this.generation, index,
      CandidateManagementAction.validatePosition(position)));
  }

  clearCandidatePosition(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.clearCandidatePosition(this.handle, this.generation, index));
  }

  removeCandidate(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.removeCandidate(this.handle, this.generation, index));
  }

  /**
   * The Windows maintenance chord's target, checked before it is acted on.
   *
   * The hardware key says which slot; whether that slot holds a candidate the user dictionary can
   * be asked to forget is this side's business. Cloud, AI, Emoji and Japanese candidates come from
   * sources the Host API would refuse, and a chord that silently does nothing eight times is worse
   * than one that was never claimed — so the caller is told whether it was acted on.
   */
  /**
   * Throw away the Engine's candidate cache for this session.
   *
   * The other half of the Windows maintenance pair: the slot keys remove one candidate the
   * dictionary got wrong, this one clears everything the Engine has cached when the whole list looks
   * stale. Nothing the user typed is lost — the view is refreshed, not the composition.
   */
  resetCache(): void {
    if (this.handle === 0) {
      return;
    }
    this.applyView(client.resetCache(this.handle));
  }

  removeManagedCandidate(index: number): boolean {
    const view: EngineView | undefined = this.lastView;
    if (this.handle === 0 || view === undefined || index < 0
        || index >= view.candidates.length) {
      return false;
    }
    const candidate: Candidate = view.candidates[index];
    if (!CandidateManagementAction.candidateActionsAvailable(
        KeyboardScheme.engineSchemeName(view.scheme), candidate.source)) {
      return false;
    }
    this.apply(client.removeCandidate(this.handle, this.generation, index));
    return true;
  }

  /** Select by position in the full list rather than in the page. */
  chooseAny(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.selectAnyCandidate(this.handle, this.generation, index));
  }

  /** Commit to one reading of the digits typed so far. */
  chooseSpelling(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.chooseNineKeySpelling(this.handle, this.generation, index));
  }
}
