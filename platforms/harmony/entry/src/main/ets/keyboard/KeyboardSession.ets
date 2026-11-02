import {
  HANDWRITING_CANVAS_SIZE, HandwritingPoint, HandwritingStroke, HandwritingStrokePolicy
} from './input/HandwritingStrokePolicy';
import { DecorationCandidateStyle } from './candidate/DecorationCandidatePolicy';
import { CandidateLayoutPolicy, CandidateResolvedLayout } from './candidate/CandidateSkinPolicy';
import { CandidateCanvasScene, CandidateCanvasResource, CandidateCanvasPolicy } from './candidate/CandidateCanvasPolicy';
import { NativeCleanup } from './NativeCleanup';
import inputMethodEngine from '@ohos.inputMethodEngine';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import { HarmonyReadAloud } from '../speech/HarmonyReadAloud';
import { ReadAloudPolicy } from '../speech/ReadAloudPolicy';
import { statusUserId } from '../account/HarmonyAccountRequests';
import { AccountSessionAdoption } from '../account/AccountSessionAdoption';
import { utf8Length, utf8WriteComplete } from './Utf8';
import { TextPolicy } from './TextPolicy';
import { KeyboardLog, describeError } from './KeyboardLog';
import { PERSONAL_WORDBOOK_FAILURE, PersonalWordbookPolicy, PersonalWordbookRoute } from './PersonalWordbookPolicy';
import { DictionaryMaintenancePolicy } from './DictionaryMaintenancePolicy';
import { IdleDictionaryRetryPolicy, PersonalDictionaryDrain } from './IdleDictionaryRetryPolicy';
import { UserWordsMirror } from './UserWordsMirror';
import {
  KEYBOARD_MAX_CHANGES, KeyboardSyncOutcome, UserWordsMirrorPolicy, UserWordsReply
} from './UserWordsMirrorPolicy';
import { TypingStatisticsPolicy } from './TypingStatisticsPolicy';
import { KeyPressBatch, KeyPressFlush } from './KeyIdPolicy';
import { readTypingStatisticsEnabled, writeKeyPresses, writeTypingStatistics } from './TypingStatisticsWriter';
import { startKeyVibration } from './KeyVibration';
import taskpool from '@ohos.taskpool';
import { KeyboardSkin } from './skin/KeyboardSkin';
import { AppThemePalette, AppThemeSeed } from './skin/AppThemePalette';
import { AppThemeStore } from './skin/AppThemeStore';
import { CommonPhraseStarters } from './CommonPhraseStarters';
import { SettingsPageLink } from './SettingsPageLink';
import { ToolbarTools, ToolbarToolsDocument, TouchToolbarPolicy } from './TouchToolbarPolicy';
import { TouchHeightRelay, TouchLayoutStylePolicy, TouchPanelFace, TouchPanelShape } from './TouchLayoutStylePolicy';
import { KeyboardAdjustPolicy } from './KeyboardAdjustPolicy';
import { FloatingKeyboardPolicy } from './FloatingKeyboardPolicy';
import { OneHandedPolicy, OneHandedSidePanel } from './OneHandedPolicy';
import { SplitKeyboardPolicy, SplitKeyboardSettings } from './SplitKeyboardPolicy';
import { CustomKeyboardSkin, CustomSkinDocument } from './skin/CustomKeyboardSkin';
import {
  CandidateColors, CustomThemeDocument, GlobalTheme, ResolvedTheme, ResolveThemeRequest, ThemeCatalogEntry
} from './skin/GlobalTheme';
import { KeyboardFormFactor } from './KeyboardFormFactor';
import { AppEdition } from './AppEdition';
import { KeyboardScheme, KeyGrid, PreferenceMapping, SchemeDefinition } from './KeyboardScheme';
import { NineKeyShuangpinFace, normalizedNineKeyShuangpinProfile } from './input/NineKeyShuangpinKeypad';
import { NineKeySwipePolicy } from './input/NineKeySwipePolicy';
import { EditorPolicy, EditorTraits } from './input/EditorPolicy';
import { EditEchoLedger } from './input/EditEchoLedger';
import { CaretContextEcho } from './input/CaretContextEcho';
import { QuickPhraseCursor, QuickPhraseEditor } from './input/QuickPhraseCursor';
import { JapaneseSpacePolicy } from './input/JapaneseSpacePolicy';
import { KoreanCompositionPolicy } from './input/KoreanCompositionPolicy';
import { SchemeCompositionPolicy } from './input/SchemeCompositionPolicy';
import { SchemeTraits } from './SchemeTraits';
import { StrokeLayout } from './input/StrokeLayout';
import {
  CapitalizationMode, EnglishCapitalizationPolicy
} from './input/EnglishCapitalizationPolicy';
import { WubiCodeHintPolicy } from './input/WubiCodeHintPolicy';
import { KeyboardGeometry } from './KeyboardGeometry';
import { FullWidthInputPolicy } from './input/FullWidthInputPolicy';
import { ChineseHelpcodePolicy } from './input/ChineseHelpcodePolicy';
import { ChineseOutputPolicy } from './input/ChineseOutputPolicy';
import { KeyboardFeedback, FeedbackSettings, HapticPlan, HapticStrength } from './KeyboardFeedback';
import { HardwareKeyRouter, HardwareNavigationPreferences, HardwareSpelling } from './HardwareKeyRouter';
import vibrator from '@ohos.vibrator';
import common from '@ohos.app.ability.common';
import { InputDiagnosticPolicy } from './input/InputDiagnosticPolicy';
import { InputModeHudPolicy } from './InputModeHudPolicy';
import { HardwareKeyTarget } from './HardwareKeyDispatch';
import { EmojiPanelKey } from './emoji/EmojiPanelKeyPolicy';
import { MascotStickerStore } from './emoji/MascotStickerStore';
import { DEFAULT_VOICE_HOTKEY_BINDINGS, VoiceHotkeyAction, VoiceHotkeyBindings, VoiceHotkeyPolicy,
  VoiceKey } from './input/VoiceHotkeyPolicy';
import { HarmonyVoiceRecordingBehaviour } from './input/HarmonyVoiceRecordingBehaviour';
import { KeySoundPlayer } from './KeySoundPlayer';
import { KeySoundPolicy, PluginPreferenceDocument } from './KeySoundPolicy';
import { MusicPlayer } from './MusicPlayer';
import { MUSIC_OFF, MusicPolicy } from './MusicPolicy';
import {
  ResolvedTypingEffectDocument, TYPING_EFFECT_COMMIT, TYPING_EFFECTS_OFF, TypingEffect, TypingEffectPolicy,
  TypingEffectSettings
} from './TypingEffectPolicy';
import { EnglishCompletions, EnglishRefresh, EnglishReplacement, EnglishSuggestionPolicy }
  from './input/EnglishSuggestionPolicy';
import { EmailSuffixMatch, EmailSuffixPolicy, EmailSuffixReplacement } from './input/EmailSuffixPolicy';
import { ImeModeScopePolicy } from './input/ImeModeScopePolicy';
import { KeyGeometryRegistry, TouchKey } from './input/TouchPointPolicy';
import { ModeBindings, DEFAULT_MODE_BINDINGS } from './InputModeRouting';
import { FloatingToolbarLayout, ToolbarButton, ToolbarComponents, ToolbarState }
  from './FloatingToolbarLayout';
import { ToolbarSkin, ToolbarSkinPolicy } from './ToolbarSkinPolicy';
import { ToolbarImagePolicy, ToolbarImageResources, ToolbarImageScene, ToolbarImageLayoutReply } from './ToolbarImagePolicy';
import { DesktopSurface } from './DesktopSurface';
import { CandidateAnnotationPreferencePolicy } from './candidate/CandidateAnnotationPreferencePolicy';
import { CandidateManagementAction } from './candidate/CandidateManagementAction';
import { NextWordPredictionPolicy, PredictionButtons } from './candidate/NextWordPredictionPolicy';
import { CandidateGlossPolicy, CandidatePronunciationPolicy } from './candidate/CandidateGlossPolicy';
import { CorrectionMarkPolicy } from './candidate/CorrectionMarkPolicy';
import {
  CandidateGlossLayoutPolicy, CandidateGlossProviderState
} from './candidate/CandidateGlossLayoutPolicy';
import { CandidatePresentationPolicy } from './candidate/CandidatePresentationPolicy';
import {
  CandidateSkinCatalogPolicy, CandidateSkinPackage, CandidateSkinDecoration, CandidateSkinBackground,
  CandidateSkinAlign, CandidateSkinFit, CandidateSkinAnimation
} from './candidate/CandidateSkinCatalogPolicy';
import { CandidateSkinPackageIdentity, SkinFileStat } from './candidate/CandidateSkinPackageIdentity';
import { CandidateWidthPolicy } from './candidate/CandidateWidthPolicy';
import { CandidateFontSizePolicy } from './candidate/CandidateFontSizePolicy';
import { CandidateScalePolicy } from './candidate/CandidateScalePolicy';
import { CandidatePagerPolicy } from './candidate/CandidatePagerPolicy';
import { CandidateLogoPolicy } from './candidate/CandidateLogoPolicy';
import { CandidateNumberFontPolicy } from './candidate/CandidateNumberFontPolicy';
import { KeyboardMetrics } from './KeyboardMetrics';
import { NativeReplyPolicy } from './NativeReplyPolicy';
import { PrivacyGate, PrivacyRecord } from './PrivacyGate';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';
import { AiGenerationGate } from './AiGenerationGate';
import { OnlineCandidatePolicy, OnlineQuery, CloudHttpDescriptor } from './candidate/OnlineCandidatePolicy';
import {
  TranslationPolicy, TranslationQuery, TranslationPlanItem, TranslationEntry,
  TranslationProviderConfig, TencentTranslationConfig, NiuTransTranslationConfig, AiTranslationConfig, TranslationCandidate
} from './candidate/TranslationPolicy';
import { TranslationSensePolicy } from './candidate/TranslationSensePolicy';
import pasteboard from '@ohos.pasteboard';
import abilityAccessCtrl from '@ohos.abilityAccessCtrl';
import bundleManager from '@ohos.bundle.bundleManager';
import http from '@ohos.net.http';
import { installNoRedirectGuard } from '../network/HarmonyHttpSecurity';
import {
  ClipboardHistoryStore, ClipboardHistoryItem, ClipboardHistoryError, ClipboardFailure
} from './clipboard/ClipboardHistoryStore';
import { ClipboardHistoryPreferencePolicy } from './clipboard/ClipboardHistoryPreferencePolicy';
import { CloudClipboardBoundListing, CloudClipboardClient } from './clipboard/CloudClipboardClient';
import {
  CloudClipboardPolicy, CloudClipboardSendOutcome
} from './clipboard/CloudClipboardPolicy';
import {
  DEFAULT_VOICE_INPUT_CONFIGURATION, VoiceInputConfiguration, VoiceInputConfigurationPolicy
} from './input/VoiceInputConfiguration';
import {
  AiPolishPolicy, MAX_POLISH_SOURCE_CHARACTERS, PolishReplacement
} from './input/AiPolishPolicy';
import { VoiceRecognitionPolicy, VoiceTarget } from './input/VoiceRecognitionPolicy';
import { HarmonyVoicePolisher } from './input/HarmonyVoicePolisher';
import { AiEndpoint } from './settings/AiEndpoint';
import { SmartPunctuationSpacePolicy, SmartPunctuationSpaceSnapshot, SpaceConvertDecision }
  from './input/SmartPunctuationSpacePolicy';
import { SmartPunctuationContext } from './input/SmartPunctuationContext';
import { PunctuationLockPolicy } from './input/PunctuationLockPolicy';
import { PairedPunctuationPolicy, PairedPunctuationCompletion, PairedPunctuationCommit } from
  './input/PairedPunctuationPolicy';
import { ChineseSymbolFaces, PunctuationMarksView } from './input/ChineseSymbolFaces';
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
import { SharedStateMirror } from './SharedStateMirror';
import { FingerprintCache } from './SharedStateMirrorPolicy';
import { SharedStateSync } from './SharedStateSync';
import { InlinePreeditPolicy, InlinePreeditStyle } from './input/InlinePreeditPolicy';
import {
  CompositionBoundary, CompositionBoundaryAction, CompositionBoundaryPolicy
} from './input/CompositionBoundaryPolicy';
import { EngineViewNumericFields, EngineViewValuePolicy } from './input/EngineViewValuePolicy';

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
  /** 「仅当前高亮」生效、这一页会请求释义时，高亮那一行为真；为假时 JSON 里没有这个键。2in1 候选窗据此在同一页里保持宽度，见 `CandidateWidthPolicy.steadyWidthVp`。 */
  translation_reserved?: boolean;
  /** 双拼触屏纠错按首选规则坐上第 0 位的那一行为真；其余行 JSON 里没有这个键（#10018），见 `CorrectionMarkPolicy`。 */
  confident_correction?: boolean;
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
  /** 引擎把这一行作为有把握的纠错放在首选（`confident_correction`），视图据此画强调色下划线、读屏加「，已纠正」。 */
  readonly confidentCorrection: boolean;
  /** 「显示读音」打开时这一行的读音（英文 `/…/` 音标，日文和韩文罗马字，法、西、俄、德语的 IPA），画在释义下面单独一行；没有读音或开关关着时为 ""。只用于显示，从不上屏。 */
  readonly pronunciation: string;
}

interface EngineView extends EngineViewNumericFields {
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
  /** Which spellings the digits typed so far could be, offered while the nine-key grid is up. 依次是完整拼音（小写）、下一个数字键上能起头的字母（大写，选它只限定下一个音节的首字母）、没有锁定任何音节时的数字本身（选它上屏这个数字）。 */
  nine_key_spellings: string[];
  /** 全拼九键组字时首选候选的拼音读音（`ning'bai`），读音行画它而不是数字；其他情况为空。旧的原生库不带这个字段。 */
  nine_key_reading?: string;
  /** 引擎当前的组码网格（`"none"`、`"nine_key"`、`"fourteen_key"` 等）；五笔 14 键据此按组对照候选编码的已打部分。旧的原生库不带这个字段。 */
  key_grid?: string;
  /** 九宫格候选当前只留单字，组字结束时恢复为假。 */
  nine_key_single_character?: boolean;
  /** 九宫格候选当前的笔顺前缀（`hspnz`），空表示不按笔画筛选；组字结束时清空。 */
  nine_key_strokes?: string;
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
  /** The non-letter keys the Engine takes as input now: the active local mode's digits and operators, or with nothing composed `/` and `@` for the modes they open. */
  spelling_symbols: string;
  dedicated_english: boolean;
  /** The scheme's openable candidate list is showing: the Korean Hanja list, the Zhuyin list. */
  candidate_list_open: boolean;
  /** 引号与方头括号样式（`quote_style`、`bracket_style`），host-api 已按繁体输出开关解析好。 */
  punctuation_marks?: PunctuationMarksView;
  /** 上屏后联想（`next_word_prediction`）：候选是选词上屏后的建议，`editing_text`、`preedit` 为空，只有点选才上屏。普通状态 JSON 里没有这个键。 */
  prediction?: boolean;
}

// The stored preferences, read and written whole. Only the fields the keyboard changes are named;
// the rest travel through untouched as a parsed value, because rewriting a document one does not
// fully understand is how settings get silently dropped.
interface PreferencesSnapshot {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
}

/** `msime_client_set_key_geometry` 的答复里键盘只看是否收下。 */
interface GeometryReply {
  ok?: boolean;
  error?: string;
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

interface TypingStatisticsVoiceAction {
  operation: string;
  day: string;
  milliseconds: number;
}

interface TypingStatisticsVoiceRequest {
  directory: string;
  action: TypingStatisticsVoiceAction;
}

interface TypingStatisticsFlushAction {
  operation: string;
}

interface TypingStatisticsFlushRequest {
  directory: string;
  action: TypingStatisticsFlushAction;
}

interface MobileClipboardAction {
  operation: string;
  text?: string;
  pinned?: boolean;
}

interface MobileClipboardRequest {
  directory: string;
  // 容量偏好在 stateDirectory，历史在它的父目录；不传时 host-api 从 directory 读偏好，会按默认 50 条淘汰。
  preferences_directory: string;
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

// The local emoji catalog lives in msime-others.db beside the dictionaries, so the query carries the
// resource directory rather than going through a session.
/** 共享查询的前缀和条数；手机通过 `correction` 明确请求原文及唯一纠错。 */
interface HandwritingNativeRequest {
  model: string;
  request: HandwritingInkRequest;
}

interface HandwritingInkRequest {
  language: string;
  strokes: HandwritingRecognitionStroke[];
}

interface HandwritingRecognitionStroke {
  points: HandwritingPoint[];
}

interface HandwritingNativeReply {
  ok: boolean;
  value?: HandwritingNativeCandidates;
  error?: string;
}

interface HandwritingNativeCandidates {
  candidates: string[];
}

interface EnglishCompletionRequest {
  prefix: string;
  limit: number;
  correction?: boolean;
  completions?: boolean;
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
  /** 同 CandidateEntry.confidentCorrection。 */
  readonly confidentCorrection: boolean;
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

interface AiTranslationRequestInput {
  text: string;
  source_language: string;
  target_language: string;
}

interface AiTranslationRequest {
  config: AiTranslationConfig;
  input: AiTranslationRequestInput;
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
  /** 手机上拼写正写在输入框里，候选栏不画读音；见 `KeyboardSession.compositionInField`。 */
  readonly compositionInField: boolean;
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
  /** 九宫格候选筛选的当前状态，展开面板的「全部/单字」和笔画栏照它画。 */
  readonly nineKeySingleCharacter: boolean;
  readonly nineKeyStrokes: string;
  /** View 报告的引号与方头括号样式，键盘画符号键和 123 层键面时用。 */
  readonly cornerQuotes: boolean;
  readonly hollowBrackets: boolean;
  /** View 报告的顿号键开关：顿号键面随它画。 */
  readonly enumerationCommaBackslash: boolean;
  readonly enumerationCommaSlash: boolean;
  /** 候选是上屏后联想而不是组字的候选，见 `NextWordPredictionPolicy`：回车、空格按空闲处理，长按不管理候选，候选栏按偏好放关闭键。 */
  readonly prediction: boolean;
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
  /** 第一个模板光标位置之后的字面尾段，空串表示正文末尾。 */
  commit_cursor_suffix?: string | null;
  /** Something the Engine wants the user told, such as a dictionary that could not be opened. */
  diagnostic: string | null | undefined;
  view: EngineView;
}

interface CommitContext {
  /** 共享运行时已转换的混合提交禁止宿主再次整行转换。 */
  script_conversion?: boolean;
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

interface NineKeyShuangpinRawFace {
  key?: string;
  initials?: string[];
  finals?: string[];
  zero_initial?: boolean;
}

interface NineKeyShuangpinRawLayout {
  keys?: NineKeyShuangpinRawFace[];
}

interface NineKeyShuangpinLayoutReply {
  ok: boolean;
  value?: NineKeyShuangpinRawLayout;
}

interface CreateResponse {
  ok: boolean;
  value: CreateValue;
  error: string;
}

/** 一个会话开关，应答其设置后的值，例如 `setPrivateSession`。 */
interface FlagResponse {
  ok: boolean;
  value: boolean;
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
  /** Always `'base64'`: the default `bytes` array costs the shared layer a JSON value per byte (about 64 MiB for a 1 MiB image, 512 MiB at the 8 MiB resource limit) and `JSON.parse` a number per byte, while every reader here ends at base64 anyway. */
  encoding: string;
}

interface SkinResourceValue {
  contentType: string;
  bytesBase64: string;
}

/** A candidate decoration image, already encoded for the component that draws it. */
interface CandidateSkinImage {
  image: string | null;
  aspectRatio: number;
}

/** What every path that has no decoration to draw returns, rather than a fresh literal each time. */
const NO_CANDIDATE_SKIN_IMAGE: CandidateSkinImage = { image: null, aspectRatio: 1 };

/** `msime_client_resolve_app_theme` 的请求：存储的应用主题、本地日历月份（`siji` 跟随它）和模式。 */
interface ResolveAppThemeRequest {
  app_theme: string;
  month: number;
  dark: boolean;
}

/** 常驻的键盘自行重新查看应用主题的间隔，这样不必等到焦点变化季节也会更替。与 Android `ImeStyler.refreshAppTheme` 使用的间隔相同。 */
const APP_THEME_CHECK_INTERVAL_MS: number = 60 * 1000;

/** 一次 `msime_client_common_phrases` 操作；只声明键盘会发出的那两种。 */
interface CommonPhrasesAction {
  operation: string;
  text?: string;
}

interface CommonPhrasesRequest {
  directory: string;
  action: CommonPhrasesAction;
}

interface CommonPhraseEntry {
  id?: string;
  text?: string;
  pack?: string;
}

interface CommonPhrasesDocument {
  phrases?: CommonPhraseEntry[];
  packs?: Object[];
}

interface CommonPhrasesReply {
  ok?: boolean;
  value?: CommonPhrasesDocument;
  error?: string;
}


// Enough for the sentence rule to walk back over trailing spaces and closing quotes to the mark that
// ended the last sentence. A caret further than this from one is in the middle of a sentence by any
// reading, and the call is made on every text change, so it stays bounded.
const MAX_CAPITALIZATION_CONTEXT_CHARACTERS: number = 64;
// `touch_panel_face.json` 只有两个布尔值，读的时候超过这个大小就当没记过。
const TOUCH_PANEL_FACE_MAX_BYTES: number = 256;
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
export interface PrepareRequest {
  resources: string;
  state_root: string;
  edition?: string;
}

interface SnapshotQueueProcessAction {
  operation: string;
  directory: string;
  staging_root: string;
  options: Object;
  account_id: string | null;
  skip_when_idle: boolean;
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
  skip_when_idle: boolean;
}

// Only the fields the keyboard acts on are declared. The rest of the document is forwarded to create
// untouched, so naming it here would only invite the two shapes to drift apart.
interface PreparedPreferences {
  default_ime_mode: string;
  /** 用户的「学习」开关。隐私模式只在会话自己的副本里把它关掉（`PrivacyGate`）。 */
  learning?: boolean;
  /** `app` remembers the mode per application; `global` shares one across all of them. */
  ime_mode_scope?: string;
  scheme: string;
  /** The Wubi remaining-code hint, always written by the shared store and on by default. */
  wubi_code_hint: boolean;
  /** Whether the mode badge is drawn after a Chinese/English switch. Defaults to on. */
  input_mode_hud?: boolean;
  /** Read-only English word completions while typing directly in English. Defaults to on. */
  english_suggestions?: boolean;
  shuangpin_profile: string;
  /** 双拼触屏字母组码布局；缺省为 26 键，与共享偏好保持一致。 */
  touch_shuangpin_layout?: string;
  /** 五笔触屏字母键布局：`twenty_six_key`（缺省，不写进文档）或 `fourteen_key`；与全拼、双拼布局独立。 */
  touch_wubi_layout?: string;
  nine_key_shuangpin_profile?: string;
  nine_key_shuangpin_custom_profile?: Object;
  /** `wubi86` 或 `wubi98`；旧文档里没有这个键，按 86 版。引擎自己从转交的整份文档里读它选表，这里只用来给五笔方案起名。 */
  wubi_profile?: string;
  touch_keyboard_layout: string;
  /** The shared global theme id; the ids and their palettes come from the theme catalog. */
  global_theme: string;
  /** What the `custom` theme is made of. Absent means the shared default: over `system`, with nothing set. */
  custom_theme?: CustomThemeDocument;
  candidate_theme: string;
  candidate_selected_bar: boolean | null;
  candidate_follow_cursor: boolean;
  number_row_selection: boolean;
  /** 上屏后联想时候选栏右端的按键样式（`expand_only`、`close_and_expand`、`close_only`）；缺省（不写进文档）是只有展开键。 */
  next_word_prediction_buttons?: string;
  navigation: PreparedNavigationPreferences;
  candidate_layout: string;
  candidate_preedit_style: string;
  /** Inline spelling in the document; the 2in1 draws it as editor preview text. */
  tsf_preedit_style?: string;
  candidate_font_size: number;
  candidate_preedit_font_size: number;
  /** 候选栏大小，50..200；client-core 总会写出它，旧文档缺省时按 100。 */
  candidate_scale_percent?: number;
  /** 50..100; absent in documents that never changed it, which means an opaque card. */
  candidate_opacity_percent?: number;
  /** 0..32 points; absent or null leaves the card to the skin package, then the host. */
  candidate_corner_radius?: number | null;
  candidate_font_family: string;
  /** Named first so Latin comes from it and Han falls through; absent means one family answers. */
  candidate_english_font?: string | null;
  candidate_fallback_fonts: string[];
  // 屏幕键盘自己的明暗（'follow'、'dark' 或 'light'）。各界面一律跟随颜色模式（`HostCapabilities::surface_themes_follow`）之后，宿主不再读它和下面五项各界面的明暗，字段留着，偏好文档照旧解析。
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
  /** 共享默认关闭；旧文档没有这个键时按关闭处理。 */
  candidate_pronunciation?: boolean;
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
  /** 26 键双拼字母键底部的声母/韵母提示；旧文档没有这个键，按开。 */
  touch_shuangpin_key_hints?: boolean;
  /** 全拼 26 键组字时 ⇧ 保留（臂辅助码）而不换成「分词」；旧文档没有这个键，按开。 */
  touch_quanpin_shift?: boolean;
  /** 中文 26 键底行样式（standard / comma_period / long_space / emoji）；等于默认值时 client-core 不写进文档。 */
  touch_twenty_six_key_style?: string;
  /** 14 键与 18 键底行样式（standard / long_space / emoji）；默认值不写进文档。 */
  touch_fourteen_key_style?: string;
  /** 九键底行样式（standard / comma_period / emoji）；默认值不写进文档。 */
  touch_nine_key_style?: string;
  /** 26/14/18 键字母上方的 1 到 0 数字行；默认关，不写进文档。 */
  touch_number_row?: boolean;
  /** 26 键底行 符 排在 123 左边；默认关，不写进文档。 */
  touch_twenty_six_symbol_first?: boolean;
  /** 九宫格数字键盘里空格的位置（platform_default / right_column / after_zero / before_zero）；默认值不写进文档。 */
  touch_number_pad_space?: string;
  /** 九键左侧符号栏字母层的表（拼音九键、九键双拼和笔画共用）；空表是本端出厂表，不写进文档。 */
  touch_nine_key_sidebar_symbols?: string[];
  /** 九宫格数字键盘左侧符号栏的表（#+= 之外的部分）；空表是本端出厂表，不写进文档。 */
  touch_nine_key_digit_sidebar_symbols?: string[];
  /** 大屏单手模式空出来的一侧放什么（off / number_pad）；默认值不写进文档。 */
  touch_one_handed_side_panel?: string;
  /** 大屏分离式键盘的主开关；默认关，不写进文档。 */
  touch_keyboard_split?: boolean;
  /** 分离式键盘开着时竖放也分离；默认关。 */
  touch_keyboard_split_portrait?: boolean;
  /** 分离式键盘的启用宽度（vp），480–1000、步进 20；默认 600，不写进文档。 */
  touch_keyboard_split_threshold?: number;
  /** 分离式键盘中间空隙的百分比，10–40、步进 5；默认 25，不写进文档。 */
  touch_keyboard_split_gap?: number;
  /** 中/英键在空格哪一边（right / left）；默认 right，不写进文档。 */
  touch_language_key_side?: string;
  /** 九键滑动输入数字（off / up / down）；默认 off，不写进文档。 */
  touch_nine_key_swipe_digits?: string;
  /** 手机工具栏的五个工具和显示方式；等于默认值时 client-core 不写进文档。 */
  harmony_toolbar?: ToolbarToolsDocument;
  /** Shared privacy switch for the private clipboard history file and keyboard entry point. */
  clipboard_history: boolean;
  voice_input: VoiceInputConfiguration;
  ai_assistant: PreparedAiAssistant;
  touch_keyboard_schemes: PreparedTouchKeyboardSchemes;
  keybindings: PreparedKeybindings;
  /** Sound packs and the rest of the plugin settings; absent while every part is at its default. */
  plugins?: PluginPreferenceDocument;
  /** 本地输入模式的开关；这里只读 / 指令那一项。 */
  local_modes?: LocalModesDocument;
}

/** 共享偏好 `local_modes` 里键盘要读的部分：/ 指令缺省关。 */
interface LocalModesDocument {
  command?: boolean;
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
  /** 只在桌面装饰图成功读取时提供文字声明。 */
  readonly candidateDecorationCandidate: DecorationCandidateStyle | null;
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
  /** 共享扫描器校验后的 canvas 快照；逐键只消费，不访问文件。 */
  readonly candidateCanvas: CandidateCanvasScene | null;
  readonly candidateSkinLayout?: CandidateResolvedLayout | null;
  readonly candidateBackgroundFit: CandidateSkinFit;
  readonly candidateBackgroundOpacity: number;
  /** The bounded native motion presets from the selected candidate package. */
  readonly candidateAnimation: CandidateSkinAnimation;
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
  /** `touch_shuangpin_layout`：双拼手机键盘的 26 / 14 / 18 键布局。 */
  readonly shuangpinLayout: string;
  /** `touch_wubi_layout`：五笔手机键盘的 26 / 14 键布局，切换 86 / 98 版后保留。 */
  readonly wubiLayout: string;
  /** 归一后的五笔版本，决定五笔方案显示「86 五笔」还是「98 五笔」。 */
  readonly wubiProfile: string;
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
  /** 触屏键盘的「候选栏大小」（`candidate_scale_percent`）：工具条高度和候选栏的字号、行高都按它缩放（#10137），上面两项字号已经乘过它。2in1 恒为 100，候选卡片只随字号调整。 */
  readonly candidateScalePercent: number;
  readonly candidateFontFamily: string;
  readonly candidateEnglishFont: string | null;
  readonly candidateFallbackFonts: string[];
  /** The 2in1 candidate window's colours in its mode, from the resolved theme with the native tokens behind it. */
  readonly candidateColors: CandidateColors;
  readonly keySpacingTenths: number;
  readonly rowSpacingTenths: number;
  readonly heightAdjustment: number;
  /** `touch_shuangpin_key_hints`：关掉时 26 键双拼不画键位提示。 */
  readonly shuangpinKeyHints: boolean;
  /** `touch_quanpin_shift`：打开时全拼 26 键组字保留 ⇧，点一下臂辅助码；缺省为开。 */
  readonly quanpinShift: boolean;
  /** `harmony_toolbar`：手机工具栏上显示哪些工具，整行是否隐藏。 */
  readonly toolbarTools: ToolbarTools;
  /** `touch_twenty_six_key_style`：中文 26 键的底行样式，不认识的值按 `standard`（`TouchLayoutStylePolicy`）。 */
  readonly twentySixKeyStyle: string;
  /** `touch_fourteen_key_style`：14 键与 18 键的底行样式。 */
  readonly fourteenKeyStyle: string;
  /** `touch_nine_key_style`：九键字母层的底行样式。 */
  readonly nineKeyStyle: string;
  /** `touch_number_row`：26/14/18 键字母上方画 1 到 0。 */
  readonly numberRow: boolean;
  /** `touch_twenty_six_symbol_first`：26 键底行 符 排在 123 左边（`TouchLayoutStylePolicy.symbolFirst`）。 */
  readonly twentySixSymbolFirst: boolean;
  /** `touch_number_pad_space`：九宫格数字键盘里空格的位置，`platform_default` 和不认识的值按鸿蒙原来的 `before_zero`（`TouchLayoutStylePolicy.numberPadSpace`）。 */
  readonly numberPadSpace: string;
  /** `touch_nine_key_sidebar_symbols`：九键字母层左列的自定义表，空表用出厂的 , . ?（`NineKeyLayout.sideMarks`）。 */
  readonly nineKeySidebarSymbols: string[];
  /** `touch_nine_key_digit_sidebar_symbols`：九宫格数字键盘左列的自定义表，空表用出厂的四则运算（`NineKeyLayout.digitSidebar`）。 */
  readonly nineKeyDigitSidebarSymbols: string[];
  /** `touch_one_handed_side_panel`：大屏单手模式空出来的一侧放什么，不认识的值按空着（`OneHandedPolicy.sidePanel`）。 */
  readonly oneHandedSidePanel: OneHandedSidePanel;
  /** `touch_keyboard_split`、`touch_keyboard_split_portrait`、`touch_keyboard_split_threshold` 和 `touch_keyboard_split_gap`：大屏分离式键盘，不合规的值按默认值（`SplitKeyboardPolicy.settings`）。 */
  readonly splitKeyboard: SplitKeyboardSettings;
  /** `touch_language_key_side` 为 `left`：触屏底行的 中/英 排到空格左边（`TouchLayoutStylePolicy.languageKeyLeft`），其余值和缺键都按右。 */
  readonly languageKeyLeft: boolean;
  /** `touch_nine_key_swipe_digits`：拼音九键字母层 1–9 键沿这个方向滑动输入数字、反方向弹出字母，缺键和不认识的值按 `off`（`NineKeySwipePolicy.mode`）。 */
  readonly nineKeySwipeDigits: string;
}

/** 刚保存的整份偏好文档，按键盘关心的字段读回来，替换缓存里那份准备时的偏好。 */
interface PreparedSnapshot {
  preferences: PreparedPreferences;
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

interface PreferencesUpdateResponse {
  ok: boolean;
  value: PreferencesUpdateValue;
  error: string;
}

interface PreferencesUpdateValue {
  view: EngineView;
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

/** 词库更新状态与后台下载（`dictionary` 的 `updates`）交给 `client.dictionary` 的引擎选项，字段名与 Rust 的选项相同；ArkTS 不允许无类型的对象字面量，所以单独声明。没有语言词库目录时不带 `language_dictionaries`。 */
interface DictionaryUpdateOptions {
  api_version: number;
  resources: string;
  preferences_directory: string;
  user_data: string;
  cache: string;
  dictionaries: string;
  preferences: Record<string, Object>;
  edition: string;
  language_dictionaries?: string;
}

interface PersonalDictionarySyncValue {
  pending_count: number;
}

interface DictionaryCollectionsFlushValue {
  sent?: number;
}

interface DictionaryCollectionsFlushResponse {
  ok: boolean;
  value: DictionaryCollectionsFlushValue;
  error?: string;
}

/** 命名词库接受的操作，与 client-services 的 `DictionaryCollectionsAction` 一致；别的一律在桥上拒绝。 */
const DICTIONARY_COLLECTION_OPERATIONS: string[] = [
  'load', 'create', 'rename', 'delete', 'set_enabled', 'add_words', 'remove_words', 'import',
  'install_community', 'queue_words', 'flush', 'third_party_matches',
];

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

/**
 * Owns the native session for the lifetime of the extension ability and turns key presses into
 * committed text. The ability creates it; the panel's UI reads state back through onChanged.
 *
 * Deliberately thin: the Engine decides what a key means, this only forwards and applies the result.
 */
export class KeyboardSession implements HardwareKeyTarget {
  static readonly shared: KeyboardSession = new KeyboardSession();
  /** 一次录音最多计入语音时长的上限，沿用 Android 的界限：识别器一直开着不等于口述了十分钟。 */
  static readonly MAX_VOICE_RECORD_MS: number = 600000;

