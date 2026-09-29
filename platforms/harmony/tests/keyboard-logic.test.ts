/**
 * Behavioural checks for the logic ported from the Android keyboard.
 *
 * HarmonyOS's own test framework (hypium) is instrumented: it needs a device or emulator, which the
 * HarmonyOS phone image is currently gated behind account eligibility. These classes hold no ArkUI or
 * NAPI dependency, so they run under plain node, and the assertions are written against the Java
 * source they were ported from rather than against the port.
 */
import { KeyboardGeometry } from "../entry/src/main/ets/keyboard/KeyboardGeometry";
import {
  LocalAsrPolicy,
  PcmFrameSlicer,
  SpeechSentenceAccumulator,
} from "../entry/src/main/ets/keyboard/input/LocalAsrPolicy";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";
import {
  KeyboardLayoutDragAxis,
  KeyboardLayoutDragPolicy,
} from "../entry/src/main/ets/keyboard/input/KeyboardLayoutDragPolicy";
import {
  ClipboardHistoryStore,
  ClipboardHistoryItem,
  ClipboardHistoryError,
  ClipboardFailure,
} from "../entry/src/main/ets/keyboard/clipboard/ClipboardHistoryStore";
import {
  EmojiCatalogModel,
  EmojiItem,
  EMOJI_PAGE_SIZE,
  EMOJI_RECENTS_LIMIT,
  MAX_TEXT_CODE_POINTS,
  normalizeGroups,
  normalizeSymbolGroups,
} from "../entry/src/main/ets/keyboard/emoji/EmojiCatalogModel";
import { CandidateWrapPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateWrapPolicy";
import { ExpandedCandidateLayout } from "../entry/src/main/ets/keyboard/candidate/ExpandedCandidateLayout";
import { CandidateChipWidth } from "../entry/src/main/ets/keyboard/candidate/CandidateChipWidth";
import {
  CandidateGlossLayoutPolicy,
  CandidateGlossProviderState,
} from "../entry/src/main/ets/keyboard/candidate/CandidateGlossLayoutPolicy";
import {
  KeyboardScheme,
  SchemeDefinition,
  PreferenceMapping,
} from "../entry/src/main/ets/keyboard/KeyboardScheme";
import { ReplyKeyboardPolicy } from "../entry/src/main/ets/keyboard/ReplyKeyboardPolicy";
import { ReplyContextPolicy } from "../entry/src/main/ets/keyboard/ReplyContextPolicy";
import { CommunityReplyLibraryPolicy } from "../entry/src/main/ets/keyboard/CommunityReplyLibraryPolicy";
import { TextPolicy } from "../entry/src/main/ets/keyboard/TextPolicy";
import { NineKeyLayout, NineKey } from "../entry/src/main/ets/keyboard/input/NineKeyLayout";
import {
  JapaneseNineKeyLayout,
  JapaneseKey,
  VariantGroup,
  DIRECTION_CENTRE,
  DIRECTION_LEFT,
  DIRECTION_UP,
  DIRECTION_RIGHT,
  DIRECTION_DOWN,
} from "../entry/src/main/ets/keyboard/input/JapaneseNineKeyLayout";
import { JapaneseNineKeyActions } from "../entry/src/main/ets/keyboard/input/JapaneseNineKeyActions";
import { ChineseHelpcodePolicy } from "../entry/src/main/ets/keyboard/input/ChineseHelpcodePolicy";
import { WubiCodeHintPolicy } from "../entry/src/main/ets/keyboard/input/WubiCodeHintPolicy";
import { LetterKeyFacePolicy } from "../entry/src/main/ets/keyboard/input/LetterKeyFacePolicy";
import {
  EnglishCapitalizationPolicy,
  CapitalizationMode,
} from "../entry/src/main/ets/keyboard/input/EnglishCapitalizationPolicy";
import {
  EnglishLetterCaseState,
  LetterCaseMode,
} from "../entry/src/main/ets/keyboard/input/EnglishLetterCaseState";
import { JapaneseVariantPolicy } from "../entry/src/main/ets/keyboard/input/JapaneseVariantPolicy";
import { JapaneseSpacePolicy } from "../entry/src/main/ets/keyboard/input/JapaneseSpacePolicy";
import { ClipboardHistoryPolicy } from "../entry/src/main/ets/keyboard/clipboard/ClipboardHistoryPolicy";
import { ClipboardHistoryPreferencePolicy } from "../entry/src/main/ets/keyboard/clipboard/ClipboardHistoryPreferencePolicy";
import { FullWidthInputPolicy } from "../entry/src/main/ets/keyboard/input/FullWidthInputPolicy";
import { InputDiagnosticPolicy } from "../entry/src/main/ets/keyboard/input/InputDiagnosticPolicy";
import { ChineseOutputPolicy } from "../entry/src/main/ets/keyboard/input/ChineseOutputPolicy";
import { LocalInputMode } from "../entry/src/main/ets/keyboard/input/LocalInputMode";
import { VoiceSessionPolicy } from "../entry/src/main/ets/keyboard/input/VoiceSessionPolicy";
import {
  QuickPunctuationPolicy,
  PunctuationEntry,
} from "../entry/src/main/ets/keyboard/input/QuickPunctuationPolicy";
import { SmartPunctuationContext } from "../entry/src/main/ets/keyboard/input/SmartPunctuationContext";
import {
  SmartPunctuationRepeatPolicy,
  SmartPunctuationRepeatSnapshot,
} from "../entry/src/main/ets/keyboard/input/SmartPunctuationRepeatPolicy";
import { PairedPunctuationPolicy } from "../entry/src/main/ets/keyboard/input/PairedPunctuationPolicy";
import {
  ReturnDispatch,
  ReturnKeyAction,
} from "../entry/src/main/ets/keyboard/input/ReturnKeyAction";
import { SpaceCursorMovement } from "../entry/src/main/ets/keyboard/input/SpaceCursorMovement";
import {
  CandidateManagementAction,
  ManagementAction,
} from "../entry/src/main/ets/keyboard/candidate/CandidateManagementAction";
import { CandidateFontFamilyPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateFontFamilyPolicy";
import { CandidateAnnotationPreferencePolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateAnnotationPreferencePolicy";
import { InputModeHudPolicy } from "../entry/src/main/ets/keyboard/InputModeHudPolicy";
import {
  VoiceResponsePolicy,
  type VoiceOutcome,
} from "../entry/src/main/ets/keyboard/input/VoiceResponsePolicy";
import {
  HardwareKeyDispatch,
  type HardwareKeyTarget,
} from "../entry/src/main/ets/keyboard/HardwareKeyDispatch";
import {
  ImeEnabledState,
  OnboardingStatePolicy,
} from "../entry/src/main/ets/keyboard/input/OnboardingStatePolicy";
import {
  AttachedKeyboardType,
  HardwareKeyboardPolicy,
  KeyRoutingTransition,
} from "../entry/src/main/ets/keyboard/input/HardwareKeyboardPolicy";
import { BusinessErrorPolicy } from "../entry/src/main/ets/keyboard/BusinessErrorPolicy";
import {
  StagedArtifact,
  StagedResourcePolicy,
} from "../entry/src/main/ets/keyboard/StagedResourcePolicy";
import {
  DEFAULT_VOICE_HOTKEY_BINDINGS,
  VoiceHotkeyAction,
  VoiceHotkeyBindings,
  VoiceHotkeyPolicy,
  VoiceKey,
} from "../entry/src/main/ets/keyboard/input/VoiceHotkeyPolicy";
import { VoiceRecordingBehaviourPolicy } from "../entry/src/main/ets/keyboard/input/VoiceRecordingBehaviourPolicy";
import {
  DEFAULT_MODE_BINDINGS,
  InputModeRouting,
  ModeBindings,
  ModeGesture,
  ModeKey,
} from "../entry/src/main/ets/keyboard/InputModeRouting";
import {
  KeyboardFeedbackBridge,
  MobileKeyboardFeedback,
} from "../entry/src/main/ets/keyboard/KeyboardFeedbackBridge";
import {
  HapticStrength,
  KeyboardFeedback,
} from "../entry/src/main/ets/keyboard/KeyboardFeedback";
import {
  EnglishCompletions,
  EnglishReplacement,
  EnglishSuggestionPolicy,
} from "../entry/src/main/ets/keyboard/input/EnglishSuggestionPolicy";
import { ImeModeScopePolicy } from "../entry/src/main/ets/keyboard/input/ImeModeScopePolicy";
import {
  HARMONY_CAPTURE_BACKEND,
  VoiceCaptureDevice,
  VoiceCaptureDevicePolicy,
} from "../entry/src/main/ets/keyboard/input/VoiceCaptureDevicePolicy";
import {
  CandidateGlossPolicy,
  GlossToken,
} from "../entry/src/main/ets/keyboard/candidate/CandidateGlossPolicy";
import { ShuangpinKeyHintPolicy } from "../entry/src/main/ets/keyboard/input/ShuangpinKeyHintPolicy";
import { EditorPolicy, EditorTraits } from "../entry/src/main/ets/keyboard/input/EditorPolicy";
import { KeyboardSkin } from "../entry/src/main/ets/keyboard/skin/KeyboardSkin";
import { ToolbarSkinPolicy } from "../entry/src/main/ets/keyboard/ToolbarSkinPolicy";
import {
  CustomKeyboardSkin,
  CustomSkinDocument,
  supportedPhoto,
} from "../entry/src/main/ets/keyboard/skin/CustomKeyboardSkin";
import { DictionaryMaintenancePolicy } from "../entry/src/main/ets/keyboard/DictionaryMaintenancePolicy";
import { KeyAccessibilityPolicy } from "../entry/src/main/ets/keyboard/input/KeyAccessibilityPolicy";
import { OsVersionPolicy } from "../entry/src/main/ets/keyboard/input/OsVersionPolicy";
import {
  AiPolishPolicy,
  MAX_POLISH_SOURCE_CHARACTERS,
  codePointLength,
} from "../entry/src/main/ets/keyboard/input/AiPolishPolicy";
import {
  HandwritingStrokePolicy,
  HANDWRITING_CANVAS_SIZE,
  HANDWRITING_MAX_CANDIDATES,
  HANDWRITING_MAX_POINTS,
  HANDWRITING_MAX_STROKES,
} from "../entry/src/main/ets/keyboard/input/HandwritingStrokePolicy";
import {
  HandwritingRecognitionQueue,
  HandwritingRecognitionTicket,
} from "../entry/src/main/ets/keyboard/input/HandwritingRecognitionQueue";
import { KeyboardFormFactorPolicy } from "../entry/src/main/ets/keyboard/KeyboardFormFactorPolicy";
import { SettingsFormFactorCapabilities } from "../entry/src/main/ets/keyboard/settings/SettingsFormFactorCapabilities";
import { SymbolPanelPolicy } from "../entry/src/main/ets/keyboard/input/SymbolPanelPolicy";
import {
  BackspaceHoldAction,
  BackspaceHoldPolicy,
  HardwareBackspaceGuard,
} from "../entry/src/main/ets/keyboard/input/BackspaceHoldPolicy";
import {
  CompositionBoundary,
  CompositionBoundaryAction,
  CompositionBoundaryPolicy,
} from "../entry/src/main/ets/keyboard/input/CompositionBoundaryPolicy";
import {
  VoiceRecognitionPolicy,
  VOICE_MAX_TEXT,
} from "../entry/src/main/ets/keyboard/input/VoiceRecognitionPolicy";
import {
  DEFAULT_VOICE_INPUT_CONFIGURATION,
  VoiceInputConfiguration,
  VoiceInputConfigurationPolicy,
} from "../entry/src/main/ets/keyboard/input/VoiceInputConfiguration";
import {
  AccountCloudBridge,
  AccountSessionStore,
  AccountTransport,
  AccountTransportResponse,
  MAX_DICTIONARY_EXPORT_BYTES,
  MAX_SNAPSHOT_DOWNLOAD_BYTES,
  dictionaryChangePageChanged,
} from "../entry/src/main/ets/account/AccountCloudBridge";
import {
  AiSkinCancelled,
  AiSkinFailure,
  AiSkinRun,
  AiSkinRunner,
  ArtworkJob,
} from "../entry/src/main/ets/account/AiSkinRunPolicy";
import {
  AccountPreferenceError,
  AccountPreferenceSchema,
  AccountPreferences,
  applyAccountPreferences,
  localAccountPreferences,
  mergeAccountPreferences,
} from "../entry/src/main/ets/account/AccountPreferencePlan";
import { TypingStatisticsPolicy } from "../entry/src/main/ets/keyboard/TypingStatisticsPolicy";
import { OnlineCandidatePolicy } from "../entry/src/main/ets/keyboard/candidate/OnlineCandidatePolicy";
import {
  TranslationPolicy,
  TranslationQuery,
  TranslationEntry,
} from "../entry/src/main/ets/keyboard/candidate/TranslationPolicy";
import { TranslationSensePolicy } from "../entry/src/main/ets/keyboard/candidate/TranslationSensePolicy";
import {
  HardwareKeyRouter,
  HardwareKeyAction,
  HardwareKey,
  HardwareKeyDecision,
  HardwareSpelling,
  PLAIN_SPELLING,
} from "../entry/src/main/ets/keyboard/HardwareKeyRouter";
import {
  CandidateTextPolicy,
  CandidateTextEdge,
} from "../entry/src/main/ets/keyboard/input/CandidateTextPolicy";
import { CandidateSkinPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateSkinPolicy";
import { CandidateNumberFontPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateNumberFontPolicy";
import { PreeditCaretPolicy } from "../entry/src/main/ets/keyboard/candidate/PreeditCaretPolicy";
import { CandidatePreeditStylePolicy } from "../entry/src/main/ets/keyboard/candidate/CandidatePreeditStylePolicy";
import {
  EmojiPanelKeyAction,
  EmojiPanelKeyPolicy,
} from "../entry/src/main/ets/keyboard/emoji/EmojiPanelKeyPolicy";
import { EmojiPanelTooltipPolicy } from "../entry/src/main/ets/keyboard/emoji/EmojiPanelTooltipPolicy";
import {
  CandidateTranslationStyle,
  TRANSLATION_OPACITY,
} from "../entry/src/main/ets/keyboard/candidate/CandidateTranslationStyle";
import {
  CandidateContextMenuPolicy,
  PointerAction,
  PointerButton,
} from "../entry/src/main/ets/keyboard/input/CandidateContextMenuPolicy";
import { PanelSurfaceAction } from "../entry/src/main/ets/keyboard/input/PanelShortcutPolicy";
import { DesktopSurface } from "../entry/src/main/ets/keyboard/DesktopSurface";
import { SurfaceRoutingPolicy } from "../entry/src/main/ets/keyboard/SurfaceRoutingPolicy";
import { PreferenceRevisionPolicy } from "../entry/src/main/ets/keyboard/input/PreferenceRevisionPolicy";
import { PreferencesErrorCode } from "../entry/src/main/ets/keyboard/settings/PreferencesErrorCode";
import { LocalVoiceModelPolicy } from "../entry/src/main/ets/keyboard/settings/LocalVoiceModelPolicy";
import {
  AiCatalogPage,
  AiModelCatalogPolicy,
} from "../entry/src/main/ets/keyboard/settings/AiModelCatalogPolicy";
import { HttpAsrConfigurationPolicy } from "../entry/src/main/ets/keyboard/input/HttpAsrConfigurationPolicy";
import { SkinImportPolicy } from "../entry/src/main/ets/keyboard/skin/SkinImportPolicy";
import {
  SmartPunctuationSpacePolicy,
  SpaceConvertDecision,
} from "../entry/src/main/ets/keyboard/input/SmartPunctuationSpacePolicy";
import {
  PanelShortcut,
  PanelShortcutPolicy,
} from "../entry/src/main/ets/keyboard/input/PanelShortcutPolicy";
import {
  CandidateSkinCatalogPolicy,
  CandidateSkinPackage,
} from "../entry/src/main/ets/keyboard/candidate/CandidateSkinCatalogPolicy";
import { CandidateWidthPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateWidthPolicy";
import { CandidatePresentationPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidatePresentationPolicy";
import { CandidateWheelPolicy } from "../entry/src/main/ets/keyboard/candidate/CandidateWheelPolicy";
import {
  CandidateAnchorPolicy,
  CandidateAnchor,
} from "../entry/src/main/ets/inputmethodextability/CandidateAnchorPolicy";
import {
  FloatingToolbarLayout,
  ToolbarButton,
  ToolbarComponents,
} from "../entry/src/main/ets/keyboard/FloatingToolbarLayout";
import { FloatingToolbarDragPolicy } from "../entry/src/main/ets/keyboard/FloatingToolbarDragPolicy";
import { InlinePreeditPolicy } from "../entry/src/main/ets/keyboard/input/InlinePreeditPolicy";

function selectedBarVisible(value: boolean | null): boolean {
  return value !== false;
}

let failures = 0;
let checks = 0;

/** Kept local so the suite needs no node type definitions, which this repository does not carry. */
function assertOk(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function assertThrows(body: () => void, pattern: RegExp, message: string): void {
  try {
    body();
  } catch (error) {
    const text = error instanceof Error ? error.message : String(error);
    if (!pattern.test(text)) {
      throw new Error(`${message}: threw "${text}", expected ${pattern}`);
    }
    return;
  }
  throw new Error(`${message}: nothing was thrown`);
}

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  assertOk(condition, message);
}

console.log("KeyboardGeometry");

console.log("DictionaryMaintenancePolicy");

console.log("HandwritingStrokePolicy");

console.log("KeyboardFormFactorPolicy");

console.log("VoiceRecognitionPolicy");

group("maps Harmony commits to shared typing-statistics sources", () => {
  check(
    TypingStatisticsPolicy.source("quanpin", "xiaohe", false, false, "none") === "quanpin",
    "quanpin uses the shared source id",
  );
  check(
    TypingStatisticsPolicy.source("quanpin", "xiaohe", false, true, "none") === "nineKey",
    "nine-key quanpin has its own source id",
  );
  check(
    TypingStatisticsPolicy.source("shuangpin", "microsoft", false, false, "none") === "microsoft",
    "shuangpin profile is retained",
  );
  check(
    TypingStatisticsPolicy.source("wubi", "xiaohe", false, false, "none") === "wubi",
    "wubi uses the shared source id",
  );
  check(
    TypingStatisticsPolicy.source("quanpin", "xiaohe", true, false, "none") === "english",
    "dedicated English takes precedence",
  );
  check(
    TypingStatisticsPolicy.source("quanpin", "xiaohe", false, false, "emoji") === "local",
    "local modes are attributed as local input",
  );
  check(
    TypingStatisticsPolicy.source("quanpin", "xiaohe", false, false, "temporary_japanese") ===
      "japanese",
    "temporary Japanese retains its language source",
  );
  check(
    TypingStatisticsPolicy.day(new Date(2026, 8, 19)) === "2026-09-19",
    "day keys use the native local calendar date",
  );
  check(
    TypingStatisticsPolicy.hour(new Date(2026, 8, 19, 0, 30)) === 0 &&
      TypingStatisticsPolicy.hour(new Date(2026, 8, 19, 23, 59)) === 23,
    "hour buckets use the same local calendar as the day beside them",
  );
});

group("bounds and deduplicates asynchronous online AI candidates", () => {
  const response = JSON.stringify({
    choices: [
      {
        message: {
          content: JSON.stringify({
            candidates: [
              { text: "你好" },
              { text: "你好" },
              { text: "世界" },
              { text: "bad\ntext" },
            ],
          }),
        },
      },
    ],
  });
  const values = OnlineCandidatePolicy.aiCandidates(response, 3);
  check(
    values !== null && values.length === 2 && values[0] === "你好" && values[1] === "世界",
    "AI response keeps provider order and removes duplicates or controls",
  );
  check(
    OnlineCandidatePolicy.aiCandidates(JSON.stringify({ error: { code: "bad" } }), 3) === null,
    "AI error envelope is rejected",
  );
  check(
    OnlineCandidatePolicy.aiCandidates(response, 0) === null,
    "AI candidate limit stays within shared bounds",
  );
  check(
    OnlineCandidatePolicy.aiCandidates("x".repeat(1024 * 1024 + 1), 3) === null,
    "oversized AI response is rejected before parsing",
  );
  check(
    OnlineCandidatePolicy.acceptsCloudBody("你".repeat(128 * 1024)) === false,
    "oversized UTF-8 cloud responses are rejected by byte size",
  );
});

group("keeps translation provider policy bounded and credential-free in signatures", () => {
  const query: TranslationQuery = {
    generation: 12,
    target_language: "en",
    target_languages: ["en", "ja"],
    candidates: [{ text: "你好" }, { text: "你好" }],
    custom_translation: null,
    tencent_tmt: null,
    niutrans: { enabled: true, app_id: "account", apikey: "secret" },
    english_gloss: true,
    resources: "/data/resources",
    user_data: "/data/state",
  };
  check(TranslationPolicy.provider(query) === "niutrans", "NiuTrans has provider precedence");
  check(TranslationPolicy.targets(query).join(",") === "en,ja", "targets are deduplicated");
  check(
    !TranslationPolicy.signature(query).includes("secret"),
    "provider signatures never contain credentials",
  );
  check(
    TranslationPolicy.cacheKey(query, "en", {
      text: "你好",
      key: "你好",
      source_language: "zh",
      target_language: "en",
    }).includes("niutrans:account"),
    "cache scope identifies the provider account",
  );
});

group("merges translation rows without unbounded display growth", () => {
  const entries: TranslationEntry[] = [];
  TranslationPolicy.append(entries, "你好", "hello");
  TranslationPolicy.append(entries, "你好", "hello");
  TranslationPolicy.append(entries, "你好", "greeting");
  TranslationPolicy.append(entries, "世界", "\u0000bad");
  check(
    entries.length === 1 && entries[0].translation === "hello / greeting",
    "rows deduplicate and join in provider order",
  );
  TranslationPolicy.append(entries, "你好", "x".repeat(5000));
  check(entries[0].translation === "hello / greeting", "oversized glosses are ignored");
});

group("offline dictionaries fill only what the user's own translator left", () => {
  const query: TranslationQuery = {
    generation: 3,
    target_language: "en",
    target_languages: ["ja", "fr"],
    candidates: [{ text: "你好" }, { text: "世界" }],
    english_gloss: true,
    offline_gloss_languages: ["en", "fr", "de"],
  };
  check(
    TranslationPolicy.offlineTargets(query).join(",") === "fr",
    "only installed non-English targets are read offline",
  );
  check(
    TranslationPolicy.offlineTargets({ ...query, offline_gloss_languages: undefined }).length === 0,
    "a query without installed dictionaries reads none",
  );
  const answered: TranslationEntry[] = [{ text: "你好", translation: "salut" }];
  TranslationPolicy.fill(answered, [
    { text: "你好", translation: "bonjour" },
    { text: "世界", translation: "monde" },
  ]);
  check(
    answered.length === 2 &&
      answered[0].translation === "salut" &&
      answered[1].translation === "monde",
    "the online answer stays and the dictionary fills the unanswered candidate",
  );
});

group("bounds native speech language, session and result text", () => {
  check(VoiceRecognitionPolicy.language("  ") === "zh-CN", "voice defaults to Chinese");
  check(VoiceRecognitionPolicy.language("x".repeat(100)).length <= 32, "voice language is bounded");
  check(
    VoiceRecognitionPolicy.sessionId(12) === "msime-voice-12",
    "voice session ids are deterministic",
  );
  check(
    VoiceRecognitionPolicy.result(" 水\n水\u0000 ") === "水\n水",
    "voice result removes control bytes and trims",
  );
  check(
    VoiceRecognitionPolicy.result("x".repeat(VOICE_MAX_TEXT + 20)).length === VOICE_MAX_TEXT,
    "voice result is bounded",
  );
});

group("voice input preference gates every Harmony entry point", () => {
  check(DEFAULT_VOICE_INPUT_CONFIGURATION.enabled, "voice input is enabled by default");
  check(
    VoiceInputConfigurationPolicy.enabled(undefined),
    "legacy documents without the field remain enabled",
  );
  check(!VoiceInputConfigurationPolicy.enabled(false), "an explicit false disables voice input");
  check(VoiceInputConfigurationPolicy.enabled(true), "an explicit true enables voice input");
});

group("keeps desktop-only chrome off touch devices", () => {
  check(KeyboardFormFactorPolicy.isDesktop("2in1"), "a 2-in-1 gets the desktop candidate window");
  check(!KeyboardFormFactorPolicy.isDesktop("phone"), "a phone keeps the touch keyboard");
  check(!KeyboardFormFactorPolicy.isDesktop("tablet"), "a tablet keeps the touch keyboard");
  check(!KeyboardFormFactorPolicy.isDesktop("default"), "an unknown form factor fails closed");
  check(!KeyboardFormFactorPolicy.isDesktop(null), "missing device information fails closed");
});

group("routes voice shortcuts to the matching form-factor surface", () => {
  check(
    SurfaceRoutingPolicy.voiceSurface(true) === DesktopSurface.VOICE,
    "2-in-1 voice shortcuts use the candidate window voice face",
  );
  check(
    SurfaceRoutingPolicy.voiceSurface(false) === null,
    "phone voice shortcuts stay on the touch keyboard face",
  );
});

group("projects the same form factor into every settings capability", () => {
  const desktop = SettingsFormFactorCapabilities.resolve("2in1");
  check(!desktop.mobileSettings, "2-in-1 settings use the desktop surface");
  check(
    desktop.panelWindows && desktop.floatingToolbar && desktop.floatingToolbarComponents,
    "2-in-1 settings expose candidate and toolbar controls",
  );
  check(
    desktop.floatingToolbarHandwriting && desktop.floatingToolbarVoice,
    "2-in-1 settings offer the pad and microphone switches its toolbar honours",
  );
  check(
    desktop.modeSwitchShortcuts && desktop.panelShortcuts && desktop.numberRowSelection,
    "2-in-1 settings expose physical-keyboard shortcuts",
  );

  const phone = SettingsFormFactorCapabilities.resolve("phone");
  check(phone.mobileSettings, "phone settings use the touch surface");
  check(
    !phone.panelWindows &&
      !phone.floatingToolbar &&
      !phone.floatingToolbarAppearance &&
      !phone.floatingToolbarComponents &&
      !phone.floatingToolbarHandwriting &&
      !phone.floatingToolbarVoice,
    "phone settings hide candidate and toolbar controls",
  );
  check(
    !phone.panelShortcuts && !phone.candidateFollowCursor && !phone.inputModeHud,
    "phone settings hide the controls only a candidate window uses",
  );
  // An attached keyboard is routed on a phone too: the mode chords are bound on every device, and number-row selection and the voice hotkeys have no desktop check. Their switches have to be reachable wherever they act.
  check(
    phone.modeSwitchShortcuts && phone.numberRowSelection && phone.voiceHotkeys,
    "phone settings offer the hardware-keyboard switches the keyboard still acts on",
  );
  check(desktop.voiceHotkeys, "2-in-1 settings offer the voice hotkeys");
  // The phone strip is always horizontal, so a layout select there is a control that does nothing; the 2in1 candidate window keeps the choice.
  check(
    phone.fixedCandidateLayout === "horizontal",
    "the phone's candidate layout is fixed horizontal",
  );
  check(desktop.fixedCandidateLayout === null, "the 2-in-1 candidate layout stays a choice");
});

group("SymbolPanelPolicy", () => {
  const symbolCategories = SymbolPanelPolicy.categories();
  check(
    symbolCategories.map((category) => category.id).join(",") ===
      "common,chinese,english,number,network",
    "the symbol panel keeps its stable category order",
  );
  check(
    symbolCategories.map((category) => category.symbols.length).join(",") === "30,50,40,50,30",
    "the complete committed symbol catalog is present",
  );
  check(
    symbolCategories.every((category) => category.title.length > 0 && category.symbols.length > 0),
    "every symbol panel category has a title and entries",
  );
  check(
    symbolCategories.every(
      (category) => category.symbols.length <= SymbolPanelPolicy.maxSymbolsPerCategory(),
    ),
    "the symbol panel bounds each category",
  );
  check(
    symbolCategories.every((category) =>
      category.symbols.every((symbol) => SymbolPanelPolicy.isValidSymbol(symbol)),
    ),
    "the symbol panel rejects no catalog symbol",
  );
  check(
    SymbolPanelPolicy.select(false).returnToKeyboard && !SymbolPanelPolicy.select(false).locked,
    "a normal symbol tap returns to the keyboard",
  );
  check(
    !SymbolPanelPolicy.select(true).returnToKeyboard && SymbolPanelPolicy.select(true).locked,
    "a locked symbol panel stays open for continuous input",
  );
});

group("a held Delete never crosses from composition into committed text", () => {
  check(
    BackspaceHoldPolicy.firstRepeat(true) === BackspaceHoldAction.CANCEL_COMPOSITION,
    "the first repeat clears an unfinished composition as one operation",
  );
  check(
    BackspaceHoldPolicy.firstRepeat(false) === BackspaceHoldAction.DELETE,
    "without a composition the hold keeps deleting editor text",
  );
  check(
    BackspaceHoldPolicy.deletesEditor(false),
    "an unhandled backspace falls through to the editor",
  );
  check(
    !BackspaceHoldPolicy.deletesEditor(true),
    "a handled backspace never also deletes editor text",
  );
});

group("composition boundaries preserve Japanese as kana", () => {
  check(
    CompositionBoundaryPolicy.action(false, true, CompositionBoundary.DEACTIVATE) ===
      CompositionBoundaryAction.NONE,
    "an idle boundary sends no Engine command",
  );
  check(
    CompositionBoundaryPolicy.action(true, false, CompositionBoundary.MODE_SWITCH) ===
      CompositionBoundaryAction.COMMIT_RAW,
    "Chinese spelling keeps the established raw-commit boundary",
  );
  check(
    CompositionBoundaryPolicy.action(true, true, CompositionBoundary.MODE_SWITCH) ===
      CompositionBoundaryAction.FINISH_COMPOSITION,
    "Japanese finishes kana instead of exposing its romaji strokes",
  );
  check(
    CompositionBoundaryPolicy.action(true, false, CompositionBoundary.DEACTIVATE) ===
      CompositionBoundaryAction.FINISH_COMPOSITION,
    "deactivation finishes the highlighted composition before Runtime focus cancellation",
  );
});

group("bounds handwriting points and rejects empty recognition requests", () => {
  const point = HandwritingStrokePolicy.point(999, -4);
  check(
    HANDWRITING_MAX_STROKES === 64 && HANDWRITING_MAX_POINTS === 512,
    "the Harmony canvas preserves the fixed Apple stroke and point budgets",
  );
  check(
    point.x === HANDWRITING_CANVAS_SIZE && point.y === 0,
    "handwriting points stay inside the canvas",
  );
  check(!HandwritingStrokePolicy.canRecognize([]), "empty ink does not trigger OCR");
  check(
    HandwritingStrokePolicy.canRecognize([{ points: [point] }]),
    "a bounded stroke is recognisable",
  );
  const maximumStrokes = Array.from({ length: HANDWRITING_MAX_STROKES }, () => ({
    points: [point],
  }));
  check(
    HandwritingStrokePolicy.canRecognize(maximumStrokes),
    "the last supported stroke is accepted",
  );
  check(
    !HandwritingStrokePolicy.canRecognize([...maximumStrokes, { points: [point] }]),
    "one stroke past the source limit is refused",
  );
  const maximumPoints = Array.from({ length: HANDWRITING_MAX_POINTS }, () => point);
  check(
    HandwritingStrokePolicy.canRecognize([{ points: maximumPoints }]),
    "the last supported point in a stroke is accepted",
  );
  check(
    !HandwritingStrokePolicy.canRecognize([{ points: [...maximumPoints, point] }]),
    "one point past the source limit is refused",
  );
});

group("normalizes OCR candidates without leaking control text or duplicates", () => {
  const candidates = HandwritingStrokePolicy.candidates(" 水\n水\u0000永木未未 ");
  check(candidates.join("") === "水永木未", "OCR candidates are unique and trimmed");
  check(
    HANDWRITING_MAX_CANDIDATES === 12,
    "the Harmony recognizer preserves all twelve alternatives from the fixed Apple source",
  );
  const fullCandidateSet = "甲乙丙丁戊己庚辛壬癸子丑";
  check(
    HandwritingStrokePolicy.candidates(fullCandidateSet).length === HANDWRITING_MAX_CANDIDATES,
    "OCR candidates are bounded without dropping the final four alternatives",
  );
  check(
    HandwritingStrokePolicy.candidates("甲乙丙", 0).length === 0 &&
      HandwritingStrokePolicy.candidates("甲乙丙", -1).length === 0,
    "zero and negative candidate limits do not leak one result",
  );
  check(
    HandwritingStrokePolicy.candidates(fullCandidateSet + "寅卯", 99).length ===
      HANDWRITING_MAX_CANDIDATES,
    "custom candidate limits cannot exceed the platform cap",
  );
});

group("new handwriting stays writable while an older OCR request runs", () => {
  const queue = new HandwritingRecognitionQueue();
  queue.changed();
  const first: HandwritingRecognitionTicket | null = queue.request();
  check(first !== null, "the first settled canvas starts recognition");
  queue.changed();
  check(first !== null && !queue.accepts(first), "a new stroke invalidates the old OCR result");
  check(queue.request() === null, "platform OCR remains serial while the old request finishes");
  queue.changed();
  check(queue.request() === null, "more strokes collapse into the same pending request");
  const latest: HandwritingRecognitionTicket | null = queue.finish();
  check(
    latest !== null && queue.accepts(latest),
    "completion immediately starts the latest canvas",
  );
  check(queue.finish() === null, "the queue drains after the newest canvas is recognised");
});

group("allows reads during composition without restarting the session", () => {
  const decision = DictionaryMaintenancePolicy.decide("list", true);
  check(decision.allowed, "dictionary reads remain available while composing");
  check(!decision.maintenance, "dictionary reads do not request maintenance");
});

group("opens an exclusive window for idle mutations", () => {
  for (const operation of ["edit", "import", "retry", "dismiss_failure"]) {
    const decision = DictionaryMaintenancePolicy.decide(operation, false);
    check(decision.allowed, `${operation} is allowed while idle`);
    check(decision.maintenance, `${operation} is marked as maintenance`);
  }
});

group("refuses every mutation while composition is active", () => {
  for (const operation of ["edit", "import", "retry", "dismiss_failure"]) {
    const decision = DictionaryMaintenancePolicy.decide(operation, true);
    check(!decision.allowed, `${operation} is refused while composing`);
    check(decision.maintenance, `${operation} remains classified as maintenance`);
    check(decision.error === "dictionary maintenance busy", `${operation} reports the busy state`);
  }
});

group("personal imports and cloud downloads are queued while the keyboard is open", () => {
  // The user chooses when to import, and they are as likely to do it with the keyboard up as with
  // it down. "dictionary maintenance busy" is not an answer to "add these words".
  for (const operation of ["import_personal", "queue_edit"]) {
    for (const composing of [false, true]) {
      const decision = DictionaryMaintenancePolicy.decide(operation, composing);
      check(decision.queued, `${operation} goes to the queue (composing: ${composing})`);
      check(decision.allowed, `and is allowed (composing: ${composing})`);
      // It writes its own file and never the Engine, so there is nothing to take a window for and
      // nothing for a live session to be stopped over.
      check(!decision.maintenance, `without asking for the Engine (composing: ${composing})`);
      check(decision.error === "", `and without a refusal (composing: ${composing})`);
    }
  }
  // Ordinary settings edits still go directly to the Engine: the user is watching the list beside
  // that form, and queueing them would look exactly like the edit having been lost.
  for (const operation of ["list", "edit", "import", "export", "retry", "dismiss_failure"]) {
    check(
      !DictionaryMaintenancePolicy.decide(operation, false).queued,
      `${operation} still goes to the Engine`,
    );
  }
});

group("polishing acts on what is in front of the caret, and only if it still is", () => {
  // HarmonyOS gives an input method no way to read a selection, so the Apple gesture — highlight,
  // then polish — has nothing to act on here. The text before the caret is what this platform does
  // offer, and on a phone it is also the more natural gesture: type, then tidy.
  check(
    AiPolishPolicy.source("今天天气不错啊") === "今天天气不错啊",
    "the preceding text is the source",
  );
  // A trailing newline is the user finishing a line, not part of the sentence they want rewritten.
  check(
    AiPolishPolicy.source("今天天气不错啊\n\n") === "今天天气不错啊",
    "trailing whitespace is dropped",
  );
  check(AiPolishPolicy.source("") === "", "an empty editor offers nothing");
  check(AiPolishPolicy.source("   ") === "", "and neither does whitespace alone");
  check(AiPolishPolicy.source("好") === "", "nor a fragment too short to be worth a request");
  const long = "字".repeat(MAX_POLISH_SOURCE_CHARACTERS + 50);
  // Bounded from the end: the sentence next to the caret is the one being written.
  check(
    codePointLength(AiPolishPolicy.source(long)) === MAX_POLISH_SOURCE_CHARACTERS,
    "an overlong context is bounded",
  );

  check(!AiPolishPolicy.usable("今天天气不错", ""), "an empty rewrite is not a result");
  check(!AiPolishPolicy.usable("今天天气不错", "   "), "and neither is whitespace");
  check(
    !AiPolishPolicy.usable("今天天气不错", "今天天气不错"),
    "a rewrite identical to the source is not worth offering",
  );
  check(AiPolishPolicy.usable("今天天气不错", "今天天气很好。"), "a genuine rewrite is");
  // Polish output is prose and may be written in paragraphs. This is the reason it does not go
  // through the AI candidate parser beside it, which rejects every control character.
  check(
    AiPolishPolicy.usable("要点一 要点二", "1. 要点一\n2. 要点二"),
    "a multi-line rewrite is a result, not a refusal",
  );

  const source = "今天天气不错";
  const result = "今天天气很好。";
  // The editor is re-read at the moment of replacing. A request takes seconds, and replacing after
  // the user has typed would delete what they just wrote and put the rewrite of something else in.
  const ready = AiPolishPolicy.replacement(source, result, `你好，${source}`);
  check(ready !== null, "an unchanged editor can be replaced");
  check(ready?.deleteCount === codePointLength(source), "deleting exactly the source");
  check(ready?.insert === result, "and inserting the rewrite");
  check(
    AiPolishPolicy.replacement(source, result, `${source}后来又下雨了`) === null,
    "text typed after the source refuses the replacement",
  );
  check(
    AiPolishPolicy.replacement(source, result, "完全不同的内容") === null,
    "and so does a different editor entirely",
  );
  check(AiPolishPolicy.replacement("", result, "") === null, "an empty source replaces nothing");

  // Counted in code points, not UTF-16 units: an emoji is one thing the user sees deleted and two
  // units of JavaScript string. The unit `deleteBackwardSync` takes is the one open question here,
  // which is why the re-read above stands between a wrong answer and a corrupted message.
  const withEmoji = "今天天气不错🙂";
  check(withEmoji.length === 8, "the source is eight UTF-16 units");
  check(codePointLength(withEmoji) === 7, "and seven code points");
  check(
    AiPolishPolicy.replacement(withEmoji, result, withEmoji)?.deleteCount === 7,
    "the delete count follows the code points",
  );
});

group("a key says what it does, not what it draws", () => {
  // A key face is as short as it can be and often not a word: read aloud, `⇧` is nothing, `123` is
  // a number and `中` is a character rather than an action. The source names every one of them.
  check(KeyAccessibilityPolicy.symbol("；") === "符号 ；", "a symbol is read as a symbol");
  check(KeyAccessibilityPolicy.symbol("") === "符号", "and a blank one still says what it is");
  check(KeyAccessibilityPolicy.delete() === "删除", "the delete glyph gets a word");
  check(KeyAccessibilityPolicy.language() === "切换中英文", "the language key names the action");
  check(
    KeyAccessibilityPolicy.layoutToggle(false) === "切换到数字和符号",
    "the layout toggle names where it goes",
  );
  check(
    KeyAccessibilityPolicy.layoutToggle(true) === "切换到所选输入方案",
    "and names the other direction when it is pointing back",
  );
  check(KeyAccessibilityPolicy.punctuation() === "常用标点", "the comma key names its long press");
  check(
    KeyAccessibilityPolicy.symbolPanel() === "符号面板",
    "the compact symbol key names its panel",
  );
});

group("every tool in the shortcut bar has a name", () => {
  // The bar is drawn entirely in icons, so a button with no name is announced as nothing at all.
  // The eighth tool is conditional, but needs a stable name when the thoughtful-reply scheme adds
  // it to the same bar.
  const names: string[] = [
    KeyAccessibilityPolicy.tools(),
    KeyAccessibilityPolicy.emoji(),
    KeyAccessibilityPolicy.voice(),
    KeyAccessibilityPolicy.reply(),
    KeyAccessibilityPolicy.skin(),
    KeyAccessibilityPolicy.scheme(),
    KeyAccessibilityPolicy.geometry(),
    KeyAccessibilityPolicy.dismiss(),
  ];
  check(names.length === 8, "all eight possible buttons have names");
  for (const name of names) {
    check(name.trim().length > 0, "no button is left nameless");
    check(
      !/[←-⯿️\u{1F300}-\u{1FAFF}]/u.test(name),
      "a name is words, not the glyph the button draws",
    );
  }
  check(new Set(names).size === names.length, "and no two buttons answer to the same name");

  // Mid-composition space selects the highlighted candidate. Someone who cannot see the candidate
  // row has no other way to know the key changed meaning under them.
  check(KeyAccessibilityPolicy.space(false) === "空格", "space is space when nothing is composing");
  check(KeyAccessibilityPolicy.space(true) === "选定", "and says so when it will select instead");

  // The return key's face is already a whole word, so the spoken name is that word rather than a
  // second description of it.
  check(KeyAccessibilityPolicy.returnKey("搜索") === "搜索", "return reads as what it will do");
  check(KeyAccessibilityPolicy.returnKey("") === "换行", "with a fallback when the face is empty");

  const plain = KeyAccessibilityPolicy.candidate(1, "你好", "", "");
  check(plain === "候选词 1：你好", "a candidate is numbered as it is shown");
  // The remaining spelling turns "this word" into "this word, if you keep going" — the difference
  // between a candidate that commits now and one that does not.
  check(
    KeyAccessibilityPolicy.candidate(2, "你", "hao", "") === "候选词 2：你，还需输入 hao",
    "and says what is still to be typed",
  );
  check(
    KeyAccessibilityPolicy.candidate(3, "绿", "", "，英文释义：green") ===
      "候选词 3：绿，英文释义：green",
    "the gloss is appended as the ported policy words it",
  );
  check(
    KeyAccessibilityPolicy.englishSuggestion(2, "hello") === "英文补全 2：hello",
    "english completions get a spoken candidate label",
  );
  check(
    KeyAccessibilityPolicy.spelling("ni hao") === "选择拼音 ni hao",
    "nine-key spellings describe their selectable action",
  );
});

group("a report says which release it came from, or says nothing", () => {
  // The page prints the platform's own name in front of this, so the product name is dropped:
  // "HarmonyOS 6.0.1.115" rather than "HarmonyOS OpenHarmony-6.0.1.115".
  check(
    OsVersionPolicy.release("OpenHarmony-6.0.1.115") === "6.0.1.115",
    "the product name in front of the version is dropped",
  );
  check(
    OsVersionPolicy.release("HarmonyOS-6.0.1.115") === "6.0.1.115",
    "whatever that product name happens to be",
  );
  check(OsVersionPolicy.release("6.0.1.115") === "6.0.1.115", "a bare version is taken whole");
  check(OsVersionPolicy.release("  6.0.1  ") === "6.0.1", "surrounding space is not part of it");

  // Nothing is invented: this goes into a report a user files, so it reports what the system says
  // or nothing at all, and the page already knows what to do with nothing.
  check(OsVersionPolicy.release(undefined) === null, "an absent value is not a version");
  check(OsVersionPolicy.release("") === null, "and neither is an empty one");
  check(
    OsVersionPolicy.release("OpenHarmony-") === null,
    "nor a product name with nothing after it",
  );
  // A word is not a release however it is formatted, and putting one where a number belongs would
  // be worse than the fallback.
  check(OsVersionPolicy.release("OpenHarmony-release") === null, "nor a version with no digit");
  check(OsVersionPolicy.release("x".repeat(65)) === null, "an implausibly long value is refused");
  check(OsVersionPolicy.release("6.0\u00001") === null, "and so is one with a control character");
});
group("spacing clamps to its range and falls back on a negative", () => {
  check(
    KeyboardGeometry.keySpacing(-1) === KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
    "a negative key spacing takes the default rather than the minimum",
  );
  check(
    KeyboardGeometry.keySpacing(10) === KeyboardGeometry.MIN_KEY_SPACING_TENTHS,
    "below-range key spacing clamps up",
  );
  check(
    KeyboardGeometry.keySpacing(999) === KeyboardGeometry.MAX_KEY_SPACING_TENTHS,
    "above-range key spacing clamps down",
  );
  check(
    KeyboardGeometry.rowSpacing(-1) === KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
    "a negative row spacing takes the default",
  );
  check(
    KeyboardGeometry.rowSpacing(10) === KeyboardGeometry.MIN_ROW_SPACING_TENTHS,
    "below-range row spacing clamps up",
  );
  check(
    KeyboardGeometry.rowSpacing(999) === KeyboardGeometry.MAX_ROW_SPACING_TENTHS,
    "above-range row spacing clamps down",
  );
});

group("an unset height adjustment is the default, not the minimum", () => {
  check(
    KeyboardGeometry.heightAdjustment(null) === KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP,
    "null stands for the Java Integer.MIN_VALUE sentinel",
  );
  check(
    KeyboardGeometry.heightAdjustment(-100) === KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP,
    "a set value below the range still clamps",
  );
  check(
    KeyboardGeometry.heightAdjustment(100) === KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP,
    "a set value above the range clamps",
  );
});

group("row heights spend the whole adjustment without losing a pixel", () => {
  const base = KeyboardGeometry.STANDARD_ROW_HEIGHT_VP;
  for (const adjustment of [0, 1, 7, -5, 47, 48]) {
    for (const rowCount of [1, 3, 4, 5]) {
      let sum = 0;
      for (let row = 0; row < rowCount; row++) {
        sum += KeyboardGeometry.adjustedRowHeight(base, adjustment, rowCount, row);
      }
      const expected = base * rowCount + KeyboardGeometry.heightAdjustment(adjustment);
      check(
        sum === expected,
        `rows must sum to ${expected} for adjustment ${adjustment} across ${rowCount} rows, got ${sum}`,
      );
    }
  }
});

group("candidate window height follows the shared layout orientation", () => {
  const horizontal = KeyboardMetrics.candidateHeightVp("horizontal", 9);
  const vertical = KeyboardMetrics.candidateHeightVp("vertical", 9);
  check(
    horizontal === KeyboardMetrics.candidateHeightVp("horizontal", 1),
    "horizontal candidates stay a single row",
  );
  check(vertical > horizontal, "vertical candidates get room for their page");
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 0) ===
      KeyboardMetrics.candidateHeightVp("vertical", 1),
    "an empty page keeps one row",
  );
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 99) === vertical,
    "vertical height is bounded to one candidate page",
  );
  check(
    KeyboardMetrics.candidateHeightVp("horizontal", 1, false) ===
      horizontal - KeyboardMetrics.COMPOSITION_ROW_HEIGHT_VP,
    "hidden preedit removes the composition row from panel height",
  );
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 2, true, 24) ===
      KeyboardMetrics.candidateHeightVp("vertical", 2) + 24,
    "candidate decoration reserves its top inset in the panel height",
  );
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 2, true, 9999) ===
      KeyboardMetrics.candidateHeightVp("vertical", 2) + 512,
    "candidate decoration height stays bounded",
  );
});

group("invalid geometry is rejected rather than silently clamped", () => {
  assertThrows(
    () => KeyboardGeometry.adjustedRowHeight(0, 0, 3, 0),
    /Invalid keyboard height/,
    "rejects invalid input",
  );
  assertThrows(
    () => KeyboardGeometry.adjustedRowHeight(48, 0, 0, 0),
    /Invalid keyboard height/,
    "rejects invalid input",
  );
  assertThrows(
    () => KeyboardGeometry.adjustedRowHeight(48, 0, 3, 3),
    /Invalid keyboard height/,
    "rejects invalid input",
  );
  assertThrows(
    () => KeyboardGeometry.adjustedRowHeight(48, 0, 3, -1),
    /Invalid keyboard height/,
    "rejects invalid input",
  );
  checks += 4;
});

group("display strings match the Java formatting", () => {
  check(KeyboardGeometry.display(60) === "6.0", "tenths render with one decimal");
  check(KeyboardGeometry.display(35) === "3.5", "tenths render the fraction");
  check(KeyboardGeometry.displayHeight(5) === "+5", "a positive adjustment carries a sign");
  check(KeyboardGeometry.displayHeight(-5) === "-5", "a negative adjustment keeps its own sign");
  check(KeyboardGeometry.displayHeight(0) === "0", "zero carries no sign");
  check(KeyboardGeometry.halfGapPixels(60, 3) === 9, "half gap rounds to whole pixels");
  check(KeyboardGeometry.halfGapPixels(60, 0) === 0, "a non-positive density yields no gap");
  check(KeyboardGeometry.halfGapPixels(60, Number.NaN) === 0, "a non-finite density yields no gap");
});

group("layout adjustment follows the first drag axis", () => {
  check(
    KeyboardLayoutDragPolicy.axis(20, 5) === KeyboardLayoutDragAxis.KEY_SPACING,
    "a mostly horizontal drag adjusts key spacing",
  );
  check(
    KeyboardLayoutDragPolicy.axis(5, 20) === KeyboardLayoutDragAxis.ROW_SPACING,
    "a mostly vertical drag adjusts row spacing",
  );
  check(
    KeyboardLayoutDragPolicy.axis(10, 10) === KeyboardLayoutDragAxis.ROW_SPACING,
    "a diagonal tie follows the source's vertical preference",
  );
  check(
    KeyboardLayoutDragPolicy.keySpacing(40, 18) === 50,
    "eighteen vp moves key spacing by one visible point",
  );
  check(
    KeyboardLayoutDragPolicy.rowSpacing(60, -18) === 50,
    "row spacing uses the same scaled gesture",
  );
  check(
    KeyboardLayoutDragPolicy.height(0, -12) === 12,
    "dragging the top edge upward increases keyboard height one-for-one",
  );
  check(
    KeyboardLayoutDragPolicy.height(48, -100) === KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP,
    "height remains inside the shared preference bounds",
  );
});

console.log("CandidateWrapPolicy");

group("a single candidate never wraps, however wide", () => {
  const rows = CandidateWrapPolicy.rows(100, 4, [400]);
  check(rows.length === 1 && rows[0] === 0, "an overlong first candidate stays on row zero");
});

group("candidates wrap when the row plus spacing would overflow", () => {
  const rows = CandidateWrapPolicy.rows(100, 10, [40, 40, 40]);
  // 40, then 40+10+40 = 90 fits, then 90+10+40 = 140 overflows.
  check(
    rows[0] === 0 && rows[1] === 0 && rows[2] === 1,
    `expected [0,0,1], got [${rows.join(",")}]`,
  );
});

group("spacing counts toward the overflow decision", () => {
  const tight = CandidateWrapPolicy.rows(90, 10, [40, 40]);
  const loose = CandidateWrapPolicy.rows(90, 0, [40, 40]);
  check(tight[1] === 0, "40+10+40 = 90 fits exactly in 90");
  check(loose[1] === 0, "without spacing the pair fits too");
  const overflow = CandidateWrapPolicy.rows(89, 10, [40, 40]);
  check(overflow[1] === 1, "one pixel narrower and the second candidate wraps");
});

group("an empty candidate list allocates no rows", () => {
  check(CandidateWrapPolicy.rows(100, 4, []).length === 0, "no candidates means no rows");
});

group("invalid wrap dimensions are rejected", () => {
  assertThrows(
    () => CandidateWrapPolicy.rows(-1, 4, [10]),
    /Invalid candidate wrap/,
    "rejects invalid input",
  );
  assertThrows(
    () => CandidateWrapPolicy.rows(100, -1, [10]),
    /Invalid candidate wrap/,
    "rejects invalid input",
  );
  assertThrows(
    () => CandidateWrapPolicy.rows(100, 4, [-10]),
    /Invalid candidate width/,
    "rejects invalid input",
  );
  checks += 3;
});

console.log("KeyboardScheme");

group("preference ids resolve to their scheme", () => {
  check(KeyboardScheme.fromPreferenceId("quanpin") === KeyboardScheme.QUANPIN, "quanpin resolves");
  check(
    KeyboardScheme.fromPreferenceId("nine_key") === KeyboardScheme.QUANPIN_NINE_KEY,
    "nine_key is the quanpin nine-key scheme, not a layout flag",
  );
  check(KeyboardScheme.fromPreferenceId("unknown") === null, "an unknown id resolves to nothing");
  check(KeyboardScheme.fromPreferenceId(null) === null, "a null id resolves to nothing");
});

group("engine preferences map back to the right scheme", () => {
  check(
    KeyboardScheme.fromPreferences("quanpin", null, "handwriting") === KeyboardScheme.HANDWRITING,
    "handwriting is a layout on top of quanpin",
  );
  check(
    KeyboardScheme.fromPreferences("quanpin", null, "nine_key") === KeyboardScheme.QUANPIN_NINE_KEY,
    "quanpin plus nine_key is the nine-key scheme",
  );
  check(
    KeyboardScheme.fromPreferences("japanese", null, "nine_key") ===
      KeyboardScheme.JAPANESE_NINE_KEY,
    "japanese plus nine_key is the japanese nine-key scheme",
  );
  check(
    KeyboardScheme.fromPreferences("japanese", null, "twenty_six_key") === KeyboardScheme.JAPANESE,
    "japanese on a full layout is the 26-key japanese scheme",
  );
  check(
    KeyboardScheme.fromPreferences("wubi", null, "twenty_six_key") === KeyboardScheme.WUBI,
    "wubi resolves by engine scheme",
  );
  check(
    KeyboardScheme.fromPreferences("shuangpin", "ziranma", "twenty_six_key") ===
      KeyboardScheme.ZIRANMA,
    "a shuangpin profile picks its scheme",
  );
  check(
    KeyboardScheme.fromPreferences("shuangpin", "nonsense", "twenty_six_key") ===
      KeyboardScheme.XIAOHE,
    "an unknown shuangpin profile falls back to xiaohe",
  );
  check(
    KeyboardScheme.fromPreferences("nonsense", null, "twenty_six_key") === KeyboardScheme.QUANPIN,
    "an unknown scheme falls back to quanpin",
  );
});

group("enabled schemes keep the fixed order and never resolve to nothing", () => {
  const enabled = KeyboardScheme.enabledFromPreferenceIds(["wubi", "quanpin", "xiaohe"]);
  check(enabled.length === 3, "three known ids yield three schemes");
  check(
    enabled[0] === KeyboardScheme.QUANPIN &&
      enabled[1] === KeyboardScheme.XIAOHE &&
      enabled[2] === KeyboardScheme.WUBI,
    "declaration order wins over the order the ids arrived in",
  );
  const unknown = KeyboardScheme.enabledFromPreferenceIds(["nope"]);
  check(
    unknown.length === 1 && unknown[0] === KeyboardScheme.QUANPIN,
    "an all-unknown list falls back to quanpin rather than an empty keyboard",
  );
  check(
    KeyboardScheme.enabledFromPreferenceIds(null).length === KeyboardScheme.SCHEMES.length,
    "a null list means everything is enabled",
  );
});

group("selection prefers the shared choice, then the applied one", () => {
  const enabled: SchemeDefinition[] = [KeyboardScheme.QUANPIN, KeyboardScheme.WUBI];
  check(
    KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "wubi", enabled) ===
      KeyboardScheme.WUBI,
    "the shared selection is authoritative",
  );
  check(
    KeyboardScheme.resolveEnabledSelection(KeyboardScheme.WUBI, null, enabled) ===
      KeyboardScheme.WUBI,
    "without a shared selection the applied scheme is preserved",
  );
  check(
    KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "xiaohe", enabled) ===
      KeyboardScheme.QUANPIN,
    "a selection outside the enabled set falls back to the first enabled",
  );
  check(
    KeyboardScheme.resolveEnabledSelection(null, null, []) === KeyboardScheme.QUANPIN,
    "an empty enabled set still yields a usable keyboard",
  );
});

group("turning off the scheme the keyboard is on moves it somewhere it can be left", () => {
  // MSIME-Apple's DisabledSchemesAreHiddenAndCurrentSchemeFallsBack: with 日语 applied, enabling
  // only [全拼9键, 五笔] makes the stored scheme 全拼9键. Until this was read the keyboard stayed on
  // a scheme the picker no longer offered, so there was no card to leave by.
  const enabled: SchemeDefinition[] = [KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.WUBI];
  check(
    KeyboardScheme.resolveEnabledSelection(KeyboardScheme.JAPANESE_NINE_KEY, null, enabled) ===
      KeyboardScheme.QUANPIN_NINE_KEY,
    "an applied scheme that is no longer enabled falls back to the first that is",
  );
  check(
    KeyboardScheme.resolveEnabledSelection(KeyboardScheme.JAPANESE_NINE_KEY, null, []) ===
      KeyboardScheme.QUANPIN,
    "and emptying the list altogether lands on 全拼, as the source asserts",
  );

  // The write-back half. It is the same pair that decides whether anything moved, so the caller
  // cannot disagree with the resolution about whether to persist.
  check(
    KeyboardScheme.mappingForRuntimeSelection(
      KeyboardScheme.JAPANESE_NINE_KEY,
      KeyboardScheme.QUANPIN_NINE_KEY,
      "quanpin",
      "xiaohe",
    )?.touchKeyboardLayout === "nine_key",
    "a move carries the layout the destination needs",
  );
  check(
    KeyboardScheme.mappingForRuntimeSelection(
      KeyboardScheme.WUBI,
      KeyboardScheme.WUBI,
      "wubi",
      "xiaohe",
    ) === null,
    "standing still writes nothing",
  );
  check(
    KeyboardScheme.mappingForRuntimeSelection(
      KeyboardScheme.QUANPIN,
      KeyboardScheme.THOUGHTFUL_REPLY,
      "quanpin",
      "xiaohe",
    ) === null,
    "and the reply keyboard is never a destination to be persisted as a scheme",
  );
});

group("mapping keeps the last Chinese scheme across a Japanese switch", () => {
  const japanese: PreferenceMapping = KeyboardScheme.mapping(
    KeyboardScheme.JAPANESE,
    "wubi",
    "xiaohe",
  );
  check(japanese.scheme === "japanese", "the engine scheme follows the selection");
  check(
    japanese.lastChineseScheme === "wubi",
    "switching to Japanese must not forget which Chinese scheme to come back to",
  );
  const wubi: PreferenceMapping = KeyboardScheme.mapping(KeyboardScheme.WUBI, "quanpin", "xiaohe");
  check(wubi.lastChineseScheme === "wubi", "a Chinese scheme becomes the one to come back to");
  const shuangpin: PreferenceMapping = KeyboardScheme.mapping(
    KeyboardScheme.ZIRANMA,
    "quanpin",
    "xiaohe",
  );
  check(shuangpin.shuangpinProfile === "ziranma", "the scheme carries its own profile");
  const kept: PreferenceMapping = KeyboardScheme.mapping(
    KeyboardScheme.QUANPIN,
    "quanpin",
    "microsoft",
  );
  check(
    kept.shuangpinProfile === "microsoft",
    "a non-shuangpin scheme preserves the stored profile",
  );
  const normalised: PreferenceMapping = KeyboardScheme.mapping(
    KeyboardScheme.QUANPIN,
    "quanpin",
    "nonsense",
  );
  check(normalised.shuangpinProfile === "xiaohe", "an unknown stored profile normalises to xiaohe");
  const fallback: PreferenceMapping = KeyboardScheme.mapping(
    KeyboardScheme.JAPANESE,
    "nonsense",
    "xiaohe",
  );
  check(
    fallback.lastChineseScheme === "quanpin",
    "an unknown last Chinese scheme normalises to quanpin",
  );
});

group("the engine scheme id names the scheme the policies compare against", () => {
  // The view is handed a number and the policies are written against the preference names. The two
  // used to be passed to each other directly, so CandidateManagementAction never saw 'japanese' and
  // offered dictionary actions on candidates that cannot take them.
  check(KeyboardScheme.engineSchemeName(0) === "quanpin", "zero is quanpin");
  check(KeyboardScheme.engineSchemeName(1) === "shuangpin", "one is shuangpin");
  check(
    KeyboardScheme.engineSchemeName(2) === "wubi",
    "two is wubi, as the wubi hint policy reads it",
  );
  check(KeyboardScheme.engineSchemeName(3) === "japanese", "three is japanese");
  check(
    KeyboardScheme.engineSchemeName(99) === "quanpin",
    "an unknown id falls back rather than throwing",
  );
  check(
    !CandidateManagementAction.candidateActionsAvailable(KeyboardScheme.engineSchemeName(3), 0),
    "a japanese candidate offers no dictionary actions once the name reaches the policy",
  );
});

group("special touch faces yield to modes that need literal keys", () => {
  check(
    KeyboardScheme.usesHandwritingFace(KeyboardScheme.HANDWRITING, false, false, "none"),
    "Chinese handwriting shows its canvas",
  );
  check(
    !KeyboardScheme.usesHandwritingFace(KeyboardScheme.HANDWRITING, true, false, "none"),
    "English leaves handwriting for alphabetic keys",
  );
  check(
    !KeyboardScheme.usesHandwritingFace(KeyboardScheme.HANDWRITING, false, true, "none"),
    "the symbol layer leaves handwriting for symbol rows",
  );
  check(
    !KeyboardScheme.usesHandwritingFace(KeyboardScheme.HANDWRITING, false, false, "unicode"),
    "a local utility leaves handwriting for literal alphabetic keys",
  );
  check(
    !KeyboardScheme.usesHandwritingFace(KeyboardScheme.QUANPIN, false, false, "none"),
    "an alphabetic scheme never borrows the handwriting canvas",
  );
  check(KeyboardScheme.usesNineKeyFace(true, "none"), "the saved nine-key layout keeps its grid");
  check(
    !KeyboardScheme.usesNineKeyFace(true, "unicode"),
    "a local utility overrides nine-key with literal alphabetic keys",
  );
});

group("a runtime selection that changes nothing produces no update", () => {
  check(
    KeyboardScheme.mappingForRuntimeSelection(
      KeyboardScheme.QUANPIN,
      KeyboardScheme.QUANPIN,
      "quanpin",
      "xiaohe",
    ) === null,
    "selecting the applied scheme is not a change",
  );
  check(
    KeyboardScheme.mappingForRuntimeSelection(
      KeyboardScheme.QUANPIN,
      KeyboardScheme.THOUGHTFUL_REPLY,
      "quanpin",
      "xiaohe",
    ) === null,
    "the reply surface is not an engine scheme",
  );
  check(
    KeyboardScheme.mappingForRuntimeSelection(KeyboardScheme.QUANPIN, null, "quanpin", "xiaohe") ===
      null,
    "no selection is not a change",
  );
  const change = KeyboardScheme.mappingForRuntimeSelection(
    KeyboardScheme.QUANPIN,
    KeyboardScheme.WUBI,
    "quanpin",
    "xiaohe",
  );
  check(change !== null && change.scheme === "wubi", "a real change produces a mapping");
});

console.log("NineKeyLayout");

group("the quanpin grid is three by three and sends characters, not labels", () => {
  const rows: NineKey[][] = NineKeyLayout.rows();
  check(rows.length === 3, "three rows");
  for (const row of rows) {
    check(row.length === 3, "three keys per row");
  }
  check(rows[0][0].input === "'", "the word-split key sends an apostrophe, not its label");
  check(rows[0][0].label === "分词", "the word-split key is labelled in Chinese");
  check(rows[0][1].input === "2" && rows[0][1].label === "ABC", "ABC sends 2");
  check(rows[2][2].input === "9" && rows[2][2].label === "WXYZ", "WXYZ sends 9");
  check(NineKeyLayout.punctuation().length === 4, "four punctuation marks ride alongside the grid");
});

group("the digit layer re-labels the grid instead of handing over ten across", () => {
  // MSIME-Apple's testNineKeyDigitLayerKeepsTheGridInsteadOfTheTwentySixKeyRows: a typist who chose
  // three columns keeps three columns, and nineKey1..nineKey9 read 1..9 rather than ABC, DEF, GHI.
  const digits: NineKey[][] = NineKeyLayout.digits();
  check(digits.length === 3, "still three rows");
  for (const row of digits) {
    check(row.length === 3, "still three keys per row");
  }
  const faces: string[] = digits.flat().map((key: NineKey) => key.label);
  check(faces.join("") === "123456789", "the nine cells read 1 through 9 in order");
  for (const key of digits.flat()) {
    check(key.input === key.label, "a digit sends the digit it prints");
  }
  check(
    NineKeyLayout.rows()[0][0].input === "'" && digits[0][0].input === "1",
    "the first cell spells with an apostrophe and counts with a 1",
  );
  check(
    NineKeyLayout.digitPunctuation().length === NineKeyLayout.punctuation().length,
    "the sidebar keeps its four cells across the switch",
  );
  check(
    NineKeyLayout.digitPunctuation().every(
      (symbol: string) => !NineKeyLayout.punctuation().includes(symbol),
    ),
    "and offers marks the letter layer does not already carry",
  );
});

group("a long press exposes the literal digit and letters", () => {
  const rows: NineKey[][] = NineKeyLayout.rows();
  check(NineKeyLayout.holdOptions(rows[0][1]).join("") === "2abc", "ABC offers 2, a, b and c");
  check(NineKeyLayout.holdOptions(rows[2][0]).join("") === "7pqrs", "PQRS keeps all four letters");
  check(
    NineKeyLayout.holdOptions(rows[0][0]).length === 0,
    "the word-split cell has no literal menu",
  );
  check(
    NineKeyLayout.holdOptions(NineKeyLayout.digits()[0][1]).length === 0,
    "the digit face does not duplicate its own tap",
  );
});

console.log("JapaneseNineKeyLayout");

group("every key carries exactly five directions", () => {
  const keys: JapaneseKey[] = JapaneseNineKeyLayout.keys();
  check(keys.length === 11, "eleven kana keys");
  for (const entry of keys) {
    check(entry.kana.length === 5, "five kana per key");
    check(entry.strokes.length === 5, "five strokes per key");
  }
  const digits: JapaneseKey[] = JapaneseNineKeyLayout.digitKeys();
  check(digits.length === 11, "eleven digit-layer keys");
  for (const entry of digits) {
    check(
      entry.kana.length === 5 && entry.strokes.length === 5,
      "the digit layer keeps the same five-direction shape",
    );
  }
});

group("kana map to the romanization the Engine expects", () => {
  const keys: JapaneseKey[] = JapaneseNineKeyLayout.keys();
  check(
    keys[0].kana[DIRECTION_CENTRE] === "あ" && keys[0].strokes[DIRECTION_CENTRE] === "a",
    "the first key composes a",
  );
  check(
    keys[2].kana[DIRECTION_LEFT] === "し" && keys[2].strokes[DIRECTION_LEFT] === "shi",
    "shi is spelled with three letters, not si",
  );
  check(
    keys[3].kana[DIRECTION_UP] === "つ" && keys[3].strokes[DIRECTION_UP] === "tsu",
    "tsu is spelled with three letters",
  );
  check(
    keys[9].kana[DIRECTION_UP] === "ん" && keys[9].strokes[DIRECTION_UP] === "n'",
    "the syllabic n carries its apostrophe so it does not swallow the next vowel",
  );
  check(
    keys[7].strokes[DIRECTION_LEFT] === "" && keys[7].kana[DIRECTION_LEFT] === "「",
    "a bracket on the ya key commits directly rather than composing",
  );
  check(keys[10].strokes[DIRECTION_CENTRE] === "", "the punctuation key composes nothing");
});

group("variant groups pair every label with a stroke", () => {
  const variants: VariantGroup[] = JapaneseNineKeyLayout.variants();
  check(variants.length === 3, "small kana, voiced and semi-voiced");
  for (const entry of variants) {
    check(
      entry.kana.length === entry.strokes.length && entry.kana.length > 0,
      `${entry.title} pairs every label with a stroke`,
    );
  }
  check(variants[1].kana.length === 21, "the voiced group carries twenty-one kana");
  check(JapaneseNineKeyLayout.digitBrackets().length === 8, "eight bracket pairs");
});

group("a flick shorter than the threshold stays on the centre", () => {
  check(
    JapaneseNineKeyLayout.direction(0, 0, 10) === DIRECTION_CENTRE,
    "no movement is the centre",
  );
  check(
    JapaneseNineKeyLayout.direction(9, 9, 10) === DIRECTION_CENTRE,
    "movement below the threshold in both axes is the centre",
  );
  check(
    JapaneseNineKeyLayout.direction(10, 0, 10) === DIRECTION_RIGHT,
    "reaching the threshold leaves the centre",
  );
});

group("the larger axis decides, so a diagonal never falls between directions", () => {
  check(JapaneseNineKeyLayout.direction(-30, 0, 10) === DIRECTION_LEFT, "left");
  check(JapaneseNineKeyLayout.direction(30, 0, 10) === DIRECTION_RIGHT, "right");
  check(JapaneseNineKeyLayout.direction(0, -30, 10) === DIRECTION_UP, "up");
  check(JapaneseNineKeyLayout.direction(0, 30, 10) === DIRECTION_DOWN, "down");
  check(
    JapaneseNineKeyLayout.direction(-30, 20, 10) === DIRECTION_LEFT,
    "a wider horizontal movement is horizontal",
  );
  check(
    JapaneseNineKeyLayout.direction(20, -30, 10) === DIRECTION_UP,
    "a taller vertical movement is vertical",
  );
  check(
    JapaneseNineKeyLayout.direction(30, 30, 10) === DIRECTION_DOWN,
    "an exact diagonal resolves vertically rather than being ambiguous",
  );
});

group("a negative flick threshold is rejected", () => {
  assertThrows(
    () => JapaneseNineKeyLayout.direction(0, 0, -1),
    /Flick threshold/,
    "rejects invalid input",
  );
  checks++;
});

group("side keys say what the next press will do", () => {
  check(JapaneseNineKeyActions.spaceTitle(true) === "変換", "space converts while composing");
  check(JapaneseNineKeyActions.spaceTitle(false) === "空白", "space inserts a blank otherwise");
  check(JapaneseNineKeyActions.returnTitle(true) === "確定", "return confirms while composing");
  check(JapaneseNineKeyActions.returnTitle(false) === "改行", "return breaks the line otherwise");
});

console.log("Input policies");

console.log("ReplyKeyboardPolicy");

group("reply source and request bounds are explicit", () => {
  check(ReplyKeyboardPolicy.source("  对方的话  ") === "对方的话", "trims source text");
  check(ReplyKeyboardPolicy.source("   ") === null, "rejects empty source");
  check(ReplyKeyboardPolicy.source("含\n换行") === null, "rejects control characters");
  const request = ReplyKeyboardPolicy.request("对方的话", "高情商");
  check(request !== null && request.prompt.includes("高情商"), "builds style prompt");
  const polish = ReplyKeyboardPolicy.request("用户原文", "成熟稳重", "", true);
  check(
    polish !== null &&
      polish.prompt.includes("润色用户文字") &&
      !polish.prompt.includes("对方发来以下内容"),
    "builds polish prompt separately",
  );
});

group("reply results are safe, unique and bounded", () => {
  const values: string[] = ReplyKeyboardPolicy.results(["一", "一", "二", "三", "四"]);
  check(values.length === 3 && values[2] === "三", "deduplicates and limits results");
  check(ReplyKeyboardPolicy.results(["好\u0000"]).length === 0, "rejects control characters");
  check(ReplyKeyboardPolicy.STYLES.length === 9, "keeps the shared nine reply styles");
});

group("community reply templates accept only bounded reply entries", () => {
  const values = CommunityReplyLibraryPolicy.parse(
    JSON.stringify([
      { id: "one", kind: "reply", name: "礼貌", content: { prompt: "保持礼貌。" } },
      { id: "dictionary", kind: "dictionary", name: "词库", content: { prompt: "忽略" } },
    ]),
  );
  check(values.length === 1 && values[0].id === "one", "filters non-reply resources");
  check(
    CommunityReplyLibraryPolicy.parse(
      '[{"id":"one","kind":"reply","name":"x","content":{"prompt":"y"}},{"id":"one","kind":"reply","name":"z","content":{"prompt":"q"}}]',
    ).length === 0,
    "rejects duplicate ids",
  );
  check(
    CommunityReplyLibraryPolicy.parse(
      '[{"id":"one","kind":"reply","name":"x","content":{"prompt":"bad\u0000"}}]',
    ).length === 0,
    "rejects control text",
  );
  check(CommunityReplyLibraryPolicy.parse("not-json").length === 0, "rejects malformed documents");
});

group("reply results stay bound to the editor context", () => {
  check(ReplyContextPolicy.matches(3, 3, 7, 7), "accepts the same editor generations");
  check(!ReplyContextPolicy.matches(3, 4, 7, 7), "rejects a changed editor");
  check(!ReplyContextPolicy.matches(3, 3, 7, 8), "rejects a changed cursor context");
  check(!ReplyContextPolicy.matches(-1, -1, 0, 0), "rejects invalid generations");
});

group("helpcode needs a composition, a pinyin scheme and no local mode", () => {
  check(
    ChineseHelpcodePolicy.entersHelpcode(false, true, "ni", 0, "none"),
    "shift during a quanpin composition enters helpcode",
  );
  check(
    !ChineseHelpcodePolicy.entersHelpcode(false, false, "ni", 0, "none"),
    "without shift there is no helpcode",
  );
  check(
    !ChineseHelpcodePolicy.entersHelpcode(true, true, "ni", 0, "none"),
    "dedicated English never enters helpcode",
  );
  check(
    !ChineseHelpcodePolicy.entersHelpcode(false, true, "", 0, "none"),
    "an empty composition has nothing to annotate",
  );
  check(
    !ChineseHelpcodePolicy.entersHelpcode(false, true, "ni", 2, "none"),
    "wubi has no helpcode",
  );
  check(
    !ChineseHelpcodePolicy.entersHelpcode(false, true, "ni", 0, "unicode"),
    "a local mode owns the keystroke instead",
  );
  check(ChineseHelpcodePolicy.eligible(false, "ni", 1, "none"), "shuangpin is eligible too");
});

group("the wubi hint shows only the untyped suffix", () => {
  check(
    WubiCodeHintPolicy.hint("ggll", "gg", true, 2, "none", false) === "ll",
    "the remaining code is the suffix",
  );
  check(
    WubiCodeHintPolicy.hint("gg", "gg", true, 2, "none", false) === "",
    "a fully typed code has no remainder",
  );
  check(
    WubiCodeHintPolicy.hint("ggll", "xx", true, 2, "none", false) === "",
    "a code that is not an extension is not annotated",
  );
  check(
    WubiCodeHintPolicy.hint("ggll", "gg", false, 2, "none", false) === "",
    "the hint can be switched off",
  );
  check(
    WubiCodeHintPolicy.hint("ggll", "gg", true, 0, "none", false) === "",
    "only the wubi scheme is annotated",
  );
  check(
    WubiCodeHintPolicy.hint("ggll", "gg", true, 2, "none", true) === "",
    "a pinyin fallback candidate is not annotated: its code is not the one being typed",
  );
  check(
    WubiCodeHintPolicy.hint("ggll", "gg", true, 2, "unicode", false) === "",
    "a local mode candidate is not annotated",
  );
  check(WubiCodeHintPolicy.hint(null, "gg", true, 2, "none", false) === "", "a null code is safe");
  check(
    WubiCodeHintPolicy.hint("g".repeat(65), "g", true, 2, "none", false) === "",
    "an implausibly long code is refused rather than rendered",
  );
});

group("the letter face and the engine input are decided separately", () => {
  check(
    LetterKeyFacePolicy.face("a", true, false, false) === "A",
    "Chinese mode prints uppercase faces while still sending lowercase",
  );
  check(
    LetterKeyFacePolicy.face("a", true, true, false) === "a",
    "a local mode drops back to the lowercase face",
  );
  check(
    LetterKeyFacePolicy.face("a", false, false, false) === "a",
    "English unshifted is lowercase",
  );
  check(LetterKeyFacePolicy.face("a", false, false, true) === "A", "English shifted is uppercase");
  check(LetterKeyFacePolicy.face("", true, false, false) === "", "an empty face stays empty");
  check(
    LetterKeyFacePolicy.accessibilityLabel("a", false, false, true) === "大写 A",
    "a shifted English key announces uppercase",
  );
  check(
    LetterKeyFacePolicy.accessibilityLabel("a", true, false, false) === "字母 A",
    "a Chinese key announces the letter",
  );
  check(
    LetterKeyFacePolicy.accessibilityLabel(null, true, false, false) === "字母",
    "a missing letter still announces something",
  );
});

group("word capitalization starts after anything that is not a letter or digit", () => {
  const words = CapitalizationMode.WORDS;
  check(
    EnglishCapitalizationPolicy.shouldShift(words, "") === true,
    "an empty field starts a word",
  );
  check(EnglishCapitalizationPolicy.shouldShift(words, "hello ") === true, "a space starts a word");
  check(
    EnglishCapitalizationPolicy.shouldShift(words, "hello") === false,
    "mid-word stays lowercase",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(words, "don'") === false,
    "an apostrophe keeps the word going rather than starting a new one",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(words, "don\u2019") === false,
    "a typographic apostrophe behaves the same",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(words, "x1") === false,
    "a digit is part of the word",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(words, null) === false,
    "no context means no shift",
  );
});

group("sentence capitalization looks past closers and whitespace", () => {
  const sentences = CapitalizationMode.SENTENCES;
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "") === true,
    "an empty field starts a sentence",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "Hi. ") === true,
    "a full stop ends a sentence",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "Hi.") === true,
    "even without the space",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "Hi") === false,
    "mid-sentence stays lowercase",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, 'Hi." ') === true,
    "a closing quote after the stop is skipped",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "Hi) ") === false,
    "a closer with no terminator behind it does not start a sentence",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "Hi\n") === true,
    "a newline starts a sentence",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "你好。") === true,
    "the full-width stop ends a sentence too",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(sentences, "   ") === true,
    "whitespace all the way back is the start of the field",
  );
});

group("the other two capitalization modes are unconditional", () => {
  check(
    EnglishCapitalizationPolicy.shouldShift(CapitalizationMode.NONE, "") === false,
    "none never shifts",
  );
  check(
    EnglishCapitalizationPolicy.shouldShift(CapitalizationMode.ALL_CHARACTERS, "abc") === true,
    "all characters always shifts",
  );
});

group("a one-shot shift is spent by the next letter", () => {
  const state = new EnglishLetterCaseState();
  check(state.mode() === LetterCaseMode.LOWERCASE, "starts lowercase");
  state.toggle(1000);
  check(state.mode() === LetterCaseMode.SHIFTED, "one tap shifts");
  check(state.consumeLetter() === true, "the letter spends it");
  check(state.mode() === LetterCaseMode.LOWERCASE, "and it falls back to lowercase");
  check(state.consumeLetter() === false, "a second letter has nothing to spend");
});

group("an automatic shift and a pressed one are told apart", () => {
  // The keyboard now applies the capitalization policy on attach and after every text change, so
  // the two have to be distinguishable: an automatic shift is recomputed from the text and must not
  // be spent by the letter, or a capitals-only field would produce exactly one capital; a pressed
  // one is the user's and must survive anything arriving before the letter that spends it.
  const automatic = new EnglishLetterCaseState();
  check(automatic.applyAutomatic(true) === true, "applying a shift is a change");
  check(automatic.mode() === LetterCaseMode.SHIFTED, "and leaves the face shifted");
  check(automatic.isAutomatic() === true, "the state says the shift was not pressed");
  check(automatic.applyAutomatic(true) === false, "recomputing the same answer redraws nothing");

  const pressed = new EnglishLetterCaseState();
  pressed.toggle(1000);
  check(pressed.mode() === LetterCaseMode.SHIFTED, "a tap shifts");
  check(pressed.isAutomatic() === false, "and says so");
  check(
    pressed.applyAutomatic(false) === true,
    "the rule is able to clear it, which is why the caller checks isAutomatic first",
  );

  const locked = new EnglishLetterCaseState();
  locked.toggle(1000);
  locked.toggle(1000 + EnglishLetterCaseState.CAPS_LOCK_INTERVAL_MILLIS);
  check(locked.mode() === LetterCaseMode.CAPS_LOCK, "a double tap locks");
  check(locked.applyAutomatic(false) === false, "and the rule cannot unlock it");
  check(locked.mode() === LetterCaseMode.CAPS_LOCK, "the lock stands");
});

group("a double tap inside the interval locks, a slow one does not", () => {
  const quick = new EnglishLetterCaseState();
  quick.toggle(1000);
  quick.toggle(1000 + EnglishLetterCaseState.CAPS_LOCK_INTERVAL_MILLIS);
  check(quick.mode() === LetterCaseMode.CAPS_LOCK, "exactly at the interval still locks");
  check(quick.keyText() === "⇪", "the key face shows the lock");
  check(quick.consumeLetter() === false, "caps lock is not spent by a letter");

  const slow = new EnglishLetterCaseState();
  slow.toggle(1000);
  slow.toggle(1000 + EnglishLetterCaseState.CAPS_LOCK_INTERVAL_MILLIS + 1);
  check(slow.mode() === LetterCaseMode.LOWERCASE, "one millisecond later it just toggles off");
  check(slow.keyText() === "⇧", "the key face shows the plain shift");
});

group("automatic shift never overrides caps lock", () => {
  const locked = new EnglishLetterCaseState();
  locked.toggle(1000);
  locked.toggle(1100);
  check(locked.applyAutomatic(false) === false, "the policy cannot unlock caps lock");
  check(locked.mode() === LetterCaseMode.CAPS_LOCK, "and the mode is unchanged");

  const plain = new EnglishLetterCaseState();
  check(plain.applyAutomatic(true) === true, "applying a shift reports the change");
  check(plain.isAutomatic() === true, "and marks it automatic");
  check(plain.accessibilityValue() === "自动开启", "which the announcement distinguishes");
  check(plain.applyAutomatic(true) === false, "reapplying the same shift is not a change");
  plain.toggle(2000);
  check(plain.isAutomatic() === false, "a manual tap clears the automatic flag");
});

group("a negative uptime is rejected rather than treated as a fast tap", () => {
  const state = new EnglishLetterCaseState();
  assertThrows(() => state.toggle(-1), /Uptime/, "rejects invalid input");
  checks++;
});

console.log("Output and editor policies");

group("maps shared candidate skins to native Harmony palettes", () => {
  check(
    CandidateSkinPolicy.rowDetailColor(false, "#111111", "#ffffff") === "#111111",
    "unselected candidate details use the normal text colour",
  );
  check(
    CandidateSkinPolicy.rowDetailColor(true, "#111111", "#ffffff") === "#ffffff",
    "selected candidate details follow the selected text colour",
  );
  // Pinned and selected are different states in the source: CandidateViewHtml wraps a
  // fixed-position item in its own #379AD3 rather than the selected-row colour. Drawing both in the
  // skin's accent made them indistinguishable on any skin whose accent is its selection colour.
  check(
    CandidateSkinPolicy.rowTextColor(false, false, "#111111", "#ffffff") === "#111111",
    "an ordinary candidate uses the normal text colour",
  );
  check(
    CandidateSkinPolicy.rowTextColor(true, false, "#111111", "#ffffff") === "#ffffff",
    "the selected candidate uses the selected text colour",
  );
  check(
    CandidateSkinPolicy.rowTextColor(false, true, "#111111", "#ffffff") === "#379AD3",
    "a pinned candidate takes the source's own colour, not the skin's",
  );
  check(
    CandidateSkinPolicy.rowTextColor(true, true, "#111111", "#ffffff") === "#379AD3",
    "being selected as well does not hide that a candidate is pinned",
  );
  // Each source skin now resolves to a palette of its own, built from its own upstream stylesheet,
  // rather than to the nearest touch-keyboard palette. The nearest-palette mapping is what put
  // 微信绿 and 杨柳青 on the same colours.
  check(CandidateSkinPolicy.harmonySkin("fluent") === "fluent", "Fluent keeps its own palette");
  check(CandidateSkinPolicy.harmonySkin("wechat") === "wechat", "WeChat keeps its own palette");
  check(
    CandidateSkinPolicy.harmonySkin("graphite") === "graphite",
    "Graphite keeps its own palette",
  );
  check(
    CandidateSkinPolicy.harmonySkin("willow_green") === "willow_green",
    "Willow green keeps its own palette",
  );
  check(CandidateSkinPolicy.harmonySkin("unknown") === "forest", "unknown ids fall back safely");
  check(CandidateSkinPolicy.showSelectedBar("fluent"), "Fluent shows its selected bar");
  check(!CandidateSkinPolicy.showSelectedBar("wechat"), "WeChat skin omits its selected bar");
  check(!CandidateSkinPolicy.showSelectedBar("graphite"), "Graphite skin omits its selected bar");
  check(
    CandidateSkinPolicy.harmonySkin("sample", "wechat") === "wechat",
    "external skins inherit the WeChat palette",
  );
  check(
    CandidateSkinPolicy.harmonySkin("sample", "graphite") === "graphite",
    "external skins inherit the Graphite palette",
  );
});

group("resolves external candidate skin tokens without trusting missing fields", () => {
  const sample: CandidateSkinPackage = {
    id: "sample",
    base: "wechat",
    layouts: ["vertical"],
    themes: ["dark"],
    minWidthDip: 360,
    decorationTopDip: 24,
    decorationWidthDip: 180,
    toolbarStylesheet: "toolbar.css",
    preview: "images/preview.svg",
    candidate: {
      dark: {
        accent: "#123456",
        selected: "#234567",
        hover: "#345678",
        surface: "#456789",
        border: "#56789A",
        text: "#6789AB",
        number: "#789ABC",
        showSelectedBar: false,
      },
      light: {
        accent: null,
        selected: null,
        hover: null,
        surface: null,
        border: null,
        text: null,
        number: null,
        showSelectedBar: null,
      },
    },
  };
  const packages: CandidateSkinPackage[] = [sample];
  const palette = CandidateSkinCatalogPolicy.palette(packages, "sample", true);
  check(palette !== null && palette.accent === "#123456", "external accent is selected by theme");
  check(
    CandidateSkinCatalogPolicy.base(packages, "sample") === "wechat",
    "external base skin is retained",
  );
  check(
    CandidateSkinCatalogPolicy.supports(packages, "sample", "vertical", "dark"),
    "manifest compatibility accepts a declared layout and theme",
  );
  check(
    !CandidateSkinCatalogPolicy.supports(packages, "sample", "horizontal", "dark"),
    "manifest compatibility rejects an undeclared layout",
  );
  check(
    CandidateSkinCatalogPolicy.minWidthVp(packages, "sample") === 360,
    "external minimum width is exposed to the native panel",
  );
  check(
    CandidateSkinCatalogPolicy.showSelectedBar(packages, "sample", true) === false,
    "external selected-bar override is retained",
  );
  check(
    CandidateSkinCatalogPolicy.toolbarStylesheet(packages, "sample") === "toolbar.css",
    "external toolbar stylesheet is retained",
  );
  const decoration = CandidateSkinCatalogPolicy.decoration(packages, "sample");
  check(
    decoration !== null && decoration.topVp === 24 && decoration.widthVp === 180,
    "external decoration geometry is retained",
  );
  check(
    CandidateSkinCatalogPolicy.decoration(packages, "missing") === null,
    "unknown packages do not invent decoration geometry",
  );
  check(
    CandidateSkinCatalogPolicy.imageDataUrl("image/png", [0, 1, 2]) ===
      "data:image/png;base64,AAEC",
    "image bytes become an image-only data URL",
  );
  check(
    CandidateSkinCatalogPolicy.imageDataUrl("text/css", [0, 1, 2]) === null,
    "non-image resources cannot become decoration URLs",
  );
  check(
    CandidateSkinCatalogPolicy.imageAspectRatio(
      "image/png",
      [0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0, 0x01],
    ) === 2,
    "PNG dimensions preserve the decoration aspect ratio",
  );
  check(
    CandidateSkinCatalogPolicy.palette(packages, "missing", true) === null,
    "unknown package does not invent a palette",
  );
  check(
    CandidateSkinCatalogPolicy.color("#123456") === "#123456",
    "hex colors remain available to ArkUI",
  );
  check(
    CandidateSkinCatalogPolicy.color("rgba(1, 2, 3, 0.5)") === "rgba(1, 2, 3, 0.5)",
    "rgba colors remain available to ArkUI",
  );
  check(
    CandidateSkinCatalogPolicy.color("red;} .poison {") === null,
    "declaration-like color tokens are rejected",
  );
  check(
    CandidateSkinCatalogPolicy.color("rgb(256, 0, 0)") === null,
    "out-of-range rgb channels are rejected",
  );
  check(
    CandidateSkinPolicy.showSelectedBar("sample", false) === false,
    "explicit external selected-bar value wins over the base default",
  );
});

group("maps safe external toolbar CSS to ArkUI values", () => {
  const base = KeyboardSkin.from("forest", true);
  const toolbar = ToolbarSkinPolicy.fromCss(
    base,
    `
    .status-bar { background: #101820; border: 1px solid #223344; border-radius: 6px; }
    .drag-handle { background-color: #556677; }
    .divider { background: rgba(80, 90, 100, .5); }
    .icon { color: rgba(200, 210, 220, .9); font-family: "Noto Sans SC", sans-serif; }
    .icon:hover { background-color: #334455; }
    .english-candidate-label { font-family: "JetBrains Mono", monospace; }
  `,
  );
  check(toolbar.backgroundColor === "#101820", "toolbar background is mapped");
  check(toolbar.borderColor === "#223344", "toolbar border is mapped");
  check(toolbar.dragHandleColor === "#556677", "toolbar drag handle color is mapped");
  check(toolbar.dividerColor === "rgba(80, 90, 100, .5)", "toolbar divider color is mapped");
  check(toolbar.buttonColor === "rgba(200, 210, 220, .9)", "toolbar icon color is mapped");
  check(toolbar.buttonHoverColor === "#334455", "toolbar hover color is mapped");
  check(toolbar.cornerRadiusVp === 6, "toolbar radius is bounded and mapped");
  check(toolbar.fontFamily === '"Noto Sans SC", sans-serif', "toolbar font family is mapped");
  check(
    toolbar.englishFontFamily === '"JetBrains Mono", monospace',
    "English toolbar font family is mapped",
  );
  const unsafe = ToolbarSkinPolicy.fromCss(
    base,
    ".status-bar { background: url(https://example.invalid/x); } .icon { color: red; }",
  );
  check(unsafe.backgroundColor === base.keyBackground, "resource URLs are ignored");
  check(unsafe.buttonColor === base.accent, "unsupported colour syntax is ignored");
});

group("applies Windows toolbar scale and font-size bounds to Harmony geometry", () => {
  check(FloatingToolbarLayout.scale(0.75) === 0.75, "minimum toolbar scale is retained");
  check(FloatingToolbarLayout.scale(1.12) === 1, "scale snaps to the nearest Windows step");
  check(FloatingToolbarLayout.scale(1.4) === 1.5, "upper scale snaps to the nearest step");
  check(FloatingToolbarLayout.scale(2) === 1, "out-of-range scale uses the Windows default");
  check(FloatingToolbarLayout.scale(Number.NaN) === 1, "invalid scale falls back to one");
  check(FloatingToolbarLayout.fontSize(16) === 16, "minimum toolbar font size is retained");
  check(
    FloatingToolbarLayout.fontSize(40) === 24,
    "out-of-range toolbar font size uses the Windows default",
  );
  check(
    FloatingToolbarLayout.fontSize(Number.NaN) === 24,
    "invalid font size uses Windows default",
  );
  check(FloatingToolbarLayout.buttonWidthVp(1.5) === 63, "button width follows toolbar scale");
  check(FloatingToolbarLayout.heightVp(0.75) === 33, "toolbar height follows toolbar scale");
  check(
    FloatingToolbarLayout.widthVp(2) * 1.5 === 172.5,
    "host width arithmetic remains consistent with scaled content",
  );
});

group("shows Japanese input mode in the Harmony toolbar", () => {
  const japaneseState = {
    english: false,
    temporaryEnglish: false,
    japanese: true,
    capsLock: false,
    chinesePunctuation: true,
    fullWidth: false,
    traditional: false,
  };
  const englishState = {
    english: true,
    temporaryEnglish: false,
    japanese: true,
    capsLock: false,
    chinesePunctuation: true,
    fullWidth: false,
    traditional: false,
  };
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, japaneseState) === "日",
    "Japanese mode has its own toolbar face",
  );
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, englishState) === "英",
    "dedicated English mode takes precedence over the Japanese scheme",
  );
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, FloatingToolbarLayout.idleState()) ===
      "中",
    "the default Chinese mode remains unchanged",
  );
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, {
      english: false,
      temporaryEnglish: false,
      japanese: true,
      capsLock: true,
      chinesePunctuation: true,
      fullWidth: false,
      traditional: false,
    }) === "A",
    "Caps Lock takes precedence over the language mode face",
  );
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, {
      english: false,
      temporaryEnglish: true,
      japanese: false,
      capsLock: false,
      chinesePunctuation: true,
      fullWidth: false,
      traditional: false,
    }) === "En",
    "temporary English mode uses the Windows En toolbar face",
  );
  check(
    FloatingToolbarLayout.face(ToolbarButton.INPUT_MODE, {
      english: true,
      temporaryEnglish: true,
      japanese: true,
      capsLock: false,
      chinesePunctuation: true,
      fullWidth: false,
      traditional: false,
    }) === "En",
    "temporary English takes precedence over dedicated language faces",
  );
});

group("keeps a dragged Harmony toolbar inside the display", () => {
  check(
    FloatingToolbarDragPolicy.position([100, 120], [20, -10], 2, 1000, 800, 200, 50).join(",") ===
      "140,100",
    "pan offsets convert from vp to px",
  );
  check(
    FloatingToolbarDragPolicy.position([100, 120], [1000, 1000], 2, 1000, 800, 200, 50).join(
      ",",
    ) === "800,750",
    "dragging beyond the display clamps to the lower-right edge",
  );
  check(
    FloatingToolbarDragPolicy.position([100, 120], [-1000, -1000], 2, 1000, 800, 200, 50).join(
      ",",
    ) === "0,0",
    "dragging beyond the upper-left edge clamps to zero",
  );
  check(
    FloatingToolbarDragPolicy.position([17, 19], [1, 1], 0, 1000, 800, 200, 50).join(",") ===
      "17,19",
    "invalid density preserves the last known position",
  );
});

group("a horizontal candidate window is as wide as its whole page", () => {
  // The Windows window measures every candidate on the page side by side (candidate_presenter.cpp); the widest single entry left most of a six-candidate row scrolled away.
  const page = ["你好", "拟好", "泥壕", "你", "尼", "呢"].map((text) => ({
    text,
    badge: "",
    hint: "",
    annotation: "",
  }));
  const row = CandidateWidthPolicy.rowWidthVp(page, "nihao", 18, 15, 12, 14);
  check(
    row > CandidateWidthPolicy.widthVp(page, "nihao", 18, 15),
    "six short candidates need more than the widest of them",
  );
  const chips = page.reduce(
    (sum, entry, index) =>
      sum + Math.ceil(CandidateWidthPolicy.chipContentVp(entry, index, 18, 14) + 24),
    0,
  );
  check(row >= chips, "every chip the view draws fits in the window");
  check(
    CandidateWidthPolicy.chipContentVp(page[0], 0, 18, 14) >
      CandidateWidthPolicy.chipContentVp(page[0], 0, 18, 0),
    "a desktop chip counts its ordinal",
  );
  check(
    CandidateWidthPolicy.rowWidthVp([page[3]], "", 18, 15, 12, 14) ===
      CandidateWidthPolicy.MIN_WIDTH_VP,
    "one short candidate keeps the minimum width",
  );
  check(
    CandidateWidthPolicy.rowWidthVp(page.concat(page, page), "", 32, 15, 12, 26) ===
      CandidateWidthPolicy.MAX_WIDTH_VP,
    "a long page stays inside the desktop bound",
  );
});

group("candidate and composition rows follow their font sizes", () => {
  // Windows: itemHeight = fontSize * 1.35 + 2, plus a 2 DIP gap, and the preedit measured at its own size.
  check(KeyboardMetrics.candidateRowHeightVp(12, true) === 21, "a 12 vp desktop row is compact");
  check(KeyboardMetrics.candidateRowHeightVp(32, true) === 48, "a 32 vp desktop row grows to fit");
  check(
    KeyboardMetrics.candidateRowHeightVp(12, false) === KeyboardMetrics.CANDIDATE_ROW_HEIGHT_VP,
    "a touch row keeps its finger-sized floor",
  );
  check(KeyboardMetrics.candidateRowHeightVp(32, false) === 48, "and still grows for a large font");
  check(
    KeyboardMetrics.compositionRowHeightVp(15) === KeyboardMetrics.COMPOSITION_ROW_HEIGHT_VP,
    "the default preedit keeps its line",
  );
  check(KeyboardMetrics.compositionRowHeightVp(32) === 44, "a large preedit gets a taller line");
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 9, true, 0, 0, 12) <
      KeyboardMetrics.CANDIDATE_ROW_HEIGHT_VP * 9,
    "nine vertical rows at 12 vp are shorter than nine fixed rows",
  );
  check(
    KeyboardMetrics.candidateHeightVp("horizontal", 1, true, 0, 0, 18, 32) -
      KeyboardMetrics.candidateHeightVp("horizontal", 1, true, 0, 0, 18, 15) ===
      44 - KeyboardMetrics.COMPOSITION_ROW_HEIGHT_VP,
    "the window grows with the preedit font",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15, true) <
      KeyboardMetrics.totalHeightVp(70, 0, 0, 18, 15),
    "a desktop surface strip is compact, a touch strip is not",
  );
});

group("sizes desktop candidate windows from bounded display estimates", () => {
  const short = CandidateWidthPolicy.widthVp([], "ni", 18, 15);
  const wide = CandidateWidthPolicy.widthVp(
    [{ text: "这是一个足够长的候选词条用于展示", badge: "", hint: "", annotation: "" }],
    "",
    18,
    15,
  );
  const annotated = CandidateWidthPolicy.widthVp(
    [{ text: "候选", badge: "云", hint: "houxuan", annotation: "candidate".repeat(10) }],
    "",
    18,
    15,
  );
  const plain = CandidateWidthPolicy.widthVp(
    [{ text: "候选", badge: "", hint: "", annotation: "" }],
    "",
    18,
    15,
  );
  check(short === CandidateWidthPolicy.MIN_WIDTH_VP, "short candidates use the compact minimum");
  check(wide > short, "wide CJK candidates receive more card width");
  check(annotated > plain, "badges and annotations contribute to width");
  check(
    CandidateWidthPolicy.widthVp(
      [{ text: "x".repeat(200), badge: "", hint: "", annotation: "" }],
      "",
      18,
      15,
    ) === CandidateWidthPolicy.MAX_WIDTH_VP,
    "provider text cannot grow the panel beyond the desktop bound",
  );
  check(
    CandidateWidthPolicy.textWidthVp("😀", 18) === 18,
    "emoji surrogate pairs count as one wide glyph",
  );
  assertThrows(
    () => CandidateWidthPolicy.textWidthVp("x", 0),
    /font size/,
    "rejects invalid font sizes",
  );
});

group(
  "preserves the candidate skin selected-bar default while honoring an explicit disable",
  () => {
    check(selectedBarVisible(null), "absent preference keeps the skin default");
    check(selectedBarVisible(true), "explicit enable keeps the bar");
    check(!selectedBarVisible(false), "explicit disable hides the bar");
  },
);

group("keeps Engine candidate selection independent from row order", () => {
  const entries = [
    { text: "first", hint: "", annotation: "", highlighted: false },
    { text: "second", hint: "", annotation: "", highlighted: true },
  ];
  check(
    !entries[0].highlighted && entries[1].highlighted,
    "a highlighted candidate may be below the first row",
  );
});

group("keeps candidate source badges bounded to known Engine sources", () => {
  check(CandidatePresentationPolicy.badge(2) === " ☁️", "cloud candidates get a cloud badge");
  check(CandidatePresentationPolicy.badge(3) === " 🤖", "AI candidates get a robot badge");
  check(CandidatePresentationPolicy.badge(0) === "", "local candidates stay unbadged");
});

group("offers all fixed candidate slots and checks the active one", () => {
  const actions = CandidateManagementAction.actionsForFixedPosition(3);
  check(actions.length === 8, "pin, five positions, clear and remove are exposed");
  check(
    actions[3].id === "FIX_3" && actions[3].checked === true,
    "the current fixed slot is marked",
  );
  check(
    actions[1].position === 1 && actions[5].position === 5,
    "positions keep their one-based slot numbers",
  );
});

group("a chip is as wide as its column, whether or not the gloss has arrived", () => {
  // MSIME-Apple's AChipKeepsItsWidthWhateverTheGlossTurnsOutToBe. The networked glosses come back
  // over a few hundred milliseconds; a width that follows them widens one chip at a time and pushes
  // every candidate to its right along with it. Columns first, answers into the columns after.
  const column = CandidateChipWidth.column(390 - 24, 6, 11);
  check(column > 0, "a visible strip yields a column");

  const waiting = CandidateChipWidth.chip(30, true, column, 11);
  const short = CandidateChipWidth.chip(30, true, column, 11);
  check(waiting === short, "the width does not move when the answer lands");

  // The word is never cut: one wider than a column widens its own chip instead.
  const wide = CandidateChipWidth.chip(column + 40, true, column, 11);
  check(wide > waiting, "a word wider than a column takes the room it needs");
  check(
    CandidateChipWidth.content(column + 40, true, column) === column + 40,
    "and takes it from the word, not from the column",
  );

  // Off, a chip is its word and nothing is reserved.
  check(
    CandidateChipWidth.content(30, false, column) === 30,
    "with glosses off the chip is the word",
  );

  check(CandidateChipWidth.GLOSS_COLUMNS === 3, "three across, as the source counts them");
  check(
    CandidateChipWidth.column(0, 6, 11) === 0 && CandidateChipWidth.column(-5, 6, 11) === 0,
    "no visible strip, no column",
  );
  check(
    CandidateChipWidth.column(30, 6, 40) === 0,
    "and padding wider than the column floors at zero rather than going negative",
  );
  check(
    Number.isInteger(CandidateChipWidth.chip(30.4, true, column, 11)),
    "a chip is a whole number of layout units",
  );
});

group("candidate gloss layout follows both independent switches before answers arrive", () => {
  const none: CandidateGlossProviderState = {
    customEnabled: false,
    customEndpoint: "",
    niuTransEnabled: false,
    niuTransAppId: "",
    niuTransApiKey: "",
    tencentEnabled: false,
    tencentSecretId: "",
    tencentSecretKey: "",
  };
  check(
    CandidateGlossLayoutPolicy.displays(true, false),
    "the packaged English gloss switch displays its own results",
  );
  check(
    CandidateGlossLayoutPolicy.displays(false, true),
    "online translations do not depend on the packaged English gloss switch",
  );
  check(
    CandidateGlossLayoutPolicy.rows(true, ["ja", "en"], false, none, []) === 1,
    "an English secondary target reserves the packaged gloss line",
  );
  check(
    CandidateGlossLayoutPolicy.rows(true, ["ja"], false, none, []) === 0,
    "the English dictionary does not reserve a wrong-language line",
  );
  const custom: CandidateGlossProviderState = {
    ...none,
    customEnabled: true,
    customEndpoint: "https://translation.example.invalid",
  };
  check(
    CandidateGlossLayoutPolicy.rows(false, ["ja"], true, custom, []) === 1,
    "a usable online provider reserves one merged Harmony gloss line",
  );
  const placeholder: CandidateGlossProviderState = {
    ...none,
    tencentEnabled: true,
    tencentSecretId: "<secret-id>",
    tencentSecretKey: "FAKESECRET_fixture",
  };
  check(
    CandidateGlossLayoutPolicy.rows(false, ["en"], true, placeholder, []) === 0,
    "placeholder credentials do not leave a permanently empty row",
  );
  check(
    CandidateGlossLayoutPolicy.rows(false, ["ja"], true, none, ["ja"]) === 1,
    "an installed Japanese dictionary reserves the line without an online provider",
  );
  check(
    CandidateGlossLayoutPolicy.rows(true, ["ja"], false, none, ["ja"]) === 1,
    "the offline gloss switch alone reaches an installed dictionary",
  );
  check(
    CandidateGlossLayoutPolicy.rows(false, ["ja"], false, none, ["ja"]) === 0,
    "both switches off reserve nothing even with a dictionary installed",
  );
});

group("the expanded panel offers the same gloss the strip does", () => {
  // MSIME-Apple's TheExpandedPanelAnswersALongPressToo and TheExpandedPanelDrawsTheSameGlossesAsThe
  // Strip. Here allCandidates() kept only the text, so the panel listed words while the strip
  // beside it explained them, and its long press had nothing to read.
  //
  // What the panel offers is the gloss alone. The management operations index the page the strip
  // shows, and the panel selects through selectAnyCandidate with a position in the whole list, so
  // there is no number here that would mean anything to them — which is also why the source's
  // panel answers with the gloss menu and nothing else.
  const gloss = CandidateManagementAction.glossAction("hello");
  check(gloss !== null, "the panel builds the same action the strip's menu does");
  check(
    gloss !== null && gloss.id === "INSERT_GLOSS",
    "so one dispatcher and one title format serve both",
  );

  // The wrap estimate reads the text, and a row now carries more than a string. Counting code
  // points rather than UTF-16 units is what keeps a surrogate pair one column wide.
  const widthOf = (text: string): number => Array.from(text).length * 20 + 8;
  check(widthOf("你好") === 48, "two CJK characters are two columns");
  check(widthOf("\u{1F600}") === 28, "and an emoji is one, not two");
});

group("expanded candidates stay on one line and inside their row", () => {
  const available = 320 - KeyboardMetrics.ROOT_HORIZONTAL_PADDING_VP * 2;
  const ordinary = ExpandedCandidateLayout.width(
    "日本",
    0,
    18,
    KeyboardMetrics.CANDIDATE_PADDING_VP,
    available,
  );
  const long = ExpandedCandidateLayout.width(
    "とてもながいこうほごがここにはいります",
    6,
    18,
    KeyboardMetrics.CANDIDATE_PADDING_VP,
    available,
  );
  check(ordinary < available, "ordinary candidates retain their natural compact width");
  check(long === available, "one long candidate is capped to the whole visible row");
  check(
    ExpandedCandidateLayout.width("😀", 9, 18, 12, available) <
      ExpandedCandidateLayout.width("😀😀", 9, 18, 12, available),
    "width estimation counts code points rather than UTF-16 halves",
  );
  const assignment = CandidateWrapPolicy.rows(available, 0, [ordinary, long, ordinary]);
  check(
    assignment[0] === 0 && assignment[1] === 1 && assignment[2] === 2,
    "a full-width long candidate owns one row without pushing outside it",
  );
});

group("a candidate with a gloss offers the gloss as something to type", () => {
  // MSIME-Apple's TheCandidateMenuOffersToInsertTheGlossItself: macOS hands the translation over
  // with Option and a digit, a touch keyboard has no modifiers, so it goes on the long press. Until
  // this, the host could draw a translation under a candidate with no way to type it.
  const gloss = CandidateManagementAction.glossAction("hello");
  check(gloss !== null, "a gloss becomes an action");
  check(gloss?.id === "INSERT_GLOSS", "with an id the dispatcher can tell apart");
  check(gloss?.title.includes("hello") === true, "the title carries the gloss itself");
  check(
    gloss?.confirmationRequired === false,
    "typing a word is not the kind of thing that needs confirming twice",
  );
  check(gloss?.announcement.includes("hello") === true, "and the announcement says what went in");

  // The source's title is the bare gloss because its menu holds nothing else. This menu holds the
  // management items too, so the title has to read as an action among actions.
  check(gloss?.title !== "hello", "a bare noun among verbs would not read as something to tap");

  check(CandidateManagementAction.glossAction("") === null, "no gloss, no action");
  check(CandidateManagementAction.glossAction("   ") === null, "and whitespace is not a gloss");

  const long = CandidateManagementAction.glossAction("x".repeat(200));
  check(long !== null, "a long gloss still gives an action");
  check(
    long !== null && Array.from(long.title).length < 40,
    "but the title is cut to something that fits one row on a phone",
  );
  check(
    long?.announcement.includes("x".repeat(200)) === true,
    "while the announcement keeps the whole thing, which is read aloud rather than laid out",
  );

  const touch = CandidateManagementAction.touchActions("hello", true);
  check(touch.length === 1, "a touch long press offers one action, not desktop management");
  check(touch[0].title === "hello", "the touch action is the bare gloss, as the source draws it");
  check(
    CandidateManagementAction.touchActions("hello", false).length === 0,
    "an Engine annotation is not offered as translated text",
  );
  check(
    CandidateManagementAction.touchActions("", true).length === 0,
    "a candidate without a gloss has no touch long-press menu",
  );

  // A phone has no right click, so its long press opens the whole menu the source's right click does, as Android's long press does; before this, pinning, fixing and deleting an entry were out of reach on a phone.
  const full = CandidateManagementAction.managementActions("hello", true, 2, true, true);
  check(full[0].id === "INSERT_GLOSS", "the gloss leads the full menu");
  check(
    full
      .slice(1)
      .map((action: ManagementAction) => action.id)
      .join(",") === "PROMOTE,FIX_1,FIX_2,FIX_3,FIX_4,FIX_5,CLEAR_POSITION,REMOVE",
    "then 优先显示, the five slots, 取消固定 and 删除词条, in the source's order",
  );
  check(
    full.find((action: ManagementAction) => action.id === "FIX_2")?.checked === true,
    "the held slot is marked",
  );
  const noGloss = CandidateManagementAction.managementActions("", false, 0, true, false);
  check(noGloss[0].id === "PROMOTE", "a candidate without a translation still gets management");
  check(
    !noGloss.some((action: ManagementAction) => action.id === "REMOVE"),
    "a single character cannot be deleted",
  );
  check(
    CandidateManagementAction.managementActions("wbcd", false, 0, true, true)[0].id === "PROMOTE",
    "an Engine annotation is not offered as translated text",
  );

  const distinct = CandidateManagementAction.actionsForFixedPosition(0).map(
    (action: ManagementAction) => action.menuItemId,
  );
  check(
    gloss !== null && !distinct.includes(gloss.menuItemId),
    "its menu id does not collide with any management item",
  );
});

group("limits candidate dictionary mutations to supported sources", () => {
  check(
    CandidateManagementAction.candidateActionsAvailable("quanpin", 0),
    "local candidates are actionable",
  );
  check(
    CandidateManagementAction.candidateActionsAvailable("quanpin", 1),
    "user dictionary candidates are actionable",
  );
  check(
    !CandidateManagementAction.candidateActionsAvailable("quanpin", 2),
    "cloud candidates are read-only",
  );
  check(
    !CandidateManagementAction.candidateActionsAvailable("japanese", 0),
    "Japanese candidates are read-only",
  );
  const disabled = CandidateManagementAction.actionsForFixedPosition(2, false);
  check(
    disabled.every((action: ManagementAction) => action.available === false),
    "unsupported candidates expose disabled actions",
  );
});

group("omits deletion for single-code-point candidates", () => {
  check(
    !CandidateManagementAction.hasMultipleCodePoints("你"),
    "one CJK code point cannot be deleted from the dictionary menu",
  );
  check(
    !CandidateManagementAction.hasMultipleCodePoints("😀"),
    "one supplementary code point is counted as one",
  );
  check(
    CandidateManagementAction.hasMultipleCodePoints("你好"),
    "multi-code-point words keep the deletion action",
  );
  check(
    CandidateManagementAction.actionsForFixedPosition(0, true, false).length === 7,
    "the delete row is omitted rather than merely disabled",
  );
});

group("keeps the candidate panel anchor when follow-cursor is disabled", () => {
  const anchor: CandidateAnchor = [100, 200, 20];
  const current: CandidateAnchor = [400, 500, 20];
  check(
    CandidateAnchorPolicy.position(false, anchor, current)[0] === 100,
    "disabled follow uses the composition anchor",
  );
  check(
    CandidateAnchorPolicy.position(true, anchor, current)[0] === 400,
    "enabled follow uses the latest caret",
  );
  check(
    CandidateAnchorPolicy.position(false, undefined, current)[1] === 500,
    "missing anchor falls back to the current caret",
  );
});

group("maps desktop candidate wheel movement to page commands", () => {
  check(CandidateWheelPolicy.previousPage(1), "positive wheel movement pages up");
  check(!CandidateWheelPolicy.nextPage(1), "positive movement does not page down");
  check(CandidateWheelPolicy.nextPage(-1), "negative wheel movement pages down");
  check(!CandidateWheelPolicy.previousPage(-1), "negative movement does not page up");
  check(
    !CandidateWheelPolicy.previousPage(0) && !CandidateWheelPolicy.nextPage(0),
    "zero movement is ignored",
  );
});

group("releases the candidate number row when the shared preference asks", () => {
  const key: HardwareKey = {
    keyCode: 0,
    unicodeChar: "2".charCodeAt(0),
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  };
  check(
    HardwareKeyRouter.route(key, true, true).action === HardwareKeyAction.SELECT,
    "the default hardware route selects a candidate",
  );
  check(
    HardwareKeyRouter.route(key, true, true, true).action === HardwareKeyAction.COMMIT_THEN_TYPE,
    "the preference turns the digit into text, after the composition it ends",
  );
  check(
    HardwareKeyRouter.route(key, false, true, true).action === HardwareKeyAction.RELEASE,
    "with nothing composed the digit is the editor's",
  );
});

group("a character the composition cannot use ends it before it is typed", () => {
  // The Windows host finalizes the composition and then lets the key through (`FUNCTION_FINALIZE_TEXTSTORE` in `IsVirtualKeyNeed`, `_HandleCompositionFinalize`). Releasing the key instead put the digit in the editor while the letters were still open, and the commit that followed landed after it: nihao then 0 gave 0你好.
  const key = (keyCode: number, character: number): HardwareKey => ({
    keyCode: keyCode,
    unicodeChar: character,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  });
  const zero: HardwareKeyDecision = HardwareKeyRouter.route(key(2000, 0x30), true, true);
  check(
    zero.action === HardwareKeyAction.COMMIT_THEN_TYPE && zero.character === 0x30,
    "0 finishes the composition and is typed after it",
  );
  check(
    HardwareKeyRouter.route(key(2103, 0), true, true).action === HardwareKeyAction.COMMIT_THEN_TYPE,
    "so does the keypad 0",
  );
  check(
    HardwareKeyRouter.route(key(2000, 0x30), false, true).action === HardwareKeyAction.RELEASE,
    "with nothing composed a 0 is the editor's",
  );
  check(
    HardwareKeyRouter.route(key(2000, 0x30), true, true, false, undefined, false, true).action ===
      HardwareKeyAction.COMMIT_THEN_TYPE,
    "a Japanese composition ends before a digit too",
  );
  check(
    HardwareKeyRouter.route(key(0, 0x21), true, true, false, undefined, false, true).action ===
      HardwareKeyAction.PUNCTUATION,
    "Japanese punctuation goes through the punctuation route, which also finishes the composition first",
  );
});

group("the keypad decimal point is always an ASCII full stop", () => {
  // Windows: `VK_DECIMAL` "should always commit ASCII '.'" (`KeyHandler.cpp`), finishing a composition first. Here it reached the Chinese punctuation path and came out as 。.
  const dot: HardwareKey = {
    keyCode: 2114,
    unicodeChar: 0x2e,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  };
  check(
    HardwareKeyRouter.route(dot, false, true).action === HardwareKeyAction.RELEASE,
    "with nothing composed the editor types the '.'",
  );
  const composing: HardwareKeyDecision = HardwareKeyRouter.route(dot, true, true);
  check(
    composing.action === HardwareKeyAction.COMMIT_THEN_TYPE && composing.character === 0x2e,
    "mid-composition it finishes the composition and types '.' after it",
  );
  check(
    HardwareKeyRouter.route({ ...dot, keyCode: 2044 }, false, true).action ===
      HardwareKeyAction.PUNCTUATION,
    "the main keyboard's period is still Chinese punctuation",
  );
});

group("punctuation locked to Chinese stays Chinese in English mode", () => {
  // Windows keeps the punctuation compartment on in English mode when `punctuation_lock` is Chinese (`ResolvePunctuationOpen`), and `_IsKeyEaten` claims punctuation outside `isOpen`.
  const comma: HardwareKey = {
    keyCode: 2043,
    unicodeChar: 0x2c,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  };
  const english = (locked: boolean): HardwareKeyAction =>
    HardwareKeyRouter.route(
      comma,
      false,
      false,
      false,
      undefined,
      false,
      false,
      "disabled",
      false,
      PLAIN_SPELLING,
      false,
      locked,
    ).action;
  check(english(true) === HardwareKeyAction.PUNCTUATION, "the lock keeps the comma the keyboard's");
  check(english(false) === HardwareKeyAction.RELEASE, "without it English punctuation is ASCII");
});

group("a Backspace held from inside a composition stops at its edge", () => {
  // Windows `_ApplyBackspaceHoldGuard` / `ShouldSuppressBackspaceRepeat`, issue #347.
  const guard: HardwareBackspaceGuard = new HardwareBackspaceGuard();
  check(!guard.down(true), "the first press is the composition's own backspace");
  check(!guard.down(true), "repeats that still find letters delete them");
  check(guard.down(false), "a repeat after the last letter is claimed, not handed to the editor");
  check(guard.down(false), "and so is every one after it");
  check(guard.up(false), "the key-up of a claimed hold is claimed too");
  check(!guard.down(false), "a fresh press with nothing composed is the editor's");
  check(!guard.down(false), "and so is its hold");
  check(!guard.up(false), "and its key-up");
  guard.down(true);
  guard.reset();
  check(
    !guard.down(false),
    "another key or a lost focus ends the hold, so a missed key-up cannot leave it armed",
  );
});

group("commits the highlighted candidate when punctuation arrives mid-composition", () => {
  // The source finishes the composition with the highlighted candidate and then emits the mark
  // (`IsCommitWithHighlightedCandidatePunctuationInCandidateMode` in server/src/ipc/event_listener.cpp,
  // which lists ` ! @ # $ % ^ & * ( ) [ ] ; : \ " , < . > ? ' ). Releasing the key instead leaves
  // the composition open and drops the mark into the editor in front of what is still being spelled.
  const mark = (character: string): HardwareKey => ({
    keyCode: 0,
    unicodeChar: character.charCodeAt(0),
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  });
  for (const character of ["!", "?", ";", '"', ":", "@", "#", "%", "&", "*", "(", ")"]) {
    check(
      HardwareKeyRouter.route(mark(character), true, true).action === HardwareKeyAction.PUNCTUATION,
      `${character} finishes the composition rather than escaping to the editor`,
    );
  }
  // With no composition open the mark is still the keyboard's, exactly as before.
  check(
    HardwareKeyRouter.route(mark("!"), false, true).action === HardwareKeyAction.PUNCTUATION,
    "punctuation on an empty composition stays with the keyboard",
  );
  // The keys a preference has turned into paging keys keep paging; they are consumed above, and
  // they are matched by key code rather than by the character they carry.
  check(
    HardwareKeyRouter.route(
      {
        keyCode: 2043,
        unicodeChar: ",".charCodeAt(0),
        ctrlKey: false,
        altKey: false,
        logoKey: false,
        shiftKey: false,
      },
      true,
      true,
    ).action === HardwareKeyAction.PREVIOUS_PAGE,
    "a comma bound to paging still pages",
  );
  // Japanese punctuation ends a composition the same way; with nothing composed it is the application's.
  for (const character of ["!", "?", "/", ";", "."]) {
    check(
      HardwareKeyRouter.route(mark(character), true, true, false, undefined, false, true).action ===
        HardwareKeyAction.PUNCTUATION,
      `Japanese ${character} finishes the composition rather than landing ahead of the kana`,
    );
  }
  check(
    HardwareKeyRouter.route(mark("!"), false, true, false, undefined, false, true).action ===
      HardwareKeyAction.RELEASE,
    "Japanese leaves punctuation to the application while nothing is composed",
  );
});

group("maps hardware navigation according to the shared preferences", () => {
  const navigation = {
    minusEqual: true,
    commaPeriod: true,
    brackets: false,
    tab: true,
    pageUpDown: true,
    mouseWheel: false,
    arrows: true,
  };
  const key = (keyCode: number, shiftKey: boolean = false): HardwareKey => ({
    keyCode,
    unicodeChar: 0,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey,
  });
  check(
    HardwareKeyRouter.route(key(2068), true, true, false, navigation).action ===
      HardwareKeyAction.PREVIOUS_PAGE,
    "PageUp goes to the previous page",
  );
  check(
    HardwareKeyRouter.route(key(2069), true, true, false, navigation).action ===
      HardwareKeyAction.NEXT_PAGE,
    "PageDown goes to the next page",
  );
  check(
    HardwareKeyRouter.route(key(2012), true, true, false, navigation).action ===
      HardwareKeyAction.PREVIOUS_CANDIDATE,
    "Up goes to the previous candidate",
  );
  check(
    HardwareKeyRouter.route(key(2013), true, true, false, navigation).action ===
      HardwareKeyAction.NEXT_CANDIDATE,
    "Down goes to the next candidate",
  );
  check(
    HardwareKeyRouter.route(key(2049, true), true, true, false, navigation).action ===
      HardwareKeyAction.PREVIOUS_PAGE,
    "Shift+Tab goes to the previous page",
  );
  check(
    HardwareKeyRouter.route(key(2049), true, true, false, navigation).action ===
      HardwareKeyAction.NEXT_PAGE,
    "Tab goes to the next page",
  );
  check(
    HardwareKeyRouter.route(key(2057), true, true, false, navigation).action ===
      HardwareKeyAction.PREVIOUS_PAGE,
    "minus goes to the previous page",
  );
  check(
    HardwareKeyRouter.route(key(2059), true, true, false, navigation).action ===
      HardwareKeyAction.IGNORED,
    "disabled brackets are consumed without text input",
  );
  check(
    HardwareKeyRouter.route({ ...key(2012), ctrlKey: true }, true, true, false, navigation)
      .action === HardwareKeyAction.RELEASE,
    "modifier shortcuts remain with the editor",
  );
});

group("maps Windows word-to-character bindings to highlighted candidate edges", () => {
  const key = (keyCode: number, shiftKey: boolean = false): HardwareKey => ({
    keyCode,
    unicodeChar: 0,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey,
  });
  check(
    HardwareKeyRouter.route(key(2059), true, true, false, undefined, false, false, "brackets", true)
      .action === HardwareKeyAction.WORD_CHARACTER_FIRST,
    "left bracket selects the first Han character",
  );
  check(
    HardwareKeyRouter.route(key(2060), true, true, false, undefined, false, false, "brackets", true)
      .action === HardwareKeyAction.WORD_CHARACTER_LAST,
    "right bracket selects the last Han character",
  );
  check(
    HardwareKeyRouter.route(
      key(2057),
      true,
      true,
      false,
      undefined,
      false,
      false,
      "minus_equal",
      true,
    ).action === HardwareKeyAction.WORD_CHARACTER_FIRST,
    "minus selects the first Han character",
  );
  check(
    HardwareKeyRouter.route(
      key(2058),
      true,
      true,
      false,
      undefined,
      false,
      false,
      "minus_equal",
      true,
    ).action === HardwareKeyAction.WORD_CHARACTER_LAST,
    "equals selects the last Han character",
  );
  check(
    HardwareKeyRouter.route(
      key(2059),
      true,
      true,
      false,
      undefined,
      false,
      false,
      "brackets",
      false,
    ).action !== HardwareKeyAction.WORD_CHARACTER_FIRST,
    "without a highlighted candidate the bracket remains navigation/editor input",
  );
  check(
    HardwareKeyRouter.route(
      key(2059, true),
      true,
      true,
      false,
      undefined,
      false,
      false,
      "brackets",
      true,
    ).action !== HardwareKeyAction.WORD_CHARACTER_FIRST,
    "shifted brackets stay with the editor",
  );
});

group("word-to-character answers only the configured unmodified pair", () => {
  // The cases the source pins for `WordToCharacterDirection`
  // (server/tests/src/test_input_key_policy.cpp), read against this router: the wrong pair for the
  // configured preference does nothing, any modifier at all disables it, and so does turning the
  // feature off.
  const KEY_LEFT_BRACKET: number = 2059;
  const KEY_RIGHT_BRACKET: number = 2060;
  const KEY_MINUS: number = 2057;
  const KEY_EQUALS: number = 2058;
  const press = (keyCode: number, held: Partial<HardwareKey> = {}): HardwareKey => ({
    keyCode,
    unicodeChar: 0,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
    ...held,
  });
  const edge = (keyCode: number, preference: string, held: Partial<HardwareKey> = {}) =>
    HardwareKeyRouter.route(
      press(keyCode, held),
      true,
      true,
      false,
      undefined,
      false,
      false,
      preference,
      true,
    ).action;

  check(
    edge(KEY_LEFT_BRACKET, "brackets") === HardwareKeyAction.WORD_CHARACTER_FIRST &&
      edge(KEY_RIGHT_BRACKET, "brackets") === HardwareKeyAction.WORD_CHARACTER_LAST,
    "the bracket pair answers when it is the configured one",
  );
  check(
    edge(KEY_MINUS, "minus_equal") === HardwareKeyAction.WORD_CHARACTER_FIRST &&
      edge(KEY_EQUALS, "minus_equal") === HardwareKeyAction.WORD_CHARACTER_LAST,
    "the minus/equal pair answers when it is the configured one",
  );
  // The source returns 0 for the pair that is not configured, in both directions.
  check(
    edge(KEY_LEFT_BRACKET, "minus_equal") !== HardwareKeyAction.WORD_CHARACTER_FIRST &&
      edge(KEY_RIGHT_BRACKET, "minus_equal") !== HardwareKeyAction.WORD_CHARACTER_LAST,
    "brackets do nothing while minus/equal is the configured pair",
  );
  check(
    edge(KEY_MINUS, "brackets") !== HardwareKeyAction.WORD_CHARACTER_FIRST &&
      edge(KEY_EQUALS, "brackets") !== HardwareKeyAction.WORD_CHARACTER_LAST,
    "minus/equal does nothing while brackets is the configured pair",
  );
  check(
    edge(KEY_LEFT_BRACKET, "disabled") !== HardwareKeyAction.WORD_CHARACTER_FIRST &&
      edge(KEY_MINUS, "disabled") !== HardwareKeyAction.WORD_CHARACTER_FIRST,
    "neither pair answers while the feature is off",
  );
  // `(modifiers & kKeyModifierMask) != 0` disables it in the source; here Ctrl, Alt and Meta are
  // released above the branch and Shift is excluded inside it, which comes to the same thing.
  for (const held of [
    { ctrlKey: true },
    { altKey: true },
    { logoKey: true },
    { shiftKey: true },
    { ctrlKey: true, shiftKey: true },
    { altKey: true, shiftKey: true },
    { ctrlKey: true, altKey: true },
  ]) {
    check(
      edge(KEY_LEFT_BRACKET, "brackets", held) !== HardwareKeyAction.WORD_CHARACTER_FIRST &&
        edge(KEY_MINUS, "minus_equal", held) !== HardwareKeyAction.WORD_CHARACTER_FIRST,
      `a held ${Object.keys(held).join("+")} stops the pair from selecting a character`,
    );
  }
});

group("extracts Han characters for word-to-character fallback", () => {
  check(
    CandidateTextPolicy.extractHanCharacter("abc中b文", CandidateTextEdge.FIRST) === "中",
    "first Han character is selected",
  );
  check(
    CandidateTextPolicy.extractHanCharacter("abc中b文", CandidateTextEdge.LAST) === "文",
    "last Han character is selected",
  );
  check(
    CandidateTextPolicy.extractHanCharacter("𠀀a", CandidateTextEdge.FIRST) === "𠀀",
    "supplementary Han character is preserved",
  );
  check(
    CandidateTextPolicy.extractHanCharacter("abc", CandidateTextEdge.FIRST) === null,
    "non-Han candidate has no fallback",
  );
  check(
    CandidateTextPolicy.extractHanCharacter("", CandidateTextEdge.LAST) === null,
    "empty candidate has no fallback",
  );
});

group("maps hardware composition editing commands like Windows", () => {
  const navigation = {
    minusEqual: true,
    commaPeriod: true,
    brackets: false,
    tab: true,
    pageUpDown: true,
    mouseWheel: false,
    arrows: true,
  };
  const key = (
    keyCode: number,
    ctrlKey: boolean = false,
    shiftKey: boolean = false,
  ): HardwareKey => ({
    keyCode,
    unicodeChar: 0,
    ctrlKey,
    altKey: false,
    logoKey: false,
    shiftKey,
  });
  check(
    HardwareKeyRouter.route(key(2014), true, true).action === HardwareKeyAction.MOVE_LEFT,
    "left moves within the composition",
  );
  check(
    HardwareKeyRouter.route(key(2015), true, true).action === HardwareKeyAction.MOVE_RIGHT,
    "right moves within the composition",
  );
  check(
    HardwareKeyRouter.route(key(2081), true, true).action === HardwareKeyAction.MOVE_HOME,
    "Home moves to the start",
  );
  check(
    HardwareKeyRouter.route(key(2082), true, true).action === HardwareKeyAction.MOVE_END,
    "End moves to the end",
  );
  check(
    HardwareKeyRouter.route(key(2071), true, true).action === HardwareKeyAction.DELETE_FORWARD,
    "Delete removes the next unit",
  );
  check(
    HardwareKeyRouter.route(key(2055, true), true, true).action ===
      HardwareKeyAction.BACKSPACE_SEGMENT,
    "Ctrl+Backspace removes one segment",
  );
  check(
    HardwareKeyRouter.route(key(2014, true), true, true).action ===
      HardwareKeyAction.MOVE_LEFT_SEGMENT,
    "Ctrl+Left moves one segment left",
  );
  check(
    HardwareKeyRouter.route(key(2015, true), true, true).action ===
      HardwareKeyAction.MOVE_RIGHT_SEGMENT,
    "Ctrl+Right moves one segment right",
  );
  check(
    HardwareKeyRouter.route(key(2055, true, true), true, true).action === HardwareKeyAction.RELEASE,
    "Shift+Ctrl remains an editor shortcut",
  );
  // A Ctrl+Backspace that empties the reading of a half-chosen phrase leaves only the chosen piece, as the Windows `keep_creating_word_after_empty_raw` does. It is still a composition, so the next Backspace, Ctrl+Backspace, Enter and Escape stay with the session.
  const heldOnly: boolean = HardwareKeyRouter.composing("", "海滩");
  check(heldOnly, "a held phrase piece with no reading is a composition");
  check(HardwareKeyRouter.composing("paobu", ""), "a reading is a composition");
  check(!HardwareKeyRouter.composing("", ""), "nothing held and nothing typed is no composition");
  check(
    HardwareKeyRouter.route(key(2055, true), heldOnly, true).action ===
      HardwareKeyAction.BACKSPACE_SEGMENT,
    "Ctrl+Backspace deletes the held piece once the reading is gone",
  );
  check(
    HardwareKeyRouter.route(key(2055, true), false, true).action === HardwareKeyAction.RELEASE,
    "Ctrl+Backspace with nothing composed is the editor's",
  );
  for (const code of [2055, 2054, 2070]) {
    check(
      HardwareKeyRouter.route(key(code), heldOnly, true).action ===
        HardwareKeyRouter.route(key(code), true, true).action &&
        HardwareKeyRouter.route(key(code), heldOnly, true).action !== HardwareKeyAction.RELEASE,
      `key ${code} stays with the session while only a phrase piece is held`,
    );
  }
  check(
    HardwareKeyRouter.route(key(2054, true), true, true, false, navigation, true).action ===
      HardwareKeyAction.COMMIT_TRANSLATION,
    "Ctrl+Enter commits a highlighted candidate translation",
  );
  // Windows claims Ctrl+Enter whenever candidates are up and answers NavigationIgnored without a translation (`HandleTranslationCommitKey`); a chat application must not send the message with the spelling still open.
  check(
    HardwareKeyRouter.route(key(2054, true), true, true, false, navigation, false).action ===
      HardwareKeyAction.IGNORED,
    "Ctrl+Enter without a translation is consumed mid-composition",
  );
  check(
    HardwareKeyRouter.route(key(2119, true), true, true, false, navigation, false).action ===
      HardwareKeyAction.IGNORED,
    "and so is the keypad Enter",
  );
  check(
    HardwareKeyRouter.route(key(2054, true), false, true, false, navigation, false).action ===
      HardwareKeyAction.RELEASE,
    "with nothing composed Ctrl+Enter is the application's",
  );
  const japaneseMinus: HardwareKey = {
    keyCode: 2057,
    unicodeChar: "-".charCodeAt(0),
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
  };
  check(
    HardwareKeyRouter.route(japaneseMinus, false, true, false, navigation, false, true).action ===
      HardwareKeyAction.COMPOSE,
    "Japanese minus starts a long-vowel composition",
  );
  check(
    HardwareKeyRouter.route(japaneseMinus, false, true, false, navigation, false, true)
      .character === "-".charCodeAt(0),
    "Japanese minus reaches the Engine as a hyphen",
  );
  check(
    HardwareKeyRouter.route(japaneseMinus, true, true, false, navigation, false, true).action ===
      HardwareKeyAction.COMPOSE,
    "Japanese minus remains a long-vowel composition key",
  );
  // Japanese '=' and '_' never page (`IsJapaneseDisabledPagingKey`), even with minus/equals paging on; mid-composition they commit the highlighted candidate and then the mark, as the source's commit-with-highlighted-candidate list does.
  const japaneseMark = (keyCode: number, character: string, shiftKey: boolean): HardwareKey => ({
    keyCode,
    unicodeChar: character.charCodeAt(0),
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey,
  });
  const equals: HardwareKey = japaneseMark(2058, "=", false);
  const underscore: HardwareKey = japaneseMark(2057, "_", true);
  check(
    HardwareKeyRouter.route(equals, true, true, false, navigation, false, true).action ===
      HardwareKeyAction.PUNCTUATION,
    "Japanese equals mid-composition commits and then types the mark",
  );
  check(
    HardwareKeyRouter.route(underscore, true, true, false, navigation, false, true).action ===
      HardwareKeyAction.PUNCTUATION,
    "so does Shift+minus",
  );
  check(
    HardwareKeyRouter.route(equals, false, true, false, navigation, false, true).action ===
      HardwareKeyAction.RELEASE,
    "with nothing composed Japanese equals is the application's",
  );
  check(
    HardwareKeyRouter.route(key(2050), true, true, false, navigation, false, true).action ===
      HardwareKeyAction.JAPANESE_CONVERT,
    "Japanese Space starts or advances conversion without committing",
  );
  check(
    HardwareKeyRouter.route(key(2054), true, true, false, navigation, false, true).action ===
      HardwareKeyAction.JAPANESE_COMMIT,
    "Japanese Return commits conversion state rather than raw romaji",
  );
});

group("routes Chinese hardware punctuation without stealing editor navigation", () => {
  const key = (
    unicodeChar: number,
    keyCode: number = 0,
    modifiers: Partial<HardwareKey> = {},
  ): HardwareKey => ({
    keyCode,
    unicodeChar,
    ctrlKey: false,
    altKey: false,
    logoKey: false,
    shiftKey: false,
    ...modifiers,
  });
  check(
    HardwareKeyRouter.route(key(0x2c), false, true).action === HardwareKeyAction.PUNCTUATION,
    "Chinese hardware comma uses the shared punctuation route",
  );
  check(
    HardwareKeyRouter.route(key(0x3f), false, true).character === 0x3f,
    "hardware punctuation preserves the ASCII key",
  );
  check(
    HardwareKeyRouter.route(key(0x2c), false, false).action === HardwareKeyAction.RELEASE,
    "English punctuation remains application-owned",
  );
  check(
    HardwareKeyRouter.route(key(0x2c), false, true, false, undefined, false, true).action ===
      HardwareKeyAction.RELEASE,
    "Japanese punctuation remains application-owned",
  );
  check(
    HardwareKeyRouter.route(key(0x2c, 2043), true, true).action === HardwareKeyAction.PREVIOUS_PAGE,
    "comma keeps candidate paging while composing",
  );
  check(
    HardwareKeyRouter.route(key(0x2c, 2043, { ctrlKey: true }), false, true).action ===
      HardwareKeyAction.RELEASE,
    "modified punctuation is left to the application",
  );
});

group("splits candidate translation senses for Ctrl+Enter", () => {
  check(
    TranslationSensePolicy.split("你好；您好；喂").join("|") === "你好|您好|喂",
    "fullwidth semicolons become separate translation choices",
  );
  check(
    TranslationSensePolicy.split("hello; greeting ; hello").join("|") === "hello|greeting",
    "ASCII senses are trimmed and deduplicated",
  );
  check(TranslationSensePolicy.split("； ; ").length === 0, "empty translation senses are ignored");
});

group("fullwidth conversion maps space to the ideographic form", () => {
  check(
    FullWidthInputPolicy.output("a", true) === "ａ",
    "a printable ASCII letter shifts by 0xfee0",
  );
  check(
    FullWidthInputPolicy.output(" ", true) === "\u3000",
    "space becomes the ideographic space, not the fullwidth space",
  );
  check(FullWidthInputPolicy.output("~", true) === "～", "the top of the printable range converts");
  check(FullWidthInputPolicy.output("\n", true) === "\n", "a control character is left alone");
  check(FullWidthInputPolicy.output("中", true) === "中", "non-ASCII is left alone");
  check(FullWidthInputPolicy.output("a", false) === "a", "disabled means unchanged");
  check(FullWidthInputPolicy.output("", true) === "", "empty stays empty");
  check(
    FullWidthInputPolicy.output("😀", true) === "😀",
    "an astral character survives rather than being split",
  );
});

group("clipboard entries are bounded in characters and in UTF-8 bytes", () => {
  check(ClipboardHistoryPolicy.acceptable("hello") === true, "ordinary text is kept");
  check(ClipboardHistoryPolicy.acceptable("   ") === false, "whitespace only is not worth keeping");
  check(ClipboardHistoryPolicy.acceptable("") === false, "empty is not worth keeping");
  check(ClipboardHistoryPolicy.acceptable(null) === false, "null is safe");
  check(
    ClipboardHistoryPolicy.acceptable("a".repeat(ClipboardHistoryPolicy.MAX_CHARS)) === true,
    "exactly at the character bound is accepted",
  );
  check(
    ClipboardHistoryPolicy.acceptable("a".repeat(ClipboardHistoryPolicy.MAX_CHARS + 1)) === false,
    "one character over is refused",
  );
  // The byte bound is measured in UTF-8 and counts multi-byte characters accordingly.
  const wide = "中".repeat(1000);
  check(wide.length === 1000, "a thousand UTF-16 units");
  check(ClipboardHistoryPolicy.acceptable(wide) === true, "three thousand bytes is well inside");
  // Worth recording: with MAX_CHARS at 10000 UTF-16 units, the worst case is 5000 astral characters
  // at four bytes each, or 20000 bytes. The byte bound of 40000 is therefore unreachable through the
  // character bound and is purely defensive. The Java original has the same property.
  const astral = "😀".repeat(ClipboardHistoryPolicy.MAX_CHARS / 2);
  check(astral.length === ClipboardHistoryPolicy.MAX_CHARS, "exactly at the character bound");
  check(
    ClipboardHistoryPolicy.acceptable(astral) === true,
    "the heaviest text the character bound allows is still inside the byte bound",
  );
  check(
    ClipboardHistoryPolicy.acceptable("😀".repeat(6000)) === true,
    "astral characters count once rather than as two UTF-16 units",
  );
  check(
    ClipboardHistoryPolicy.acceptable("a\u0000b") === false,
    "NUL cannot be persisted by the shared store",
  );
});

group("diagnostics are trimmed, bounded and elided", () => {
  check(InputDiagnosticPolicy.normalize("  hi  ") === "hi", "surrounding whitespace goes");
  check(InputDiagnosticPolicy.normalize("   ") === "", "whitespace only becomes empty");
  check(InputDiagnosticPolicy.normalize(null) === "", "null becomes empty");
  const long = "x".repeat(InputDiagnosticPolicy.MAX_LENGTH + 100);
  const bounded = InputDiagnosticPolicy.normalize(long);
  check(
    bounded.length === InputDiagnosticPolicy.MAX_LENGTH,
    "an overlong diagnostic is cut to the bound including the ellipsis",
  );
  check(bounded.endsWith("…"), "and says it was cut");
  check(InputDiagnosticPolicy.visible("hi") === true, "a real diagnostic shows");
  check(InputDiagnosticPolicy.visible("   ") === false, "an empty one does not");
});

group("traditional output never loses text when conversion fails", () => {
  const upper = (text: string): string => text.toUpperCase();
  check(
    ChineseOutputPolicy.output("ab", true, true, upper) === "AB",
    "a working converter is used",
  );
  check(
    ChineseOutputPolicy.output("ab", false, true, upper) === "ab",
    "simplified output is untouched",
  );
  check(
    ChineseOutputPolicy.output("ab", true, false, upper) === "ab",
    "a context the policy does not apply to is untouched",
  );
  check(
    ChineseOutputPolicy.output("ab", true, true, () => null) === "ab",
    "a converter returning nothing falls back to the original",
  );
  check(
    ChineseOutputPolicy.output("ab", true, true, () => {
      throw new Error("boom");
    }) === "ab",
    "a converter that throws must not lose the text",
  );
  // The host hands the policy the native OpenCC s2t converter, which works on phrases: 发 is 髮 in 头发 and 發 in 发展. The policy passes the whole string through rather than splitting it, which is what lets the phrase tables see the word.
  const phrase = (text: string): string => text.replace("头发", "頭髮").replace("发展", "發展");
  check(
    ChineseOutputPolicy.output("头发", true, true, phrase) === "頭髮",
    "头发 converts as a phrase",
  );
  check(
    ChineseOutputPolicy.output("发展", true, true, phrase) === "發展",
    "发展 converts as a phrase",
  );
  check(ChineseOutputPolicy.applies(false, 0, "none") === true, "quanpin converts");
  check(ChineseOutputPolicy.applies(true, 0, "none") === false, "dedicated English does not");
  check(ChineseOutputPolicy.applies(false, 3, "none") === false, "Japanese has nothing to convert");
  check(
    ChineseOutputPolicy.applies(false, 0, "temporary_japanese") === false,
    "temporary Japanese has nothing to convert either",
  );
});

group("local modes are addressable by trigger and by preference key", () => {
  check(LocalInputMode.MODES.length === 8, "eight local modes");
  const unicode = LocalInputMode.fromTrigger("U");
  check(unicode !== null && unicode.preferenceKey === "unicode", "U enters Unicode code points");
  const emoji = LocalInputMode.fromPreferenceKey("emoji");
  check(emoji !== null && emoji.trigger === "E", "emoji is entered with E");
  check(LocalInputMode.fromTrigger("Z") === null, "an unassigned letter enters nothing");
  const triggers = new Set(LocalInputMode.MODES.map((entry) => entry.trigger));
  check(triggers.size === LocalInputMode.MODES.length, "no two modes share a trigger");
});

group("voice providers share one recording session", () => {
  check(
    VoiceSessionPolicy.canStart(false, false, false, false, false),
    "an idle keyboard can start voice recognition",
  );
  check(
    !VoiceSessionPolicy.canStart(true, false, false, false, false) &&
      !VoiceSessionPolicy.canStart(false, true, false, false, false) &&
      !VoiceSessionPolicy.canStart(false, false, true, false, false) &&
      !VoiceSessionPolicy.canStart(false, false, false, true, false) &&
      !VoiceSessionPolicy.canStart(false, false, false, false, true),
    "a second provider cannot overlap any active microphone session",
  );
});

group("quick punctuation prints one glyph and sends another", () => {
  const chinese: PunctuationEntry[] = QuickPunctuationPolicy.entries(false, 0, "none");
  check(
    chinese[0].face === "，" && chinese[0].input === ",",
    "the Chinese comma is drawn but an ASCII comma is sent",
  );
  check(
    chinese[4].face === "、" && chinese[4].input === "\\",
    "the enumeration comma is reached through backslash",
  );
  const japanese: PunctuationEntry[] = QuickPunctuationPolicy.entries(false, 3, "none");
  check(japanese[0].face === "、", "Japanese leads with the enumeration comma");
  check(japanese[4].face === "「" && japanese[4].input === "[", "Japanese brackets come from [");
  const ascii: PunctuationEntry[] = QuickPunctuationPolicy.entries(true, 0, "none");
  check(ascii[0].face === "," && ascii[0].input === ",", "dedicated English sends what it draws");
  check(
    QuickPunctuationPolicy.entries(false, 0, "emoji")[0].face === ",",
    "a local mode falls back to ASCII",
  );
});

group("bounds Harmony smart punctuation editor context", () => {
  check(SmartPunctuationContext.precedingCodePoint(null) === 0, "missing context is unavailable");
  check(
    SmartPunctuationContext.precedingCodePoint("fixture7") === "7".charCodeAt(0),
    "ASCII digit context is preserved",
  );
  check(
    SmartPunctuationContext.precedingCodePoint("fixture中") === 0x4e2d,
    "CJK context is preserved for the shared policy",
  );
  check(
    SmartPunctuationContext.precedingCodePoint("fixture🌲") === 0x1f332,
    "supplementary context is reduced to one scalar",
  );
  check(
    SmartPunctuationContext.precedingCodePoint("fixture\ud800") === 0,
    "unpaired surrogate context is rejected",
  );
});

group("replaces a repeated smart ASCII mark only in the same short-lived editor state", () => {
  const first: SmartPunctuationRepeatSnapshot | null = SmartPunctuationRepeatPolicy.snapshot(
    0x2c,
    ",",
    1000,
    4,
  );
  check(first !== null, "ASCII comma arms the repeat state");
  check(
    SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 2999, 4, true, true, false, 0),
    "same mark and editor within two seconds replaces",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 3001, 4, true, true, false, 0),
    "the repeat window expires",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2e, 0x2c, 1500, 4, true, true, false, 0),
    "a different mark does not replace",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 1500, 5, true, true, false, 0),
    "a different editor does not replace",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 1500, 4, true, true, true, 0),
    "composition blocks replacement",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 1500, 4, true, true, false, 1),
    "candidates block replacement",
  );
  check(
    !SmartPunctuationRepeatPolicy.shouldReplace(first, 0x2c, 0x2c, 1500, 4, false, true, false, 0),
    "smart punctuation can disable replacement",
  );
  check(
    SmartPunctuationRepeatPolicy.chineseMark(0x2e) === "。",
    "period maps to ideographic full stop",
  );
  check(SmartPunctuationRepeatPolicy.chineseMark(0x3a) === "：", "colon maps to full-width colon");
  check(
    SmartPunctuationRepeatPolicy.snapshot(0x2c, "，", 1000, 4) !== null,
    "a full-width ASCII mark remains replaceable when full-width input is enabled",
  );
});

group("completes Engine opening punctuation only when paired mode is enabled", () => {
  check(
    PairedPunctuationPolicy.completion("“", true)?.closing === "”",
    "double quote opens a Chinese pair",
  );
  check(
    PairedPunctuationPolicy.completion("拟（", true)?.closing === "）",
    "a composition commit can end in an opening parenthesis",
  );
  check(
    PairedPunctuationPolicy.completion("《", true)?.opening === 0x3c,
    "book title marks retain the balance input",
  );
  check(
    PairedPunctuationPolicy.completion("”", true) === null,
    "a closing quote is not completed again",
  );
  check(
    PairedPunctuationPolicy.completion("“", false) === null,
    "disabled paired punctuation leaves the editor to the Engine",
  );
  check(
    PairedPunctuationPolicy.completion("", true) === null,
    "an empty commit cannot open a pair",
  );
});

group("with pairing on every quote press opens a fresh pair, as the reference does", () => {
  // The Engine alternates the quote keys, so after an auto-closed “” the next press arrives as ”.
  const reopened = PairedPunctuationPolicy.reopenQuote("”", 0x22, true);
  check(reopened === "“", "a closing double quote from the Engine is rewritten to the opening one");
  check(
    PairedPunctuationPolicy.completion(reopened, true)?.closing === "”",
    "and the rewritten quote is then completed to a pair",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("’", 0x27, true) === "‘",
    "single quotes are rewritten the same way",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("你好”", 0x22, true) === "你好“",
    "a composition committed ahead of the quote keeps its text",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("“", 0x22, true) === "“",
    "an opening quote is left as it is",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("”", 0x22, false) === "”",
    "with pairing off the Engine's alternation reaches the editor unchanged",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("’", 0x22, true) === "’",
    "only the quote key that was pressed is rewritten",
  );
  check(
    PairedPunctuationPolicy.reopenQuote("）", 0x29, true) === "）",
    "other closing marks are not quotes and stay closing",
  );
  check(
    PairedPunctuationPolicy.reopenQuote(null, 0x22, true) === null,
    "no commit stays no commit",
  );
});

group("return performs an editor action only when nothing else claimed it", () => {
  const send = 4;
  check(ReturnKeyAction.performsEditorAction(send, false) === true, "send is an editor action");
  check(ReturnKeyAction.performsEditorAction(0, false) === false, "unspecified is not");
  check(ReturnKeyAction.performsEditorAction(1, false) === false, "none is not");
  check(
    ReturnKeyAction.performsEditorAction(send, true) === false,
    "a disabled action is not performed",
  );
  check(
    ReturnKeyAction.shouldPerformEditorAction(send, false, true) === false,
    "a handled composition consumes Return first",
  );
  check(
    ReturnKeyAction.shouldPerformEditorAction(send, false, false) === true,
    "otherwise the editor action runs",
  );
  check(ReturnKeyAction.title(send, false) === "发送", "the key says what it will do");
  check(ReturnKeyAction.title(send, true) === "换行", "a disabled action falls back to newline");
  check(ReturnKeyAction.title(0, false) === "换行", "an unspecified action is a newline");
  check(
    ReturnKeyAction.dispatch(true, true, 0) === ReturnDispatch.COMMIT_READING,
    "Japanese Return commits unconverted kana as its reading",
  );
  check(
    ReturnKeyAction.dispatch(true, true, 3, false) === ReturnDispatch.COMMIT_READING,
    "visible Japanese candidates do not imply that Space started conversion",
  );
  check(
    ReturnKeyAction.dispatch(true, true, 3, true) === ReturnDispatch.COMMIT_HIGHLIGHTED,
    "Japanese Return commits the selected conversion when candidates exist",
  );
  check(
    ReturnKeyAction.dispatch(false, true, 0) === ReturnDispatch.FINISH_COMPOSITION,
    "a candidate-less non-Japanese composition is still finished before Return",
  );
  check(
    ReturnKeyAction.dispatch(false, false, 0) === ReturnDispatch.EDITOR,
    "an idle Return belongs to the editor",
  );

  // Kept as a description of the Android split rather than of this host: HarmonyOS hands the enter
  // key type to sendKeyFunction and the framework resolves it, newline included, so nothing here
  // has to choose. Measured — see the ALLOWED entry in test-harmony-unwired-symbols.py.
  for (const action of [0, 1, 2, 3, 4, 5, 6, 7]) {
    const newline: boolean = ReturnKeyAction.title(action, false) === "换行";
    check(
      ReturnKeyAction.shouldPerformEditorAction(action, false, false) === !newline,
      "the split agrees with the face it would have drawn",
    );
  }
});

group("the Japanese variant key needs kana already composing", () => {
  check(JapaneseVariantPolicy.enabled(true, false, true) === true, "composing kana enables it");
  check(
    JapaneseVariantPolicy.enabled(true, false, false) === false,
    "nothing composed, nothing to vary",
  );
  check(
    JapaneseVariantPolicy.enabled(true, true, true) === false,
    "the symbol layer has no variants",
  );
  check(
    JapaneseVariantPolicy.enabled(false, false, true) === false,
    "other schemes have no variant key",
  );
  check(
    JapaneseVariantPolicy.accessibilityLabel(false).includes("请先输入假名"),
    "the disabled announcement says what to do first",
  );
});

group("on the digit layer the same cell holds the brackets instead", () => {
  // MSIME-Apple's JapaneseNineKeyView.setDigits: the post-modifier has nothing to modify once the
  // keys stop producing kana, so the slot goes to the brackets, which have no other home on this
  // layout. Here the cell was simply dead — enabled() is false for the whole digit layer.
  check(
    JapaneseVariantPolicy.brackets(true, true) === true,
    "the Japanese digit layer turns the cell into the bracket key",
  );
  check(
    JapaneseVariantPolicy.brackets(true, false) === false,
    "the kana layer keeps it as the variant key",
  );
  check(
    JapaneseVariantPolicy.brackets(false, true) === false,
    "and no other scheme has this cell at all",
  );
  check(
    JapaneseVariantPolicy.enabled(true, true, true) === false &&
      JapaneseVariantPolicy.brackets(true, true) === true,
    "the two jobs never overlap, so one key can carry both",
  );

  const brackets: string[] = JapaneseNineKeyLayout.digitBrackets();
  check(brackets.length === 8, "eight of them, as the source lists");
  check(new Set(brackets).size === brackets.length, "and no repeats");
  for (const bracket of brackets) {
    check([...bracket].length === 1, "each is one character, inserted as it stands");
  }
  check(
    !JapaneseVariantPolicy.bracketsLabel().includes("（）"),
    "the spoken name is not the pair it draws, which reads as two characters",
  );
});

console.log("Candidate and cursor policies");

group("space drag accumulates whole steps and carries the remainder", () => {
  const drag = new SpaceCursorMovement();
  const document = {};
  check(drag.isActive() === false, "inactive before it begins");
  drag.begin(100, document);
  check(drag.isActive() === true, "active after begin");
  check(drag.advance(110, document, 20) === 0, "half a step moves nothing yet");
  check(drag.advance(120, document, 20) === 1, "the carried remainder completes the step");
  check(drag.advance(100, document, 20) === -1, "dragging back moves back");
});

group("a drag against a different document is abandoned, not redirected", () => {
  const drag = new SpaceCursorMovement();
  const first = {};
  const second = {};
  drag.begin(100, first);
  check(drag.advance(200, second, 20) === 0, "another editor gets no movement");
  check(drag.isActive() === false, "and the drag is cancelled rather than transferred");
});

group("implausible input cancels the drag rather than clamping it", () => {
  const drag = new SpaceCursorMovement();
  const document = {};
  drag.begin(0, document);
  check(drag.advance(99999, document, 20) === 0, "a jump beyond the bound yields nothing");
  check(drag.isActive() === false, "and cancels: the gesture was already lost");

  const nan = new SpaceCursorMovement();
  nan.begin(Number.NaN, document);
  check(nan.isActive() === false, "beginning at a non-finite position never starts");

  const zeroStep = new SpaceCursorMovement();
  zeroStep.begin(0, document);
  check(zeroStep.advance(100, document, 0) === 0, "a step size below one pixel is refused");
  check(zeroStep.isActive() === false, "and cancels");

  const noDocument = new SpaceCursorMovement();
  noDocument.begin(0, null);
  check(noDocument.isActive() === false, "beginning without a document never starts");
});

group("menu ids follow declaration order and round-trip", () => {
  check(CandidateManagementAction.ACTIONS.length === 4, "four management actions");
  for (const entry of CandidateManagementAction.ACTIONS) {
    const resolved: ManagementAction = CandidateManagementAction.fromMenuItemId(entry.menuItemId);
    check(resolved === entry, `${entry.id} round-trips through its menu id`);
  }
  check(
    CandidateManagementAction.REMOVE.confirmationRequired === true,
    "deleting an entry asks first",
  );
  check(CandidateManagementAction.PROMOTE.confirmationRequired === false, "promoting one does not");
  assertThrows(
    () => CandidateManagementAction.fromMenuItemId(999),
    /Unknown candidate/,
    "rejects invalid input",
  );
  assertThrows(
    () => CandidateManagementAction.fromMenuItemId(1004),
    /Unknown candidate/,
    "rejects invalid input",
  );
  checks += 2;
});

group("only the fix-first action names a position", () => {
  check(
    CandidateManagementAction.fixedPosition(CandidateManagementAction.FIX_FIRST) === 1,
    "fixing means position one",
  );
  assertThrows(
    () => CandidateManagementAction.fixedPosition(CandidateManagementAction.PROMOTE),
    /does not fix a position/,
    "rejects invalid input",
  );
  check(CandidateManagementAction.validatePosition(1) === 1, "position one is valid");
  check(CandidateManagementAction.validatePosition(5) === 5, "position five is valid");
  assertThrows(
    () => CandidateManagementAction.validatePosition(0),
    /between 1 and 5/,
    "rejects invalid input",
  );
  assertThrows(
    () => CandidateManagementAction.validatePosition(6),
    /between 1 and 5/,
    "rejects invalid input",
  );
  checks += 3;
});

group("a stale gloss is dropped rather than painted onto the current candidates", () => {
  const token: GlossToken = CandidateGlossPolicy.token(1, 5, 2);
  check(CandidateGlossPolicy.isCurrent(token, 1, 5, 2) === true, "the matching state accepts it");
  check(CandidateGlossPolicy.isCurrent(token, 2, 5, 2) === false, "another session rejects it");
  check(CandidateGlossPolicy.isCurrent(token, 1, 6, 2) === false, "a newer generation rejects it");
  check(CandidateGlossPolicy.isCurrent(token, 1, 5, 3) === false, "a newer epoch rejects it");
  assertThrows(
    () => CandidateGlossPolicy.token(0, 0, 0),
    /Invalid candidate gloss token/,
    "rejects invalid input",
  );
  checks++;
});

group("an engine annotation outranks a gloss in the shared hint slot", () => {
  check(
    CandidateGlossPolicy.annotation("ggll", "green", true) === "ggll",
    "the wubi code wins the slot",
  );
  check(
    CandidateGlossPolicy.annotation("", "green", true) === "green",
    "with no annotation the gloss shows",
  );
  check(
    CandidateGlossPolicy.annotation(null, "green", false) === "",
    "a disabled gloss shows nothing",
  );
  check(CandidateGlossPolicy.annotation(null, null, true) === "", "no gloss shows nothing");
  const long = "x".repeat(5000);
  check(
    CandidateGlossPolicy.annotation(null, long, true) === "",
    "an implausibly long gloss is refused rather than rendered",
  );
  check(
    CandidateGlossPolicy.accessibilitySuffix("ggll", "green", true) === "，提示：ggll",
    "the announcement calls an annotation a hint",
  );
  check(
    CandidateGlossPolicy.accessibilitySuffix(null, "green", true) === "，英文释义：green",
    "and calls a gloss a definition",
  );
  check(
    CandidateGlossPolicy.accessibilitySuffix(null, null, true) === "",
    "with neither there is nothing to announce",
  );
});

group("the candidate family list names the English font ahead of the Chinese one", () => {
  // ArkUI resolves the list per glyph, so the order is the whole mechanism: Latin comes from the
  // first family that has it, Han falls through to the one behind.
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", "Segoe UI", ["Microsoft YaHei"]) ===
      "Segoe UI, Noto Sans SC, Microsoft YaHei",
    "English first, then Chinese, then fallbacks",
  );
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", null, ["Microsoft YaHei"]) ===
      "Noto Sans SC, Microsoft YaHei",
    "with no English font one family answers for everything",
  );
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", "  ", ["Microsoft YaHei"]) ===
      "Noto Sans SC, Microsoft YaHei",
    "and a blank one is the same as none",
  );
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", "Noto Sans SC", []) === "Noto Sans SC",
    "naming the same family twice does not repeat it",
  );
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", "A, B", []) === "Noto Sans SC",
    "a name carrying a comma is refused rather than split into two families",
  );
  check(
    CandidateFontFamilyPolicy.families("Noto Sans SC", null, ["", " ", "Microsoft YaHei"]) ===
      "Noto Sans SC, Microsoft YaHei",
    "empty fallback entries never reach the renderer",
  );
});

group("the two candidate annotations read their own shared preferences", () => {
  check(
    CandidateAnnotationPreferencePolicy.wubiCodeHint(undefined) === true,
    "an omitted wubi hint keeps the shared default-on behaviour",
  );
  check(
    CandidateAnnotationPreferencePolicy.wubiCodeHint(null) === true,
    "a null wubi hint is the same omission",
  );
  check(CandidateAnnotationPreferencePolicy.wubiCodeHint(true) === true, "an explicit yes is on");
  check(
    CandidateAnnotationPreferencePolicy.wubiCodeHint(false) === false,
    "only an explicit no turns the wubi hint off",
  );
  check(
    CandidateAnnotationPreferencePolicy.englishGloss(true) === true,
    "the gloss shows when the document says so",
  );
  check(
    CandidateAnnotationPreferencePolicy.englishGloss(false) === false,
    "and hides when it does not",
  );
  check(
    CandidateAnnotationPreferencePolicy.englishGloss(undefined) === false,
    "an absent gloss preference stays off, matching the shared default",
  );
  check(
    CandidateAnnotationPreferencePolicy.englishGloss("true") === false,
    "a malformed gloss value is not read as consent",
  );
});

group("a capture device choice is only read when it names this host", () => {
  check(VoiceCaptureDevicePolicy.selects("") === true, "no choice leaves the system default");
  check(VoiceCaptureDevicePolicy.selects("auto") === true, "auto leaves the system default");
  check(VoiceCaptureDevicePolicy.selects(null) === true, "an absent backend is no choice");
  check(
    VoiceCaptureDevicePolicy.selects("harmony") === true,
    "this host reads its own enumeration",
  );
  check(
    VoiceCaptureDevicePolicy.selects("windows") === false,
    "a windows endpoint id is not reinterpreted as a harmony device",
  );
  check(VoiceCaptureDevicePolicy.selects("pulse") === false, "nor is a pulse source name");
});

group("a capture device id survives a restart", () => {
  check(
    VoiceCaptureDevicePolicy.stableId(15, "") === "15:",
    "the built-in microphone has no address, and that is itself stable",
  );
  check(
    VoiceCaptureDevicePolicy.stableId(8, "00:11:22:33:44:55") === "8:00:11:22:33:44:55",
    "a bluetooth headset is named by its address",
  );
  check(
    VoiceCaptureDevicePolicy.stableId(8, "a\u0000b") === "8:ab",
    "a control character never reaches the preference file",
  );
  check(VoiceCaptureDevicePolicy.stableId(-1, "") === "", "a nonsense device type produces no id");
  check(
    VoiceCaptureDevicePolicy.stableId(8, "x".repeat(300)) === "",
    "an implausibly long address is refused rather than stored",
  );
});

group("a capture device shows the most specific name it has", () => {
  check(
    VoiceCaptureDevicePolicy.label("USB 麦克风", "usb-mic", 8) === "USB 麦克风",
    "the display name wins",
  );
  check(VoiceCaptureDevicePolicy.label("", "usb-mic", 8) === "usb-mic", "then the device name");
  check(
    VoiceCaptureDevicePolicy.label(null, null, 15) === "录音设备 15",
    "and with neither the type still says which device it is",
  );
  check(
    VoiceCaptureDevicePolicy.label("a".repeat(200), "", 8).length === 128,
    "an implausibly long name is bounded rather than rendered whole",
  );
});

group("an absent capture device falls back rather than failing the recording", () => {
  const devices: VoiceCaptureDevice[] = [
    { backend: HARMONY_CAPTURE_BACKEND, id: "15:", label: "内置麦克风" },
    { backend: HARMONY_CAPTURE_BACKEND, id: "8:aa", label: "蓝牙耳机" },
  ];
  const chosen = VoiceCaptureDevicePolicy.match(devices, "harmony", "8:aa");
  check(chosen !== null && chosen.label === "蓝牙耳机", "the stored choice is found");
  check(
    VoiceCaptureDevicePolicy.match(devices, "harmony", "8:bb") === null,
    "a headset that was unplugged leaves the system default rather than refusing to record",
  );
  check(
    VoiceCaptureDevicePolicy.match(devices, "windows", "15:") === null,
    "another host\u0027s backend is not matched against this enumeration",
  );
  check(
    VoiceCaptureDevicePolicy.match(devices, "", "") === null,
    "no choice is the system default",
  );
});

group("the input mode is remembered per application", () => {
  const scope: ImeModeScopePolicy = new ImeModeScopePolicy();
  scope.activate("com.example.chat", "app");
  check(scope.mode(false) === false, "an application nobody has typed in yet opens at the default");
  scope.remember(true);
  scope.activate("com.example.mail", "app");
  check(scope.mode(false) === false, "another application is unaffected by that choice");
  scope.remember(false);
  scope.activate("com.example.chat", "app");
  check(scope.mode(false) === true, "and coming back finds the mode this one was left in");
  scope.activate(null, "app");
  check(scope.mode(true) === true, "an editor with no bundle name falls back to the default");
  scope.remember(false);
  scope.activate(null, "app");
  check(scope.mode(true) === true, "and nothing was recorded against it");
});

group("the global scope shares one mode across applications", () => {
  const scope: ImeModeScopePolicy = new ImeModeScopePolicy();
  scope.activate("com.example.chat", "global");
  scope.remember(true);
  scope.activate("com.example.mail", "global");
  check(scope.mode(false) === true, "the mode follows the user rather than the application");
  scope.activate("com.example.mail", "app");
  check(
    scope.mode(false) === false,
    "switching the setting does not turn the shared mode into a per-application one",
  );
});

group("the remembered applications are bounded", () => {
  const scope: ImeModeScopePolicy = new ImeModeScopePolicy();
  for (let index: number = 0; index < 80; index++) {
    scope.activate(`com.example.app${index}`, "app");
    scope.remember(index % 2 === 0);
  }
  check(scope.size() === 64, "the map does not grow without limit");
  scope.activate("com.example.app79", "app");
  check(scope.mode(false) === false, "the most recent application is still remembered");
  scope.activate("com.example.app0", "app");
  check(
    scope.mode(false) === false,
    "the one that has gone longest without being typed in is the one dropped",
  );
  const long: ImeModeScopePolicy = new ImeModeScopePolicy();
  long.activate("x".repeat(300), "app");
  long.remember(true);
  check(long.size() === 0, "an implausible bundle name is not an identity to remember");
});

group("every toolbar button is optional, the settings gear included", () => {
  const all: ToolbarComponents = FloatingToolbarLayout.allComponents();
  check(
    FloatingToolbarLayout.buttons(all).includes(ToolbarButton.SETTINGS),
    "the gear is there when the shared switch is on",
  );
  const hidden: ToolbarComponents = { ...all, settings: false };
  check(
    !FloatingToolbarLayout.buttons(hidden).includes(ToolbarButton.SETTINGS),
    "and gone when it is off, which the switch previously could not achieve",
  );
  check(
    FloatingToolbarLayout.buttons(hidden).length === FloatingToolbarLayout.buttons(all).length - 1,
    "hiding it narrows the bar rather than leaving a gap",
  );
  const none: ToolbarComponents = {
    englishMode: false,
    punctuation: false,
    fullwidth: false,
    characterSet: false,
    emoji: false,
    handwriting: false,
    screenKeyboard: false,
    voice: false,
    settings: false,
  };
  check(FloatingToolbarLayout.buttons(none).length === 0, "turning everything off leaves nothing");
});

group("the 2in1 toolbar carries the pad and the microphone, as the macOS one does", () => {
  const all: ToolbarComponents = FloatingToolbarLayout.allComponents();
  const order: ToolbarButton[] = FloatingToolbarLayout.buttons(all);
  check(order.length === 9, "all nine buttons are drawn when every switch is on");
  check(
    order.indexOf(ToolbarButton.EMOJI) < order.indexOf(ToolbarButton.HANDWRITING) &&
      order.indexOf(ToolbarButton.HANDWRITING) < order.indexOf(ToolbarButton.SCREEN_KEYBOARD) &&
      order.indexOf(ToolbarButton.SCREEN_KEYBOARD) < order.indexOf(ToolbarButton.VOICE) &&
      order.indexOf(ToolbarButton.VOICE) < order.indexOf(ToolbarButton.SETTINGS),
    "emoji, pad, keyboard, microphone, then the gear last",
  );
  check(
    !FloatingToolbarLayout.buttons({ ...all, handwriting: false }).includes(
      ToolbarButton.HANDWRITING,
    ),
    "the shared handwriting switch hides the pad button",
  );
  check(
    !FloatingToolbarLayout.buttons({ ...all, voice: false }).includes(ToolbarButton.VOICE),
    "the shared voice switch hides the microphone button",
  );
  check(
    FloatingToolbarLayout.widthVp(order.length) > FloatingToolbarLayout.widthVp(order.length - 2),
    "the bar grows to hold both rather than clipping at the old seven",
  );
  const idle = FloatingToolbarLayout.idleState();
  check(
    FloatingToolbarLayout.face(ToolbarButton.HANDWRITING, idle) !==
      FloatingToolbarLayout.face(ToolbarButton.SETTINGS, idle) &&
      FloatingToolbarLayout.face(ToolbarButton.VOICE, idle) !==
        FloatingToolbarLayout.face(ToolbarButton.SETTINGS, idle),
    "neither new button falls through to the gear's face",
  );
  check(
    (ToolbarButton.SCREEN_KEYBOARD as number) === (PanelSurfaceAction.SCREEN_KEYBOARD as number),
    "adding them did not renumber the button the panel chord mirrors",
  );
});

group("toolbar surfaces route candidates and end their own work on the 2in1", () => {
  check(
    SurfaceRoutingPolicy.showsHandwritingCandidates(true, DesktopSurface.HANDWRITING, false),
    "the toolbar pad shows its results whatever the scheme",
  );
  check(
    SurfaceRoutingPolicy.showsHandwritingCandidates(true, DesktopSurface.SCREEN_KEYBOARD, true),
    "a handwriting scheme's screen keyboard still shows them",
  );
  check(
    !SurfaceRoutingPolicy.showsHandwritingCandidates(true, DesktopSurface.NONE, true),
    "the bare candidate window belongs to the physical keys",
  );
  check(
    !SurfaceRoutingPolicy.showsHandwritingCandidates(true, DesktopSurface.SCREEN_KEYBOARD, false),
    "a letter screen keyboard does not show ink results",
  );
  check(
    SurfaceRoutingPolicy.showsHandwritingCandidates(false, DesktopSurface.NONE, true) &&
      !SurfaceRoutingPolicy.showsHandwritingCandidates(false, DesktopSurface.HANDWRITING, false),
    "a phone follows its scheme alone",
  );
  check(
    SurfaceRoutingPolicy.endsVoice(DesktopSurface.VOICE, DesktopSurface.NONE) &&
      SurfaceRoutingPolicy.endsVoice(DesktopSurface.VOICE, DesktopSurface.HANDWRITING),
    "leaving the voice face by any route ends the recording",
  );
  check(
    !SurfaceRoutingPolicy.endsVoice(DesktopSurface.VOICE, DesktopSurface.VOICE) &&
      !SurfaceRoutingPolicy.endsVoice(DesktopSurface.NONE, DesktopSurface.VOICE),
    "opening or re-asserting it does not",
  );
  check(
    SurfaceRoutingPolicy.endsHandwriting(DesktopSurface.HANDWRITING, DesktopSurface.NONE) &&
      !SurfaceRoutingPolicy.endsHandwriting(DesktopSurface.EMOJI, DesktopSurface.NONE),
    "closing the pad drops its ink, closing something else does not",
  );
});

group("the other Windows maintenance chord clears the engine cache", () => {
  const chord: HardwareKey = {
    keyCode: 2019,
    unicodeChar: 0,
    ctrlKey: true,
    altKey: true,
    logoKey: false,
    shiftKey: true,
  };
  check(
    HardwareKeyRouter.route(chord, true, true).action === HardwareKeyAction.RESET_CACHE,
    "Ctrl+Shift+Alt+C clears the cache while composing",
  );
  check(
    HardwareKeyRouter.route(chord, false, true).action === HardwareKeyAction.RESET_CACHE,
    "and with nothing composed, which is when a stale list is most likely to be on screen",
  );
  const noAlt: HardwareKey = { ...chord, altKey: false };
  check(
    HardwareKeyRouter.route(noAlt, true, true).action !== HardwareKeyAction.RESET_CACHE,
    "Ctrl+Shift+C is not the chord",
  );
  const logo: HardwareKey = { ...chord, logoKey: true };
  check(
    HardwareKeyRouter.route(logo, true, true).action === HardwareKeyAction.RELEASE,
    "adding Super makes it a desktop shortcut",
  );
});

group("the Windows maintenance chord reaches candidate removal on a hardware keyboard", () => {
  const chord = (keyCode: number): HardwareKey => ({
    keyCode: keyCode,
    unicodeChar: 0,
    ctrlKey: true,
    altKey: true,
    logoKey: false,
    shiftKey: true,
  });
  const first = HardwareKeyRouter.route(chord(2001), true, true);
  check(
    first.action === HardwareKeyAction.REMOVE_CANDIDATE && first.index === 0,
    "Ctrl+Shift+Alt+1 names the first slot",
  );
  const eighth = HardwareKeyRouter.route(chord(2008), true, true);
  check(
    eighth.action === HardwareKeyAction.REMOVE_CANDIDATE && eighth.index === 7,
    "and Ctrl+Shift+Alt+8 the eighth, matching the Windows baseline",
  );
  check(
    HardwareKeyRouter.route(chord(2009), true, true).action === HardwareKeyAction.RELEASE,
    "a ninth slot is not part of the chord and stays the application\u0027s",
  );
  check(
    HardwareKeyRouter.route(chord(2001), false, true).action === HardwareKeyAction.RELEASE,
    "with nothing being spelled there is no candidate to delete",
  );
  const noAlt: HardwareKey = { ...chord(2001), altKey: false };
  check(
    HardwareKeyRouter.route(noAlt, true, true).action !== HardwareKeyAction.REMOVE_CANDIDATE,
    "Ctrl+Shift+1 is not the chord",
  );
  const logo: HardwareKey = { ...chord(2001), logoKey: true };
  check(
    HardwareKeyRouter.route(logo, true, true).action === HardwareKeyAction.RELEASE,
    "adding Super makes it a desktop shortcut, which belongs to the desktop",
  );
});

group("the word being completed is read backwards from the caret", () => {
  check(
    EnglishSuggestionPolicy.currentWord("type hel") === "hel",
    "letters run back to a boundary",
  );
  check(
    EnglishSuggestionPolicy.currentWord("hel") === "hel",
    "with nothing before it the word is all of it",
  );
  check(EnglishSuggestionPolicy.currentWord("hello ") === "", "a space ends the word");
  check(EnglishSuggestionPolicy.currentWord("say,hel") === "hel", "so does punctuation");
  check(
    EnglishSuggestionPolicy.currentWord("\uff48\uff45\uff4c") === "hel",
    "full-width letters normalise for the ASCII dictionary, because that is still English being typed",
  );
  check(EnglishSuggestionPolicy.currentWord("\u4f60\u597dhel") === "hel", "Han is a boundary");
  check(EnglishSuggestionPolicy.currentWord("") === "", "an empty editor has no word");
  check(EnglishSuggestionPolicy.currentWord(null) === "", "nor has an unreadable one");
  check(
    EnglishSuggestionPolicy.CONTEXT_CHARACTERS === 129,
    "the editor read includes one character beyond the longest accepted word",
  );
  check(
    EnglishSuggestionPolicy.currentWord("a".repeat(128)) === "a".repeat(128),
    "the longest accepted word remains intact",
  );
  check(
    EnglishSuggestionPolicy.currentWord("a".repeat(129)) === "",
    "one letter beyond the limit is refused rather than querying its trailing suffix",
  );
});

group("a prefix is only queried when it could mean something", () => {
  check(EnglishSuggestionPolicy.suggestible("hel") === true, "three letters is a word being typed");
  check(EnglishSuggestionPolicy.suggestible("he") === true, "two is the threshold");
  check(
    EnglishSuggestionPolicy.suggestible("h") === false,
    "one letter matches most of the dictionary and would cost a query per keystroke",
  );
  check(EnglishSuggestionPolicy.suggestible("") === false, "and none matches nothing");
  check(EnglishSuggestionPolicy.suggestible("he1") === false, "a digit is not part of a word");
});

group("accepting a completion replaces exactly what was typed", () => {
  const plain: EnglishReplacement | null = EnglishSuggestionPolicy.replacement(
    "hel",
    "hello",
    false,
  );
  check(
    plain !== null && plain.deleteCount === 3 && plain.insert === "hello",
    "the typed letters go and the whole word arrives",
  );
  const capital: EnglishReplacement | null = EnglishSuggestionPolicy.replacement(
    "Hel",
    "hello",
    true,
  );
  check(
    capital !== null && capital.insert === "Hello",
    "a capitalised prefix asks for a capitalised word",
  );
  check(
    EnglishSuggestionPolicy.replacement("hello", "hello", false) === null,
    "a word already typed in full is not a completion, and re-typing it would only move the caret",
  );
  check(EnglishSuggestionPolicy.replacement("hel", "", false) === null, "an empty word is not one");
  check(
    EnglishSuggestionPolicy.startedCapitalized("Hel") === true,
    "capitalisation is read off the first letter",
  );
  check(
    EnglishSuggestionPolicy.startedCapitalized("hel") === false,
    "and lower case asks for none",
  );
});

group("a completion reply is bounded before it reaches the strip", () => {
  const good: EnglishCompletions | null = EnglishSuggestionPolicy.decode(
    '{"ok":true,"value":{"prefix":"hel","items":["hello","help"]}}',
  );
  check(
    good !== null && good.prefix === "hel" && good.items.length === 2,
    "a well-formed reply is read",
  );
  check(
    EnglishSuggestionPolicy.decode('{"ok":false,"error":"no"}') === null,
    "a refusal offers nothing",
  );
  check(
    EnglishSuggestionPolicy.decode("not json") === null,
    "nor does something that is not a reply",
  );
  check(EnglishSuggestionPolicy.decode(null) === null, "nor an absent one");
  check(
    EnglishSuggestionPolicy.decode('{"ok":true,"value":{"prefix":"hel","items":["hello",7]}}') ===
      null,
    "an item that is not a word rejects the whole reply rather than being skipped past",
  );
  const many: string[] = [];
  for (let index: number = 0; index < 50; index++) many.push(`word${index}`);
  const bounded: EnglishCompletions | null = EnglishSuggestionPolicy.decode(
    JSON.stringify({ ok: true, value: { prefix: "hel", items: many } }),
  );
  check(
    bounded !== null && bounded.items.length === 32,
    "more items than the strip can hold are cut off",
  );
  check(
    EnglishSuggestionPolicy.decode(
      JSON.stringify({ ok: true, value: { prefix: "hel", items: ["x".repeat(200)] } }),
    ) === null,
    "an implausibly long word is refused",
  );
});

group("the settings page and the keyboard agree on what the loudest haptic is called", () => {
  // The shared DTO says 'strong' and the keyboard's own enum says 'heavy'. Assigned directly, a
  // save from the page would store a value KeyboardFeedback.parse falls back from, so the setting
  // would appear to save and the keys would go on feeling the same.
  const heavy: MobileKeyboardFeedback = KeyboardFeedbackBridge.toShared({
    sound: true,
    haptics: true,
    strength: HapticStrength.HEAVY,
  });
  check(heavy.hapticStrength === "strong", "the keyboard\u0027s heavy reaches the page as strong");
  check(
    heavy.soundEnabled === true && heavy.hapticsEnabled === true,
    "the two switches travel as they are",
  );
  check(
    KeyboardFeedbackBridge.fromShared({
      soundEnabled: false,
      hapticsEnabled: true,
      hapticStrength: "strong",
    }).strength === HapticStrength.HEAVY,
    "and the page\u0027s strong comes back as heavy",
  );
  check(
    KeyboardFeedbackBridge.fromShared({
      soundEnabled: false,
      hapticsEnabled: true,
      hapticStrength: "heavy",
    }).strength === HapticStrength.HEAVY,
    "a document already carrying the keyboard spelling still reads",
  );
  check(
    KeyboardFeedbackBridge.toShared({
      sound: false,
      haptics: false,
      strength: HapticStrength.LIGHT,
    }).hapticStrength === "light",
    "the other two names are the same on both sides",
  );
});

group("an unfamiliar feedback value falls back by field rather than wholesale", () => {
  const partial = KeyboardFeedbackBridge.fromShared({
    soundEnabled: true,
    hapticsEnabled: true,
    hapticStrength: "thunderous",
  });
  check(
    partial.strength === HapticStrength.MEDIUM,
    "a strength nobody knows becomes the default one",
  );
  check(
    partial.sound === true && partial.haptics === true,
    "and the two switches beside it are kept, which discarding the record whole would have lost",
  );
  const missing = KeyboardFeedbackBridge.fromShared(null);
  check(missing.sound === false && missing.haptics === false, "no record at all is the defaults");
  check(
    KeyboardFeedbackBridge.previewDuration("strong") >
      KeyboardFeedbackBridge.previewDuration("light"),
    "the preview buzzes longer for the stronger setting",
  );
  check(
    KeyboardFeedbackBridge.previewDuration("thunderous") ===
      KeyboardFeedbackBridge.previewDuration("medium"),
    "and an unknown one previews the default rather than nothing",
  );
});

group("an oversized feedback document is refused before parsing", () => {
  const oversized = JSON.stringify({
    sound: true,
    haptics: true,
    strength: HapticStrength.HEAVY,
    padding: "x".repeat(4096),
  });
  const parsed = KeyboardFeedback.parse(oversized);
  check(
    parsed.sound === false && parsed.haptics === false && parsed.strength === HapticStrength.MEDIUM,
    "feedback documents over the storage bound fall back to defaults",
  );
});

const KEY_SHIFT: number = 2047;
const KEY_CTRL: number = 2072;
const KEY_SPACE: number = 2050;
const KEY_E: number = 2021;
const KEY_F: number = 2022;
const KEY_PERIOD: number = 2044;
const KEY_A: number = 2017;

function modeKey(
  keyCode: number,
  down: boolean,
  timestamp: number,
  modifiers: Partial<ModeKey> = {},
): ModeKey {
  return {
    keyCode: keyCode,
    down: down,
    timestamp: timestamp,
    shiftKey: modifiers.shiftKey === true,
    ctrlKey: modifiers.ctrlKey === true,
    altKey: modifiers.altKey === true,
    logoKey: modifiers.logoKey === true,
  };
}

group("the Windows mode chords are answered on a hardware keyboard", () => {
  const routing: InputModeRouting = new InputModeRouting();
  routing.use(DEFAULT_MODE_BINDINGS);
  check(
    routing.accept(modeKey(KEY_E, true, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.ENGLISH_CANDIDATES,
    "Ctrl+Shift+E switches the English candidate mode, as Windows IsEnglishModeToggleKey does",
  );
  check(
    routing.accept(modeKey(KEY_SPACE, true, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.TOGGLE_WIDTH,
    "Ctrl+Shift+Space switches halfwidth and fullwidth",
  );
  check(
    routing.accept(modeKey(KEY_PERIOD, true, 0, { ctrlKey: true })) ===
      ModeGesture.TOGGLE_PUNCTUATION,
    "Ctrl+. switches the punctuation set",
  );
  check(
    routing.accept(modeKey(KEY_PERIOD, true, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.NONE,
    "Ctrl+Shift+. is a different chord and is not one of ours",
  );
  check(
    routing.accept(modeKey(KEY_E, true, 0, { ctrlKey: true, shiftKey: true, logoKey: true })) ===
      ModeGesture.NONE,
    "adding Super makes it a desktop shortcut",
  );
  check(
    routing.accept(modeKey(KEY_E, false, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.NONE,
    "the release does nothing a second time",
  );
});

group("the Windows chords are fixed rather than following the four optional bindings", () => {
  const off: ModeBindings = {
    switchLanguageShift: false,
    switchLanguageCtrl: false,
    switchLanguageCtrlAltSpace: false,
    toggleCharacterSetCtrlShiftF: false,
  };
  const routing: InputModeRouting = new InputModeRouting();
  routing.use(off);
  // Turning off the Shift tap says nothing about Ctrl+Shift+E, and Windows binds these fixed.
  check(
    routing.accept(modeKey(KEY_E, true, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.ENGLISH_CANDIDATES,
    "Ctrl+Shift+E still answers",
  );
  check(
    routing.accept(modeKey(KEY_PERIOD, true, 0, { ctrlKey: true })) ===
      ModeGesture.TOGGLE_PUNCTUATION,
    "and so does Ctrl+.",
  );
  check(
    routing.accept(modeKey(KEY_F, true, 0, { ctrlKey: true, shiftKey: true })) === ModeGesture.NONE,
    "while the optional Ctrl+Shift+F obeys the document",
  );
  check(
    routing.accept(modeKey(KEY_SPACE, true, 0, { ctrlKey: true, altKey: true })) ===
      ModeGesture.NONE,
    "as does the optional Ctrl+Alt+Space",
  );
});

group("a solitary modifier is a tap only when nothing happened in between", () => {
  const routing: InputModeRouting = new InputModeRouting();
  routing.use(DEFAULT_MODE_BINDINGS);
  routing.accept(modeKey(KEY_SHIFT, true, 0));
  check(
    routing.accept(modeKey(KEY_SHIFT, false, 100)) === ModeGesture.SWITCH_LANGUAGE,
    "press and release with nothing between them is a tap",
  );
  routing.accept(modeKey(KEY_SHIFT, true, 0));
  check(
    routing.accept(modeKey(KEY_SHIFT, false, 900)) === ModeGesture.NONE,
    "held too long it was doing something else, such as holding a capital",
  );
  routing.accept(modeKey(KEY_SHIFT, true, 0));
  routing.accept(modeKey(KEY_A, true, 10, { shiftKey: true }));
  check(
    routing.accept(modeKey(KEY_SHIFT, false, 20)) === ModeGesture.NONE,
    "a key in between means the modifier was modifying it",
  );
  routing.accept(modeKey(KEY_SHIFT, true, 0));
  routing.accept(modeKey(KEY_CTRL, true, 5));
  check(
    routing.accept(modeKey(KEY_SHIFT, false, 10)) === ModeGesture.NONE,
    "and a competing modifier means it is a chord",
  );
  const ctrlOff: InputModeRouting = new InputModeRouting();
  ctrlOff.use(DEFAULT_MODE_BINDINGS);
  ctrlOff.accept(modeKey(KEY_CTRL, true, 0));
  check(
    ctrlOff.accept(modeKey(KEY_CTRL, false, 50)) === ModeGesture.NONE,
    "the Ctrl tap is off by default and stays off",
  );
});

const KEY_ALT_RIGHT: number = 2046;
const KEY_ESCAPE: number = 2070;
const KEY_CTRL_RIGHT: number = 2073;
const KEY_META_LEFT: number = 2076;
const KEY_F9: number = 2098;
const KEY_VOICE_SPACE: number = 2050;

function voiceKey(keyCode: number, down: boolean, modifiers: Partial<VoiceKey> = {}): VoiceKey {
  return {
    keyCode: keyCode,
    down: down,
    ctrlKey: modifiers.ctrlKey === true,
    altKey: modifiers.altKey === true,
    shiftKey: modifiers.shiftKey === true,
    logoKey: modifiers.logoKey === true,
  };
}

group("a held voice key records until it is released", () => {
  const voice: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  voice.use(DEFAULT_VOICE_HOTKEY_BINDINGS);
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, true), false) === VoiceHotkeyAction.START,
    "right Alt begins recording",
  );
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, true), true) === VoiceHotkeyAction.NONE,
    "the keyboard repeating the held key is not a second request",
  );
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, false), true) === VoiceHotkeyAction.STOP,
    "and releasing it recognizes what was said",
  );
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, false), false) === VoiceHotkeyAction.NONE,
    "a release with nothing held is nobody\u0027s",
  );
});

group("Space locks a recording so the hold key can be let go", () => {
  const voice: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  voice.use(DEFAULT_VOICE_HOTKEY_BINDINGS);
  voice.accept(voiceKey(KEY_ALT_RIGHT, true), false);
  check(
    voice.accept(voiceKey(KEY_VOICE_SPACE, true), true) === VoiceHotkeyAction.LOCK,
    "Space while holding locks it",
  );
  check(voice.isLocked() === true, "and the panel can say so");
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, false), true) === VoiceHotkeyAction.NONE,
    "releasing the hold key no longer ends it",
  );
  check(
    voice.accept(voiceKey(KEY_F9, true, { ctrlKey: true }), true) === VoiceHotkeyAction.STOP,
    "Ctrl+F9 ends a locked recording, which is the whole reason it is a toggle",
  );
  check(voice.isLocked() === false, "and the lock is gone with it");
});

group("Space and Escape are only voice keys while recording", () => {
  const voice: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  voice.use(DEFAULT_VOICE_HOTKEY_BINDINGS);
  check(
    voice.accept(voiceKey(KEY_VOICE_SPACE, true), false) === VoiceHotkeyAction.NONE,
    "Space with nothing recording belongs to the composition",
  );
  check(
    voice.accept(voiceKey(KEY_ESCAPE, true), false) === VoiceHotkeyAction.NONE,
    "and so does Escape",
  );
  voice.accept(voiceKey(KEY_ALT_RIGHT, true), false);
  check(
    voice.accept(voiceKey(KEY_ESCAPE, true), true) === VoiceHotkeyAction.CANCEL,
    "while recording, Escape throws it away",
  );
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, false), true) === VoiceHotkeyAction.NONE,
    "and the hold it cancelled is no longer held",
  );
});

group("each voice shortcut obeys its own switch", () => {
  const off: VoiceHotkeyBindings = {
    ralt: false,
    ctrlWin: false,
    rctrlRalt: false,
    holdSpaceLock: false,
    ctrlF9: false,
  };
  const voice: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  voice.use(off);
  check(
    voice.accept(voiceKey(KEY_ALT_RIGHT, true), false) === VoiceHotkeyAction.NONE,
    "right Alt turned off does nothing",
  );
  check(
    voice.accept(voiceKey(KEY_F9, true, { ctrlKey: true }), false) === VoiceHotkeyAction.NONE,
    "and neither does Ctrl+F9",
  );
  const chords: VoiceHotkeyBindings = {
    ralt: false,
    ctrlWin: true,
    rctrlRalt: true,
    holdSpaceLock: true,
    ctrlF9: true,
  };
  const chordVoice: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  chordVoice.use(chords);
  check(
    chordVoice.accept(voiceKey(KEY_ALT_RIGHT, true, { ctrlKey: true }), false) ===
      VoiceHotkeyAction.START,
    "Ctrl held with right Alt is the RCtrl+RAlt chord",
  );
  chordVoice.accept(voiceKey(KEY_ALT_RIGHT, false), true);
  check(
    chordVoice.accept(voiceKey(KEY_META_LEFT, true, { ctrlKey: true }), false) ===
      VoiceHotkeyAction.START,
    "Ctrl+Win is the other chord",
  );
  const bare: VoiceHotkeyPolicy = new VoiceHotkeyPolicy();
  bare.use(chords);
  check(
    bare.accept(voiceKey(KEY_ALT_RIGHT, true), false) === VoiceHotkeyAction.NONE,
    "with only the chords on, a bare right Alt is not one of them",
  );
  check(
    bare.accept(voiceKey(KEY_CTRL_RIGHT, true), false) === VoiceHotkeyAction.NONE,
    "and a right Control alone is half a chord, not a binding",
  );
});

group("the recording tones follow their own switches under one master", () => {
  const base: VoiceInputConfiguration = DEFAULT_VOICE_INPUT_CONFIGURATION;
  check(
    VoiceRecordingBehaviourPolicy.playsStartTone(base) === true,
    "both tones are on by default",
  );
  check(VoiceRecordingBehaviourPolicy.playsEndTone(base) === true, "including the closing one");
  const noStart: VoiceInputConfiguration = { ...base, start_sound: false };
  check(VoiceRecordingBehaviourPolicy.playsStartTone(noStart) === false, "one can be turned off");
  check(VoiceRecordingBehaviourPolicy.playsEndTone(noStart) === true, "without taking the other");
  const silent: VoiceInputConfiguration = { ...base, sound_enabled: false };
  check(
    VoiceRecordingBehaviourPolicy.playsStartTone(silent) === false,
    "the master silences both regardless of what they say",
  );
  check(VoiceRecordingBehaviourPolicy.playsEndTone(silent) === false, "both of them");
  const legacy: VoiceInputConfiguration = { ...base };
  legacy.sound_enabled = undefined;
  legacy.start_sound = undefined;
  check(
    VoiceRecordingBehaviourPolicy.playsStartTone(legacy) === true,
    "an older document without the fields takes the shared default rather than falling silent",
  );
});

group("partial recognizer output reaches the panel only when asked for", () => {
  const base: VoiceInputConfiguration = DEFAULT_VOICE_INPUT_CONFIGURATION;
  check(
    VoiceRecordingBehaviourPolicy.showsInterimResults(base) === false,
    "a stream of guesses rewriting itself is not the default",
  );
  check(
    VoiceRecordingBehaviourPolicy.showsInterimResults({ ...base, stream_inline_preedit: true }) ===
      true,
    "and appears when the user asks to watch",
  );
  const legacy: VoiceInputConfiguration = { ...base };
  legacy.stream_inline_preedit = undefined;
  check(
    VoiceRecordingBehaviourPolicy.showsInterimResults(legacy) === false,
    "an older document without the field stays quiet",
  );
});

group("quietening other applications is off unless asked for", () => {
  const base: VoiceInputConfiguration = DEFAULT_VOICE_INPUT_CONFIGURATION;
  check(
    VoiceRecordingBehaviourPolicy.quietensOthers(base) === false,
    "taking the audio session from whatever is playing is intrusive and is not the default",
  );
  check(
    VoiceRecordingBehaviourPolicy.quietensOthers({ ...base, mute_system_audio: true }) === true,
    "and happens when the user asks",
  );
  const legacy: VoiceInputConfiguration = { ...base };
  legacy.mute_system_audio = undefined;
  check(
    VoiceRecordingBehaviourPolicy.quietensOthers(legacy) === false,
    "an absent field is not consent",
  );
});

group("a staged resource copy is trusted only while it matches the package", () => {
  const set: StagedArtifact[] = [
    { name: "msime.db", size: 107552768 },
    { name: "dict_pinyin.dat", size: 1068442 },
  ];
  const token: string = StagedResourcePolicy.generationToken(set);
  check(token.length > 0, "a package can be described");
  check(
    StagedResourcePolicy.needsStaging(token, token) === false,
    "the same package is not copied out again",
  );
  // The defect this replaces: a marker saying only "staged" went on saying so after the package
  // changed, and the shared verification then refused the directory outright.
  const upgraded: StagedArtifact[] = [
    { name: "msime.db", size: 107552769 },
    { name: "dict_pinyin.dat", size: 1068442 },
  ];
  check(
    StagedResourcePolicy.needsStaging(token, StagedResourcePolicy.generationToken(upgraded)) ===
      true,
    "an artifact that changed size is a different generation",
  );
  const dropped: StagedArtifact[] = [{ name: "msime.db", size: 107552768 }];
  check(
    StagedResourcePolicy.needsStaging(token, StagedResourcePolicy.generationToken(dropped)) ===
      true,
    "so is a package with one fewer artifact",
  );
  const added: StagedArtifact[] = [...set, { name: "wubi.db", size: 4096 }];
  check(
    StagedResourcePolicy.needsStaging(token, StagedResourcePolicy.generationToken(added)) === true,
    "and one with an extra, which is the case that fails verification",
  );
  check(StagedResourcePolicy.needsStaging(null, token) === true, "no marker means never staged");
});

group("an undescribable package is staged rather than skipped", () => {
  check(StagedResourcePolicy.generationToken([]) === "", "an empty package describes nothing");
  check(
    StagedResourcePolicy.needsStaging("anything", "") === true,
    "copying twice costs a moment; skipping when it was needed costs a keyboard that will not start",
  );
  check(
    StagedResourcePolicy.generationToken([{ name: "a:b", size: 1 }]) === "",
    "a name carrying the separator would make two packages look alike",
  );
  check(
    StagedResourcePolicy.generationToken([{ name: "a", size: -1 }]) === "",
    "and a nonsense size describes nothing either",
  );
  const reordered: string = StagedResourcePolicy.generationToken([
    { name: "b", size: 2 },
    { name: "a", size: 1 },
  ]);
  const ordered: string = StagedResourcePolicy.generationToken([
    { name: "a", size: 1 },
    { name: "b", size: 2 },
  ]);
  check(
    reordered === ordered,
    "the filesystem ordering is not guaranteed and must not change the token",
  );
});

group("an AsyncCallback reports failure only when it actually failed", () => {
  // OHOS hands every AsyncCallback a BusinessError, on success too, carrying code 0. Testing the
  // object itself is always true, which is how the settings page logged "would not load" on every
  // successful load and handwriting reported a canvas failure before it looked at the snapshot.
  check(BusinessErrorPolicy.failed({ code: 0 }) === false, "code 0 is the success every host sees");
  check(
    BusinessErrorPolicy.failed({ code: 0, message: "" }) === false,
    "with an empty message beside it, which is what made this look like a real error",
  );
  check(
    BusinessErrorPolicy.failed({ code: 401, message: "denied" }) === true,
    "a non-zero code is a failure",
  );
  check(BusinessErrorPolicy.failed(null) === false, "no error is no failure");
  check(BusinessErrorPolicy.failed(undefined) === false, "nor is an absent one");
  check(
    BusinessErrorPolicy.failed(new Error("thrown")) === true,
    "a plain Error has no code and is still a failure",
  );
});

group("an AsyncCallback failure is described without inventing a code", () => {
  check(
    BusinessErrorPolicy.describe({ code: 401, message: "denied" }) === "code 401: denied",
    "the code and the message travel together",
  );
  check(
    BusinessErrorPolicy.describe({ code: 401 }) === "code 401",
    "a code with no message says the code",
  );
  check(
    BusinessErrorPolicy.describe(new Error("thrown")) === "thrown",
    "and something with no code says what it does have rather than claiming one",
  );
  check(BusinessErrorPolicy.describe(null) === "unknown error", "nothing at all says so plainly");
});

function recordingTarget(log: string[]): HardwareKeyTarget {
  return {
    press: (character: number, shifted: boolean) => {
      log.push(`press ${character} ${shifted}`);
      return true;
    },
    punctuation: (character: number) => log.push(`punctuation ${character}`),
    backspace: () => log.push("backspace"),
    cancel: () => log.push("cancel"),
    moveLeft: () => log.push("moveLeft"),
    moveRight: () => log.push("moveRight"),
    moveHome: () => log.push("moveHome"),
    moveEnd: () => log.push("moveEnd"),
    deleteForward: () => log.push("deleteForward"),
    backspaceSegment: () => log.push("backspaceSegment"),
    moveLeftSegment: () => log.push("moveLeftSegment"),
    moveRightSegment: () => log.push("moveRightSegment"),
    commitHighlighted: () => log.push("commitHighlighted"),
    commitRaw: () => log.push("commitRaw"),
    commitTranslation: () => log.push("commitTranslation"),
    choose: (index: number) => log.push(`choose ${index}`),
    resetCache: () => log.push("resetCache"),
    removeManagedCandidate: (index: number) => {
      log.push(`removeManagedCandidate ${index}`);
      return true;
    },
    selectEdge: (edge: number) => log.push(`selectEdge ${edge}`),
    nextPage: () => log.push("nextPage"),
    previousPage: () => log.push("previousPage"),
    nextCandidate: () => log.push("nextCandidate"),
    previousCandidate: () => log.push("previousCandidate"),
    convertJapanese: () => {
      log.push("convertJapanese");
      return true;
    },
    commitJapanese: () => {
      log.push("commitJapanese");
      return true;
    },
    widen: (character: number) => {
      log.push(`widen ${character}`);
      return false;
    },
    commitThenType: (character: number) => log.push(`commitThenType ${character}`),
  };
}

function dispatched(
  action: HardwareKeyAction,
  character: number = 0,
  index: number = 0,
  shifted: boolean = false,
): string[] {
  const log: string[] = [];
  HardwareKeyDispatch.apply(
    { action: action, character: character, index: index },
    shifted,
    recordingTarget(log),
  );
  return log;
}

group("every routed hardware key reaches the method that means it", () => {
  // This mapping lived as a switch inside the extension ability where no test could reach it, while
  // both ends of it were covered. A left arrow wired to moveRight would have looked like working
  // code until someone typed on a 2in1.
  check(
    dispatched(HardwareKeyAction.COMPOSE, 0x61, 0, true)[0] === "press 97 true",
    "a letter composes, and carries whether shift was down for helpcode",
  );
  check(
    dispatched(HardwareKeyAction.PUNCTUATION, 0x2c)[0] === "punctuation 44",
    "a punctuation mark goes to the punctuation path with its character",
  );
  check(dispatched(HardwareKeyAction.BACKSPACE)[0] === "backspace", "backspace deletes a letter");
  check(dispatched(HardwareKeyAction.CANCEL)[0] === "cancel", "escape throws the composition away");
  check(dispatched(HardwareKeyAction.MOVE_LEFT)[0] === "moveLeft", "left goes left");
  check(dispatched(HardwareKeyAction.MOVE_RIGHT)[0] === "moveRight", "and right goes right");
  check(dispatched(HardwareKeyAction.MOVE_HOME)[0] === "moveHome", "home goes to the start");
  check(dispatched(HardwareKeyAction.MOVE_END)[0] === "moveEnd", "and end to the end");
  check(
    dispatched(HardwareKeyAction.DELETE_FORWARD)[0] === "deleteForward",
    "delete removes ahead",
  );
  check(
    dispatched(HardwareKeyAction.BACKSPACE_SEGMENT)[0] === "backspaceSegment",
    "the segment chords are their own methods, not the plain ones",
  );
  check(
    dispatched(HardwareKeyAction.MOVE_LEFT_SEGMENT)[0] === "moveLeftSegment",
    "left by segment",
  );
  check(
    dispatched(HardwareKeyAction.MOVE_RIGHT_SEGMENT)[0] === "moveRightSegment",
    "right by segment",
  );
  check(
    dispatched(HardwareKeyAction.COMMIT)[0] === "commitHighlighted",
    "space takes the highlight",
  );
  check(
    dispatched(HardwareKeyAction.COMMIT_RAW)[0] === "commitRaw",
    "enter takes the letters typed",
  );
  check(
    dispatched(HardwareKeyAction.COMMIT_TRANSLATION)[0] === "commitTranslation",
    "Ctrl+Enter takes the translation rather than the candidate",
  );
  check(dispatched(HardwareKeyAction.SELECT, 0, 4)[0] === "choose 4", "a digit picks its slot");
  check(dispatched(HardwareKeyAction.RESET_CACHE)[0] === "resetCache", "the cache chord clears it");
  check(
    dispatched(HardwareKeyAction.REMOVE_CANDIDATE, 0, 7)[0] === "removeManagedCandidate 7",
    "the slot chord removes that slot rather than choosing it",
  );
  check(dispatched(HardwareKeyAction.NEXT_PAGE)[0] === "nextPage", "paging pages");
  check(dispatched(HardwareKeyAction.PREVIOUS_PAGE)[0] === "previousPage", "both ways");
  check(
    dispatched(HardwareKeyAction.NEXT_CANDIDATE)[0] === "nextCandidate",
    "and moving the highlight is not paging",
  );
  check(dispatched(HardwareKeyAction.PREVIOUS_CANDIDATE)[0] === "previousCandidate", "both ways");
  check(
    dispatched(HardwareKeyAction.JAPANESE_CONVERT)[0] === "convertJapanese",
    "Japanese Space reaches conversion state",
  );
  check(
    dispatched(HardwareKeyAction.JAPANESE_COMMIT)[0] === "commitJapanese",
    "Japanese Return reaches conversion-aware commit",
  );
  check(
    dispatched(HardwareKeyAction.COMMIT_THEN_TYPE, 0x30)[0] === "commitThenType 48",
    "a key the composition cannot use finishes it and carries its character",
  );
});

group("word-character keys take the end of the candidate they name", () => {
  check(
    dispatched(HardwareKeyAction.WORD_CHARACTER_FIRST)[0] === "selectEdge 0",
    "the opening bracket takes the first character",
  );
  check(
    dispatched(HardwareKeyAction.WORD_CHARACTER_LAST)[0] === "selectEdge 1",
    "and the closing one the last, which swapping would make silently wrong",
  );
});

group("a key that was claimed without an effect does nothing at all", () => {
  check(
    dispatched(HardwareKeyAction.IGNORED).length === 0,
    "a disabled navigation binding is consumed rather than turned into text",
  );
  check(
    dispatched(HardwareKeyAction.RELEASE).length === 0,
    "and a released key never reaches here, but would still do nothing if it did",
  );
});

group("a batch transcription reply is judged before it is parsed", () => {
  const ok: VoiceOutcome = VoiceResponsePolicy.batchResult(200, JSON.stringify({ text: "你好" }));
  check(ok.text === "你好" && ok.failure === "", "a 2xx reply carrying text is the result");
  check(
    VoiceResponsePolicy.batchResult(500, JSON.stringify({ text: "你好" })).failure.length > 0,
    "a server error is a failure however well formed its body",
  );
  check(VoiceResponsePolicy.batchResult(200, null).failure.length > 0, "so is a missing body");
  check(
    VoiceResponsePolicy.batchResult(200, "not json").failure.length > 0,
    "and a body that is not a reply at all",
  );
  check(
    VoiceResponsePolicy.batchResult(200, JSON.stringify({ text: "" })).failure.length > 0,
    "an empty transcription says so rather than committing nothing",
  );
  // Judged on the raw body: a provider answering with a megabyte is not one to parse first.
  const huge: string = JSON.stringify({ text: "x".repeat(2 * 1024 * 1024) });
  check(
    VoiceResponsePolicy.batchResult(200, huge).failure.length > 0,
    "an implausibly large body is refused before it reaches a parser",
  );
});

group("a streaming frame is read at whichever level answered", () => {
  const top: VoiceOutcome = VoiceResponsePolicy.streamingFrame(
    JSON.stringify({ result: { text: "你好" } }),
    false,
  );
  check(top.text === "你好" && !top.last, "text at the top level is the result");
  const nested: VoiceOutcome = VoiceResponsePolicy.streamingFrame(
    JSON.stringify({ payload_msg: { result: { text: "你好" } } }),
    false,
  );
  check(nested.text === "你好", "and so is text nested under payload_msg");
  check(
    VoiceResponsePolicy.streamingFrame(JSON.stringify({ code: 1002 }), false).failure.length > 0,
    "a non-zero code at the top level is a refusal",
  );
  check(
    VoiceResponsePolicy.streamingFrame(JSON.stringify({ payload_msg: { code: 1002 } }), false)
      .failure.length > 0,
    "and so is one nested, which is the level that is easy to forget to check",
  );
  check(
    VoiceResponsePolicy.streamingFrame(JSON.stringify({ error: "denied" }), false).failure.length >
      0,
    "an error member is a refusal whatever the code says",
  );
  check(
    VoiceResponsePolicy.streamingFrame(JSON.stringify({ code: 0, result: { text: "hi" } }), false)
      .failure === "",
    "code 0 is success rather than a refusal",
  );
  check(
    VoiceResponsePolicy.streamingFrame("not json", false).failure.length > 0,
    "a frame that is not JSON is a refusal rather than an empty result",
  );
});

group("a final frame ends the recording even when it carries no text", () => {
  const last: VoiceOutcome = VoiceResponsePolicy.streamingFrame(JSON.stringify({}), true);
  check(
    last.last === true && last.text === "" && last.failure === "",
    "an empty last frame is still the last frame; treating it as nothing would keep listening",
  );
  const lastWithText: VoiceOutcome = VoiceResponsePolicy.streamingFrame(
    JSON.stringify({ result: { text: "你好" } }),
    true,
  );
  check(
    lastWithText.last === true && lastWithText.text === "你好",
    "and a last frame with text carries both",
  );
  check(
    VoiceResponsePolicy.streamingFrame(JSON.stringify({ code: 7 }), true).last === true,
    "a refusal on the last frame is still the last frame",
  );
});

group("the mode badge is built only when the shared preference allows it", () => {
  check(InputModeHudPolicy.enabled(true) === true, "an explicit yes shows the badge");
  check(InputModeHudPolicy.enabled(false) === false, "an explicit no hides it");
  check(
    InputModeHudPolicy.enabled(undefined) === true,
    "a document written before the field existed keeps the shared default-on behaviour",
  );
  check(
    InputModeHudPolicy.enabled("false") === true,
    "a malformed value is not read as a request to hide it",
  );
});

console.log("ShuangpinKeyHintPolicy");

group("hints appear only for a shuangpin composition", () => {
  check(ShuangpinKeyHintPolicy.visible(false, 1, "none") === true, "shuangpin shows hints");
  check(ShuangpinKeyHintPolicy.visible(false, 0, "none") === false, "quanpin has no key units");
  check(ShuangpinKeyHintPolicy.visible(true, 1, "none") === false, "dedicated English shows none");
  check(ShuangpinKeyHintPolicy.visible(false, 1, "emoji") === false, "a local mode owns the keys");
  check(
    ShuangpinKeyHintPolicy.hint("xiaohe", "q", true, 1, "none") === "",
    "an invisible context yields no hint",
  );
  check(ShuangpinKeyHintPolicy.hint("xiaohe", null, false, 1, "none") === "", "a null key is safe");
  check(
    ShuangpinKeyHintPolicy.hint("nonsense", "q", false, 1, "none") === "",
    "an unknown profile yields no hint rather than a wrong one",
  );
});

group("the key hint pairs the initial with the finals", () => {
  check(
    ShuangpinKeyHintPolicy.hint("xiaohe", "U", false, 1, "none") === "sh / u",
    "u carries both an initial and a final, separated",
  );
  check(
    ShuangpinKeyHintPolicy.hint("xiaohe", "u", false, 1, "none") === "sh / u",
    "the lookup is case-insensitive",
  );
  check(
    ShuangpinKeyHintPolicy.hint("xiaohe", "W", false, 1, "none") === "ei",
    "a key with only a final shows just the final",
  );
});

group("v is printed as u-umlaut, which is what the user is looking for", () => {
  const v = ShuangpinKeyHintPolicy.hint("xiaohe", "V", false, 1, "none");
  check(v.includes("ü"), `xiaohe v should print u-umlaut, got "${v}"`);
  check(!v.includes("v="), "the raw table spelling never reaches the label");
  const t = ShuangpinKeyHintPolicy.hint("xiaohe", "T", false, 1, "none");
  check(t.includes("ü"), `xiaohe t carries ue and ve, so it shows u-umlaut too, got "${t}"`);
});

group("finals sharing a key are listed in a stable order", () => {
  const s = ShuangpinKeyHintPolicy.hint("xiaohe", "S", false, 1, "none");
  check(s === "iong ong", `two finals sort rather than following table order, got "${s}"`);
  const l = ShuangpinKeyHintPolicy.hint("xiaohe", "L", false, 1, "none");
  check(l === "iang uang", `and so do these, got "${l}"`);
});

group("each profile has its own table", () => {
  const xiaohe = ShuangpinKeyHintPolicy.hint("xiaohe", "W", false, 1, "none");
  const ziranma = ShuangpinKeyHintPolicy.hint("ziranma", "W", false, 1, "none");
  check(xiaohe !== ziranma, "the same key means different things in different profiles");
  check(ziranma === "ia ua", `ziranma w carries ia and ua, got "${ziranma}"`);
  check(
    ShuangpinKeyHintPolicy.hint("microsoft", ";", false, 1, "none") === "ing",
    "microsoft is the profile that uses the semicolon key",
  );
  check(
    ShuangpinKeyHintPolicy.hint("xiaohe", ";", false, 1, "none") === "",
    "xiaohe leaves the semicolon unassigned",
  );
  check(
    ShuangpinKeyHintPolicy.hint("shoudao", "E", false, 1, "none") === "sh / e",
    "shoudao puts sh on e rather than on u",
  );
});

console.log("EditorPolicy");

function traits(
  text: boolean,
  password: boolean,
  uri: boolean,
  email: boolean,
  noSuggestions: boolean,
): EditorTraits {
  return { text: text, password: password, uri: uri, email: email, noSuggestions: noSuggestions };
}

const PLAIN: EditorTraits = traits(true, false, false, false, false);
const PASSWORD: EditorTraits = traits(true, true, false, false, false);
const URI: EditorTraits = traits(true, false, true, false, false);
const EMAIL: EditorTraits = traits(true, false, false, true, false);
const NO_SUGGESTIONS: EditorTraits = traits(true, false, false, false, true);
const NUMERIC: EditorTraits = traits(false, false, false, false, false);

group("a delayed editor callback never interrupts typing", () => {
  check(
    EditorPolicy.appliesDelayedLanguage(false, false, true),
    "an idle keyboard adopts the editor's Latin override",
  );
  check(
    !EditorPolicy.appliesDelayedLanguage(true, false, true),
    "a composition typed while attributes load is not reset",
  );
  check(
    !EditorPolicy.appliesDelayedLanguage(false, true, true),
    "an unchanged mode does not issue a redundant Engine reset",
  );
  check(
    !EditorPolicy.appliesDelayedLanguage(false, false, true, true),
    "a delayed Latin preference cannot undo a manual Chinese choice in this editor",
  );
  check(
    EditorPolicy.appliesDelayedLanguage(false, false, true, false),
    "the same field preference still applies in a fresh editor generation",
  );
});

group("editor change echoes preserve keyboard-owned composition", () => {
  check(
    !EditorPolicy.isExternalTextChange(1),
    "a pending keyboard edit consumes its own asynchronous text-change echo",
  );
  check(
    EditorPolicy.isExternalTextChange(0),
    "a text change with no pending keyboard mutation came from the host",
  );
});

group("a password field never sees a composition buffer", () => {
  check(EditorPolicy.useEngine(PLAIN) === true, "prose composes through the Engine");
  check(EditorPolicy.useEngine(PASSWORD) === false, "a password never does");
  check(
    EditorPolicy.useEngine(NO_SUGGESTIONS) === false,
    "a field that asked for no suggestions has said it does not want one",
  );
  check(EditorPolicy.useEngine(NUMERIC) === false, "a number field has nothing to compose");
  check(
    EditorPolicy.useEngine(URI) === true,
    "a URI still composes: it prefers Latin but is not forbidden the Engine",
  );
});

group("Latin is preferred where Chinese would only be in the way", () => {
  check(EditorPolicy.prefersLatin(URI) === true, "addresses are Latin");
  check(EditorPolicy.prefersLatin(EMAIL) === true, "so are email addresses");
  check(EditorPolicy.prefersLatin(PASSWORD) === true, "so are passwords");
  check(EditorPolicy.prefersLatin(NO_SUGGESTIONS) === true, "so are one-time codes");
  check(EditorPolicy.prefersLatin(PLAIN) === false, "prose is not");
});

group("an address field gets the full letter face instead of a nine-key grid", () => {
  // MSIME-Apple's testLatinFieldsUseFullKeyboardAndRestoreNineKeyHeight: with the nine-key scheme
  // selected, focusing an asciiCapable, email or URL field hides the grid and shows Q, and going
  // back to an ordinary field brings 全拼 9 键 back. A grid resolves a digit sequence against a
  // dictionary, and none of these fields holds dictionary words.
  check(EditorPolicy.prefersFullFace(URI) === true, "a URL is spelled out, not disambiguated");
  check(EditorPolicy.prefersFullFace(EMAIL) === true, "so is an email address");
  check(EditorPolicy.prefersFullFace(PASSWORD) === true, "so is a password");
  check(EditorPolicy.prefersFullFace(NO_SUGGESTIONS) === true, "so is a one-time code");
  check(EditorPolicy.prefersFullFace(PLAIN) === false, "prose keeps whichever face the user chose");
});

group("an address is never capitalized, whatever the platform says", () => {
  check(
    EditorPolicy.capitalizationMode(URI, CapitalizationMode.SENTENCES) === CapitalizationMode.NONE,
    "the first character of a URI is part of an address",
  );
  check(
    EditorPolicy.capitalizationMode(EMAIL, CapitalizationMode.WORDS) === CapitalizationMode.NONE,
    "and so is the first character of an email address",
  );
  check(
    EditorPolicy.capitalizationMode(NUMERIC, CapitalizationMode.SENTENCES) ===
      CapitalizationMode.NONE,
    "a non-text field has nothing to capitalize",
  );
  check(
    EditorPolicy.capitalizationMode(PLAIN, CapitalizationMode.SENTENCES) ===
      CapitalizationMode.SENTENCES,
    "otherwise the platform value is honoured",
  );
  check(
    EditorPolicy.capitalizationMode(PLAIN, CapitalizationMode.NONE) === CapitalizationMode.NONE,
    "including when it asks for none",
  );
  check(
    EditorPolicy.capitalizationMode(PASSWORD, CapitalizationMode.WORDS) ===
      CapitalizationMode.WORDS,
    "a password is not an address: the platform value stands, as in the Java",
  );
});

console.log("KeyboardSkin");

group("the theme resolves keyboard first, then global, then the system", () => {
  check(KeyboardSkin.resolveDark("dark", "light", false) === true, "the keyboard theme wins");
  check(KeyboardSkin.resolveDark("light", "dark", true) === false, "in both directions");
  check(KeyboardSkin.resolveDark("system", "dark", false) === true, "then the global theme");
  check(KeyboardSkin.resolveDark("system", "light", true) === false, "in both directions");
  check(KeyboardSkin.resolveDark("system", "system", true) === true, "then the system");
  check(KeyboardSkin.resolveDark("system", "system", false) === false, "in both directions");
  check(
    KeyboardSkin.resolveDark("dark", "light", false) === true,
    "voice dark overrides the global light theme",
  );
  check(
    KeyboardSkin.resolveDark("light", "dark", true) === false,
    "voice light overrides the global dark theme",
  );
  check(
    KeyboardSkin.resolveDark("follow", "dark", false) === true,
    "voice follow inherits the global theme",
  );
});

group("an unknown skin id falls back to forest rather than failing", () => {
  check(KeyboardSkin.from("nonsense", false).id === "forest", "an unknown id is forest");
  check(KeyboardSkin.from("", false).id === "forest", "so is an empty one");
  check(KeyboardSkin.from(null, false).id === "forest", "so is null");
  check(KeyboardSkin.builtIns(false).length === 8, "eight built-in skins");
  check(KeyboardSkin.choices(false, null).length === 9, "plus the custom one");
  check(KeyboardSkin.choices(false, null)[8].id === "custom", "custom comes last");
  const design = CustomKeyboardSkin.from({
    background: 0x102030,
    keyBackground: 0x203040,
    accent: 0x80c0ff,
    cornerRadius: 14,
    borderWidth: 1,
    keyOpacity: 0.8,
  });
  const choices = KeyboardSkin.choices(false, design);
  check(
    choices[8].id === "custom" && choices[8].background === "#102030",
    "the custom picker entry uses the shared design",
  );
  check(
    choices[8].cornerRadius === 14 && choices[8].keyOpacity === 0.8,
    "custom geometry and opacity cross the native skin boundary",
  );
});

group("colours are formatted the way the Java formats them", () => {
  const forest = KeyboardSkin.from("forest", false);
  check(
    /^#[0-9A-F]{6}$/.test(forest.background),
    `six upper-case hex digits, got ${forest.background}`,
  );
  check(forest.keyBackground === "#FFFFFF", "the light forest key is pure white");
  check(forest.keyForeground === "#000000", "and its label is black");
  const dark = KeyboardSkin.from("forest", true);
  check(dark.keyForeground === "#FFFFFF", "the dark label is white");
  check(dark.background !== forest.background, "dark mode changes the background");
  // rgb(.094, .36, .28) rounds to 24, 92, 71.
  check(forest.accent === "#185C47", `forest accent rounds to #185C47, got ${forest.accent}`);
});

group("the border colour carries an alpha byte in front", () => {
  const forest = KeyboardSkin.from("forest", false);
  check(
    /^#[0-9A-F]{8}$/.test(forest.borderColor),
    `an eight-digit ARGB value, got ${forest.borderColor}`,
  );
  check(
    forest.borderColor.substring(3) === forest.accent.substring(1),
    "the RGB part is the accent",
  );
  // 0.28 * 255 rounds to 71, which is 0x47.
  check(forest.borderColor.substring(1, 3) === "47", "the alpha is 0.28 of full");
  const midnight = KeyboardSkin.from("midnight", true);
  check(
    midnight.borderColor.substring(1, 3) === "A6",
    "midnight carries a neon edge at 0.65, which is 0xA6",
  );
});

group("each built-in skin is distinct and self-consistent", () => {
  const skins = KeyboardSkin.builtIns(false);
  const ids = new Set(skins.map((skin) => skin.id));
  check(ids.size === skins.length, "no duplicate ids");
  const keys = new Set(skins.map((skin) => skin.key()));
  check(keys.size === skins.length, "no two skins share a cache key");
  for (const skin of skins) {
    check(skin.title.length > 0 && skin.description.length > 0, `${skin.id} is described`);
    check(
      skin.keyShape === "rounded" && skin.keyMaterial === "flat",
      `${skin.id} uses the built-in key treatment`,
    );
    check(skin.photo === null, `${skin.id} carries no photo`);
  }
  check(KeyboardSkin.from("typewriter", false).monospaced === true, "typewriter is monospaced");
  check(KeyboardSkin.from("blueprint", false).monospaced === true, "so is blueprint");
  check(KeyboardSkin.from("forest", false).monospaced === false, "forest is not");
});

group("the cache key separates light from dark and carries the design", () => {
  check(
    KeyboardSkin.from("forest", false).key() !== KeyboardSkin.from("forest", true).key(),
    "light and dark are different skins to the cache",
  );
  const custom = KeyboardSkin.from("custom", false, CustomKeyboardSkin.defaults());
  check(custom.key().startsWith("custom:false:"), "the custom key carries the design behind it");
});

console.log("CustomKeyboardSkin");

function document(fields: CustomSkinDocument): CustomSkinDocument {
  return fields;
}

group("out-of-range values are clamped, not rejected outright", () => {
  const wild = CustomKeyboardSkin.from(
    document({
      cornerRadius: 500,
      borderWidth: -3,
      shadow: 9,
      keyOpacity: 0,
      patternOpacity: 5,
      pattern: 99,
      photoShade: 9,
      photoPosition: -1,
    }),
  );
  check(wild.cornerRadius() === 20, "corner radius clamps to 20");
  check(wild.borderWidth() === 0, "border width clamps to 0");
  check(wild.shadow() === 0.4, "shadow clamps to 0.4");
  check(wild.keyOpacity() === 0.25, "key opacity clamps to a quarter, not to invisible");
  check(wild.patternOpacity() === 0.5, "pattern opacity clamps to a half");
  check(wild.pattern() === 3, "pattern clamps to the last one");
  check(wild.photoShade() === 0.8, "photo shade clamps");
  check(wild.photoPosition() === 0, "photo position clamps");
});

group("a non-finite value falls back rather than clamping", () => {
  const broken = CustomKeyboardSkin.from(
    document({ cornerRadius: Number.NaN, shadow: Number.NaN }),
  );
  check(broken.cornerRadius() === 8, "NaN takes the default corner radius");
  check(broken.shadow() === 0, "and the default shadow");
});

group("a design saved without a photo carries photo: null and still loads", () => {
  // The shared preference document serializes a missing photo as JSON null; reading it used to throw inside onCreate and left the input method with no session at all.
  const saved = CustomKeyboardSkin.from(document({ background: 0x102030, photo: null }));
  check(saved.background() === "#102030", "the rest of the design is kept");
  check(saved.photo() === null, "no photo is drawn");
  check(saved.photoSource() === null, "and no photo source is offered");
});

group("an unknown enum value takes the first choice", () => {
  const odd = CustomKeyboardSkin.from(document({ keyShape: "triangle", keyMaterial: "velvet" }));
  check(odd.keyShape() === "rounded", "an unknown shape is rounded");
  check(odd.keyMaterial() === "flat", "an unknown material is flat");
  const valid = CustomKeyboardSkin.from(document({ keyShape: "capsule", keyMaterial: "glass" }));
  check(valid.keyShape() === "capsule", "a known shape is kept");
  check(valid.keyMaterial() === "glass", "a known material is kept");
});

group("the action label flips to black on a light action key", () => {
  const light = CustomKeyboardSkin.from(document({ actionBackground: 0xffffff }));
  check(light.actionForeground() === "#000000", "white needs a black label");
  const dark = CustomKeyboardSkin.from(document({ actionBackground: 0x000000 }));
  check(dark.actionForeground() === "#FFFFFF", "black needs a white label");
  const mid = CustomKeyboardSkin.from(document({ actionBackground: 0x185c47 }));
  check(mid.actionForeground() === "#FFFFFF", "the default green is dark enough for white");
});

group("colours print as six upper-case hex digits with the high byte dropped", () => {
  const skin = CustomKeyboardSkin.from(document({ background: 0xff123456 }));
  check(skin.background() === "#123456", "anything above the low three bytes is masked off");
  const small = CustomKeyboardSkin.from(document({ background: 0x0000ff }));
  check(small.background() === "#0000FF", "a small value is padded to six digits");
});

group("the border colour falls back to the accent", () => {
  const inherited = CustomKeyboardSkin.from(document({ accent: 0x123456 }));
  check(inherited.borderColor() === "#123456", "with no custom border, the accent is used");
  const explicit = CustomKeyboardSkin.from(
    document({ accent: 0x123456, customBorderColor: 0xabcdef }),
  );
  check(explicit.borderColor() === "#ABCDEF", "an explicit border colour wins");
});

group("only real image bytes are accepted as a photo", () => {
  const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0, 0]);
  const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10, 0]);
  const gif = new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0]);
  const webp = new Uint8Array([0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50]);
  check(
    supportedPhoto(jpeg) && supportedPhoto(png) && supportedPhoto(gif) && supportedPhoto(webp),
    "JPEG, PNG, GIF and WebP are recognised by magic number",
  );
  check(!supportedPhoto(new Uint8Array([1, 2, 3, 4])), "arbitrary bytes are not an image");
  check(!supportedPhoto(new Uint8Array([])), "nothing is not an image");
  check(
    !supportedPhoto(new Uint8Array([0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 1, 2, 3, 4])),
    "a RIFF container that is not WebP is refused",
  );
  const withPhoto = CustomKeyboardSkin.from(document({}), png);
  check(withPhoto.photo() !== null, "a valid photo is kept");
  const bogus = CustomKeyboardSkin.from(document({}), new Uint8Array([1, 2, 3]));
  check(bogus.photo() === null, "an invalid one is dropped");
  const huge = new Uint8Array(512001);
  huge.set(png.subarray(0, 8));
  check(
    CustomKeyboardSkin.from(document({}), huge).photo() === null,
    "an oversized photo is dropped even when it is a real PNG",
  );
  const encoded = CustomKeyboardSkin.from(document({ photo: "iVBORw0KGgo=" }));
  check(encoded.photo()?.length === 8, "the shared base64 photo is decoded and bounded");
  check(
    encoded.photoSource() === "data:image/png;base64,iVBORw0KGgo=",
    "a validated photo becomes an ArkUI image source",
  );
  check(
    CustomKeyboardSkin.from(document({ photo: "not-base64" })).photoSource() === null,
    "invalid preference text never reaches the image decoder",
  );
});

group("custom key treatments reach native surface values", () => {
  const capsule = KeyboardSkin.from(
    "custom",
    false,
    CustomKeyboardSkin.from(
      document({ keyShape: "capsule", keyMaterial: "glass", keyOpacity: 0.45, shadow: 0.3 }),
    ),
  );
  check(capsule.keyCornerRadius() === 999, "a capsule asks ArkUI for a pill radius");
  check(capsule.materialTop() === "#3DFFFFFF", "glass carries a visible top highlight");
  check(
    capsule.keySurfaceBackground(false).startsWith("#73"),
    "key opacity changes only the fill alpha",
  );
  check(capsule.shadowColor().startsWith("#4D"), "the configured shadow reaches its ARGB colour");
});

group("the design key changes whenever the drawing would", () => {
  const base = CustomKeyboardSkin.defaults().key();
  check(
    CustomKeyboardSkin.from(document({ accent: 0x123456 })).key() !== base,
    "a colour change changes the key",
  );
  check(
    CustomKeyboardSkin.from(document({ cornerRadius: 12 })).key() !== base,
    "so does a geometry change",
  );
  const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10, 42]);
  check(CustomKeyboardSkin.from(document({}), png).key() !== base, "so does adding a photo");
  check(CustomKeyboardSkin.from(document({})).key() === base, "an empty document is the default");
});

group("the panel is as tall as what the view stacks inside it", () => {
  const expected =
    KeyboardMetrics.COMPOSITION_ROW_HEIGHT_VP +
    KeyboardMetrics.CANDIDATE_ROW_HEIGHT_VP +
    KeyboardMetrics.ROW_HEIGHT_VP * KeyboardMetrics.KEY_ROWS +
    KeyboardMetrics.ROW_SPACING_VP * KeyboardMetrics.KEY_ROWS +
    KeyboardMetrics.ROOT_VERTICAL_PADDING_VP * 2;
  check(
    KeyboardMetrics.totalHeightVp() === expected,
    "the total counts the strip, every key row, the gaps between them and both paddings",
  );
  // One gap per key row: one between the strip and the first row, then one before each of the rest.
  check(
    KeyboardMetrics.ROW_SPACING_VP * KeyboardMetrics.KEY_ROWS ===
      KeyboardMetrics.ROW_SPACING_VP * 4,
    "four rows are separated by four gaps, not three",
  );
  check(KeyboardMetrics.totalHeightVp() > 0, "a panel given a height of zero never appears");
  check(
    KeyboardMetrics.totalHeightVp(70, 0, 1, 18) - KeyboardMetrics.totalHeightVp(70, 0, 0, 18) ===
      KeyboardMetrics.glossHeightVp(1, 18),
    "one reserved gloss row grows the panel instead of taking height from the keys",
  );
  check(
    KeyboardMetrics.candidateHeightVp("horizontal", 3, true, 0, 1, 18) -
      KeyboardMetrics.candidateHeightVp("horizontal", 3, true, 0, 0, 18) ===
      KeyboardMetrics.glossHeightVp(1, 18),
    "a horizontal desktop strip also reserves the asynchronous gloss row",
  );
  check(
    KeyboardMetrics.candidateHeightVp("vertical", 3, true, 0, 1) ===
      KeyboardMetrics.candidateHeightVp("vertical", 3, true, 0, 0),
    "a vertical 2-in-1 list keeps its beside-the-candidate adaptation",
  );
  check(
    KeyboardMetrics.glossHeightVp(1, 32) > KeyboardMetrics.glossHeightVp(1, 18),
    "a large candidate font grows its gloss line instead of clipping it",
  );

  // The user's settings move both, and the panel has to move with them or the bottom row is clipped.
  check(
    KeyboardMetrics.totalHeightVp(70, 12) - KeyboardMetrics.totalHeightVp(70, 0) === 12,
    "a height adjustment lands on the total exactly once",
  );
  check(
    KeyboardMetrics.totalHeightVp(100, 0) - KeyboardMetrics.totalHeightVp(70, 0) ===
      3 * KeyboardMetrics.KEY_ROWS,
    "wider row spacing adds one gap per key row",
  );
  check(
    KeyboardMetrics.totalHeightVp(70, -12) < KeyboardMetrics.totalHeightVp(70, 0),
    "a negative adjustment shortens it",
  );
});

group("the height adjustment is spread across rows without losing a pixel", () => {
  const rows = KeyboardMetrics.KEY_ROWS;
  const base = KeyboardMetrics.ROW_HEIGHT_VP;
  for (const adjustment of [0, 1, 7, 12, -12, 48]) {
    let total = 0;
    for (let index = 0; index < rows; index++) {
      total += KeyboardGeometry.adjustedRowHeight(base, adjustment, rows, index);
    }
    check(
      total === base * rows + adjustment,
      `the rows still sum to the adjusted total at ${adjustment}`,
    );
  }
  check(
    KeyboardGeometry.heightAdjustment(null) === KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP,
    "an unset adjustment takes the default rather than clamping to the minimum",
  );
  check(
    KeyboardGeometry.heightAdjustment(999) === KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP,
    "an out-of-range adjustment is clamped",
  );
});

group("an emoji catalog page is checked before it is trusted", () => {
  const item: EmojiItem = EmojiCatalogModel.item("\u{1f600}", "grinning", "Smileys and emotion");
  check(item.text === "\u{1f600}", "a valid entry survives intact");
  let rejected = false;
  try {
    EmojiCatalogModel.item("", "empty", "Smileys and emotion");
  } catch (error) {
    rejected = true;
  }
  check(rejected, "an entry with no text is refused");
  // Code points, not UTF-16 units: a single emoji is routinely two units and a sequence is more.
  const long = "\u{1f600}".repeat(MAX_TEXT_CODE_POINTS + 1);
  rejected = false;
  try {
    EmojiCatalogModel.item(long, "", "Smileys and emotion");
  } catch (error) {
    rejected = true;
  }
  check(rejected, "length is counted in code points, so a long sequence is still refused");

  const page = EmojiCatalogModel.validatePage([item], 0, EMOJI_PAGE_SIZE, 1, false);
  check(page.nextOffset === 1 && !page.complete, "a page that advances is accepted");
  rejected = false;
  try {
    // The loop-forever case: not complete, yet the offset has not moved.
    EmojiCatalogModel.validatePage([item], 4, EMOJI_PAGE_SIZE, 4, false);
  } catch (error) {
    rejected = true;
  }
  check(rejected, "an incomplete page that advances nowhere is refused rather than paged forever");
  rejected = false;
  try {
    EmojiCatalogModel.validatePage([item], 8, EMOJI_PAGE_SIZE, 4, true);
  } catch (error) {
    rejected = true;
  }
  check(rejected, "an offset that goes backwards is refused");
});

group("recent emoji are most-recent first, deduplicated and bounded", () => {
  const recents = EmojiCatalogModel.recordRecent(["a", "b"], "c");
  check(recents[0] === "c" && recents.length === 3, "the newest goes to the front");
  const promoted = EmojiCatalogModel.recordRecent(["a", "b", "c"], "b");
  check(
    promoted[0] === "b" && promoted.length === 3,
    "choosing one already there promotes it rather than duplicating it",
  );
  const many: string[] = [];
  for (let index = 0; index < EMOJI_RECENTS_LIMIT + 10; index++) {
    many.push(`e${index}`);
  }
  check(
    EmojiCatalogModel.normalizeRecents(many).length === EMOJI_RECENTS_LIMIT,
    "the stored list is bounded however long it grew",
  );
  check(EmojiCatalogModel.normalizeRecents(null).length === 0, "nothing stored is no recents");
  check(
    EmojiCatalogModel.normalizeRecents(["", "a", "a"]).join(",") === "a",
    "empty and repeated entries are dropped without discarding the rest",
  );
  check(
    EmojiCatalogModel.parseRecents('["😀","😀","",42]').join(",") === "😀",
    "persisted recents keep only bounded strings and remove duplicates",
  );
  check(
    EmojiCatalogModel.parseRecents('{"recent":[]}').length === 0,
    "a recents document with the wrong shape is treated as empty",
  );
  check(
    EmojiCatalogModel.parseRecents("not json").length === 0,
    "malformed recents do not break the keyboard",
  );
  check(
    JSON.parse(EmojiCatalogModel.serializeRecents(["b", "a", "b"]))[0] === "b",
    "serialization writes the normalized newest-first list",
  );
});

group("engine catalog groups are bounded before reaching the touch panel", () => {
  const groups = normalizeGroups(["happy", "", "happy", "  ", 42, "sad"]);
  check(groups.join(",") === "happy,sad", "kaomoji groups drop invalid and repeated values");
  check(normalizeGroups(["x".repeat(129)]).length === 0, "an oversized group name is ignored");
  check(
    normalizeGroups(Array.from({ length: 65 }, (_unused, index) => `g${index}`)).length === 0,
    "an oversized group list is rejected as empty",
  );
  check(normalizeGroups(null).length === 0, "a malformed group response is empty");

  const symbols = normalizeSymbolGroups([
    { parent: "math", title: "数学" },
    { parent: "math", title: "数学" },
    { parent: "math", title: "" },
    { parent: "  ", title: "空白" },
    { parent: "arrows", title: "箭头" },
    "not an object",
  ]);
  check(
    symbols.length === 2 && symbols[1].parent === "arrows",
    "symbol groups validate fields and remove duplicate pairs",
  );
  check(
    normalizeSymbolGroups([{ parent: "x".repeat(129), title: "太长" }]).length === 0,
    "an oversized symbol parent is ignored",
  );
  check(
    normalizeSymbolGroups(
      Array.from({ length: 65 }, (_unused, index) => ({ parent: `p${index}`, title: `t${index}` })),
    ).length === 0,
    "an oversized symbol list is rejected as empty",
  );
  check(normalizeSymbolGroups(undefined).length === 0, "a malformed symbol response is empty");
});

function clip(text: string, at: number, pinned = false): ClipboardHistoryItem {
  return { text, at, pinned };
}

function refuses(action: () => void, failure: ClipboardFailure): boolean {
  try {
    action();
    return false;
  } catch (error) {
    return error instanceof ClipboardHistoryError && error.failure === failure;
  }
}

group("clipboard history is ordered pinned first, then most recent", () => {
  const items = ClipboardHistoryStore.ordered([
    clip("old", 100),
    clip("new", 300),
    clip("kept", 200, true),
  ]);
  check(items[0].text === "kept", "a pinned entry sorts above an unpinned one however old it is");
  check(items[1].text === "new" && items[2].text === "old", "the rest are most recent first");
});

group("clipboard history preference is a privacy gate", () => {
  check(
    !ClipboardHistoryPreferencePolicy.enabled(undefined),
    "a missing shared preference defaults to disabled",
  );
  check(
    !ClipboardHistoryPreferencePolicy.enabled(false),
    "an explicit false preference disables history",
  );
  check(
    ClipboardHistoryPreferencePolicy.enabled(true),
    "an explicit true preference enables history",
  );
  check(
    !ClipboardHistoryPreferencePolicy.canReadOrWrite(false),
    "disabled history cannot read or write the private file",
  );
  check(
    ClipboardHistoryPreferencePolicy.canReadOrWrite(true),
    "enabled history can read and write the private file",
  );
  check(
    ClipboardHistoryPreferencePolicy.shouldClearHistory(false),
    "disabled history requires immediate cleanup",
  );
  check(
    !ClipboardHistoryPreferencePolicy.shouldClearHistory(true),
    "enabled history does not clear existing entries",
  );
});

group("adding to the clipboard history", () => {
  const existing = [clip("a", 100)];
  const added = ClipboardHistoryStore.add(existing, "b", 200);
  check(added[0].text === "b" && added.length === 2, "a new entry goes to the front");
  const again = ClipboardHistoryStore.add(added, "a", 300);
  check(
    again.length === 2 && again[0].text === "a",
    "text already present is moved rather than duplicated",
  );
  check(
    refuses(() => ClipboardHistoryStore.add([], "   ", 1), ClipboardFailure.EMPTY),
    "whitespace alone is nothing worth saving",
  );
  const long = "x".repeat(ClipboardHistoryStore.LIMIT > 0 ? 10001 : 0);
  check(
    refuses(() => ClipboardHistoryStore.add([], long, 1), ClipboardFailure.TOO_LONG),
    "an entry past the character bound is refused",
  );

  const full: ClipboardHistoryItem[] = [];
  for (let index = 0; index < ClipboardHistoryStore.LIMIT; index++) {
    full.push(clip(`e${index}`, index));
  }
  const evicted = ClipboardHistoryStore.add(full, "fresh", 1000);
  check(evicted.length === ClipboardHistoryStore.LIMIT, "the list stays at its limit");
  check(!evicted.some((entry) => entry.text === "e0"), "the oldest unpinned entry made the room");

  const allPinned = full.map((entry) => clip(entry.text, entry.at, true));
  check(
    refuses(() => ClipboardHistoryStore.add(allPinned, "fresh", 1000), ClipboardFailure.FULL),
    "with every entry pinned there is nothing to evict, so it refuses rather than dropping a pin",
  );
});

group("a clipboard history document is not trusted because we wrote it", () => {
  check(ClipboardHistoryStore.parse(null).length === 0, "no file is an empty history");
  check(ClipboardHistoryStore.parse("").length === 0, "an empty file is an empty history");
  check(
    refuses(() => ClipboardHistoryStore.parse("{"), ClipboardFailure.INVALID_FILE),
    "a document that will not parse is reported, not silently emptied",
  );
  check(
    refuses(() => ClipboardHistoryStore.parse('{"text":"a"}'), ClipboardFailure.INVALID_FILE),
    "a document that is not a list is refused",
  );
  check(
    refuses(
      () => ClipboardHistoryStore.parse('[{"text":"","at":1,"pinned":false}]'),
      ClipboardFailure.INVALID_FILE,
    ),
    "an entry the policy would not accept invalidates the file",
  );
  check(
    refuses(
      () => ClipboardHistoryStore.parse('[{"text":"a","at":"soon","pinned":false}]'),
      ClipboardFailure.INVALID_FILE,
    ),
    "a timestamp that is not a number invalidates the file",
  );
  const tooMany = JSON.stringify(
    Array.from({ length: ClipboardHistoryStore.LIMIT + 1 }, (_unused, index) =>
      clip(`e${index}`, index),
    ),
  );
  check(
    refuses(() => ClipboardHistoryStore.parse(tooMany), ClipboardFailure.INVALID_FILE),
    "more entries than the limit invalidates the file",
  );
  const good = ClipboardHistoryStore.parse('[{"text":"a","at":1,"pinned":false}]');
  check(good.length === 1 && good[0].text === "a", "a sound document round-trips");
});

group("account and cloud clipboard bridge keeps secrets native", () => {
  let stored: string | null = null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const calls: { method: string; path: string; token?: string; body?: Record<string, unknown> }[] =
    [];
  const transport: AccountTransport = {
    request: async (method, path, token, body) => {
      calls.push({ method, path, token, body });
      if (path === "/v1/auth/providers")
        return { status: 200, body: '{"providers":{"email":true,"phone":false}}' };
      if (path === "/v1/auth/challenges")
        return { status: 200, body: '{"challenge_id":"challenge","expires_in":60}' };
      if (path === "/v1/auth/login")
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "u1", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      if (path.includes("/dictionaries/pinyin"))
        return { status: 200, body: '{"entries":[],"has_more":false,"offset":0}' };
      if (path.includes("/clipboard"))
        return {
          status: 200,
          body: path.endsWith("/clipboard")
            ? '{"enabled":true,"items":[{"id":"1","text":"hello"}]}'
            : "{}",
        };
      return {
        status: 200,
        body: '{"user":{"id":"u1","display_name":"Test","created_at":"2026-01-01"},"identities":[]}',
      };
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  void bridge
    .handle('{"operation":"request_code","provider":"email","target":"user@example.com"}')
    .then((result) => {
      check(JSON.parse(result).ok === true, "challenge response is structured");
    });
  void bridge
    .handle('{"operation":"login","challenge_id":"challenge","credential":"123456"}')
    .then((result) => {
      check(JSON.parse(result).ok === true && stored !== null, "login stores a native session");
      const beforeInvalidDelete = calls.length;
      void bridge
        .handle(
          JSON.stringify({
            operation: "clipboard",
            clipboard_operation: "delete",
            id: "../auth/logout",
          }),
        )
        .then((reply) => {
          check(
            JSON.parse(reply).error === "account_invalid",
            "a path-like clipboard id is rejected locally",
          );
          check(
            calls.length === beforeInvalidDelete,
            "an invalid clipboard id never carries the session to transport",
          );
          void bridge
            .handle(
              JSON.stringify({
                operation: "clipboard",
                clipboard_operation: "delete",
                id: "c".repeat(64),
              }),
            )
            .then((validReply) => {
              check(JSON.parse(validReply).ok === true, "a digest clipboard id can be deleted");
              check(
                calls.some((call) => call.path === `/v1/users/me/clipboard/${"c".repeat(64)}`),
                "the validated id occupies exactly one path segment",
              );
            });
        });
    });
  void bridge.handle('{"operation":"unknown"}').then((result) => {
    check(JSON.parse(result).ok === false, "unknown clipboard shape is rejected");
  });
  void bridge
    .handle(JSON.stringify({ operation: "clipboard", clipboard_operation: "add", text: "\u0000" }))
    .then((result) => {
      check(
        JSON.parse(result).error === "account_invalid",
        "control characters never reach transport",
      );
    });
  void bridge
    .handle(
      JSON.stringify({
        operation: "clipboard",
        clipboard_operation: "add",
        text: "😀".repeat(2001),
      }),
    )
    .then((result) => {
      check(
        JSON.parse(result).error === "account_invalid",
        "clipboard text is bounded in UTF-16 units before transport",
      );
    });
  void bridge.handle('{"operation":"profile"}').then((result) => {
    check(
      JSON.parse(result).error === "account_unauthorized",
      "requests before login return unauthorized",
    );
    check(
      calls.every((call) => call.token === undefined),
      "invalid requests do not carry a token",
    );
  });
  void bridge
    .handle(
      JSON.stringify({
        operation: "dictionary",
        dictionary_operation: "list",
        kind: "pinyin",
        offset: 0,
        search: "ni hao",
      }),
    )
    .then((result) => {
      check(
        JSON.parse(result).error === "account_unauthorized",
        "dictionary requests require the native session",
      );
    });
  void bridge
    .handle(
      JSON.stringify({
        operation: "dictionary",
        dictionary_operation: "list",
        kind: "pinyin",
        offset: -1,
        search: "",
      }),
    )
    .then((result) => {
      check(
        JSON.parse(result).error === "account_invalid",
        "dictionary offsets are bounded before transport",
      );
    });
});

group("cloud candidate mutations preserve the service protocol", () => {
  const stored = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600000,
    user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
  });
  const calls: { method: string; body?: Record<string, unknown> }[] = [];
  const bridge = new AccountCloudBridge(
    {
      request: async (method, path, _token, body) => {
        if (path === "/v1/users/me/dictionary/positions") calls.push({ method, body });
        return { status: 200, body: '{"revision":43}' };
      },
    },
    { load: () => stored, save: () => {}, clear: () => {} },
  );
  const mutate = (position: number | null) =>
    bridge.handle(
      JSON.stringify({
        operation: "dictionary",
        dictionary_operation: "set_fixed_position",
        context: "server:context",
        code: "ni'hao",
        word: "你好",
        position,
        revision: 42,
      }),
    );
  void mutate(null).then((reply) => {
    check(JSON.parse(reply).ok === true, "a fixed position can be removed");
    check(calls[0]?.method === "DELETE", "removal uses DELETE");
    check(calls[0]?.body?.context === "server:context", "removal preserves server context");
    check(
      calls[0]?.body !== undefined && !("position" in calls[0].body),
      "removal omits position instead of serializing null",
    );
    void mutate(3).then((assigned) => {
      check(JSON.parse(assigned).ok === true, "a fixed position can be assigned");
      check(calls[1]?.method === "PUT", "assignment uses PUT");
      check(calls[1]?.body?.position === 3, "assignment sends the requested position");
    });
  });
});

group("cloud dictionary exports are bounded before a native host saves them", () => {
  const stored = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600000,
    user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
  });
  let response: AccountTransportResponse = { status: 200, body: "A" };
  let calls = 0;
  const bridge = new AccountCloudBridge(
    {
      request: async () => {
        calls += 1;
        return response;
      },
    },
    { load: () => stored, save: () => {}, clear: () => {} },
  );
  const ordinaryJsonLimit = "A".repeat(2 * 1024 * 1024);
  response = { status: 200, body: ordinaryJsonLimit, contentLength: ordinaryJsonLimit.length };
  void bridge.downloadDictionary("quick", "standard").then((reply) => {
    check(
      reply.body?.length === ordinaryJsonLimit.length,
      "a 2 MiB export reaches the native saver",
    );
    response = { status: 200, body: "A".repeat(3 * 1024 * 1024 + 1) };
    void bridge.downloadDictionary("quick", "standard").then((oversized) => {
      check(oversized.body === undefined, "an export above 3 MiB is refused before saving");
      response = { status: 200, body: "short", contentLength: 3000000 };
      void bridge.downloadDictionary("quick", "standard").then((truncated) => {
        check(truncated.body === undefined, "a truncated export is refused before saving");
        const beforeInvalid = calls;
        void bridge.downloadDictionary("quick", "hans").then((invalid) => {
          check(invalid.error === "account_invalid", "an unsupported export format is refused");
          check(
            calls === beforeInvalid,
            "an invalid export never carries the session to transport",
          );
        });
      });
    });
  });
});

group("large account files use the authenticated streaming transport", () => {
  const stored = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600000,
    user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
  });
  const downloads: { path: string; maximum: number; media: string; token: string }[] = [];
  let response = {
    status: 200,
    bytes: MAX_DICTIONARY_EXPORT_BYTES,
    contentLength: MAX_DICTIONARY_EXPORT_BYTES,
    contentType: "text/plain",
  };
  const bridge = new AccountCloudBridge(
    {
      request: async () => ({ status: 500, body: "" }),
      download: async (path, token, _destination, maximum, media) => {
        downloads.push({ path, maximum, media, token });
        return response;
      },
    },
    { load: () => stored, save: () => {}, clear: () => {} },
  );
  void bridge
    .downloadAuthenticated(
      "/v1/users/me/dictionaries/quick/export?format=standard",
      "/private/export.tsv",
      MAX_DICTIONARY_EXPORT_BYTES,
      "text/plain",
    )
    .then((downloaded) => {
      check(
        downloaded.bytes === MAX_DICTIONARY_EXPORT_BYTES,
        "the full 384 MiB dictionary contract is accepted without a bridged string",
      );
      check(
        downloads[0]?.maximum === MAX_DICTIONARY_EXPORT_BYTES &&
          downloads[0]?.media === "text/plain",
        "the native transport receives the source limit and media type",
      );
      response = {
        status: 200,
        bytes: 100,
        contentLength: 101,
        contentType: "application/x-ndjson",
      };
      void bridge
        .downloadAuthenticated(
          "/v1/users/me/dictionary/snapshot",
          "/private/snapshot.ndjson",
          MAX_SNAPSHOT_DOWNLOAD_BYTES,
          "application/x-ndjson",
        )
        .then((truncated) => {
          check(truncated.bytes === undefined, "a truncated streamed snapshot is refused");
          const beforeInvalid = downloads.length;
          void bridge
            .downloadAuthenticated(
              "/v1/users/me/dictionary/snapshot",
              "/private/snapshot.ndjson",
              MAX_SNAPSHOT_DOWNLOAD_BYTES + 1,
              "application/x-ndjson",
            )
            .then((invalid) => {
              check(invalid.error === "account_invalid", "download bounds stop at 512 MiB");
              check(
                downloads.length === beforeInvalid,
                "an invalid streaming request never carries the session to transport",
              );
            });
        });
    });
});

group("snapshot restores stream privately and retry one rejected access token", () => {
  let stored = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600000,
    user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
  });
  const uploads: { source: string; revision: number; sha256: string; token: string }[] = [];
  const digest = "c".repeat(64);
  const bridge = new AccountCloudBridge(
    {
      request: async (method, path) => {
        check(method === "POST" && path === "/v1/auth/refresh", "a rejected upload refreshes once");
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "d".repeat(64),
            refresh_token: "e".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      },
      uploadSnapshot: async (source, revision, sha256, token) => {
        uploads.push({ source, revision, sha256, token });
        return token.startsWith("a")
          ? { status: 401, body: "" }
          : { status: 200, body: '{"revision":10,"reset":true}' };
      },
    },
    {
      load: () => stored,
      save: (value) => {
        stored = value;
      },
      clear: () => {
        stored = "";
      },
    },
  );

  void bridge
    .restoreSnapshotAuthenticated("/private/restore.ndjson", 9, digest)
    .then((restored) => {
      check(restored.value?.revision === 10, "the advancing reset response is accepted");
      check(uploads.length === 2, "the private file upload is attempted exactly twice");
      check(
        uploads[0]?.source === "/private/restore.ndjson" &&
          uploads[0]?.sha256 === digest &&
          uploads[1]?.token === "d".repeat(64),
        "the file identity survives access-token rotation",
      );
      const beforeInvalid = uploads.length;
      void bridge.restoreSnapshotAuthenticated("relative.ndjson", 9, digest).then((invalid) => {
        check(invalid.error === "account_invalid", "only absolute private files may be uploaded");
        check(uploads.length === beforeInvalid, "invalid restore input never reaches transport");
      });
    });

  const deferred: { resolve?: (response: AccountTransportResponse) => void } = {};
  const cancelled = new AccountCloudBridge(
    {
      request: async () => ({ status: 500, body: "" }),
      uploadSnapshot: async () =>
        await new Promise<AccountTransportResponse>((resolve) => {
          deferred.resolve = resolve;
        }),
    },
    { load: () => stored, save: () => {}, clear: () => {} },
  );
  void cancelled
    .restoreSnapshotAuthenticated("/private/restore.ndjson", 9, digest)
    .then((result) => {
      check(result.error === "account_cancelled", "logout invalidates an in-flight restore reply");
    });
  void cancelled.handle('{"operation":"clear_expired"}').then(() => {
    check(deferred.resolve !== undefined, "the upload had started before the session changed");
    deferred.resolve?.({ status: 200, body: '{"revision":10,"reset":true}' });
  });
});

group("snapshot conflict pages cannot lie about their cursor", () => {
  const changed = JSON.stringify({
    changes: [
      {
        revision: 2,
        ranking: [
          {
            id: "",
            kind: "pinyin",
            code: "ni",
            word: "你",
            weight: 8,
            revision: 2,
            user_inserted: false,
          },
        ],
        selection: { context: "pinyin", code: "ni", word: "你", count: 0 },
        position: { context: "pinyin", code: "ni", word: "你", position: 0 },
        reset: true,
      },
    ],
    next: 2,
    has_more: true,
  });
  check(
    dictionaryChangePageChanged(changed, 0) === true,
    "any well-formed change conflicts with a prepared snapshot",
  );
  check(
    dictionaryChangePageChanged('{"changes":[],"next":4,"has_more":false}', 4) === false,
    "an empty stationary final page keeps the snapshot current",
  );
  for (const invalid of [
    '{"changes":[{"revision":5}],"next":5,"has_more":false}',
    '{"changes":[],"next":7,"has_more":false}',
    '{"changes":[],"next":7,"has_more":true}',
  ]) {
    check(
      dictionaryChangePageChanged(invalid, invalid.includes("5") ? 5 : 6) === null,
      "a non-advancing or inconsistent cursor is refused",
    );
  }
});

group("account access tokens rotate once and cannot outlive logout", () => {
  const savedSession = (access: string, refresh: string, expiresAt: number) =>
    JSON.stringify({
      access_token: access.repeat(64),
      refresh_token: refresh.repeat(64),
      token_type: "Bearer",
      expires_at: expiresAt,
      user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
    });
  const refreshBody = (access: string) =>
    JSON.stringify({
      access_token: access.repeat(64),
      refresh_token: "f".repeat(64),
      token_type: "Bearer",
      expires_in: 900,
      user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
    });

  let stored: string | null = savedSession("a", "b", Date.now() - 1);
  const refreshDeferred: { resolve?: (response: AccountTransportResponse) => void } = {};
  let refreshCalls = 0;
  const transport: AccountTransport = {
    request: async (_method, path, token) => {
      if (path === "/v1/auth/refresh") {
        refreshCalls += 1;
        return await new Promise<AccountTransportResponse>((resolve) => {
          refreshDeferred.resolve = resolve;
        });
      }
      check(token === "c".repeat(64), "authenticated work receives the rotated access token");
      return {
        status: 200,
        body: '{"user":{"id":"synthetic-user","display_name":"Test","created_at":"2026-01-01"},"identities":[]}',
      };
    },
  };
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  void bridge.handle('{"operation":"status"}').then((reply) => {
    check(
      JSON.parse(reply).value.user.id === "synthetic-user",
      "an expired access token does not erase the refreshable account",
    );
  });
  const first = bridge.handle('{"operation":"profile"}');
  const second = bridge.handle('{"operation":"profile"}');
  check(refreshCalls === 1, "concurrent account callers share one refresh request");
  refreshDeferred.resolve?.({ status: 200, body: refreshBody("c") });
  void Promise.all([first, second]).then((replies) => {
    check(
      replies.every((reply) => JSON.parse(reply).ok === true),
      "both callers resume after rotation",
    );
    check(
      stored !== null && JSON.parse(stored).access_token === "c".repeat(64),
      "rotated credentials replace the persisted session",
    );
  });

  let lateStored: string | null = savedSession("d", "e", Date.now() - 1);
  const lateDeferred: { resolve?: (response: AccountTransportResponse) => void } = {};
  const lateBridge = new AccountCloudBridge(
    {
      request: async () =>
        await new Promise<AccountTransportResponse>((resolve) => {
          lateDeferred.resolve = resolve;
        }),
    },
    {
      load: () => lateStored,
      save: (value) => {
        lateStored = value;
      },
      clear: () => {
        lateStored = null;
      },
    },
  );
  const late = lateBridge.handle('{"operation":"profile"}');
  void lateBridge.handle('{"operation":"clear_expired"}').then(() => {
    lateDeferred.resolve?.({ status: 200, body: refreshBody("f") });
  });
  void late.then((reply) => {
    check(JSON.parse(reply).error === "account_cancelled", "logout rejects a late refresh result");
    check(lateStored === null, "a late refresh cannot restore cleared storage");
  });
});

group("profile updates preserve the session and cannot outlive logout", () => {
  const expiresAt = Date.now() + 600000;
  const original = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: expiresAt,
    user: { id: "synthetic-user", display_name: "Before", created_at: "2026-01-01" },
  });
  let stored: string | null = original;
  const calls: string[] = [];
  const bridge = new AccountCloudBridge(
    {
      request: async (method, path, token) => {
        calls.push(`${method} ${path}`);
        check(token === "a".repeat(64), "profile updates keep credentials inside the host");
        if (method === "PATCH") return { status: 204, body: "" };
        return {
          status: 200,
          body: JSON.stringify({
            user: {
              id: "synthetic-user",
              display_name: "Canonical name",
              created_at: "2026-01-01",
            },
            identities: [{ provider: "email", subject: "synthetic@example.test" }],
          }),
        };
      },
    },
    {
      load: () => stored,
      save: (value) => {
        stored = value;
      },
      clear: () => {
        stored = null;
      },
    },
  );
  void bridge
    .handle(JSON.stringify({ operation: "rename", display_name: "Requested name" }))
    .then(async (reply) => {
      check(
        JSON.parse(reply).value.user.display_name === "Canonical name",
        "rename returns the canonical profile",
      );
      check(
        JSON.stringify(calls) === JSON.stringify(["PATCH /v1/users/me", "GET /v1/users/me"]),
        "rename reads the canonical profile after the no-content update",
      );
      const saved = stored === null ? null : JSON.parse(stored);
      check(
        saved?.user.display_name === "Canonical name",
        "the canonical user replaces the cached user",
      );
      check(saved?.expires_at === expiresAt, "updating the cached user preserves session expiry");
      check(
        saved?.access_token === "a".repeat(64) && saved?.refresh_token === "b".repeat(64),
        "updating the cached user preserves both tokens",
      );
      const status = JSON.parse(await bridge.handle('{"operation":"status"}'));
      check(
        status.value.user.display_name === "Canonical name",
        "status immediately sees the updated cache",
      );
      const beforeInvalid = calls.length;
      const invalid = await bridge.handle(
        JSON.stringify({ operation: "rename", display_name: "😀".repeat(65) }),
      );
      check(
        JSON.parse(invalid).error === "account_invalid",
        "native rename enforces the 64-character limit",
      );
      check(calls.length === beforeInvalid, "an oversized nickname never reaches transport");
    });

  let lateStored: string | null = original;
  let resolveProfile: ((response: AccountTransportResponse) => void) | undefined;
  let markProfileStarted: (() => void) | undefined;
  const profileStarted = new Promise<void>((resolve) => {
    markProfileStarted = resolve;
  });
  const lateBridge = new AccountCloudBridge(
    {
      request: async (method) => {
        if (method === "PATCH") return { status: 204, body: "" };
        markProfileStarted?.();
        return await new Promise<AccountTransportResponse>((resolve) => {
          resolveProfile = resolve;
        });
      },
    },
    {
      load: () => lateStored,
      save: (value) => {
        lateStored = value;
      },
      clear: () => {
        lateStored = null;
      },
    },
  );
  const lateRename = lateBridge.handle(
    JSON.stringify({ operation: "rename", display_name: "Late name" }),
  );
  void profileStarted.then(async () => {
    await lateBridge.handle('{"operation":"clear_expired"}');
    resolveProfile?.({
      status: 200,
      body: JSON.stringify({
        user: { id: "synthetic-user", display_name: "Late name", created_at: "2026-01-01" },
        identities: [],
      }),
    });
  });
  void lateRename.then((reply) => {
    check(JSON.parse(reply).error === "account_cancelled", "logout rejects a late profile result");
    check(lateStored === null, "a late profile cannot restore cleared storage");
  });
});

group("a rejected account token refreshes and retries once", () => {
  let stored: string | null = JSON.stringify({
    access_token: "a".repeat(64),
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600000,
    user: { id: "synthetic-user", display_name: "Test", created_at: "2026-01-01" },
  });
  const tokens: string[] = [];
  let refreshCalls = 0;
  const bridge = new AccountCloudBridge(
    {
      request: async (_method, path, token) => {
        if (path === "/v1/auth/refresh") {
          refreshCalls += 1;
          return {
            status: 200,
            body: JSON.stringify({
              access_token: "c".repeat(64),
              refresh_token: "d".repeat(64),
              token_type: "Bearer",
              expires_in: 900,
              user: {
                id: "synthetic-user",
                display_name: "Test",
                created_at: "2026-01-01",
              },
            }),
          };
        }
        tokens.push(token ?? "");
        return token === "a".repeat(64)
          ? { status: 401, body: "private failure" }
          : {
              status: 200,
              body: '{"user":{"id":"synthetic-user","display_name":"Test","created_at":"2026-01-01"},"identities":[]}',
            };
      },
    },
    {
      load: () => stored,
      save: (value) => {
        stored = value;
      },
      clear: () => {
        stored = null;
      },
    },
  );
  void bridge.handle('{"operation":"profile"}').then((reply) => {
    check(JSON.parse(reply).ok === true, "one unauthorized response is retried after refresh");
    check(refreshCalls === 1, "a rejected access token rotates exactly once");
    check(
      JSON.stringify(tokens) === JSON.stringify(["a".repeat(64), "c".repeat(64)]),
      "the retry uses the new token and never repeats the rejected one",
    );
  });
});

/** A schema declaring everything this host maps, which is what a caught-up server would send. */
function fullPreferenceSchema(): AccountPreferenceSchema {
  const fields: Record<string, { type: string }> = {};
  const declare = (keys: string[], type: string) => {
    for (const key of keys) fields[key] = { type };
  };
  declare(
    [
      "input.schema",
      "input.character_set",
      "input.shuangpin_schema",
      "input.frequency_mode",
      "platform.harmony.keyboard_layout",
      "platform.harmony.keyboard_skin",
      "platform.harmony.custom_keyboard_skin",
      "platform.harmony.theme",
      "platform.harmony.candidate_skin",
      "platform.harmony.haptic_strength",
    ],
    "string",
  );
  declare(
    [
      "input.learning",
      "input.chinese_punctuation",
      "input.smart_punctuation",
      "input.paired_punctuation",
      "input.wubi_code_hint",
      "platform.harmony.voice_shortcut",
      "platform.harmony.sound_enabled",
      "platform.harmony.haptics_enabled",
    ],
    "boolean",
  );
  declare(
    [
      "input.frequency_trigger_count",
      "input.frequency_linear_step",
      "platform.harmony.touch_key_spacing_tenths",
      "platform.harmony.touch_row_spacing_tenths",
      "platform.harmony.keyboard_height_adjustment",
    ],
    "integer",
  );
  return { fields, maximumBytes: 64 * 1024, updateMode: "replace", revisionRequired: true };
}

const syncFeedback = { soundEnabled: true, hapticsEnabled: false, hapticStrength: "light" };

group("the account settings sync maps this host's document, not another's", () => {
  const local = {
    scheme: "shuangpin",
    traditional_chinese_output: true,
    shuangpin_profile: "ziranma",
    learning: false,
    frequency: { mode: "linear", trigger_count: 3, linear_step: 2 },
    chinese_punctuation: false,
    touch_keyboard_layout: "nine_key",
    touch_keyboard_skin: "midnight",
    candidate_skin: "wechat",
    touch_key_spacing_tenths: 40,
    custom_touch_keyboard_skin: { background: 1 },
  };
  const values = localAccountPreferences(local, syncFeedback);
  check(values["input.schema"] === "shuangpin", "the input schema travels");
  check(values["input.character_set"] === "traditional", "and the character set as a word");
  check(values["input.frequency_trigger_count"] === 3, "and the frequency numbers");
  check(values["platform.harmony.keyboard_skin"] === "midnight", "and the touch skin");
  // Not platform.android: the two are separate devices with separate keyboards, and sharing the
  // namespace would let a HarmonyOS phone overwrite the skin on the user's Android keyboard.
  check(
    Object.keys(values).every((key) => !key.startsWith("platform.android.")),
    "this host never writes another platform's keys",
  );
  check(
    values["platform.harmony.custom_keyboard_skin"] === JSON.stringify({ background: 1 }),
    "the custom design travels as one string, as the other hosts send it",
  );
  check(values["platform.harmony.haptic_strength"] === "light", "feedback comes from its own file");

  // A document written by an older build is missing the keys that build did not have. Refusing to
  // sync at all because of one absent field would help nobody.
  const sparse = localAccountPreferences({}, syncFeedback);
  check(sparse["input.schema"] === "quanpin", "an absent member takes the shared default");
  check(sparse["input.learning"] === true, "including the ones that default to on");
});

group("uploading keeps what other devices wrote", () => {
  const schema = fullPreferenceSchema();
  const base: AccountPreferences = {
    revision: 7,
    settings: { "platform.ios.keyboard_skin": "rose", "input.schema": "quanpin" },
  };
  const merged = mergeAccountPreferences(base, { "input.schema": "wubi" }, schema);
  check(merged.settings["input.schema"] === "wubi", "this host's value wins for its own key");
  check(
    merged.settings["platform.ios.keyboard_skin"] === "rose",
    "another platform's field is kept rather than cleared",
  );
  check(merged.revision === 7, "the revision is the one that was read");

  let refusedUnknown = false;
  try {
    mergeAccountPreferences(base, { "input.unknown_field": "x" }, schema);
  } catch (error) {
    refusedUnknown = error instanceof AccountPreferenceError && error.message === "account_invalid";
  }
  check(refusedUnknown, "writing a key the schema does not declare is refused, not dropped");

  let refusedType = false;
  try {
    mergeAccountPreferences(base, { "input.learning": "yes" }, schema);
  } catch (error) {
    refusedType = error instanceof AccountPreferenceError && error.message === "account_invalid";
  }
  check(refusedType, "and so is the right key with the wrong type");

  const photo = "A".repeat(4 * Math.floor((512000 + 2) / 3));
  const design = JSON.stringify({ photo });
  const designKey = "platform.harmony.custom_keyboard_skin";
  const photoSchema: AccountPreferenceSchema = {
    fields: { [designKey]: { type: "string" } },
    maximumBytes: 1024 * 1024,
    updateMode: "replace",
    revisionRequired: true,
  };
  const withPhoto = mergeAccountPreferences(
    { revision: 1, settings: {} },
    { [designKey]: design },
    photoSchema,
  );
  check(withPhoto.settings[designKey] === design, "a photo-sized private setting stays intact");
  let refusedLegacyLimit = false;
  try {
    mergeAccountPreferences(
      { revision: 1, settings: {} },
      { [designKey]: design },
      { ...photoSchema, maximumBytes: 64 * 1024 },
    );
  } catch (error) {
    refusedLegacyLimit =
      error instanceof AccountPreferenceError && error.message === "account_invalid";
  }
  check(refusedLegacyLimit, "the same photo is refused by an older negotiated 64 KiB limit");
});

group("applying writes only what the schema declares", () => {
  const schema = fullPreferenceSchema();
  const local = { scheme: "quanpin", learning: true, frequency: { mode: "promote" } };
  const cloud: AccountPreferences = {
    revision: 3,
    settings: {
      "input.schema": "wubi",
      "input.learning": false,
      "input.frequency_trigger_count": 5,
      "platform.harmony.candidate_skin": "graphite",
    },
  };
  const applied = applyAccountPreferences(local, cloud, schema, syncFeedback);
  check(applied.preferences.scheme === "wubi", "a declared string is written");
  check(applied.preferences.learning === false, "and a declared boolean");
  check(
    (applied.preferences.frequency as Record<string, unknown>).trigger_count === 5,
    "the frequency record is merged rather than replaced",
  );
  check(
    (applied.preferences.frequency as Record<string, unknown>).mode === "promote",
    "so a member the cloud said nothing about survives",
  );
  check(applied.preferences.candidate_skin === "graphite", "and the candidate skin is written");
  // Nothing in the cloud document mentioned the three feedback keys, so the file is left alone
  // rather than rewritten with whatever the defaults happen to be.
  check(applied.feedback === null, "an untouched feedback file is not rewritten");

  // A key the server has not declared yet does not travel and is not read. This is the state every
  // platform.harmony.* key is in until the service declares it, so it has to be the quiet case.
  const bare: AccountPreferenceSchema = {
    fields: { "input.schema": { type: "string" } },
    maximumBytes: 1024,
    updateMode: "replace",
    revisionRequired: true,
  };
  const partial = applyAccountPreferences(local, cloud, bare, syncFeedback);
  check(partial.preferences.scheme === "wubi", "the declared key is still applied");
  check(partial.preferences.learning === true, "while an undeclared one leaves the local value");

  let refusedValue = false;
  try {
    applyAccountPreferences(
      local,
      { revision: 1, settings: { "input.schema": "esperanto" } },
      schema,
      syncFeedback,
    );
  } catch (error) {
    refusedValue = error instanceof AccountPreferenceError && error.message === "account_invalid";
  }
  check(refusedValue, "a declared key carrying a value this host has no meaning for is refused");

  let refusedMismatch = false;
  try {
    applyAccountPreferences(
      local,
      { revision: 1, settings: { "input.learning": 1 } },
      schema,
      syncFeedback,
    );
  } catch (error) {
    refusedMismatch =
      error instanceof AccountPreferenceError && error.message === "account_invalid";
  }
  check(refusedMismatch, "and a declared key arriving with the wrong type is refused, not ignored");

  const withFeedback = applyAccountPreferences(
    local,
    { revision: 1, settings: { "platform.harmony.sound_enabled": false } },
    schema,
    syncFeedback,
  );
  check(withFeedback.feedback?.soundEnabled === false, "a feedback key is written");
  check(
    withFeedback.feedback?.hapticStrength === "light",
    "and the members it did not mention keep their local values",
  );
});

/** A runner whose answers are scripted, so the rules between the steps can be exercised alone. */
function aiSkinRunner(overrides: Partial<AiSkinRunner> = {}): {
  runner: AiSkinRunner;
  created: string[];
  deleted: string[];
} {
  const created: string[] = [];
  const deleted: string[] = [];
  const jobId = (index: number) => `${index}`.repeat(48).slice(0, 48);
  let next = 1;
  const plan = (suffix: string) => ({
    name: `晨雾${suffix}`,
    description: "说明",
    artworkPrompt: `场景${suffix}`,
    design: {},
  });
  const runner: AiSkinRunner = {
    defaultModel: async () => "fast",
    chat: async () => "{}",
    plans: () => [plan("甲"), plan("乙"), plan("丙")],
    createJob: async (artworkPrompt: string) => {
      created.push(artworkPrompt);
      const id = jobId(next++);
      return { id, state: "succeeded", artwork: { b64_json: "x" } } as ArtworkJob;
    },
    readJob: async (id: string) => ({ id, state: "succeeded", artwork: { b64_json: "x" } }),
    deleteJob: async (id: string) => {
      deleted.push(id);
    },
    validateArtwork: () => true,
    wait: async () => {},
    now: () => 0,
    ...overrides,
  };
  return { runner, created, deleted };
}

group("an AI skin run releases what it started, whichever way it ends", () => {
  const { runner, created, deleted } = aiSkinRunner();
  const run = new AiSkinRun(runner);
  const ticks: number[] = [];
  void run
    .generate("晨雾里的竹林", (completed) => ticks.push(completed))
    .then((proposals) => {
      check(proposals.length === 3, "three illustrated designs come back");
      check(created.length === 3, "one artwork job per plan");
      // Every job is released. The user's account is what an abandoned upstream task is charged to,
      // and nothing left on this device would ever go back to stop it.
      check(deleted.length === 3, "and every one of them is released");
      // Counted in finished pictures rather than as a fraction of the whole run: the catalog and the
      // chat are quick and the pictures are not, so a percentage would sit still and say nothing.
      check(
        ticks.length === 3 && Math.max(...ticks) === 3,
        "progress counts finished pictures, up to three",
      );
    });

  // A run that fails partway must still release the job it had already created, and must not leave
  // its siblings generating pictures for a set nobody will see.
  const failingCreated: string[] = [];
  const failing = aiSkinRunner({
    createJob: async (artworkPrompt: string) => {
      if (artworkPrompt === "场景乙") throw new AiSkinFailure("ai_skin_unavailable");
      failingCreated.push(artworkPrompt);
      return {
        id: "a".repeat(48),
        state: "succeeded",
        artwork: { b64_json: "x" },
      } as ArtworkJob;
    },
  });
  const failed = new AiSkinRun(failing.runner);
  void failed
    .generate("晨雾", () => {})
    .then(() => check(false, "a failing job must not resolve"))
    .catch((error) => {
      check(error instanceof AiSkinFailure, "the failure is reported, not swallowed");
      check(failed.cancelled, "and the siblings are stopped");
      check(
        failing.deleted.length === failingCreated.length && failingCreated.length > 0,
        "while the jobs that were created are still released",
      );
    });

  // Cancelling before anything starts costs nothing, and has to be recognisable as a cancellation
  // rather than as a failure — the page says different things about the two.
  const idle = aiSkinRunner();
  const cancelled = new AiSkinRun(idle.runner);
  cancelled.cancel();
  void cancelled
    .generate("晨雾", () => {})
    .then(() => check(false, "a cancelled run must not resolve"))
    .catch((error) => {
      check(error instanceof AiSkinCancelled, "cancelling is not a failure");
      check(idle.created.length === 0, "and nothing was requested");
    });

  // Cancelling while the service is answering a poll must not wait for that HTTP request to time
  // out before releasing its jobs. The requests may still complete in the system stack, but no
  // caller waits on them and the upstream work is deleted immediately.
  let inFlightSequence = 0;
  let inFlightReads = 0;
  const interruptedFixture = aiSkinRunner({
    createJob: async () => ({
      id: `${++inFlightSequence}`.repeat(48),
      state: "running",
    }),
    wait: async () => {},
    readJob: async () => {
      inFlightReads += 1;
      if (inFlightReads === 3) interrupted.cancel();
      return await new Promise<ArtworkJob>(() => {});
    },
  });
  const interrupted = new AiSkinRun(interruptedFixture.runner);
  void interrupted
    .generate("晨雾", () => {})
    .then(() => check(false, "an interrupted poll must not resolve"))
    .catch((error) => {
      check(error instanceof AiSkinCancelled, "an in-flight poll observes cancellation");
      check(inFlightReads === 3, "all three artwork jobs had reached their poll");
      check(interruptedFixture.deleted.length === 3, "cancellation releases every in-flight job");
    });

  // One failed picture is a generation failure, not a user cancellation, but it still interrupts
  // the two polls that otherwise have no reason to finish.
  let siblingSequence = 3;
  const siblingReads: { id: string; resolve: (job: ArtworkJob) => void }[] = [];
  const siblingFixture = aiSkinRunner({
    createJob: async () => ({
      id: `${++siblingSequence}`.repeat(48),
      state: "running",
    }),
    wait: async () => {},
    readJob: async (id: string) =>
      await new Promise<ArtworkJob>((resolve) => {
        siblingReads.push({ id, resolve });
        if (siblingReads.length === 3) {
          siblingReads[1].resolve({ id: siblingReads[1].id, state: "failed" });
        }
      }),
  });
  const siblingFailure = new AiSkinRun(siblingFixture.runner);
  void siblingFailure
    .generate("晨雾", () => {})
    .then(() => check(false, "a failed sibling must not resolve"))
    .catch((error) => {
      check(
        error instanceof AiSkinFailure && error.message === "ai_skin_unavailable",
        "the real artwork failure is preserved after sibling cancellation",
      );
      check(siblingFixture.deleted.length === 3, "a failed picture releases its stalled siblings");
    });

  // A job the service never finishes is given the shared 200 seconds and then abandoned, rather
  // than polled until the page is closed.
  let clock = 0;
  const slow = aiSkinRunner({
    createJob: async () => ({ id: "b".repeat(48), state: "running" }) as ArtworkJob,
    readJob: async (id: string) => ({ id, state: "running" }),
    wait: async () => {
      clock += 5000;
    },
    now: () => clock,
  });
  const stalled = new AiSkinRun(slow.runner);
  void stalled
    .generate("晨雾", () => {})
    .then(() => check(false, "a stalled run must not resolve"))
    .catch((error) => {
      check(
        error instanceof AiSkinFailure && error.message === "ai_skin_unavailable",
        "a job that never finishes is abandoned",
      );
      check(slow.deleted.length > 0, "and released on the way out");
    });

  // The job id goes into a path, so its shape is checked before it does.
  const forged = aiSkinRunner({
    createJob: async () => ({ id: "../../users/me", state: "succeeded" }) as ArtworkJob,
  });
  void new AiSkinRun(forged.runner)
    .generate("晨雾", () => {})
    .then(() => check(false, "a forged job id must not resolve"))
    .catch((error) => {
      check(
        error instanceof AiSkinFailure && error.message === "ai_skin_response",
        "a job id that is not 48 hex characters never reaches a path",
      );
    });

  // A picture the shared client will not show is a failed proposal, not one drawn with a blank.
  const unusable = aiSkinRunner({ validateArtwork: () => false });
  void new AiSkinRun(unusable.runner)
    .generate("晨雾", () => {})
    .then(() => check(false, "an unusable picture must not resolve"))
    .catch((error) => {
      check(
        error instanceof AiSkinFailure && error.message === "ai_skin_response",
        "an artwork the shared client refuses fails the proposal",
      );
      check(unusable.deleted.length > 0, "and its job is still released");
    });
});

group("shared dictionaries and reply templates keep their own bounds", () => {
  let stored: string | null = null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const calls: { method: string; path: string; token?: string; body?: Record<string, unknown> }[] =
    [];
  const transport: AccountTransport = {
    request: async (method, path, token, body) => {
      calls.push({ method, path, token, body });
      if (path === "/v1/auth/login")
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "u1", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      if (path.includes("/dictionaries/quick/catalog"))
        return { status: 200, body: '{"revision":12}' };
      return { status: 200, body: '{"items":[],"has_more":false}' };
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  const resources = (action: Record<string, unknown>) =>
    bridge.handle(JSON.stringify({ operation: "community_resource", ...action }));
  const id = "10000000-0000-4000-8000-000000000001";

  void resources({
    resource_operation: "list",
    kind: "dictionary",
    scope: "",
    search: "",
    offset: 0,
  }).then((result) => {
    check(JSON.parse(result).ok === true, "the public list reads without an account");
  });
  // 我的作品 and 收藏 are questions about an account. Without one the answer is either empty or
  // somebody else's, so the host refuses rather than asking.
  void resources({
    resource_operation: "list",
    kind: "reply",
    scope: "mine",
    search: "",
    offset: 0,
  }).then((result) => {
    check(JSON.parse(result).error === "community_unauthorized", "while a scoped list needs one");
  });
  void resources({
    resource_operation: "list",
    kind: "song",
    scope: "",
    search: "",
    offset: 0,
  }).then((result) => {
    check(JSON.parse(result).error === "community_invalid", "an unknown kind is refused");
  });

  void bridge
    .handle('{"operation":"login","challenge_id":"challenge","credential":"123456"}')
    .then(() => {
      // Applying is two requests: the server has to be told which revision of the user's own
      // dictionary this is merging into, so the read's answer goes into the write.
      void resources({ resource_operation: "apply", id, resource_revision: 3 }).then((result) => {
        check(JSON.parse(result).ok === true, "applying a shared dictionary is accepted");
        const applied = calls.find((call) => call.path.endsWith("/apply"));
        check(
          applied?.body?.dictionary_revision === 12,
          "and it carries the revision the catalog just reported",
        );
        check(applied?.body?.resource_revision === 3, "together with the resource revision");
      });

      // A reply is a prompt and nothing else; a dictionary is entries and no prompt. The shared
      // service refuses the other combinations rather than ignoring the extra half, because a
      // "dictionary" carrying a prompt is a resource whose author believed it was something else.
      void resources({
        resource_operation: "publish",
        id,
        kind: "reply",
        name: "高情商",
        description: "",
        content: { prompt: "换个说法" },
        revision: 1,
      }).then((result) => {
        check(JSON.parse(result).ok === true, "a reply carrying only a prompt publishes");
      });
      void resources({
        resource_operation: "publish",
        id,
        kind: "reply",
        name: "高情商",
        description: "",
        content: {
          prompt: "换个说法",
          entries: [{ kind: "pinyin", code: "ni", word: "你", weight: 1 }],
        },
        revision: 1,
      }).then((result) => {
        check(
          JSON.parse(result).error === "community_invalid",
          "a reply carrying dictionary entries does not",
        );
      });
      void resources({
        resource_operation: "publish",
        id,
        kind: "dictionary",
        name: "词库",
        description: "",
        content: { entries: [] },
        revision: 1,
      }).then((result) => {
        check(JSON.parse(result).error === "community_invalid", "nor an empty dictionary");
      });
      void resources({
        resource_operation: "publish",
        id,
        kind: "dictionary",
        name: "词库",
        description: "",
        content: {
          entries: [
            { kind: "pinyin", code: "ni", word: "你", weight: 1 },
            { kind: "pinyin", code: "ni", word: "你", weight: 5 },
          ],
        },
        revision: 1,
      }).then((result) => {
        check(
          JSON.parse(result).error === "community_invalid",
          "nor one that lists the same word twice",
        );
      });
      void resources({
        resource_operation: "publish",
        id,
        kind: "dictionary",
        name: "词库",
        description: "",
        content: {
          entries: Array.from({ length: 129 }, (_, index) => ({
            kind: "pinyin",
            code: `code${index}`,
            word: `词${index}`,
            weight: 1,
          })),
        },
        revision: 1,
      }).then((result) => {
        check(JSON.parse(result).error === "community_invalid", "nor one past 128 entries");
      });

      // 128 entries of long words is more than the 64 KB the action envelope used to allow, and
      // the service accepts 350,000 bytes of content. A smaller envelope would have refused here
      // what the server would have taken.
      void resources({
        resource_operation: "publish",
        id,
        kind: "dictionary",
        name: "词库",
        description: "",
        content: {
          entries: Array.from({ length: 128 }, (_, index) => ({
            kind: "pinyin",
            code: `code${index}`,
            word: "词".repeat(1000),
            weight: 1,
          })),
        },
        revision: 1,
      }).then((result) => {
        check(JSON.parse(result).ok === true, "a full-size dictionary fits through the envelope");
      });
    });
});

group("the skin gallery is public to browse and signed in to change", () => {
  let stored: string | null = null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const calls: { method: string; path: string; token?: string }[] = [];
  let status = 200;
  const transport: AccountTransport = {
    request: async (method, path, token) => {
      calls.push({ method, path, token });
      if (path === "/v1/auth/login")
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "u1", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      return { status, body: '{"skins":[],"has_more":false}' };
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  const gallery = (action: Record<string, unknown>) =>
    bridge.handle(JSON.stringify({ operation: "community_skin", ...action }));
  const id = "10000000-0000-4000-8000-000000000001";

  // Browsing signed out is the point: someone who cannot see the gallery has no way to decide
  // whether an account is worth making.
  void gallery({ community_operation: "list", offset: 0, search: "" }).then((result) => {
    check(JSON.parse(result).ok === true, "the gallery is readable without an account");
    const listed = calls.find((call) => call.path.startsWith("/v1/community/skins?"));
    check(listed?.token === undefined, "and that request carries no token");
  });
  void gallery({ community_operation: "download", id }).then((result) => {
    check(
      JSON.parse(result).error === "community_unauthorized",
      "but downloading needs an account",
    );
  });
  void gallery({ community_operation: "rate", id, stars: 5 }).then((result) => {
    check(JSON.parse(result).error === "community_unauthorized", "and so does rating");
  });

  // An id goes into the URL path. Interpolating whatever the page sent would let a page turn a
  // skin id into a different endpoint, so the shape is the check.
  void gallery({ community_operation: "detail", id: "../../users/me" }).then((result) => {
    check(
      JSON.parse(result).error === "community_invalid",
      "an id that is not a uuid never reaches a path",
    );
  });
  void gallery({ community_operation: "rate", id, stars: 9 }).then((result) => {
    check(JSON.parse(result).error === "community_invalid", "a rating outside 1..5 is refused");
  });
  void gallery({ community_operation: "list", offset: 0, search: "x".repeat(129) }).then(
    (result) => {
      check(JSON.parse(result).error === "community_invalid", "and an overlong search");
    },
  );
  void gallery({ community_operation: "unknown", id }).then((result) => {
    check(JSON.parse(result).error === "community_invalid", "and an unknown operation is named");
  });

  void bridge
    .handle('{"operation":"login","challenge_id":"challenge","credential":"123456"}')
    .then(() => {
      void gallery({ community_operation: "list", offset: 0, search: "森林" }).then(() => {
        const listed = calls.filter((call) => call.path.startsWith("/v1/community/skins?")).pop();
        check(listed?.token !== undefined, "a signed-in browse carries the session");
        check(
          listed?.path.includes(encodeURIComponent("森林")) === true,
          "and the search is encoded rather than pasted into the URL",
        );
      });
      void gallery({
        community_operation: "publish",
        id,
        name: "  晨雾  ",
        description: "",
        design: {},
      }).then((result) => {
        check(
          JSON.parse(result).error === "community_invalid",
          "an untrimmed name is refused the way the shared service refuses it",
        );
      });
      void gallery({
        community_operation: "publish",
        id,
        name: "晨雾",
        description: "第一行\n第二行",
        design: {},
      }).then((result) => {
        // A description is prose and may be written in paragraphs; a name may not.
        check(JSON.parse(result).ok === true, "a multi-line description is allowed");
      });
      void gallery({
        community_operation: "publish",
        id,
        name: "晨\n雾",
        description: "",
        design: {},
      }).then((result) => {
        check(JSON.parse(result).error === "community_invalid", "while a multi-line name is not");
      });

      // The community pages decode their own vocabulary; an account_* code would arrive as the
      // one generic sentence instead of "已达到发布上限".
      status = 409;
      void gallery({ community_operation: "detail", id }).then((result) => {
        check(
          JSON.parse(result).error === "community_conflict",
          "a refusal carries the community code, not the account one",
        );
        status = 403;
        void gallery({ community_operation: "detail", id }).then((forbidden) => {
          check(JSON.parse(forbidden).error === "community_forbidden", "and so does a forbidden");
          status = 200;
        });
      });
    });
});

group("the account assistant answers with a model list and one reply", () => {
  let stored: string | null = null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const calls: {
    method: string;
    path: string;
    body?: Record<string, unknown>;
    timeoutMs?: number;
  }[] = [];
  let models = '{"data":[{"id":"fast"},{"id":"careful"}],"default_model":"careful"}';
  let completion = JSON.stringify({
    choices: [{ message: { role: "assistant", content: "第一行\n第二行" } }],
  });
  const transport: AccountTransport = {
    request: async (method, path, _token, body, timeoutMs) => {
      calls.push({ method, path, body, timeoutMs });
      if (path === "/v1/auth/login")
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "u1", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      if (path === "/v1/models") return { status: 200, body: models };
      if (path === "/v1/chat/completions") return { status: 200, body: completion };
      return { status: 404, body: "{}" };
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  const ask = (action: Record<string, unknown>) =>
    bridge.handle(JSON.stringify({ operation: "chat", ...action }));

  void ask({ chat_operation: "models" }).then((result) => {
    check(
      JSON.parse(result).error === "account_unauthorized",
      "chat needs the native session, not a token from the page",
    );
  });
  void bridge
    .handle('{"operation":"login","challenge_id":"challenge","credential":"123456"}')
    .then(() => {
      void ask({ chat_operation: "models" }).then((result) => {
        const reply = JSON.parse(result);
        check(reply.ok === true, "the model catalog is accepted");
        // The page's ChatModels is camelCase; the service answers in snake_case. Reshaping here is
        // what keeps every host's page reading one field name.
        check(reply.value.defaultModel === "careful", "the default model is named for the page");
        check(
          reply.value.data.length === 2 && reply.value.data[0].id === "fast",
          "and the catalog keeps its order",
        );
      });
      void ask({
        chat_operation: "complete",
        model: "careful",
        messages: [{ role: "user", content: "你好" }],
      }).then((result) => {
        const reply = JSON.parse(result);
        check(reply.ok === true, "a completion is accepted");
        // A conversation is written in paragraphs. The bridge's other validators reject control
        // characters, which would have refused every multi-line answer the assistant gives.
        check(reply.value.content === "第一行\n第二行", "newlines in a reply are content");
        const sent = calls.find((call) => call.path === "/v1/chat/completions");
        check(sent?.body?.max_tokens === 2048, "the shared request shape is sent");
        check(sent?.body?.stream === false, "and it does not ask for a stream");
        check(
          typeof sent?.timeoutMs === "number" && sent.timeoutMs === 125000,
          "a model writing text gets the shared 125-second budget",
        );
        const catalog = calls.find((call) => call.path === "/v1/models");
        check(
          catalog?.timeoutMs === undefined,
          "while a catalog lookup keeps the ordinary timeout",
        );
      });
      void ask({
        chat_operation: "complete",
        model: "careful",
        messages: [{ role: "narrator", content: "旁白" }],
      }).then((result) => {
        check(
          JSON.parse(result).error === "account_invalid",
          "an unknown role never reaches transport",
        );
      });
      void ask({
        chat_operation: "complete",
        model: "careful",
        messages: Array.from({ length: 17 }, () => ({ role: "user", content: "x" })),
      }).then((result) => {
        check(
          JSON.parse(result).error === "account_invalid",
          "and neither does a history past the shared limit",
        );
      });
      void ask({ chat_operation: "complete", model: "careful", messages: [] }).then((result) => {
        check(JSON.parse(result).error === "account_invalid", "nor an empty conversation");
      });
      void ask({ chat_operation: "unknown" }).then((result) => {
        check(JSON.parse(result).error === "account_invalid", "an unknown chat operation is named");
      });
    });

  // The two shapes the service could send back that must not become a visible reply: the refusal
  // has to be distinguishable from an answer, or the page shows an empty bubble and no error.
  void bridge
    .handle('{"operation":"login","challenge_id":"challenge","credential":"123456"}')
    .then(() => {
      completion = JSON.stringify({ choices: [{ message: { role: "user", content: "回声" } }] });
      void ask({
        chat_operation: "complete",
        model: "careful",
        messages: [{ role: "user", content: "你好" }],
      }).then((result) => {
        check(
          JSON.parse(result).error === "account_unavailable",
          "a reply that is not from the assistant is refused",
        );
        models = '{"data":[{"id":"fast"}],"default_model":"missing"}';
        void ask({ chat_operation: "models" }).then((catalog) => {
          check(
            JSON.parse(catalog).error === "account_unavailable",
            "and a default model absent from the catalog is refused",
          );
        });
      });
    });
});

group("a device's own buttons are not a keyboard", () => {
  // Every phone enumerates a keyboard source for volume and power. Only the type separates them,
  // which is the whole reason this decision is not `sources.includes("keyboard")`.
  const phoneButtons = [{ deviceId: 1, keyboardType: AttachedKeyboardType.DIGITAL }];
  check(
    HardwareKeyboardPolicy.routes(false, phoneButtons) === false,
    "a keypad does not make a phone route keys",
  );
  check(
    HardwareKeyboardPolicy.routes(false, [
      { deviceId: 2, keyboardType: AttachedKeyboardType.HANDWRITING_PEN },
      { deviceId: 3, keyboardType: AttachedKeyboardType.REMOTE_CONTROL },
      { deviceId: 4, keyboardType: AttachedKeyboardType.UNKNOWN },
    ]) === false,
    "a stylus, a remote and an unknown device cannot type pinyin",
  );
  check(
    HardwareKeyboardPolicy.routes(false, [
      ...phoneButtons,
      { deviceId: 5, keyboardType: AttachedKeyboardType.ALPHABETIC },
    ]) === true,
    "one attached alphabetic keyboard is enough",
  );
});

group("a desktop routes keys whatever the enumeration says", () => {
  // A 2in1 draws no keys of its own, so an enumeration that fails or comes back empty must not be
  // allowed to leave it inert: there would be no other way to reach the Engine.
  check(HardwareKeyboardPolicy.routes(true, []) === true, "an empty list does not disarm a 2in1");
  check(HardwareKeyboardPolicy.routes(true, null) === true, "nor does a failed query");
  check(
    HardwareKeyboardPolicy.routes(false, null) === false,
    "a phone with no answer draws its own keys and routes none",
  );
});

group("keyboards coming and going", () => {
  const attached = HardwareKeyboardPolicy.applyChange([], {
    type: "add",
    deviceId: 7,
    keyboardType: AttachedKeyboardType.ALPHABETIC,
  });
  check(HardwareKeyboardPolicy.routes(false, attached) === true, "plugging one in starts routing");
  const again = HardwareKeyboardPolicy.applyChange(attached, {
    type: "add",
    deviceId: 7,
    keyboardType: AttachedKeyboardType.ALPHABETIC,
  });
  check(again.length === 1, "the same device announced twice is still one device");
  // A keyboard unplugged mid-composition is gone before its key-up arrives, so removal cannot ask
  // the service what type it was.
  const removed = HardwareKeyboardPolicy.applyChange(again, {
    type: "remove",
    deviceId: 7,
    keyboardType: AttachedKeyboardType.NONE,
  });
  check(removed.length === 0, "removal matches on the id alone");
  check(
    HardwareKeyboardPolicy.routes(false, removed) === false,
    "unplugging the last keyboard stops routing",
  );
});

group("the subscription changes only when it has to", () => {
  // A second on('keyEvent') is not idempotent: it would deliver one keystroke as two characters.
  check(
    HardwareKeyboardPolicy.transition(true, true) === null,
    "an already-routing keyboard is not subscribed twice",
  );
  check(
    HardwareKeyboardPolicy.transition(false, false) === null,
    "nor is an idle one unsubscribed twice",
  );
  check(
    HardwareKeyboardPolicy.transition(false, true) === KeyRoutingTransition.SUBSCRIBE,
    "attaching subscribes",
  );
  check(
    HardwareKeyboardPolicy.transition(true, false) === KeyRoutingTransition.UNSUBSCRIBE,
    "detaching unsubscribes",
  );
});

group("a malformed enumeration is not trusted", () => {
  check(
    HardwareKeyboardPolicy.alphabetic([
      null as unknown as { deviceId: number; keyboardType: AttachedKeyboardType },
      { deviceId: -1, keyboardType: AttachedKeyboardType.ALPHABETIC },
      { deviceId: 1.5, keyboardType: AttachedKeyboardType.ALPHABETIC },
    ]).length === 0,
    "a null entry and unusable ids are skipped rather than routed",
  );
  check(
    HardwareKeyboardPolicy.applyChange(null, null).length === 0,
    "no devices and no change is no devices",
  );
});

group("setup has two steps and they fail separately", () => {
  const own = "app.msime.client";
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.DISABLED,
      currentBundle: "",
      ownBundle: own,
    }) === true,
    "a keyboard nobody enabled opens the welcome flow",
  );
  // Enabled but not selected is where someone lands after doing half the setup, and it looks
  // exactly like a working install until they try to type.
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
      currentBundle: "com.example.other",
      ownBundle: own,
    }) === true,
    "enabled but not current still has a step left",
  );
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
      currentBundle: own,
      ownBundle: own,
    }) === false,
    "enabled and current goes straight to settings",
  );
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.BASIC_MODE,
      currentBundle: own,
      ownBundle: own,
    }) === false,
    "basic mode is enabled too",
  );
});

group("an unanswerable setup query does not send anyone back to a welcome screen", () => {
  // Someone who has typed with this keyboard for weeks must not meet a welcome screen because one
  // system call failed. Settings is reachable from the flow; the flow is not reachable from
  // settings, so the safe direction is settings.
  check(
    OnboardingStatePolicy.required({
      enabled: null,
      currentBundle: "",
      ownBundle: "app.msime.client",
    }) === false,
    "an unreadable enablement state opens settings",
  );
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
      currentBundle: "",
      ownBundle: "app.msime.client",
    }) === false,
    "an unreadable current keyboard opens settings",
  );
  // An unreadable own name would otherwise compare unequal to every keyboard, including this one.
  check(
    OnboardingStatePolicy.required({
      enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
      currentBundle: "app.msime.client",
      ownBundle: "",
    }) === false,
    "an unreadable own bundle opens settings",
  );
  check(
    OnboardingStatePolicy.describe({
      enabled: ImeEnabledState.FULL_EXPERIENCE_MODE,
      currentBundle: "com.example.other",
      ownBundle: "app.msime.client",
    }) === "enabled but not current, opening welcome flow",
    "the reason the window opened where it did is recorded, not inferred afterwards",
  );
});

group("each source candidate skin keeps its own colours", () => {
  const ids = ["fluent", "wechat", "graphite", "willow_green"];
  const resolved = ids.map((id) => CandidateSkinPolicy.harmonySkin(id));
  check(
    new Set(resolved).size === ids.length,
    "four source skins resolve to four palettes, not two",
  );
  // 微信绿 and 杨柳青 both used to land on `forest`, which made them pixel-identical. #07c160 and
  // #58b980 are not close; one of the four skins was effectively missing.
  const accents = ids.map(
    (id) => KeyboardSkin.from(CandidateSkinPolicy.harmonySkin(id), false).accent,
  );
  check(new Set(accents).size === ids.length, "and to four different highlight colours");
  check(
    KeyboardSkin.from("wechat", false).accent.toLowerCase() === "#07c160",
    "微信绿 keeps the upstream WeChat green",
  );
  check(
    KeyboardSkin.from("willow_green", false).accent.toLowerCase() === "#58b980",
    "杨柳青 keeps its own, lighter green",
  );
  check(
    KeyboardSkin.from("fluent", false).accent.toLowerCase() === "#6b69d6",
    "Fluent keeps the upstream indigo",
  );
});

group("candidate palettes follow the upstream dark stylesheets", () => {
  // Taken from horizontal_dark.css per skin, not derived by darkening the light values: the source
  // picks a different hue for some skins and a mechanical transform would drift from it.
  check(
    KeyboardSkin.from("graphite", true).accent.toLowerCase() === "#8993a0",
    "graphite lightens its slate in dark mode",
  );
  check(
    KeyboardSkin.from("willow_green", true).accent.toLowerCase() === "#65c98d",
    "杨柳青 lightens its green in dark mode",
  );
  check(
    KeyboardSkin.from("wechat", true).accent.toLowerCase() === "#07c160",
    "微信绿 keeps the same green in both, as upstream does",
  );
  check(
    KeyboardSkin.from("fluent", true).keyForeground.toLowerCase() === "#e9e8e8",
    "dark text comes from the dark stylesheet",
  );
});

group("candidate skins are not offered as touch-keyboard skins", () => {
  // Two different preferences. A candidate palette in the keyboard picker would offer the user a
  // keyboard skin built from a candidate window's colours, which is not a skin anyone designed.
  for (const id of ["fluent", "wechat", "graphite", "willow_green"]) {
    check(!KeyboardSkin.BUILT_IN_IDS.includes(id), `${id} stays out of the touch-keyboard picker`);
  }
});

group("the panel chord opens the screen keyboard", () => {
  const chord = (over: Record<string, unknown> = {}) => ({
    keyCode: 2027,
    down: true,
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    logoKey: true,
    ...over,
  });
  check(
    PanelShortcutPolicy.shortcut(chord()) === PanelShortcut.SCREEN_KEYBOARD,
    "Ctrl+Shift+Super+K asks for the screen keyboard, as Windows binds it",
  );
  check(
    PanelShortcutPolicy.shortcut(chord(), false) === PanelShortcut.NONE,
    "a phone does not expose the 2-in-1 screen-keyboard shortcut",
  );
  // A chord that fires on a superset would swallow a combination the editor was meant to receive.
  check(
    PanelShortcutPolicy.shortcut(chord({ altKey: true })) === PanelShortcut.NONE,
    "adding Alt makes it a different chord, not this one",
  );
  for (const missing of ["ctrlKey", "shiftKey", "logoKey"]) {
    check(
      PanelShortcutPolicy.shortcut(chord({ [missing]: false })) === PanelShortcut.NONE,
      `${missing} is required, not merely allowed`,
    );
  }
  check(
    PanelShortcutPolicy.shortcut(chord({ keyCode: 2021 })) === PanelShortcut.NONE,
    "another letter with the same modifiers is not the panel chord",
  );
});

group("the panel chord is claimed on release as well as press", () => {
  const release = {
    keyCode: 2027,
    down: false,
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    logoKey: true,
  };
  // Acting twice would toggle the panel straight back shut; letting the release through would put
  // a bare K in the editor after the panel had already opened.
  check(
    PanelShortcutPolicy.shortcut(release) === PanelShortcut.NONE,
    "the release does not open the panel a second time",
  );
  check(PanelShortcutPolicy.claims(release), "but it is still claimed, so no stray K is typed");
  check(
    !PanelShortcutPolicy.claims(release, false),
    "a phone leaves the same physical chord untouched",
  );
  check(
    !PanelShortcutPolicy.claims({ ...release, logoKey: false }),
    "a key that is not part of the chord is left to the editor",
  );
});

group(
  "a hardware letter takes its case from shift and caps lock, not from the resolved character",
  () => {
    const key = (over: Record<string, unknown> = {}) => ({
      keyCode: 2030,
      unicodeChar: 0x4e,
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      logoKey: false,
      ...over,
    });
    // A 2in1 resolves a bare N as 'N'. The Engine reads a capital as a help code, so the first letter on a hardware keyboard never started a composition.
    check(
      HardwareKeyRouter.normalizeLetterCase(key(), false).unicodeChar === 0x6e,
      "a bare letter is lower case",
    );
    check(
      HardwareKeyRouter.normalizeLetterCase(key({ unicodeChar: 0x6e, shiftKey: true }), false)
        .unicodeChar === 0x4e,
      "shift makes it a capital",
    );
    check(
      HardwareKeyRouter.normalizeLetterCase(key(), true).unicodeChar === 0x4e,
      "so does caps lock",
    );
    check(
      HardwareKeyRouter.normalizeLetterCase(key({ shiftKey: true }), true).unicodeChar === 0x6e,
      "and shift with caps lock cancels out",
    );
    const digit = HardwareKeyRouter.normalizeLetterCase(
      key({ keyCode: 2001, unicodeChar: 0x31 }),
      true,
    );
    check(digit.unicodeChar === 0x31, "a digit is left alone");
    check(
      HardwareKeyRouter.normalizeLetterCase(key({ unicodeChar: 0x40, shiftKey: true }), false)
        .unicodeChar === 0x40,
      "so is the mark just below the capitals",
    );
    check(
      HardwareKeyRouter.normalizeLetterCase(key({ unicodeChar: 0x5b }), true).unicodeChar === 0x5b,
      "and the one just above them",
    );
  },
);

group("Japanese Space commits a lone Fallback row instead of converting it", () => {
  check(
    !JapaneseSpacePolicy.converts(1, JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK),
    "the raw composition alone is not something to convert",
  );
  check(JapaneseSpacePolicy.converts(1, 0), "a lone real candidate still converts");
  check(
    JapaneseSpacePolicy.converts(2, JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK),
    "several rows still convert even when the first is Fallback",
  );
  check(!JapaneseSpacePolicy.converts(0, -1), "no candidates leaves Space to its normal meaning");
  const log: string[] = [];
  const declining: HardwareKeyTarget = {
    ...recordingTarget(log),
    convertJapanese: () => {
      log.push("convertJapanese");
      return false;
    },
  };
  HardwareKeyDispatch.apply(
    { action: HardwareKeyAction.JAPANESE_CONVERT, character: 0, index: 0 },
    false,
    declining,
  );
  check(
    log.join(",") === "convertJapanese,commitHighlighted",
    "hardware Space the conversion declines commits the highlighted row",
  );
  check(
    dispatched(HardwareKeyAction.JAPANESE_CONVERT).join(",") === "convertJapanese",
    "and Space the conversion claims commits nothing",
  );
});

group("a letter the Engine declines is handed back rather than swallowed", () => {
  const declining: HardwareKeyTarget = { ...recordingTarget([]), press: () => false };
  check(
    !HardwareKeyDispatch.apply(
      { action: HardwareKeyAction.COMPOSE, character: 0x4e, index: 0 },
      true,
      declining,
    ),
    "a declined letter reports that it was not consumed",
  );
  check(
    HardwareKeyDispatch.apply(
      { action: HardwareKeyAction.COMPOSE, character: 0x6e, index: 0 },
      false,
      recordingTarget([]),
    ),
    "an accepted one reports that it was",
  );
  check(
    HardwareKeyDispatch.apply(
      { action: HardwareKeyAction.IGNORED, character: 0, index: 0 },
      false,
      recordingTarget([]),
    ),
    "a deliberately ignored key is still consumed",
  );
});

group("fullwidth mode widens what a hardware keyboard would hand the application", () => {
  // Windows eats every printable ASCII key while the double-byte mode is on and no candidate list is open, and inserts its fullwidth form (`KeyEventSink.cpp`, `IsDoubleSingleByte`). The 2in1 released those keys, so the application typed them halfwidth.
  const key = (over: Record<string, unknown> = {}): HardwareKey => ({
    keyCode: 2017,
    unicodeChar: 0x61,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    logoKey: false,
    ...over,
  });
  const route = (hardware: HardwareKey, composing: boolean, chinese: boolean, fullWidth: boolean) =>
    HardwareKeyRouter.route(
      hardware,
      composing,
      chinese,
      false,
      undefined,
      false,
      false,
      "disabled",
      false,
      PLAIN_SPELLING,
      fullWidth,
    );
  const english = route(key(), false, false, true);
  check(
    english.action === HardwareKeyAction.WIDEN && english.character === 0x61,
    "an English letter is widened",
  );
  check(
    route(key(), false, false, false).action === HardwareKeyAction.RELEASE,
    "and released with fullwidth off",
  );
  check(
    route(key({ keyCode: 2001, unicodeChar: 0x31 }), false, true, true).action ===
      HardwareKeyAction.WIDEN,
    "an idle digit is widened in Chinese mode too",
  );
  check(
    route(key({ keyCode: 2050, unicodeChar: 0x20 }), false, true, true).action ===
      HardwareKeyAction.WIDEN,
    "so is an idle space",
  );
  check(
    route(key({ keyCode: 2072, unicodeChar: 0x2c }), false, false, true).action ===
      HardwareKeyAction.WIDEN,
    "and an English-mode punctuation mark",
  );
  check(
    route(key({ keyCode: 2072, unicodeChar: 0x2c }), false, true, true).action ===
      HardwareKeyAction.PUNCTUATION,
    "a Chinese-mode mark still goes to the punctuation path, which widens its own literal",
  );
  check(
    route(key(), false, true, true).action === HardwareKeyAction.COMPOSE,
    "a Chinese-mode letter still composes",
  );
  check(
    route(key({ ctrlKey: true }), false, false, true).action === HardwareKeyAction.RELEASE,
    "a modifier chord stays the application's",
  );
  check(
    route(key({ keyCode: 2054, unicodeChar: 0x0d }), false, false, true).action ===
      HardwareKeyAction.RELEASE,
    "a control character is not printable ASCII",
  );
  check(
    route(key({ keyCode: 2062, unicodeChar: 0x3b }), true, false, true).action ===
      HardwareKeyAction.COMMIT_THEN_TYPE,
    "nothing is widened over a candidate list: the composition is finished and the key typed as it is",
  );
  const log: string[] = [];
  const widening: HardwareKeyTarget = {
    ...recordingTarget(log),
    press: () => false,
    widen: (character: number) => {
      log.push(`widen ${character}`);
      return true;
    },
  };
  check(
    HardwareKeyDispatch.apply(
      { action: HardwareKeyAction.COMPOSE, character: 0x4e, index: 0 },
      true,
      widening,
    ) && log[0] === "widen 78",
    "a capital the Engine declines is widened rather than handed back",
  );
  check(
    dispatched(HardwareKeyAction.WIDEN, 0x61)[0] === "widen 97",
    "a widened key reaches the widen path with its character",
  );
});

group("hardware characters handed to the application count as typed", () => {
  // Windows `ShouldCountPassthroughChar`: printable, and no Ctrl, Alt or Win. Shift is how capitals are typed.
  check(TypingStatisticsPolicy.countsPassthrough(0x61, false, false, false), "a letter counts");
  check(TypingStatisticsPolicy.countsPassthrough(0x20, false, false, false), "so does a space");
  check(
    TypingStatisticsPolicy.countsPassthrough(0x4e2d, false, false, false),
    "and a character beyond ASCII",
  );
  check(!TypingStatisticsPolicy.countsPassthrough(0x61, true, false, false), "Ctrl does not");
  check(!TypingStatisticsPolicy.countsPassthrough(0x61, false, true, false), "nor Alt");
  check(!TypingStatisticsPolicy.countsPassthrough(0x61, false, false, true), "nor the logo key");
  check(!TypingStatisticsPolicy.countsPassthrough(0x0d, false, false, false), "nor Enter");
  check(!TypingStatisticsPolicy.countsPassthrough(0x7f, false, false, false), "nor DEL");
  check(!TypingStatisticsPolicy.countsPassthrough(0, false, false, false), "nor an arrow key");
});

group("a numeric keypad is a number row", () => {
  const key = (over: Record<string, unknown> = {}) => ({
    keyCode: 2104,
    unicodeChar: 0,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    logoKey: false,
    ...over,
  });
  // The source normalises the keypad in one place so every digit path gets it at once. Without it
  // the maintenance chord matched only the number row, because with Ctrl+Shift+Alt held the system
  // resolves no character and the chord has to match on the code.
  const normalized = HardwareKeyRouter.normalizeNumpad(key());
  check(normalized.keyCode === 2001, "keypad 1 reads as the number-row 1");
  check(normalized.unicodeChar === 0x31, "and carries the digit the system did not resolve");
  check(
    HardwareKeyRouter.normalizeNumpad(key({ keyCode: 2112 })).keyCode === 2009,
    "keypad 9 reads as the number-row 9",
  );
  const letter = HardwareKeyRouter.normalizeNumpad(key({ keyCode: 2017, unicodeChar: 0x61 }));
  check(letter.keyCode === 2017 && letter.unicodeChar === 0x61, "anything else is left alone");
  // A keypad key that did resolve a character keeps it rather than having one invented.
  check(
    HardwareKeyRouter.normalizeNumpad(key({ keyCode: 2106, unicodeChar: 0x33 })).unicodeChar ===
      0x33,
    "a resolved character is kept",
  );
});

group("keypad digits reach both digit paths", () => {
  const compose = {
    minusEqual: true,
    commaPeriod: true,
    brackets: false,
    tab: true,
    pageUpDown: true,
    mouseWheel: false,
    arrows: true,
  };
  const chord = HardwareKeyRouter.route(
    { keyCode: 2105, unicodeChar: 0, ctrlKey: true, altKey: true, shiftKey: true, logoKey: false },
    true,
    true,
    false,
    compose,
  );
  check(
    chord.action === HardwareKeyAction.REMOVE_CANDIDATE && chord.index === 1,
    "Ctrl+Shift+Alt+keypad2 deletes the second candidate, as Ctrl+Shift+Alt+2 does",
  );
  const select = HardwareKeyRouter.route(
    {
      keyCode: 2106,
      unicodeChar: 0,
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      logoKey: false,
    },
    true,
    true,
    false,
    compose,
  );
  check(
    select.action === HardwareKeyAction.SELECT && select.index === 2,
    "keypad 3 picks the third candidate",
  );
});

group("Ctrl+Shift+F switches simplified and traditional, not the character width", () => {
  const routing: InputModeRouting = new InputModeRouting();
  routing.use(DEFAULT_MODE_BINDINGS);
  // Windows `HandleImeKey` answers `IsCharacterSetShortcut` with `SetConfiguredCharacterSet`; the settings page labels the binding 切换简繁.
  check(
    routing.accept(modeKey(2022, true, 0, { ctrlKey: true, shiftKey: true })) ===
      ModeGesture.TOGGLE_CHARACTER_SET,
    "Ctrl+Shift+F is the simplified/traditional chord",
  );
  routing.use({ ...DEFAULT_MODE_BINDINGS, toggleCharacterSetCtrlShiftF: false });
  check(
    routing.accept(modeKey(2022, true, 0, { ctrlKey: true, shiftKey: true })) === ModeGesture.NONE,
    "turning the binding off leaves the chord to the application",
  );
});

group("a hardware key spells or punctuates depending on what is being spelled", () => {
  const route = (over: Record<string, unknown>, spelling: Partial<HardwareSpelling>) =>
    HardwareKeyRouter.route(
      {
        keyCode: 0,
        unicodeChar: 0,
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        logoKey: false,
        ...over,
      } as HardwareKey,
      true,
      true,
      false,
      undefined,
      false,
      false,
      "disabled",
      true,
      { ...PLAIN_SPELLING, ...spelling },
    );
  const unicode: Partial<HardwareSpelling> = { localMode: "unicode", editing: "u4e", caret: 3 };
  const digit = route({ keyCode: 2000, unicodeChar: 0x30 }, unicode);
  check(
    digit.action === HardwareKeyAction.COMPOSE && digit.character === 0x30,
    "in U mode a plain 0 is part of the code point",
  );
  const four = route({ keyCode: 2004, unicodeChar: 0x34 }, unicode);
  check(
    four.action === HardwareKeyAction.COMPOSE && four.character === 0x34,
    "and so is a plain 4, which would otherwise pick the fourth candidate",
  );
  const pick = route({ keyCode: 2002, unicodeChar: 0x40, shiftKey: true }, unicode);
  check(
    pick.action === HardwareKeyAction.SELECT && pick.index === 1,
    "Shift+2 picks the second candidate in U mode, as on Windows",
  );
  const plus = route(
    { keyCode: 2058, unicodeChar: 0x2b, shiftKey: true },
    { localMode: "unicode", editing: "U", caret: 1 },
  );
  check(
    plus.action === HardwareKeyAction.COMPOSE && plus.character === 0x2b,
    "the + of U+ is spelled rather than taken as a paging key or a mark",
  );
  check(
    route({ keyCode: 2058, unicodeChar: 0x2b, shiftKey: true }, unicode).action !==
      HardwareKeyAction.COMPOSE,
    "a + anywhere else in the code point is not part of it",
  );
  check(
    route({ keyCode: 2004, unicodeChar: 0x34 }, { editing: "ni", caret: 2 }).action ===
      HardwareKeyAction.SELECT,
    "outside U mode a digit still picks",
  );

  const separator = route({ keyCode: 2063, unicodeChar: 0x27 }, { editing: "xi", caret: 2 });
  check(
    separator.action === HardwareKeyAction.COMPOSE && separator.character === 0x27,
    "' separates pinyin syllables mid-composition",
  );
  check(
    route({ keyCode: 2063, unicodeChar: 0x27 }, { editing: "xi", caret: 0 }).action ===
      HardwareKeyAction.PUNCTUATION,
    "with the caret at the start there is nothing to separate",
  );
  check(
    route({ keyCode: 2063, unicodeChar: 0x27 }, { editing: "abcd", caret: 4, wubi: true })
      .action === HardwareKeyAction.PUNCTUATION,
    "Wubi codes have no syllables, so ' stays a mark",
  );
  check(
    route({ keyCode: 2063, unicodeChar: 0x27 }, { localMode: "emoji", editing: "xiao", caret: 4 })
      .action === HardwareKeyAction.COMPOSE,
    "emoji spellings are separated the same way",
  );

  const microsoft: Partial<HardwareSpelling> = { microsoftShuangpin: true };
  const ing = route({ keyCode: 2062, unicodeChar: 0x3b }, { ...microsoft, editing: "x", caret: 1 });
  check(
    ing.action === HardwareKeyAction.COMPOSE && ing.character === 0x3b,
    "Microsoft shuangpin spells ing with ; as the second key of a syllable",
  );
  check(
    route({ keyCode: 2062, unicodeChar: 0x3b }, { ...microsoft, editing: "xm", caret: 2 })
      .action === HardwareKeyAction.PUNCTUATION,
    "as the first key of the next syllable it is a mark",
  );
  check(
    route({ keyCode: 2062, unicodeChar: 0x3b }, { ...microsoft, editing: "xm'x", caret: 4 })
      .action === HardwareKeyAction.COMPOSE,
    "syllables are counted from the last separator",
  );
  check(
    route({ keyCode: 2062, unicodeChar: 0x3b }, { editing: "x", caret: 1 }).action ===
      HardwareKeyAction.PUNCTUATION,
    "other layouts never spell with ;",
  );

  const english: Partial<HardwareSpelling> = { editing: "don", caret: 3, englishCandidates: true };
  check(
    route({ keyCode: 2063, unicodeChar: 0x27 }, english).action === HardwareKeyAction.PUNCTUATION,
    "in the English candidate mode ' ends the word instead of being spelled into it",
  );
  check(
    route({ keyCode: 2062, unicodeChar: 0x3b }, { ...english, microsoftShuangpin: true }).action ===
      HardwareKeyAction.PUNCTUATION,
    "and ; is not a shuangpin final there",
  );
  check(
    route({ keyCode: 2004, unicodeChar: 0x34 }, english).action === HardwareKeyAction.SELECT,
    "a digit still picks an English word",
  );
});

group("space after a Chinese mark rewrites it as ASCII", () => {
  // Transcribed from SmartPunctuationAsciiFor in the source. Both quote directions map to the same
  // straight quote, as they do there.
  check(SmartPunctuationSpacePolicy.asciiFor(0x3002) === 0x2e, "。 becomes .");
  check(SmartPunctuationSpacePolicy.asciiFor(0x3001) === 0x2f, "、 becomes / rather than a comma");
  check(SmartPunctuationSpacePolicy.asciiFor(0x201c) === 0x22, "“ becomes a straight quote");
  check(SmartPunctuationSpacePolicy.asciiFor(0x201d) === 0x22, "and so does ”");
  check(SmartPunctuationSpacePolicy.asciiFor(0x4e2d) === 0, "a Han character has no ASCII twin");

  const armed = SmartPunctuationSpacePolicy.arm("。", false, true, true, 7);
  check(armed !== null && armed.ascii === 0x2e, "committing 。 arms the conversion");
  check(
    SmartPunctuationSpacePolicy.decide(armed, 0x20, 0x3002, false, 7) ===
      SpaceConvertDecision.CONVERT,
    "a space with the mark still before the caret converts",
  );
});

group("the conversion declines rather than rewriting the wrong character", () => {
  const armed = SmartPunctuationSpacePolicy.arm("。", false, true, true, 7);
  // The arming records what was committed, not what is still there.
  check(
    SmartPunctuationSpacePolicy.decide(armed, 0x20, 0x4e2d, false, 7) === SpaceConvertDecision.NONE,
    "something else before the caret means the caret moved",
  );
  check(
    SmartPunctuationSpacePolicy.decide(armed, 0x20, 0x3002, false, 8) === SpaceConvertDecision.NONE,
    "a different editor session does not convert",
  );
  check(
    SmartPunctuationSpacePolicy.decide(armed, 0x61, 0x3002, false, 7) === SpaceConvertDecision.NONE,
    "a key that is not a space is not this gesture",
  );
  check(
    SmartPunctuationSpacePolicy.decide(armed, 0x20, 0x3002, true, 7) === SpaceConvertDecision.NONE,
    "a space mid-composition belongs to the composition",
  );
  check(
    SmartPunctuationSpacePolicy.decide(null, 0x20, 0x3002, false, 7) === SpaceConvertDecision.NONE,
    "nothing armed, nothing converted",
  );
});

group("what the conversion refuses to arm on", () => {
  // The caret sits between the two marks of an auto-closed pair, so the character before it is the
  // opening one and rewriting it would break the pair. The source refuses the same case.
  check(
    SmartPunctuationSpacePolicy.arm("（", true, true, true, 1) === null,
    "an auto-closed pair does not arm",
  );
  check(
    SmartPunctuationSpacePolicy.arm("。", false, true, false, 1) === null,
    "the switch being off means the setting is honoured, not ignored",
  );
  check(
    SmartPunctuationSpacePolicy.arm("。", false, false, true, 1) === null,
    "smart punctuation being off takes the whole family with it",
  );
  check(
    SmartPunctuationSpacePolicy.arm("你好", false, true, true, 1) === null,
    "a commit of more than one scalar is not a mark the space is about",
  );
  check(
    SmartPunctuationSpacePolicy.arm("", false, true, true, 1) === null,
    "nor is an empty commit",
  );
  check(
    SmartPunctuationSpacePolicy.arm(null, false, true, true, 1) === null,
    "nor is no commit at all",
  );
});

group("a right click opens the candidate menu, as it does in the source", () => {
  check(
    CandidateContextMenuPolicy.opens(PointerButton.RIGHT, PointerAction.PRESS),
    "the right button opens the management menu",
  );
  // Acting on both press and release would open the menu and immediately act again on whatever
  // entry the cursor had landed on.
  check(
    !CandidateContextMenuPolicy.opens(PointerButton.RIGHT, PointerAction.RELEASE),
    "the release does not open it a second time",
  );
  check(
    !CandidateContextMenuPolicy.opens(PointerButton.RIGHT, PointerAction.MOVE),
    "nor does moving with the button held",
  );
  check(
    !CandidateContextMenuPolicy.opens(PointerButton.LEFT, PointerAction.PRESS),
    "the left button still chooses the candidate",
  );
});

group("the candidate window does not swallow the other mouse buttons", () => {
  // The middle button pastes on some systems and the side buttons navigate. A candidate window
  // that claimed them would be taking away a gesture it never offered.
  for (const button of [PointerButton.MIDDLE, PointerButton.BACK, PointerButton.FORWARD]) {
    check(
      !CandidateContextMenuPolicy.opens(button, PointerAction.PRESS),
      `button ${button} is left to the system`,
    );
  }
  check(
    !CandidateContextMenuPolicy.opens(PointerButton.NONE, PointerAction.PRESS),
    "and so is a press with no button at all",
  );
});

group("the candidate number keeps its proportion, as the source states it", () => {
  check(
    !CandidateNumberFontPolicy.visible(false),
    "a touch candidate does not draw an ordinal with no number key to answer it",
  );
  check(
    CandidateNumberFontPolicy.visible(true),
    "a 2-in-1 candidate keeps the ordinal for its physical number row",
  );
  // `.num { font-size: 0.8em }` in every candidate stylesheet. The host used to subtract a
  // constant with a floor, which agrees with the ratio at no size at all.
  check(
    CandidateNumberFontPolicy.size(18) === 14,
    "at the shared default the number is 14, not 10",
  );
  check(CandidateNumberFontPolicy.size(12) === 10, "at the smallest allowed size");
  check(CandidateNumberFontPolicy.size(32) === 26, "at the largest allowed size");
  // The old formula was max(10, size - 8): identical at no point in the range, and wrong in both
  // directions around it.
  for (const size of [12, 16, 18, 20, 24, 28, 32]) {
    const previous = Math.max(10, size - 8);
    const ratio = CandidateNumberFontPolicy.size(size);
    check(
      ratio === Math.round(size * 0.8),
      `size ${size} follows the ratio (was ${previous}, now ${ratio})`,
    );
  }
});

group("a malformed candidate size cannot produce an unusable number", () => {
  // The shared document validates 12..=32, so this is a guard rather than a design choice: a zero
  // or negative font size is a crash or an invisible row, not a small number.
  check(CandidateNumberFontPolicy.size(0) === 1, "zero does not become zero");
  check(CandidateNumberFontPolicy.size(-4) === 1, "nor does a negative size");
  check(CandidateNumberFontPolicy.size(Number.NaN) === 1, "nor does a size that is not a number");
});

group("「候选栏预编辑：不显示」 hides the spelling on the phone line", () => {
  // Windows candidate_window_preedit_style = "empty" is preeditVisible=false. The phone showed the spelling whatever the setting said; Android honours it through the same policy.
  const shown = CandidatePreeditStylePolicy.visible(true, "", "nihao", "none");
  check(shown.text === "nihao" && shown.caret, "pinyin shows the spelling and its caret");
  const hidden = CandidatePreeditStylePolicy.visible(false, "", "nihao", "none");
  check(hidden.text === "" && !hidden.caret, "empty hides the spelling and the caret");
  const phrase = CandidatePreeditStylePolicy.visible(false, "你好", "你好shijie", "none");
  check(
    phrase.text === "你好",
    "the chosen part of a phrase stays, since it is nowhere else on screen",
  );
  const trigger = CandidatePreeditStylePolicy.visible(false, "", "E", "emoji");
  check(trigger.text === "E", "a local mode's trigger stays, since it names the running mode");
  const modeSpelling = CandidatePreeditStylePolicy.visible(false, "", "Ksmile", "quick_phrase");
  check(
    modeSpelling.text === "",
    "what a mode spells beyond its trigger is hidden like any spelling",
  );
});

group("the composition is split where the caret is", () => {
  // Ctrl+Left, Ctrl+Right and Ctrl+Backspace edit one Engine segment at a time on a 2in1, and the
  // caret they move was never drawn: the shared view carries caret_position and the ArkTS side had
  // not declared it. A segment editor with an invisible caret cannot be used at all.
  const split = PreeditCaretPolicy.split("nihao", 2);
  check(split.before === "ni" && split.after === "hao", "the caret sits between the segments");
  const end = PreeditCaretPolicy.split("nihao", 5);
  check(end.before === "nihao" && end.after === "", "at the end nothing follows it");
  const start = PreeditCaretPolicy.split("nihao", 0);
  check(start.before === "" && start.after === "nihao", "at the start nothing precedes it");
});

group("a caret outside the composition is clamped, not indexed past", () => {
  // The view and the caret arrive in the same message so they cannot normally disagree. A clamp
  // costs nothing, and the alternative is a caret that vanishes exactly while someone is editing.
  const over = PreeditCaretPolicy.split("ni", 9);
  check(over.before === "ni" && over.after === "", "past the end lands at the end");
  const under = PreeditCaretPolicy.split("ni", -3);
  check(under.before === "" && under.after === "ni", "before the start lands at the start");
  const empty = PreeditCaretPolicy.split("", 4);
  check(empty.before === "" && empty.after === "", "an empty composition stays empty");
  const nan = PreeditCaretPolicy.split("ni", Number.NaN);
  check(nan.before === "ni" && nan.after === "", "an unusable position does not split the text");
});

group("the caret is as tall as the source draws it", () => {
  // `.cursor { height: 1.2em }`.
  check(PreeditCaretPolicy.height(18) === 22, "1.2em of the shared default preedit size");
  check(PreeditCaretPolicy.height(12) === 14, "at the smallest allowed size");
  check(PreeditCaretPolicy.height(32) === 38, "at the largest allowed size");
  check(
    PreeditCaretPolicy.height(0) === 1,
    "a zero size still draws something rather than nothing",
  );
});

group("the offline gloss is drawn the way the source draws it", () => {
  // .cand-translation { margin-left: 0.65em; font-size: 0.78em; opacity: 0.62 }, identical in all
  // four skins' vertical stylesheets.
  check(CandidateTranslationStyle.fontSize(18) === 14, "0.78em of the shared default");
  check(CandidateTranslationStyle.fontSize(12) === 9, "at the smallest allowed candidate size");
  check(CandidateTranslationStyle.fontSize(32) === 25, "at the largest allowed candidate size");
  check(TRANSLATION_OPACITY === 0.62, "and the source's transparency rather than the host's 0.7");
});

group("the gloss gap resolves against the gloss, not the candidate", () => {
  // `em` in margin-left resolves against the element's own computed font size; only font-size
  // itself looks upwards. The gloss is already 0.78 of the row, so the gap is 0.65 of that.
  check(CandidateTranslationStyle.gap(18) === Math.round(18 * 0.78 * 0.65), "0.65em of the gloss");
  check(CandidateTranslationStyle.gap(18) === 9, "which is 9 at the shared default, not 12");
  check(CandidateTranslationStyle.gap(32) === 16, "and scales with the candidate size");
});

group("a malformed candidate size cannot make the gloss unusable", () => {
  check(CandidateTranslationStyle.fontSize(0) === 1, "a zero size still draws something");
  check(CandidateTranslationStyle.fontSize(-2) === 1, "and so does a negative one");
  check(
    CandidateTranslationStyle.fontSize(Number.NaN) === 1,
    "and so does one that is not a number",
  );
  check(
    CandidateTranslationStyle.gap(0) === 0,
    "a zero size asks for no gap rather than a negative one",
  );
});

group("only a translation takes the translation's appearance", () => {
  // The Engine's annotation shares the slot here but has no rule in either stylesheet, so giving it
  // the translation's styling would be assuming the source meant both.
  check(
    CandidateGlossPolicy.annotationIsTranslation(null, "hello", true),
    "a resolved gloss with no Engine annotation is a translation",
  );
  check(
    !CandidateGlossPolicy.annotationIsTranslation("qwer", "hello", true),
    "an Engine annotation wins the slot and is not a translation",
  );
  check(
    !CandidateGlossPolicy.annotationIsTranslation(null, "hello", false),
    "a gloss the user turned off is not in the slot at all",
  );
  check(
    !CandidateGlossPolicy.annotationIsTranslation(null, null, true),
    "and neither is one that was never resolved",
  );
  check(!CandidateGlossPolicy.annotationIsTranslation(null, "", true), "nor an empty one");
});

group("the panel chord asks for the same action the toolbar button does", () => {
  // The chord used to call the host's window helper directly. That helper shows and sizes a panel
  // but never records which surface is open, so the view went on drawing candidates and the window
  // was sized around them — a thin empty bar on a 2in1 where a keyboard had been asked for. Naming
  // the action makes the two entries comparable, which is the only way this stays fixed.
  check(
    (PanelSurfaceAction.SCREEN_KEYBOARD as number) === (ToolbarButton.SCREEN_KEYBOARD as number),
    "the shortcut's action is the toolbar's screen-keyboard button",
  );
  check(
    PanelShortcutPolicy.action(PanelShortcut.SCREEN_KEYBOARD) ===
      PanelSurfaceAction.SCREEN_KEYBOARD,
    "the screen-keyboard shortcut maps to that action",
  );
  check(
    PanelShortcutPolicy.action(PanelShortcut.NONE) === PanelSurfaceAction.NONE,
    "no shortcut asks for no surface",
  );
});

group("the settings page is refreshed on a changed document, not on every visit", () => {
  // The page answers an announcement with "设置已被其他窗口修改" when it holds unsaved edits, so an
  // announcement that did not correspond to a change costs the user their draft for nothing.
  check(PreferenceRevisionPolicy.changed(7, 8), "a newer document is a change");
  check(!PreferenceRevisionPolicy.changed(7, 7), "the same document is not");
  // The only ways a revision goes backwards are a restored profile or a rewritten document, and in
  // both the page is holding something that no longer describes the file.
  check(PreferenceRevisionPolicy.changed(8, 3), "a document that went backwards is a change too");
  check(
    !PreferenceRevisionPolicy.changed(Number.NaN, 4),
    "an unusable observation announces nothing",
  );
  check(!PreferenceRevisionPolicy.changed(4, Number.NaN), "and neither does an unusable reading");
});

group("an unreadable revision does not replace a good one", () => {
  check(PreferenceRevisionPolicy.observe(5, 9) === 9, "a usable revision is remembered");
  check(PreferenceRevisionPolicy.observe(5, Number.NaN) === 5, "NaN leaves the previous in place");
  check(PreferenceRevisionPolicy.observe(5, -2) === 5, "and so does a negative one");
  // -1 is what the bridge starts with, and it must not compare equal to any real revision.
  check(PreferenceRevisionPolicy.changed(-1, 0), "the initial value counts as not yet observed");
});

group("an imported skin folder name comes from outside and is checked", () => {
  check(SkinImportPolicy.destinationName("midnight") === "midnight", "an ordinary name is kept");
  check(
    SkinImportPolicy.destinationName("  forest  ") === "forest",
    "surrounding space is trimmed",
  );
  // The skins root sits beside the staged dictionary, so a name that climbs out of it would write
  // over the Engine's resources.
  check(SkinImportPolicy.destinationName("../../engine") === null, "a traversal is refused");
  check(SkinImportPolicy.destinationName("a/b") === null, "and so is any forward slash");
  check(SkinImportPolicy.destinationName("a\\b") === null, "and any backslash");
  check(SkinImportPolicy.destinationName(".") === null, "a lone dot means the skins root itself");
  check(SkinImportPolicy.destinationName("..") === null, "and two dots mean its parent");
  check(SkinImportPolicy.destinationName("") === null, "an empty name is not a folder");
  check(SkinImportPolicy.destinationName("   ") === null, "nor is one made only of space");
  check(SkinImportPolicy.destinationName("a\u0007b") === null, "a control character is refused");
  check(SkinImportPolicy.destinationName("x".repeat(65)) === null, "and so is an over-long name");
  check(SkinImportPolicy.destinationName("x".repeat(64)) !== null, "64 is still allowed");
});

group("the folder name is taken from the picked URI", () => {
  check(
    SkinImportPolicy.pickedName("file://docs/a/b/midnight") === "midnight",
    "the last segment is the folder the user chose",
  );
  check(
    SkinImportPolicy.pickedName("file://docs/a/midnight/") === "midnight",
    "a trailing separator does not make the name empty",
  );
  check(
    SkinImportPolicy.pickedName("file://docs/a/my%20skin") === "my skin",
    "an escaped segment is decoded",
  );
  // A name that survives decoding still has to pass the same check.
  check(
    SkinImportPolicy.pickedName("file://docs/a/%2E%2E") === null,
    "an escaped traversal is refused after decoding, not before",
  );
  check(SkinImportPolicy.pickedName(null) === null, "no pick is no name");
});

group("a refused preferences write carries the code the page has a sentence for", () => {
  // The whole point of the compare-and-swap: the keyboard's toolbar wrote while a settings window
  // was open. The page asks the user to reload, but only if it can tell this apart from a failure.
  check(
    PreferencesErrorCode.of("preferences changed; reload before saving") === "conflict",
    "a stale revision is a conflict",
  );
  check(
    PreferencesErrorCode.of("candidate page size must be between 1 and 9") === "invalid",
    "an out-of-range page size is invalid",
  );
  check(
    PreferencesErrorCode.of("frequency trigger count and linear step must be between 1 and 10") ===
      "frequency_invalid",
    "a bad frequency has its own code",
  );
  check(
    PreferencesErrorCode.of("mixed English minimum prefix must be between 1 and 8") ===
      "mixed_input_invalid",
    "and so does a bad mixed-input prefix",
  );
  check(
    PreferencesErrorCode.of("word-to-character and paging cannot use the same keys") ===
      "key_conflict",
    "colliding key bindings are a key conflict",
  );
  check(
    PreferencesErrorCode.of("unsupported preferences format") === "format",
    "an unreadable format is a format failure",
  );
  check(
    PreferencesErrorCode.of("invalid preferences document: expected value at line 1 column 1") ===
      "format",
    "a nested cause is matched by its prefix",
  );
  // Anything unnamed becomes the code the desktop uses for everything it does not name, so the page
  // says its general sentence rather than printing English at the user.
  check(
    PreferencesErrorCode.of("preferences storage failed: permission denied") === "storage",
    "an unnamed refusal falls back to storage",
  );
  check(PreferencesErrorCode.of("") === "storage", "and so does a refusal with no reason at all");
});

group("rewriting a reply touches only the refusals", () => {
  check(
    PreferencesErrorCode.rewrite(
      JSON.stringify({ ok: false, error: "preferences changed; reload before saving" }),
    ) === JSON.stringify({ ok: false, error: "conflict" }),
    "a refusal is rewritten",
  );
  const accepted = JSON.stringify({ ok: true, value: { revision: 4 } });
  check(PreferencesErrorCode.rewrite(accepted) === accepted, "an accepted write is left alone");
  // This sits on the path that carries the whole document; forwarding something unreadable is
  // better than replacing it with a failure invented here.
  check(
    PreferencesErrorCode.rewrite("not json") === "not json",
    "an unreadable reply is forwarded",
  );
  check(PreferencesErrorCode.rewrite("null") === "null", "and so is a reply that is not a record");
});

group("AI model catalogs keep each provider's protocol and path", () => {
  check(TextPolicy.hasAuthority("https://example.test/path", "https://"), "accepts a populated HTTPS authority");
  check(!TextPolicy.hasAuthority("https:///path", "https://"), "rejects an empty HTTPS authority");
  check(TextPolicy.hasAuthority("wss://speech.example.test/live", "wss://"), "shares authority parsing for WSS");
  check(TextPolicy.validMultiline("line\nfeed", 32, true), "allows prompt line breaks");
  check(!TextPolicy.validMultiline("bad\u0001", 32, true), "rejects other control characters");
  check(TextPolicy.validSecureAuthority("https://remote.example/api", true),
    "accepts remote HTTPS endpoints");
  check(TextPolicy.validSecureAuthority("http://127.0.0.1:8080/api", true),
    "accepts loopback HTTP endpoints");
  check(TextPolicy.validSecureAuthority("http://[::1]:8080/api", true),
    "accepts IPv6 loopback HTTP endpoints");
  check(!TextPolicy.validSecureAuthority("http://remote.example/api", true),
    "rejects remote HTTP endpoints");
  check(!TextPolicy.validSecureAuthority("http://localhost.example/api", true),
    "rejects lookalike loopback hosts");
  check(
    AiModelCatalogPolicy.modelsUrl("https://api.everyapi.ai/v1/chat/completions") ===
      "https://api.everyapi.ai/v1/models",
    "an OpenAI-compatible chat endpoint keeps its version prefix",
  );
  check(
    AiModelCatalogPolicy.modelsUrl(
      "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    ) === "https://generativelanguage.googleapis.com/v1beta/openai/models",
    "Gemini does not get rewritten to a different API version",
  );
  check(
    AiModelCatalogPolicy.modelsUrl("https://user:secret@example.test/v1/chat/completions") === null,
    "embedded credentials are rejected",
  );
  check(
    AiModelCatalogPolicy.modelsUrl("http://example.test/v1/chat/completions") === null,
    "catalog credentials are never sent over HTTP",
  );
  const anthropic = "https://api.anthropic.com/v1/models";
  check(
    AiModelCatalogPolicy.isAnthropic("anthropic", anthropic),
    "the Anthropic preset selects its native authentication",
  );
  check(
    AiModelCatalogPolicy.pageUrl(anthropic, true, "") ===
      "https://api.anthropic.com/v1/models?limit=1000",
    "the first Anthropic page asks for the bounded maximum",
  );
  check(
    AiModelCatalogPolicy.pageUrl(anthropic, true, "claude/first") ===
      "https://api.anthropic.com/v1/models?limit=1000&after_id=claude%2Ffirst",
    "the next Anthropic cursor is encoded",
  );
});

group("AI model catalogs filter capabilities and paginate safely", () => {
  const page: AiCatalogPage = {
    data: [
      { id: "chat-model", supported_endpoint_types: ["openai"] },
      { id: "speech-model", supported_endpoint_types: ["audio-transcription"] },
      { id: "chat-model", supported_endpoint_types: ["openai"] },
      { id: "disabled", active: false },
      {
        id: "response-model",
        supported_endpoint_types: ["openai-response"],
        chat_completions_bridge: true,
      },
    ],
    has_more: true,
    last_id: "response-model",
  };
  const models: string[] = [];
  check(AiModelCatalogPolicy.append(models, page), "a well-formed page is accepted");
  check(
    JSON.stringify(models) === JSON.stringify(["chat-model", "response-model"]),
    "duplicates, inactive rows and voice-only models are removed",
  );
  check(
    AiModelCatalogPolicy.nextCursor(page, true, []) === "response-model",
    "Anthropic can continue with a fresh cursor",
  );
  check(
    AiModelCatalogPolicy.nextCursor(page, false, []) === null,
    "a non-Anthropic pagination envelope is refused",
  );
  check(
    AiModelCatalogPolicy.nextCursor(page, true, ["response-model"]) === null,
    "a repeated cursor cannot loop forever",
  );
  check(
    AiModelCatalogPolicy.append([], { data: [{ id: "first" }, { id: "second" }] }, 1) === false,
    "the aggregate model bound is enforced across pages",
  );
});

group("Harmony batch transcription accepts every shared cloud preset", () => {
  const everyapi: VoiceInputConfiguration = {
    ...DEFAULT_VOICE_INPUT_CONFIGURATION,
    asr_provider: "everyapi",
    asr_endpoint: "",
    asr_model: "",
    asr_token: "",
    asr_tokens: { everyapi: "synthetic-everyapi-key" },
  };
  check(
    HttpAsrConfigurationPolicy.endpoint(everyapi) ===
      "https://api.everyapi.ai/v1/audio/transcriptions",
    "EveryAPI resolves to its transcription endpoint",
  );
  check(
    HttpAsrConfigurationPolicy.model(everyapi) === "openai/whisper-large-v3-turbo",
    "EveryAPI resolves to its shared default model",
  );
  check(
    HttpAsrConfigurationPolicy.token(everyapi) === "synthetic-everyapi-key",
    "a provider token survives switching away and back",
  );
  check(HttpAsrConfigurationPolicy.valid(everyapi), "the complete EveryAPI preset can record");

  const mistral: VoiceInputConfiguration = {
    ...everyapi,
    asr_provider: "mistral",
    asr_tokens: { mistral: "synthetic-mistral-key" },
  };
  check(
    HttpAsrConfigurationPolicy.endpoint(mistral) ===
      "https://api.mistral.ai/v1/audio/transcriptions",
    "Mistral resolves to its transcription endpoint",
  );
  check(
    HttpAsrConfigurationPolicy.model(mistral) === "voxtral-mini-latest",
    "Mistral resolves to its shared default model",
  );
  check(HttpAsrConfigurationPolicy.valid(mistral), "the complete Mistral preset can record");
  check(
    !HttpAsrConfigurationPolicy.valid({
      ...mistral,
      asr_endpoint: "https://user:secret@example.test/v1/audio/transcriptions",
    }),
    "embedded endpoint credentials are refused before recording",
  );
  check(
    !HttpAsrConfigurationPolicy.valid({ ...mistral, asr_provider: "local" }),
    "a provider without the HTTP adapter cannot fall through to it",
  );
});

group("the 2in1 draws the composition inline as tsf_preedit_style says", () => {
  check(
    InlinePreeditPolicy.style(undefined) === "raw",
    "a document without the field shows raw letters",
  );
  check(InlinePreeditPolicy.style("pinyin") === "pinyin", "pinyin is read");
  check(InlinePreeditPolicy.style("empty") === "empty", "empty is read");
  check(InlinePreeditPolicy.style("local") === "raw", "an unknown style falls back to raw");
  check(
    InlinePreeditPolicy.text("raw", true, true, "nihao", "ni'hao", "") === "nihao",
    "raw shows the letters as typed",
  );
  check(
    InlinePreeditPolicy.text("pinyin", true, true, "nihao", "ni'hao", "") === "ni'hao",
    "pinyin shows the segmented spelling",
  );
  check(
    InlinePreeditPolicy.text("pinyin", true, true, "nihao", "", "") === "nihao",
    "pinyin without a segmented spelling falls back to the letters",
  );
  check(
    InlinePreeditPolicy.text("raw", true, true, "hao", "hao", "你") === "你hao",
    "a held phrase piece leads the spelling",
  );
  check(
    InlinePreeditPolicy.text("empty", true, true, "nihao", "ni'hao", "") === "",
    "empty leaves the document alone",
  );
  check(
    InlinePreeditPolicy.text("raw", false, true, "nihao", "ni'hao", "") === "",
    "a phone keeps the spelling above its keys",
  );
  check(
    InlinePreeditPolicy.text("raw", true, false, "nihao", "ni'hao", "") === "",
    "an editor without preview text gets none",
  );
  check(
    InlinePreeditPolicy.text("raw", true, true, "", "", "") === "",
    "no composition, no preview",
  );
  check(
    InlinePreeditPolicy.text("raw", true, true, "", "", "海滩") === "海滩",
    "a held phrase piece stays in the preview after Ctrl+Backspace empties the reading",
  );
  check(
    InlinePreeditPolicy.beforePreview("好nihao", "nihao") === "好",
    "context before the caret skips the preview the editor counts as text",
  );
  check(
    InlinePreeditPolicy.beforePreview("好a", "nihao") === "好a",
    "an editor that keeps preview text out of its content is read as is",
  );
  check(InlinePreeditPolicy.beforePreview("好a", "") === "好a", "no preview, nothing removed");
});

// The account bridge deliberately models the asynchronous device HTTP API. Give its immediate
// mock responses one microtask turn before reporting the suite result.
setTimeout(() => {
  console.log("");
  if (failures > 0) {
    throw new Error(`${failures} group(s) failed`);
  }
  console.log(`all groups passed (${checks} assertions)`);
}, 0);

group("the 2in1 emoji panel answers keys the way the focused Windows panel does", () => {
  const key = (keyCode: number, unicodeChar = 0, ctrlKey = false) => ({
    keyCode,
    unicodeChar,
    ctrlKey,
    altKey: false,
    logoKey: false,
  });
  const LEFT = 2014;
  const RIGHT = 2015;
  const UP = 2012;
  const DOWN = 2013;
  const HOME = 2081;
  const END = 2082;
  const ENTER = 2054;
  const SPACE = 2050;
  const ESCAPE = 2070;
  const DEL = 2055;
  const TAB = 2049;
  check(EmojiPanelKeyPolicy.decide(key(RIGHT), 20, 8, 3, "").index === 4, "Right moves one item");
  check(
    EmojiPanelKeyPolicy.decide(key(RIGHT), 20, 8, 19, "").index === 19,
    "Right stops at the last item",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(LEFT), 20, 8, 0, "").index === 0,
    "Left stops at the first item",
  );
  check(EmojiPanelKeyPolicy.decide(key(DOWN), 20, 8, 3, "").index === 11, "Down moves one row");
  check(
    EmojiPanelKeyPolicy.decide(key(DOWN), 20, 8, 15, "").index === 19,
    "Down from the last full row lands on the last item",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(UP), 20, 8, 5, "").index === 0,
    "Up from the first row goes to the first item",
  );
  check(EmojiPanelKeyPolicy.decide(key(UP), 20, 8, 13, "").index === 5, "Up moves one row");
  check(
    EmojiPanelKeyPolicy.decide(key(HOME), 20, 8, 13, "").index === 0,
    "Home selects the first item",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(END), 20, 8, 1, "").index === 19,
    "End selects the last item",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(DOWN), 20, 1, 2, "").index === 3,
    "a one-column list moves one entry per row",
  );
  const enter = EmojiPanelKeyPolicy.decide(key(ENTER), 20, 8, 7, "");
  check(
    enter.action === EmojiPanelKeyAction.ACTIVATE && enter.index === 7,
    "Enter inserts the selection",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(SPACE), 20, 8, 7, "").action === EmojiPanelKeyAction.ACTIVATE,
    "Space inserts the selection",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(ENTER), 0, 8, 0, "").action === EmojiPanelKeyAction.NONE,
    "Enter over an empty panel is the editor's",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(DOWN), 0, 8, 0, "").action === EmojiPanelKeyAction.NONE,
    "an arrow over an empty panel is the editor's",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(ENTER), 5, 8, 12, "").index === 4,
    "a selection past the end is clamped before it is used",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(ESCAPE), 20, 8, 3, "").action === EmojiPanelKeyAction.CLOSE,
    "Esc with no search closes the panel",
  );
  const cleared = EmojiPanelKeyPolicy.decide(key(ESCAPE), 20, 8, 3, "cat");
  check(
    cleared.action === EmojiPanelKeyAction.SEARCH && cleared.query === "",
    "Esc first clears a search",
  );
  const typed = EmojiPanelKeyPolicy.decide(key(2019, 0x63), 20, 8, 3, "");
  check(
    typed.action === EmojiPanelKeyAction.SEARCH && typed.query === "c" && typed.index === 0,
    "a letter starts a search from the first result",
  );
  const erased = EmojiPanelKeyPolicy.decide(key(DEL), 20, 8, 3, "ca");
  check(
    erased.action === EmojiPanelKeyAction.SEARCH && erased.query === "c",
    "Backspace takes one letter off the search",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(DEL), 20, 8, 3, "").action === EmojiPanelKeyAction.NONE,
    "Backspace with no search deletes in the editor",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(2019, 0x63, true), 20, 8, 3, "").action ===
      EmojiPanelKeyAction.NONE,
    "a chord is left alone",
  );
  check(
    EmojiPanelKeyPolicy.decide(key(TAB), 20, 8, 3, "").action === EmojiPanelKeyAction.NONE,
    "Tab is left alone",
  );
  const full = "a".repeat(32);
  const capped = EmojiPanelKeyPolicy.decide(key(2017, 0x61), 20, 8, 3, full);
  check(
    capped.action === EmojiPanelKeyAction.MOVE && capped.query === full,
    "a full search takes the key without growing",
  );
  check(
    EmojiPanelKeyPolicy.matches("😺", "Grinning Cat", "cat"),
    "keywords match without regard to case",
  );
  check(EmojiPanelKeyPolicy.matches("(^_^)", "", "^_^"), "the item's own text matches");
  check(
    !EmojiPanelKeyPolicy.matches("😺", "grinning cat", "dog"),
    "an unrelated search does not match",
  );
});

group("2in1 emoji panel tooltips read the way the Windows tooltips do", () => {
  check(
    EmojiPanelTooltipPolicy.clipboardText("a\tb\r\n\n") === "a b ",
    "tabs and returns become spaces, trailing newlines go",
  );
  check(
    EmojiPanelTooltipPolicy.clipboardText("line\nnext") === "line\nnext",
    "inner newlines stay",
  );
  check(
    EmojiPanelTooltipPolicy.clipboardText("x".repeat(200)) === "x".repeat(200),
    "200 characters are shown whole",
  );
  check(
    EmojiPanelTooltipPolicy.clipboardText("x".repeat(201)) === "x".repeat(200) + "...",
    "a longer entry is cut at 200",
  );
  check(
    EmojiPanelTooltipPolicy.clipboardText("😀".repeat(201)) === "😀".repeat(200) + "...",
    "an emoji counts as one character and is never split",
  );
  check(
    EmojiPanelTooltipPolicy.displayName("grinning face 笑 高兴", "😀") === "笑",
    "the first Chinese keyword names the item",
  );
  check(
    EmojiPanelTooltipPolicy.displayName("grinning  face", "😀") === "grinning",
    "without Chinese the first keyword does",
  );
  check(
    EmojiPanelTooltipPolicy.displayName("  ", "😀") === "😀",
    "no keywords falls back to the item",
  );
  check(
    EmojiPanelTooltipPolicy.displayName("arrow 「箭头」", "→") === "「箭头」",
    "CJK punctuation counts as Chinese, as IsCjk does",
  );
});

group("LocalAsrPolicy", () => {
  check(
    LocalAsrPolicy.usesLocalModel("local", "/data/models/zipformer"),
    "an absolute directory under the local provider is a model",
  );
  check(
    !LocalAsrPolicy.usesLocalModel("system", "/data/models/zipformer"),
    "another provider never loads a local model",
  );
  check(!LocalAsrPolicy.usesLocalModel("local", ""), "no picked model is not a model");
  check(!LocalAsrPolicy.usesLocalModel("local", "models/zipformer"), "a relative path is refused");
  check(
    LocalAsrPolicy.modelDirectory(" /data/m/ ") === "/data/m",
    "the path is trimmed and loses its trailing slash",
  );
  check(LocalAsrPolicy.modelDirectory("/data/\u0000m") === "", "control characters are refused");
  const transducer = LocalAsrPolicy.plan(
    "/m",
    JSON.stringify({
      kind: "online_transducer",
      hotwords: "native",
      modeling_unit: "cjkchar+bpe",
      files: {
        encoder: "e.onnx",
        decoder: "d.onnx",
        joiner: "j.onnx",
        tokens: "tokens.txt",
        bpe_vocab: "bpe.vocab",
      },
    }),
  );
  check(
    transducer !== null && transducer.encoder === "/m/e.onnx",
    "manifest roles resolve inside the model directory",
  );
  check(
    transducer !== null && LocalAsrPolicy.transducerNativeHotwords(transducer),
    "a transducer with a BPE vocabulary takes hotwords natively",
  );
  check(
    transducer !== null && !LocalAsrPolicy.correctsWithPinyin(transducer),
    "native hotwords need no pinyin correction",
  );
  check(
    LocalAsrPolicy.plan(
      "/m",
      JSON.stringify({
        kind: "online_transducer",
        files: { encoder: "../e.onnx", decoder: "d.onnx", joiner: "j.onnx", tokens: "tokens.txt" },
      }),
    ) === null,
    "a path escaping the directory is refused",
  );
  check(
    LocalAsrPolicy.plan(
      "/m",
      JSON.stringify({
        kind: "online_transducer",
        files: { encoder: "/e.onnx", decoder: "d.onnx", joiner: "j.onnx", tokens: "tokens.txt" },
      }),
    ) === null,
    "an absolute manifest path is refused",
  );
  check(
    LocalAsrPolicy.plan(
      "/m",
      JSON.stringify({ kind: "offline_sense_voice", files: { model: "m.onnx", tokens: "t.txt" } }),
    ) === null,
    "SenseVoice without its VAD is incomplete",
  );
  check(
    LocalAsrPolicy.plan("/m", JSON.stringify({ kind: "whisper", files: { model: "m.bin" } })) ===
      null,
    "an unknown kind is refused",
  );
  check(LocalAsrPolicy.plan("/m", "{not json") === null, "a malformed manifest is refused");
  const sense = LocalAsrPolicy.plan(
    "/m",
    JSON.stringify({
      kind: "offline_sense_voice",
      hotwords: "pinyin",
      files: { model: "m.onnx", tokens: "t.txt", vad: "silero.onnx" },
    }),
  );
  check(
    sense !== null && LocalAsrPolicy.correctsWithPinyin(sense),
    "a pinyin manifest asks for post-correction",
  );
  check(
    sense !== null && LocalAsrPolicy.requiredFiles(sense).length === 3,
    "SenseVoice needs model, tokens and VAD",
  );
  const tokens = LocalAsrPolicy.tokenSet("<blk> 0\r\n你 1\n好 2\n\n");
  check(
    tokens.has("你") && tokens.has("<blk>") && !tokens.has("0"),
    "the first column of tokens.txt is the token",
  );
  check(
    LocalAsrPolicy.transducerHotwords(["你好", "你们", "AI/ML  x", "C++"], tokens) ===
      "你好\nAI ML x\n",
    "words with unknown tokens or punctuation are left out",
  );
  check(
    LocalAsrPolicy.transducerHotwords(
      Array.from({ length: 250 }, () => "你好"),
      tokens,
    ).split("\n").length === 201,
    "at most 200 transducer hotwords",
  );
  check(
    LocalAsrPolicy.funAsrHotwords(["甲乙", "", "a,b", "丙丁"]) === "甲乙,丙丁",
    "FunASR hotwords are comma-joined without commas inside",
  );
  check(
    LocalAsrPolicy.funAsrHotwords(Array.from({ length: 40 }, (_, index) => `词${index}`)).split(",")
      .length === 30,
    "at most 30 FunASR hotwords",
  );
  check(LocalAsrPolicy.senseVoiceLanguage("zh-HK") === "yue", "Hong Kong Chinese pins Cantonese");
  check(LocalAsrPolicy.senseVoiceLanguage("ja-JP") === "ja", "Japanese is pinned");
  check(LocalAsrPolicy.senseVoiceLanguage("zh-cn") === "auto", "Mandarin is left to the model");
  check(
    LocalAsrPolicy.threads(0) === 2 &&
      LocalAsrPolicy.threads(8) === 4 &&
      LocalAsrPolicy.threads(3) === 3,
    "threads default to two and cap at four",
  );
  const pcm = new ArrayBuffer(5);
  const view = new DataView(pcm);
  view.setInt16(0, -32768, true);
  view.setInt16(2, 16384, true);
  const samples = LocalAsrPolicy.pcm16ToFloat(pcm);
  check(
    samples.length === 2 && samples[0] === -1 && samples[1] === 0.5,
    "PCM16 scales to floats and drops an odd byte",
  );
  check(
    LocalAsrPolicy.joinSegments(["hello", "world", "", "你好", "ok"]) === "hello world你好ok",
    "a space only between ASCII alphanumerics",
  );
  check(
    LocalAsrPolicy.tidyTranscript(" 你好 ， 世界  A I 模型 ") === "你好，世界 AI 模型",
    "spaces around CJK marks, inside initialisms and at the ends go",
  );
});

group("PcmFrameSlicer", () => {
  const slicer = new PcmFrameSlicer();
  check(slicer.push(new ArrayBuffer(1000)).length === 0, "less than a frame is held back");
  const frames = slicer.push(new Uint8Array(3000).fill(7).buffer);
  check(
    frames.length === 3 && frames.every((frame) => frame.length === 1280),
    "whole 1280-byte frames come out",
  );
  check(frames[0][999] === 0 && frames[0][1000] === 7, "the held bytes lead the next frame");
  const tail = slicer.flush();
  check(
    tail !== null && tail.length === 1280 && tail[159] === 7 && tail[160] === 0,
    "the tail is padded with silence",
  );
  check(slicer.flush() === null, "nothing pending flushes nothing");
  slicer.push(new ArrayBuffer(10));
  slicer.reset();
  check(slicer.flush() === null, "reset drops the pending bytes");
});

group("SpeechSentenceAccumulator", () => {
  const sentences = new SpeechSentenceAccumulator();
  check(sentences.accept("你好", false) === "你好", "a partial sentence is shown alone");
  check(sentences.accept("你好。", true) === "你好。", "a closed sentence is kept");
  check(
    sentences.accept("再见", false) === "你好。再见",
    "the next sentence follows the closed ones",
  );
  check(sentences.accept("再见。", true) === "你好。再见。", "every closed sentence stays");
  sentences.reset();
  check(sentences.accept("新", false) === "新", "reset starts a new session");
});

group("LocalVoiceModelPolicy", () => {
  check(
    LocalVoiceModelPolicy.root("/data/storage/el2/base/haps/entry/files") ===
      "/data/storage/el2/base/haps/entry/files/voice-models",
    "models live under the files directory the keyboard shares",
  );
  // The core's Display text carries detail after the code; the page matches on the code alone.
  check(
    LocalVoiceModelPolicy.errorCode("local_model_network: dns error") === "local_model_network",
    "the detail after the code is dropped",
  );
  check(
    LocalVoiceModelPolicy.errorCode("local_model_size_mismatch: model.onnx") ===
      "local_model_checksum_mismatch",
    "a size mismatch reads as a failed verification, as on desktop",
  );
  check(
    LocalVoiceModelPolicy.errorCode("local_model_unsafe_archive: ../x") ===
      "local_model_invalid_archive" &&
      LocalVoiceModelPolicy.errorCode("local_model_missing_file: tokens.txt") ===
        "local_model_invalid_archive",
    "an unsafe or incomplete archive is an invalid archive",
  );
  check(
    LocalVoiceModelPolicy.errorCode("local_model_install_running") === "busy",
    "a second install of the same model is busy",
  );
  check(
    LocalVoiceModelPolicy.errorCode("local_model_cancelled") === "local_model_cancelled",
    "a cancel keeps its code so the page does not report it as a failure",
  );
  check(
    LocalVoiceModelPolicy.errorCode("invalid local model root") === "local_model_invalid_root",
    "the ABI's own root refusal is the invalid-root code",
  );
  check(
    LocalVoiceModelPolicy.errorCode("internal runtime failure") === "local_model_failed" &&
      LocalVoiceModelPolicy.errorCode("") === "local_model_failed",
    "anything unnamed is the general failure",
  );
  check(
    LocalVoiceModelPolicy.rewrite(JSON.stringify({ ok: true, value: true })) ===
      JSON.stringify({ ok: true, value: true }),
    "an accepted reply is left alone",
  );
  check(
    LocalVoiceModelPolicy.rewrite("not json") ===
      JSON.stringify({ ok: false, error: "local_model_failed" }),
    "an unreadable reply is a refusal",
  );

  const listed = JSON.parse(
    LocalVoiceModelPolicy.listReply(
      JSON.stringify({
        ok: true,
        value: {
          models: [
            { id: "x-asr-zh-en-streaming", desktop_only: false, installed: false, title: "X" },
            { id: "fun-asr-nano", desktop_only: true, installed: false },
            { id: "kept", desktop_only: true, installed: true },
          ],
          default: "x-asr-zh-en-streaming",
        },
      }),
      "/files/voice-models",
    ),
  );
  check(
    listed.ok === true &&
      listed.value.root === "/files/voice-models" &&
      listed.value.default === "x-asr-zh-en-streaming",
    "the list carries the root and the default",
  );
  check(
    listed.value.models.map((model: { id: string }) => model.id).join(",") ===
      "x-asr-zh-en-streaming,kept",
    "a desktop-only model is offered only once installed, so it can still be removed",
  );
  check(listed.value.models[0].title === "X", "the other catalog fields pass through");
  check(
    LocalVoiceModelPolicy.listReply(
      JSON.stringify({ ok: false, error: "invalid local model root" }),
      "/r",
    ) === JSON.stringify({ ok: false, error: "local_model_invalid_root" }),
    "a refused list is mapped",
  );

  check(
    LocalVoiceModelPolicy.installReply(
      JSON.stringify({ ok: true, value: { path: "/files/voice-models/x" } }),
    ) === JSON.stringify({ ok: true, value: "/files/voice-models/x", error: "" }),
    "an install answers with the installed directory",
  );
  check(
    LocalVoiceModelPolicy.installReply(
      JSON.stringify({ ok: false, error: "local_model_http_status: 404" }),
    ) === JSON.stringify({ ok: false, error: "local_model_http_status" }),
    "a refused install is mapped",
  );
  check(
    LocalVoiceModelPolicy.installReply(JSON.stringify({ ok: true, value: {} })) ===
      JSON.stringify({ ok: false, error: "local_model_failed" }),
    "an install without a path is not a success",
  );

  check(
    LocalVoiceModelPolicy.mirror(
      JSON.stringify({
        ok: true,
        value: {
          revision: 3,
          preferences: { voice_input: { asr_model_mirror: " https://m.example/ " } },
        },
      }),
    ) === "https://m.example/",
    "the saved mirror is read and trimmed",
  );
  check(
    LocalVoiceModelPolicy.mirror(JSON.stringify({ ok: false, error: "storage" })) === "" &&
      LocalVoiceModelPolicy.mirror("{") === "",
    "unreadable preferences download from the catalog URLs",
  );

  check(LocalVoiceModelPolicy.action('{"operation":"list"}') !== null, "a list needs no id");
  check(
    LocalVoiceModelPolicy.action('{"operation":"install","id":"sense-voice-small"}')?.id ===
      "sense-voice-small",
    "an install names its model",
  );
  check(
    LocalVoiceModelPolicy.action('{"operation":"remove"}') === null &&
      LocalVoiceModelPolicy.action('{"operation":"format","id":"x"}') === null &&
      LocalVoiceModelPolicy.action("[") === null,
    "a missing id, an unknown operation or unreadable text is refused",
  );
});