  private handle: number = 0;
  /** 本会话注册过的 26 键双拼键位几何，会话一换就作废。 */
  private keyGeometry: KeyGeometryRegistry = new KeyGeometryRegistry();
  private inputClient: inputMethodEngine.InputClient | undefined = undefined;
  // 系统收起键盘后编辑框可能仍有焦点，空闲工作不能只靠 inputClient 判断是否显示。
  private idleDictionaryVisible: boolean = false;
  private personalDictionarySettled: boolean = true;
  private personalDictionaryTimer: number = -1;
  // 个人词库排空连续没有进展的次数，以及上一次同步之后队列里剩的条数（不知道时为 -1），用来给重建会话的重试退避，见 `IdleDictionaryRetryPolicy`。
  private personalDictionaryStalls: number = 0;
  private personalDictionaryPending: number = -1;
  // 与设置应用交换「我的词条」（`UserWordsMirrorPolicy`）：建会话之前要不要 sync、上次做完时设置应用发布的标记，以及失去焦点时 publish 的节流。
  private userWordsDue: boolean = true;
  private userWordsMergedMark: string | null = null;
  private userWordsFailures: number = 0;
  private userWordsPublishing: boolean = false;
  private userWordsPublishedJournal: string | null = null;
  private userWordsPublishMs: number = 0;
  private snapshotOptions: Object | undefined = undefined;
  private snapshotQueueTimer: number = -1;
  // 快照队列连续几轮查到待处理请求、为它重建了会话却仍没处理掉，重建后的下一次检查按它退避。
  private snapshotQueueStalls: number = 0;
  // Key press counts for the key heatmap. Nothing is held while statistics are off, which is read from the store on every focus; until the editor's attribute has said what kind of field it is, a press could be a password's, so none is counted.
  private keyStatisticsEnabled: boolean = false;
  private keyStatisticsEditorKnown: boolean = false;
  private readonly keyPresses: KeyPressBatch = new KeyPressBatch();
  private keyFlushTimer: number = -1;
  // 提交、语音时长和 flush 都排在这一个 SequenceRunner 上，在 TaskPool 线程里调用原生统计：存储要加文件锁、读写整份文件，不能占着 UI 线程；排成一列是为了让提交按输入顺序到达存储（活跃时长按相邻提交的间隔算），也让 flush 落在它之前排入的记录之后。
  private readonly typingStatisticsWrites: taskpool.SequenceRunner = new taskpool.SequenceRunner();
  // 上次 flush 之后是否又送出过提交。原生层把提交攒在进程内存里，最多三十秒或 flush 时才写盘；没有新提交时收起键盘、换编辑框不必再排一次 flush。
  private typingStatisticsUnflushed: boolean = false;
  // 原生库拒绝过 flush 并已记过一次日志。拒绝多半每次都会重复（例如统计目录不可用），只记一次，免得每次收起键盘都写一条。
  private typingStatisticsFlushRefusalLogged: boolean = false;
  // 最近一次排入的 flush。收起键盘紧接着失去焦点时，第二次 flush 没有新提交可写，但发布统计文件仍要等第一次写完。
  private typingStatisticsFlushed: Promise<void> = Promise.resolve();
  private editorGeneration: number = 0;
  /** 同步编辑器调用重入时，较新的转换持有后续预编辑。 */
  private applyGeneration: number = 0;
  /** 正文可能要等宿主布局才落地；只保留当前请求，回声完成后再写最新预编辑。 */
  private phraseCursor: QuickPhraseCursor | null = null;
  private phraseCursorTimer: number = -1;
  private phraseCursorEditing: boolean = false;
  private phraseCursorOwnEcho: boolean = false;
  private phraseCursorCurrent: () => boolean = (): boolean => false;
  // Bumped only by attach/detach: an idle Engine restart (which bumps editorGeneration) must not
  // drop the focused editor's attribute.
  private attributeGeneration: number = 0;
  // 编辑器把每次改动异步回报为 `KeyboardDelegate.textChange`。自己的每次写入都在这里记一笔，等到回声才消掉，免得输入法自己的写入被当成宿主修改，结束引擎有意留着的组字；没等来回声的记录到时作废，见 `EditEchoLedger`。
  private readonly ownEdits: EditEchoLedger = new EditEchoLedger();
  private inlinePreeditStyle: InlinePreeditStyle = 'raw';
  /** Whether the focused editor said it takes preview text; learned from its attribute. */
  private previewSupported: boolean = false;
  /** The preview text the editor holds now, empty when it holds none. */
  private previewText: string = '';
  private replyContextGeneration: number = 0;
  private voiceTarget: VoiceTarget | null = null;
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
  private englishCorrectionRejectedPrefix: string = '';
  // The completion list currently on the strip, the prefix it was asked for, and a counter that
  // makes every earlier answer stale. A reply that arrives after the word moved on is discarded.
  private englishSuggestions: string[] = [];
  private englishSuggestionPrefix: string = '';
  /** 候选栏当前的英文建议第二行是否为纠错；只有补全时点原文不算拒绝纠错。 */
  private englishSuggestionCorrected: boolean = false;
  private englishSuggestionEpoch: number = 0;
  private englishSuggestionRequested: string = '';
  // 候选栏上正在给的邮箱后缀，以及它们接上的那个 `@` 之后已打的域名；没有时为 null。
  private emailSuffixMatch: EmailSuffixMatch | null = null;
  // 邮箱后缀最近一次按读到的上文匹配之后，编辑器是否只回报了输入法自己改动的回声；是的话选区回报不再重读，见 `CaretContextEcho`。
  private readonly caretContextEcho: CaretContextEcho = new CaretContextEcho();
  // The Windows voice shortcuts, which only a machine with a hardware keyboard can press.
  private voiceHotkeys: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  // Whether the recognizer is running. The view owns it and says so; the shortcuts need to know,
  // because Space and Escape belong to a composition at every other moment.
  private voiceRecording: boolean = false;
  // 当前录音的开始时间，以及上一次未取消的录音时长：即「统计」页的语音时长，在识别结果进入编辑框时记录（对应 Android 的 `elapsedMillis`）。
  private voiceRecordingStartedMs: number = 0;
  private voiceRecordedMs: number = 0;
  private voiceBehaviour: HarmonyVoiceRecordingBehaviour = new HarmonyVoiceRecordingBehaviour();
  // Key, melody, commit and achievement sounds. Configured off on a phone, which keeps its own key feedback.
  private readonly soundPlayer: KeySoundPlayer = new KeySoundPlayer();
  private readonly musicPlayer: MusicPlayer = new MusicPlayer();
  /** 表情面板里随包的水杉娘表情包；扩展在创建时用自己的 `resourceDir` 配置它，见 `MascotStickerStore.use`。 */
  readonly mascotStickers: MascotStickerStore = new MascotStickerStore();
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
  // 网格也归引擎管：九键下它决定一个 2 是什么意思，14/18 键下它把一组字母解成拼音，都要先告诉它网格开着。取值见 `KeyGrid`。
  private keyGrid: number = KeyGrid.NONE;
  private nineKeyShuangpinRequest: string = '';
  private nineKeyShuangpinFaceCache: NineKeyShuangpinFace[] = [];
  // 还没读到 Engine 的视图时按本版本的默认方案算（full 是全拼）。
  private schemeName: string = AppEdition.current().defaultScheme;
  private profileName: string = 'xiaohe';
  private clipboardHistoryEnabled: boolean = false;
  // The two optional candidate annotations. Held here rather than in KeyboardPreferences because the
  // session builds the candidate entries itself; the view only draws what it is handed.
  private wubiCodeHint: boolean = true;
  private candidateEnglishGloss: boolean = false;
  private candidateGlossEnabled: boolean = false;
  private candidateGlossRows: number = 0;
  // 「显示读音」：开关来自共享偏好，表是随 HAP 暂存到引擎资源旁的 pronunciations/en-phonetic.db 和 target-pronunciations/<语言>-phonetic.db。读音按「语言 + 文字」缓存，与候选代次无关。
  private candidatePronunciationEnabled: boolean = false;
  private candidatePronunciationAvailable: boolean = false;
  private candidatePronunciations: Map<string, string> = new Map<string, string>();
  // Mirrored for the function panel's 模糊音 tile; the rules themselves stay in the document.
  private fuzzyPinyin: boolean = false;
  // Read before the panels are built, so a badge the user turned off is never created at all.
  private modeBadge: boolean = true;
  private resourceDirectory: string = '';
  private stateDirectory: string = '';
  /** 扩展上下文的 `tempDir`，见 `useVoiceContext`。 */
  private voiceScratchDirectory: string = '';
  // 共享沙箱里与设置应用交换状态的镜像，没有共享目录时为 null；`sharedFullMode` 报告键盘此刻是否处于完整体验模式，只有那时键盘才能写共享目录。
  private sharedMirror: SharedStateMirror | null = null;
  private sharedFullMode: () => boolean = (): boolean => false;
  private cloudClipboardClient: CloudClipboardClient | undefined = undefined;
  private systemDark: boolean = false;
  // What the editor wants its return key to do. Sending this rather than a newline is what makes the
  // key search in a search field and send in a message field, which is the whole point of having it.
  private enterKey: number = inputMethodEngine.ENTER_KEY_TYPE_UNSPECIFIED;
  /** 扩展能力建手机面板时定下的形状（`decideFirstTouchPanel`），视图出现时从它出发核对高度。 */
  private openedTouchPanel: TouchPanelShape | null = null;
  /** 扩展能力建手机面板时算进第一帧高度的底部抬高（vp，`KeyboardAdjustPolicy.liftVp`），视图出现时从它出发核对。 */
  private openedTouchLiftVp: number = 0;
  /** 上次读到或写下的 `touch_panel_face.json` 内容，键面没变时不重写。 */
  private touchPanelFaceWritten: string = '';
  private traits: EditorTraits = {
    text: true, password: false, uri: false, email: false, noSuggestions: false, digits: false
  };
  /** What the attached editor asked for, already reconciled with the kind of field it is. */
  private capitalization: CapitalizationMode = CapitalizationMode.NONE;
  private appearance: KeyboardPreferences = {
    dark: false,
    globalThemeId: 'system',
    candidateMinWidthVp: CandidateWidthPolicy.MIN_WIDTH_VP,
    candidateDecorationImage: null,
    candidateDecorationCandidate: null,
    candidateDecorationTopVp: 0,
    candidateDecorationWidthVp: 0,
    candidateDecorationAspectRatio: 1,
    candidateDecorationAlign: 'right',
    candidateCornerRadiusVp: null,
    candidateCornerRadiusUser: false,
    candidateBackgroundImage: null,
    candidateCanvas: null,
    candidateBackgroundFit: 'cover',
    candidateBackgroundOpacity: 1,
    candidateAnimation: { enter: 'none', exit: 'none', selection: 'none', durationMs: 0, staggerMs: 0 },
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
    scheme: AppEdition.current().defaultScheme,
    shuangpinProfile: 'xiaohe',
    shuangpinLayout: 'twenty_six_key',
    wubiLayout: 'twenty_six_key',
    wubiProfile: KeyboardScheme.WUBI_86,
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
    candidateScalePercent: 100,
    candidateFontFamily: 'Noto Sans SC',
    candidateEnglishFont: null,
    candidateFallbackFonts: ['Noto Sans SC', 'Microsoft YaHei'],
    candidateColors: GlobalTheme.candidateColors(null, false),
    keySpacingTenths: KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
    rowSpacingTenths: KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
    heightAdjustment: KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP,
    shuangpinKeyHints: false,
    quanpinShift: true,
    toolbarTools: TouchToolbarPolicy.DEFAULT,
    twentySixKeyStyle: TouchLayoutStylePolicy.STANDARD,
    fourteenKeyStyle: TouchLayoutStylePolicy.STANDARD,
    nineKeyStyle: TouchLayoutStylePolicy.STANDARD,
    numberRow: false,
    twentySixSymbolFirst: false,
    numberPadSpace: TouchLayoutStylePolicy.BEFORE_ZERO,
    nineKeySidebarSymbols: [],
    nineKeyDigitSidebarSymbols: [],
    oneHandedSidePanel: OneHandedSidePanel.OFF,
    splitKeyboard: SplitKeyboardPolicy.DEFAULTS,
    languageKeyLeft: false,
    nineKeySwipeDigits: NineKeySwipePolicy.OFF
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
    candidate_limit: 3, prompt_id: 'custom_1', prompt_custom_1: '',
    prompt_custom_2: '', prompt_custom_3: ''
  };

  onChanged: ((state: KeyboardState) => void) | undefined = undefined;
  /** The view removes clipboard history immediately when the shared privacy switch is turned off. */
  onClipboardHistoryChanged: ((enabled: boolean) => void) | undefined = undefined;
  /** Read-only English completions for the strip: the prefix they answer and the words offered. */
  onEnglishSuggestions: ((prefix: string, items: string[]) => void) | undefined = undefined;
  /** 候选栏上的邮箱后缀，没有时是空数组；见 `EmailSuffixPolicy`。 */
  onEmailSuffixes: ((suffixes: string[]) => void) | undefined = undefined;
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
  /** The native host owns panel geometry; the touch view reports its newly requested total height. 扩展能力挂上回调之前到达的请求先留着，见 `TouchHeightRelay`。 */
  readonly touchHeight: TouchHeightRelay = new TouchHeightRelay();
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
               toolbarSkin: ToolbarSkin, image: ToolbarImageScene | null) => void) | undefined = undefined;
  /** 原生面板负责先调整物理尺寸，绘制层随后设置窗口形状。 */
  onToolbarResize: (() => Promise<void>) | undefined = undefined;
  /** The status-bar panel owns the window; the ArkUI handle only reports its pan gesture. */
  onToolbarDragStart: (() => void) | undefined = undefined;
  onToolbarDrag: ((offsetX: number, offsetY: number) => void) | undefined = undefined;
  /** 设置应用，只有宿主能打开它：启动它所需的 context 归宿主所有。2in1 工具栏的齿轮和手机上的「更多设置」卡片都从这里进入。`page` 是 `SettingsPageLink.PAGES` 之一，用于直接打开对应页面；为 `undefined` 时打开设置首页。`target` 是这一页的子目标（`SettingsPageLink.targets(page)` 之一），没有时为 `undefined`。 */
  onOpenSettings: ((page: string | undefined, requestMicrophone?: boolean, target?: string) => void) | undefined = undefined;
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
  /** 浮动键盘开关变了（`setFloatingKeyboard`）：持有面板的扩展能力换面板标志、大小和位置。 */
  onFloatingKeyboardChanged: (() => void) | undefined = undefined;
  /** 浮动键盘的拖动条：按下、移动（相对按下那一刻的屏幕位移，vp）和松手。面板归扩展能力，视图只报告手指。 */
  onFloatingDragStart: (() => void) | undefined = undefined;
  onFloatingDrag: ((offsetX: number, offsetY: number) => void) | undefined = undefined;
  onFloatingDragEnd: (() => void) | undefined = undefined;
  /** 同一编辑框收起后可直接重新显示，不一定再次触发 `inputStart`；恢复空闲任务且不重建已有计时器。 */
  keyboardShown(): void {
    if (this.handle === 0 || this.inputClient === undefined) return;
    this.idleDictionaryVisible = true;
    this.scheduleDictionarySnapshotPoll();
    this.schedulePersonalDictionaryDrain();
  }

  /** 键盘收起（系统的 keyboardHide，或 inputStop 时宿主隐藏面板）。面板收起时视图不会销毁，视图靠它停掉键盘内的语音识别。 */
  onPanelHidden: (() => void) | undefined = undefined;
  /** 接上一个编辑框（每次 inputStart，`attach`）。换输入框时系统可能不发 inputStop、键盘也不收起，视图靠它离开只属于上一个输入框的界面（键盘调整）。 */
  onEditorAttached: (() => void) | undefined = undefined;

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
      if (response.ok !== true || response.value === undefined || response.value === null
        || !Array.isArray(response.value.packages)) {
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
        directory: `${stateRoot}/skins`, id: id, relative: relative, kind: 'image', encoding: 'base64'
      };
      const response: SkinResourceResponse =
        JSON.parse(client.skinResource(JSON.stringify(request))) as SkinResourceResponse;
      if (response.ok !== true || response.value === undefined || response.value === null) {
        return NO_CANDIDATE_SKIN_IMAGE;
      }
      const url: string | null = CandidateSkinCatalogPolicy.imageDataUrl(response.value.contentType,
        response.value.bytesBase64);
      if (url === null) {
        return NO_CANDIDATE_SKIN_IMAGE;
      }
      // The ratio is read from the header, so only its first few kilobytes are decoded.
      const header: number[] = [];
      new util.Base64Helper().decodeSync(response.value.bytesBase64.substring(0,
        CandidateSkinCatalogPolicy.ASPECT_RATIO_HEADER_BASE64)).forEach((byte: number) => {
        header.push(byte);
      });
      const image: CandidateSkinImage = {
        image: url,
        aspectRatio: CandidateSkinCatalogPolicy.imageAspectRatio(response.value.contentType, header)
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
      return response.ok === true && typeof response.value === 'string' ? response.value : null;
    } catch (error) {
      KeyboardLog.error(`candidate skin toolbar unavailable: ${describeError(error)}`);
      return null;
    }
  }

  private canvasGeneration: number = 0;
  private canvasPreferences: PreparedPreferences | null = null;
  /** 上次 `applyAppearance` 读包之前取的选中包目录和 `skin.toml` 的身份，以及读完之后取的画出的装饰图、背景图的身份，见 `reloadReplacedSkinPackages`。 */
  private skinPackageIdentity: string = '';
  private skinImagePaths: string[] = [];
  private skinImageIdentity: string = '';

  /** 一次 `fs.statSync`。包目录被删掉、图片不在时返回 null，身份里记作缺失，不算错误。 */
  private static statSkinFile(path: string): SkinFileStat | null {
    try {
      const stat: fs.Stat = fs.statSync(path);
      const nanoseconds: bigint | undefined = stat.mtimeNs;
      return {
        ino: stat.ino.toString(),
        mtime: nanoseconds === undefined ? `${stat.mtime}` : nanoseconds.toString(),
        size: stat.size
      };
    } catch {
      return null;
    }
  }

  /** 浅色、深色两个槽位选中的包（`custom_theme.candidate_skin` 与 `candidate_skin_dark`）的目录和 `skin.toml` 的身份。 */
  private selectedSkinPackageIdentity(preferences: PreparedPreferences): string {
    const paths: string[] = CandidateSkinPackageIdentity.packagePaths(`${this.stateDirectory}/skins`,
      [preferences.custom_theme?.candidate_skin, preferences.custom_theme?.candidate_skin_dark]);
    return CandidateSkinPackageIdentity.identity(paths, KeyboardSession.statSkinFile);
  }

  /**
   * 选中的皮肤包在原处被同名替换后重新应用外观，返回是否重新应用了。
   *
   * 设置应用重装图库作品、皮肤编辑器覆盖保存或导入同名包时，选择和偏好修订号都不变，`reloadChangedPreferences` 不会重建会话，`applyAppearance` 解析出的配色、装饰图、背景图和贴图面板会一直是旧包的，直到键盘进程重启。这里对选中的包目录、`skin.toml` 和画出的图各做一次 stat（见 `CandidateSkinPackageIdentity`），什么都没变时不读包。
   */
  reloadReplacedSkinPackages(): boolean {
    const preferences: PreparedPreferences | undefined = this.preparedPreferences;
    if (this.handle === 0 || preferences === undefined) {
      return false;
    }
    if (this.skinPackageIdentity === this.selectedSkinPackageIdentity(preferences)
      && this.skinImageIdentity === CandidateSkinPackageIdentity.identity(this.skinImagePaths,
        KeyboardSession.statSkinFile)) {
      return false;
    }
    KeyboardLog.info('selected candidate skin package changed on disk; appearance reapplied');
    this.applyAppearance(preferences);
    this.refreshToolbar();
    return true;
  }

  private readCandidateCanvas(packages: CandidateSkinPackage[], id: string | null, dark: boolean): CandidateCanvasScene | null {
    const entry: CandidateSkinPackage | null = id === null ? null : CandidateSkinCatalogPolicy.package(packages, id);
    if (entry === null || entry.canvas !== true || id === null) return null;
    const paths: string[] = [];
    if (entry.borderImage !== undefined && entry.borderImage !== null) {
      paths.push(!dark && entry.borderImage.lightSource ? entry.borderImage.lightSource : entry.borderImage.source);
    }
    for (const decoration of entry.decorations ?? []) {
      if (!paths.includes(decoration.image)) paths.push(decoration.image);
    }
    const resources: CandidateCanvasResource[] = [];
    let remainingBytes: number = 16 * 1024 * 1024;
    for (const relative of paths) {
      try {
        const request: SkinResourceRequest = {
          directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'image', encoding: 'base64'
        };
        const reply: SkinResourceResponse = JSON.parse(client.skinResource(JSON.stringify(request))) as SkinResourceResponse;
        const length: number = reply.ok === true && reply.value !== undefined && reply.value !== null
          ? CandidateSkinCatalogPolicy.base64ByteLength(reply.value.bytesBase64) : -1;
        if (length >= 0 && length <= remainingBytes) {
          remainingBytes -= length;
          resources.push({ relative: relative, contentType: reply.value.contentType, base64: reply.value.bytesBase64 });
        }
      } catch (_) {
        // 单张资源失败由绘制层回退，不影响候选输入。
      }
    }
    return {
      key: `${++this.canvasGeneration}:${id}:${dark}`, dark: dark, canvas: true,
      borderImage: entry.borderImage, decorations: entry.decorations, resources: resources,
      contentInsetsDip: entry.contentInsetsDip, outerInsetsDip: entry.outerInsetsDip, lineGapDip: entry.lineGapDip,
    };
  }

  /** 外观重载时一次读取全部状态图，切换模式只消费缓存。 */
  private readToolbarImage(packages: CandidateSkinPackage[], id: string | null): ToolbarImageResources | null {
    const entry: CandidateSkinPackage | null = id === null ? null : CandidateSkinCatalogPolicy.package(packages, id);
    if (!KeyboardFormFactor.isDesktop() || entry === null || entry.toolbarImage === undefined
      || entry.toolbarImage === null || id === null) return null;
    const resources: CandidateCanvasResource[] = [];
    let remaining: number = 16 * 1024 * 1024;
    for (const relative of ToolbarImagePolicy.paths(entry.toolbarImage)) {
      try {
        const request: SkinResourceRequest = {
          directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'image', encoding: 'base64'
        };
        const reply: SkinResourceResponse = JSON.parse(client.skinResource(JSON.stringify(request))) as SkinResourceResponse;
        const length: number = reply.ok && reply.value !== undefined && reply.value !== null
          ? CandidateSkinCatalogPolicy.base64ByteLength(reply.value.bytesBase64) : -1;
        if (length >= 0 && length <= remaining) {
          remaining -= length;
          resources.push({ relative: relative, contentType: reply.value.contentType, base64: reply.value.bytesBase64 });
        }
      } catch (_) {
        // 坏图留给原生文字与背景矩形回退，不影响输入会话。
      }
    }
    return { key: `toolbar:${++this.canvasGeneration}:${id}`, spec: entry.toolbarImage, resources: resources };
  }

  /** 仅在视图确实绘制当前快照的面板后使用贴图配色，取色器仍由共享 Rust 叠加。 */
  canvasCandidateColors(key: string): CandidateColors | null {
    if (this.appearance.candidateCanvas?.key !== key || this.canvasPreferences === null) return null;
    const resolved: ResolvedTheme | null = this.resolveTheme(this.canvasPreferences, this.appearance.candidateDark, true);
    return resolved === null ? null : GlobalTheme.candidateColors(resolved.candidate, this.appearance.candidateDark);
  }

  /** 外观重载时统一解析明暗主题；各界面保留自己的模式规则，逐键绘制不读取皮肤目录。 */
  private applyAppearance(preferences: PreparedPreferences): void {
    this.canvasPreferences = preferences;
    // 在读包之前取：读的过程中包又被换掉时，下一次检查看到的身份和这里不同，会再应用一次，而不是把新身份记在旧内容上。
    this.skinPackageIdentity = this.selectedSkinPackageIdentity(preferences);
    if (this.themeCatalog.length === 0) {
      this.themeCatalog = this.readThemeCatalog();
    }
    this.globalThemeId = preferences.global_theme;
    this.customThemeBase = preferences.custom_theme?.base ?? 'system';
    const design: CustomSkinDocument | undefined = preferences.custom_theme?.keyboard;
    this.customDesign = design === undefined ? null : CustomKeyboardSkin.from(design);
    this.themeLight = this.resolveTheme(preferences, false);
    this.themeDark = this.resolveTheme(preferences, true);
    // 键盘、工具栏与各个面板一律跟随颜色模式（`HostCapabilities::surface_themes_follow`），存下的各界面单独明暗不再读；只有候选窗口仍按 `candidate_theme` 单独设明暗。
    const keyboardDark: boolean = this.themeDarkFor('follow', preferences.theme);
    const candidateDark: boolean = this.themeDarkFor(preferences.candidate_theme, preferences.theme);
    const candidateTheme: ResolvedTheme | null = candidateDark ? this.themeDark : this.themeLight;
    const candidateColors: CandidateColors = GlobalTheme.candidateColors(
      candidateTheme === null ? null : candidateTheme.candidate, candidateDark);
    const toolbarDark: boolean = this.themeDarkFor('follow', preferences.theme);
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
    const candidateAnimation: CandidateSkinAnimation = packageId === null
      ? { enter: 'none', exit: 'none', selection: 'none', durationMs: 0, staggerMs: 0 }
      : CandidateSkinCatalogPolicy.animation(skinPackages, packageId);
    // Like the decoration, the background is a desktop candidate window's; the touch strip keeps the keyboard's look.
    const backgroundImage: CandidateSkinImage = KeyboardFormFactor.isDesktop() && packageId !== null
      ? this.readCandidateSkinImage(this.stateDirectory, packageId,
        skinBackground === null ? null : skinBackground.relative)
      : NO_CANDIDATE_SKIN_IMAGE;
    // 读完之后取刚读的两张图的身份：包目录 inode 不变、只原地改写了图片时靠它们发现。触屏键盘不画这两张图，不取。
    this.skinImagePaths = KeyboardFormFactor.isDesktop()
      ? CandidateSkinPackageIdentity.imagePaths(`${this.stateDirectory}/skins`, packageId,
        [skinDecoration === null ? null : skinDecoration.relative,
          skinBackground === null ? null : skinBackground.relative])
      : [];
    this.skinImageIdentity = CandidateSkinPackageIdentity.identity(this.skinImagePaths, KeyboardSession.statSkinFile);
    // The store validates both to the shared bounds; an out-of-range value from an older or foreign document is ignored rather than drawn.
    const userRadius: number | null | undefined = preferences.candidate_corner_radius;
    const userRadiusValid: boolean =
      typeof userRadius === 'number' && Number.isFinite(userRadius) && userRadius >= 0 && userRadius <= 32;
    const candidateRadius: number | null = userRadiusValid
      ? userRadius as number
      : candidateTheme?.candidate_layout?.cornerRadius ?? (packageId === null ? null : CandidateSkinCatalogPolicy.cornerRadiusVp(skinPackages, packageId));
    const opacityPercent: number | undefined = preferences.candidate_opacity_percent;
    const windowOpacity: number = typeof opacityPercent === 'number' && Number.isFinite(opacityPercent)
      ? Math.max(50, Math.min(100, opacityPercent)) / 100 : 1;
    // 「候选栏大小」只作用于触屏工具条；2in1 的设置桥不声明它，候选卡片照旧只随字号调整。
    const touchScale: number = KeyboardFormFactor.isDesktop()
      ? CandidateScalePolicy.DEFAULT_PERCENT : CandidateScalePolicy.percent(preferences.candidate_scale_percent);
    this.appearance = {
      dark: keyboardDark,
      globalThemeId: preferences.global_theme,
      candidateMinWidthVp: skinMinWidth === null
        ? CandidateWidthPolicy.MIN_WIDTH_VP
        : Math.max(CandidateWidthPolicy.MIN_WIDTH_VP,
          Math.min(CandidateWidthPolicy.MAX_WIDTH_VP, skinMinWidth)),
      candidateDecorationImage: skinImage.image,
      candidateDecorationCandidate: skinImage.image === null || packageId === null ? null
        : CandidateSkinCatalogPolicy.decorationCandidate(skinPackages, packageId),
      candidateDecorationTopVp: skinImage.image === null ? 0 : (skinDecoration?.topVp ?? 0),
      candidateDecorationWidthVp: skinImage.image === null ? 0 : (skinDecoration?.widthVp ?? 0),
      candidateDecorationAspectRatio: skinImage.aspectRatio,
      candidateDecorationAlign: skinDecoration === null ? 'right' : skinDecoration.align,
      candidateCornerRadiusVp: candidateRadius,
      candidateCornerRadiusUser: userRadiusValid,
      candidateBackgroundImage: backgroundImage.image,
      candidateCanvas: this.readCandidateCanvas(skinPackages, packageId, candidateDark),
      candidateSkinLayout: candidateTheme?.candidate_layout ?? null,
      candidateBackgroundFit: skinBackground === null ? 'cover' : skinBackground.fit,
      candidateBackgroundOpacity: skinBackground === null ? 1 : skinBackground.opacity,
      candidateAnimation: candidateAnimation,
      candidateWindowOpacity: windowOpacity,
      candidateDark: candidateDark,
      candidateSelectedBar: preferences.candidate_selected_bar ?? candidateColors.showSelectedBar,
      pairedPunctuation: preferences.paired_punctuation !== false,
      // 各界面跟随颜色模式，见上面的 `keyboardDark`。
      keyboardTheme: 'follow',
      toolbarTheme: 'follow',
      voiceTheme: 'follow',
      emojiTheme: 'follow',
      handwritingTheme: 'follow',
      menuTheme: 'follow',
      appMode: preferences.theme,
      scheme: preferences.scheme,
      shuangpinProfile: preferences.shuangpin_profile,
      shuangpinLayout: preferences.touch_shuangpin_layout ?? 'twenty_six_key',
      wubiLayout: preferences.touch_wubi_layout ?? 'twenty_six_key',
      wubiProfile: KeyboardScheme.normalizedWubiProfile(preferences.wubi_profile),
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
        // Tab 和 PageUp/PageDown 两组翻页键固定开启，不读存着的值：组字时关掉的绑定被 HardwareKeyRouter 判为 IGNORED 吃掉，并不会还给应用，关掉只让键失效。韩文汉字 / 注音列表原本会把关掉的键交还应用，固定开启后在列表里也翻页。设置页也不再给这两组的开关（HostCapabilities.tab_page_paging_switches）。
        tab: true,
        pageUpDown: true,
        mouseWheel: preferences.navigation?.mouse_wheel ?? false,
        arrows: preferences.navigation?.arrows ?? true
      },
      // The shared store validates these to 12..32 and strips control
      // characters from family names. Clamp again at the host boundary so
      // an older staged document cannot produce an unusable panel.
      candidateFontSize: CandidateScalePolicy.scaled(
        CandidateFontSizePolicy.bounded(preferences.candidate_font_size), touchScale),
      candidatePreeditFontSize: CandidateScalePolicy.scaled(
        CandidateFontSizePolicy.bounded(preferences.candidate_preedit_font_size), touchScale),
      candidateScalePercent: touchScale,
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
      shuangpinKeyHints: preferences.touch_shuangpin_key_hints === true,
      // 新文档默认保留 Shift；明确写 false 才切回分词，兼容缺字段的旧文档。
      quanpinShift: preferences.touch_quanpin_shift !== false,
      toolbarTools: TouchToolbarPolicy.fromDocument(preferences.harmony_toolbar),
      twentySixKeyStyle: TouchLayoutStylePolicy.twentySixKeyStyle(preferences.touch_twenty_six_key_style),
      fourteenKeyStyle: TouchLayoutStylePolicy.fourteenKeyStyle(preferences.touch_fourteen_key_style),
      nineKeyStyle: TouchLayoutStylePolicy.nineKeyStyle(preferences.touch_nine_key_style),
      numberRow: preferences.touch_number_row === true,
      twentySixSymbolFirst: preferences.touch_twenty_six_symbol_first === true,
      numberPadSpace: TouchLayoutStylePolicy.numberPadSpace(preferences.touch_number_pad_space),
      nineKeySidebarSymbols: TouchLayoutStylePolicy.sidebarSymbols(
        preferences.touch_nine_key_sidebar_symbols,
        TouchLayoutStylePolicy.NINE_KEY_SIDEBAR_DEFAULTS
      ),
      nineKeyDigitSidebarSymbols: TouchLayoutStylePolicy.sidebarSymbols(
        preferences.touch_nine_key_digit_sidebar_symbols,
        TouchLayoutStylePolicy.DIGIT_SIDEBAR_DEFAULTS
      ),
      oneHandedSidePanel: OneHandedPolicy.sidePanel(preferences.touch_one_handed_side_panel),
      splitKeyboard: SplitKeyboardPolicy.settings(preferences.touch_keyboard_split,
        preferences.touch_keyboard_split_portrait, preferences.touch_keyboard_split_threshold,
        preferences.touch_keyboard_split_gap),
      languageKeyLeft: TouchLayoutStylePolicy.languageKeyLeft(preferences.touch_language_key_side),
      nineKeySwipeDigits: NineKeySwipePolicy.mode(preferences.touch_nine_key_swipe_digits)
    };
    // The theme's bar, then the package's `[toolbar]` colours for that mode, then its stylesheet.
    const toolbarColors: CandidateColors = GlobalTheme.candidateColors(
      toolbarTheme === null ? null : toolbarTheme.candidate, toolbarDark);
    this.toolbarImage = this.readToolbarImage(skinPackages, toolbarPackageId);
    this.toolbarImageRejected = false;
    if (this.toolbarImage !== null) {
      this.skinImagePaths.push(...CandidateSkinPackageIdentity.imagePaths(`${this.stateDirectory}/skins`,
        toolbarPackageId, ToolbarImagePolicy.paths(this.toolbarImage.spec)));
      this.skinImageIdentity = CandidateSkinPackageIdentity.identity(this.skinImagePaths, KeyboardSession.statSkinFile);
    }
    this.toolbarSkin = ToolbarSkinPolicy.fromCss(toolbarColors, skinToolbar,
      ToolbarSkinPolicy.withPackage(toolbarColors,
        CandidateSkinCatalogPolicy.toolbar(skinPackages, toolbarPackageId), toolbarDark));
  }

  /** A surface's mode: fixed by a theme with an appearance, otherwise its own setting, then the app mode, then the system. */
  private themeDarkFor(surfaceTheme: string, appMode: string): boolean {
    return GlobalTheme.surfaceDark(this.fixedAppearance(),
      KeyboardSkin.resolveDark(surfaceTheme, appMode, this.systemDark));
  }

  /** 全局主题固定了明暗时为 `light` 或 `dark`：两种明暗的解析结果一致才算，见 `GlobalTheme.fixedAppearance`。 */
  private fixedAppearance(): string | null {
    return GlobalTheme.fixedAppearance(this.themeLight, this.themeDark);
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
  private resolveTheme(preferences: PreparedPreferences, dark: boolean, panelDrawn: boolean = false): ResolvedTheme | null {
    const request: ResolveThemeRequest = {
      // 固定明暗的主题在另一种明暗下按跟随系统解析，见 `GlobalTheme.fitsMode`。两种明暗于是不再一致，`fixedAppearance` 也就不再把各界面钉在那种明暗上。
      global_theme: GlobalTheme.effectiveId(this.themeCatalog, preferences.global_theme, dark),
      // 编号色和边框色跟随主题，存下的这两个槽位不交给解析器，见 `GlobalTheme.hostDrawnCustomTheme`。
      custom_theme: GlobalTheme.hostDrawnCustomTheme(preferences.custom_theme),
      dark: dark,
      layout: preferences.candidate_layout === 'horizontal' ? 'horizontal' : 'vertical',
      skins_directory: `${this.stateDirectory}/skins`,
      canvas_panel_drawn: panelDrawn
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
    // AI 生成类功能的总闸取自 Rust 的宿主能力（`HostCapabilities.ai_generation`），鸿蒙版拿到资质之前为关。
    AiGenerationGate.adopt(client.hostCapabilities('harmony'));
    this.refreshFeedback();
    this.refreshAppTheme(true);
    this.preferencesRevision = this.storedRevision(stateRoot);
    const request: PrepareRequest = KeyboardSession.prepareRequest(resources, stateRoot);
    let options: string;
    try {
      const prepared: PrepareResponse =
        JSON.parse(client.prepareHost(JSON.stringify(request))) as PrepareResponse;
      if (prepared.ok !== true) {
        KeyboardLog.error(`host preparation refused: ${prepared.error}`);
        return false;
      }
      const requested: PreparedOptions = prepared.value;
      requested.phrase_preedit = true;
      // 隐私模式下以关闭学习的方式构建会话，与 Android 的 `restartSessionForPrivacy` 相同：只改会话的副本，存储的文档保留用户自己的开关。
      if (this.feedbackCurrent.incognito) {
        requested.preferences.learning = false;
      }
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
      this.candidatePronunciationEnabled = preferences.candidate_pronunciation === true;
      this.applyCandidateGloss(preferences);
      this.fuzzyPinyin = preferences.fuzzy_pinyin !== undefined && preferences.fuzzy_pinyin.enabled;
      this.modeBadge = InputModeHudPolicy.enabled(preferences.input_mode_hud);
      this.englishSuggestionsEnabled = preferences.english_suggestions !== false;
      this.predictionButtonStyle = NextWordPredictionPolicy.buttons(preferences.next_word_prediction_buttons);
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
      this.keyGrid = KeyboardScheme.keyGrid(preferences.touch_keyboard_layout, preferences.scheme,
        KeyboardFormFactor.isDesktop(), preferences.touch_shuangpin_layout ?? 'twenty_six_key',
        preferences.touch_wubi_layout ?? 'twenty_six_key');
      this.soundPlugins = preferences.plugins;
      this.configureSounds();
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
    const drained: PersonalDictionaryDrain = this.drainPersonalDictionary(options);
    // 同一个边界上交换「我的词条」；队列和镜像都完成后才停止空闲重试。
    const mirrored: boolean = this.syncUserWords(options);
    this.personalDictionarySettled = drained === 'settled' && mirrored;
    this.personalDictionaryStalls = drained === 'stalled' ? this.personalDictionaryStalls + 1 : 0;
    try {
      const created: CreateResponse = JSON.parse(client.create(options)) as CreateResponse;
      if (created.ok !== true) {
        KeyboardLog.error(`session refused: ${created.error}`);
        return false;
      }
      this.handle = created.value.session;
      this.keyGeometry.reset();
      KeyboardLog.info(`session ${this.handle} created`);
      this.markPrivateSession(this.feedbackCurrent.incognito);
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
  private drainPersonalDictionary(options: string): PersonalDictionaryDrain {
    try {
      const reply: PersonalDictionarySyncResponse =
        JSON.parse(client.personalDictionarySync(options)) as PersonalDictionarySyncResponse;
      if (reply.ok !== true) {
        KeyboardLog.error(`personal dictionary queue not applied; it will be retried: ${reply.error}`);
        return 'stalled';
      }
      // 命名词库的词分批交给个人词库队列（同时最多 128 条在途）。每处理完一批就送下一批，和 Android 键盘的 `flushSent` 一样，导入的大词库不用等用户回设置页刷新。
      const sent: number = this.flushDictionaryCollections(options);
      const pending: number = reply.value.pending_count;
      const drained: PersonalDictionaryDrain =
        IdleDictionaryRetryPolicy.drain(pending, sent, this.personalDictionaryPending);
      this.personalDictionaryPending = drained === 'settled' ? -1 : pending;
      return drained;
    } catch (error) {
      KeyboardLog.error(`personal dictionary queue unavailable: ${describeError(error)}`);
      return 'stalled';
    }
  }

  /**
   * 建会话之前与设置应用交换「我的词条」（`msime_client_user_words_mirror` 的 sync）。设置应用的发布从上次做完以后没变、上次也做完了时不做。完整体验模式下顺带发布键盘的；基础访问模式下键盘写不了共享目录，只收不发。返回这一次是否做完，没做完（忙、到了这次的上限）时由空闲时的会话重建接着做。
   */
  private syncUserWords(options: string): boolean {
    const mirror: SharedStateMirror | null = this.sharedMirror;
    if (mirror === null || this.stateDirectory.length === 0) return true;
    const theirs: string = mirror.userWordsFile('settings');
    const mark: string = UserWordsMirror.fileMark(theirs);
    if (!UserWordsMirrorPolicy.keyboardSyncDue(this.userWordsDue, this.userWordsMergedMark, mark)) return true;
    const publish: string | null = this.sharedFullMode() ? mirror.userWordsFile('keyboard') : null;
    const reply: UserWordsReply = UserWordsMirror.run(UserWordsMirrorPolicy.request('sync', 'keyboard', options,
      this.stateDirectory, theirs, publish, KEYBOARD_MAX_CHANGES));
    const outcome: KeyboardSyncOutcome = UserWordsMirrorPolicy.keyboardOutcome(reply, this.userWordsFailures);
    const settled: boolean = outcome.settled;
    this.userWordsFailures = outcome.failures;
    this.userWordsDue = !settled;
    if (settled) this.userWordsMergedMark = mark;
    if (reply.published) {
      this.userWordsPublishedJournal = UserWordsMirror.journalMark(this.stateDirectory);
      this.userWordsPublishMs = Date.now();
    }
    return settled;
  }

  /**
   * 失去焦点时把这次编辑里学到、删掉的词发布给设置应用（publish：只记录，不写词库，会话开着也能做），在工作线程上。只在完整体验模式下、日志变了、距上次至少 30 秒时做。
   */
  private publishUserWords(): void {
    const mirror: SharedStateMirror | null = this.sharedMirror;
    if (mirror === null || this.stateDirectory.length === 0 || this.snapshotOptions === undefined) return;
    const journal: string = UserWordsMirror.journalMark(this.stateDirectory);
    const now: number = Date.now();
    if (!UserWordsMirrorPolicy.keyboardPublishDue(this.sharedFullMode(), this.userWordsPublishing,
      this.userWordsPublishedJournal, journal, this.userWordsPublishMs, now)) {
      return;
    }
    this.userWordsPublishing = true;
    this.userWordsPublishMs = now;
    const request: string = UserWordsMirrorPolicy.request('publish', 'keyboard', JSON.stringify(this.snapshotOptions),
      this.stateDirectory, null, mirror.userWordsFile('keyboard'), KEYBOARD_MAX_CHANGES);
    UserWordsMirror.runAsync(request).then((reply: UserWordsReply): void => {
      this.userWordsPublishing = false;
      if (reply.ok) this.userWordsPublishedJournal = journal;
    });
  }

  /** 把命名词库里还没送出的增删再送一批，返回这次送出的条数；失败按 0 算，下次排空时再试。 */
  private flushDictionaryCollections(options: string): number {
    try {
      const reply: DictionaryCollectionsFlushResponse = JSON.parse(client.dictionaryCollections(
        `{"options":${options},"action":{"operation":"flush"}}`)) as DictionaryCollectionsFlushResponse;
      if (reply.ok !== true) {
        KeyboardLog.error(`dictionary collections not flushed: ${reply.error}`);
        return 0;
      }
      return typeof reply.value.sent === 'number' ? reply.value.sent : 0;
    } catch (error) {
      KeyboardLog.error(`dictionary collections unavailable: ${describeError(error)}`);
      return 0;
    }
  }

  /**
   * 编辑器空闲时按有界批次继续导入。
   *
   * 每次同步最多处理 128 条，批次之间交还事件循环，避免大词库导入占满输入法。组字和本地模式期间不重建会话，避免干扰输入。
   *
   * 有进展时两秒后接着排下一批；整批被拒或队列原地不动时按 `IdleDictionaryRetryPolicy` 退避到最多五分钟一次。隐藏后才停滞的队列也在这里停排，下一次显示再恢复。
   */
  private schedulePersonalDictionaryDrain(): void {
    if (this.personalDictionarySettled || this.personalDictionaryTimer >= 0 || this.handle === 0
        || (!this.idleDictionaryVisible && this.personalDictionaryStalls > 0)) {
      return;
    }
    this.personalDictionaryTimer = setTimeout((): void => {
      this.personalDictionaryTimer = -1;
      if (this.composing() || this.localMode !== 'none') {
        this.schedulePersonalDictionaryDrain();
        return;
      }
      this.restartIdleSession('personal dictionary queue');
    }, IdleDictionaryRetryPolicy.delay(this.personalDictionaryStalls));
  }

  /**
   * Check the cross-process snapshot queue once after the keyboard is shown, and keep checking only while a request is waiting.
   *
   * 原来从会话建好起每两秒查一次，键盘收起后也不停；每次查询都要在主线程上扫一遍整个用户词库算哈希，再把 state.json 重写并 fsync。请求不是键盘自己排的，而键盘每次构建会话（`start`）都会先处理一次队列，所以这里只在键盘显示时（`attach`）查一次：没有待处理的请求就停下，等下一次显示；有请求就重建会话去处理，重建后仍没处理掉（例如属于账号的请求而键盘读不到会话）就按 `IdleDictionaryRetryPolicy` 退避，不再每两秒重建一次。键盘收起时（`detach`）取消。
   */
  private scheduleDictionarySnapshotPoll(): void {
    if (this.snapshotQueueTimer >= 0 || this.handle === 0 || !this.idleDictionaryVisible
        || this.inputClient === undefined) return;
    this.snapshotQueueTimer = setTimeout((): void => {
      this.snapshotQueueTimer = -1;
      if (this.composing() || this.localMode !== 'none') {
        this.scheduleDictionarySnapshotPoll();
        return;
      }
      if (this.dictionarySnapshotNeedsWork()) {
        this.snapshotQueueStalls++;
        // 重建会话的 start() 会处理队列并按退避后的间隔再排一次检查。
        this.restartIdleSession('snapshot queue');
        return;
      }
      this.snapshotQueueStalls = 0;
    }, IdleDictionaryRetryPolicy.delay(this.snapshotQueueStalls));
  }

  private cancelDictionarySnapshotPoll(): void {
    if (this.snapshotQueueTimer >= 0) {
      clearTimeout(this.snapshotQueueTimer);
      this.snapshotQueueTimer = -1;
    }
  }

  /**
   * 键盘收起或编辑框失去焦点：不再查快照队列；个人词库排空上一次没有进展时也停下重试，下一次显示（`attach`）再接着排。
   *
   * 卡住的队列（整批被拒、剩余条数原地不动）按退避最多五分钟重建一次会话，隐藏的键盘不该为它在整个进程生命周期里一直醒来。有进展的排空照常在后台跑完，大词库导入不用等下一次显示。
   */
  private pauseIdleDictionaryWork(): void {
    this.idleDictionaryVisible = false;
    this.cancelDictionarySnapshotPoll();
    if (this.personalDictionaryStalls > 0 && this.personalDictionaryTimer >= 0) {
      clearTimeout(this.personalDictionaryTimer);
      this.personalDictionaryTimer = -1;
    }
  }

  /**
   * Querying queue state takes no Engine maintenance lock and never acknowledges terminal state.
   *
   * 带 `skip_when_idle`：队列空着时（几乎总是）Rust 只读一下状态就返回，不在 UI 线程上扫整个用户词库算哈希、不重写并 fsync state.json。鸿蒙没有任何东西往键盘的这个队列里排请求，发布出去的本机版本没人读；真有请求时，重建会话的 `processDictionarySnapshot` 会在认领前先发布。
   */
  private dictionarySnapshotNeedsWork(): boolean {
    if (this.snapshotOptions === undefined || this.stateDirectory.length === 0) return false;
    try {
      const action: SnapshotQueueStateAction = {
        operation: 'state', directory: `${this.stateDirectory}/dictionary-snapshots`,
        options: this.snapshotOptions, acknowledge: false, skip_when_idle: true
      };
      const reply: SnapshotQueueStateReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueStateReply;
      if (reply.ok !== true) {
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
    const keyGrid: number = this.keyGrid;
    const revision: number = this.preferencesRevision;
    const seedLight: AppThemeSeed | null = this.appThemeSeedLight;
    const seedDark: AppThemeSeed | null = this.appThemeSeedDark;
    const fullWidth: boolean = this.fullWidth;
    this.stop();
    if (!this.start(this.resourceDirectory, this.stateDirectory, this.systemDark)) {
      KeyboardLog.error(`${reason} left the session unavailable`);
      return;
    }
    this.englishMode = english;
    this.keyGrid = keyGrid;
    if (attached !== undefined && this.inputClient === attached) {
      this.apply(client.focus(this.handle, true));
      this.refreshTypingEffects();
      this.applyView(client.setEnglishMode(this.handle, english));
      this.applyView(client.setKeyGrid(this.handle, keyGrid));
    }
    // Any rebuild reads the document afresh, whatever it was for, so the surfaces follow whenever the stored preferences moved underneath it.
    if (this.preferencesRevision !== revision) {
      this.announcePreferencesReloaded();
    } else if ((!KeyboardSession.sameSeed(seedLight, this.appThemeSeedLight)
        || !KeyboardSession.sameSeed(seedDark, this.appThemeSeedDark) || this.fullWidth !== fullWidth)
        && this.onViewPreferencesReloaded !== undefined) {
      // 偏好文档没变，但重建改变了视图所画的内容：start() 已读入新的应用主题种子，此后 attach 的比较不会再看到变化；全角/半角是会话状态，新会话回到初始值（Android 的 restartSessionForPrivacy 新建的引擎会话也是如此），视图的全角磁贴若不跟着刷新，就会仍显示开启，再点一次反而打开全角。只让视图重绘，ability 不必重读偏好。
      this.onViewPreferencesReloaded();
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
    const revision: number = this.storedRevisionIfMoved(this.stateDirectory);
    if (revision < 0 || revision === this.preferencesRevision) {
      return;
    }
    KeyboardLog.info(`preferences moved from revision ${this.preferencesRevision} to ${revision}`);
    this.restartIdleSession('preferences reload');
  }

  /**
   * 与 `storedRevision` 相同，但 `preferences.json` 的文件身份（inode、大小、纳秒修改时间）与上次读到时相同，就直接用上次的修订号，不经 NAPI 读出、解析整份文档。每次获得焦点都要比一次，而设置大多没改过。修订号就写在这份文档里，存储每次保存都先写暂存文件再改名换入，inode 必变。拿不到纳秒修改时间、读不到修订号时不记，下次照旧读。
   */
  private storedRevisionIfMoved(stateRoot: string): number {
    const path: string = `${stateRoot}/preferences.json`;
    let identity: string | null = null;
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      const nanoseconds: bigint | undefined = stat.mtimeNs;
      const stamp: string | null = stat.isFile() ? FingerprintCache.stamp(stat.ino.toString(), stat.size,
        nanoseconds === undefined ? undefined : nanoseconds.toString()) : null;
      identity = stamp === null ? null : `${path}#${stamp}`;
    } catch {
      // 文档不在时由 `storedRevision` 照旧回答。
      identity = null;
    }
    if (identity !== null && identity === this.checkedPreferencesIdentity) {
      return this.checkedPreferencesRevision;
    }
    const revision: number = this.storedRevision(stateRoot);
    this.checkedPreferencesIdentity = revision < 0 ? null : identity;
    this.checkedPreferencesRevision = revision;
    return revision;
  }

  /** The revision of the stored preference document, or -1 when it cannot be read. */
  private storedRevision(stateRoot: string): number {
    try {
      const loaded: SnapshotResponse = JSON.parse(client.loadPreferences(stateRoot)) as SnapshotResponse;
      const snapshot: PreferencesSnapshot | null = NativeReplyPolicy.successfulValue(loaded);
      return snapshot === null ? -1 : snapshot.revision;
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

  /** 当前九键双拼方案的键面。设置或偏好改变后请求串改变才重新解析，避免每次重绘都跨 NAPI。 */
  nineKeyShuangpinFaces(): readonly NineKeyShuangpinFace[] {
    const preferences: PreparedPreferences | undefined = this.preparedPreferences;
    const profile: string = normalizedNineKeyShuangpinProfile(
      preferences?.nine_key_shuangpin_profile);
    const table: Object | undefined = profile === 'custom'
      ? preferences?.nine_key_shuangpin_custom_profile : undefined;
    const request: string = JSON.stringify(table === undefined ? { profile } : { profile, table });
    if (request === this.nineKeyShuangpinRequest) return this.nineKeyShuangpinFaceCache;
    this.nineKeyShuangpinRequest = request;
    this.nineKeyShuangpinFaceCache = [];
    try {
      const reply: NineKeyShuangpinLayoutReply = JSON.parse(
        client.nineKeyShuangpinLayout(request)) as NineKeyShuangpinLayoutReply;
      if (reply.ok !== true || reply.value === undefined) {
        return this.nineKeyShuangpinFaceCache;
      }
      const keys: NineKeyShuangpinRawFace[] = reply.value.keys ?? [];
      this.nineKeyShuangpinFaceCache = keys.filter((key: NineKeyShuangpinRawFace): boolean =>
        typeof key.key === 'string' && Array.isArray(key.initials) &&
        Array.isArray(key.finals)).map((key: NineKeyShuangpinRawFace): NineKeyShuangpinFace => ({
          key: key.key as string,
          initials: key.initials as string[],
          finals: key.finals as string[],
          zeroInitial: key.zero_initial === true
        }));
    } catch (error) {
      KeyboardLog.error(`nine-key shuangpin layout unavailable: ${describeError(error)}`);
    }
    return this.nineKeyShuangpinFaceCache;
  }

  /**
   * 全局主题在某一模式下的键盘配色：自定义主题带有自定义设计时用该设计，否则用解析出的配色，再否则用季节配色，最后用原生令牌。
   *
   * 该模式下应用主题的季节种子在每种情况下都会带上：它是 `system` 键盘的配色，也是其他键盘回车键和已开启功能块的配色（`KeyboardSkin.fromTheme`）；logo 取当前皮肤自己的强调色和底色。键盘开着超过一分钟时会在这里拿到更替后的季节，因为视图每次重绘皮肤时都会来取。
   */
  keyboardSkin(dark: boolean): KeyboardSkin {
    this.refreshAppTheme(false);
    const seed: AppThemeSeed | null = this.appThemeSeed(dark);
    // 面板按这个 id 标出选中的卡片：所选主题属于另一种明暗时，生效的是「跟随系统」。
    const id: string = GlobalTheme.effectiveId(this.themeCatalog, this.globalThemeId, dark);
    const title: string = this.themeTitle(id);
    if (id === 'custom' && this.customDesign !== null) {
      return KeyboardSkin.fromDesign(title, this.customDesign, dark, seed);
    }
    const theme: ResolvedTheme | null = dark ? this.themeDark : this.themeLight;
    return KeyboardSkin.fromTheme(id, title, theme === null ? null : theme.keyboard, dark, seed);
  }

  /** 某一模式解析出的季节种子；应用主题无法解析时为 `null`。 */
  private appThemeSeed(dark: boolean): AppThemeSeed | null {
    return dark ? this.appThemeSeedDark : this.appThemeSeedLight;
  }

  /**
   * 重新解析应用主题：取设置应用存放在 `key-feedback.json` 旁边的 id 和本地日历月份，两种模式一次全部解析，这样任一模式下的界面绘制都无需再调用一次，与全局主题的解析方式相同。
   *
   * `force` 用于启动和每次编辑框获得焦点；否则最多每分钟运行一次。返回任一模式的种子是否变化，以便调用方判断已绘制的皮肤是否过期。
   */
  private refreshAppTheme(force: boolean): boolean {
    const now: number = Date.now();
    if (!force && this.appThemeCheckedAt !== 0 && now - this.appThemeCheckedAt < APP_THEME_CHECK_INTERVAL_MS) {
      return false;
    }
    this.appThemeCheckedAt = now;
    if (this.stateDirectory.length === 0) {
      return false;
    }
    const id: string = this.readAppThemeId();
    const month: number = new Date().getMonth() + 1;
    const light: AppThemeSeed | null = KeyboardSession.resolveAppTheme(id, month, false);
    const dark: AppThemeSeed | null = KeyboardSession.resolveAppTheme(id, month, true);
    const changed: boolean = !KeyboardSession.sameSeed(light, this.appThemeSeedLight)
      || !KeyboardSession.sameSeed(dark, this.appThemeSeedDark);
    this.appThemeSeedLight = light;
    this.appThemeSeedDark = dark;
    return changed;
  }

  /** 存储的应用主题 id。文件缺失或不可读时取默认值，与 `AppThemeStore.parse` 对待任何错误文档的方式一致。 */
  private readAppThemeId(): string {
    const path: string = `${this.stateDirectory}/${AppThemeStore.FILE_NAME}`;
    try {
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)) {
        return AppThemeStore.DEFAULT_ID;
      }
      const text: string | null = KeyboardSession.readPrivateText(path, AppThemeStore.MAX_BYTES);
      return text === null ? AppThemeStore.DEFAULT_ID : AppThemeStore.parse(text);
    } catch (error) {
      KeyboardLog.error(`app theme unreadable: ${describeError(error)}`);
      return AppThemeStore.DEFAULT_ID;
    }
  }

  /** 应用主题的某一模式。调用被拒绝，或原生模块不提供该调用时，键盘停留在基础令牌上，而不是让会话失败。 */
  private static resolveAppTheme(id: string, month: number, dark: boolean): AppThemeSeed | null {
    const request: ResolveAppThemeRequest = { app_theme: id, month: month, dark: dark };
    try {
      const seed: AppThemeSeed | null =
        AppThemePalette.parseResolved(client.resolveAppTheme(JSON.stringify(request)));
      if (seed === null) {
        KeyboardLog.error(`app theme ${id} refused; drawing the base tokens`);
      }
      return seed;
    } catch (error) {
      KeyboardLog.error(`app theme unavailable: ${describeError(error)}`);
      return null;
    }
  }

  private static sameSeed(first: AppThemeSeed | null, second: AppThemeSeed | null): boolean {
    if (first === null || second === null) {
      return first === second;
    }
    return first.accent === second.accent && first.accent_soft === second.accent_soft
      && first.on_accent === second.on_accent && first.background === second.background
      && first.card === second.card && first.candidate_accent === second.candidate_accent;
  }

  /** The skin a keyboard-side surface (voice, emoji, handwriting, menus) wears: its own setting, then the app mode, then whatever the keyboard is in; a theme with a fixed appearance overrides all three. */
  surfaceSkin(surfaceTheme: string): KeyboardSkin {
    return this.keyboardSkin(GlobalTheme.surfaceDark(this.fixedAppearance(),
      KeyboardSkin.resolveDark(surfaceTheme, this.appearance.appMode, this.appearance.dark)));
  }

  /** 2in1 候选右键菜单的颜色。菜单从候选调色板派生而不是键盘的，明暗跟随颜色模式（`appearance.menuTheme` 固定为 `follow`），与 `surfaceSkin` 解析键盘一侧各界面的方式相同。 */
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
    // 只列当前明暗下能用的主题，见 `GlobalTheme.fitsMode`。
    const offered: ThemeCatalogEntry[] = this.themeCatalog.filter((entry: ThemeCatalogEntry): boolean =>
      GlobalTheme.fitsMode(entry.appearance, dark));
    return offered.map((entry: ThemeCatalogEntry): KeyboardSkin => {
      if (entry.id === this.globalThemeId) {
        return this.keyboardSkin(GlobalTheme.surfaceDark(entry.appearance, dark));
      }
      // 每张卡片都带上选中后绘制时所用的季节，因此「跟随系统」卡片显示季节色调，而不是裸的令牌。
      if (entry.id === 'custom') {
        if (this.customDesign !== null) {
          return KeyboardSkin.fromDesign(entry.title, this.customDesign, dark, this.appThemeSeed(dark));
        }
        const base: ThemeCatalogEntry | null = GlobalTheme.entry(this.themeCatalog, this.customThemeBase);
        if (base === null) {
          return KeyboardSkin.fromTheme(entry.id, entry.title, null, dark, this.appThemeSeed(dark));
        }
        const baseDark: boolean = GlobalTheme.surfaceDark(base.appearance, dark);
        return KeyboardSkin.fromTheme(entry.id, entry.title, base.keyboard, baseDark, this.appThemeSeed(baseDark));
      }
      const entryDark: boolean = GlobalTheme.surfaceDark(entry.appearance, dark);
      return KeyboardSkin.fromTheme(entry.id, entry.title, entry.keyboard, entryDark, this.appThemeSeed(entryDark));
    });
  }

  /** Store a global theme picked on the keyboard and redraw every surface from it. */
  selectGlobalTheme(id: string): boolean {
    const changes: Map<string, Object> = new Map<string, Object>();
    changes.set('global_theme', id);
    // changePreferences 存好后已经按存下的文档重算了外观。
    if (!this.changePreferences(changes)) {
      return false;
    }
    this.refreshToolbar();
    return true;
  }

  /**
   * 功能面板「本地输入」列不列「指令」：设置里打开了 / 指令（`local_modes.command`，缺省关），并且现在出中文标点。Engine 只在中文标点下把没有组字时的 / 当作进入指令的键，英文标点下点它只会打出一个 /。
   */
  offersCommandMode(): boolean {
    const prepared: PreparedPreferences | undefined = this.preparedPreferences;
    return prepared !== undefined && prepared.local_modes !== undefined
      && prepared.local_modes.command === true && this.usesChinesePunctuation();
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
      if (loaded.ok !== true) {
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
      if (saved.ok !== true) {
        KeyboardLog.error(`preferences not persisted: ${saved.error}`);
        return this.fuzzyPinyin;
      }
      const applied: PreferencesUpdateResponse = JSON.parse(client.updatePreferences(this.handle,
        PrivacyGate.sessionPreferences(saved.value, this.sessionPrivate))) as PreferencesUpdateResponse;
      if (applied.ok !== true) {
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
    // 手机和平板只显示一种候选翻译（TranslationPolicy.keepPrimaryOnly），第二种语言只在 2in1 的候选窗上显示。
    if (KeyboardFormFactor.isDesktop()
        && preferences.translation_secondary_language !== undefined
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
    // 开关或表变了，缓存的音标不再对应现在画的东西，整份丢掉。
    const available: boolean = this.candidatePronunciationEnabled
      && this.pronunciationTableInstalled(preferences.translation_target_language);
    if (available !== this.candidatePronunciationAvailable) this.candidatePronunciations.clear();
    this.candidatePronunciationAvailable = available;
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
        || pattern === inputMethodEngine.PATTERN_USER_NAME,
      // 只收数字的编辑框，即 Android `EditorPolicy.numberInput` 认的数字与电话号码两类。
      digits: pattern === inputMethodEngine.PATTERN_NUMBER
        || pattern === inputMethodEngine.PATTERN_PHONE
        || pattern === inputMethodEngine.PATTERN_NUMBER_DECIMAL
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
    return this.englishMode && !this.englishCorrectionEnabled()
      && this.capitalization !== CapitalizationMode.NONE;
  }

  shouldShiftNextLetter(): boolean {
    if (!this.englishMode || this.englishCorrectionEnabled()
        || this.capitalization === CapitalizationMode.NONE) {
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

  /** The helpcode trigger a touch 123-layer key types in this state, or null; see `ChineseHelpcodePolicy.layerTrigger`. */
  helpcodeTrigger(face: string): string | null {
    return ChineseHelpcodePolicy.layerTrigger(face, this.lastView?.spelling_symbols ?? '',
      ChineseHelpcodePolicy.eligible(this.englishMode, this.editing, this.scheme, this.localMode));
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
    return this.translationRows() + this.pronunciationRows();
  }

  /** `glossRows()` 里放释义和 훈음 的那几行，不含下面单独的音标行。 */
  translationRows(): number {
    return CandidateGlossLayoutPolicy.schemeRows(this.candidateGlossRows, this.schemeName === 'korean');
  }

  /** 「显示读音」打开且随包的读音表在时，释义下面为读音单独预留的一行。按设置预留，不等读音到了才把候选撑高。 */
  pronunciationRows(): number {
    return this.pronunciationVisible() ? 1 : 0;
  }

  private pronunciationVisible(): boolean {
    return CandidatePronunciationPolicy.visible(this.candidatePronunciationEnabled,
      this.candidatePronunciationAvailable);
  }

  /** 这一行查读音用的键：英文候选是它本身，中文候选是显示出来的释义，按释义的目标语言读。 */
  private pronunciationText(candidate: Candidate): string {
    if (!this.pronunciationVisible()) return '';
    const translation: string | null = this.candidateGlossEnabled
      && !TranslationPolicy.sourceGetsNoGloss(candidate.source) ? candidate.translation ?? null : null;
    return CandidatePronunciationPolicy.lookupText(candidate.text, translation, this.pronunciationTarget());
  }

  /** 释义的目标语言，读音按它查表。 */
  private pronunciationTarget(): string {
    return this.preparedPreferences?.translation_target_language ?? 'en';
  }

  /** 查一批候选里还没缓存的音标。表在本机，与离线释义一样在界面线程上同步查；失败只是不画音标，不影响输入，也不记录查询的文字。 */
  private resolvePronunciations(candidates: Candidate[]): void {
    if (!this.pronunciationVisible()) return;
    const missing: string[] = [];
    for (const candidate of candidates) {
      if (!CandidatePresentationPolicy.isVisible(candidate.source)) continue;
      const text: string = this.pronunciationText(candidate);
      if (text.length > 0 && !this.candidatePronunciations.has(text) && !missing.includes(text)) missing.push(text);
      if (missing.length >= CandidatePronunciationPolicy.MAX_ITEMS) break;
    }
    if (missing.length === 0) return;
    const readings: Map<string, string> | null = CandidatePronunciationPolicy.decode(
      client.candidatePronunciations(CandidatePronunciationPolicy.request(missing), this.resourceDirectory));
    if (readings === null) {
      KeyboardLog.error('candidate pronunciations unavailable');
      return;
    }
    if (this.candidatePronunciations.size + missing.length > CandidatePronunciationPolicy.MAX_ITEMS * 8) {
      this.candidatePronunciations.clear();
    }
    for (const text of missing) this.candidatePronunciations.set(text, readings.get(text) ?? '');
  }

  private candidatePronunciation(candidate: Candidate): string {
    const text: string = this.pronunciationText(candidate);
    return text.length === 0 ? '' : this.candidatePronunciations.get(text) ?? '';
  }

  /** 随 HAP 暂存的读音表是否在引擎资源旁，共享层在那里找它：英文表（英文候选总要它），或者释义目标语言的那一张。 */
  private pronunciationTableInstalled(target: string): boolean {
    try {
      const parent: string = this.resourceDirectory.substring(0, this.resourceDirectory.lastIndexOf('/'));
      return parent.length > 0 && (fs.accessSync(`${parent}/pronunciations/en-phonetic.db`)
        || (target !== 'en' && CandidatePronunciationPolicy.LANGUAGES.includes(target)
          && fs.accessSync(`${parent}/target-pronunciations/${target}-phonetic.db`)));
    } catch (error) {
      KeyboardLog.error('keyboard.KeyboardSession.pronunciationTableInstalled failed');
      return false;
    }
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

  /** 切换中文标点有没有作用：固定标点锁定时没有（`PunctuationLockPolicy.toggles`），功能面板据此把「中文标点」磁贴置灰，而不是让它点了又弹回。 */
  punctuationToggles(): boolean {
    return PunctuationLockPolicy.toggles(this.punctuationLock);
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
      view.candidate_list_open);
  }

  /** Which mode-switch gestures the user left bound, as the shared document has them. */
  modeBindings(): ModeBindings {
    return this.bindings;
  }

  /** Which unmodified hardware pair selects the first/last Han character. */
  wordCharacterBinding(): string {
    return this.wordCharacter;
  }

  /** Whether punctuation keys write Chinese punctuation, which the touch strip and the function panel show as a switch. 固定标点锁定时显示锁定值，与真正出标点的共享层一致（`PunctuationLockPolicy`）。 */
  usesChinesePunctuation(): boolean {
    return PunctuationLockPolicy.effectiveChinese(this.chinesePunctuation, this.punctuationLock);
  }

  /**
   * Chinese punctuation or ASCII, which the toolbar button and the Windows Ctrl+. binding share.
   *
   * Written as a preference rather than as a live session mode because that is how this host has always changed it: the same value drives the toolbar face and the settings page, and two mechanisms for one switch is how they come to disagree.
   *
   * The switch moves only once the store has taken the write: the reload that follows skips this session's own revision, so a refused write that still flipped the flag would write punctuation the settings page says is off. Returns the state now in effect.
   *
   * 固定标点锁定时什么都不做，返回锁定值：这时共享层不看这个开关，写它只会借账号同步改掉用户其他设备的起始标点。Windows TIP 和 fcitx5 在锁定时同样不理会切换。
   */
  togglePunctuation(): boolean {
    if (!PunctuationLockPolicy.toggles(this.punctuationLock)) {
      return this.usesChinesePunctuation();
    }
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
      || EditorPolicy.excludesKeyStatistics(this.traits) || !this.privacyAllows(PrivacyRecord.KEYS)
      || this.stateDirectory.length === 0) {
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
    const generation: number = this.attributeGeneration;
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    taskpool.execute(readTypingStatisticsEnabled, this.stateDirectory).then((value: Object): void => {
      if (!TypingStatisticsPolicy.isCurrentGeneration(generation, this.attributeGeneration)
        || inputClient !== this.inputClient) return;
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

  private toolbarState(): ToolbarState {
    return {
      english: this.englishMode,
      temporaryEnglish: this.localMode === 'temporary_english' || this.isEnglishCandidates(),
      japanese: !this.englishMode && this.schemeName === 'japanese',
      korean: !this.englishMode && this.schemeName === 'korean',
      vietnamese: !this.englishMode && this.schemeName === 'vietnamese',
      tibetan: !this.englishMode && this.schemeName === 'tibetan',
      stroke: !this.englishMode && this.schemeName === 'stroke',
      capsLock: this.capsLock,
      // Korean and Vietnamese write ASCII punctuation whatever the switch says (`uses_chinese_punctuation`), and the face reports what is written. 固定标点锁定时按锁定值显示（usesChinesePunctuation）。
      chinesePunctuation: this.usesChinesePunctuation() && !this.writesAsciiPunctuation(),
      fullWidth: this.fullWidth, traditional: this.traditional
    };
  }

  toolbarImageScene(): ToolbarImageScene | null {
    const image: ToolbarImageResources | null = this.toolbarImage;
    if (image === null || this.toolbarImageRejected) return null;
    try {
      const request: string = JSON.stringify({ toolbar: image.spec,
        shown: ToolbarImagePolicy.shown(FloatingToolbarLayout.buttons(this.toolbarComponents()), this.toolbarState()) });
      const reply: ToolbarImageLayoutReply = JSON.parse(client.skinToolbarImageLayout(request)) as ToolbarImageLayoutReply;
      if (!reply.ok || reply.value === undefined || reply.value === null) return null;
      return { key: image.key, spec: image.spec, resources: image.resources, layout: reply.value };
    } catch (_) {
      return null;
    }
  }

  toolbarSize(): number[] {
    const scene: ToolbarImageScene | null = this.toolbarImageScene();
    return scene === null
      ? [FloatingToolbarLayout.widthVp(FloatingToolbarLayout.buttons(this.toolbarComponents()).length),
        FloatingToolbarLayout.heightVp()]
      : [scene.layout.widthDip, scene.layout.heightDip];
  }

  async resizeToolbarWindow(): Promise<void> {
    if (this.onToolbarResize !== undefined) await this.onToolbarResize();
  }

  /** 系统拒绝非矩形状态窗或形状预算超限时，整条回退为普通工具栏。 */
  rejectToolbarImage(key: string): void {
    if (this.toolbarImage?.key !== key || this.toolbarImageRejected) return;
    this.toolbarImageRejected = true;
    KeyboardLog.info('toolbar window shape unavailable; using native bar');
    this.refreshToolbar();
  }

  /** 将同一个状态与共享布局交给窗口。 */
  refreshToolbar(): void {
    if (this.onToolbar === undefined) return;
    this.onToolbar(this.toolbarState(), FloatingToolbarLayout.buttons(this.toolbarComponents()),
      this.toolbarFontSize(), this.toolbarScale(), this.toolbarSkin, this.toolbarImageScene());
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

  /** 告诉正在画模式角标的窗口要画什么：按悬浮工具栏的尺寸，用工具栏的调色板，明暗跟随颜色模式（`theme`，再是系统），与 macOS 的角标一致。 */
  setModeBadge(english: boolean): void {
    if (this.onModeBadge !== undefined) {
      this.onModeBadge(english, this.toolbarFontSize(), this.toolbarScale(), this.toolbarSkin);
    }
  }

  /** 当前编辑框只用标准底行（`EditorPolicy.keepsStandardRows`：密码框或只收数字）；属性还没到达时说的是上一个编辑框，到达后 `onEditorChanged` 会再通知一次。 */
  editorKeepsStandardRows(): boolean {
    return EditorPolicy.keepsStandardRows(this.traits);
  }

  /**
   * 扩展能力建手机面板时调一次：按视图上次记下的键面（`touch_panel_face.json`）和现在的偏好定第一帧的工具条与多出的行（`TouchLayoutStylePolicy.firstPanel`），并留着给视图出现时从这个高度出发核对（`firstTouchPanel`）。
   */
  decideFirstTouchPanel(phoneLandscape: boolean): TouchPanelShape {
    const settings: KeyboardPreferences = this.preferences();
    this.openedTouchPanel = TouchLayoutStylePolicy.firstPanel(this.rememberedTouchPanelFace(), settings.numberRow,
      settings.twentySixKeyStyle, TouchToolbarPolicy.rowHidden(settings.toolbarTools), phoneLandscape);
    // 底部抬高只在停靠、非手机横屏时算进面板高度；单手和分离只影响宽度，这里不用知道。
    const feedback: FeedbackSettings = this.feedbackCurrent;
    this.openedTouchLiftVp = KeyboardAdjustPolicy.liftVp(feedback.bottomLift, KeyboardAdjustPolicy.availability(
      FloatingKeyboardPolicy.active(feedback.floating, false), phoneLandscape, false, false, false));
    return this.openedTouchPanel;
  }

  /** 扩展能力建面板时算进第一帧高度的底部抬高（vp）；没有经过 `decideFirstTouchPanel` 时为 0。 */
  firstTouchLift(): number {
    return this.openedTouchLiftVp;
  }

  /** 扩展能力建面板时用的形状；没有经过 `decideFirstTouchPanel` 时就是它没记过键面时的样子。 */
  firstTouchPanel(): TouchPanelShape {
    if (this.openedTouchPanel !== null) {
      return this.openedTouchPanel;
    }
    const settings: KeyboardPreferences = this.preferences();
    return TouchLayoutStylePolicy.firstPanel(null, settings.numberRow, settings.twentySixKeyStyle,
      TouchToolbarPolicy.rowHidden(settings.toolbarTools), false);
  }

  /** 记下视图这一刻的键面，下次冷启动建面板时用；和上次写的一样时不写盘（视图每次重定高度都会调）。写不进去只记日志，下次照没记过建面板。 */
  rememberTouchPanelFace(face: TouchPanelFace): void {
    const text: string = TouchLayoutStylePolicy.touchPanelFaceText(face);
    if (text === this.touchPanelFaceWritten || this.stateDirectory.length === 0) {
      return;
    }
    this.touchPanelFaceWritten = text;
    try {
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)) {
        return;
      }
      KeyboardSession.writePrivateText(this.touchPanelFaceFile(), text);
    } catch (error) {
      KeyboardLog.error(`touch panel face not persisted: ${describeError(error)}`);
    }
  }

  private touchPanelFaceFile(): string {
    return `${this.stateDirectory}/touch_panel_face.json`;
  }

  private rememberedTouchPanelFace(): TouchPanelFace | null {
    if (this.stateDirectory.length === 0) {
      return null;
    }
    try {
      const text: string | null =
        KeyboardSession.readPrivateText(this.touchPanelFaceFile(), TOUCH_PANEL_FACE_MAX_BYTES);
      if (text === null) {
        return null;
      }
      this.touchPanelFaceWritten = text;
      return TouchLayoutStylePolicy.parseTouchPanelFace(text);
    } catch (error) {
      KeyboardLog.error(`touch panel face unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /** Whether something is being spelled right now, which decides who a physical key belongs to. */
  composing(): boolean {
    return HardwareKeyRouter.composing(this.editing, this.phrasePrefix);
  }

  /** 候选栏上此刻是不是上屏后联想（#9433）：没有组字，候选只是建议，见 `NextWordPredictionPolicy`。 */
  predicting(): boolean {
    return NextWordPredictionPolicy.showing(this.lastView?.prediction, this.currentCandidates.length);
  }

  /** 2in1 候选窗该不该在屏幕上：有组字，或者在显示联想。 */
  candidateWindowWanted(): boolean {
    return NextWordPredictionPolicy.candidateWindowWanted(this.composing(), this.predicting());
  }

  /** 联想栏按键的样式（共享偏好 `next_word_prediction_buttons`）。 */
  predictionButtons(): PredictionButtons {
    return this.predictionButtonStyle;
  }

  /**
   * 关闭上屏后联想：候选栏的关闭键 × 和实体键盘的 Esc。送 Cancel，引擎清掉建议并报已处理，不上屏、不往编辑器写字，已上屏的词序照旧保留；按空格关闭会多一个空格。不在显示联想时什么也不做，返回假。
   */
  dismissPrediction(): boolean {
    if (this.handle === 0 || !this.predicting()) return false;
    this.command(InputCommand.CANCEL);
    return true;
  }

  /** 编辑器的改动不是键盘自己写入的回声而且正显示联想：重新聚焦同一会话，清掉旧建议并切断词序上下文，免得随后点选的词被记成旧位置的后续词（Android `onUpdateSelection` 同样处理）。 */
  private refocusAfterExternalEdit(ownEcho: boolean): void {
    if (this.handle === 0 || !NextWordPredictionPolicy.refocusesOnEditorChange(ownEcho, this.predicting())) return;
    this.apply(client.focus(this.handle, true));
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

  /** 告诉引擎键面现在是哪种网格（`KeyGrid`），只在组字结束后调用。 */
  setKeyGrid(grid: number): void {
    // 拼音和双拼使用共享引擎的字母组码网格，五笔只用 14 键；日文有自己的假名九宫格，其他方案引擎会拒绝。照样去开的话，每次日文获得焦点都会在日志里留一条拒绝。
    const wanted: number = KeyboardScheme.acceptedKeyGrid(this.schemeName, grid);
    if (this.handle === 0 || this.keyGrid === wanted) {
      return;
    }
    this.keyGrid = wanted;
    this.applyView(client.setKeyGrid(this.handle, wanted));
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
    // 收起键盘时编辑框可能仍有焦点、不会有 inputStop，在这里把攒着的打字统计写盘。
    this.flushTypingStatistics();
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
   *
   * 多行编辑器里这一键插入换行，编辑器照样回报一次 `textChange`，所以和插入一样先记一笔；GO、SEARCH 这类动作不一定改动文字，没有回声的那一笔到时作废。
   */
  submit(): void {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    const generation: number = this.editorGeneration;
    this.ownEdits.reserve(Date.now());
    this.caretContextEcho.edited(true);
    inputClient.sendKeyFunction(this.enterKey).then((handled: boolean): void => {
      if (!handled && generation === this.editorGeneration && this.inputClient === inputClient) {
        this.ownEdits.release();
      }
    }).catch((error: Object): void => {
      if (generation === this.editorGeneration && this.inputClient === inputClient) {
        this.ownEdits.release();
      }
      KeyboardLog.error(`editor key function unavailable: ${describeError(error)}`);
    });
  }

  /** Begin one host-independent voice generation, so late platform callbacks can be rejected. */
  voiceStart(): number {
    if (this.handle === 0 || !this.voiceInputEnabled()) {
      return 0;
    }
    try {
      const response: VoiceGenerationResponse = JSON.parse(client.voiceStart(this.handle)) as VoiceGenerationResponse;
      return response.ok === true && Number.isSafeInteger(response.value) && response.value > 0
        ? response.value : 0;
    } catch (error) {
      KeyboardLog.error(`voice session unavailable: ${describeError(error)}`);
      return 0;
    }
  }

  voiceInputConfiguration(): VoiceInputConfiguration {
    const root: string = this.clipboardRoot();
    // 交出去的是一份拷贝，受管目录不能写回缓存的配置。ArkTS 不能展开对象（arkts-no-spread），配置里只有字符串、布尔和 token 表，JSON 往返拷贝不丢东西。
    const configuration: VoiceInputConfiguration =
      JSON.parse(JSON.stringify(this.voiceInput)) as VoiceInputConfiguration;
    configuration.local_model_root = root.length > 0 ? `${root}/voice-models` : '';
    configuration.local_model_scratch = this.voiceScratchDirectory;
    return configuration;
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

  /** 记下开始聆听时编辑框所处的位置，以便 `voiceTargetMatches` 判断识别结果能否不经确认直接上屏。 */
  captureVoiceTarget(): void {
    this.voiceTarget = this.inputClient === undefined ? null
      : { editor: this.editorGeneration, context: this.replyContextGeneration };
  }

  /** 编辑框是否仍处在 `captureVoiceTarget` 记录的位置，且没有正在进行的组字。 */
  voiceTargetMatches(): boolean {
    return this.inputClient !== undefined && VoiceRecognitionPolicy.insertsDirectly(this.voiceTarget,
      { editor: this.editorGeneration, context: this.replyContextGeneration }, this.composing());
  }

  /** Accept only the active generation and never log the recognized text. */
  voiceApply(generation: number, text: string): string | null {
    if (this.handle === 0 || generation <= 0 || text.length === 0) {
      return null;
    }
    try {
      const response: VoiceTextResponse = JSON.parse(client.voiceApply(this.handle, generation, text)) as VoiceTextResponse;
      return response.ok === true ? VoiceRecognitionPolicy.accepted(response.value) : null;
    } catch (error) {
      KeyboardLog.error(`voice result unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /** The framework hands over a fresh client on every inputStart. */
  attach(inputClient: inputMethodEngine.InputClient): void {
    // 先让视图离开键盘调整：离开时保存，此时会话还对着上一个编辑框。
    if (this.onEditorAttached !== undefined) {
      this.onEditorAttached();
    }
    this.idleDictionaryVisible = true;
    this.clearEnglishSuggestions();
    this.clearEmailSuffixes();
    this.cancelPhraseCursor();
    const editorGeneration: number = ++this.editorGeneration;
    const attributeGeneration: number = ++this.attributeGeneration;
    this.ownEdits.clear();
    this.caretContextEcho.reset();
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
      // 属性到达前不读上文（还不知道是不是密码框），到达时补读一次：焦点进入一个已经以 `xxx@` 结尾的输入框也能给出后缀。
      this.refreshEmailSuffixes();
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
      this.applyView(client.setKeyGrid(this.handle, this.keyGrid));
    }
    // 先导入设置应用经共享沙箱发布的改动，随后的修订号比较、按键反馈和应用主题的重新读取就会看到它们。
    this.syncSharedState();
    // 隐私政策的授权也随镜像导入：在设置应用里刚同意或撤回，下一次获得焦点就生效。同意时放行扩展创建时等着的匿名注册和使用统计。
    if (this.stateDirectory.length > 0) HarmonyPrivacyConsent.load(this.stateDirectory);
    this.reloadChangedPreferences();
    // 镜像导入的 `skins` 可能换掉了正在用的包而偏好修订号没动，上面那步不会重新应用外观。会话建好之后键盘只在这里导入皮肤目录（扩展 `onCreate` 的那次在 `start` 之前，`start` 本来就读新包），所以也只在这里查。视图的 reload 钩子按新外观重画。
    if (this.reloadReplacedSkinPackages() && this.onViewPreferencesReloaded !== undefined) {
      this.onViewPreferencesReloaded();
    }
    // 按键反馈和手写板设置是另一个文件，设置应用写它时不会推进偏好修订号，所以每次获得焦点都要读取，而不只在构建会话时读取。文件存在却读不到时保留上次的值，一次读取失败不能把隐私模式当成关闭。
    this.refreshFeedback();
    this.followPrivacyMode();
    // 应用主题也一样：月份变了或在设置应用里选了主题，皮肤会立即重绘，而不是最多一分钟后。真正重绘皮肤的是视图的 reload 钩子。
    if (this.refreshAppTheme(true) && this.onViewPreferencesReloaded !== undefined) {
      this.onViewPreferencesReloaded();
    }
    // 键盘显示时查一次词库快照队列；上面的重建若已排过检查，这里不会重复排。收起时停下的个人词库重试也在这里按退避接着排。
    this.scheduleDictionarySnapshotPoll();
    this.schedulePersonalDictionaryDrain();
  }

  /**
   * 长按菜单的「加入生词本」：写进键盘自己沙箱里的生词本（`PersonalWordbookPolicy`），完整体验模式下马上发布，设置应用下次回到前台时并进背单词的生词本。返回给用户看的提示。不经过 Engine，组字不变。
   *
   * 加之前先清走设置应用已经确认并过的词：用户在背单词里移出一个词以后再从这里加回来，键盘那本里不能还留着它，否则共享层会回答「已在生词本里」，这个词却回不到背单词。
   */
  async addToPersonalWordbook(candidate: string, gloss: string): Promise<string> {
    if (this.stateDirectory.length === 0) {
      return PERSONAL_WORDBOOK_FAILURE;
    }
    const mirror: SharedStateMirror | null = this.sharedMirror;
    const route: PersonalWordbookRoute = PersonalWordbookPolicy.route(mirror !== null, this.sharedFullMode());
    if (mirror !== null && route === 'settings') {
      SharedStateSync.keyboardDrainPersonalWordbook(mirror, this.stateDirectory);
    }
    let response: string;
    try {
      response = await client.personalWordbookAdd(
        PersonalWordbookPolicy.addRequest(this.stateDirectory, candidate, gloss));
    } catch (error) {
      KeyboardLog.error(`personal wordbook add failed: ${describeError(error)}`);
      return PERSONAL_WORDBOOK_FAILURE;
    }
    if (mirror !== null && route === 'settings') {
      try {
        SharedStateSync.keyboardPublish(mirror);
      } catch (error) {
        // 下次失焦或获得焦点时照常再发布。
        KeyboardLog.warn(`personal wordbook not published yet: ${describeError(error)}`);
      }
    }
    return PersonalWordbookPolicy.notice(response, route);
  }

  /** 长按菜单「朗读」要读的英文：经 `msime_client_read_aloud_text` 按候选原文和显示着的释义问共享层。设备没有文本转语音能力、没有能读的英文时为 null，菜单就不出这一项。打开菜单时在主线程调用，不读写文件。 */
  readAloudText(candidate: string, gloss: string): string | null {
    if (!HarmonyReadAloud.available()) {
      return null;
    }
    try {
      return ReadAloudPolicy.textFromReply(client.readAloudText(ReadAloudPolicy.textRequest(candidate, gloss)));
    } catch (error) {
      KeyboardLog.warn(`read aloud text failed: ${describeError(error)}`);
      return null;
    }
  }

  /** 用系统语音读出 `text`；读不出来时得到给用户看的那句话。不经过 Engine，组字不变。 */
  readAloud(text: string): Promise<string | null> {
    return HarmonyReadAloud.speak(text);
  }

  /** 设置应用和键盘共享沙箱的镜像，由扩展在拿到共享目录后交给会话；`fullMode` 每次调用时读当前的安全模式。 */
  useSharedSandbox(mirror: SharedStateMirror, fullMode: () => boolean): void {
    this.sharedMirror = mirror;
    this.sharedFullMode = fullMode;
  }

  private syncSharedState(): void {
    if (this.sharedMirror === null || this.stateDirectory.length === 0) {
      return;
    }
    const fullMode: boolean = this.sharedFullMode();
    const imported: string[] = SharedStateSync.keyboardSync(this.sharedMirror, this.stateDirectory, fullMode);
    // 设置应用发布的会话由 Rust 在锁里决定要不要采用；这里排一次让本地状态尽早跟上，云剪贴板这类认证请求自己经 `beforeRequest` 再排一次。基础访问模式下键盘不交换会话。
    if (fullMode) {
      AccountSessionAdoption.adopt(this.sharedMirror, this.stateDirectory, 'settings');
    }
    if (imported.includes('dictionary-selection') && this.handle !== 0) {
      const resources: string = this.resourceDirectory;
      const state: string = this.stateDirectory;
      const dark: boolean = this.systemDark;
      this.finishCompositionBoundary(CompositionBoundary.DEACTIVATE);
      this.stop();
      this.start(resources, state, dark);
      if (this.handle !== 0) this.apply(client.focus(this.handle, true));
    }
    // 设置应用的「我的词条」发布变了（设置页改了词、网盘同步写进了词）：写本机词库要等没有会话开着，排一次空闲时的会话重建，在那里的 `syncUserWords` 写进来。
    if (UserWordsMirrorPolicy.keyboardSyncDue(this.userWordsDue, this.userWordsMergedMark,
      UserWordsMirror.fileMark(this.sharedMirror.userWordsFile('settings')))) {
      this.userWordsDue = true;
      this.personalDictionarySettled = false;
      this.schedulePersonalDictionaryDrain();
    }
  }

  detach(): void {
    // 宿主随后隐藏面板。先让视图停掉录音，取消发生在旧编辑框还连着的时候，与 Android 在 onFinishInput 里先取消语音再收尾一致。
    if (this.onPanelHidden !== undefined) {
      this.onPanelHidden();
    }
    this.cancelPhraseCursor();
    this.editorGeneration++;
    this.attributeGeneration++;
    this.manualLanguageGeneration = -1;
    this.ownEdits.clear();
    this.caretContextEcho.reset();
    this.modeScope.deactivate();
    this.clearEnglishSuggestions();
    this.clearEmailSuffixes();
    this.invalidateReplyContext();
    this.clearSmartPunctuationRepeat();
    this.cancelOnlineProviders();
    // 键盘收起后不再查快照队列、不再重试卡住的个人词库排空，下一次显示（attach）时再查。
    this.pauseIdleDictionaryWork();
    // 编辑框失去焦点：释义词库连接和自定义译文同样不留，见 `keyboardHidden`。
    client.releaseGlossCaches();
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
    // 编辑框失去焦点：攒着的打字统计写盘。完整体验模式下把这次编辑里记下的统计、在键盘里改的偏好发布给设置应用；发布直接拷贝统计文件，所以等 flush 写完再发布，否则设置应用看到的会少掉最后一段提交。基础访问模式下键盘写不了共享目录。
    const mirror: SharedStateMirror | null =
      this.sharedMirror !== null && this.sharedFullMode() ? this.sharedMirror : null;
    this.flushTypingStatistics().then((): void => {
      if (mirror !== null) {
        SharedStateSync.keyboardPublish(mirror);
      }
    });
    // 这次编辑里学到、删掉的词也发布给设置应用，词库同步从那里送上网盘。
    this.publishUserWords();
  }

  stop(): void {
    this.cancelPhraseCursor();
    this.editorGeneration++;
    this.ownEdits.clear();
    this.caretContextEcho.reset();
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
    this.cancelDictionarySnapshotPoll();
    if (this.handle !== 0) {
      client.destroy(this.handle);
      this.handle = 0;
      this.keyGeometry.reset();
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
    this.flushTypingStatistics();
  }

  /** Invalidate host-side network work before the focused editor changes or the session dies. */
  private cancelOnlineProviders(): void {
    this.invalidateOnlineProviders();
    this.invalidateTranslationProviders();
  }

  private invalidateOnlineProviders(): void {
    if (!OnlineCandidatePolicy.hasActiveWork(
      this.onlineSignature, this.onlineTimer >= 0, this.onlineRequests.size)) return;
    this.onlineEpoch++;
    this.onlineSignature = '';
    if (this.onlineTimer >= 0) {
      clearTimeout(this.onlineTimer);
      this.onlineTimer = -1;
    }
    for (const request of this.onlineRequests) request.destroy();
    this.onlineRequests.clear();
  }

  private invalidateTranslationProviders(): void {
    if (!TranslationPolicy.hasActiveWork(
      this.translationSignature, this.translationTimer >= 0, this.translationRequests.size)) return;
    this.translationEpoch++;
    this.translationSignature = '';
    if (this.translationTimer >= 0) {
      clearTimeout(this.translationTimer);
      this.translationTimer = -1;
    }
    for (const request of this.translationRequests) request.destroy();
    this.translationRequests.clear();
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
    if (!KeyboardFormFactor.isDesktop()) {
      this.touchHeight.request(heightVp);
    }
  }

  /** Reconcile a document mutation reported by the editor with keyboard-owned edit echoes. `textLength` is the length of the editor text the change reported; only the length is kept. */
  editorTextChanged(textLength: number): void {
    this.invalidateReplyContext();
    const now: number = Date.now();
    this.phraseCursorOwnEcho = this.ownEdits.acknowledge(now);
    if (this.phraseCursorOwnEcho) {
      this.caretContextEcho.textChanged(true, this.ownEdits.pending(now) === 0, textLength);
      if (this.onEditorChanged !== undefined) {
        this.onEditorChanged();
      }
      return;
    }
    this.cancelPhraseCursor();
    this.caretContextEcho.textChanged(false, false, textLength);
    // A host edit may clear the field, replace the document or move text around the active composition. Commit what the user typed before the Engine's state can drift from the editor.
    this.finishCompositionBoundary(CompositionBoundary.DEACTIVATE);
    // 实体键盘交给应用的空格、回车、数字和退格也走到这里：联想照常让它们输入，随后在这里清掉。
    this.refocusAfterExternalEdit(false);
    this.clearEnglishSuggestions();
    this.refreshEmailSuffixes();
    this.clearSmartPunctuationRepeat();
    this.clearSmartPunctuationSpace();
    if (this.onEditorChanged !== undefined) {
      this.onEditorChanged();
    }
  }

  /** A selection move changes contextual UI but does not itself discard an active composition. */
  editorSelectionChanged(collapsed: boolean = true): void {
    // 编辑器先回报 `textChange` 再回报 `selectionChange`（见 `EditEchoLedger`），这里读到的就是紧挨着的那次 `textChange` 是不是键盘自己的回声；下面会把它清掉。
    const ownEcho: boolean = this.phraseCursorOwnEcho;
    if (this.phraseCursor !== null) {
      if (collapsed && this.phraseCursorOwnEcho) {
        this.resumePhraseCursor();
      } else {
        this.cancelPhraseCursor();
        this.syncPreview();
      }
    }
    this.phraseCursorOwnEcho = false;
    this.invalidateReplyContext();
    // 用户点到别处、拖了选区，或应用自己移了光标：旧位置的联想不再适用。
    this.refocusAfterExternalEdit(ownEcho);
    // 编辑器在每次 insertTextSync 之后也回报一次选区变化，这里只按光标前的当前词重新核对：词没变就保留建议，光标移开或词变了才重新查询或收起。直接清空会让手机纠错模式下每一键刚出的建议马上消失，用户看不到候选，也无法点原文拒绝纠错。选中了一段文字时没有可替换的当前词，照旧收起。
    if (collapsed) {
      this.refreshEnglishSuggestions();
    } else {
      this.clearEnglishSuggestions();
    }
    // 与 Android 在 `onUpdateSelection` 里重新匹配相同：无论 `@` 从哪里上屏（字母键、符号层、符面板、九键常用标点、引擎上屏），编辑器随后都会回报一次选区变化。
    // 这次回报只是输入法自己同步上屏的回声、上屏后已经按新的上文匹配过时不再读：直接英文里每个字母都会走到这里，每读一次就是一次等宿主 UI 线程作答的同步跨进程调用。
    const current: boolean = this.caretContextEcho.selectionChanged(this.ownEdits.pending(Date.now()) === 0);
    if (!current || !this.emailSuffixesReadContext()) {
      this.refreshEmailSuffixes();
    }
    if (this.onEditorChanged !== undefined) {
      this.onEditorChanged();
    }
  }

  /** Insert through the editor while reserving the asynchronous text-change echo it produces. */
  private insertEditorText(text: string): void {
    if (!this.phraseCursorEditing) this.cancelPhraseCursor();
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    if (this.previewText.length > 0 || this.commitSynchronously || this.englishCorrectionEnabled()) {
      // The insertion takes the preview text's place. It has to land before the next preview update, which is synchronous, and an asynchronous insert gives no such order.
      try {
        this.insertEditorTextSync(text);
      } catch (error) {
        KeyboardLog.error(`editor insertion unavailable: ${describeError(error)}`);
      }
      return;
    }
    const generation: number = this.editorGeneration;
    this.ownEdits.reserve(Date.now());
    this.caretContextEcho.edited(true);
    inputClient.insertText(text).catch((error: Object): void => {
      if (generation === this.editorGeneration && this.inputClient === inputClient) {
        this.ownEdits.release();
      }
      KeyboardLog.error(`editor insertion unavailable: ${describeError(error)}`);
    });
  }

  /** 同步交付完整正文；读回和替换编辑器时沿用同一条入口。 */
  private insertEditorTextSync(text: string): void {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    const editor: number = this.editorGeneration;
    const applied: number = this.applyGeneration;
    this.editEditorSync((): void => inputClient.insertTextSync(text));
    // 较新的转换或编辑器持有自己的预编辑，旧调用返回时不得清掉它。
    if (editor === this.editorGeneration && applied === this.applyGeneration && this.inputClient === inputClient) {
      this.previewText = '';
    }
  }

  /** 同步改动一次编辑器，并为它的 `textChange` 回声记一笔；调用抛错说明编辑器没有改动，那一笔随即撤回。 */
  private editEditorSync(edit: () => void): void {
    if (!this.phraseCursorEditing) this.cancelPhraseCursor();
    this.ownEdits.reserve(Date.now());
    this.caretContextEcho.edited(false);
    try {
      edit();
    } catch (error) {
      this.ownEdits.release();
      throw new Error(describeError(error));
    }
  }

  /** 不保留跨焦点或跨其他写入的模板位置，也不让迟到回声再移动新文档。 */
  private cancelPhraseCursor(): void {
    this.phraseCursor = null;
    this.phraseCursorOwnEcho = false;
    this.phraseCursorCurrent = (): boolean => false;
    if (this.phraseCursorTimer >= 0) clearTimeout(this.phraseCursorTimer);
    this.phraseCursorTimer = -1;
  }

  private resumePhraseCursor(): void {
    const request: QuickPhraseCursor | null = this.phraseCursor;
    if (request === null) return;
    const current: boolean = this.phraseCursorCurrent();
    if (current) {
      const wasEditing: boolean = this.phraseCursorEditing;
      this.phraseCursorEditing = true;
      try {
        request.resume(this.phraseCursorCurrent);
      } finally {
        this.phraseCursorEditing = wasEditing;
      }
    }
    if (this.phraseCursor !== request) return;
    if (!current || !request.pending() || !this.phraseCursorCurrent()) {
      this.cancelPhraseCursor();
      if (current) this.syncPreview();
    }
  }

  /** 手机上拼写此刻是否写在输入框里，候选栏因此不画读音；见 InlinePreeditPolicy.hidesStripReading。 */
  compositionInField(): boolean {
    const view: EngineView | undefined = this.lastView;
    return view !== undefined && InlinePreeditPolicy.hidesStripReading(KeyboardFormFactor.isDesktop(),
      this.previewText, SchemeCompositionPolicy.drawsPreedit(
        SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode)));
  }

  /**
   * Bring the editor's preview text in line with the composition the Engine last reported.
   *
   * Every change is a document edit the editor echoes back like an insertion, so it is reserved the same way; otherwise the echo would read as the user editing the field and end the composition it came from. 设置预上屏文字和结束预上屏是两次调用，各记一笔：结束时文字没变就不会有回声，多记的那一笔到时作废。
   */
  private syncPreview(): void {
    if (this.phraseCursor !== null) return;
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    const view: EngineView | undefined = this.lastView;
    // 拼写一栏传的是候选条画出来的那一串：多数方案就是 editing_text，笔画方案是笔画字形，文档里不出现键入的字母。分词一栏是拼音分词形式，全拼九键是读音而不是按下的数字。
    const rulesScheme: number = view === undefined ? -1
      : SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode);
    const wanted: string = view === undefined ? '' : InlinePreeditPolicy.text(this.inlinePreeditStyle,
      KeyboardFormFactor.isDesktop(), this.previewSupported, KeyboardSession.drawnReading(view),
      SchemeCompositionPolicy.fieldSpelling(rulesScheme, view.editing_text, view.preedit, view.nine_key_reading ?? ''),
      view.phrase_prefix ?? '', SchemeCompositionPolicy.drawsPreedit(rulesScheme),
      SchemeCompositionPolicy.keepsReadingOnStrip(rulesScheme,
        this.keyGrid === KeyGrid.FOURTEEN_KEY || this.keyGrid === KeyGrid.EIGHTEEN_KEY
          || this.keyGrid === KeyGrid.EIGHTEEN_KEY_SHUANGPIN));
    if (inputClient === undefined || wanted === this.previewText) return;
    const shown: string = this.previewText;
    try {
      this.editEditorSync((): void => inputClient.setPreviewTextSync(wanted, PREVIEW_REPLACE_CURRENT));
      if (wanted.length === 0) this.editEditorSync((): void => inputClient.finishTextPreviewSync());
      this.previewText = wanted;
      return;
    } catch (error) {
      KeyboardLog.error(`editor preview unavailable: ${describeError(error)}`);
    }
    // The editor refused preview text after saying it took it; stop offering it for this focus.
    this.previewSupported = false;
    this.previewText = '';
    if (shown.length === 0) return;
    // Letters of a composition must not stay behind in the document: end the preview and take them out, the way a cancelled composition leaves nothing.
    try {
      this.editEditorSync((): void => inputClient.finishTextPreviewSync());
      // Only when the letters are still there as plain text: an editor that dropped them with the preview must not lose what precedes them.
      if (inputClient.getForwardSync(shown.length) === shown) {
        this.deleteEditorBackwardSync(shown.length);
      }
    } catch (error) {
      KeyboardLog.error(`editor preview cleanup unavailable: ${describeError(error)}`);
    }
  }

  /**
   * 删掉光标前的 `count` 个字符，并为它在编辑器里产生的异步文本变化回声预留一笔。
   *
   * 鸿蒙输入法框架的方向命名与 Android 相反：`deleteForwardSync` 删光标之前的文字，`deleteBackwardSync` 删光标之后的。英文文档只写了 "Delete text forward/backward"，看不出以什么为参照，这里以设备实测为准（HarmonyOS 6.0.2 模拟器，光标在 `abc` 末尾：`deleteForward(1)` 得到 `ab`，`deleteBackward(1)` 什么也不删且照样报告成功）。以前这里调的是 `deleteBackwardSync`，于是退格、智能标点替换、AI 润色和翻译的替换在光标位于末尾时都悄无声息地什么也不删，光标在中间时删的是后面的字。
   */
  private deleteEditorBackwardSync(count: number): void {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    this.editEditorSync((): void => inputClient.deleteForwardSync(count));
  }

  /** Resume one durable cloud snapshot before a shared Engine session takes the read lock. */
  private processDictionarySnapshot(options: Object, stateRoot: string): void {
    const staging: string = `${stateRoot}/dictionary-snapshot-staging`;
    try {
      if (!fs.accessSync(staging)) fs.mkdirSync(staging, true);
      // 每次空闲处理都重新读本沙箱的会话文件（目录模式的 status，只读本地文件）：键盘进程可能比设置里的一次账号切换活得更久。
      const accountId: string | null = statusUserId(client.accountStatus(stateRoot));
      // 队列空着时（几乎总是）Rust 只读一下状态就返回，不在构建会话、键盘出来之前扫整个用户词库算本机版本、重写并 fsync state.json。显示后的队列查询（`dictionarySnapshotNeedsWork`）同样只读；本机版本只在真有请求、这里认领之前发布。
      const action: SnapshotQueueProcessAction = {
        operation: 'process', directory: `${stateRoot}/dictionary-snapshots`,
        staging_root: staging, options: options, account_id: accountId, skip_when_idle: true
      };
      const reply: SnapshotQueueProcessReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueProcessReply;
      if (reply.ok !== true) {
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
    if (action.operation === 'updates') {
      // 更新状态与后台下载不准备工作词库，也不触碰正在组字的会话。
      try {
        const loaded: SnapshotResponse = JSON.parse(client.loadPreferences(stateRoot)) as SnapshotResponse;
        const snapshot: PreferencesSnapshot | null = NativeReplyPolicy.successfulValue(loaded);
        if (snapshot === null) return JSON.stringify(loaded);
        const options: DictionaryUpdateOptions = {
          api_version: 1, resources: resources, preferences_directory: stateRoot,
          user_data: `${stateRoot}/user`, cache: `${stateRoot}/cache`,
          dictionaries: `${stateRoot}/user/dictionaries`, preferences: snapshot.preferences,
          edition: AppEdition.current().id,
        };
        const languages: string = `${resources.substring(0, resources.lastIndexOf('/'))}/language-dictionaries`;
        if (fs.accessSync(languages)) options.language_dictionaries = languages;
        return client.dictionary(JSON.stringify({ options: options, action: action }));
      } catch (error) {
        KeyboardLog.error(`dictionary update unavailable: ${describeError(error)}`);
        return JSON.stringify({ ok: false, error: 'dictionary update unavailable' });
      }
    }
    const decision = DictionaryMaintenancePolicy.decide(action.operation, this.composing());
    if (!decision.allowed) {
      return JSON.stringify({ ok: false, error: decision.error });
    }
    const maintenance: boolean = decision.maintenance;
    const hadSession: boolean = this.handle !== 0;
    const english: boolean = this.englishMode;
    const keyGrid: number = this.keyGrid;
    if (maintenance && hadSession) {
      this.stop();
    }
    let result: string;
    try {
      const bootstrap: PrepareRequest = KeyboardSession.prepareRequest(resources, stateRoot);
      const prepared: PrepareResponse =
        JSON.parse(client.prepareHost(JSON.stringify(bootstrap))) as PrepareResponse;
      if (prepared.ok !== true) {
        return JSON.stringify(prepared);
      }
      const request: DictionaryRequest = { options: prepared.value, action: action };
      // Same request shape, different store. The queued route writes the personal-dictionary file
      // beside the preferences and is drained in bounded idle turns; the Engine route needs the
      // maintenance lock this method has just taken.
      if (decision.queued) {
        result = client.personalDictionaryRequest(JSON.stringify(request));
        this.personalDictionarySettled = false;
        // 用户刚排进新的词条：之前因失败退避的重试作废，按两秒重新开始。
        this.personalDictionaryStalls = 0;
        if (this.personalDictionaryTimer >= 0) {
          clearTimeout(this.personalDictionaryTimer);
          this.personalDictionaryTimer = -1;
        }
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
          this.keyGrid = keyGrid;
          if (this.inputClient !== undefined) {
            this.apply(client.focus(this.handle, true));
            this.refreshTypingEffects();
            this.applyView(client.setEnglishMode(this.handle, english));
            this.applyView(client.setKeyGrid(this.handle, keyGrid));
          }
        } else {
          KeyboardLog.error('dictionary maintenance left the session unavailable');
        }
      }
    }
    return result;
  }

  /**
   * 设置页的命名词库操作（`msime_client_dictionary_collections`），在原生工作线程上执行。
   *
   * 集合只是元数据，词条经由个人词库队列交给 Engine，所以这里不碰键盘的会话，也不需要维护锁。API 12 起设置应用和键盘各有自己的沙箱，在共享沙箱接上之前，这里写下的集合键盘看不到（platforms/harmony/README.md「输入法扩展的独立沙箱」）。
   */
  async dictionaryCollections(actionDocument: string, resources: string,
    stateRoot: string): Promise<string> {
    let action: DictionaryAction;
    try {
      const decoded: DictionaryAction | null = JSON.parse(actionDocument) as DictionaryAction | null;
      if (decoded === null || !DICTIONARY_COLLECTION_OPERATIONS.includes(decoded.operation)) {
        return JSON.stringify({ ok: false, error: 'collections_invalid' });
      }
      action = decoded;
    } catch (error) {
      return JSON.stringify({ ok: false, error: 'collections_invalid' });
    }
    const bootstrap: PrepareRequest = KeyboardSession.prepareRequest(resources, stateRoot);
    const prepared: PrepareResponse =
      JSON.parse(client.prepareHost(JSON.stringify(bootstrap))) as PrepareResponse;
    if (prepared.ok !== true) {
      return JSON.stringify(prepared);
    }
    const request: DictionaryRequest = { options: prepared.value, action: action };
    return await client.dictionaryCollectionsAsync(JSON.stringify(request));
  }

  /** Whether the key did anything. The touch keyboard ignores the answer; the hardware path hands a declined letter back to the application. */
  press(ascii: number, shift: boolean): boolean {
    return this.pressKey(ascii, shift, null);
  }

  /**
   * 触屏 26 键双拼的字母键和 `;`：与 `press` 相同，只是把按下点和键位几何一起交给引擎（`msime_client_touch_character`），双拼纠错按落点计价。几何只在布局变了或换了会话时下发；下发被拒时这一键照常走 `msime_client_character`。是否该走这里由视图按 `TouchPointPolicy.usesTouch` 决定。
   */
  pressTouch(ascii: number, shift: boolean, touch: TouchKey): boolean {
    return this.pressKey(ascii, shift, touch);
  }

  private pressKey(ascii: number, shift: boolean, touch: TouchKey | null): boolean {
    if (this.commitEnglishBoundary(String.fromCharCode(ascii))) {
      this.tapFeedback();
      return true;
    }
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
    const handled: boolean = this.apply(this.characterReply(ascii, shift, touch));
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
    // 笔画方案空组合时只有 h s p n z 开始组字，通配键 x 和其它字母按约定交回宿主，同样不是故障。
    const strokeDeclined: boolean = this.rulesScheme() === SchemeTraits.STROKE
      && !StrokeLayout.startsComposition(String.fromCharCode(ascii));
    if (!consumed && before.length === 0 && !capital && !strokeDeclined) {
      KeyboardLog.error(`composing key produced no composition: scheme=${this.schemeName}`
        + ` local=${this.localMode} english=${this.englishMode} candidates=${this.currentCandidates.length}`
        + ` shift=${shift} caps=${this.capsLock}`);
    }
    return consumed;
  }

  /** 一键的引擎答复：有按下点且几何已注册时走 `touchCharacter`，否则走 `character`。 */
  private characterReply(ascii: number, shift: boolean, touch: TouchKey | null): string {
    if (touch === null) {
      return client.character(this.handle, ascii, shift);
    }
    const handle: number = this.handle;
    const geometry: number = this.keyGeometry.register(handle, touch.layout,
      (json: string): boolean => KeyboardSession.keyGeometryAccepted(client.setKeyGeometry(handle, json), json));
    if (geometry === 0) {
      return client.character(handle, ascii, shift);
    }
    return client.touchCharacter(handle, ascii, shift, geometry, touch.point.x, touch.point.y);
  }

  /** 引擎是否收下了这份几何。收下时把整份 JSON 打进 hilog（`hdc shell hilog | grep 'key geometry'`），这是导出真机几何、替换评测参考布局的调试出口；几何只有键的位置和大小，不含输入内容。 */
  private static keyGeometryAccepted(raw: string, json: string): boolean {
    const reply: GeometryReply = JSON.parse(raw) as GeometryReply;
    if (reply.ok !== true) {
      KeyboardLog.error(`key geometry refused: ${reply.error ?? 'malformed reply'}`);
      return false;
    }
    KeyboardLog.info(`key geometry ${json}`);
    return true;
  }

  /**
   * 14/18 键键面上的一键：`ascii` 是这一组的首字母，经 `msime_client_grid_key` 交给引擎，由它归成组码按网格解码。不走 `press()`：那条路给 26 键字母，会按 Shift 当辅助码、在双拼和英文直出里另有分支，而组码键的一下是一组字母，不是其中某一个。
   *
   * 引擎不受理（26 键正在组字、专用英文或本地模式）时这一下不输入任何东西，与九键在这些情况下的数字键相同；键面在这些情况下本来也不画 14 键。
   *
   * 不组字的编辑框（数字、电话、日期这类不是文本的输入框）同样不输入：组码键的一下是一组字母，照字面写进去只能挑这一组的首字母，未必是用户要的那个。与 Android 相同（没有引擎会话时 `gridKey` 不处理）。密码、网址和邮箱框画 26 键（`EditorPolicy.prefersFullFace`），走不到这里。
   */
  pressGridKey(ascii: number): boolean {
    this.clearSmartPunctuationRepeat();
    if (!this.composes()) {
      return false;
    }
    if (this.handle === 0) {
      KeyboardLog.error('grid key ignored: no session');
      return false;
    }
    this.tapFeedback();
    const before: string = this.editing;
    const handled: boolean = this.apply(client.gridKey(this.handle, ascii));
    const consumed: boolean = handled || this.editing !== before;
    if (!consumed) {
      // 只记失败的形状，不记按键：hilog 能被工具读到，按了什么是用户的输入。
      KeyboardLog.error(`grid key produced no composition: scheme=${this.schemeName}`
        + ` local=${this.localMode} english=${this.englishMode} grid=${this.keyGrid}`);
    }
    return consumed;
  }

  /**
   * One glide stroke (滑行输入), as `GlideTypingPolicy.request` built it, applied exactly as a letter's reply is. Whether the Engine typed it: a stroke it declines is dropped, and its keys are not typed instead. The stroke gets the one key feedback a press would, however many keys it crossed.
   */
  glide(request: string): boolean {
    if (!this.composes() || this.handle === 0) {
      return false;
    }
    this.clearSmartPunctuationRepeat();
    this.tapFeedback();
    return this.apply(client.glide(this.handle, request));
  }

  /** Route an ASCII punctuation key through the shared editor-context policy. */
  punctuation(ascii: number): void {
    if (this.handle === 0 || this.inputClient === undefined) return;
    if (!this.punctuationLockedChinese()
        && this.commitEnglishBoundary(String.fromCharCode(ascii))) return;
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
          // deleteEditorBackwardSync(1) removes the one scalar just before the caret. The preceding
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
      if (EngineViewValuePolicy.isObject(response) && response.ok === true
          && EngineViewValuePolicy.isObject(response.value)) {
        // 成对补全整段交给 `PairedPunctuationPolicy.punctuationCommit`：光标短语模板原样落地、不补后半个；否则在 apply() 上屏之前把后引号改回前引号，编辑器收到前引号，再由下面的补全写后半个。
        const landed: PairedPunctuationCommit = PairedPunctuationPolicy.punctuationCommit(
          response.value.commit, response.value.commit_cursor_suffix, ascii, this.appearance.pairedPunctuation);
        if (landed.commit !== response.value.commit) {
          response.value.commit = landed.commit;
          raw = JSON.stringify(response);
        }
        committed = response.value.commit;
        completion = landed.completion;
      }
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
    // 顿号的反查按按下的键和 View 的顿号开关定（契约 enumerationComma 段）。
    const punctuationMarks: PunctuationMarksView | undefined =
      this.lastView?.punctuation_marks;
    this.smartPunctuationSpaceSnapshot = SmartPunctuationSpacePolicy.arm(
      committed, completion !== null, this.smartPunctuation, this.smartPunctuationSpaceConvert,
      this.editorGeneration, ascii,
      ChineseSymbolFaces.enumerationCommaBackslash(punctuationMarks),
      ChineseSymbolFaces.enumerationCommaSlash(punctuationMarks));
    if (completion === null || this.inputClient === undefined) return;
    try {
      this.insertEditorTextSync(completion.closing);
      this.recordTypingStatistics(completion.closing, this.typingSource());
      this.refreshEnglishSuggestions();
      if (completion.opening === 0x3c) {
        const balanced: string = client.balancePairedPunctuationAfterAutoClose(
          this.handle, completion.opening);
        const balanceResponse: NativeReply<Object> = JSON.parse(balanced) as NativeReply<Object>;
        if (balanceResponse.ok !== true) {
          KeyboardLog.error(`book-title punctuation balance refused: ${balanceResponse.error}`);
        }
      }
      // 移动光标同样会回报一次 `textChange`。
      const inputClient: inputMethodEngine.InputClient = this.inputClient;
      this.editEditorSync((): void => inputClient.moveCursorSync(inputMethodEngine.Direction.CURSOR_LEFT));
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
      wubi: HardwareKeyRouter.spellsWithoutSyllables(this.schemeName),
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

  /**
   * 数字行的数字，与 Android 数字行（`DeclinedKeyPolicy`）相同：先交给引擎，当前页有对应候选的 1–9 由运行时选词（韩语打开汉字列表时选汉字）；引擎不收时先结束还在的组合，再在后面上屏这个数字。是否收下只看引擎的 handled，不能像 `press` 那样把组字变了也算作收下：韩语音节、越南语单词等方案的引擎会先上屏组字再把数字交回宿主，这时组字已经空了，数字仍要由键盘输入；组合已经被引擎结束时 `commitThenType` 的结束组合什么也不做。
   */
  pressElseCommitThenType(ascii: number): void {
    if (!this.composes() || this.handle === 0) {
      this.press(ascii, false);
      return;
    }
    this.clearSmartPunctuationRepeat();
    this.tapFeedback();
    if (this.apply(client.character(this.handle, ascii, false))) return;
    this.commitThenType(ascii);
  }

  /**
   * 藏文的空格：先把键交给引擎，有组字时引擎上屏藏文加音节点并吞掉这个键；引擎没处理时（没有组字，或 Esc 锁定原文后只上屏原文），由键盘在上屏内容之后插入这个键本身，和硬件键盘把未处理的键交回应用的效果一致。
   */
  pressThenType(ascii: number): void {
    if (!this.composes() || this.handle === 0) {
      this.press(ascii, false);
      return;
    }
    this.clearSmartPunctuationRepeat();
    this.tapFeedback();
    const handled: boolean = this.apply(client.character(this.handle, ascii, false));
    if (handled || this.inputClient === undefined) return;
    const text: string = String.fromCharCode(ascii);
    this.insertEditorText(text);
    this.recordTypingStatistics(text, this.typingSource());
  }

  /**
   * 触屏键盘的空格，与 iOS 的 `handleSpace` 相同：刚上屏的中文标点先由智能标点改回半角；其余交给运行时的「选高亮候选」（`SELECT_HIGHLIGHTED`，注音这类用空格拼写的方案由运行时转成空格字符），组字中就是上屏高亮的候选；运行时不处理（没有组字）时由键盘自己输入空格。引擎的 `character(0x20)` 从不选词，空格以前走 `press` 时，中文模式下组字中按空格什么也不上屏，没有组字时也打不出空格。
   */
  pressSpace(): void {
    if (!this.composes() || this.handle === 0) {
      this.press(0x20, false);
      return;
    }
    if (this.commitEnglishBoundary(' ')) {
      this.tapFeedback();
      return;
    }
    if (this.convertPunctuationSpace(0x20)) {
      return;
    }
    this.clearSmartPunctuationRepeat();
    this.tapFeedback();
    if (this.command(InputCommand.SELECT_HIGHLIGHTED) || this.inputClient === undefined) return;
    this.insertEditorText(' ');
    this.recordTypingStatistics(' ', this.typingSource());
  }

  /** `commitThenType` for text the keyboard chose rather than a key's own character: a Zhuyin symbol-layer mark whose ASCII key the Dachen editor would take as a phonetic or tone key. */
  commitThenInsert(text: string): void {
    this.finishComposition();
    if (this.inputClient === undefined) return;
    this.insertEditorText(text);
    this.recordTypingStatistics(text, this.typingSource());
  }

  /**
   * 九键左侧符号栏里用户自定义的一项（#10223）：先结束组字，与点同一层出厂项时一样（字母层按首选上屏；数字层 `digits` 为真，与 `insertNumberPadLiteral` 一样原样上屏），再按字面上屏，不做全半角和繁简转换；整串正好是一整对符号（`PairedPunctuationPolicy.literalPairClosing`）时光标退到两半之间，与 Android `commitLiteralText` 相同。
   *
   * 组字的上屏和这一项都同步写进编辑器，次序不会被异步插入打乱，退光标时后半个已经在光标右边。
   */
  insertSidebarSymbol(text: string, digits: boolean = false): void {
    if (digits) {
      this.commitSynchronously = true;
      this.commitRaw();
      this.commitSynchronously = false;
    } else {
      this.finishBeforeKey();
    }
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return;
    try {
      this.insertEditorTextSync(text);
      if (PairedPunctuationPolicy.literalPairClosing(text) !== null) {
        this.editEditorSync((): void => inputClient.moveCursorSync(inputMethodEngine.Direction.CURSOR_LEFT));
      }
    } catch (error) {
      KeyboardLog.error(`sidebar symbol insertion unavailable: ${describeError(error)}`);
      return;
    }
    this.recordTypingStatistics(text, this.typingSource());
    this.refreshCaretSuggestions();
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
    const actualIndex: number = this.engineCandidateIndex(index);
    if (actualIndex < 0) return;
    this.apply(client.select(this.handle, this.generation, actualIndex));
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
    if (!EngineViewValuePolicy.isObject(response) || response.ok !== true
        || !EngineViewValuePolicy.isObject(response.value)) {
      KeyboardLog.error(EngineViewValuePolicy.isObject(response)
        ? `candidate edge refused: ${response.error}` : 'candidate edge refused: malformed reply');
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
    const applyGeneration: number = ++this.applyGeneration;
    const editorGeneration: number = this.editorGeneration;
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    const current: () => boolean = (): boolean => applyGeneration === this.applyGeneration
      && editorGeneration === this.editorGeneration && inputClient === this.inputClient;
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
    if (!EngineViewValuePolicy.isObject(response) || response.ok !== true) {
      KeyboardLog.error(EngineViewValuePolicy.isObject(response)
        ? `engine refused: ${response.error}` : 'engine refused: malformed reply');
      return false;
    }
    const value: EngineValue = response.value;
    if (!EngineViewValuePolicy.isObject(value) || !EngineViewValuePolicy.isValid(value.view)) {
      KeyboardLog.error('engine view has malformed numeric fields');
      return false;
    }
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
      // 来源 18 的中文片段已由运行时转换，按提交上下文保留日文和英文。
      const output: string = value.commit_context?.script_conversion === false ? commit : this.asTraditional(commit);
      const requestedSuffix: string | null | undefined = value.commit_cursor_suffix;
      const convertedSuffix: string | null = typeof requestedSuffix === 'string'
        ? (value.commit_context?.script_conversion === false ? requestedSuffix : this.asTraditional(requestedSuffix))
        : null;
      const suffix: string | null = QuickPhraseCursor.suffix(commit, requestedSuffix, output, convertedSuffix);
      if (suffix !== null && suffix.length > 0 && inputClient !== undefined) {
        const editor: QuickPhraseEditor = {
          insert: (text: string): void => this.insertEditorTextSync(text),
          forward: (length: number): string => inputClient.getForwardSync(length),
          backward: (length: number): string => inputClient.getBackwardSync(length),
          cursor: (): number => inputClient.getTextIndexAtCursorSync(),
          select: (position: number): void => this.editEditorSync((): void =>
            inputClient.selectByRangeSync({ start: position, end: position }))
        };
        this.cancelPhraseCursor();
        const request: QuickPhraseCursor = new QuickPhraseCursor(editor, output, suffix);
        const wasEditing: boolean = this.phraseCursorEditing;
        try {
          this.phraseCursor = request;
          this.phraseCursorCurrent = (): boolean => this.phraseCursor === request
            && editorGeneration === this.editorGeneration && inputClient === this.inputClient;
          this.phraseCursorEditing = true;
          request.submit(this.phraseCursorCurrent, this.previewText);
          if (this.phraseCursor !== request) return value.handled === true;
          if (!request.pending() || !this.phraseCursorCurrent()) {
            this.cancelPhraseCursor();
          } else {
            this.phraseCursorTimer = setTimeout((): void => {
              if (this.phraseCursor !== request) return;
              const stillCurrent: boolean = this.phraseCursorCurrent();
              this.cancelPhraseCursor();
              if (stillCurrent) this.syncPreview();
            }, EditEchoLedger.ECHO_TIMEOUT_MS);
          }
        } catch (error) {
          if (this.phraseCursor === request) this.cancelPhraseCursor();
          KeyboardLog.error(`editor insertion unavailable: ${describeError(error)}`);
        } finally {
          this.phraseCursorEditing = wasEditing;
        }
      } else {
        this.cancelPhraseCursor();
        this.insertEditorText(output);
      }
      if (!current()) return value.handled === true;
      // What the V, / and @ modes generated is not the user's typing: an evaluated sum or an expanded command template would count characters nobody typed.
      if (value.commit_context?.typing_statistics !== false) {
        this.recordTypingStatistics(output, this.typingSource());
      }
      if (!this.soundsSilencedByEditor()) {
        this.soundPlayer.commit();
        this.typingEffect(TYPING_EFFECT_COMMIT);
      }
    }
    if (current()) this.render(value.view);
    return value.handled === true;
  }

  /** For the entry points whose reply is a bare view. */
  private applyView(raw: string): void {
    this.applyGeneration++;
    this.exitTranslationPage();
    let response: ViewResponse;
    try {
      response = JSON.parse(raw) as ViewResponse;
    } catch (error) {
      KeyboardLog.error(`unreadable engine view: ${describeError(error)}`);
      return;
    }
    if (!EngineViewValuePolicy.isObject(response) || response.ok !== true) {
      KeyboardLog.error(EngineViewValuePolicy.isObject(response)
        ? `engine refused: ${response.error}` : 'engine refused: malformed reply');
      return;
    }
    if (!EngineViewValuePolicy.isValid(response.value)) {
      KeyboardLog.error('engine view has malformed numeric fields');
      return;
    }
    this.render(response.value);
  }

  private diagnostic: string = '';
  private fullWidth: boolean = false;
  private bindings: ModeBindings = DEFAULT_MODE_BINDINGS;
  /** 上次读到的设备本地反馈和手写设置：在启动时、每次编辑框获得焦点时以及键盘保存它们时更新。 */
  private feedbackCurrent: FeedbackSettings = KeyboardFeedback.DEFAULTS;
  /** 最近一次应用的偏好里的 `plugins`，声音播放器的包和音量从这里取。 */
  private soundPlugins: PluginPreferenceDocument | undefined = undefined;
  /** 当前会话是否按隐私模式构建：关闭学习并标记为私密。隐私模式与之不再相符时重建会话。 */
  private sessionPrivate: boolean = false;
  // 上次解析出的应用主题季节种子，每种模式一份；为 `null` 时键盘停留在基础令牌上。
  private appThemeSeedLight: AppThemeSeed | null = null;
  private appThemeSeedDark: AppThemeSeed | null = null;
  /** 上次解析应用主题的时间，取 `Date.now()`；0 表示从未解析，因此第一次检查一定会执行。 */
  private appThemeCheckedAt: number = 0;
  // The global theme as last resolved, one answer per mode; null draws the native tokens.
  private preparedPreferences: PreparedPreferences | undefined = undefined;
  private themeCatalog: ThemeCatalogEntry[] = [];
  private themeLight: ResolvedTheme | null = null;
  private themeDark: ResolvedTheme | null = null;
  private globalThemeId: string = 'system';
  private customThemeBase: string = 'system';
  private customDesign: CustomKeyboardSkin | null = null;
  private vibratorFailed: boolean = false;
  /** 上一下按键振动还没交给振动服务。这时再按一下就不振了：排队的振动会在手指离开后才一串串地振。 */
  private vibrationPending: boolean = false;
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
  private toolbarImage: ToolbarImageResources | null = null;
  private toolbarImageRejected: boolean = false;
  private toolbarSkin: ToolbarSkin = ToolbarSkinPolicy.base(GlobalTheme.candidateColors(null, false));
  private onlineTimer: number = -1;
  private onlineEpoch: number = 0;
  private readonly onlineRequests: Set<http.HttpRequest> = new Set<http.HttpRequest>();
  private onlineSignature: string = '';
  /** 问过 AI 却没有结果的那次请求的签名：同一组字不再向 AI 发请求；组字一变就清掉（#6888）。 */
  private aiResolvedSignature: string = '';
  private translationTimer: number = -1;
  private translationEpoch: number = 0;
  private readonly translationRequests: Set<http.HttpRequest> = new Set<http.HttpRequest>();
  private translationSignature: string = '';
  /** 2in1 候选窗上一次量宽度时的那一页和量出的宽度，「仅当前高亮」时同一页里不让窗口变窄。 */
  private candidateWidthPage: string = '';
  private candidateWidthFloorVp: number = 0;
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
  /** 候选条隐藏内部占位行后，显示序号对应的 Engine 索引。 */
  private visibleCandidateIndices: number[] = [];
  /** 展开面板隐藏内部占位行后，显示序号对应的 Engine 全量列表索引。 */
  private expandedCandidateIndices: number[] = [];
  private translationPage: string[] | null = null;
  private translationHighlight: number = 0;
  private lastView: EngineView | undefined = undefined;
  /** 联想栏按键的样式，见 `predictionButtons`。 */
  private predictionButtonStyle: PredictionButtons = PredictionButtons.EXPAND_ONLY;
  private scheme: number = 0;
  private localMode: string = 'none';
  /** Revision of the preference document the session was last built from; -1 before the first. */
  private preferencesRevision: number = -1;
  /** `reloadChangedPreferences` 上次读到的偏好文档的路径和文件身份，以及当时的修订号，见 `storedRevisionIfMoved`。 */
  private checkedPreferencesIdentity: string | null = null;
  private checkedPreferencesRevision: number = -1;
  /** Set by Ctrl+Shift+E; only meaningful while `englishMode` is on. */
  private englishCandidates: boolean = false;

  private render(view: EngineView): void {
    if (!EngineViewValuePolicy.isValid(view)) {
      KeyboardLog.error('engine view has malformed numeric fields');
      return;
    }
    this.generation = view.generation;
    this.currentCandidates = view.candidates.slice();
    this.visibleCandidateIndices = CandidatePresentationPolicy.visibleIndices(
      view.candidates.map((candidate: Candidate): number => candidate.source));
    // Special sources are filtered before the existing candidate.translation ?? null, this.candidateGlossEnabled presentation path consumes them.
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
    // 2in1 候选窗跟着它出现和收起：联想没有组字，也要有候选窗，否则建议看不见（`NextWordPredictionPolicy.candidateWindowWanted`）。
    const active: boolean = NextWordPredictionPolicy.candidateWindowWanted(
      HardwareKeyRouter.composing(view.editing_text, view.phrase_prefix ?? ''),
      NextWordPredictionPolicy.showing(view.prediction, view.candidates.length));
    if (active !== this.composedLast) {
      this.composedLast = active;
      if (this.onComposition !== undefined) {
        this.onComposition(active);
      }
    }
    const reading: string = KeyboardSession.drawnReading(view);
    // Korean's only candidates are the Hanja of the composing syllable, and their annotation is the 훈음 (msime_client.h).
    const hanja: boolean = KoreanCompositionPolicy.active(view.scheme, view.dedicated_english, view.local_mode);
    this.resolvePronunciations(view.candidates);
    const entries: CandidateEntry[] = [];
    for (let index: number = 0; index < view.candidates.length; index++) {
      const candidate: Candidate = view.candidates[index];
      if (!CandidatePresentationPolicy.isVisible(candidate.source)) {
        continue;
      }
      entries.push({
        text: this.asTraditional(candidate.text, candidate.source),
        highlighted: candidate.highlighted,
        badge: CandidatePresentationPolicy.badge(candidate.source),
        fixedPosition: candidate.fixed_position,
        source: candidate.source,
        translation: TranslationPolicy.sourceGetsNoGloss(candidate.source)
          ? null : candidate.translation ?? null,
        // Only the part of the code still untyped, and only where that means something. The policy
        // decides; this just supplies what it reasons about.
        hint: WubiCodeHintPolicy.hint(candidate.code,
          WubiCodeHintPolicy.typed(candidate.code, view.preedit, view.key_grid ?? "none", view.editing_text),
          this.wubiCodeHint, view.scheme,
          view.local_mode, view.answered_by_pinyin_fallback),
        annotation: CandidateGlossPolicy.annotation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
          this.candidateGlossEnabled),
        annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
          this.candidateGlossEnabled),
        hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation),
        confidentCorrection: CorrectionMarkPolicy.confident(candidate.confident_correction),
        pronunciation: this.candidatePronunciation(candidate)
      });
    }
    if (this.onChanged !== undefined) {
      this.onChanged({
        candidates: entries,
        // 已选的那一段领在读音前面，与来源把 word_for_creating_word 拼在读音前面是同一件事。
        editing: (view.phrase_prefix ?? '') + reading,
        editingCaret: KeyboardSession.drawnCaret(view, reading) + (view.phrase_prefix ?? '').length,
        phrasePrefix: view.phrase_prefix ?? '',
        compositionInField: this.compositionInField(),
        spellings: view.nine_key_spellings,
        scheme: view.scheme,
        shuangpinProfile: view.shuangpin_profile,
        localMode: view.local_mode,
        english: view.dedicated_english,
        diagnostic: this.diagnostic,
        page: view.page,
        pageCount: view.page_count,
        candidateListOpen: this.candidateListOpen(),
        nineKeySingleCharacter: view.nine_key_single_character === true,
        nineKeyStrokes: view.nine_key_strokes ?? '',
        cornerQuotes: ChineseSymbolFaces.cornerQuotes(view.punctuation_marks),
        hollowBrackets: ChineseSymbolFaces.hollowBrackets(view.punctuation_marks),
        enumerationCommaBackslash:
          ChineseSymbolFaces.enumerationCommaBackslash(view.punctuation_marks),
        enumerationCommaSlash: ChineseSymbolFaces.enumerationCommaSlash(view.punctuation_marks),
        prediction: NextWordPredictionPolicy.showing(view.prediction, entries.length)
      });
    }
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, reading, view.page, view.page_count,
        KeyboardSession.translationReserved(view)));
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
    // Every typed letter and every backspace arrives here, which is the one place that sees the editor change in direct English as well as in a composition.
    // 组字开始时收起后缀（候选栏归引擎），结束时按上屏后的上文重新匹配。
    this.refreshCaretSuggestions();
  }

  private publishCurrentCandidates(): void {
    const view: EngineView | undefined = this.lastView;
    if (view === undefined || this.onChanged === undefined) return;
    const hanja: boolean = KoreanCompositionPolicy.active(view.scheme, view.dedicated_english, view.local_mode);
    this.resolvePronunciations(view.candidates);
    const entries: CandidateEntry[] = [];
    for (const candidate of view.candidates) {
      if (!CandidatePresentationPolicy.isVisible(candidate.source)) {
        continue;
      }
      entries.push({
        text: this.asTraditional(candidate.text, candidate.source), highlighted: candidate.highlighted,
        badge: CandidatePresentationPolicy.badge(candidate.source),
        fixedPosition: candidate.fixed_position, source: candidate.source,
        translation: TranslationPolicy.sourceGetsNoGloss(candidate.source)
          ? null : candidate.translation ?? null,
        hint: WubiCodeHintPolicy.hint(candidate.code,
          WubiCodeHintPolicy.typed(candidate.code, view.preedit, view.key_grid ?? "none", view.editing_text),
          this.wubiCodeHint, view.scheme,
          view.local_mode, view.answered_by_pinyin_fallback),
        annotation: CandidateGlossPolicy.annotation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
          this.candidateGlossEnabled),
        annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
          CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
          TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
          this.candidateGlossEnabled),
        hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation),
        confidentCorrection: CorrectionMarkPolicy.confident(candidate.confident_correction),
        pronunciation: this.candidatePronunciation(candidate)
      });
    }
    const reading: string = KeyboardSession.drawnReading(view);
    this.onChanged({
      candidates: entries, editing: (view.phrase_prefix ?? '') + reading,
      editingCaret: KeyboardSession.drawnCaret(view, reading) + (view.phrase_prefix ?? '').length,
      phrasePrefix: view.phrase_prefix ?? '',
      compositionInField: this.compositionInField(),
      spellings: view.nine_key_spellings,
      scheme: view.scheme, shuangpinProfile: view.shuangpin_profile, localMode: view.local_mode,
      english: view.dedicated_english, diagnostic: this.diagnostic,
      page: view.page, pageCount: view.page_count, candidateListOpen: this.candidateListOpen(),
      nineKeySingleCharacter: view.nine_key_single_character === true,
      nineKeyStrokes: view.nine_key_strokes ?? '',
      cornerQuotes: ChineseSymbolFaces.cornerQuotes(view.punctuation_marks),
      hollowBrackets: ChineseSymbolFaces.hollowBrackets(view.punctuation_marks),
      enumerationCommaBackslash:
        ChineseSymbolFaces.enumerationCommaBackslash(view.punctuation_marks),
      enumerationCommaSlash: ChineseSymbolFaces.enumerationCommaSlash(view.punctuation_marks),
      prediction: NextWordPredictionPolicy.showing(view.prediction, entries.length)
    });
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, reading, view.page, view.page_count,
        KeyboardSession.translationReserved(view)));
    }
    if (this.onCandidateCount !== undefined) this.onCandidateCount(entries.length);
  }

  /** The composition the strip draws: for Korean, Zhuyin and Vietnamese editing_text is only the keys behind the composition, so there the written text itself is drawn. */
  private static drawnReading(view: EngineView): string {
    return SchemeCompositionPolicy.reading(
      SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode),
      view.editing_text, view.preedit, view.nine_key_reading ?? '');
  }

  /** The caret within drawnReading: the Engine's own for a spelling, the end of a composition the scheme keeps the caret at the end of (a Vietnamese caret_position is a byte offset into a word that is not ASCII, so it is never used as an index). */
  private static drawnCaret(view: EngineView, reading: string): number {
    const rulesScheme: number =
      SchemeCompositionPolicy.rulesScheme(view.scheme, view.dedicated_english, view.local_mode);
    return SchemeCompositionPolicy.caret(rulesScheme, view.caret_position, reading,
      SchemeCompositionPolicy.drawsNineKeyReading(rulesScheme, view.editing_text, view.nine_key_reading ?? ''));
  }

  /** 视图里有没有候选标了 `translation_reserved`，即「仅当前高亮」对这一页生效。 */
  private static translationReserved(view: EngineView): boolean {
    return view.candidates.some((candidate: Candidate): boolean => candidate.translation_reserved === true);
  }

  /** A horizontal candidate window on a machine with its own keys is as wide as its whole page; a vertical list is as wide as its widest row. A 2in1 window that shows the composition line also makes room on it for the brand mark and the page indicator. `reserved` 为真（「仅当前高亮」对这一页生效）时，同一页里移动高亮不让窗口变窄，见 `CandidateWidthPolicy.steadyWidthVp`。 */
  private candidateWidthVp(entries: CandidateEntry[], editing: string, page: number, pageCount: number,
    reserved: boolean): number {
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
    // 横排窗口和候选格使用同一份版式，序号缩为零时仍预留 9vp。
    let layoutExtra: number = 0;
    if (settings.candidateLayout !== 'vertical' && settings.candidateSkinLayout !== null
      && settings.candidateSkinLayout !== undefined) {
      const geometry = settings.candidateSkinLayout.horizontal;
      const numberSize: number = CandidateLayoutPolicy.numberSize(geometry, settings.candidateFontSize,
        CandidateNumberFontPolicy.size(settings.candidateFontSize));
      entries.forEach((entry: CandidateEntry, index: number): void => {
        const oldContent: number = CandidateWidthPolicy.chipContentVp(entry, index, settings.candidateFontSize,
          CandidateNumberFontPolicy.size(settings.candidateFontSize));
        const content: number = CandidateWidthPolicy.chipContentVp(entry, index, settings.candidateFontSize, numberSize)
          + Math.max(0, 9 - (numberSize > 0 ? CandidateWidthPolicy.textWidthVp(`${index + 1}`, numberSize) : -CandidateWidthPolicy.ORDINAL_GAP_VP));
        layoutExtra += CandidateLayoutPolicy.chipWidth(geometry, settings.candidateFontSize, content,
          KeyboardMetrics.CANDIDATE_PADDING_VP) - oldContent - 2 * KeyboardMetrics.CANDIDATE_PADDING_VP;
      });
      layoutExtra += CandidateLayoutPolicy.gap(settings.candidateSkinLayout, false, settings.candidateFontSize)
        * Math.max(0, entries.length - 1);
    }
    const width: number = settings.candidateShowPreedit
      ? CandidatePagerPolicy.panelWidthVp(base, editing, settings.candidatePreeditFontSize, page, pageCount,
        CandidateWidthPolicy.MAX_WIDTH_VP, CandidateLogoPolicy.slotWidthVp(
          CandidateLogoPolicy.visible(KeyboardFormFactor.isDesktop(), settings.candidateShowPreedit)))
      : base;
    const measured: number = Math.min(CandidateWidthPolicy.MAX_WIDTH_VP,
      Math.max(settings.candidateMinWidthVp, width + layoutExtra)
      + CandidateCanvasPolicy.extraWidth(settings.candidateCanvas));
    const pageKey: string = CandidateWidthPolicy.pageKey(entries, page);
    const steady: number = CandidateWidthPolicy.steadyWidthVp(measured, reserved,
      pageKey === this.candidateWidthPage, this.candidateWidthFloorVp);
    this.candidateWidthPage = pageKey;
    this.candidateWidthFloorVp = steady;
    return steady;
  }

  private publishTranslationPage(): void {
    const translations: string[] | null = this.translationPage;
    if (translations === null || this.onChanged === undefined) return;
    const view: EngineView | undefined = this.lastView;
    const entries: CandidateEntry[] = translations.map((text: string, index: number): CandidateEntry => ({
      text: text, hint: '', annotation: '', annotationIsTranslation: false, hunEum: '',
      highlighted: index === this.translationHighlight, badge: '',
      fixedPosition: 0, source: 0, translation: null, confidentCorrection: false, pronunciation: ''
    }));
    this.onChanged({
      candidates: entries,
      editing: view?.editing_text ?? this.editing,
      editingCaret: view?.caret_position ?? this.editingCaret, phrasePrefix: '', spellings: [],
      compositionInField: this.compositionInField(),
      scheme: view?.scheme ?? this.scheme, shuangpinProfile: view?.shuangpin_profile ?? this.profileName,
      localMode: view?.local_mode ?? this.localMode,
      english: view?.dedicated_english ?? this.englishMode, diagnostic: this.diagnostic,
      page: 0, pageCount: 1, candidateListOpen: false,
      nineKeySingleCharacter: view?.nine_key_single_character === true,
      nineKeyStrokes: view?.nine_key_strokes ?? '',
      cornerQuotes: ChineseSymbolFaces.cornerQuotes(view?.punctuation_marks),
      hollowBrackets: ChineseSymbolFaces.hollowBrackets(view?.punctuation_marks),
      enumerationCommaBackslash:
        ChineseSymbolFaces.enumerationCommaBackslash(view?.punctuation_marks),
      enumerationCommaSlash: ChineseSymbolFaces.enumerationCommaSlash(view?.punctuation_marks),
      // 译文页是某个组字候选的几种译法，不是上屏后联想。
      prediction: false
    });
    if (this.onCandidateWidth !== undefined) {
      this.onCandidateWidth(this.candidateWidthVp(entries, view?.editing_text ?? this.editing, 0, 1, false));
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
      this.invalidateOnlineProviders();
      return;
    }
    const query: OnlineQuery | null = NativeReplyPolicy.successfulValue<OnlineQuery>(reply);
    if (query === null) {
      this.aiResolvedSignature = '';
      this.invalidateOnlineProviders();
      return;
    }
    const signature: string = OnlineCandidatePolicy.signature(query);
    if (signature === this.onlineSignature) return;
    if (signature !== this.aiResolvedSignature) this.aiResolvedSignature = '';
    this.invalidateOnlineProviders();
    this.onlineSignature = signature;
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
          const currentValue: OnlineQuery | null = NativeReplyPolicy.successfulValue(current);
          if (currentValue !== null) {
            aiQuery = currentValue;
            aiDocument = JSON.stringify(currentValue);
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
    // AI 联想把组字和光标前的文字发给大模型；总闸 `AiGenerationGate` 关着时存下的 `ai_assistant.enabled` 打开了也不问。
    if (assistant?.enabled === true && aiQuery.ai_eligible && AiGenerationGate.enabled()) {
      // 这个组字问过 AI 而没有结果（云候选失败触发的重试走到这里）：不再发请求。
      const aiApplied: boolean = requestSignature !== this.aiResolvedSignature
        && await this.fetchAi(handle, aiDocument, assistant.candidate_limit, epoch);
      // 向 AI 发问了却没有采用任何回答（请求发不出去、失败、空回答、被拒）：告诉 Host API 这次没有回答；同一组字不再问，所以不为 AI 释放签名，只有云候选失败才重试（#6888）。
      if (!aiApplied) {
        this.resolveAiSeat(handle, aiDocument, epoch);
        if (epoch === this.onlineEpoch && handle === this.handle) this.aiResolvedSignature = requestSignature;
      }
    }
    if (!complete) this.releaseOnlineSignatureIfCurrent(requestSignature, epoch, handle);
  }

  private releaseOnlineSignatureIfCurrent(requestSignature: string, requestEpoch: number,
    requestHandle: number): void {
    if (OnlineCandidatePolicy.shouldReleaseAfterFailure(requestSignature, this.onlineSignature,
      requestEpoch, this.onlineEpoch, requestHandle, this.handle)) this.onlineSignature = '';
  }

  private async fetchCloud(handle: number, queryDocument: string, epoch: number): Promise<boolean> {
    const query: OnlineQuery = JSON.parse(queryDocument) as OnlineQuery;
    const descriptor: CloudHttpDescriptor | null | undefined = query.cloud_request;
    if (descriptor === null || descriptor === undefined || !AiEndpoint.allowed(descriptor.url)
      || (descriptor.method !== 'GET' && descriptor.method !== 'POST')
      || descriptor.timeout_ms < 100 || descriptor.timeout_ms > 10000
      || descriptor.max_response_bytes !== OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES) return false;
    if (descriptor.cached_response !== undefined) {
      if (epoch !== this.onlineEpoch || handle !== this.handle) return false;
      return this.applyCloudBody(handle, queryDocument, descriptor.cached_response);
    }
    // 云联想把正在组的拼音发出去，没有隐私政策的授权就不发。
    if (!HarmonyPrivacyConsent.networkAllowed()) return false;
    const request = http.createHttp();
    installNoRedirectGuard(request);
    this.onlineRequests.add(request);
    const deadline: number = setTimeout((): void => { request.destroy(); }, descriptor.timeout_ms);
    try {
      const response: http.HttpResponse = await request.request(descriptor.url, {
        method: descriptor.method === 'POST' ? http.RequestMethod.POST : http.RequestMethod.GET,
        header: descriptor.headers,
        connectTimeout: descriptor.connect_timeout_ms,
        readTimeout: descriptor.timeout_ms,
        maxLimit: descriptor.max_response_bytes,
        extraData: descriptor.method === 'POST' ? descriptor.body_utf8 : undefined
      });
      if (epoch !== this.onlineEpoch || handle !== this.handle || response.responseCode < 200
        || response.responseCode >= 300 || typeof response.result !== 'string'
        || !OnlineCandidatePolicy.acceptsCloudBody(response.result)) return false;
      return this.applyCloudBody(handle, queryDocument, response.result);
    } catch (error) {
      KeyboardLog.error('cloud candidate unavailable');
      return false;
    } finally {
      clearTimeout(deadline);
      this.onlineRequests.delete(request);
      request.destroy();
    }
  }

  private applyCloudBody(handle: number, queryDocument: string, body: string): boolean {
    if (!OnlineCandidatePolicy.acceptsCloudBody(body)) return false;
    const applied: NativeReply<OnlineApplyValue> = JSON.parse(client.applyCloudResponse(
      handle, queryDocument, body)) as NativeReply<OnlineApplyValue>;
    if (applied.ok === true && NativeReplyPolicy.hasValue(applied.value)
      && applied.value.applied) this.render(applied.value.view);
    return applied.ok === true && NativeReplyPolicy.hasValue(applied.value) && applied.value.applied;
  }

  /** 告诉 Host API 这次 AI 没有回答。共享内核已经不放 AI 占位行，这个调用答 applied:false、不改候选列表；调用和接口留着不变；万一答了 applied:true 才重画。 */
  private resolveAiSeat(handle: number, queryDocument: string, epoch: number): void {
    if (epoch !== this.onlineEpoch || handle !== this.handle) return;
    try {
      const resolved: NativeReply<OnlineApplyValue> = JSON.parse(client.resolveOnlineCandidates(
        handle, queryDocument, 1)) as NativeReply<OnlineApplyValue>;
      if (resolved.ok === true && NativeReplyPolicy.hasValue(resolved.value)
        && resolved.value.applied) this.render(resolved.value.view);
    } catch (error) {
      KeyboardLog.error(`AI seat unresolved: ${describeError(error)}`);
    }
  }

  private async fetchAi(handle: number, queryDocument: string, limit: number,
    epoch: number): Promise<boolean> {
    // AI 联想把上下文发给用户配置的服务，没有隐私政策的授权就不发。
    if (!HarmonyPrivacyConsent.networkAllowed()) return false;
    let descriptor: AiHttpDescriptor | null;
    try {
      const reply: NativeReply<AiHttpDescriptor | null> = JSON.parse(
        client.aiRequestForQuery(handle, queryDocument)) as NativeReply<AiHttpDescriptor | null>;
      if (reply.ok !== true || !NativeReplyPolicy.hasValue(reply.value)) return false;
      descriptor = reply.value;
    } catch (error) {
      KeyboardLog.error(`AI query unavailable: ${describeError(error)}`);
      return false;
    }
    // 描述符由 client-providers 按同一规则生成，这里在发出前再经 Rust 拦一次：http 只能发往本机或局域网，Token 不明文经过互联网。
    if (typeof descriptor.url !== 'string' || !AiEndpoint.allowed(descriptor.url)) return false;
    const request = http.createHttp();
    installNoRedirectGuard(request);
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
      if (applied.ok === true && NativeReplyPolicy.hasValue(applied.value)
        && applied.value.applied) this.render(applied.value.view);
      return applied.ok === true && NativeReplyPolicy.hasValue(applied.value)
        && applied.value.applied;
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
      this.invalidateTranslationProviders();
      return;
    }
    const query: TranslationQuery | null = NativeReplyPolicy.successfulValue<TranslationQuery>(reply);
    if (query === null) {
      this.invalidateTranslationProviders();
      return;
    }
    if (!KeyboardFormFactor.isDesktop()) TranslationPolicy.keepPrimaryOnly(query);
    const signature: string = TranslationPolicy.signature(query);
    if (signature === this.translationSignature) return;
    this.invalidateTranslationProviders();
    this.translationSignature = signature;
    const epoch: number = ++this.translationEpoch;
    const handle: number = this.handle;
    const requestSignature: string = signature;
    // 本地词典先显示，不等待网络防抖或服务返回；在线阶段复用本次查词结果。
    const english: TranslationEntry[] = [];
    const offline: Map<string, TranslationEntry[]> = this.offlineTargetGlosses(query);
    const local: TranslationEntry[] = this.localTranslations(query, english, offline);
    // 「仅当前高亮」时 query 只问高亮的候选，整页的离线释义在 gloss_query 里：走同一条离线路径（它不带服务，不联网），回答标 gloss 后和主查询的回答一起交给运行时。
    const glossQuery: TranslationQuery | null = query.gloss_query ?? null;
    if (glossQuery !== null) {
      const glossEnglish: TranslationEntry[] = [];
      const glosses: TranslationEntry[] = this.localTranslations(glossQuery, glossEnglish,
        this.offlineTargetGlosses(glossQuery));
      for (const entry of TranslationPolicy.glossAnswers(glosses)) local.push(entry);
    }
    this.applyTranslationEntries(handle, query.generation, local);
    if (!this.translationCurrent(handle, epoch) || TranslationPolicy.provider(query).length === 0) return;
    this.translationTimer = setTimeout((): void => {
      this.translationTimer = -1;
      void this.fetchTranslations(handle, query, epoch, requestSignature, english, offline);
    }, TranslationPolicy.QUIET_INTERVAL_MS);
  }

  /** 一份查询的本地结果：离线英文释义收进 `english`，再按目标语言顺序并上 `offline` 里其他语言的离线词典结果。不联网，`english` 和 `offline` 留给在线阶段复用。 */
  private localTranslations(query: TranslationQuery, english: TranslationEntry[],
    offline: Map<string, TranslationEntry[]>): TranslationEntry[] {
    if (query.english_gloss && typeof query.resources === 'string' && query.resources.length > 0) {
      this.appendOfflineGloss(query, english);
    }
    const local: TranslationEntry[] = english.slice();
    for (const target of TranslationPolicy.targets(query)) {
      for (const entry of offline.get(target) ?? []) {
        TranslationPolicy.append(local, entry.text, entry.translation);
      }
    }
    return local;
  }

  /** 本地和在线结果共用代次校验与重绘入口，空结果不清空已经显示的释义。 */
  private applyTranslationEntries(handle: number, generation: number, entries: TranslationEntry[]): boolean {
    if (entries.length === 0) return false;
    try {
      const applied: NativeReply<OnlineApplyValue> = JSON.parse(client.applyTranslations(
        handle, generation, JSON.stringify(entries))) as NativeReply<OnlineApplyValue>;
      const value: OnlineApplyValue | null = NativeReplyPolicy.successfulValue(applied);
      if (value === null || value.view === undefined || value.applied !== true) return false;
      this.render(value.view);
      return true;
    } catch (error) {
      KeyboardLog.error(`candidate translations unavailable: ${describeError(error)}`);
      return false;
    }
  }

  private async fetchTranslations(handle: number, query: TranslationQuery, epoch: number,
    requestSignature: string, english: TranslationEntry[],
    offline: Map<string, TranslationEntry[]>): Promise<void> {
    if (!this.translationCurrent(handle, epoch)) return;
    const entries: TranslationEntry[] = english.slice();
    let providerComplete: boolean = true;
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
      appliedCurrent = this.applyTranslationEntries(handle, query.generation, entries);
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
        if (planReply.ok === true && Array.isArray(planReply.value)) plan = planReply.value;
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
        } else if (provider === 'ai' && query.ai_assistant !== null
          && query.ai_assistant !== undefined) {
          const result: TranslationProviderResult = await this.fetchAiTranslation(
            query.ai_assistant, item, epoch, handle);
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
            text: candidate.text, source: candidate.source
          })),
          target_language: language
        };
        const reply: NativeReply<TranslationResponseValue> = JSON.parse(
          client.candidateGlosses(JSON.stringify(request), resources)) as NativeReply<TranslationResponseValue>;
        const replyValue: TranslationResponseValue | null = NativeReplyPolicy.successfulValue(reply);
        if (replyValue === null || replyValue.generation !== query.generation
          || !Array.isArray(replyValue.translations)) continue;
        const glosses: TranslationEntry[] = [];
        for (const entry of replyValue.translations) {
          TranslationPolicy.append(glosses, entry.text, entry.translation);
        }
        result.set(language, glosses);
      } catch (error) {
        KeyboardLog.error(`offline ${language} candidate glosses unavailable: ${describeError(error)}`);
      }
    }
    return result;
  }

  /** 只传当前笔画到原生工作线程；不截图、不联网、不保留文字或笔画日志。 */
  async recognizeHandwriting(strokes: HandwritingStroke[], canvasWidth: number,
    canvasHeight: number): Promise<string[]> {
    const files: string = this.stateDirectory.substring(0, this.stateDirectory.lastIndexOf('/'));
    const width: number = Number.isFinite(canvasWidth) && canvasWidth > 0
      ? canvasWidth : HANDWRITING_CANVAS_SIZE;
    const height: number = Number.isFinite(canvasHeight) && canvasHeight > 0
      ? canvasHeight : HANDWRITING_CANVAS_SIZE;
    const recognitionStrokes: HandwritingRecognitionStroke[] = [];
    for (const stroke of strokes) {
      recognitionStrokes.push({ points: stroke.points.map((point: HandwritingPoint): HandwritingPoint =>
        HandwritingStrokePolicy.recognitionPoint(point.x, point.y, width, height)) });
    }
    const request: HandwritingNativeRequest = {
      model: `${files}/handwriting/handwriting-zh_CN.model`,
      request: { language: 'zh-CN', strokes: recognitionStrokes }
    };
    const reply: HandwritingNativeReply = JSON.parse(await client.handwriting(JSON.stringify(request))) as HandwritingNativeReply;
    if (!reply.ok || reply.value === undefined) {
      throw new Error(reply.error ?? 'handwriting_unavailable');
    }
    return HandwritingStrokePolicy.candidates(reply.value.candidates.join(''));
  }

  /** Whether the Cantonese or Zhuyin dictionary `file` is staged in `language-dictionaries` beside the resources, which is where host preparation looks for it. */
  private languageDictionaryInstalled(file: string): boolean {
    try {
      const parent: string = this.resourceDirectory.substring(0, this.resourceDirectory.lastIndexOf('/'));
      return parent.length > 0 && fs.accessSync(`${parent}/language-dictionaries/${file}`);
    } catch (error) {
      KeyboardLog.error('keyboard.KeyboardSession.languageDictionaryInstalled failed');
      return false;
    }
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
          text: candidate.text, source: candidate.source
        }))
      };
      const glossResponse: string = client.candidateGlosses(
        JSON.stringify(request), query.resources ?? '');
      const reply: NativeReply<TranslationResponseValue> =
        JSON.parse(glossResponse) as NativeReply<TranslationResponseValue>;
      const replyValue: TranslationResponseValue | null = NativeReplyPolicy.successfulValue(reply);
      if (replyValue === null || replyValue.generation !== query.generation
        || !Array.isArray(replyValue.translations)) return;
      for (const entry of replyValue.translations) {
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
        descriptor = NativeReplyPolicy.successfulValue(reply);
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
        if (parsed.ok !== true || parsed.value === null || parsed.value.length !== group.length) {
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
      const value: string | null = NativeReplyPolicy.successfulValue(reply);
      return { value: value, completed: value !== null };
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
      const value: string | null = NativeReplyPolicy.successfulValue(reply);
      return { value: value, completed: value !== null };
    } catch {
      return { value: null, completed: false };
    }
  }

  private async fetchAiTranslation(config: AiTranslationConfig, item: TranslationPlanItem,
    epoch: number, handle: number): Promise<TranslationProviderResult> {
    const request: AiTranslationRequest = {
      config: config,
      input: {
        text: item.key,
        source_language: item.source_language,
        target_language: item.target_language
      }
    };
    const descriptor: TranslationHttpDescriptor | null = this.translationDescriptor(
      client.aiTranslationHttpRequest(JSON.stringify(request)));
    if (descriptor === null) return { value: null, completed: false };
    const body: string | null = await this.translationHttp(descriptor, false, epoch, handle);
    if (body === null) return { value: null, completed: false };
    try {
      const reply: NativeReply<string | null> = JSON.parse(
        client.parseAiTranslationResponse(body)) as NativeReply<string | null>;
      const value: string | null = NativeReplyPolicy.successfulValue(reply);
      return { value: value, completed: value !== null };
    } catch {
      return { value: null, completed: false };
    }
  }

  private recordTranslation(query: TranslationQuery, target: string, item: TranslationPlanItem,
    value: string | null, entries: TranslationEntry[]): void {
    const key: string = TranslationPolicy.cacheKey(query, target, item);
    // A `/fy` answer is the command's row, not a candidate gloss: it is neither cached nor remembered as a failure.
    if (query.sentence === true) {
      if (value !== null) TranslationPolicy.append(entries, item.text, value);
      return;
    }
    if (value === null) {
      if (!this.translationNegativeCache.has(key)
        && TranslationPolicy.shouldResetCache(this.translationNegativeCache.size)) {
        this.translationNegativeCache.clear();
      }
      this.translationNegativeCache.set(key, Date.now() + TranslationPolicy.NEGATIVE_CACHE_MS);
      return;
    }
    const normalised: string = TranslationPolicy.normalise(value);
    if (normalised.length === 0) {
      if (!this.translationNegativeCache.has(key)
        && TranslationPolicy.shouldResetCache(this.translationNegativeCache.size)) {
        this.translationNegativeCache.clear();
      }
      this.translationNegativeCache.set(key, Date.now() + TranslationPolicy.NEGATIVE_CACHE_MS);
      return;
    }
    if (!this.translationCache.has(key)
      && TranslationPolicy.shouldResetCache(this.translationCache.size)) {
      this.translationCache.clear();
    }
    this.translationCache.set(key, normalised);
    this.translationNegativeCache.delete(key);
    TranslationPolicy.append(entries, item.text, normalised);
  }

  private translationDescriptor(raw: string): TranslationHttpDescriptor | null {
    try {
      const reply: NativeReply<TranslationHttpDescriptor | null> =
        JSON.parse(raw) as NativeReply<TranslationHttpDescriptor | null>;
      return reply.ok === true && NativeReplyPolicy.hasValue(reply.value) ? reply.value : null;
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
    // 在线翻译把候选发给翻译服务，没有隐私政策的授权就不发，候选照常显示本机释义。
    if (!HarmonyPrivacyConsent.networkAllowed()) return null;
    const request = http.createHttp();
    installNoRedirectGuard(request);
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
      if (!KeyboardSession.safeStateDirectory(stateRoot)
        || KeyboardSession.safePrivateFile(path, KeyboardFeedback.MAX_BYTES) === null) {
        return KeyboardFeedback.DEFAULTS;
      }
      const text: string | null = KeyboardSession.readPrivateText(path, KeyboardFeedback.MAX_BYTES);
      return text === null ? KeyboardFeedback.DEFAULTS : KeyboardFeedback.parse(text);
    } catch (error) {
      KeyboardLog.error(`key feedback unreadable: ${describeError(error)}`);
      return KeyboardFeedback.DEFAULTS;
    }
  }

  /** `prepareHost` 的请求。full 不带版本，与引入版本之前相同；其他版本让 host-api 按本版本收窄方案、按本版本的资源锁校验词库。 */
  static prepareRequest(resources: string, stateRoot: string): PrepareRequest {
    const request: PrepareRequest = { resources: resources, state_root: stateRoot };
    const edition: AppEdition = AppEdition.current();
    if (!edition.isFull()) {
      request.edition = edition.id;
    }
    return request;
  }

  static saveFeedbackIn(stateRoot: string, settings: FeedbackSettings): boolean {
    try {
      const path: string = KeyboardSession.feedbackFileIn(stateRoot);
      if (!KeyboardSession.safeStateDirectory(stateRoot)) {
        return false;
      }
      return KeyboardSession.writePrivateText(path, KeyboardFeedback.serialize(settings));
    } catch (error) {
      KeyboardLog.error(`key feedback not saved: ${describeError(error)}`);
      return false;
    }
  }

  /** 会话持有的设备本地设置（含手写延迟和笔迹），不读磁盘：即上次编辑框获得焦点或启动时读到的值，或此后保存的值。 */
  feedbackSettings(): FeedbackSettings {
    return this.feedbackCurrent;
  }

  /**
   * 从功能面板开关「隐私模式」：把它和其他设备本地设置存在一起，然后重建会话，使会话的学习状态和私密标记与之一致，与 Android 的 `restartSessionForPrivacy` 相同（运行中的会话不会自行接受学习开关的变化）。返回隐私模式最终是否开启；保存失败时保持原状。
   */
  setPrivacyMode(on: boolean): boolean {
    const current: FeedbackSettings = this.feedbackCurrent;
    if (current.incognito === on) {
      return on;
    }
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
      oneHanded: current.oneHanded,
      incognito: on,
      floating: current.floating,
      floatingX: current.floatingX,
      floatingY: current.floatingY,
      bottomLift: current.bottomLift,
      widthPercent: current.widthPercent,
      positionX: current.positionX
    };
    if (!this.saveFeedback(next)) {
      return current.incognito;
    }
    this.followPrivacyMode();
    return on;
  }

  /** 功能面板「浮动键盘」磁贴和拖动条的停靠键：存下开关（设备本地，不同步），再让扩展能力换面板。保存失败时保持原状，返回最终是否浮动。 */
  setFloatingKeyboard(on: boolean): boolean {
    const current: FeedbackSettings = this.feedbackCurrent;
    if (current.floating === on) {
      return on;
    }
    if (!this.saveFeedback(KeyboardFeedback.withFloating(current, on, current.floatingX, current.floatingY))) {
      return current.floating;
    }
    if (this.onFloatingKeyboardChanged !== undefined) {
      this.onFloatingKeyboardChanged();
    }
    return on;
  }

  /** 键盘调整确认或以别的方式离开时存下底部抬高、宽度和水平位置（设备本地，不同步）。三项都没变时不写盘。返回是否已存下。 */
  saveKeyboardAdjust(bottomLift: number, widthPercent: number, positionX: number): boolean {
    const current: FeedbackSettings = this.feedbackCurrent;
    const next: FeedbackSettings = KeyboardFeedback.withAdjust(current, bottomLift, widthPercent, positionX);
    if (next.bottomLift === current.bottomLift && next.widthPercent === current.widthPercent
      && next.positionX === current.positionX) {
      return true;
    }
    if (!this.saveFeedback(next)) {
      KeyboardLog.error('keyboard adjustment not saved');
      return false;
    }
    return true;
  }

  /** 拖动松手后存下浮动面板的位置（千分比）。写不进去时位置只在这次键盘进程里有效，不打扰用户，与 Android 相同。 */
  saveFloatingPosition(xFraction: number, yFraction: number): void {
    const current: FeedbackSettings = this.feedbackCurrent;
    this.saveFeedback(KeyboardFeedback.withFloating(current, current.floating, xFraction, yFraction));
  }

  beginFloatingDrag(): void {
    if (this.onFloatingDragStart !== undefined) {
      this.onFloatingDragStart();
    }
  }

  dragFloating(offsetX: number, offsetY: number): void {
    if (this.onFloatingDrag !== undefined) {
      this.onFloatingDrag(offsetX, offsetY);
    }
  }

  endFloatingDrag(): void {
    if (this.onFloatingDragEnd !== undefined) {
      this.onFloatingDragEnd();
    }
  }

  /**
   * 隐私模式与会话构建时不再相符时重建会话。先提交正在进行的组字，与 Android 在重启前先完成组字的做法一样，这样已输入的内容不会因重建而丢失；处于本地模式时留待其结束，由下一次获得焦点时补上。
   */
  private followPrivacyMode(): void {
    if (this.handle === 0 || this.sessionPrivate === this.feedbackCurrent.incognito
      || this.localMode !== 'none') {
      return;
    }
    if (this.composing()) {
      this.finishComposition();
    }
    this.restartIdleSession('privacy mode');
  }

  /** 为 `host-api` 把新会话标记为私密（或取消标记），`host-api` 随后不再为它统计候选位置。 */
  private markPrivateSession(enabled: boolean): void {
    this.sessionPrivate = enabled;
    if (!enabled) {
      return;
    }
    const reply: FlagResponse = JSON.parse(client.setPrivateSession(this.handle, true)) as FlagResponse;
    if (reply.ok !== true) {
      KeyboardLog.error(`private session refused: ${reply.error}`);
    }
  }

  /** 当前获得焦点的编辑框此刻是否允许保留这类记录（`PrivacyGate`）。 */
  private privacyAllows(record: PrivacyRecord): boolean {
    return PrivacyGate.allows(record, this.feedbackCurrent.incognito, this.traits.password);
  }

  /** 「保存」此刻能否把系统剪贴板放进历史：不在隐私模式下，也不在密码框里。历史自身的开关是 `clipboardHistoryAllowed`。 */
  clipboardCaptureAllowed(): boolean {
    return this.privacyAllows(PrivacyRecord.CLIPBOARD_HISTORY);
  }

  /** 读取这台设备的按键反馈等本地设置（共享偏好不携带它们）：文件不存在时为默认值；文件存在但这次读不到（例如设置应用正以写后改名替换它）时返回 `null`，由调用方保留上次读到的值（`KeyboardFeedback.fromRead`）。 */
  private readFeedback(): FeedbackSettings | null {
    const path: string = this.feedbackFile();
    try {
      const exists: boolean = fs.accessSync(path);
      return KeyboardFeedback.fromRead(exists,
        exists ? KeyboardSession.readPrivateText(path, KeyboardFeedback.MAX_BYTES) : null);
    } catch (error) {
      KeyboardLog.error(`key feedback unreadable: ${describeError(error)}`);
      return null;
    }
  }

  /** 重新读取设备本地设置并存入会话；这次读不到时保留上次的值，一次读取失败不能把隐私模式当成关闭。 */
  private refreshFeedback(): void {
    const fresh: FeedbackSettings | null = this.readFeedback();
    if (fresh !== null) {
      this.feedbackCurrent = fresh;
      this.configureSounds();
    }
  }

  /** 按当前偏好和本机按键音开关配置声音播放器：2in1 按共享插件设置播实体键的声音，手机和平板按 `key-feedback.json` 的开关播触屏按键音（`KeySoundPolicy.touchSettings`）。只有换了包才会重新加载。 */
  private configureSounds(): void {
    this.soundPlayer.configure(KeyboardFormFactor.isDesktop()
      ? KeySoundPolicy.settings(this.soundPlugins)
      : KeySoundPolicy.touchSettings(this.soundPlugins, this.feedbackCurrent.sound), this.stateDirectory);
  }

  /**
   * 触屏上的一次按键：记入按键统计（`recordKey`），开着按键音时按键类播放声音包里的样本。视图里每个软键每次按下调一次，按住删除连删也只算一次。2in1 的实体键在 `hardwareKeySound` 里另外出声。
   */
  softKeyPress(id: string | null): void {
    this.recordKey(id);
    if (!KeyboardFormFactor.isDesktop() && this.feedbackCurrent.sound) {
      this.soundPlayer.key(KeySoundPolicy.touchKeyClass(id, this.soundsSilencedByEditor()));
    }
  }

  saveFeedback(settings: FeedbackSettings): boolean {
    try {
      const path: string = this.feedbackFile();
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)) {
        return false;
      }
      if (!KeyboardSession.writePrivateText(path, KeyboardFeedback.serialize(settings))) {
        return false;
      }
      this.feedbackCurrent = settings;
      this.configureSounds();
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
  /** 长按工具栏按钮时的那一下按键反馈，与 Android 长按工具栏打开编辑页时一样；九键滑动输入数字抬手时也用它补上点按会有的那一下振动（数字走字面上屏，`insert` 不振动）。 */
  holdFeedback(): void {
    this.tapFeedback();
  }

  private tapFeedback(): void {
    const settings: FeedbackSettings = this.feedbackCurrent;
    if (!settings.haptics || this.vibratorFailed || this.vibrationPending) {
      return;
    }
    this.vibrationPending = true;
    try {
      KeyboardSession.vibrate(settings.strength).then((): void => {
        this.vibrationPending = false;
      }).catch((error: Object): void => {
        this.vibrationPending = false;
        this.vibratorFailed = true;
        KeyboardLog.error(`vibration unavailable: ${describeError(error)}`);
      });
    } catch (error) {
      this.vibrationPending = false;
      this.vibratorFailed = true;
      KeyboardLog.error(`vibration unavailable: ${describeError(error)}`);
    }
  }

  /** 每个预置效果设备支不支持，查一次记住：按键时不再每下都问系统。 */
  private static readonly effectSupport: Map<string, boolean> = new Map<string, boolean>();

  private static supportsEffect(effectId: string): boolean {
    const known: boolean | undefined = KeyboardSession.effectSupport.get(effectId);
    if (known !== undefined) {
      return known;
    }
    let supported: boolean = false;
    try {
      supported = vibrator.isSupportEffectSync(effectId);
    } catch (error) {
      KeyboardLog.error(`vibration effect ${effectId} unavailable: ${describeError(error)}`);
    }
    KeyboardSession.effectSupport.set(effectId, supported);
    return supported;
  }

  /** 按键振动在 TaskPool 线程上发（`startKeyVibration`）；那条路失败过一次就改回在 UI 线程上发，至少还能振。 */
  private static vibratesOnWorker: boolean = true;

  /**
   * 按一档振一下，按键和设置页的试听共用。档位怎么振见 `KeyboardFeedback.plan`：设备支持预置效果就用效果加强度，不支持就按时长振；「跟随系统」用 `touch` 用途，由系统的触感反馈设置决定振不振、多强，不带自定义强度。
   *
   * 振动请求在 TaskPool 线程上发出，理由见 `KeyVibration.ets`。效果支不支持仍在这里查：每个效果只查一次，结果记在 UI 线程这一侧。
   */
  static vibrate(strength: HapticStrength): Promise<void> {
    const plan: HapticPlan = KeyboardFeedback.plan(strength);
    if (!KeyboardSession.vibratesOnWorker) {
      return KeyboardSession.vibrateHere(plan);
    }
    const effectId: string = KeyboardSession.supportsEffect(plan.effectId) ? plan.effectId : '';
    let task: Promise<Object>;
    try {
      // 高优先级：TaskPool 里还跑着打字统计这类要拿文件锁、整份重写的任务，振动排在它们后面就晚了，晚到的振动比不振更乱手感。
      task = taskpool.execute(new taskpool.Task(startKeyVibration, effectId, plan.intensity, plan.fallbackMillis,
        plan.followsSystem), taskpool.Priority.HIGH);
    } catch (error) {
      return KeyboardSession.workerVibrationFailed(plan, error as Object);
    }
    return task.then((): void => {}, (error: Object): Promise<void> => {
      return KeyboardSession.workerVibrationFailed(plan, error);
    });
  }

  private static workerVibrationFailed(plan: HapticPlan, error: Object): Promise<void> {
    KeyboardSession.vibratesOnWorker = false;
    KeyboardLog.error(`vibration off the UI thread unavailable: ${describeError(error)}`);
    return KeyboardSession.vibrateHere(plan);
  }

  private static vibrateHere(plan: HapticPlan): Promise<void> {
    try {
      const attribute: vibrator.VibrateAttribute = { usage: plan.followsSystem ? 'touch' : 'physicalFeedback' };
      if (!KeyboardSession.supportsEffect(plan.effectId)) {
        const timed: vibrator.VibrateTime = { type: 'time', duration: plan.fallbackMillis };
        return vibrator.startVibration(timed, attribute);
      }
      const preset: vibrator.VibratePreset = { type: 'preset', effectId: plan.effectId, count: 1 };
      if (plan.intensity > 0) {
        preset.intensity = plan.intensity;
      }
      return vibrator.startVibration(preset, attribute);
    } catch (error) {
      KeyboardLog.error('keyboard.KeyboardSession.vibrateHere failed');
      throw error as Error;
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
      directory: directory, preferences_directory: this.stateDirectory, action: action
    };
    try {
      const reply: MobileClipboardReply = JSON.parse(
        client.mobileClipboardHistory(JSON.stringify(request))) as MobileClipboardReply;
      if (reply.ok !== true || reply.value === undefined || reply.value === null
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
    if (!this.clipboardHistoryAllowed() || !this.clipboardCaptureAllowed()) {
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

  /** 当前获得焦点的编辑框能否看到云剪贴板条目：密码框永远不能，属性尚未得知的编辑框也不能，隐私模式开启时一概不能。 */
  cloudClipboardVisible(): boolean {
    return this.inputClient !== undefined
      && CloudClipboardPolicy.editorAllows(this.editorAttributesKnown, this.traits.password)
      && this.privacyAllows(PrivacyRecord.CLOUD_CLIPBOARD);
  }

  /**
   * The account's cloud clipboard, fetched now.
   *
   * Null when the listing no longer belongs to the focused editor: the field changed while the request was out, or it is one that may not see cloud items. A late answer for another field is dropped rather than shown, since tapping one of its items would paste into a field it was never fetched for.
   */
  async cloudClipboardItems(): Promise<CloudClipboardBoundListing | null> {
    const editor: number = this.attributeGeneration;
    if (!this.cloudClipboardVisible()) return null;
    const client: CloudClipboardClient = this.cloudClipboard();
    const listing: CloudClipboardBoundListing | null = await client.list();
    if (listing === null || (listing.binding !== null && !(await client.matches(listing.binding)))) return null;
    if (!CloudClipboardPolicy.current(editor, this.attributeGeneration) || !this.cloudClipboardVisible()) {
      return null;
    }
    return listing;
  }

  async cloudClipboardBindingCurrent(binding: string): Promise<boolean> {
    return await this.cloudClipboard().matches(binding);
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
  async sendToCloudClipboard(text: string, binding: string): Promise<CloudClipboardSendOutcome> {
    // 面板只在云端那一半可见时提供「发到云剪贴板」；如果隐私模式在提供之后才开启，这里守住这条线。
    if (!this.cloudClipboardVisible()) {
      return CloudClipboardSendOutcome.FAILED;
    }
    return await this.cloudClipboard().add(text, binding);
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
      if (KeyboardSession.safePrivateFile(file, EMOJI_RECENTS_MAX_BYTES) === null) {
        return [];
      }
      const text: string | null = KeyboardSession.readPrivateText(file, EMOJI_RECENTS_MAX_BYTES);
      return text === null ? [] : EmojiCatalogModel.parseRecents(text);
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
      const path: string = this.emojiRecentsFile();
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)) {
        return recents;
      }
      KeyboardSession.writePrivateText(path, EmojiCatalogModel.serializeRecents(recents));
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
      const path: string = this.emojiRecentsFile();
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)) {
        return false;
      }
      return KeyboardSession.writePrivateText(path, '[]');
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
    // KeyboardExtensionAbility passes files/state as stateDirectory, and the settings app writes its explicit library one level above, at files/CommunityLibrary.json. From API 12 the input method extension has its own sandbox (see platforms/harmony/README.md), so this is the settings app's file only once both sides use the shared group directory; until then the keyboard's own files/ holds no library and no template is offered.
    const file: string = `${this.stateDirectory}/../CommunityLibrary.json`;
    try {
      const parent: string = this.stateDirectory.substring(0, this.stateDirectory.lastIndexOf('/'));
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory)
        || !KeyboardSession.safeStateDirectory(parent)
        || KeyboardSession.safePrivateFile(file, CommunityReplyLibraryPolicy.MAX_BYTES) === null) {
        return [];
      }
      const text: string | null = KeyboardSession.readPrivateText(
        file, CommunityReplyLibraryPolicy.MAX_BYTES);
      return text === null ? [] : CommunityReplyLibraryPolicy.parse(text);
    } catch (error) {
      KeyboardLog.error(`community reply library unavailable: ${describeError(error)}`);
      return [];
    }
  }

  private static safeStateDirectory(directory: string): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch {
      return false;
    }
  }

  private static safePrivateFile(path: string, maximumBytes: number): fs.Stat | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      return !stat.isSymbolicLink() && stat.isFile() && stat.size <= maximumBytes ? stat : null;
    } catch {
      return null;
    }
  }

  private static writePrivateText(path: string, text: string): boolean {
    try {
      const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
      let renamed: boolean = false;
      try {
        const handle: fs.File = fs.openSync(staging,
          fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
        try {
          const written: number = fs.writeSync(handle.fd, text);
          if (!utf8WriteComplete(text, written)) throw new Error('private_write');
        } finally {
          fs.closeSync(handle);
        }
        fs.renameSync(staging, path);
        renamed = true;
        return true;
      } finally {
        if (!renamed) {
          try {
            if (fs.accessSync(staging)) fs.unlinkSync(staging);
          } catch {
            // 私有文件写入失败时，最多留下以唯一名称创建的暂存文件。
          }
        }
      }
    } catch (error) {
      KeyboardLog.error('keyboard.KeyboardSession.writePrivateText failed');
      throw error as Error;
    }
  }

  private static readPrivateText(path: string, maximumBytes: number): string | null {
    let handle: fs.File | null = null;
    try {
      const expected: fs.Stat | null = KeyboardSession.safePrivateFile(path, maximumBytes);
      if (expected === null) return null;
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > maximumBytes || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      return util.TextDecoder.create('utf-8').decodeToString(new Uint8Array(buffer, 0, count));
    } catch {
      return null;
    } finally {
      if (handle !== null) NativeCleanup.close(handle);
    }
  }

  private static isSymbolicLink(path: string): boolean {
    try {
      return fs.lstatSync(path).isSymbolicLink();
    } catch {
      return false;
    }
  }

  /** Generates bounded reply candidates through the shared AI ABI without involving Engine state. */
  async thoughtfulReplies(source: string, style: string, extraPrompt: string = '', polish: boolean = false): Promise<string[]> {
    const request: ReplyRequest | null = ReplyKeyboardPolicy.request(source, style, extraPrompt, polish);
    if (request === null || !AiGenerationGate.enabled() || !this.aiAssistant.enabled
      || this.aiAssistant.endpoint.length === 0 || !HarmonyPrivacyConsent.networkAllowed()) {
      return [];
    }
    const editorGeneration: number = this.editorGeneration;
    const contextGeneration: number = this.replyContextGeneration;
    // The reply instructions ride in the first custom slot, which the shared request builder reads as the system prompt; the user's candidate prompts do not apply to replies.
    const config: PreparedAiAssistant = {
      enabled: this.aiAssistant.enabled, provider: this.aiAssistant.provider,
      model: this.aiAssistant.model, token: this.aiAssistant.token, tokens: this.aiAssistant.tokens,
      endpoint: this.aiAssistant.endpoint, candidate_limit: 3, prompt_id: 'custom_1',
      prompt_custom_1: request.prompt, prompt_custom_2: '', prompt_custom_3: ''
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
      const rawValue: AiHttpDescriptor | null = NativeReplyPolicy.successfulValue(raw);
      if (rawValue === null) return [];
      descriptor = rawValue;
    } catch (error) {
      KeyboardLog.error(`thoughtful reply request unavailable: ${describeError(error)}`);
      return [];
    }
    const requestHttp = http.createHttp();
    installNoRedirectGuard(requestHttp);
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
    const results: string[] = parsed.ok === true ? ReplyKeyboardPolicy.results(parsed.value) : [];
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
   * 键盘的常用语（不带编码），按用户排定的顺序，供工具栏的常用语面板使用。
   *
   * 文件是偏好目录下的 `CommonPhrases.json`，设置应用在锁下共享它，因此每个操作都在原生工作线程上执行。有史以来第一次加载时按 `CommonPhraseStarters` 的规则决定是否添加初始短语：空文档（没有短语也没有短语包）会逐条加入初始短语，无论哪种情况都会写入标记，这样之后的空列表（用户自己清空的）会保持为空。两次首次加载同时发生也不会重复添加，因为共享层会拒绝重复的文本。
   *
   * 列表无法读取时 reject，面板会显示为加载错误，而不是空列表。
   */
  async commonPhrases(): Promise<string[]> {
    try {
      const directory: string = this.stateDirectory;
      if (directory.length === 0) {
        throw new Error('common phrases unavailable: no session directory');
      }
      let document: CommonPhrasesDocument =
        KeyboardSession.commonPhrasesDocument(await KeyboardSession.commonPhrasesCall(directory, { operation: 'load' }));
      const marker: string = `${directory}/${CommonPhraseStarters.MARKER_FILE_NAME}`;
      const marked: boolean = fs.accessSync(marker);
      if (!marked) {
        const seeded: string[] = [];
        if (CommonPhraseStarters.seeds(marked, (document.phrases ?? []).length, (document.packs ?? []).length)) {
          for (const text of CommonPhraseStarters.TEXTS) {
            const added: CommonPhrasesReply =
              await KeyboardSession.commonPhrasesCall(directory, { operation: 'add', text: text });
            if (added.ok === true && added.value !== undefined && added.value !== null) {
              document = added.value;
              seeded.push(text);
            } else {
              KeyboardLog.error(`common phrase starter not added: ${added.error ?? 'malformed reply'}`);
            }
          }
        }
        this.writeCommonPhrasesMarker(marker, seeded);
        // 初始化依次执行各自加锁的原生操作；设置进程可能在操作间修改文档，因此重新加锁读取快照，不把最后一次添加的响应当作完整列表。
        document = KeyboardSession.commonPhrasesDocument(
          await KeyboardSession.commonPhrasesCall(directory, { operation: 'load' }));
      }
      const texts: string[] = [];
      for (const phrase of document.phrases ?? []) {
        if (typeof phrase.text === 'string' && phrase.text.length > 0) {
          texts.push(phrase.text);
        }
      }
      return texts;
    } catch (error) {
      KeyboardLog.error('keyboard.KeyboardSession.commonPhrases failed');
      throw error as Error;
    }
  }

  /** 在原生工作线程上执行一次加锁的常用语操作；工作线程的 reject 原样传出。 */
  private static async commonPhrasesCall(directory: string, action: CommonPhrasesAction): Promise<CommonPhrasesReply> {
    const request: CommonPhrasesRequest = { directory: directory, action: action };
    const raw: string = await client.commonPhrases(JSON.stringify(request));
    return JSON.parse(raw) as CommonPhrasesReply;
  }

  /** 一次加载得到的文档，或带有共享层 `common_phrases_*` 错误码的错误。 */
  private static commonPhrasesDocument(reply: CommonPhrasesReply): CommonPhrasesDocument {
    if (reply.ok !== true || reply.value === undefined || reply.value === null
      || typeof reply.value !== 'object' || (reply.value.phrases !== undefined && !Array.isArray(reply.value.phrases))) {
      throw new Error(`common phrases refused: ${reply.error ?? 'malformed reply'}`);
    }
    return reply.value;
  }

  /** 记录初始短语已决定。写入失败只记日志：下一次加载会再试，而由于列表此时已不为空（除非用户自己清空），通常不会添加任何内容。 */
  private writeCommonPhrasesMarker(marker: string, seeded: string[]): void {
    try {
      if (!KeyboardSession.safeStateDirectory(this.stateDirectory) || KeyboardSession.isSymbolicLink(marker)) {
        KeyboardLog.error('common phrases marker not written: unsafe state directory');
        return;
      }
      KeyboardSession.writePrivateText(marker, CommonPhraseStarters.marker(seeded));
    } catch (error) {
      KeyboardLog.error(`common phrases marker not written: ${describeError(error)}`);
    }
  }

  /**
   * 把一条常用语放进编辑框：先按原样提交正在进行的组字（`MSIME_COMMIT_RAW`，Android 面板在打开时和选取时都会发送它），再把常用语作为当前方案名下的输入文字插入，与 Android 的 `commitText` 完全一致。内容逐字插入，不做全角或繁体转换，因为它是用户自己存的文本。
   */
  commitPhrase(text: string): void {
    if (text.length === 0) {
      return;
    }
    if (this.composing()) {
      this.commitRaw();
    }
    this.dismissPrediction();
    if (this.inputClient === undefined) {
      return;
    }
    this.insertEditorText(text);
    this.recordTypingStatistics(text, this.typingSource());
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
    this.useScheme(KeyboardScheme.engineSchemeOf(selected), selected.shuangpinProfile);
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
      if (loaded.ok !== true) {
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
      if (loaded.ok !== true) {
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
      const applied: PreferencesUpdateResponse = JSON.parse(client.updatePreferences(this.handle,
        PrivacyGate.sessionPreferences(snapshot, this.sessionPrivate))) as PreferencesUpdateResponse;
      if (applied.ok !== true) {
        KeyboardLog.error(`preferences refused: ${applied.error}`);
        return false;
      }
      const saved: SnapshotResponse =
        JSON.parse(client.savePreferences(this.stateDirectory, expected, document)) as
          SnapshotResponse;
      if (saved.ok !== true) {
        // The session already has them; only the write failed. Say so rather than pretending the
        // change did not happen, because on screen it plainly did.
        KeyboardLog.error(`preferences not persisted: ${saved.error}`);
      } else {
        // Our own write, already applied; the next focus must not rebuild the session for it. The revision is the store's answer rather than expected + 1: a save that changes nothing keeps the stored revision, and assuming the next one would make the next real change made elsewhere look like this one.
        this.preferencesRevision = saved.value.revision;
      }
      if (EngineViewValuePolicy.isValid(applied.value.view)) {
        this.lastView = applied.value.view;
      }
      this.refreshPreparedPreferences(document);
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
   * 把刚交给会话的整份文档换成缓存的偏好，并按它重算外观，让 `preferences()` 立刻反映这次改动。
   *
   * 以前只有 `selectGlobalTheme` 自己补这一步，其他改动（键盘里拖高度、间距，± 按钮）存进文档、版本号也跟上了，`preferences()` 却还是准备会话时的旧值：视图存完立刻读回旧高度，拖动弹回原位、± 不累加，面板尺寸也按旧值算；下次聚焦时版本号已经一致，不会重建，旧值就一直留着。
   */
  private refreshPreparedPreferences(document: string): void {
    const saved: PreparedSnapshot = JSON.parse(document) as PreparedSnapshot;
    this.preparedPreferences = saved.preferences;
    this.applyAppearance(saved.preferences);
  }

  /**
   * One page of the local emoji catalog.
   *
   * Reads msime-others.db directly rather than through the session: the catalog is a property of the
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
      if (response.ok !== true) {
        KeyboardLog.error(`emoji catalog refused: ${response.error}`);
        return null;
      }
      return response.value === undefined || response.value === null ? null : response.value;
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
      if (response.ok !== true || response.value === undefined || response.value === null
        || !Array.isArray(response.value.groups)) {
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
      if (response.ok !== true || response.value === undefined || response.value === null
        || !Array.isArray(response.value.symbol_groups)) {
        return [];
      }
      return normalizeSymbolGroups(response.value.symbol_groups);
    } catch (error) {
      KeyboardLog.error(`emoji symbol groups unavailable: ${describeError(error)}`);
      return [];
    }
  }

  /**
   * 已安装符号集插件的组，读自 `<stateDirectory>/plugins`，即键盘自己沙箱里的插件目录。设置页把包装进设置应用的 `files/state/plugins`；API 12 起输入法扩展使用独立沙箱，设置应用把插件目录发布到共享沙箱，键盘在启动和获得焦点时导入到这里（见 platforms/harmony/README.md）。签名不带共享沙箱授权时（开发构建、模拟器）这里读不到设置页装的包，通常是空的。
   *
   * 不依赖 `msime-others.db`；读不出来时返回空表，表情面板照旧只显示内置内容。
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
      if (response.ok !== true || response.value === undefined || response.value === null) {
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
  private asTraditional(text: string, source: number = -1): string {
    return ChineseOutputPolicy.candidateOutput(text, this.traditional,
      ChineseOutputPolicy.applies(this.englishMode, this.scheme, this.localMode),
      source,
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
    // 直接写进编辑器的文字（123 层的数字、表情、剪贴板）照常输入，旧位置的联想随之作废；它们不经过引擎，引擎自己清不掉。
    this.dismissPrediction();
    if (this.commitEnglishBoundary(text)) return;
    if (this.inputClient !== undefined) {
      const widened: string | null = FullWidthInputPolicy.output(text, this.fullWidth);
      const output: string = this.asTraditional(widened === null ? text : widened);
      this.insertEditorText(output);
      this.recordTypingStatistics(output, this.typingSource());
      this.refreshCaretSuggestions();
    }
  }

  /**
   * 打字或上屏之后，按光标前的上文刷新英文补全和邮箱后缀。
   *
   * 两者都要读上文时只向编辑器要一次：每次 `getForwardSync` 都是一次同步跨进程调用，要等宿主应用的 UI 线程作答，而直接英文里每个字母都走到这里。读的长度取两者中较长的；邮箱后缀只看末尾 `EmailSuffixPolicy.MAX_CONTEXT` 个字符，和它自己去读时看到的一样。
   */
  private refreshCaretSuggestions(): void {
    if (!this.englishSuggestionsApply() || !this.emailSuffixesReadContext()) {
      this.refreshEnglishSuggestions();
      this.refreshEmailSuffixes();
      return;
    }
    const context: string | null = this.contextBeforeCaret(
      Math.max(EnglishSuggestionPolicy.CONTEXT_CHARACTERS, EmailSuffixPolicy.MAX_CONTEXT));
    this.suggestEnglish(EnglishSuggestionPolicy.currentWord(context));
    this.matchEmailSuffixes(true,
      context === null ? null : context.slice(-EmailSuffixPolicy.MAX_CONTEXT));
  }

  /** 光标前最多 `length` 个字符；编辑器读不到时为 null。 */
  private contextBeforeCaret(length: number): string | null {
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (inputClient === undefined) return null;
    try {
      return inputClient.getForwardSync(length);
    } catch (error) {
      return null;
    }
  }

  /**
   * Offer completions for the English word being typed.
   *
   * Only in direct English with nothing composing: a pinyin composition already has a candidate strip of its own, and the two would be competing for it. The query is a packaged-dictionary lookup that creates no session. It runs in a microtask after the key's own work, still on the UI thread, and its answer is dropped if another edit has bumped the epoch before it is drawn.
   */
  private refreshEnglishSuggestions(): void {
    if (!this.englishSuggestionsApply()) {
      this.clearEnglishSuggestions();
      return;
    }
    this.suggestEnglish(this.englishWordBeforeCursor());
  }

  private englishSuggestionsApply(): boolean {
    return this.englishSuggestionsEnabled && this.englishMode && !this.composing()
      && this.inputClient !== undefined && this.resourceDirectory.length > 0
      && !EditorPolicy.prefersLatin(this.traits) && EditorPolicy.useEngine(this.traits);
  }

  /** 按光标前正在打的词 `prefix` 查补全；`prefix` 是调用方刚从编辑器读到的。 */
  private suggestEnglish(prefix: string): void {
    if (prefix !== this.englishCorrectionRejectedPrefix) this.englishCorrectionRejectedPrefix = '';
    const correction: boolean = this.englishCorrectionEnabled();
    const action: EnglishRefresh =
      EnglishSuggestionPolicy.refresh(prefix, this.englishSuggestionRequested, correction);
    if (action === EnglishRefresh.CLEAR) {
      this.clearEnglishSuggestions();
      return;
    }
    if (action === EnglishRefresh.KEEP) {
      return;
    }
    const epoch: number = ++this.englishSuggestionEpoch;
    this.englishSuggestionRequested = prefix;
    // 手机纠错模式请求共享引擎的合并展示：有纠错时只有原文与纠错，原文已是完整的词时只有原文，否则原文后接前缀补全；空格和标点边界仍只查询唯一纠错。
    const request: EnglishCompletionRequest = {
      prefix: prefix, limit: correction ? EnglishSuggestionPolicy.DISPLAY_LIMIT : EnglishSuggestionPolicy.LIMIT,
      correction: correction, completions: correction
    };
    const resources: string = this.resourceDirectory;
    // A promise rather than a direct call: the lookup waits until the key that asked for it has finished its own work. It still runs on this thread.
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

  /**
   * 不再向编辑器重读当前词：这个微任务紧跟在发出查询的那次刷新之后、同一轮事件里运行，编辑器的任何改动都要等之后的回报才进来，而改动编辑器的路径和每次编辑器回报都会清掉或重发补全，让 `englishSuggestionEpoch` 前进。选用补全时 `useEnglishSuggestion` 还会按编辑器里的词再核对一次，不会删错字。
   */
  private applyEnglishSuggestions(epoch: number, prefix: string,
                                  completions: EnglishCompletions | null): void {
    if (epoch !== this.englishSuggestionEpoch || completions === null
        || completions.prefix !== prefix) {
      return;
    }
    this.englishSuggestions = completions.items;
    this.englishSuggestionCorrected = completions.corrected;
    this.englishSuggestionPrefix = prefix;
    this.publishEnglishSuggestions();
  }

  private clearEnglishSuggestions(): void {
    this.englishCorrectionRejectedPrefix = '';
    this.englishSuggestionEpoch++;
    this.englishSuggestionRequested = '';
    this.englishSuggestionCorrected = false;
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
      // 只有展示了纠错时点原文才是拒绝纠错；只有补全时点原文仅收起候选。
      const rejected: boolean = this.englishSuggestionCorrected;
      this.clearEnglishSuggestions();
      if (rejected) this.englishCorrectionRejectedPrefix = typed;
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

  /**
   * 光标前打到 `xxx@` 时给出邮箱后缀，在任何输入框里都给（Android 原来只在邮箱输入框里给）。
   *
   * 上文只在内存里匹配一次，不记录、不保存；密码框和属性尚未到达的输入框不读。结果变了才通知视图。
   */
  private refreshEmailSuffixes(): void {
    const reads: boolean = this.emailSuffixesReadContext();
    this.matchEmailSuffixes(reads,
      reads ? this.contextBeforeCaret(EmailSuffixPolicy.MAX_CONTEXT) : null);
  }

  private emailSuffixesReadContext(): boolean {
    return this.inputClient !== undefined && EmailSuffixPolicy.readsContext(
      KeyboardFormFactor.isDesktop(), this.editorAttributesKnown, this.traits.password, this.composing());
  }

  /**
   * 按光标前的上文 `context` 更新后缀。`reads` 为假表示这时不该读上文，后缀收起；`context` 为 null 表示上文读不到，也不给后缀。不记录读到或没读到的文字。
   */
  private matchEmailSuffixes(reads: boolean, context: string | null): void {
    const next: EmailSuffixMatch | null = reads ? EmailSuffixPolicy.match(context) : null;
    if (reads && context !== null) {
      this.caretContextEcho.read();
    } else {
      this.caretContextEcho.unread();
    }
    if (EmailSuffixPolicy.same(next, this.emailSuffixMatch)) return;
    this.emailSuffixMatch = next;
    this.publishEmailSuffixes();
  }

  private clearEmailSuffixes(): void {
    this.caretContextEcho.unread();
    if (this.emailSuffixMatch === null) return;
    this.emailSuffixMatch = null;
    this.publishEmailSuffixes();
  }

  private publishEmailSuffixes(): void {
    if (this.onEmailSuffixes !== undefined) {
      const match: EmailSuffixMatch | null = this.emailSuffixMatch;
      this.onEmailSuffixes(match === null ? [] : match.suffixes.slice());
    }
  }

  /**
   * 补全邮箱后缀：按点下这一刻的上文重新匹配，删掉已经打的域名，上屏完整后缀（ASCII，不随全角或繁体转换）。上文已经变了、对不上这个后缀时只收起。
   */
  useEmailSuffix(index: number): void {
    const shown: EmailSuffixMatch | null = this.emailSuffixMatch;
    const inputClient: inputMethodEngine.InputClient | undefined = this.inputClient;
    if (shown === null || index < 0 || index >= shown.suffixes.length || inputClient === undefined
        || !EmailSuffixPolicy.readsContext(KeyboardFormFactor.isDesktop(), this.editorAttributesKnown,
          this.traits.password, this.composing())) {
      this.clearEmailSuffixes();
      return;
    }
    const suffix: string = shown.suffixes[index];
    let replacement: EmailSuffixReplacement | null = null;
    try {
      replacement = EmailSuffixPolicy.replacement(
        EmailSuffixPolicy.match(inputClient.getForwardSync(EmailSuffixPolicy.MAX_CONTEXT)), suffix);
    } catch (error) {
      replacement = null;
    }
    if (replacement !== null) {
      try {
        if (replacement.deleteCount > 0) this.deleteEditorBackwardSync(replacement.deleteCount);
        this.insertEditorTextSync(replacement.insert);
        this.recordTypingStatistics(replacement.insert, this.typingSource());
      } catch (error) {
        KeyboardLog.error(`email suffix completion unavailable: ${describeError(error)}`);
      }
    }
    // 补全后上文以完整后缀结尾，不再匹配；编辑器随后的选区回报也会再匹配一次。
    this.clearEmailSuffixes();
    this.refreshEnglishSuggestions();
  }

  private englishCorrectionEnabled(): boolean {
    return !KeyboardFormFactor.isDesktop() && this.englishMode && this.englishSuggestionsEnabled
      && !this.englishCandidates && this.keyGrid !== KeyGrid.NINE_KEY
      && this.keyGrid !== KeyGrid.NINE_KEY_SHUANGPIN && !this.composing()
      && EditorPolicy.useEngine(this.traits) && !EditorPolicy.prefersLatin(this.traits);
  }

  /** 边界键必须以编辑器当前词为准，不能拿旧候选删除光标旁边的其他文字。 */
  private commitEnglishBoundary(boundary: string): boolean {
    if (!this.englishCorrectionEnabled() || boundary.length !== 1
        || ' ,.!?:;'.indexOf(boundary) < 0 || this.inputClient === undefined) return false;
    if (boundary !== ' ' && this.punctuationLockedChinese()) return false;
    const typed: string = this.englishWordBeforeCursor();
    if (typed.length === 0) return false;
    let candidate: string = typed;
    if (typed !== this.englishCorrectionRejectedPrefix && this.resourceDirectory.length > 0) {
      try {
        const request: EnglishCompletionRequest = { prefix: typed, limit: 2, correction: true };
        const result: EnglishCompletions | null = EnglishSuggestionPolicy.decode(
          client.englishCompletions(JSON.stringify(request), this.resourceDirectory));
        if (result !== null && result.prefix === typed && result.items.length === 2
            && result.items[0] === typed) candidate = result.items[1];
      } catch (error) {
        // 可选纠错查询失败时仍然输入原文和边界，不记录实际文字。
      }
    }
    if (typed !== this.englishWordBeforeCursor()) return false;
    const replacement: EnglishReplacement | null = EnglishSuggestionPolicy.replacement(
      typed, candidate, EnglishSuggestionPolicy.startedCapitalized(typed));
    const suffix: string = boundary === ' ' ? ' ' : boundary + ' ';
    try {
      if (replacement !== null) this.deleteEditorBackwardSync(replacement.deleteCount);
      const output: string = (replacement === null ? '' : replacement.insert) + suffix;
      this.insertEditorTextSync(output);
      this.recordTypingStatistics(suffix, this.typingSource());
    } catch (error) {
      // 编辑器可能已断开；不再次输入同一边界，避免部分写入后重复。
    }
    this.clearEnglishSuggestions();
    return true;
  }

  /**
   * The five switches as the policy reads them; an older document keeps the shared defaults.
   *
   * 空格锁定固定为开，不读存下的 `hotkey_hold_space_lock`：空格只在按住录音组合键期间才被拦下，平常打字不受影响。设置页不再给这个开关（`voice_space_lock_switch` 为 false），与 macOS、Windows、Linux 一致。
   */
  private static hotkeyBindings(voice: VoiceInputConfiguration): VoiceHotkeyBindings {
    return {
      ralt: voice.hotkey_ralt ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ralt,
      ctrlWin: voice.hotkey_ctrl_win ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ctrlWin,
      rctrlRalt: voice.hotkey_rctrl_ralt ?? DEFAULT_VOICE_HOTKEY_BINDINGS.rctrlRalt,
      holdSpaceLock: DEFAULT_VOICE_HOTKEY_BINDINGS.holdSpaceLock,
      ctrlF9: voice.hotkey_ctrl_f9 ?? DEFAULT_VOICE_HOTKEY_BINDINGS.ctrlF9
    };
  }

  /** 麦克风未授权时只打开本应用的授权窗口，不在输入法扩展里采集或申请权限。 */
  requestVoicePermission(): void {
    this.onOpenSettings?.(undefined, true);
  }

  /**
   * 打开设置应用：词库、快捷短语、云端和语音服务都在那里，而不在键盘上。
   *
   * `page` 用于直接打开某一页（功能面板的「词库」「反馈」「关于」功能块）：`'dictionary'`、`'feedback'` 或 `'about'`。其他值或不传时打开设置首页，这样设置应用不能路由的页面 id 永远不会发给它。
   *
   * `target` 是那一页里要直接打开的一处（长按九键左侧符号栏带上那张表的共享偏好名），只在它属于 `page` 时随请求发出（`SettingsPageLink.acceptsTarget`）；不属于时照旧只打开页面。
   */
  openSettings(page?: string, target?: string): void {
    if (this.onOpenSettings === undefined) {
      return;
    }
    const requested: string | undefined =
      page !== undefined && SettingsPageLink.accepts(page) ? page : undefined;
    if (page !== undefined && requested === undefined) {
      KeyboardLog.error(`settings page ${page} is not routed; opening the settings root`);
    }
    const requestedTarget: string | undefined =
      requested !== undefined && target !== undefined && SettingsPageLink.acceptsTarget(requested, target)
        ? target : undefined;
    if (target !== undefined && requestedTarget === undefined) {
      KeyboardLog.error(`settings target ${target} is not routed on ${page ?? 'the settings root'}; opening the page`);
    }
    this.onOpenSettings(requested, false, requestedTarget);
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
    if (active) {
      this.voiceRecordingStartedMs = Date.now();
      this.voiceRecordedMs = 0;
    } else {
      this.voiceRecordedMs = cancelled ? 0 : Math.max(0, Date.now() - this.voiceRecordingStartedMs);
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

  /** 扩展的上下文：靠它把打包的提示音变成能播放的声音。它的临时目录同时用来放离线 transducer 加载时读的热词文件。 */
  useVoiceContext(context: common.Context): void {
    this.voiceBehaviour.use(context);
    this.voiceScratchDirectory = context.tempDir;
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

  /** 按键预览气泡能否显示正在按下的键：在密码框上不能，否则会把每个字符展示给任何看着屏幕的人（Android 的 `ImeLetterRows.showKeyPreview`）；在属性尚未到达的编辑框上也不能，理由与按键音等待属性相同。 */
  keyPreviewAllowed(): boolean {
    return !this.soundsSilencedByEditor();
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
      if (answer.ok !== true) {
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
    this.dismissPrediction();
    const output: string = this.asTraditional(text);
    this.insertEditorText(output);
    this.recordTypingStatistics(output, source);
  }

  /**
   * 识别结果进入编辑框：在打字统计里记为语音输入，并在隐私闸门允许时把这次录音的时长计入「统计」页的语音时长一次（`record_voice`，「语音达人」徽章），与 Android 的 `ImeVoiceEntry` 在提交后的做法相同。
   */
  insertVoiceResult(text: string): void {
    if (this.inputClient === undefined || text.length === 0) return;
    this.insertWithSource(text, 'voice');
    const milliseconds: number = Math.min(this.voiceRecordedMs, KeyboardSession.MAX_VOICE_RECORD_MS);
    this.voiceRecordedMs = 0;
    if (milliseconds <= 0 || this.stateDirectory.length === 0
      || !this.privacyAllows(PrivacyRecord.VOICE_DURATION)) return;
    const requestDocument: TypingStatisticsVoiceRequest = {
      directory: this.stateDirectory,
      action: { operation: 'record_voice', day: TypingStatisticsPolicy.day(new Date()), milliseconds: milliseconds }
    };
    const request: string = JSON.stringify(requestDocument);
    // 存储会加文件锁，和提交一样排到 TaskPool 线程上写，排在刚才那次提交之后。
    this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request)).then((reply: Object): void => {
      const response: TypingStatisticsRecordResponse = JSON.parse(reply as string) as TypingStatisticsRecordResponse;
      if (response.ok !== true) {
        KeyboardLog.error('voice duration not recorded');
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`voice duration unavailable: ${describeError(error)}`);
    });
  }

  private typingSource(): string {
    return TypingStatisticsPolicy.source(this.schemeName, this.profileName, this.englishMode,
      this.keyGrid, this.localMode);
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
    if (text.length === 0 || this.stateDirectory.length === 0
      || !this.privacyAllows(PrivacyRecord.TYPING)) return;
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
    // 原生存储要加文件锁、读写统计文件，交给 TaskPool 线程，不占 UI 线程，下一个键的延迟不受慢文件系统影响。以前用 setTimeout(0) 推迟，那仍在 UI 线程上，低端机上每次提交都会卡一下。
    this.typingStatisticsUnflushed = true;
    this.typingStatisticsWrites.execute(new taskpool.Task(writeTypingStatistics, request)).then((response: Object): void => {
      const value: TypingStatisticsRecordResponse = JSON.parse(response as string) as TypingStatisticsRecordResponse;
      if (value.ok !== true) {
        KeyboardLog.error('typing statistics write refused');
      } else if (typeof value.value?.milestone === 'number' && !this.soundsSilencedByEditor()) {
        // host-api reports the milestone only while achievements are on; it plays the jingle itself on the desktop, and here the player does. Not over a password field, like every other sound, and not once the editor has let go: its attributes are forgotten then.
        this.soundPlayer.achievement();
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`typing statistics unavailable: ${describeError(error)}`);
    });
  }

  /**
   * 让原生层把这个键盘进程攒着的打字统计现在写进文件：键盘收起、编辑框失去焦点、扩展销毁时调用。排在同一个 SequenceRunner 上，之前送出的提交都会包含在这次写入里。返回的 Promise 在写完后完成，失败也完成；上次 flush 之后没有新提交时不再排一次，返回上一次的。
   *
   * 原生库拒绝 flush 时记一次日志：攒着的计数这时只能等原生层自己在失焦、销毁或满三十秒时写出，原因能从诊断日志里看出来。
   */
  private flushTypingStatistics(): Promise<void> {
    if (!this.typingStatisticsUnflushed || this.stateDirectory.length === 0) {
      return this.typingStatisticsFlushed;
    }
    this.typingStatisticsUnflushed = false;
    const requestDocument: TypingStatisticsFlushRequest = {
      directory: this.stateDirectory, action: { operation: 'flush' }
    };
    const task: taskpool.Task = new taskpool.Task(writeTypingStatistics, JSON.stringify(requestDocument));
    this.typingStatisticsFlushed = this.typingStatisticsWrites.execute(task).then((response: Object): void => {
      const value: TypingStatisticsRecordResponse = JSON.parse(response as string) as TypingStatisticsRecordResponse;
      if (value.ok !== true && !this.typingStatisticsFlushRefusalLogged) {
        this.typingStatisticsFlushRefusalLogged = true;
        KeyboardLog.error(`typing statistics flush refused: ${value.error ?? 'unknown'}`);
      }
    }).catch((error: Object): void => {
      KeyboardLog.error(`typing statistics flush unavailable: ${describeError(error)}`);
    });
    return this.typingStatisticsFlushed;
  }

  /** 键盘收起（系统的 keyboardHide，例如返回手势）：停掉键盘内的语音识别，把攒着的打字统计写盘，停下词库的空闲检查，关掉缓存的释义词库。编辑框可能仍有焦点，所以不等 inputStop。 */
  keyboardHidden(): void {
    if (this.onPanelHidden !== undefined) {
      this.onPanelHidden();
    }
    this.flushTypingStatistics();
    this.pauseIdleDictionaryWork();
    // 候选释义按文件身份缓存在 UI 线程上的词库连接和自定义译文，隐藏的键盘不留着它们（关掉释义以后也不会一直占着）；下一次显示后的第一次查询重新打开。
    client.releaseGlossCaches();
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
    this.expandedCandidateIndices = [];
    if (this.handle === 0) {
      return [];
    }
    try {
      const response: SnapshotCandidates =
        JSON.parse(client.allCandidates(this.handle)) as SnapshotCandidates;
      if (response.ok !== true) {
        KeyboardLog.error(`candidate list refused: ${response.error}`);
        return [];
      }
      if (response.value === undefined || response.value === null
        || !EngineViewValuePolicy.isGeneration(response.value.generation)) {
        KeyboardLog.error('candidate list has malformed generation');
        return [];
      }
      // The generation identifies the composition these belong to; selecting against a stale one
      // would act on a list the user is no longer looking at.
      this.generation = response.value.generation;
      // The snapshot carries no scheme; the list being the Hanja one is read from the view it expands.
      const hanja: boolean = this.candidateListOpen() && this.lastView?.scheme === SchemeTraits.KOREAN;
      const entries: ExpandedCandidate[] = [];
      for (let index: number = 0; index < response.value.candidates.length; index++) {
        const candidate: Candidate = response.value.candidates[index];
        if (!CandidatePresentationPolicy.isVisible(candidate.source)) {
          continue;
        }
        this.expandedCandidateIndices.push(index);
        entries.push({
          text: this.asTraditional(candidate.text, candidate.source),
          annotation: CandidateGlossPolicy.annotation(
            CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
            TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
            this.candidateGlossEnabled),
          annotationIsTranslation: CandidateGlossPolicy.annotationIsTranslation(
            CandidateGlossPolicy.slotAnnotation(hanja, candidate.annotation),
            TranslationPolicy.sourceGetsNoGloss(candidate.source) ? null : candidate.translation ?? null,
            this.candidateGlossEnabled),
          hunEum: CandidateGlossPolicy.hunEum(hanja, candidate.annotation),
          confidentCorrection: CorrectionMarkPolicy.confident(candidate.confident_correction)
        });
      }
      return entries;
    } catch (error) {
      KeyboardLog.error(`candidate list unreadable: ${describeError(error)}`);
      return [];
    }
  }

  /** 上一次 `allCandidates` 取到的是哪个会话的哪一代候选；展开网格据此分辨重取的是同一份列表（释义晚到）还是换了候选。 */
  candidateListIdentity(): string {
    return `${this.handle}:${this.generation}`;
  }

  /**
   * The candidate-management operations behind a long press: promote it, pin it to the front, let go
   * of a pinned position, or take the entry out of the dictionary altogether.
   */
  private engineCandidateIndex(index: number): number {
    return index >= 0 && index < this.visibleCandidateIndices.length
      ? this.visibleCandidateIndices[index] : -1;
  }

  /** 候选能不能固定、调序或删除：上屏后联想的候选不属于正在组的字，和 Android 的 `candidateManagementEnabled` 一样不给管理。 */
  candidateManagementAllowed(): boolean {
    return !this.predicting();
  }

  promoteCandidate(index: number): void {
    if (this.handle === 0 || !this.candidateManagementAllowed()) {
      return;
    }
    const actualIndex: number = this.engineCandidateIndex(index);
    if (actualIndex < 0) return;
    this.apply(client.pinCandidate(this.handle, this.generation, actualIndex));
  }

  /** Position 1 is the front of the list, which is what 固定到首位 means. */
  fixCandidateFirst(index: number): void {
    this.fixCandidatePosition(index, 1);
  }

  fixCandidatePosition(index: number, position: number): void {
    if (this.handle === 0 || !this.candidateManagementAllowed()) {
      return;
    }
    // Checked here rather than trusted from the menu. Today the menu is the only caller and it only
    // ever builds slots 1 to 5, so this throws for nobody; it is here because this is the last point
    // before the slot becomes an argument to the Engine, and a session method is callable by
    // anything in the host.
    const actualIndex: number = this.engineCandidateIndex(index);
    if (actualIndex < 0) return;
    this.apply(client.fixCandidatePosition(this.handle, this.generation, actualIndex,
      CandidateManagementAction.validatePosition(position)));
  }

  clearCandidatePosition(index: number): void {
    if (this.handle === 0 || !this.candidateManagementAllowed()) {
      return;
    }
    const actualIndex: number = this.engineCandidateIndex(index);
    if (actualIndex < 0) return;
    this.apply(client.clearCandidatePosition(this.handle, this.generation, actualIndex));
  }

  removeCandidate(index: number): void {
    if (this.handle === 0 || !this.candidateManagementAllowed()) {
      return;
    }
    const actualIndex: number = this.engineCandidateIndex(index);
    if (actualIndex < 0) return;
    this.apply(client.removeCandidate(this.handle, this.generation, actualIndex));
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
    const actualIndex: number = this.engineCandidateIndex(index);
    if (this.handle === 0 || view === undefined || actualIndex < 0
        || actualIndex >= view.candidates.length || !this.candidateManagementAllowed()) {
      return false;
    }
    const candidate: Candidate = view.candidates[actualIndex];
    if (!CandidateManagementAction.candidateActionsAvailable(
        KeyboardScheme.engineSchemeName(view.scheme), candidate.source)) {
      return false;
    }
    this.apply(client.removeCandidate(this.handle, this.generation, actualIndex));
    return true;
  }

  /** Select by position in the full list rather than in the page. 返回引擎是否接受了这次选择，展开面板据此决定留不留（`CandidatePanelSelectionPolicy`）。 */
  chooseAny(index: number): boolean {
    if (this.handle === 0) {
      return false;
    }
    const actualIndex: number = this.expandedCandidateIndices[index] ?? -1;
    if (actualIndex < 0) return false;
    return this.apply(client.selectAnyCandidate(this.handle, this.generation, actualIndex));
  }

  /** Commit to one reading of the digits typed so far. */
  chooseSpelling(index: number): void {
    if (this.handle === 0) {
      return;
    }
    this.apply(client.chooseNineKeySpelling(this.handle, this.generation, index));
  }

  /**
   * 九宫格候选筛选：`singleCharacter` 只留单字，`strokes` 是首字的笔顺前缀（`hspnz`，空串不按笔画筛）。只在组字时有意义，引擎在组字结束时自己清掉；笔画词库不可用时引擎带回诊断、筛选不变，诊断照常画在读音行上。
   */
  setNineKeyFilter(singleCharacter: boolean, strokes: string): void {
    if (this.handle === 0 || !this.composing()) {
      return;
    }
    this.apply(client.setNineKeyFilter(this.handle, singleCharacter, strokes));
  }
}
