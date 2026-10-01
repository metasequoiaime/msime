package app.msime.android;

import android.inputmethodservice.InputMethodService;
import app.msime.android.core.Telemetry;
import android.app.AlertDialog;
import android.content.ClipDescription;
import android.content.Intent;
import android.content.ClipboardManager;
import android.content.SharedPreferences;
import android.content.res.Configuration;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.InsetDrawable;
import android.graphics.drawable.StateListDrawable;
import android.media.AudioManager;
import android.os.Handler;
import android.os.Looper;
import android.os.Build;
import android.os.SystemClock;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ForegroundColorSpan;
import android.text.style.RelativeSizeSpan;
import android.view.Gravity;
import android.view.HapticFeedbackConstants;
import android.view.KeyEvent;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.util.TypedValue;
import android.widget.PopupMenu;
import android.widget.PopupWindow;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.GridLayout;
import android.widget.LinearLayout;
import android.widget.SeekBar;
import android.widget.ScrollView;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.HorizontalScrollView;
import android.widget.Toast;
import app.msime.android.candidate.EnglishSuggestionModel;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.keyboard.EnglishSuggestionPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.nio.file.Path;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Preview system host. No algorithms, pagination state or persistent input logs live here. */
public final class MSIMEInputService extends InputMethodService {
    private static final int STANDARD_TOUCH_LAYOUT = 0;
    private static final int QUANPIN_NINE_KEY_LAYOUT = 1;
    private static final int JAPANESE_NINE_KEY_LAYOUT = 2;
    private static final int HANDWRITING_LAYOUT = 3;
    private static final long HANDWRITING_DEBOUNCE_MILLIS = 550;
    private static final long BACKSPACE_REPEAT_DELAY_MILLIS = 400;
    private static final long BACKSPACE_REPEAT_INTERVAL_MILLIS = 75;
    private static final long PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS = 500;
    private static final long INPUT_VIEW_REFRESH_DELAY_MILLIS = 32;
    /** How long counted key presses wait in memory before they are written anyway. */
    private static final long KEY_PRESS_FLUSH_DELAY_MILLIS = 30_000;
    private static final String SCHEME_HOST_PREFERENCES = "android-keyboard-schemes";
    private static final String INPUT_MODE_PREFERENCES = "android-input-modes";
    private static final String SELECTED_HOST_SCHEME = "selected-scheme";
    private static final String THOUGHTFUL_REPLY_ENABLED = "thoughtful-reply-enabled";
    private static final String EMOJI_RECENTS_PREFERENCES = "android-emoji-recents";
    private static final String EMOJI_RECENTS_KEY = "items";
    private static final String SPACE_CURSOR_DESCRIPTION =
        "空格；轻点输入空格或选词，左右滑动移动光标";
    private static final int CAPITALIZATION_CONTEXT_LIMIT = 128;
    /**
     * Shared host command 9, `Action::Finish`: commit the highlighted candidate and whatever the
     * composition still holds. This is Apple's `finishComposition`, not `CommitRaw`, so a
     * composition ended this way reaches the editor as 你好 rather than as the pinyin letters.
     */
    private static final int FINISH_COMPOSITION_COMMAND = 9;
    /** Shared host command 10 delegates small-kana, dakuten and handakuten cycling to Engine. */
    private static final int CYCLE_KANA_VARIANT_COMMAND = 10;
    private long session;
    private InputConnection connection;
    private EditorBridge bridge = new EditorBridge();
    private JSONObject view;
    private FrameLayout keyboardRoot;
    private FrameLayout keyboardSurface;
    private LinearLayout candidates;
    private LinearLayout verticalCandidates;
    private FrameLayout candidateViewport;
    private HorizontalScrollView horizontalCandidateScroll;
    private ScrollView verticalCandidateScroll;
    private final java.util.List<Button> candidateButtons = new java.util.ArrayList<>();
    private final java.util.List<Button> englishSuggestionButtons = new java.util.ArrayList<>();
    private LinearLayout candidatePaging;
    private HorizontalScrollView candidatePagingScroll;
    private LinearLayout expandedCandidates;
    private ScrollView expandedCandidateScroll;
    private TextView preedit;
    private TextView candidatePage;
    private KeyboardBrandMark candidateBrandMark;
    private Button exitLocalModeButton;
    /** 漢 in the candidate header: converts the composing Korean syllable to Hanja, or closes its list. */
    private Button hanjaButton;
    private LinearLayout nineKeySpellings;
    private HorizontalScrollView nineKeySpellingScroll;
    private final java.util.List<Button> nineKeySpellingButtons = new java.util.ArrayList<>();
    private java.util.List<Integer> nineKeySpellingIndices = java.util.List.of();
    private long nineKeySpellingGeneration = -1;
    private long candidateScrollSession = -1;
    private long candidateScrollGeneration = -1;
    private int candidateScrollPage = -1;
    private Button expandCandidates;
    private boolean candidatePanelOpen;
    private JSONObject candidatePanelSnapshot;
    private PopupWindow nineKeyHoldPopup;
    private ScrollView clipboardScroll;
    private LinearLayout clipboardPanel;
    private ScrollView schemeScroll;
    private LinearLayout schemePanel;
    private ScrollView skinScroll;
    private LinearLayout skinPanel;
    private ScrollView layoutSettingsScroll;
    private LinearLayout layoutSettingsPanel;
    private KeyboardLayoutAdjustView layoutAdjustView;
    private ScrollView moreToolsScroll;
    private LinearLayout moreToolsPanel;
    private boolean localInputToolsOpen;
    private LinearLayout emojiPanel;
    private HorizontalScrollView emojiTabsScroll;
    private LinearLayout emojiTabs;
    private ScrollView emojiGridScroll;
    private GridLayout emojiGrid;
    private TextView emojiStatus;
    private SeekBar keySpacingSlider;
    private SeekBar rowSpacingSlider;
    private SeekBar keyboardHeightSlider;
    private Switch voiceShortcutSwitch;
    private Button resetLayoutSettingsButton;
    private TextView keySpacingValue;
    private TextView rowSpacingValue;
    private TextView keyboardHeightValue;
    private ClipboardHistoryStore clipboardHistory;
    private boolean clipboardHistoryEnabled;
    private CloudClipboardPanelPolicy.Tab clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
    private CloudClipboardPanelPolicy.Status cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    private java.util.List<BackendAccount.ClipboardItem> cloudClipboardItems = java.util.List.of();
    // Bumped whenever the field or the open panel changes; a cloud answer started under an older value is dropped rather than drawn into a field it was not fetched for.
    private long cloudClipboardGeneration;
    private boolean candidateEnglishGloss;
    private boolean candidateTranslationsEnabled;
    // Gates only the account network path (fetch, apply, reserved rows); candidateTranslationsEnabled stays the display gate for translations a candidate already carries.
    private boolean candidateTranslationAccount;
    private boolean englishSuggestionsEnabled = true;
    private java.util.List<String> candidateTranslationTargets = java.util.List.of("en");
    private CandidateTranslationStore candidateTranslationStore;
    private boolean wubiCodeHint = true;
    private boolean wubiMixedPinyin;
    private String candidateGlossResources = "";
    private String onlineSignature = "";
    /** Read from the online worker as an early-out, so it must not tear across threads. */
    private volatile long onlineEpoch;
    private Runnable onlineTask;
    private long candidateGlossEpoch;
    private long candidateGlossRequestedSession;
    private long candidateGlossRequestedGeneration = -1;
    private String candidateOfflineTargetsKey;
    private java.util.List<String> candidateOfflineTargets = java.util.List.of();
    // Offline glosses by target language, English included, for one candidate generation. Kept only while a non-English dictionary is installed for the targets, so the account path can merge with them: applying a translation payload replaces the previous one.
    private java.util.Map<String, java.util.Map<String, String>> candidateOfflineGlosses;
    private long candidateOfflineGlossSession;
    private long candidateOfflineGlossGeneration = -1;
    private java.util.List<String> englishSuggestions = java.util.List.of();
    private String englishSuggestionPrefix = "";
    private long englishSuggestionEpoch;
    private long englishSuggestionRequestedEpoch = -1;
    private String englishSuggestionRequestedPrefix = "";
    private boolean candidateHorizontal;
    private int candidateFontSize = 16;
    private int candidatePreeditFontSize = 16;
    private CandidateAppearance.Palette candidateAppearance =
        CandidateAppearance.fromSkin(KeyboardSkin.system(false));
    private int touchKeySpacingTenths = KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS;
    private int touchRowSpacingTenths = KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS;
    private int touchKeyboardHeightAdjustment = KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_DP;
    private boolean touchVoiceShortcutEnabled;
    private boolean voiceInputEnabled = true;
    private String voiceLanguage = "zh-CN";
    private KeyboardSkin skin = KeyboardSkin.system(false);
    /** 表情面板有自己的明暗设置，跟随时才继承键盘皮肤解析出的明暗。 */
    private KeyboardSkin emojiSkin = KeyboardSkin.system(false);
    private KeyboardSkin handwritingSkin = KeyboardSkin.system(false);
    /** The shared theme catalog (`msime_client_theme_catalog`), read once: ids, titles and palettes are fixed per build. */
    private JSONArray themeCatalog;
    /** Resolved keyboards by request, so the four surfaces of one snapshot cost at most two native calls. */
    private final java.util.Map<String, KeyboardSkin> resolvedThemes = new java.util.HashMap<>();
    private JSONObject localModes = new JSONObject();
    private Button moreButton;
    private Button schemeButton;
    private Button skinButton;
    private Button layoutSettingsButton;
    private Button scriptShortcutButton;
    private Button emojiShortcutButton;
    private Button voiceShortcutButton;
    private Button aiPolishShortcutButton;
    private Button replyShortcutButton;
    private Button microsoftFinalKey;
    private final java.util.List<ShuangpinHintButton> shuangpinKeyButtons =
        new java.util.ArrayList<>();
    private final java.util.List<String> shuangpinKeyInputs = new java.util.ArrayList<>();
    private String shuangpinHintsProfile = "";
    private java.util.Map<String, String> shuangpinHints = java.util.Map.of();
    private KeyboardScheme selectedScheme = KeyboardScheme.QUANPIN;
    private java.util.List<KeyboardScheme> enabledSchemes =
        KeyboardScheme.enabledFromPreferenceIds(null);
    private boolean sharedSchemePreferences;
    private SharedPreferences schemeHostPreferences;
    private boolean soundEnabled = true;
    private boolean hapticsEnabled;
    private KeyboardFeedbackPreferences.HapticStrength hapticStrength =
        KeyboardFeedbackPreferences.HapticStrength.MEDIUM;
    private Vibrator vibrator;
    private LinearLayout keyRows;
    /** The fixed bottom row; {@link KeyboardActionRow} decides what it carries. */
    private LinearLayout actionRow;
    private Button globeButton;
    private Button deleteButton;
    private View nineKeySidebar;
    private String actionRowSignature = "";
    private boolean brandPillVisible;
    private JapaneseFlickPreview japaneseFlickPreview;
    private LinearLayout shortcutBar;
    private HorizontalScrollView shortcutScroll;
    private final java.util.List<Button> symbolKeyButtons = new java.util.ArrayList<>();
    private final java.util.List<String> symbolKeyInputs = new java.util.ArrayList<>();
    private HandwritingCanvas handwritingCanvas;
    private LinearLayout handwritingCandidates;
    private TextView handwritingStatus;
    private Button handwritingDownload;
    private HandwritingRecognizer handwritingRecognizer;
    private Runnable handwritingRecognitionTask;
    private Runnable handwritingAvailabilityTask;
    private boolean handwritingDownloading;
    private java.util.List<String> handwritingResults = java.util.List.of();
    private HandwritingRequestTracker.Token handwritingCandidateToken;
    private final HandwritingRequestTracker handwritingRequests = new HandwritingRequestTracker();
    private final SpaceCursorMovement cursorMovement = new SpaceCursorMovement();
    private Button layerButton;
    private Button symbolPanelButton;
    private Button quickPunctuationButton;
    private Button shiftButton;
    private Button languageButton;
    private Button enterButton;
    private Button spaceButton;
    private Button japaneseSpaceKey;
    private Button japaneseReturnKey;
    private Button japaneseSymbolsKey;
    private Button japaneseVariantsButton;
    private Integer japaneseConversionIndex;
    private String japaneseConversionEditingText = "";
    private TextView status;
    private TextView diagnosticView;
    private String message = "MSIME Preview";
    private String diagnosticMessage = "";
    private long diagnosticGeneration;
    private Runnable diagnosticDismissTask;
    private final EnglishLetterCaseState letterCase = new EnglishLetterCaseState();
    private boolean dedicatedEnglish;
    private boolean hardwareLanguageShift = true;
    private boolean hardwareLanguageCtrlAltSpace = true;
    private boolean hardwareCharacterSet = true;
    private boolean hardwareFullWidth = true;
    private boolean numberRowSelection = true;
    /** Resolved 以词定字 binding: `disabled`, `brackets` or `minus_equal`. */
    private String wordCharacterBinding = WordCharacterPolicy.DISABLED;
    /** 「候选栏预编辑」: whether the strip draws what is being spelled. */
    private String candidatePreeditStyle = CandidatePreeditStylePolicy.PINYIN;
    /** Which hardware keys page the candidate list, from the shared `navigation` preferences. */
    private CandidateNavigationPolicy.Bindings candidateNavigation =
        CandidateNavigationPolicy.Bindings.defaults();
    private boolean hardwareLanguageCtrl;
    private long modifierTapStartedAt = -1;
    private int modifierTapKeyCode = -1;
    private boolean modifierTapInvalid;
    private String defaultImeMode = "chinese";
    private String imeModeScope = "app";
    private String currentEditorPackage = "";
    private InputModeStore inputModeStore;
    /** Mirrors the runtime's own character width; the toolbar card and the chord move it. */
    private boolean fullWidthInput;
    /** The shared preference this session started from, so only a change to it overrides the toggle. */
    private boolean fullWidthPreference;
    /** Mirrors the runtime's Chinese/English punctuation state; the card and the chord move it. */
    private boolean chinesePunctuation = true;
    private boolean chinesePunctuationPreference = true;
    private boolean traditionalChineseOutput;
    private int editorInputType;
    private long currentDocumentIdentifier;
    private long nextDocumentIdentifier = 1;
    private final KeyboardInputContext inputContext = new KeyboardInputContext();
    private KeyboardLayout.Layer keyboardLayer = KeyboardLayout.Layer.LETTERS;
    private boolean allowLearning;
    private String preferencesNotice = "";
    private String preferencesDirectory = "";
    private long appearanceLoadGeneration;
    private String runtimeOptionsForSnapshot = "";
    private JSONObject preferencesSnapshot;
    private long preferenceSaveGeneration;
    private boolean schemeSaving;
    private boolean touchGeometrySaving;
    private boolean skinSaving;
    private boolean traditionalOutputSaving;
    private static final class KeyboardHeightRole {
        final int baseHeight;
        final int rowCount;
        final int rowIndex;
        final boolean includesRowSpacing;

        KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex,
                           boolean includesRowSpacing) {
            this.baseHeight = baseHeight;
            this.rowCount = rowCount;
            this.rowIndex = rowIndex;
            this.includesRowSpacing = includesRowSpacing;
        }
    }
    private ScrollView voiceResultScroll;
    private LinearLayout voiceResultPanel;
    private VoiceResultStore voiceResultStore;
    private VoiceResultStore.Entry voiceResultEntry;
    private EditorContextSnapshot voiceTarget;
    private ScrollView aiPolishScroll;
    private LinearLayout aiPolishContainer;
    private LinearLayout aiPolishPanel;
    private LinearLayout aiPolishActions;
    private AiPolishConfiguration aiPolishConfiguration;
    private AiPolishConfiguration aiRequestConfiguration;
    private AiPolishClient.Operation aiOperation;
    private EditorContextSnapshot aiTarget;
    private String aiSourceText = "";
    private String aiOutputText = "";
    private String aiError = "";
    private boolean aiBusy;
    private LinearLayout replyKeyboard;
    private LinearLayout replyMain;
    private LinearLayout replyActions;
    private TextView replyStatus;
    private Button replyReplyModeButton;
    private Button replyPolishModeButton;
    private Button replySourceButton;
    private Button replyTemplateButton;
    private CommunityReplyLibrary communityReplyLibrary;
    private final ReplyKeyboardModel replyModel = new ReplyKeyboardModel();
    private AiPolishClient.Operation replyOperation;
    private EditorContextSnapshot replyTarget;
    private AiPolishConfiguration replyRequestConfiguration;
    private boolean replySuppressed;
    private boolean statisticsFailureReported;
    /** Key presses since the last write, per key and local day. Main thread only. */
    private final KeyPressBatch keyPresses = new KeyPressBatch();
    /** Each soft key's heatmap id. Weak, so the keys of a rebuilt row leave with it. */
    private final java.util.Map<View, String> keyIds = new java.util.WeakHashMap<>();
    /** The store's statistics switch as last read; until it has been read, nothing is counted. */
    private boolean keyStatisticsEnabled;
    /** A password or no-learning field, where no key press is counted. */
    private boolean keyStatisticsExcluded = true;
    private String keyStatisticsDirectory = "";
    private long keyStatisticsGeneration;
    private Runnable keyPressFlushTask;
    private long editorContextRevision;
    /** Editor-owned smart-punctuation snapshots; never persisted or sent to the UI. */
    private JSONObject smartRepeatSnapshot;
    private JSONObject smartSpaceSnapshot;
    private SharedPreferences emojiPreferences;
    private java.util.List<String> emojiRecents = java.util.List.of();
    private java.util.List<EmojiCatalogModel.Item> emojiItems = java.util.List.of();
    private String emojiResources = "";
    private SymbolPanelView symbolPanel;
    private int emojiSelectedCategory = Integer.MIN_VALUE;
    private int emojiNextOffset;
    private boolean emojiComplete;
    private boolean emojiLoading;
    private long emojiLoadGeneration;
    private final Handler main = new Handler(Looper.getMainLooper());
    private Runnable backspaceRepeatTask;
    private Button backspaceRepeatButton;
    private boolean backspaceRepeated;
    private boolean backspaceClearedComposition;
    private long personalDictionarySyncGeneration;
    private Runnable personalDictionarySyncTask;
    private long engineStartGeneration;
    private Runnable inputViewRefreshTask;
    private final ExecutorService preferencesWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService typingStatisticsWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(32),
        new ThreadPoolExecutor.AbortPolicy());
    private final ExecutorService emojiWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService cloudClipboardWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService candidateGlossWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(1),
        new ThreadPoolExecutor.DiscardOldestPolicy());
    private final ExecutorService candidateTranslationWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(1),
        new ThreadPoolExecutor.DiscardOldestPolicy());
    private final ExecutorService englishSuggestionWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(1),
        new ThreadPoolExecutor.DiscardOldestPolicy());
    private final ExecutorService onlineCandidateWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(1),
        new ThreadPoolExecutor.DiscardOldestPolicy());
    private final AiPolishClient aiPolishClient = new AiPolishClient(new AiPolishHttpTransport());
    private final PreferencesReloader preferencesReloader = new PreferencesReloader(
        (task, delay) -> main.postDelayed(task, delay), preferencesWorker, NativeClient::loadPreferences);

    private boolean legacyThoughtfulReplyEnabled() {
        return schemeHostPreferences == null
            || schemeHostPreferences.getBoolean(THOUGHTFUL_REPLY_ENABLED, true);
    }

    private boolean thoughtfulReplyEnabled() {
        return sharedSchemePreferences
            ? enabledSchemes.contains(KeyboardScheme.THOUGHTFUL_REPLY)
            : legacyThoughtfulReplyEnabled();
    }

    private KeyboardScheme hostScheme(KeyboardScheme engineScheme) {
        String stored = schemeHostPreferences == null ? null
            : schemeHostPreferences.getString(SELECTED_HOST_SCHEME, null);
        KeyboardScheme resolved = KeyboardScheme.fromHostSelection(
            stored, legacyThoughtfulReplyEnabled(), engineScheme);
        if (resolved != KeyboardScheme.THOUGHTFUL_REPLY && stored != null
                && !resolved.name().equals(stored)) saveHostScheme(resolved);
        return resolved;
    }

    private void saveHostScheme(KeyboardScheme scheme) {
        if (schemeHostPreferences != null)
            schemeHostPreferences.edit().putString(SELECTED_HOST_SCHEME, scheme.name()).apply();
    }

    private record SchemeConfiguration(
        java.util.List<KeyboardScheme> enabled, KeyboardScheme selected, boolean shared) {}

    private record SkinChoice(String id, String title, KeyboardSkin skin, JSONObject design) {}

    private SchemeConfiguration schemeConfiguration(
            JSONObject preferences, KeyboardScheme engineScheme) {
        JSONObject shared = preferences == null ? null
            : preferences.optJSONObject("touch_keyboard_schemes");
        if (shared == null) {
            java.util.List<String> legacyIds = new java.util.ArrayList<>();
            for (KeyboardScheme candidate : KeyboardScheme.values()) {
                if (candidate != KeyboardScheme.THOUGHTFUL_REPLY
                        || legacyThoughtfulReplyEnabled()) {
                    legacyIds.add(candidate.preferenceId());
                }
            }
            return new SchemeConfiguration(
                KeyboardScheme.enabledFromPreferenceIds(legacyIds), hostScheme(engineScheme), false);
        }
        JSONArray values = shared.optJSONArray("enabled");
        java.util.List<String> ids = new java.util.ArrayList<>();
        if (values != null) {
            for (int index = 0; index < values.length(); index++) {
                String value = values.isNull(index) ? null : values.optString(index, null);
                if (value != null) ids.add(value);
            }
        }
        java.util.List<KeyboardScheme> enabled = KeyboardScheme.enabledFromPreferenceIds(ids);
        String selected = shared.isNull("selected") ? null : shared.optString("selected", null);
        return new SchemeConfiguration(enabled,
            KeyboardScheme.resolveEnabledSelection(engineScheme, selected, enabled), true);
    }

    /**
     * Everything about this keyboard that the user chose and can see: the skin, the scheme badge,
     * the candidate strip and the key geometry.
     *
     * <p>Applied for every editor, not only the ones that take the Engine. A password box, a number
     * field and the placeholder editor the system sends first after a cold start all draw this
     * keyboard, and drawing them in the factory skin makes it look like a different input method.
     */
    private void applyEditorPreferences(JSONObject preferences) throws JSONException {
        numberRowSelection = preferences == null
            || preferences.optBoolean("number_row_selection", true);
        // The width a session starts at. Applied to the runtime once there is one to tell; this
        // method also runs for editors that never get a session, and those only commit directly.
        fullWidthPreference = preferences != null && CharacterWidthPolicy.preferenceIsFullWidth(
            preferences.optString(CharacterWidthPolicy.PREFERENCE_KEY, "halfwidth"));
        if (session == 0) fullWidthInput = fullWidthPreference;
        chinesePunctuationPreference = preferences == null
            || preferences.optBoolean("chinese_punctuation", true);
        if (session == 0) chinesePunctuation = chinesePunctuationPreference;
        wordCharacterBinding = wordCharacterBindingFrom(preferences);
        candidatePreeditStyle = CandidatePreeditStylePolicy.style(preferences == null ? null
            : preferences.optString("candidate_preedit_style", CandidatePreeditStylePolicy.PINYIN));
        candidateNavigation = candidateNavigationFrom(
            preferences == null ? null : preferences.optJSONObject("navigation"));
        JSONObject keybindings = preferences == null ? null : preferences.optJSONObject("keybindings");
        hardwareLanguageShift = keybindings == null || keybindings.optBoolean("switch_language_shift", true);
        hardwareLanguageCtrlAltSpace = keybindings == null
            || keybindings.optBoolean("switch_language_ctrl_alt_space", true);
        hardwareCharacterSet = keybindings == null
            || keybindings.optBoolean("toggle_character_set_ctrl_shift_f", true);
        hardwareFullWidth = keybindings == null
            || keybindings.optBoolean("toggle_fullwidth_option_shift_h", true);
        hardwareLanguageCtrl = keybindings != null
            && keybindings.optBoolean("switch_language_ctrl", false);
        defaultImeMode = preferences != null
            && "english".equals(preferences.optString("default_ime_mode", "chinese"))
            ? "english" : "chinese";
        imeModeScope = preferences != null
            && "global".equals(preferences.optString("ime_mode_scope", "app"))
            ? "global" : "app";
        KeyboardScheme engineScheme = KeyboardScheme.fromPreferences(
            preferences == null ? "quanpin" : preferences.optString("scheme", "quanpin"),
            preferences == null ? "xiaohe" : preferences.optString("shuangpin_profile", "xiaohe"),
            preferences == null ? "twenty_six_key"
                : preferences.optString("touch_keyboard_layout", "twenty_six_key"));
        SchemeConfiguration schemeConfiguration = schemeConfiguration(preferences, engineScheme);
        alignEngineSchemeWithSelection(preferences, engineScheme, schemeConfiguration);
        enabledSchemes = schemeConfiguration.enabled();
        selectedScheme = schemeConfiguration.selected();
        sharedSchemePreferences = schemeConfiguration.shared();
        skin = keyboardSkin(preferences);
        emojiSkin = surfaceSkin(preferences, "emoji_theme");
        handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
        localModes = preferences == null ? new JSONObject()
            : preferences.optJSONObject("local_modes");
        if (localModes == null) localModes = new JSONObject();
        applyCandidateAppearance(preferences);
        applyTouchGeometry(preferences);
        applyVoicePreferences(preferences);
        applyAiPreferences(preferences);
        applyClipboardPreference(preferences);
        applyChineseOutputPreference(preferences);
        applyCandidateGlossPreference(preferences);
        applyEnglishSuggestionsPreference(preferences);
        applyCandidateTranslationPreference(preferences);
        applyWubiCodeHintPreference(preferences);
        wubiMixedPinyin = preferences != null
            && preferences.optBoolean("wubi_mixed_pinyin", false);
    }

    /**
     * Read the live preferences for an editor that has no Engine session, and repaint.
     *
     * <p>{@link #applyPreferencesSnapshot} cannot serve this case: it hands the snapshot to the
     * Engine and reads the view back, which needs a session. This applies the visible half only and
     * touches no Engine state. Failure keeps whatever is on screen -- the user asked for a text
     * field, not for a report about preferences.
     */
    private void loadAppearanceWithoutSession(String directory) {
        if (directory.isEmpty() || !new File(directory).isAbsolute()) return;
        long generation = ++appearanceLoadGeneration;
        try {
            preferencesWorker.execute(() -> {
                final String response;
                try {
                    response = NativeClient.loadPreferences(directory);
                } catch (RuntimeException | LinkageError error) {
                    return;
                }
                main.post(() -> {
                    if (generation != appearanceLoadGeneration || session != 0) return;
                    try {
                        applyEditorPreferences(value(response).optJSONObject("preferences"));
                        render();
                    } catch (JSONException | LinkageError ignored) {
                        // Unreadable preferences leave the keyboard as it is.
                    }
                });
            });
        } catch (RuntimeException ignored) {
            // A worker shutdown just means this editor keeps the appearance it already has.
        }
    }

    /** Keep a shared picker selection and the Engine session on the same scheme after a fallback. */
    private void alignEngineSchemeWithSelection(
            JSONObject preferences, KeyboardScheme engineScheme,
            SchemeConfiguration configuration) throws JSONException {
        if (preferences == null || !configuration.shared()
                || configuration.selected() == KeyboardScheme.THOUGHTFUL_REPLY
                || configuration.selected() == engineScheme) return;
        KeyboardScheme.PreferenceMapping mapping = KeyboardScheme.mappingForRuntimeSelection(
            engineScheme, configuration.selected(),
            preferences.optString("last_chinese_scheme", preferences.optString("scheme", "quanpin")),
            preferences.optString("shuangpin_profile", "xiaohe"));
        preferences.put("scheme", mapping.scheme());
        preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
        preferences.put("shuangpin_profile", mapping.shuangpinProfile());
        preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
    }

    private JSONObject value(String response) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!envelope.getBoolean("ok")) throw new JSONException("Shared runtime rejected operation");
        return envelope.getJSONObject("value");
    }

    private EditorBridge.Sink sink(TypingSource source) {
        final InputConnection target = connection;
        return new EditorBridge.Sink() {
            public void begin() { target.beginBatchEdit(); }
            public boolean commit(String text) {
                boolean committed = target.commitText(text, 1);
                if (committed) recordTypingStatistics(text, source);
                return committed;
            }
            public boolean compose(String text) { return target.setComposingText(text, 1); }
            public boolean finish() { return target.finishComposingText(); }
            public void end() { target.endBatchEdit(); }
        };
    }

    private TypingSource typingSource() {
        return TypingSource.resolve(selectedScheme, dedicatedEnglish,
            view == null || view.isNull("local_mode") ? null : view.optString("local_mode", null));
    }

    private String typingStatisticsDirectory() {
        if (!preferencesDirectory.isEmpty()) return preferencesDirectory;
        File files = getFilesDir();
        return files == null ? "" : new File(files, "bootstrap/state").getAbsolutePath();
    }

    private void recordTypingStatistics(String text, TypingSource source) {
        String directory = typingStatisticsDirectory();
        if (directory.isEmpty() || text == null || text.isEmpty()) return;
        final String request;
        // One instant for both fields: a day and an hour read separately either side of
        // midnight would file the commit under one day and the other day's hour.
        final LocalDateTime instant = LocalDateTime.now();
        try {
            request = new JSONObject().put("directory", directory).put("action",
                new JSONObject().put("operation", "record").put("text", text)
                    .put("source", source.id()).put("day", instant.toLocalDate().toString())
                    .put("hour", instant.getHour()))
                .toString();
        } catch (JSONException error) {
            reportTypingStatisticsFailure();
            return;
        }
        submitTypingStatistics(request, null);
    }

    /**
     * Send one statistics request on the worker; {@code nothingRecorded}, when given, runs on the main thread if the store took none of a non-empty batch, which is how it answers once statistics are off.
     */
    private void submitTypingStatistics(String request, Runnable nothingRecorded) {
        try {
            typingStatisticsWorker.execute(() -> {
                try {
                    JSONObject result = new JSONObject(NativeClient.typingStatistics(request));
                    if (!result.getBoolean("ok")) {
                        reportTypingStatisticsFailure();
                    } else if (nothingRecorded != null
                        && result.getJSONObject("value").getLong("recorded") == 0) {
                        main.post(nothingRecorded);
                    }
                } catch (Exception | LinkageError error) {
                    reportTypingStatisticsFailure();
                }
            });
        } catch (RuntimeException error) {
            reportTypingStatisticsFailure();
        }
    }

    /**
     * Where key counts go: the preferences directory the settings page reads them back from, or the bootstrap state root when there is none, in the order HostStore.statisticsDirectory reads them.
     */
    private String keyStatisticsDirectory(String preferences) {
        if (!preferences.isEmpty() && new File(preferences).isAbsolute()) return preferences;
        File files = getFilesDir();
        return files == null ? "" : new File(files, "bootstrap/state").getAbsolutePath();
    }

    /**
     * Decide whether this editor's key presses are counted, and read the statistics switch again.
     *
     * <p>The switch lives in the shared store, which the settings app writes from another process, so it is read on the worker when a new editor starts rather than per key. Until the answer arrives nothing is buffered: off is the store's default and the user's choice must hold before anything is kept, even in memory.
     */
    private void refreshKeyStatistics(EditorInfo info, boolean restarting, String directory) {
        keyStatisticsExcluded = info == null
            || EditorPolicy.excludesKeyStatistics(info.inputType, info.imeOptions);
        if (restarting && directory.equals(keyStatisticsDirectory)) return;
        // Presses already counted belong to the directory they were counted for.
        flushKeyPresses();
        keyStatisticsDirectory = directory;
        long generation = ++keyStatisticsGeneration;
        // Off until this read answers, so a switch turned off in the settings app holds from the first key of the new editor.
        disableKeyStatistics();
        if (directory.isEmpty()) return;
        try {
            typingStatisticsWorker.execute(() -> {
                boolean read;
                try {
                    read = NativeClient.typingStatisticsEnabled(directory);
                } catch (RuntimeException | LinkageError error) {
                    read = false;
                }
                boolean enabled = read;
                main.post(() -> {
                    if (generation != keyStatisticsGeneration) return;
                    if (enabled) keyStatisticsEnabled = true;
                    else disableKeyStatistics();
                });
            });
        } catch (RuntimeException error) {
            // The worker is shutting down or full; the switch keeps its last known value.
        }
    }

    private void disableKeyStatistics() {
        keyStatisticsEnabled = false;
        cancelKeyPressFlush();
        keyPresses.clear();
    }

    /** Tag a soft key with its heatmap id; a key without one is never counted. */
    private <T extends View> T keyId(T key, String id) {
        if (id != null) keyIds.put(key, id);
        return key;
    }

    private void countKey(View key) { countKey(keyIds.get(key)); }

    /**
     * Count one key press for the heatmap: its id and its local day, nothing else.
     *
     * <p>Presses are batched here and written by the worker -- on {@link KeyPressBatch#FLUSH_PRESSES} presses, at the first press of a new day (the old day first, under its own date), when the editor or the keyboard goes away, and otherwise half a minute after the batch opened. The worker's queue is short and every write rewrites the whole document under the file lock, so it never sees a single key.
     */
    private void countKey(String id) {
        if (id == null || !keyStatisticsEnabled || keyStatisticsExcluded) return;
        KeyPressBatch.Flush previous = keyPresses.add(id, LocalDate.now().toString());
        if (previous != null) writeKeyPresses(previous);
        if (keyPresses.full()) {
            flushKeyPresses();
        } else if (keyPressFlushTask == null && !keyPresses.isEmpty()) {
            keyPressFlushTask = () -> {
                keyPressFlushTask = null;
                flushKeyPresses();
            };
            main.postDelayed(keyPressFlushTask, KEY_PRESS_FLUSH_DELAY_MILLIS);
        }
    }

    private void cancelKeyPressFlush() {
        if (keyPressFlushTask != null) main.removeCallbacks(keyPressFlushTask);
        keyPressFlushTask = null;
    }

    private void flushKeyPresses() {
        cancelKeyPressFlush();
        KeyPressBatch.Flush flush = keyPresses.drain();
        if (flush != null) writeKeyPresses(flush);
    }

    private void writeKeyPresses(KeyPressBatch.Flush flush) {
        if (keyStatisticsDirectory.isEmpty()) return;
        final String request;
        try {
            JSONObject keys = new JSONObject();
            for (java.util.Map.Entry<String, Long> entry : flush.keys().entrySet())
                keys.put(entry.getKey(), entry.getValue().longValue());
            request = new JSONObject().put("directory", keyStatisticsDirectory).put("action",
                new JSONObject().put("operation", "record_keys").put("day", flush.day())
                    .put("keys", keys))
                .toString();
        } catch (JSONException error) {
            reportTypingStatisticsFailure();
            return;
        }
        // A batch is never empty, so nothing taken means statistics were turned off in the settings app while this editor kept focus; stop counting until the next editor reads the switch again. A newer read has the last word, so an answer about an earlier editor is ignored.
        long generation = keyStatisticsGeneration;
        submitTypingStatistics(request, () -> {
            if (generation == keyStatisticsGeneration) disableKeyStatistics();
        });
    }

    private void reportTypingStatisticsFailure() {
        main.post(() -> {
            if (statisticsFailureReported) return;
            statisticsFailureReported = true;
            preferencesNotice = " · 打字统计未能写入";
            render();
        });
    }

    private boolean commitText(String text, TypingSource source) {
        if (connection == null) return false;
        boolean committed;
        try { committed = connection.commitText(text, 1); }
        catch (RuntimeException error) { return false; }
        if (committed) recordTypingStatistics(text, source);
        return committed;
    }

    private boolean commitText(String text) { return commitText(text, typingSource()); }

    private void cancelInputViewRefresh() {
        if (inputViewRefreshTask != null) main.removeCallbacks(inputViewRefreshTask);
        inputViewRefreshTask = null;
    }

    /** Re-read host settings at the appearance boundary, matching Apple's viewWillAppear path. */
    private void refreshPreferencesOnInputView() {
        if (session == 0 || preferencesDirectory.isEmpty() || hasEngineComposition()) return;
        preferencesReloader.start(preferencesDirectory, this::reloadPreferences);
    }

    @Override public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        cancelInputViewRefresh();
        long startGeneration = ++engineStartGeneration;
        cloudClipboardGeneration++;
        cancelPersonalDictionarySynchronization();
        boolean newDocument = !restarting || currentDocumentIdentifier == 0;
        if (newDocument) {
            currentDocumentIdentifier = nextDocumentIdentifier++;
        }
        resetSpaceCursor();
        stop(false);
        connection = getCurrentInputConnection();
        ensureCandidateTranslationStore();
        editorContextRevision++;
        clearSmartPunctuationSnapshots();
        bridge = new EditorBridge();
        schemeHostPreferences = getSharedPreferences(SCHEME_HOST_PREFERENCES, MODE_PRIVATE);
        if (inputModeStore == null) {
            inputModeStore = InputModeStore.from(
                getSharedPreferences(INPUT_MODE_PREFERENCES, MODE_PRIVATE));
        }
        sharedSchemePreferences = false;
        enabledSchemes = KeyboardScheme.enabledFromPreferenceIds(null);
        letterCase.reset();
        clearEnglishSuggestions();
        keyboardLayer = KeyboardLayout.Layer.LETTERS;
        editorInputType = info == null ? 0 : info.inputType;
        currentEditorPackage = info == null || info.packageName == null ? "" : info.packageName;
        allowLearning = info != null && EditorPolicy.allowLearning(info.imeOptions);
        preferencesNotice = "";
        statisticsFailureReported = false;
        message = "直接输入";
        // 外观和引擎是两件事。皮肤、方案角标和键高来自同一份偏好，但它们该跟着用户走，而不是跟着
        // 这个输入框用不用引擎走：系统在冷启动后发来的第一个编辑器 inputType 是 0，密码框和数字
        // 框也都不走引擎，把读偏好一起挡在外面，键盘就先按字段初始值画成默认绿的 26 键，等下一个
        // 普通输入框到了才跳成用户自己的皮肤——那一跳看起来就像换了个输入法。
        boolean engineWanted = info != null && connection != null
            && EditorPolicy.useEngine(info.inputType);
        String statisticsPreferences = "";
        try {
            File file = new File(getFilesDir(), "runtime-options.json");
            JSONObject options = new JSONObject(HostOptionsPolicy.read(file));
            statisticsPreferences = options.optString("preferences_directory", "");
            JSONObject preferences = options.optJSONObject("preferences");
            applyEditorPreferences(preferences);
            if (newDocument) {
                boolean defaultEnglish = "english".equals(defaultImeMode);
                dedicatedEnglish = inputModeStore.modeFor(
                    imeModeScope, currentEditorPackage, defaultEnglish);
            }
            Boolean englishOverride = inputContext.englishOverride(
                EditorPolicy.prefersLatin(editorInputType), currentDocumentIdentifier, dedicatedEnglish);
            if (englishOverride != null) dedicatedEnglish = englishOverride;
            // 那份快照是首次安装时写下的，之后再没更新过（见 Bootstrap.prepare），所以它的偏好
            // 永远是出厂默认。有引擎会话时实时偏好会由 preferencesReloader 补上；没有会话的输入
            // 框走不到那条路，只能自己读一次，否则键盘就一直是默认皮肤和默认方案。
            if (!engineWanted) {
                loadAppearanceWithoutSession(options.optString("preferences_directory", ""));
            }
            if (engineWanted) {
                if (!allowLearning) options.getJSONObject("preferences").put("learning", false);
                runtimeOptionsForSnapshot = options.toString();
                message = "共享运行时准备中";
                scheduleEngineStartup(runtimeOptionsForSnapshot, startGeneration);
            }
        } catch (Exception | LinkageError error) {
            stop(false);
            // 这句说的是引擎起不来。没打算起引擎的输入框读不到偏好，只是外观退回默认，说「未就绪」
            // 会把一个不存在的故障摆到用户面前。
            if (engineWanted) message = "共享运行时未就绪：仅直接输入";
        }
        refreshKeyStatistics(info, restarting, keyStatisticsDirectory(statisticsPreferences));
        updateAutomaticCapitalization();
        rebuildKeyRows();
        render();
        replySuppressed = false;
        synchronizeReplyKeyboard();
    }

    // One reporting session per keyboard process: a crash here is recorded against it, and a session that ends through onDestroy counts as a normal one.
    @Override public void onCreate() {
        super.onCreate();
        Telemetry.beginInputSession(this);
    }

    @Override public void onStartInputView(EditorInfo info, boolean restarting) {
        super.onStartInputView(info, restarting);
        Telemetry.inputViewShown(this);
        cancelInputViewRefresh();
        final long expectedGeneration = engineStartGeneration;
        final InputConnection expectedConnection = connection;
        inputViewRefreshTask = () -> {
            inputViewRefreshTask = null;
            InputConnection currentConnection = getCurrentInputConnection();
            boolean viewValid = keyboardRoot != null && keyboardRoot.getWindowToken() != null;
            if (!InputViewRefreshPolicy.shouldRefresh(
                    expectedGeneration, engineStartGeneration, expectedConnection,
                    currentConnection, viewValid, hasEngineComposition())) return;
            EditorInfo currentInfo = getCurrentInputEditorInfo();
            EditorInfo effectiveInfo = currentInfo == null ? info : currentInfo;
            if (effectiveInfo != null) {
                editorInputType = effectiveInfo.inputType;
                allowLearning = EditorPolicy.allowLearning(effectiveInfo.imeOptions);
                keyStatisticsExcluded = EditorPolicy.excludesKeyStatistics(
                    effectiveInfo.inputType, effectiveInfo.imeOptions);
            }
            loadFeedbackPreferences();
            refreshPreferencesOnInputView();
            updateAutomaticCapitalization();
            render();
        };
        main.postDelayed(inputViewRefreshTask, INPUT_VIEW_REFRESH_DELAY_MILLIS);
    }

    @Override public void onFinishInput() {
        flushKeyPresses();
        cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        cloudClipboardGeneration++;
        resetSpaceCursor();
        stop(true);
        schedulePersonalDictionarySynchronization(false);
        scheduleDictionarySnapshotProcessing();
        connection = null;
        currentDocumentIdentifier = 0;
        super.onFinishInput();
    }

    @Override public void onFinishInputView(boolean finishingInput) {
        flushKeyPresses();
        cancelInputViewRefresh();
        if (!finishingInput) finishInputViewPresentation();
        super.onFinishInputView(finishingInput);
    }

    /** Match Apple's viewWillDisappear boundary while keeping the editor session alive. */
    private void finishInputViewPresentation() {
        cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        resetSpaceCursor();
        dismissNineKeyHoldOptions();
        hideJapaneseFlickPreview();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeSkinPicker();
        closeLayoutSettings();
        closeMoreTools();
        closeEmojiPicker();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        closeSymbolPanel();
        deactivateHandwriting();
        clearDiagnostic();
        // macOS resolves the same focus-loss boundary with finish_composition, so a keyboard put
        // away mid-composition leaves 你好 behind rather than the letters `nihao`. This was
        // CommitRaw only because command 9 was unmapped in the FFI when the path was written.
        if (session != 0 && connection != null && view != null
                && !view.optString("editing_text", "").isEmpty()) {
            command(FINISH_COMPOSITION_COMMAND);
        }
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        JSONObject preferences = preferencesSnapshot == null ? null
            : preferencesSnapshot.optJSONObject("preferences");
        KeyboardSkin next = keyboardSkin(preferences);
        boolean skinChanged = !skin.key().equals(next.key());
        skin = next;
        // The input service can survive rotation and window/inset changes. Reapply the
        // geometry even when the palette is unchanged; otherwise the existing key tree keeps
        // margins and height adjustments calculated with the previous configuration. A held
        // nine-key popup is anchored to the old tree, so it must not outlive the reflow.
        dismissNineKeyHoldOptions();
        hideJapaneseFlickPreview();
        if (keyboardRoot == null) return;
        applyKeyboardSurfaceGeometry();
        if (skinChanged) applySkin();
        applyKeyboardGeometry();
        renderLayoutSettingsState();
        render();
    }
    @Override public void onDestroy() {
        cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        cancelPersonalDictionarySynchronization();
        stop(false);
        schedulePersonalDictionarySynchronization(true);
        preferencesWorker.shutdown();
        // Queued before the shutdown, so the worker still writes it.
        flushKeyPresses();
        typingStatisticsWorker.shutdown();
        emojiWorker.shutdown();
        cloudClipboardWorker.shutdownNow();
        candidateGlossWorker.shutdownNow();
        candidateTranslationWorker.shutdownNow();
        englishSuggestionWorker.shutdownNow();
        onlineCandidateWorker.shutdownNow();
        aiPolishClient.close();
        connection = null;
        Telemetry.endInputSession(this);
        super.onDestroy();
    }
    @Override public boolean onEvaluateFullscreenMode() { return false; }

    private void stop(boolean finish) {
        cancelBackspaceRepeat();
        clearDiagnostic();
        clearSmartPunctuationSnapshots();
        deactivateHandwriting();
        invalidateOnlineProviders();
        invalidateCandidateGlosses();
        preferencesReloader.stop();
        preferenceSaveGeneration++;
        preferencesDirectory = "";
        emojiResources = "";
        candidateGlossResources = "";
        candidateEnglishGloss = false;
        candidateTranslationsEnabled = false;
        candidateTranslationAccount = false;
        candidateTranslationTargets = java.util.List.of("en");
        clearEnglishSuggestions();
        if (candidateTranslationStore != null) candidateTranslationStore.clear();
        wubiCodeHint = true;
        wubiMixedPinyin = false;
        preferencesSnapshot = null;
        schemeSaving = false;
        touchGeometrySaving = false;
        skinSaving = false;
        traditionalOutputSaving = false;
        if (session != 0) {
            try { if (finish && connection != null) apply(NativeClient.command(session, 2)); }
            catch (Exception | LinkageError ignored) { /* Never log editor text or native responses. */ }
            try { NativeClient.destroy(session); } catch (LinkageError ignored) { }
            session = 0;
        }
        if (connection != null) bridge.abandon(sink(typingSource()));
        view = null;
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeSkinPicker();
        closeLayoutSettings();
        closeMoreTools();
        closeEmojiPicker();
        closeSymbolPanel();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
    }

    /**
     * Apply queued personal-dictionary edits only after the Engine session is gone.
     * Android has no keyboard-extension full-access callback, so the service lifecycle
     * is the safe maintenance boundary; the next input start retries before creating
     * another session.
     */
    private void cancelPersonalDictionarySynchronization() {
        personalDictionarySyncGeneration++;
        if (personalDictionarySyncTask != null) {
            main.removeCallbacks(personalDictionarySyncTask);
            personalDictionarySyncTask = null;
        }
    }

    /**
     * Dictionary maintenance owns an exclusive lock, while Engine creation owns a
     * shared session lock. Keep both operations off the input thread and only create
     * the session after the bounded maintenance transaction has finished.
     */
    private void scheduleEngineStartup(String options, long generation) {
        Runnable complete = () -> {
            if (generation != engineStartGeneration || connection == null || session != 0) return;
            startEngineSession(options);
        };
        try {
            preferencesWorker.execute(() -> {
                String notice = "";
                try {
                    JSONObject sync = value(NativeClient.personalDictionarySync(options));
                    if (sync.optString("snapshot_error", "").length() > 0) {
                        notice = " · 个人词库同步稍后重试";
                    }
                } catch (Exception | LinkageError ignored) {
                    // Personal dictionary maintenance is optional; session startup continues.
                }
                String finalNotice = notice;
                main.post(() -> {
                    if (generation != engineStartGeneration || connection == null || session != 0) return;
                    if (!finalNotice.isEmpty()) preferencesNotice = finalNotice;
                    complete.run();
                });
            });
        } catch (RuntimeException ignored) {
            // A worker shutdown must not leave a still-valid editor without its session.
            main.post(complete);
        }
    }

    private void startEngineSession(String optionsText) {
        try {
            JSONObject options = new JSONObject(optionsText);
            // 选中只吃掉部分输入的候选时，让运行时把已选的那一段留在组字里而不是立刻上屏。这个宿主
            // 会把它画在组字与候选条的读音前面，两件事必须一起做：请求了却不画，用户已经选中的字
            // 既不在文档里也不在屏幕上（scripts/test-phrase-preedit-hosts.py 守的就是这一半状态）。
            options.put("phrase_preedit", true);
            int drawnLayout = displayedTouchLayout(view);
            view = value(NativeClient.create(options.toString()));
            session = view.getLong("session");
            String resources = options.optString("resources", "");
            if (new File(resources).isAbsolute()) {
                emojiResources = resources;
                candidateGlossResources = resources;
            }
            apply(NativeClient.focus(session, true));
            view = value(NativeClient.setEnglishMode(session, dedicatedEnglish));
            applyCharacterWidth(fullWidthPreference);
            applyChinesePunctuation(chinesePunctuationPreference);
            // The rows were drawn before the session existed, from no view at all; a scheme with its own surface (the Korean keycaps) has to replace them now that the view says which one applies.
            if (displayedTouchLayout(view) != drawnLayout) {
                letterCase.reset();
                rebuildKeyRows();
            }
            refreshEnglishSuggestions();
            render();
            message = "MSIME Preview";
            String directory = options.optString("preferences_directory", "");
            if (!directory.isEmpty() && new File(directory).isAbsolute()) {
                preferencesDirectory = directory;
                preferencesReloader.start(directory, this::reloadPreferences);
            }
        } catch (Exception | LinkageError error) {
            stop(false);
            message = "共享运行时未就绪：仅直接输入";
        }
    }

    private void schedulePersonalDictionarySynchronization(boolean immediate) {
        if (runtimeOptionsForSnapshot.isEmpty()) return;
        String options = runtimeOptionsForSnapshot;
        long generation = ++personalDictionarySyncGeneration;
        Runnable task = () -> {
            if (generation != personalDictionarySyncGeneration || session != 0) return;
            personalDictionarySyncTask = null;
            try {
                preferencesWorker.execute(() -> {
                    try {
                        NativeClient.personalDictionarySync(options);
                    } catch (Exception | LinkageError ignored) {
                        // The next idle boundary retries a busy or unavailable journal.
                    }
                });
            } catch (RuntimeException ignored) {
                // Service shutdown owns the final worker state.
            }
        };
        personalDictionarySyncTask = task;
        if (immediate) {
            task.run();
        } else {
            main.postDelayed(task, PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS);
        }
    }

    /** The session has been destroyed, so native maintenance may take the exclusive lock. */
    private void scheduleDictionarySnapshotProcessing() {
        if (runtimeOptionsForSnapshot.isEmpty()) return;
        String options = runtimeOptionsForSnapshot;
        Path filesRoot = getFilesDir().toPath();
        Path root = filesRoot.resolve("bootstrap/state/dictionary-snapshots");
        Path staging = root.resolve("staging");
        try {
            preferencesWorker.execute(() -> {
                try { DictionarySnapshotWorker.process(filesRoot, root, staging, options); }
                catch (Exception | LinkageError ignored) { /* Retry at the next idle boundary. */ }
            });
        } catch (RuntimeException ignored) { /* Service shutdown owns the final worker state. */ }
    }

    private void applyCandidateAppearance(JSONObject preferences) {
        candidateAppearance = CandidateAppearance.from(preferences,
            surfaceSkin(preferences, "candidate_theme"));
        if (preferences == null) {
            candidateHorizontal = false;
            candidateFontSize = 16;
            candidatePreeditFontSize = 16;
            return;
        }
        String layout = preferences.optString("candidate_layout",
            preferences.optString("candidate_orientation", "vertical"));
        candidateHorizontal = CandidateAppearance.isHorizontal(layout);
        candidateFontSize = CandidateAppearance.fontSize(preferences.optInt("candidate_font_size", 16));
        candidatePreeditFontSize = CandidateAppearance.fontSize(
            preferences.optInt("candidate_preedit_font_size", candidateFontSize));
    }

    private void applyTouchGeometry(JSONObject preferences) {
        touchKeySpacingTenths = KeyboardGeometry.keySpacing(preferences == null ? -1
            : preferences.optInt("touch_key_spacing_tenths", -1));
        touchRowSpacingTenths = KeyboardGeometry.rowSpacing(preferences == null ? -1
            : preferences.optInt("touch_row_spacing_tenths", -1));
        touchKeyboardHeightAdjustment = KeyboardGeometry.heightAdjustment(preferences == null
            ? Integer.MIN_VALUE : preferences.optInt("touch_keyboard_height_adjustment",
                Integer.MIN_VALUE));
        touchVoiceShortcutEnabled = preferences != null
            && preferences.optBoolean("touch_voice_shortcut", false);
    }

    private void applyVoicePreferences(JSONObject preferences) {
        JSONObject voice = preferences == null ? null : preferences.optJSONObject("voice_input");
        voiceInputEnabled = voice == null || voice.optBoolean("enabled", true);
        voiceLanguage = voice == null ? "zh-CN" : voice.optString("language", "zh-CN");
    }

    private void applyAiPreferences(JSONObject preferences) {
        AiPolishConfiguration next = null;
        JSONObject ai = preferences == null ? null : preferences.optJSONObject("ai_assistant");
        if (ai != null && ai.optBoolean("enabled", false)) {
            try {
                String endpoint = ai.optString("endpoint", "");
                String origin = AiPolishConfiguration.credentialOrigin(endpoint);
                JSONObject tokens = ai.optJSONObject("tokens");
                String token = tokens == null ? "" : tokens.optString(origin, "");
                String prompt = ai.optString("prompt", "");
                if (prompt.trim().isEmpty()) prompt = AiPolishConfiguration.DEFAULT_PROMPT;
                next = new AiPolishConfiguration(endpoint, ai.optString("model", ""), prompt, token);
            } catch (IllegalArgumentException ignored) {
                // Invalid settings disable this entry; never log endpoints, models or credentials.
            }
        }
        AiPolishConfiguration previous = aiPolishConfiguration;
        aiPolishConfiguration = next;
        if (aiPolishContainer != null && aiPolishContainer.getVisibility() == View.VISIBLE
                && aiRequestConfiguration != null && !aiRequestConfiguration.equals(next)) {
            cancelAiRequest();
            aiError = "AI 配置已变化，请返回键盘后重新打开。";
            renderAiPolish();
        }
        if (replyRequestConfiguration != null && !replyRequestConfiguration.equals(next)) {
            invalidateReplyContext("AI 配置已变化，请重新选择回复方式");
        }
        if (previous != null && !previous.equals(next)) render();
    }

    private void applyClipboardPreference(JSONObject preferences) {
        clipboardHistoryEnabled = preferences != null
            && preferences.optBoolean("clipboard_history", false);
        if (!clipboardHistoryEnabled && clipboardHistory != null) clipboardHistory.clearQuietly();
    }

    private void applyChineseOutputPreference(JSONObject preferences) {
        traditionalChineseOutput = preferences != null
            && preferences.optBoolean("traditional_chinese_output", false);
    }

    private void applyCandidateGlossPreference(JSONObject preferences) {
        boolean next = preferences != null
            && preferences.optBoolean("candidate_english_gloss", true);
        if (candidateEnglishGloss != next) invalidateCandidateGlosses();
        candidateEnglishGloss = next;
    }

    private void applyEnglishSuggestionsPreference(JSONObject preferences) {
        boolean next = preferences == null
            || preferences.optBoolean("english_suggestions", true);
        if (englishSuggestionsEnabled != next) clearEnglishSuggestions();
        englishSuggestionsEnabled = next;
    }

    private void applyCandidateTranslationPreference(JSONObject preferences) {
        boolean nextEnabled = preferences == null
            || preferences.optBoolean("candidate_translations", true);
        boolean nextAccount = candidateTranslationAccountFrom(preferences);
        java.util.List<String> nextTargets = translationTargetsFrom(preferences);
        if (candidateTranslationsEnabled != nextEnabled
                || candidateTranslationAccount != nextAccount
                || !candidateTranslationTargets.equals(nextTargets)) {
            if (candidateTranslationStore != null) candidateTranslationStore.clear();
            invalidateCandidateGlosses();
        }
        candidateTranslationsEnabled = nextEnabled;
        candidateTranslationAccount = nextAccount;
        candidateTranslationTargets = nextTargets;
    }

    /** Whether the user explicitly chose the MSIME account for candidate translations; absent preferences never choose it. */
    private static boolean candidateTranslationAccountFrom(JSONObject preferences) {
        if (preferences == null) return false;
        return CandidateTranslationPolicy.accountSelected(
            preferences.optBoolean("candidate_translations", true),
            preferences.optBoolean("translation_account", false),
            translationProviderEnabled(preferences, "niutrans"),
            translationProviderEnabled(preferences, "custom_translation"));
    }

    private static boolean translationProviderEnabled(JSONObject preferences, String key) {
        JSONObject provider = preferences.optJSONObject(key);
        return provider != null && provider.optBoolean("enabled", false);
    }

    private java.util.List<String> translationTargetsFrom(JSONObject preferences) {
        String primary = preferences == null ? "en"
            : preferences.optString("translation_target_language", "en");
        String secondary = "";
        if (preferences != null) {
            Object value = preferences.opt("translation_secondary_language");
            if (value instanceof String) secondary = (String) value;
        }
        return CandidateTranslationPolicy.targets(primary, secondary);
    }

    private void applyWubiCodeHintPreference(JSONObject preferences) {
        wubiCodeHint = preferences == null
            || preferences.optBoolean("wubi_code_hint", true);
    }

    private void invalidateCandidateGlosses() {
        candidateGlossEpoch = candidateGlossEpoch == Long.MAX_VALUE ? 0 : candidateGlossEpoch + 1;
        candidateGlossRequestedSession = 0;
        candidateGlossRequestedGeneration = -1;
        candidateOfflineTargetsKey = null;
        candidateOfflineGlosses = null;
    }

    /** The non-English targets with an installed offline dictionary, rechecked whenever the targets, resources or gloss preferences change. */
    private java.util.List<String> candidateOfflineTargets() {
        String key = candidateGlossResources + "\n" + candidateTranslationTargets;
        if (!key.equals(candidateOfflineTargetsKey)) {
            candidateOfflineTargetsKey = key;
            candidateOfflineTargets = CandidateTranslationPolicy.offlineTargets(
                candidateTranslationTargets, candidateGlossResources);
        }
        return candidateOfflineTargets;
    }

    private void ensureCandidateTranslationStore() {
        if (candidateTranslationStore == null) {
            candidateTranslationStore = new CandidateTranslationStore(
                this, candidateTranslationWorker, main,
                generation -> applyCandidateTranslations(generation));
        }
    }

    private String chineseOutput(String text, JSONObject context) {
        int scheme = context == null
            ? (view == null ? -1 : view.optInt("scheme", -1))
            : context.optInt("scheme", -1);
        String localMode = context == null
            ? (view == null ? "none" : view.optString("local_mode", "none"))
            : context.optString("local_mode", "none");
        return AndroidChineseTextConversion.outputString(
            text, traditionalChineseOutput, dedicatedEnglish, scheme, localMode);
    }

    private String candidateAppearanceKey() {
        return (candidateHorizontal ? "horizontal" : "vertical") + ":"
            + candidateFontSize + ":" + candidatePreeditFontSize + ":"
            + candidateAppearance.key();
    }

    private String touchGeometryKey() {
        return touchKeySpacingTenths + ":" + touchRowSpacingTenths + ":"
            + touchKeyboardHeightAdjustment + ":"
            + touchVoiceShortcutEnabled + ":" + voiceInputEnabled + ":" + voiceLanguage;
    }

    private void reloadPreferences(String response) {
        if (session == 0) return;
        boolean previousPreferencesReady = preferencesSnapshot != null;
        String previousView = view == null ? "" : view.toString();
        String previousNotice = preferencesNotice;
        String previousSkin = skin.key();
        String previousAppearance = candidateAppearanceKey();
        String previousGeometry = touchGeometryKey();
        AiPolishConfiguration previousAi = aiPolishConfiguration;
        boolean previousClipboard = clipboardHistoryEnabled;
        boolean previousTraditional = traditionalChineseOutput;
        boolean previousCandidateGloss = candidateEnglishGloss;
        boolean previousCandidateTranslations = candidateTranslationsEnabled;
        boolean previousCandidateTranslationAccount = candidateTranslationAccount;
        boolean previousEnglishSuggestions = englishSuggestionsEnabled;
        java.util.List<String> previousTranslationTargets = candidateTranslationTargets;
        boolean previousWubiCodeHint = wubiCodeHint;
        boolean previousWubiMixedPinyin = wubiMixedPinyin;
        KeyboardScheme previousScheme = selectedScheme;
        try {
            if (response == null) throw new JSONException("Preferences unavailable");
            JSONObject snapshot = value(response);
            applyPreferencesSnapshot(snapshot);
        } catch (JSONException | LinkageError error) {
            // Never replace the working session or log preferences/native responses.
            preferencesNotice = " · 设置读取或应用失败，保留当前设置";
        }
        if (previousPreferencesReady != (preferencesSnapshot != null)
                || !previousNotice.equals(preferencesNotice) || !previousSkin.equals(skin.key())
                || !previousAppearance.equals(candidateAppearanceKey())
                || !previousGeometry.equals(touchGeometryKey())
                || (previousAi == null ? aiPolishConfiguration != null
                    : !previousAi.equals(aiPolishConfiguration))
                || previousClipboard != clipboardHistoryEnabled
                || previousTraditional != traditionalChineseOutput
                || previousCandidateGloss != candidateEnglishGloss
                || previousCandidateTranslations != candidateTranslationsEnabled
                || previousCandidateTranslationAccount != candidateTranslationAccount
                || previousEnglishSuggestions != englishSuggestionsEnabled
                || !previousTranslationTargets.equals(candidateTranslationTargets)
                || previousWubiCodeHint != wubiCodeHint
                || previousWubiMixedPinyin != wubiMixedPinyin
                || previousScheme != selectedScheme
                || !previousView.equals(view == null ? "" : view.toString())) render();
    }

    private void applyPreferencesSnapshot(JSONObject snapshot) throws JSONException {
        JSONObject accepted = new JSONObject(snapshot.toString());
        JSONObject preferences = accepted.getJSONObject("preferences");
        KeyboardSkin nextSkin = keyboardSkin(preferences);
        JSONObject nextLocalModes = preferences.optJSONObject("local_modes");
        if (nextLocalModes == null) nextLocalModes = new JSONObject();
        String nextLayout = preferences.optString("candidate_layout",
            preferences.optString("candidate_orientation", "vertical"));
        CandidateAppearance.Palette nextCandidateAppearance = CandidateAppearance.from(
            preferences, surfaceSkin(preferences, "candidate_theme"));
        boolean nextHorizontal = CandidateAppearance.isHorizontal(nextLayout);
        int nextFontSize = CandidateAppearance.fontSize(preferences.optInt("candidate_font_size", 16));
        int nextPreeditFontSize = CandidateAppearance.fontSize(
            preferences.optInt("candidate_preedit_font_size", nextFontSize));
        int nextKeySpacing = KeyboardGeometry.keySpacing(
            preferences.optInt("touch_key_spacing_tenths", -1));
        int nextRowSpacing = KeyboardGeometry.rowSpacing(
            preferences.optInt("touch_row_spacing_tenths", -1));
        int nextHeightAdjustment = KeyboardGeometry.heightAdjustment(
            preferences.optInt("touch_keyboard_height_adjustment", Integer.MIN_VALUE));
        boolean nextVoiceShortcut = preferences.optBoolean("touch_voice_shortcut", false);
        JSONObject nextVoice = preferences.optJSONObject("voice_input");
        boolean nextVoiceEnabled = nextVoice == null || nextVoice.optBoolean("enabled", true);
        String nextVoiceLanguage = nextVoice == null ? "zh-CN"
            : nextVoice.optString("language", "zh-CN");
        boolean nextClipboard = preferences.optBoolean("clipboard_history", false);
        boolean nextTraditional = preferences.optBoolean("traditional_chinese_output", false);
        boolean nextCandidateGloss = preferences.optBoolean("candidate_english_gloss", true);
        boolean nextCandidateTranslations = preferences.optBoolean("candidate_translations", true);
        boolean nextCandidateTranslationAccount = candidateTranslationAccountFrom(preferences);
        boolean nextEnglishSuggestions = preferences.optBoolean("english_suggestions", true);
        java.util.List<String> nextTranslationTargets = translationTargetsFrom(preferences);
        boolean nextWubiCodeHint = preferences.optBoolean("wubi_code_hint", true);
        boolean nextWubiMixedPinyin = preferences.optBoolean("wubi_mixed_pinyin", false);
        JSONObject nextKeybindings = preferences.optJSONObject("keybindings");
        boolean nextLanguageShift = nextKeybindings == null
            || nextKeybindings.optBoolean("switch_language_shift", true);
        boolean nextLanguageCtrlAltSpace = nextKeybindings == null
            || nextKeybindings.optBoolean("switch_language_ctrl_alt_space", true);
        boolean nextCharacterSet = nextKeybindings == null
            || nextKeybindings.optBoolean("toggle_character_set_ctrl_shift_f", true);
        boolean nextFullWidth = nextKeybindings == null
            || nextKeybindings.optBoolean("toggle_fullwidth_option_shift_h", true);
        boolean nextNumberRowSelection = preferences.optBoolean("number_row_selection", true);
        boolean nextFullWidthPreference = CharacterWidthPolicy.preferenceIsFullWidth(
            preferences.optString(CharacterWidthPolicy.PREFERENCE_KEY, "halfwidth"));
        boolean nextChinesePunctuation = preferences.optBoolean("chinese_punctuation", true);
        String nextWordCharacterBinding = wordCharacterBindingFrom(preferences);
        String nextCandidatePreeditStyle = CandidatePreeditStylePolicy.style(
            preferences.optString("candidate_preedit_style", CandidatePreeditStylePolicy.PINYIN));
        CandidateNavigationPolicy.Bindings nextCandidateNavigation =
            candidateNavigationFrom(preferences.optJSONObject("navigation"));
        boolean nextLanguageCtrl = nextKeybindings != null
            && nextKeybindings.optBoolean("switch_language_ctrl", false);
        String nextDefaultImeMode = "english".equals(
            preferences.optString("default_ime_mode", "chinese")) ? "english" : "chinese";
        String nextImeModeScope = "global".equals(
            preferences.optString("ime_mode_scope", "app")) ? "global" : "app";
        KeyboardScheme nextScheme = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", "quanpin"),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"));
        SchemeConfiguration nextSchemeConfiguration = schemeConfiguration(preferences, nextScheme);
        JSONObject sessionSnapshot = new JSONObject(accepted.toString());
        // Keep the accepted disk snapshot intact while enforcing editor privacy in this session.
        if (!allowLearning) sessionSnapshot.getJSONObject("preferences").put("learning", false);
        JSONObject result = value(NativeClient.updatePreferences(session, sessionSnapshot.toString()));
        boolean geometryChanged = touchKeySpacingTenths != nextKeySpacing
            || touchRowSpacingTenths != nextRowSpacing
            || touchKeyboardHeightAdjustment != nextHeightAdjustment;
        skin = nextSkin;
        candidateAppearance = nextCandidateAppearance;
        localModes = nextLocalModes;
        candidateHorizontal = nextHorizontal;
        candidateFontSize = nextFontSize;
        candidatePreeditFontSize = nextPreeditFontSize;
        touchKeySpacingTenths = nextKeySpacing;
        touchRowSpacingTenths = nextRowSpacing;
        touchKeyboardHeightAdjustment = nextHeightAdjustment;
        touchVoiceShortcutEnabled = nextVoiceShortcut;
        voiceInputEnabled = nextVoiceEnabled;
        voiceLanguage = nextVoiceLanguage;
        applyAiPreferences(preferences);
        clipboardHistoryEnabled = nextClipboard;
        traditionalChineseOutput = nextTraditional;
        if (candidateEnglishGloss != nextCandidateGloss) invalidateCandidateGlosses();
        candidateEnglishGloss = nextCandidateGloss;
        if (englishSuggestionsEnabled != nextEnglishSuggestions) clearEnglishSuggestions();
        englishSuggestionsEnabled = nextEnglishSuggestions;
        if (candidateTranslationsEnabled != nextCandidateTranslations
                || candidateTranslationAccount != nextCandidateTranslationAccount
                || !candidateTranslationTargets.equals(nextTranslationTargets)) {
            if (candidateTranslationStore != null) candidateTranslationStore.clear();
            invalidateCandidateGlosses();
        }
        candidateTranslationsEnabled = nextCandidateTranslations;
        candidateTranslationAccount = nextCandidateTranslationAccount;
        candidateTranslationTargets = nextTranslationTargets;
        wubiCodeHint = nextWubiCodeHint;
        wubiMixedPinyin = nextWubiMixedPinyin;
        hardwareLanguageShift = nextLanguageShift;
        hardwareLanguageCtrlAltSpace = nextLanguageCtrlAltSpace;
        hardwareCharacterSet = nextCharacterSet;
        hardwareFullWidth = nextFullWidth;
        numberRowSelection = nextNumberRowSelection;
        // Only a change to 「全角输入」 itself overrides a toggle the user made from the toolbar
        // card or the chord; saving any other setting must not quietly drop it.
        boolean characterWidthChanged =
            CharacterWidthPolicy.overridesToggle(fullWidthPreference, nextFullWidthPreference);
        fullWidthPreference = nextFullWidthPreference;
        // Same rule as the width: only a change to 中文标点 itself overrides a toggle the user
        // made, so saving any other setting cannot quietly drop it.
        boolean punctuationChanged =
            CharacterWidthPolicy.overridesToggle(chinesePunctuationPreference, nextChinesePunctuation);
        chinesePunctuationPreference = nextChinesePunctuation;
        wordCharacterBinding = nextWordCharacterBinding;
        candidatePreeditStyle = nextCandidatePreeditStyle;
        candidateNavigation = nextCandidateNavigation;
        hardwareLanguageCtrl = nextLanguageCtrl;
        defaultImeMode = nextDefaultImeMode;
        imeModeScope = nextImeModeScope;
        JSONObject nextView = result.getJSONObject("view");
        boolean rebuildLayout = displayedTouchLayout(view) != displayedTouchLayout(nextView);
        enabledSchemes = nextSchemeConfiguration.enabled();
        selectedScheme = nextSchemeConfiguration.selected();
        sharedSchemePreferences = nextSchemeConfiguration.shared();
        preferencesSnapshot = accepted;
        if (!clipboardHistoryEnabled && clipboardHistory != null) {
            clipboardHistory.clearQuietly();
            // The cloud half does not depend on this switch; only a panel left with nothing to show closes.
            if (clipboardPanelOpen() && cloudClipboardAllowed()) renderClipboardHistory();
            else closeClipboardHistory();
        }
        view = nextView;
        // After the view is in place, because this replaces it with the runtime's answer.
        if (characterWidthChanged) applyCharacterWidth(nextFullWidthPreference);
        if (punctuationChanged) applyChinesePunctuation(nextChinesePunctuation);
        // A Shift latched on the Korean keycaps means a double consonant, so it must not outlive the surface it was set on.
        if (rebuildLayout || !KeyboardLayout.carriesLetterCase(displayedTouchLayout(view)))
            letterCase.reset();
        if (rebuildLayout) rebuildKeyRows();
        else if (geometryChanged) applyKeyboardGeometry();
        renderLayoutSettingsState();
        if (voiceResultScroll != null && voiceResultScroll.getVisibility() == View.VISIBLE)
            renderVoiceResult();
        preferencesNotice = result.getBoolean("deferred") ? " · 设置将在组词结束后应用" : "";
    }

    private boolean apply(String response) throws JSONException {
        JSONObject result = value(response);
        JSONObject next = result.getJSONObject("view");
        boolean rebuildLayout = displayedTouchLayout(view) != displayedTouchLayout(next);
        String commit = result.isNull("commit") ? null : result.getString("commit");
        if (commit != null) {
            // The runtime widened this already - it holds the character width now, and applies it
            // to everything it finishes. Widening again here would be the second pass over text
            // that is already fullwidth, and would put this host's own rule ahead of the shared one.
            commit = chineseOutput(commit, result.optJSONObject("commit_context"));
        }
        // Korean marks the composing Hangul, not the key letters editing_text holds; a transition may carry the syllable the key finished and the next one together, and the bridge writes the commit first.
        String composing = KoreanInputPolicy.composing(
            KoreanInputPolicy.active(next.optInt("scheme", -1),
                next.optBoolean("dedicated_english", dedicatedEnglish)),
            next.optString("phrase_prefix", ""), next.getString("editing_text"),
            next.optString("reading", ""));
        if (connection != null
                && !bridge.apply(sink(typingSource()), commit, composing)) {
            throw new JSONException("Editor rejected update");
        }
        view = next;
        showDiagnostic(result.isNull("diagnostic") ? null
            : result.optString("diagnostic", ""));
        if (rebuildLayout || !KeyboardLayout.carriesLetterCase(displayedTouchLayout(view)))
            letterCase.reset();
        if (rebuildLayout) rebuildKeyRows();
        render();
        return result.getBoolean("handled");
    }

    private boolean englishNineKeyActive() {
        return dedicatedEnglish && displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT;
    }

    private boolean directEnglishActive() {
        return dedicatedEnglish && !englishNineKeyActive();
    }

    private boolean englishSuggestionsActive() {
        return directEnglishActive() && englishSuggestionsEnabled;
    }

    private void clearEnglishSuggestions() {
        englishSuggestionEpoch++;
        englishSuggestionRequestedEpoch = -1;
        englishSuggestionRequestedPrefix = "";
        englishSuggestionPrefix = "";
        englishSuggestions = java.util.List.of();
    }

    private String englishWordBeforeCursor() {
        if (!directEnglishActive() || connection == null) return "";
        CharSequence before;
        try {
            before = connection.getTextBeforeCursor(CAPITALIZATION_CONTEXT_LIMIT, 0);
        } catch (RuntimeException ignored) {
            return "";
        }
        return EnglishSuggestionPolicy.currentWord(before);
    }

    private void refreshEnglishSuggestions() {
        if (!englishSuggestionsActive() || candidateGlossResources.isEmpty()) {
            if (!englishSuggestions.isEmpty() || !englishSuggestionPrefix.isEmpty()) {
                clearEnglishSuggestions();
                render();
            }
            return;
        }
        String prefix = englishWordBeforeCursor();
        if (prefix.length() < 2 || !prefix.chars().allMatch(value ->
                value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z')) {
            if (!englishSuggestions.isEmpty() || !englishSuggestionPrefix.isEmpty()) {
                clearEnglishSuggestions();
                render();
            }
            return;
        }
        if (prefix.equals(englishSuggestionRequestedPrefix)
                && englishSuggestionRequestedEpoch == englishSuggestionEpoch) return;
        clearEnglishSuggestions();
        englishSuggestionPrefix = prefix;
        long epoch = englishSuggestionEpoch;
        String resources = candidateGlossResources;
        englishSuggestionRequestedEpoch = epoch;
        englishSuggestionRequestedPrefix = prefix;
        try {
            englishSuggestionWorker.execute(() -> {
                try {
                    EnglishSuggestionModel.Result result = EnglishSuggestionModel.decode(
                        NativeClient.englishCompletions(prefix, resources));
                    main.post(() -> applyEnglishSuggestions(epoch, prefix, result));
                } catch (Exception | LinkageError ignored) {
                    // Completion failures are optional display state and never block input.
                }
            });
        } catch (RuntimeException ignored) {
            // A stopped or saturated host must not affect input.
        }
    }

    private void applyEnglishSuggestions(long epoch, String prefix,
                                         EnglishSuggestionModel.Result result) {
        if (epoch != englishSuggestionEpoch || !englishSuggestionsActive()
                || !prefix.equals(englishWordBeforeCursor())
                || !prefix.equals(result.prefix())) return;
        englishSuggestions = result.items();
        englishSuggestionPrefix = prefix;
        render();
    }

    private void useEnglishSuggestion(int slot, Button button) {
        if (slot < 0 || slot >= englishSuggestions.size() || connection == null) return;
        String typed = englishWordBeforeCursor();
        if (!typed.equals(englishSuggestionPrefix)) {
            clearEnglishSuggestions();
            render();
            return;
        }
        String candidate = englishSuggestions.get(slot);
        boolean startedCapitalized = !typed.isEmpty() && Character.isUpperCase(typed.codePointAt(0));
        EnglishSuggestionPolicy.Replacement replacement = EnglishSuggestionPolicy.replacement(
            typed, candidate, startedCapitalized);
        if (replacement == null) {
            clearEnglishSuggestions();
            render();
            return;
        }
        try {
            connection.beginBatchEdit();
            if (!connection.deleteSurroundingText(replacement.deleteCount(), 0)) return;
            if (!commitText(fullWidthOutput(replacement.insert()))) return;
        } finally {
            connection.endBatchEdit();
        }
        clearEnglishSuggestions();
        render();
    }

    private Button makeEnglishSuggestionButton(int slot) {
        Button button = new KeyboardPressButton(this);
        button.setAllCaps(false);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            useEnglishSuggestion(slot, button);
        });
        return button;
    }

    private void renderEnglishSuggestions(LinearLayout activeCandidates) {
        for (int slot = 0; slot < englishSuggestions.size(); slot++) {
            while (englishSuggestionButtons.size() <= slot)
                englishSuggestionButtons.add(makeEnglishSuggestionButton(englishSuggestionButtons.size()));
            Button button = englishSuggestionButtons.get(slot);
            String text = englishSuggestions.get(slot);
            button.setVisibility(View.VISIBLE);
            button.setText(text);
            button.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidateFontSize);
            button.setContentDescription("英文建议 " + (slot + 1) + "：" + text);
            styleButton(button, false);
            button.setTypeface(candidateTypeface());
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = englishSuggestions.size(); slot < englishSuggestionButtons.size(); slot++)
            englishSuggestionButtons.get(slot).setVisibility(View.GONE);
    }

    /** Copies Engine candidates on the IME thread, then performs only session-free IO off-thread. */
    private void scheduleCandidateGlosses() {
        if (!candidateEnglishGloss || session == 0 || view == null
                || candidateGlossResources.isEmpty()) return;
        long generation = view.optLong("generation", -1);
        if (generation < 0 || (candidateGlossRequestedSession == session
                && candidateGlossRequestedGeneration == generation)) return;
        JSONArray visible = view.optJSONArray("candidates");
        if (visible == null || visible.length() == 0) return;
        final long targetSession = session;
        final long targetEpoch = candidateGlossEpoch;
        final String targetResources = candidateGlossResources;
        final String request;
        final java.util.Map<String, String> targetRequests = new java.util.LinkedHashMap<>();
        try {
            JSONObject snapshot = value(NativeClient.allCandidates(targetSession));
            if (snapshot.optLong("session") != targetSession
                    || snapshot.optLong("generation") != generation) return;
            request = CandidateGlossModel.request(generation,
                snapshot.getJSONArray("candidates"));
            // The account path's scheme gate: a Japanese composition is not glossed into other languages. Korean Hanja rows are, as the shared translation query answers them.
            if ("none".equals(view.optString("local_mode", "none")) && view.optInt("scheme", -1) != 3) {
                for (String language : candidateOfflineTargets()) {
                    targetRequests.put(language, CandidateGlossModel.request(generation,
                        snapshot.getJSONArray("candidates"), language));
                }
            }
        } catch (JSONException | RuntimeException | LinkageError error) {
            candidateGlossRequestedSession = targetSession;
            candidateGlossRequestedGeneration = generation;
            return;
        }
        CandidateGlossPolicy.Token token =
            new CandidateGlossPolicy.Token(targetSession, generation, targetEpoch);
        candidateGlossRequestedSession = targetSession;
        candidateGlossRequestedGeneration = generation;
        try {
            candidateGlossWorker.execute(() -> {
                try {
                    CandidateGlossModel.Result result = CandidateGlossModel.decode(
                        NativeClient.candidateGlosses(request, targetResources));
                    java.util.Map<String, java.util.Map<String, String>> offline = null;
                    if (!targetRequests.isEmpty()) {
                        offline = new java.util.HashMap<>();
                        offline.put("en", glossMap(result));
                        for (java.util.Map.Entry<String, String> target : targetRequests.entrySet()) {
                            try {
                                CandidateGlossModel.Result glosses = CandidateGlossModel.decode(
                                    NativeClient.candidateGlosses(target.getValue(), targetResources));
                                if (glosses.generation() == generation)
                                    offline.put(target.getKey(), glossMap(glosses));
                            } catch (JSONException | RuntimeException | LinkageError error) {
                                // One missing or unreadable dictionary leaves the other rows intact.
                            }
                        }
                    }
                    final java.util.Map<String, java.util.Map<String, String>> targetGlosses = offline;
                    main.post(() -> applyCandidateGlosses(token, result, targetGlosses));
                } catch (JSONException | RuntimeException | LinkageError error) {
                    // Display-only lookup failures remain silent and never include candidate text.
                }
            });
        } catch (RuntimeException ignored) {
            // A stopped or saturated host must not affect input.
        }
    }

    private void applyCandidateGlosses(CandidateGlossPolicy.Token token,
            CandidateGlossModel.Result result,
            java.util.Map<String, java.util.Map<String, String>> offline) {
        long currentGeneration = view == null ? -1 : view.optLong("generation", -1);
        if (!candidateEnglishGloss || result.generation() != token.generation()
                || !token.isCurrent(session, currentGeneration, candidateGlossEpoch)) return;
        String translations = result.translations();
        if (offline != null) {
            candidateOfflineGlosses = offline;
            candidateOfflineGlossSession = token.session();
            candidateOfflineGlossGeneration = token.generation();
            translations = mergedCandidateGlosses(token.generation()).toString();
        }
        try {
            JSONObject applied = value(NativeClient.applyTranslations(
                token.session(), token.generation(), translations));
            if (!applied.optBoolean("applied", false)) return;
            JSONObject next = applied.getJSONObject("view");
            if (next.optLong("session") != token.session()
                    || next.optLong("generation") != token.generation()) return;
            view = next;
            if (candidatePanelOpen) {
                JSONObject snapshot = value(NativeClient.allCandidates(token.session()));
                if (snapshot.optLong("session") == token.session()
                        && snapshot.optLong("generation") == token.generation()) {
                    candidatePanelSnapshot = snapshot;
                }
            }
            render();
        } catch (JSONException | RuntimeException | LinkageError error) {
            // Candidate glosses are optional display state; keep the current Engine view.
        }
    }

    private void scheduleCandidateTranslations() {
        if (!candidateTranslationAccount || session == 0 || view == null
                || candidateTranslationStore == null
                || !"none".equals(view.optString("local_mode", "none"))) return;
        if (view.optInt("scheme", -1) == 3) return;
        JSONArray entries = view.optJSONArray("candidates");
        long generation = view.optLong("generation", -1);
        if (entries == null || entries.length() == 0 || generation < 0) return;
        java.util.ArrayList<String> words = new java.util.ArrayList<>();
        for (int index = 0; index < Math.min(entries.length(), 32); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null) words.add(candidate.optString("text", ""));
        }
        candidateTranslationStore.refresh(words, candidateTranslationTargets, generation);
    }

    private void applyCandidateTranslations(long generation) {
        if (!candidateTranslationAccount || session == 0 || view == null
                || view.optLong("generation", -1) != generation) return;
        JSONArray entries = view.optJSONArray("candidates");
        if (entries == null || entries.length() == 0) return;
        JSONArray translations = mergedCandidateGlosses(generation);
        if (translations.length() == 0) return;
        try {
            JSONObject applied = value(NativeClient.applyTranslations(session, generation,
                translations.toString()));
            if (!applied.optBoolean("applied", false)) return;
            JSONObject next = applied.getJSONObject("view");
            if (next.optLong("session") != session || next.optLong("generation") != generation) return;
            view = next;
            render();
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // Online translations are optional display state.
        }
    }

    /**
     * The apply_translations payload for one generation: per candidate, each target's offline gloss, else its account translation, in target order.
     *
     * <p>Without an installed non-English dictionary this is the account translations of the first 32 candidates, as before; with one it also covers every candidate the offline dictionaries answered, since the payload replaces the one applied before it.
     */
    private JSONArray mergedCandidateGlosses(long generation) {
        java.util.Map<String, java.util.Map<String, String>> offline =
            candidateOfflineGlosses != null && candidateOfflineGlossSession == session
                && candidateOfflineGlossGeneration == generation ? candidateOfflineGlosses : java.util.Map.of();
        java.util.LinkedHashSet<String> texts = new java.util.LinkedHashSet<>();
        JSONArray entries = view == null ? null : view.optJSONArray("candidates");
        for (int index = 0; entries != null && index < Math.min(entries.length(), 32); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null) texts.add(candidate.optString("text", ""));
        }
        for (java.util.Map<String, String> glosses : offline.values()) texts.addAll(glosses.keySet());
        boolean account = candidateTranslationAccount && candidateTranslationStore != null;
        JSONArray translations = new JSONArray();
        for (String text : texts) {
            java.util.HashMap<String, String> offlineRows = new java.util.HashMap<>();
            java.util.HashMap<String, String> accountRows = new java.util.HashMap<>();
            for (String target : candidateTranslationTargets) {
                java.util.Map<String, String> glosses = offline.get(target);
                if (glosses != null && glosses.containsKey(text)) offlineRows.put(target, glosses.get(text));
                if (account) {
                    String translation = candidateTranslationStore.gloss(text, target);
                    if (translation != null) accountRows.put(target, translation);
                }
            }
            String translation = CandidateTranslationPolicy.mergeGlosses(
                candidateTranslationTargets, offlineRows, accountRows);
            if (text.isEmpty() || translation.isEmpty()) continue;
            try { translations.put(new JSONObject().put("text", text).put("translation", translation)); }
            catch (JSONException ignored) { return new JSONArray(); }
        }
        return translations;
    }

    private static java.util.Map<String, String> glossMap(CandidateGlossModel.Result result)
            throws JSONException {
        JSONArray entries = new JSONArray(result.translations());
        java.util.LinkedHashMap<String, String> glosses = new java.util.LinkedHashMap<>();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.getJSONObject(index);
            glosses.put(entry.getString("text"), entry.getString("translation"));
        }
        return glosses;
    }

    /**
     * The current online query, or null when there is nothing either provider could answer.
     *
     * <p>A session with no eligible composition reports a null value rather than an error, so this
     * cannot use {@link #value} — that helper requires an object and treats its absence as a
     * failure. Every failure here is the same answer: do not ask a provider.
     */
    private JSONObject onlineQuery(long targetSession) {
        try {
            JSONObject envelope = new JSONObject(NativeClient.onlineQuery(targetSession));
            return envelope.optBoolean("ok", false) ? envelope.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    private boolean requestsCloud(JSONObject query) {
        return OnlineCandidatePolicy.requestsCloud(query.optBoolean("cloud_candidates", false),
            query.optBoolean("cloud_eligible", false));
    }

    private boolean requestsAi(JSONObject query) {
        JSONObject assistant = query.optJSONObject("ai_assistant");
        return OnlineCandidatePolicy.requestsAi(query.optBoolean("ai_eligible", false),
            assistant != null && assistant.optBoolean("enabled", false));
    }

    /** Invalidate delayed and in-flight optional provider work at an editor boundary. */
    private void invalidateOnlineProviders() {
        onlineSignature = "";
        if (onlineTask != null) {
            main.removeCallbacks(onlineTask);
            onlineTask = null;
        }
        onlineEpoch = onlineEpoch == Long.MAX_VALUE ? 0 : onlineEpoch + 1;
    }

    /**
     * The candidate texts inside a Chat Completions reply, in provider order.
     *
     * <p>An empty list means the reply supplies nothing, whether it was rejected or simply had no
     * candidates; either way nothing reaches Engine. Bounds and the per-entry rules belong to
     * {@link OnlineCandidatePolicy}, so only the envelope shape is read here.
     */
    private java.util.List<String> aiCandidateTexts(String body) {
        java.util.List<String> texts = new java.util.ArrayList<>();
        if (!OnlineCandidatePolicy.acceptsAiBody(body)) return texts;
        try {
            JSONObject envelope = new JSONObject(body);
            if (!envelope.isNull("error")) return texts;
            JSONArray choices = envelope.optJSONArray("choices");
            if (choices == null || choices.length() == 0) return texts;
            JSONObject message = choices.getJSONObject(0).optJSONObject("message");
            if (message == null) return texts;
            String content = message.optString("content", "");
            if (!OnlineCandidatePolicy.acceptsAiContent(content)) return texts;
            JSONArray entries = new JSONObject(content).optJSONArray("candidates");
            if (entries == null) return texts;
            for (int index = 0; index < entries.length(); index++) {
                JSONObject entry = entries.optJSONObject(index);
                if (entry != null) texts.add(entry.optString("text", ""));
            }
            return texts;
        } catch (JSONException | RuntimeException error) {
            texts.clear();
            return texts;
        }
    }

    /** The cloud service URL the shared host built for this query, or empty when unavailable. */
    private String cloudRequestUrl(String document) {
        try {
            JSONObject envelope = new JSONObject(NativeClient.cloudRequestUrl(document));
            return envelope.optBoolean("ok", false) ? envelope.optString("value", "") : "";
        } catch (JSONException | RuntimeException | LinkageError error) {
            return "";
        }
    }

    /** The AI HTTPS descriptor in this envelope, or null when it no longer matches the settings. */
    private static JSONObject aiRequestDescriptor(String raw) {
        if (raw == null) return null;
        try {
            JSONObject envelope = new JSONObject(raw);
            return envelope.optBoolean("ok", false) ? envelope.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException error) {
            return null;
        }
    }

    /**
     * 云联想和 AI 联想：组字停下来之后再问，不是每敲一个键都问。
     *
     * <p>Both providers answer a query Engine has usually moved past by the time the network
     * replies, so the request carries the query document it was built from and the shared host
     * discards anything that no longer matches. The signature is what stops a second request for a
     * state already asked about; the epoch is what stops a late reply from a previous composition.
     */
    private void scheduleOnlineProviders() {
        if (session == 0) {
            invalidateOnlineProviders();
            return;
        }
        JSONObject query = onlineQuery(session);
        if (query == null || (!requestsCloud(query) && !requestsAi(query))) {
            invalidateOnlineProviders();
            return;
        }
        JSONObject assistant = query.optJSONObject("ai_assistant");
        String signature = OnlineCandidatePolicy.signature(query.optLong("session_id", 0),
            query.optString("cache_key", ""), query.optString("identity", ""),
            query.optBoolean("cloud_candidates", false),
            assistant != null && assistant.optBoolean("enabled", false) ? assistant.toString() : "");
        if (signature.equals(onlineSignature)) return;
        onlineSignature = signature;
        if (onlineTask != null) main.removeCallbacks(onlineTask);
        onlineEpoch = onlineEpoch == Long.MAX_VALUE ? 0 : onlineEpoch + 1;
        final long epoch = onlineEpoch;
        final long targetSession = session;
        final String document = query.toString();
        final String requestSignature = signature;
        onlineTask = () -> {
            onlineTask = null;
            if (epoch != onlineEpoch || targetSession != session) return;
            try {
                onlineCandidateWorker.execute(() -> fetchOnlineCandidates(
                    targetSession, document, epoch, requestSignature));
            } catch (RuntimeException ignored) {
                releaseOnlineSignatureIfCurrent(requestSignature, epoch, targetSession);
            }
        };
        main.postDelayed(onlineTask, OnlineCandidatePolicy.QUIET_INTERVAL_MILLIS);
    }

    /** 在线 worker 执行网络请求；失败时释放仍属于本请求的签名。 */
    private void fetchOnlineCandidates(long targetSession, String document, long epoch,
            String requestSignature) {
        boolean cloudComplete = false;
        boolean aiComplete = false;
        try {
            JSONObject query = new JSONObject(document);
            cloudComplete = !requestsCloud(query);
            String aiDocument = document;
            if (requestsCloud(query)) {
                String url = cloudRequestUrl(document);
                if (!url.isEmpty()) {
                    String body = OnlineCandidateTransport.cloud(url);
                    if (OnlineCandidatePolicy.acceptsCloudBody(body)) {
                        String refreshed = applyOnlineResult(targetSession, epoch,
                            () -> NativeClient.applyCloudResponse(targetSession, document, body));
                        cloudComplete = refreshed != null;
                        if (refreshed != null) aiDocument = refreshed;
                    }
                }
            }
            if (epoch != onlineEpoch) return;
            JSONObject aiQuery = new JSONObject(aiDocument);
            JSONObject aiAssistant = aiQuery.optJSONObject("ai_assistant");
            int limit = OnlineCandidatePolicy.aiCandidateLimit(
                aiAssistant == null ? 0 : aiAssistant.optInt("candidate_limit", 0));
            if (!requestsAi(aiQuery) || limit == 0) {
                aiComplete = true;
                return;
            }
            final String queryDocument = aiDocument;
            JSONObject descriptor = aiRequestDescriptor(onSessionThread(targetSession, epoch,
                () -> NativeClient.aiRequestForQuery(targetSession, queryDocument)));
            if (descriptor == null) return;
            java.util.List<String> candidates = OnlineCandidatePolicy.aiCandidates(
                aiCandidateTexts(OnlineCandidateTransport.ai(descriptor)), limit);
            if (candidates.isEmpty()) return;
            JSONArray payload = new JSONArray();
            for (String candidate : candidates) payload.put(candidate);
            final String finalDocument = aiDocument;
            aiComplete = applyOnlineResult(targetSession, epoch,
                () -> NativeClient.applyOnlineCandidates(
                    targetSession, finalDocument, payload.toString(), 1)) != null;
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 在线候选是可选能力，解析或服务异常交给 finally 触发下一次重试。
        } finally {
            if (!cloudComplete || !aiComplete)
                releaseOnlineSignatureIfCurrent(requestSignature, epoch, targetSession);
        }
    }

    private void releaseOnlineSignatureIfCurrent(String requestSignature, long requestEpoch,
            long targetSession) {
        Runnable release = () -> {
            if (OnlineCandidatePolicy.shouldReleaseAfterFailure(requestSignature, onlineSignature,
                    requestEpoch, onlineEpoch, targetSession, session)) onlineSignature = "";
        };
        if (Looper.myLooper() == main.getLooper()) release.run();
        else main.post(release);
    }

    /**
     * Hand one provider's result back to Engine on the session thread.
     *
     * <p>Returns the query document as it stands after a result was applied, so the next provider
     * can use it, or null when nothing was applied.
     */
    private String applyOnlineResult(long targetSession, long epoch,
            java.util.function.Supplier<String> call) {
        final java.util.concurrent.atomic.AtomicReference<String> refreshed =
            new java.util.concurrent.atomic.AtomicReference<>();
        final java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        main.post(() -> {
            try {
                if (epoch != onlineEpoch || targetSession != session) return;
                JSONObject applied = value(call.get());
                if (!applied.optBoolean("applied", false)) return;
                JSONObject next = applied.getJSONObject("view");
                if (next.optLong("session") != targetSession) return;
                view = next;
                render();
                JSONObject current = onlineQuery(targetSession);
                if (current != null) refreshed.set(current.toString());
            } catch (JSONException | RuntimeException | LinkageError error) {
                // Online candidates are optional; keep the current Engine view.
            } finally {
                done.countDown();
            }
        });
        try {
            if (!done.await(2, TimeUnit.SECONDS)) return null;
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            return null;
        }
        return refreshed.get();
    }

    /** Run a session-bound call on the session thread and return its raw result, or null. */
    private String onSessionThread(long targetSession, long epoch,
            java.util.function.Supplier<String> call) {
        final java.util.concurrent.atomic.AtomicReference<String> result =
            new java.util.concurrent.atomic.AtomicReference<>();
        final java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        main.post(() -> {
            try {
                if (epoch != onlineEpoch || targetSession != session) return;
                result.set(call.get());
            } catch (RuntimeException | LinkageError error) {
                // Online candidates are optional.
            } finally {
                done.countDown();
            }
        });
        try {
            if (!done.await(2, TimeUnit.SECONDS)) return null;
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            return null;
        }
        return result.get();
    }

    private int candidateGlossLineCount() {
        return CandidateTranslationPolicy.glossLines(
            candidateTranslationTargets, candidateEnglishGloss, candidateTranslationAccount,
            candidateOfflineTargets());
    }

    private void updateCandidateViewportHeight() {
        if (candidateViewport == null) return;
        android.view.ViewGroup.LayoutParams params = candidateViewport.getLayoutParams();
        if (params == null) return;
        int height = pixels(KeyboardGeometry.CANDIDATE_ROW_HEIGHT_DP
            + CandidateTranslationPolicy.reservedGlossRows(candidateGlossLineCount(), koreanHanjaRows()) * 16);
        if (params.height == height) return;
        params.height = height;
        candidateViewport.setLayoutParams(params);
    }

    private void showDiagnostic(String value) {
        diagnosticGeneration++;
        if (diagnosticDismissTask != null) {
            main.removeCallbacks(diagnosticDismissTask);
            diagnosticDismissTask = null;
        }
        diagnosticMessage = InputDiagnosticPolicy.normalize(value);
        if (diagnosticMessage.isEmpty()) return;
        long generation = diagnosticGeneration;
        diagnosticDismissTask = () -> {
            if (generation != diagnosticGeneration) return;
            diagnosticMessage = "";
            diagnosticDismissTask = null;
            render();
        };
        main.postDelayed(diagnosticDismissTask, InputDiagnosticPolicy.DISMISS_DELAY_MILLIS);
    }

    private void clearDiagnostic() {
        diagnosticGeneration++;
        if (diagnosticDismissTask != null) {
            main.removeCallbacks(diagnosticDismissTask);
            diagnosticDismissTask = null;
        }
        diagnosticMessage = "";
    }

    private void fail() { stop(false); message = "输入连接失败：仅直接输入"; render(); }

    private boolean character(int ascii) {
        return character(ascii, letterCase.usesUppercase());
    }

    private boolean character(int ascii, boolean shifted) {
        if (session == 0) return false;
        try { return apply(NativeClient.character(session, ascii, shifted)); }
        catch (JSONException | LinkageError error) { fail(); return true; }
    }

    private boolean helpcodeCompositionEligible() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.eligible(dedicatedEnglish,
            view.optString("editing_text", ""), view.optInt("scheme", -1),
            view.optString("local_mode", "none"));
    }

    private boolean entersHelpcode() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.entersHelpcode(dedicatedEnglish, letterCase.usesUppercase(),
            view.optString("editing_text", ""), view.optInt("scheme", -1),
            view.optString("local_mode", "none"));
    }

    private boolean command(int code) {
        if (session == 0) return false;
        try { return apply(NativeClient.command(session, code)); }
        catch (JSONException | LinkageError error) { fail(); return true; }
    }

    private void type(char key) {
        if (connection == null) return;
        if (directEnglishActive()) {
            char output = letterCase.usesUppercase() ? Character.toUpperCase(key) : key;
            if (isAsciiLetter(output)) {
                commitEnglishLiteral(output);
                if (letterCase.consumeLetter()) {
                    rebuildKeyRows();
                    render();
                }
                return;
            }
        }
        if (dedicatedEnglish && !isAsciiLetter(key)) {
            commitEnglishLiteral(key);
            return;
        }
        if (koreanSchemeActive() && isAsciiLetter(key)) {
            // Shift picks the double consonant or ㅒ ㅖ; the other keys send their lowercase letter, which types the same jamo.
            char input = KoreanKeyboardLayout.input(key, letterCase.usesUppercase());
            if (!character(input, Character.isUpperCase(input))) commitText(String.valueOf(input));
            if (letterCase.consumeLetter()) {
                rebuildKeyRows();
                render();
            }
            return;
        }
        if (entersHelpcode() && isAsciiLetter(key)) {
            character(Character.toUpperCase(key), true);
            if (letterCase.consumeLetter()) {
                rebuildKeyRows();
                render();
            }
            return;
        }
        char output = letterCase.usesUppercase() ? Character.toUpperCase(key) : key;
        boolean punctuationKey = SmartPunctuationContext.isAsciiPunctuation(output);
        boolean handled = punctuationKey ? punctuation(output) : character(output);
        if (!handled) {
            // 引擎不收的标点是一次自动上屏，先把组合按首选结束掉。The preedit is a real composing
            // region, so committing into it would replace the pinyin instead of following it.
            if (DeclinedKeyPolicy.finishesComposition(punctuationKey, hasEngineComposition())) {
                command(FINISH_COMPOSITION_COMMAND);
            }
            commitText(fullWidthOutput(String.valueOf(output)));
        }
        if (letterCase.consumeLetter()) {
            rebuildKeyRows();
            render();
        }
    }

    private boolean punctuation(int ascii) {
        if (session == 0) return false;
        CharSequence before = null;
        if (connection != null) {
            try {
                // Two UTF-16 code units are sufficient for the immediately preceding scalar.
                before = connection.getTextBeforeCursor(2, 0);
            } catch (RuntimeException ignored) {
                // Editor context is optional and must never be logged or persisted.
            }
        }
        int preceding = SmartPunctuationContext.precedingCodePoint(before);
        try {
            JSONObject decision = smartPunctuationDecision((char) ascii, before);
            // `isNull` first: org.json's optString hands back the four-letter string "null" for a
            // JSON null, not the fallback. Reading it without this asked the editor to delete the
            // character before the cursor and commit "null" - on every punctuation key.
            String replacement = decision == null || decision.isNull("replace_with")
                ? null : decision.optString("replace_with", null);
            if (replacement != null && !replacement.isEmpty()) {
                if (connection == null || !connection.deleteSurroundingText(1, 0)) return false;
                smartRepeatSnapshot = null;
                return commitText(replacement);
            }
            String response = NativeClient.punctuationWithContext(session, ascii, preceding);
            boolean handled = apply(response);
            armSmartPunctuation(ascii, response, false);
            return handled;
        }
        catch (JSONException | LinkageError error) { fail(); return true; }
    }

    private void clearSmartPunctuationSnapshots() {
        smartRepeatSnapshot = null;
        smartSpaceSnapshot = null;
    }

    private JSONObject smartPunctuationDecision(char character, CharSequence before) {
        if (session == 0) return null;
        try {
            int preceding = SmartPunctuationContext.precedingCodePoint(before);
            JSONObject request = new JSONObject().put("character", (int) character)
                .put("preceding", preceding == 0 ? JSONObject.NULL
                    : String.valueOf(Character.toChars(preceding)))
                .put("timestamp_ms", SystemClock.elapsedRealtime())
                .put("editor_generation", editorContextRevision)
                .put("repeat", smartRepeatSnapshot == null ? JSONObject.NULL : smartRepeatSnapshot)
                .put("space", smartSpaceSnapshot == null ? JSONObject.NULL : smartSpaceSnapshot);
            return value(NativeClient.smartPunctuationDecide(session, request.toString()));
        } catch (Exception | LinkageError ignored) {
            return null;
        }
    }

    private void armSmartPunctuation(int ascii, String response, boolean autoClosedPair) {
        if (session == 0) return;
        try {
            JSONObject result = value(response);
            JSONObject request = new JSONObject().put("ascii", ascii)
                .put("commit", result.isNull("commit") ? "" : result.optString("commit", ""))
                .put("timestamp_ms", SystemClock.elapsedRealtime())
                .put("editor_generation", editorContextRevision)
                .put("auto_closed_pair", autoClosedPair);
            JSONObject armed = value(NativeClient.smartPunctuationArm(session, request.toString()));
            smartRepeatSnapshot = armed.isNull("repeat") ? null : armed.getJSONObject("repeat");
            smartSpaceSnapshot = armed.isNull("space") ? null : armed.getJSONObject("space");
        } catch (Exception | LinkageError ignored) {
            clearSmartPunctuationSnapshots();
        }
    }

    private static boolean isAsciiLetter(int value) {
        return (value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z');
    }

    private void commitEnglishLiteral(int value) {
        if (connection == null || value < 32 || value > 126) return;
        if (englishNineKeyActive() && session != 0) command(2);
        commitText(fullWidthOutput(String.valueOf((char) value)));
        if (directEnglishActive()) refreshEnglishSuggestions();
    }

    private void space() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        JSONObject spaceDecision = smartPunctuationDecision(' ', getTextBeforeCursor());
        if (spaceDecision != null && !spaceDecision.isNull("space_ascii")) {
            int ascii = spaceDecision.optInt("space_ascii", 0);
            if (ascii >= 32 && ascii <= 126 && connection.deleteSurroundingText(1, 0)) {
                smartSpaceSnapshot = null;
                commitText(String.valueOf((char) ascii));
                return;
            }
        }
        if (japaneseSchemeActive() && view != null) {
            String editingText = view.optString("editing_text", "");
            JSONArray candidates = view.optJSONArray("candidates");
            int count = candidates == null ? 0 : candidates.length();
            JSONObject first = count > 0 ? candidates.optJSONObject(0) : null;
            int firstSource = first == null ? -1 : first.optInt("source", -1);
            if (!editingText.isEmpty() && JapaneseSpacePolicy.converts(count, firstSource)) {
                if (japaneseConversionIndex == null
                        || !editingText.equals(japaneseConversionEditingText)) {
                    japaneseConversionIndex = 0;
                    japaneseConversionEditingText = editingText;
                    render();
                } else {
                    int next = japaneseConversionIndex + 1;
                    japaneseConversionIndex = next < count ? next : 0;
                    command(next < count ? 102 : 104);
                }
                return;
            }
        }
        if (directEnglishActive()) {
            commitEnglishLiteral(' ');
            return;
        }
        if (dedicatedEnglish) {
            if (session != 0) command(1);
            if (connection != null) commitText(fullWidthOutput(" "));
        } else if (!command(1)) {
            commitText(fullWidthOutput(" "));
        }
    }

    private CharSequence getTextBeforeCursor() {
        if (connection == null) return null;
        try { return connection.getTextBeforeCursor(2, 0); }
        catch (RuntimeException ignored) { return null; }
    }

    private boolean japaneseSchemeActive() {
        return view != null && view.optInt("scheme", -1) == 3;
    }

    private boolean koreanSchemeActive() {
        return view != null
            && KoreanInputPolicy.active(view.optInt("scheme", -1), dedicatedEnglish);
    }

    /** Whether the Hanja list of the composing Korean syllable is on the strip. */
    private boolean koreanHanjaListOpen() {
        if (view == null) return false;
        JSONArray entries = view.optJSONArray("candidates");
        return KoreanInputPolicy.hanjaListOpen(koreanSchemeActive(),
            view.optString("local_mode", "none"), entries == null ? 0 : entries.length());
    }

    /** Drops the composition without writing it. With a Korean Hanja list open the first cancel only closes the list, so this sends as many as KoreanInputPolicy.cancelsToDiscard says. */
    private void discardComposition() {
        for (int cancels = KoreanInputPolicy.cancelsToDiscard(koreanHanjaListOpen()); cancels > 0; cancels--)
            command(3);
    }

    /** Whether the Hanja command applies now: a Korean syllable is composing, with or without its list open. */
    private boolean koreanConvertsHanja() {
        return view != null && KoreanInputPolicy.convertsHanja(koreanSchemeActive(),
            view.optString("local_mode", "none"), view.optString("editing_text", ""));
    }

    private boolean japaneseNineKeyActive() {
        return displayedTouchLayout(view) == JAPANESE_NINE_KEY_LAYOUT;
    }

    private String spaceKeyTitle() {
        return japaneseSchemeActive()
            ? JapaneseNineKeyActions.spaceTitle(view != null
                && !view.optString("editing_text", "").isEmpty()) : "空格";
    }

    private String spaceKeyDescription() {
        return japaneseSchemeActive()
            ? spaceKeyTitle() + "；左右滑动移动光标" : SPACE_CURSOR_DESCRIPTION;
    }

    private int displayedTouchLayout(JSONObject value) {
        return dedicatedEnglish ? STANDARD_TOUCH_LAYOUT : touchLayout(value);
    }

    private boolean sendsChinesePunctuation() {
        return view != null && ChineseSymbolFaces.shouldUseChineseFaces(dedicatedEnglish,
            view.optInt("scheme", -1), view.optString("local_mode", "none"), chinesePunctuation);
    }

    private java.util.List<QuickPunctuationPolicy.Entry> quickPunctuationEntries() {
        return QuickPunctuationPolicy.entries(dedicatedEnglish,
            view == null ? -1 : view.optInt("scheme", -1),
            view == null ? "none" : view.optString("local_mode", "none"));
    }

    private boolean quickPunctuationVisible() {
        int layout = displayedTouchLayout(view);
        return keyboardLayer == KeyboardLayout.Layer.LETTERS
            && layout != QUANPIN_NINE_KEY_LAYOUT && layout != JAPANESE_NINE_KEY_LAYOUT
            && selectedScheme != KeyboardScheme.QUANPIN_NINE_KEY
            && selectedScheme != KeyboardScheme.JAPANESE_NINE_KEY;
    }

    private void insertQuickPunctuation() {
        java.util.List<QuickPunctuationPolicy.Entry> entries = quickPunctuationEntries();
        if (!entries.isEmpty()) type(entries.get(0).input());
    }

    private void showQuickPunctuationMenu() {
        if (quickPunctuationButton == null || !quickPunctuationVisible()) return;
        PopupMenu popup = new PopupMenu(this, quickPunctuationButton);
        for (QuickPunctuationPolicy.Entry entry : quickPunctuationEntries()) {
            popup.getMenu().add(entry.face()).setOnMenuItemClickListener(ignored -> {
                playFeedback(quickPunctuationButton);
                type(entry.input());
                return true;
            });
        }
        popup.show();
    }

    private void updateQuickPunctuation() {
        if (quickPunctuationButton == null) return;
        java.util.List<QuickPunctuationPolicy.Entry> entries = quickPunctuationEntries();
        boolean visible = quickPunctuationVisible() && !entries.isEmpty();
        quickPunctuationButton.setVisibility(visible ? View.VISIBLE : View.GONE);
        if (!visible) return;
        String face = entries.get(0).face();
        quickPunctuationButton.setText(face);
        quickPunctuationButton.setContentDescription("常用标点：" + face
            + "；长按选择常用标点");
    }

    private void updateSymbolKeyFaces() {
        boolean chineseMode = sendsChinesePunctuation();
        for (int index = 0; index < symbolKeyButtons.size(); index++) {
            String face = ChineseSymbolFaces.face(symbolKeyInputs.get(index), chineseMode);
            Button button = symbolKeyButtons.get(index);
            button.setText(face);
            button.setContentDescription("按键 " + face);
        }
    }

    private void updateShuangpinKeyHints() {
        int scheme = view == null ? -1 : view.optInt("scheme", -1);
        String profile = view == null ? "" : view.optString("shuangpin_profile", "");
        String localMode = view == null ? "none" : view.optString("local_mode", "none");
        boolean chineseMode = !dedicatedEnglish;
        boolean local = view != null && !"none".equals(localMode);
        boolean shifted = letterCase.usesUppercase();
        if (!profile.equals(shuangpinHintsProfile)) {
            shuangpinHintsProfile = profile;
            shuangpinHints = java.util.Map.of();
            if (!profile.isEmpty()) {
                try {
                    shuangpinHints = ShuangpinKeyHintPolicy.decode(
                        NativeClient.shuangpinKeyHints(profile));
                } catch (RuntimeException | LinkageError ignored) {
                    // Native/profile failures hide hints rather than guessing another keymap.
                }
            }
        }
        for (int index = 0; index < shuangpinKeyButtons.size(); index++) {
            ShuangpinHintButton button = shuangpinKeyButtons.get(index);
            String input = shuangpinKeyInputs.get(index);
            String hint = ShuangpinKeyHintPolicy.hint(
                shuangpinHints, input, dedicatedEnglish, scheme, localMode);
            button.setHintText(hint);
            button.setHintColor(Color.parseColor(skin.accent()));
            if (";".equals(input)) continue;
            String description = LetterKeyFacePolicy.accessibilityLabel(
                input, chineseMode, local, shifted);
            button.setContentDescription(hint.isEmpty()
                ? description : description + "；双拼提示 " + hint);
        }
    }

    private void updateAutomaticCapitalization() {
        if (!dedicatedEnglish) {
            // On the Korean keycaps Shift is the double consonant the user chose, not capitalisation, so an editor update must not drop it.
            if (!koreanSchemeActive()) letterCase.reset();
            return;
        }
        CharSequence context = null;
        if (connection != null) {
            try {
                context = connection.getTextBeforeCursor(CAPITALIZATION_CONTEXT_LIMIT, 0);
            } catch (RuntimeException ignored) {
                // Editor context is optional and must never be logged or persisted.
            }
        }
        boolean next = EnglishCapitalizationPolicy.shouldShift(
            EditorPolicy.capitalizationMode(editorInputType), context);
        if (letterCase.applyAutomatic(next)) rebuildKeyRows();
        render();
    }

    private void toggleInputLanguage() {
        if (session == 0) return;
        boolean nextEnglish = !dedicatedEnglish;
        if (dedicatedEnglish) command(3); else command(2);
        if (session == 0) return;
        int previousLayout = displayedTouchLayout(view);
        boolean previousUppercase = letterCase.usesUppercase();
        try {
            JSONObject nextView = value(NativeClient.setEnglishMode(session, nextEnglish));
            dedicatedEnglish = nextEnglish;
            if (inputModeStore != null) {
                inputModeStore.remember(imeModeScope, currentEditorPackage, nextEnglish);
            }
            view = nextView;
            keyboardLayer = KeyboardLayout.Layer.LETTERS;
            letterCase.reset();
            if (previousLayout != displayedTouchLayout(view) || previousUppercase) rebuildKeyRows();
            updateAutomaticCapitalization();
            if (directEnglishActive()) refreshEnglishSuggestions();
            else { clearEnglishSuggestions(); render(); }
        } catch (JSONException | LinkageError error) {
            fail();
        }
    }

    private static int touchLayout(JSONObject value) {
        if (value == null) return STANDARD_TOUCH_LAYOUT;
        return KeyboardLayout.resolveTouchLayout(
            "handwriting".equals(value.optString("touch_keyboard_layout")),
            value.optBoolean("nine_key", false), value.optInt("scheme", -1),
            value.optString("touch_keyboard_layout"));
    }

    private void enter() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        // With a Korean Hanja list open Return chooses the highlighted Hanja, as Space does: only the session knows the highlight, so it is the candidate command (msime_client.h). With no list Return writes the syllable out and then does its editor action below.
        if (koreanHanjaListOpen() && command(1)) return;
        if (japaneseSchemeActive() && view != null
                && !view.optString("editing_text", "").isEmpty()) {
            if (japaneseConversionIndex != null && command(1)) return;
            if (command(11)) return;
        }
        if (command(2)) return;
        EditorInfo info = getCurrentInputEditorInfo();
        int action = info == null ? EditorInfo.IME_ACTION_NONE : info.imeOptions & EditorInfo.IME_MASK_ACTION;
        boolean disabled = info == null
                || (info.imeOptions & EditorInfo.IME_FLAG_NO_ENTER_ACTION) != 0;
        if (ReturnKeyAction.shouldPerformEditorAction(action, disabled, false)
                && connection.performEditorAction(action)) return;
        commitText("\n");
    }

    /**
     * Whether the return key only confirms the composition. A Korean syllable is committed by Enter and the key still does its own work, so the key keeps its editor action there, unless the syllable's Hanja list is open: then Enter only chooses a Hanja.
     */
    private boolean returnKeyConfirms() {
        return view != null && !view.optString("editing_text", "").isEmpty()
            && (!koreanSchemeActive() || koreanHanjaListOpen());
    }

    /** The return key's face: accent-filled 确认 while composing, the function tint otherwise. */
    private KeyboardKeyRole returnKeyRole() {
        boolean composing = returnKeyConfirms();
        return composing ? KeyboardKeyRole.RETURN : KeyboardKeyRole.ACCENT;
    }

    private void updateReturnKey() {
        EditorInfo info = getCurrentInputEditorInfo();
        int action = info == null ? EditorInfo.IME_ACTION_NONE
                : info.imeOptions & EditorInfo.IME_MASK_ACTION;
        boolean disabled = info == null
                || (info.imeOptions & EditorInfo.IME_FLAG_NO_ENTER_ACTION) != 0;
        boolean composing = returnKeyConfirms();
        String title = ReturnKeyAction.title(action, disabled);
        // The shared design turns 换行 into an accent-filled 确认 while a composition is open,
        // because that is what the key does then: it commits the reading instead of a newline.
        String shownTitle = composing ? (japaneseSchemeActive() ? "確定" : "确认") : title;
        if (enterButton != null) {
            enterButton.setText(shownTitle);
            enterButton.setContentDescription(shownTitle);
            KeyboardKeyRole role = returnKeyRole();
            if (enterButton instanceof KeyboardPressButton press && press.keyboardRole() != role) {
                press.setKeyboardRole(role);
                styleButton(enterButton, role, skin);
            }
        }
        if (japaneseReturnKey != null) {
            String japaneseTitle = JapaneseNineKeyActions.returnTitle(composing);
            japaneseReturnKey.setText(japaneseTitle);
            japaneseReturnKey.setContentDescription(japaneseTitle);
        }
        if (japaneseSpaceKey != null) {
            japaneseSpaceKey.setText(spaceKeyTitle());
            japaneseSpaceKey.setContentDescription(spaceKeyDescription());
        }
        if (japaneseSymbolsKey != null) {
            boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
            japaneseSymbolsKey.setText(symbols ? "あいう" : "123");
            japaneseSymbolsKey.setContentDescription(symbols
                ? "切换到假名" : "切换到数字和符号");
        }
        if (japaneseVariantsButton != null) {
            boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
            boolean enabled = symbols ? japaneseNineKeyActive() : JapaneseVariantPolicy.enabled(
                japaneseNineKeyActive(), false, composing);
            japaneseVariantsButton.setEnabled(enabled);
            japaneseVariantsButton.setContentDescription(
                symbols ? "括号；长按选择其他括号"
                    : JapaneseVariantPolicy.accessibilityLabel(enabled));
        }
        if (spaceButton != null && !cursorMovement.isActive()) {
            spaceButton.setText(spaceKeyTitle());
            spaceButton.setContentDescription(spaceKeyDescription());
        }
    }

    private void resetSpaceCursor() {
        cursorMovement.cancel();
        if (spaceButton != null) {
            spaceButton.setPressed(false);
            spaceButton.setText(spaceKeyTitle());
            spaceButton.setContentDescription(spaceKeyDescription());
        }
        if (japaneseSpaceKey != null && japaneseSpaceKey != spaceButton) {
            japaneseSpaceKey.setPressed(false);
            japaneseSpaceKey.setText(spaceKeyTitle());
            japaneseSpaceKey.setContentDescription(spaceKeyDescription());
        }
    }

    private void moveEditorCursor(int offset) {
        int keyCode = offset < 0 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
        for (int index = 0; index < Math.abs(offset); index++) sendDownUpKeyEvents(keyCode);
    }

    private void bindSpaceCursor(Button button) {
        final float[] origin = new float[2];
        final boolean[] dragging = new boolean[1];
        final boolean[] cancelled = new boolean[1];
        final int touchSlop = ViewConfiguration.get(this).getScaledTouchSlop();
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    dragging[0] = false;
                    cancelled[0] = false;
                    button.setPressed(true);
                    button.getParent().requestDisallowInterceptTouchEvent(true);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (!dragging[0] && !cancelled[0]) {
                        float horizontal = event.getX() - origin[0];
                        float vertical = event.getY() - origin[1];
                        if (Math.abs(horizontal) <= touchSlop && Math.abs(vertical) <= touchSlop)
                            return true;
                        if (Math.abs(horizontal) <= Math.abs(vertical) || connection == null) {
                            cancelled[0] = true;
                            button.setPressed(false);
                            return true;
                        }
                        command(2);
                        cursorMovement.begin(origin[0], connection);
                        dragging[0] = cursorMovement.isActive();
                        cancelled[0] = !dragging[0];
                        button.setPressed(false);
                        if (dragging[0]) {
                            button.setText("移动光标");
                            button.setContentDescription("正在移动光标");
                            moveEditorCursor(cursorMovement.advance(
                                event.getX(), connection, pixels(12)));
                        }
                        return true;
                    }
                    if (dragging[0]) {
                        moveEditorCursor(cursorMovement.advance(
                            event.getX(), connection, pixels(12)));
                        if (!cursorMovement.isActive()) {
                            dragging[0] = false;
                            cancelled[0] = true;
                            resetSpaceCursor();
                        }
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    button.setPressed(false);
                    if (dragging[0] || cancelled[0]) {
                        // The thumb still pressed the space bar; dragging it moved the cursor instead of typing.
                        countKey(button);
                        resetSpaceCursor();
                    } else button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    dragging[0] = false;
                    cancelled[0] = true;
                    resetSpaceCursor();
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
        // Every physical press counts once, whoever ends up handling it; the OS's auto-repeat does not.
        if (event.getRepeatCount() == 0) countKey(KeyPressIds.forKeyCode(keyCode));
        if (keyCode == KeyEvent.KEYCODE_BACK && emojiPickerVisible()) {
            closeEmojiPicker();
            return true;
        }
        if (event.getRepeatCount() == 0
                && (keyCode == KeyEvent.KEYCODE_SHIFT_LEFT || keyCode == KeyEvent.KEYCODE_SHIFT_RIGHT
                    || keyCode == KeyEvent.KEYCODE_CTRL_LEFT || keyCode == KeyEvent.KEYCODE_CTRL_RIGHT)) {
            modifierTapStartedAt = android.os.SystemClock.uptimeMillis();
            modifierTapKeyCode = keyCode;
            modifierTapInvalid = false;
            return true;
        }
        if (modifierTapStartedAt >= 0 && keyCode != modifierTapKeyCode) modifierTapInvalid = true;
        // F9 while a Korean syllable composes converts it to Hanja, or closes its open list. The key stays the input method's whatever the Engine answers: a lone jamo has no Hanja, and F9 handed on would reach the editor beside a syllable still composing. A held key converts once, so the list does not flicker open and shut. With nothing composing F9 is the editor's as before.
        if (KoreanInputPolicy.hanjaKey(keyCode, event.isShiftPressed(), event.isCtrlPressed(),
                event.isAltPressed(), event.isMetaPressed()) && session != 0 && koreanConvertsHanja()) {
            if (event.getRepeatCount() == 0) command(KoreanInputPolicy.CONVERT_HANJA_COMMAND);
            return true;
        }
        HardwareShortcutPolicy.Action shortcut = HardwareShortcutPolicy.chord(
            keyCode, event.isShiftPressed(), event.isCtrlPressed(), event.isAltPressed(),
            event.getRepeatCount(), hardwareLanguageShift, hardwareCharacterSet, hardwareFullWidth);
        if (keyCode == KeyEvent.KEYCODE_SPACE && event.isCtrlPressed() && event.isAltPressed()) {
            shortcut = hardwareLanguageCtrlAltSpace
                ? HardwareShortcutPolicy.Action.TOGGLE_LANGUAGE : HardwareShortcutPolicy.Action.NONE;
        }
        if (shortcut == HardwareShortcutPolicy.Action.TOGGLE_LANGUAGE) {
            toggleInputLanguage();
            return true;
        }
        if (shortcut == HardwareShortcutPolicy.Action.TOGGLE_CHARACTER_SET) {
            toggleChineseOutput();
            return true;
        }
        if (shortcut == HardwareShortcutPolicy.Action.TOGGLE_FULL_WIDTH) {
            toggleFullWidthInput();
            return true;
        }
        if (shortcut == HardwareShortcutPolicy.Action.TOGGLE_PUNCTUATION) {
            toggleChinesePunctuation();
            return true;
        }
        // Shift belongs to the policy rather than to this condition: which face of the number row
        // picks a candidate depends on the local mode, because in U mode the plain digits are the
        // code point and the pick moves to the shifted face.
        int candidateSlot = NumberRowSelectionPolicy.slotForKeyCode(keyCode,
            event.isShiftPressed(), numberRowSelection,
            view == null ? "none" : view.optString("local_mode", "none"));
        if (candidateSlot >= 0 && !dedicatedEnglish
                && !event.isCtrlPressed() && !event.isAltPressed() && !event.isMetaPressed()
                && view != null
                && view.optJSONArray("candidates") != null
                && candidateSlot < view.optJSONArray("candidates").length()
                && selectHardwareCandidate(candidateSlot)) return true;
        if (directEnglishActive() && !event.isCtrlPressed() && !event.isAltPressed()
                && !event.isMetaPressed()) {
            if (keyCode == KeyEvent.KEYCODE_DEL) {
                clearEnglishSuggestions();
                return super.onKeyDown(keyCode, event);
            }
            int unicode = event.getUnicodeChar();
            if (unicode >= 32 && unicode <= 126) {
                if (isAsciiLetter(unicode)) {
                    char output = letterCase.usesUppercase() || event.isShiftPressed()
                        ? Character.toUpperCase((char) unicode) : Character.toLowerCase((char) unicode);
                    commitEnglishLiteral(output);
                    if (letterCase.consumeLetter()) rebuildKeyRows();
                } else {
                    commitEnglishLiteral(unicode);
                }
                render();
                return true;
            }
        }
        // Before the modifier bail-out below, because both maintenance chords are Ctrl+Shift+Alt
        // and that branch hands every such combination to the application.
        int maintenance = HardwareMaintenancePolicy.action(keyCode, event.isShiftPressed(),
            event.isCtrlPressed(), event.isAltPressed(), event.isMetaPressed(),
            event.getRepeatCount(), hasEngineComposition());
        if (maintenance != HardwareMaintenancePolicy.NONE && session != 0
                && runMaintenanceChord(maintenance)) {
            return true;
        }
        if (session == 0 || event.isCtrlPressed() || event.isAltPressed() || event.isMetaPressed()) {
            if (session != 0 && connection != null) {
                // Preserve displayed source text before the editor handles a shortcut.
                command(2);
            }
            return super.onKeyDown(keyCode, event);
        }
        if (keyCode == KeyEvent.KEYCODE_DEL && handwritingActive()) {
            deleteFromHandwriting();
            return true;
        }
        // A Korean Hanja is one character already, so there is no word to take one from: with the list open the pair is punctuation, which closes the list and writes the Hangul with the mark.
        WordCharacterPolicy.Edge wordCharacterEdge = WordCharacterPolicy.edgeFor(
            keyCode, event.isShiftPressed(), wordCharacterBinding,
            highlightedCandidate() != null && !koreanSchemeActive());
        if (wordCharacterEdge != WordCharacterPolicy.Edge.NONE
                && selectCandidateEdge(wordCharacterEdge)) return true;
        // Paging only means something while there is a candidate list. With nothing composed these
        // keys are the editor's: Tab moves focus, Page Down scrolls, and a comma is a comma.
        // Korean has no candidate list until its Hanja list opens: until then its punctuation follows the syllable, and Home/End end the syllable through HardwareKeyPolicy below and then move the caret. With the list open these keys page and move the highlight as for any list, except that the marks stay punctuation and Left/Right, which have no caret inside a syllable to move, move the highlight; Escape closes the list and keeps the syllable.
        boolean koreanHanjaList = koreanHanjaListOpen();
        if (koreanHanjaList && keyCode == KeyEvent.KEYCODE_ESCAPE)
            return command(3) || super.onKeyDown(keyCode, event);
        if (hasEngineComposition() && (!koreanSchemeActive() || koreanHanjaList)
                && !(koreanHanjaList && KoreanInputPolicy.hanjaListMark(keyCode))) {
            int navigationCommand = CandidateNavigationPolicy.commandFor(
                keyCode, event.isShiftPressed(), candidateNavigation, japaneseSchemeActive());
            if (navigationCommand == CandidateNavigationPolicy.NONE && koreanHanjaList) {
                navigationCommand = KoreanInputPolicy.hanjaListArrowCommand(keyCode,
                    candidateNavigation != null && candidateNavigation.arrows());
            }
            if (navigationCommand != CandidateNavigationPolicy.NONE) {
                return command(navigationCommand) || super.onKeyDown(keyCode, event);
            }
        }
        int engineCommand = HardwareKeyPolicy.commandFor(keyCode);
        if (engineCommand >= 0) return command(engineCommand) || super.onKeyDown(keyCode, event);
        if (keyCode == KeyEvent.KEYCODE_SPACE && dedicatedEnglish) { space(); return true; }
        if (keyCode == KeyEvent.KEYCODE_SPACE && handwritingActive()) { space(); return true; }
        if (keyCode == KeyEvent.KEYCODE_SPACE) return command(1) || super.onKeyDown(keyCode, event);
        if (keyCode == KeyEvent.KEYCODE_ENTER) { enter(); return true; }
        int unicode = event.getUnicodeChar();
        if (dedicatedEnglish && unicode >= 32 && unicode <= 126 && !isAsciiLetter(unicode)) {
            commitEnglishLiteral(unicode);
            return true;
        }
        if (koreanSchemeActive()) unicode = KoreanInputPolicy.hardwareCharacter(unicode, event.isShiftPressed());
        boolean handled = unicode >= 32 && unicode <= 126
            && character(unicode, event.isShiftPressed());
        return handled || super.onKeyDown(keyCode, event);
    }

    @Override public boolean onKeyUp(int keyCode, KeyEvent event) {
        if (keyCode == modifierTapKeyCode && modifierTapStartedAt >= 0) {
            long elapsed = android.os.SystemClock.uptimeMillis() - modifierTapStartedAt;
            boolean valid = !modifierTapInvalid && elapsed <= 600;
            boolean shift = keyCode == KeyEvent.KEYCODE_SHIFT_LEFT
                || keyCode == KeyEvent.KEYCODE_SHIFT_RIGHT;
            modifierTapStartedAt = -1;
            modifierTapKeyCode = -1;
            modifierTapInvalid = false;
            // A lone right Ctrl tap is the Hanja key while a Korean syllable composes, where many keyboards without one put it, and it takes precedence over the Ctrl language toggle only then, as on Windows. With nothing composing the tap keeps its meaning.
            if (valid && keyCode == KeyEvent.KEYCODE_CTRL_RIGHT && session != 0 && koreanConvertsHanja()) {
                command(KoreanInputPolicy.CONVERT_HANJA_COMMAND);
                return true;
            }
            if (valid && ((shift && hardwareLanguageShift) || (!shift && hardwareLanguageCtrl))) {
                toggleInputLanguage();
                return true;
            }
            return true;
        }
        return super.onKeyUp(keyCode, event);
    }

    @Override public void onUpdateSelection(int oldStart, int oldEnd, int newStart, int newEnd, int composingStart, int composingEnd) {
        super.onUpdateSelection(oldStart, oldEnd, newStart, newEnd, composingStart, composingEnd);
        boolean selectionChanged = oldStart != newStart || oldEnd != newEnd;
        if (selectionChanged) {
            editorContextRevision++;
            // Android reports this boundary after the host has moved the selection. Do not carry
            // Apple service-panel or handwriting state into the new editor context.
            clearHandwriting();
            closeVoiceResult();
            closeAiPolish();
        }
        if (aiPolishContainer != null && aiPolishContainer.getVisibility() == View.VISIBLE
                && aiTarget != null && !aiTargetMatches()) {
            cancelAiRequest();
            aiError = "输入位置已变化，请返回键盘后重新选择文字。";
            renderAiPolish();
        }
        boolean replyVisible = replyKeyboard != null
            && replyKeyboard.getVisibility() == View.VISIBLE;
        if (selectedScheme == KeyboardScheme.THOUGHTFUL_REPLY
                && (replyTarget != null || (selectionChanged && replyVisible))) {
            invalidateReplyContext("输入位置已变化，请重新选择回复方式");
        }
        if (session != 0 && view != null && !view.optString("editing_text").isEmpty()
                && (newStart != composingEnd || newEnd != composingEnd)) {
            // Don't apply an empty composition over the editor's newly moved selection.
            // A Korean syllable is already the final Hangul, marked inline; finishing the region below leaves it in the document, so it counts as typed.
            if (koreanSchemeActive()) recordTypingStatistics(view.optString("reading", ""), typingSource());
            try {
                // With a Korean Hanja list open the first cancel only closes the list (msime_client.h), so it takes a second to drop the syllable the editor now holds as typed text.
                for (int cancels = KoreanInputPolicy.cancelsToDiscard(koreanHanjaListOpen()); cancels > 0; cancels--)
                    value(NativeClient.command(session, 3));
            } catch (JSONException | LinkageError error) { fail(); }
            if (connection != null) bridge.abandon(sink(typingSource()));
            view = null;
            render();
        }
        updateAutomaticCapitalization();
        if (directEnglishActive()) refreshEnglishSuggestions();
    }

    private Button button(LinearLayout row, String label, Runnable action) {
        Button button = new KeyboardPressButton(this);
        button.setAllCaps(false);
        button.setText(label);
        styleButton(button, true);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            countKey(button);
            action.run();
        });
        row.addView(button, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    /** Give a control an explicit face and hand it back, for use inline where it is created. */
    private static Button role(Button button, KeyboardKeyRole face) {
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(face);
        return button;
    }

    private Button shortcutButton(LinearLayout row, String label,
            KeyboardShortcutIconPolicy.Icon icon, Runnable action) {
        KeyboardShortcutButton button = new KeyboardShortcutButton(this, icon);
        button.setAllCaps(false);
        button.setText(label);
        styleButton(button, true);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            countKey(button);
            action.run();
        });
        row.addView(button, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    private Button brandButton(LinearLayout row, Runnable action) {
        KeyboardBrandButton button = new KeyboardBrandButton(this,
            () -> Color.parseColor(skin.accent()));
        button.setAllCaps(false);
        button.setText("更多");
        styleButton(button, true);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            action.run();
        });
        row.addView(button, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    /** The idle top row's scheme pill (the design's 全拼). */
    private Button pillButton(LinearLayout row, String label, Runnable action) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        button.setKeyboardRole(KeyboardKeyRole.PILL);
        button.setAllCaps(false);
        button.setText(label);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        styleButton(button, KeyboardKeyRole.PILL, skin);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            action.run();
        });
        row.addView(button, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    private Button borderlessButton(LinearLayout row, String label, Runnable action) {
        Button button = new KeyboardBorderlessButton(this);
        button.setAllCaps(false);
        button.setText(label);
        styleButton(button, true);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            action.run();
        });
        row.addView(button, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    private Button keyboardKey(String label, String description, Runnable action) {
        Button button = new KeyboardPressButton(this);
        button.setAllCaps(false);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        styleButton(button, false);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            countKey(button);
            action.run();
        });
        return button;
    }

    /**
     * A composing-only control on the candidate paging row.
     *
     * <p>Flat and compact rather than a cap: this row shares the candidate strip's surface, and a
     * line of filled buttons there read as candidates the user could pick.
     */
    private KeyboardPressButton pagingKey(String label, String description, Runnable action) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        button.setAllCaps(false);
        button.setText(label);
        button.setContentDescription(description);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        button.setPadding(pixels(10), 0, pixels(10), 0);
        button.setMinWidth(0);
        button.setMinimumWidth(0);
        button.setKeyboardRole(KeyboardKeyRole.GLYPH);
        styleButton(button, KeyboardKeyRole.GLYPH, skin);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            action.run();
        });
        candidatePaging.addView(button, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, pixels(34)));
        return button;
    }

    /** A nine-key grid cap: the same key as {@link #keyboardKey}, plus room for its digit. */
    private NineKeyDigitButton nineKeyGridKey(String label, String description, Runnable action) {
        NineKeyDigitButton button = new NineKeyDigitButton(this);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        styleButton(button, false);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            countKey(button);
            action.run();
        });
        return button;
    }

    private ShuangpinHintButton shuangpinKeyboardKey(
            String label, String description, Runnable action) {
        ShuangpinHintButton button = new ShuangpinHintButton(this);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        styleButton(button, false);
        button.setOnClickListener(ignored -> {
            playFeedback(button);
            countKey(button);
            action.run();
        });
        return button;
    }

    /** Matches the Apple delete key: a short tap deletes once, a held press repeats. */
    private void bindBackspaceRepeat(Button button, Runnable action) {
        button.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    cancelBackspaceRepeat();
                    backspaceRepeatButton = button;
                    backspaceRepeated = false;
                    backspaceClearedComposition = false;
                    button.setPressed(true);
                    backspaceRepeatTask = new Runnable() {
                        @Override public void run() {
                            if (backspaceRepeatButton != button || !button.isPressed()) return;
                            // A held delete is one press however often it repeats; a short tap counts through performClick instead.
                            if (!backspaceRepeated) countKey(button);
                            backspaceRepeated = true;
                            if (hasEngineComposition()) {
                                playFeedback(button);
                                // A held delete drops the whole syllable, also through an open Hanja list.
                                discardComposition();
                                backspaceClearedComposition = true;
                                main.removeCallbacks(this);
                                backspaceRepeatTask = null;
                                return;
                            }
                            playFeedback(button);
                            action.run();
                            main.postDelayed(this, BACKSPACE_REPEAT_INTERVAL_MILLIS);
                        }
                    };
                    main.postDelayed(backspaceRepeatTask, BACKSPACE_REPEAT_DELAY_MILLIS);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (event.getX() < 0 || event.getY() < 0
                            || event.getX() >= button.getWidth()
                            || event.getY() >= button.getHeight()) {
                        cancelBackspaceRepeat();
                        button.setPressed(false);
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    boolean active = backspaceRepeatButton == button;
                    boolean repeated = active && (backspaceRepeated || backspaceClearedComposition);
                    cancelBackspaceRepeat();
                    button.setPressed(false);
                    if (active && !repeated) button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> {
                    cancelBackspaceRepeat();
                    button.setPressed(false);
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    private void cancelBackspaceRepeat() {
        if (backspaceRepeatTask != null) main.removeCallbacks(backspaceRepeatTask);
        backspaceRepeatTask = null;
        if (backspaceRepeatButton != null) backspaceRepeatButton.setPressed(false);
        backspaceRepeatButton = null;
        backspaceRepeated = false;
        backspaceClearedComposition = false;
    }

    private boolean hasEngineComposition() {
        return view != null && !view.optString("editing_text", "").isEmpty();
    }

    private int pixels(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    private int pixels(double value) {
        if (value <= 0) return 0;
        return Math.max(1, Math.round((float) value * getResources().getDisplayMetrics().density));
    }

    private int halfSpacingPixels(int tenths) {
        return KeyboardGeometry.halfGapPixels(tenths,
            getResources().getDisplayMetrics().density);
    }

    /**
     * Whether the key spacing setting insets this control inside its row.
     *
     * <p>The accessibility description used to be the only marker, because every control it applied
     * to was a key cap. The action row holds caps whose descriptions read as sentences, so the role
     * answers first and the description remains the fallback for everything that never asked.
     */
    private boolean followsKeySpacing(View node) {
        if (node instanceof KeyboardPressButton press) {
            KeyboardKeyRole role = press.keyboardRole();
            return role == null || role.followsKeySpacing();
        }
        CharSequence description = node.getContentDescription();
        return node instanceof Button && description != null
            && description.toString().startsWith("按键 ");
    }

    private void applyKeyboardGeometry(View node) {
        if (followsKeySpacing(node)
                && node.getLayoutParams() instanceof android.view.ViewGroup.MarginLayoutParams) {
            android.view.ViewGroup.MarginLayoutParams params =
                (android.view.ViewGroup.MarginLayoutParams) node.getLayoutParams();
            int horizontal = halfSpacingPixels(touchKeySpacingTenths);
            int vertical = halfSpacingPixels(touchRowSpacingTenths);
            params.setMargins(horizontal, vertical, horizontal, vertical);
            node.setLayoutParams(params);
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applyKeyboardGeometry(group.getChildAt(index));
        }
    }

    private void applyKeyboardGeometry() {
        if (keyRows == null) return;
        applyKeyboardGeometry(keyRows);
        applyKeyboardHeight(keyRows);
        // The action row is a sibling of the key rows rather than one of them -- its height is fixed
        // so the height setting cannot squeeze 换行 -- but its caps take the same spacing.
        if (actionRow != null) {
            applyKeyboardGeometry(actionRow);
            actionRow.requestLayout();
        }
        keyRows.requestLayout();
        if (keyboardRoot != null) {
            keyboardRoot.requestLayout();
            keyboardRoot.getRootView().requestLayout();
        }
    }

    private void applyKeyboardHeight(View node) {
        Object tag = node.getTag();
        if (tag instanceof KeyboardHeightRole) {
            KeyboardHeightRole role = (KeyboardHeightRole) tag;
            int height = pixels(KeyboardGeometry.adjustedRowHeight(role.baseHeight,
                touchKeyboardHeightAdjustment, role.rowCount, role.rowIndex));
            if (role.includesRowSpacing)
                height += halfSpacingPixels(touchRowSpacingTenths) * 2;
            if (node.getLayoutParams() != null) {
                android.view.ViewGroup.LayoutParams params = node.getLayoutParams();
                params.height = height;
                node.setLayoutParams(params);
            }
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applyKeyboardHeight(group.getChildAt(index));
        }
    }

    private void adjustFixedHeight(View view, int baseHeight) {
        view.setTag(new KeyboardHeightRole(baseHeight, 1, 0, false));
    }

    private void styleButton(Button button, boolean action) { styleButton(button, action, skin); }

    private void styleButton(Button button, boolean action, KeyboardSkin target) {
        styleButton(button, action ? KeyboardKeyRole.ACCENT : KeyboardKeyRole.KEY, target);
    }

    /** `target` is the surface's own skin: the emoji and handwriting panels carry their own theme. */
    private void styleButton(Button button, KeyboardKeyRole role, KeyboardSkin target) {
        boolean selected = button.isSelected();
        // A selected control is the one thing that always wears the filled face: that is how the
        // case key and the script toggle show they are on, whatever role they carry otherwise.
        // 确认 and the function-panel tiles draw their own on state, so they keep their role.
        KeyboardKeyRole face = selected && role != KeyboardKeyRole.RETURN
            && role != KeyboardKeyRole.TILE ? KeyboardKeyRole.ACCENT : role;
        if (face == KeyboardKeyRole.PILL) {
            // The pill is a label on the strip rather than a key, so it keeps a plain rounded face even over a designed skin, inset so the 44dp target stays.
            GradientDrawable pill = new GradientDrawable();
            pill.setColor(Color.parseColor(target.keyBackground()));
            pill.setCornerRadius(pixels(14));
            button.setBackground(new InsetDrawable(pill,
                pixels(2), pixels(8), pixels(2), pixels(8)));
            button.setTextColor(Color.parseColor(target.keyForeground()));
            button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            button.setElevation(0);
            return;
        }
        if (!face.drawsCap()) {
            button.setBackground(null);
            button.setTextColor(Color.parseColor(
                face.usesAccentLabel() ? target.accent() : target.keyForeground()));
            button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            button.setElevation(0);
            return;
        }
        boolean action = face == KeyboardKeyRole.ACCENT;
        boolean confirm = face == KeyboardKeyRole.RETURN;
        boolean tile = face == KeyboardKeyRole.TILE;
        String background = confirm ? target.returnBackground()
            : tile ? (selected ? target.accentSoft() : target.keyBackground())
            : selected ? target.accent()
            : action ? target.functionBackground() : target.keyBackground();
        String foreground = confirm ? target.returnForeground()
            : tile ? (selected ? target.accentText() : target.keyForeground())
            : selected ? target.onAccent()
            : action ? target.actionForeground() : target.keyForeground();
        if (target.designed()) {
            button.setBackground(new KeyboardSkinKeyDrawable(target,
                Color.parseColor(background), selected || action || confirm,
                getResources().getDisplayMetrics().density));
        } else {
            GradientDrawable drawable = new GradientDrawable();
            drawable.setColor(Color.parseColor(background));
            drawable.setCornerRadius(pixels(tile ? MoreToolsLayout.TILE_RADIUS_DP : target.cornerRadius()));
            int borderWidth = pixels(target.borderWidth());
            if (borderWidth > 0)
                drawable.setStroke(borderWidth, Color.parseColor(target.borderColor()));
            button.setBackground(drawable);
        }
        button.setTextColor(Color.parseColor(foreground));
        if (button instanceof ShuangpinHintButton hintButton)
            hintButton.setHintColor(Color.parseColor(target.accent()));
        if (button instanceof NineKeyDigitButton digitButton)
            digitButton.setDigitColor(Color.parseColor(target.accent()));
        button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        int shadowAlpha = (int) Math.round(255 * target.shadowOpacity());
        int shadowColor = Color.argb(shadowAlpha, 0, 0, 0);
        button.setOutlineAmbientShadowColor(shadowColor);
        button.setOutlineSpotShadowColor(shadowColor);
        button.setElevation(target.shadowOpacity() > 0
            ? pixels(Math.max(1, target.shadowRadius() + target.shadowOffset())) : 0);
    }

    /** One of the skin's colours at a fraction of its opacity. */
    private static int fade(String color, double opacity) {
        int value = Color.parseColor(color);
        return Color.argb((int) Math.round(255 * KeyboardGeometry.bounded(opacity, 0, 1)),
            Color.red(value), Color.green(value), Color.blue(value));
    }

    /** The outlined badge the keyboard wears while nothing is being composed. */
    private GradientDrawable brandPillDrawable() {
        GradientDrawable pill = new GradientDrawable();
        pill.setColor(Color.TRANSPARENT);
        pill.setCornerRadius(pixels(14));
        pill.setStroke(Math.max(1, pixels(1)), fade(skin.accent(), .45));
        return pill;
    }

    private GradientDrawable candidateDrawable(int color) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(color);
        drawable.setCornerRadius(pixels(6));
        if (Color.alpha(candidateAppearance.border()) > 0)
            drawable.setStroke(Math.max(1, pixels(1)), candidateAppearance.border());
        return drawable;
    }

    private Typeface candidateTypeface() {
        try {
            return Typeface.create(candidateAppearance.preferredFont(), Typeface.NORMAL);
        } catch (RuntimeException ignored) {
            return Typeface.DEFAULT;
        }
    }

    private void styleCandidateButton(Button button) {
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_selected},
            candidateDrawable(candidateAppearance.selected()));
        states.addState(new int[] {android.R.attr.state_pressed},
            candidateDrawable(candidateAppearance.hover()));
        states.addState(new int[] {android.R.attr.state_focused},
            candidateDrawable(candidateAppearance.hover()));
        states.addState(new int[] {android.R.attr.state_hovered},
            candidateDrawable(candidateAppearance.hover()));
        states.addState(new int[0], candidateDrawable(candidateAppearance.surface()));
        button.setBackground(states);
        button.setTextColor(new ColorStateList(
            new int[][] {{android.R.attr.state_selected}, {}},
            new int[] {candidateAppearance.textFor(true), candidateAppearance.text()}));
        // The design marks the highlighted candidate with bold accent text and no fill.
        button.setTypeface(candidateTypeface(), button.isSelected() ? Typeface.BOLD : Typeface.NORMAL);
        button.setElevation(0);
    }

    private void applySkinToView(View node) {
        applySkinToView(node, node == candidateViewport || node == expandedCandidates, skin);
    }

    /** Re-walk one subtree with its own surface skin after the keyboard-wide pass. */
    private void applySkinToView(View node, KeyboardSkin target) {
        applySkinToView(node, false, target);
    }

    private void applySkinToView(View node, boolean candidateContext, KeyboardSkin target) {
        CharSequence description = node.getContentDescription();
        boolean candidate = candidateContext || node == candidateViewport || node == expandedCandidates
            || (description != null && description.toString().startsWith("候选 "));
        if (node instanceof Button) {
            boolean key = description != null && (description.toString().startsWith("按键 ")
                || description.toString().startsWith("候选 ")
                || description.toString().startsWith("输入方案卡片 "));
            KeyboardKeyRole role = node instanceof KeyboardPressButton press
                ? press.keyboardRole() : null;
            if (candidate) styleCandidateButton((Button) node);
            else if (role != null) styleButton((Button) node, role, target);
            else styleButton((Button) node, !key, target);
            if (description != null && "恢复默认".contentEquals(description))
                ((Button) node).setTextColor(Color.RED);
        } else if (node instanceof TextView) {
            TextView text = (TextView) node;
            text.setTextColor(candidate ? candidateAppearance.text() : Color.parseColor(target.keyForeground()));
            text.setTypeface(candidate ? candidateTypeface()
                : target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applySkinToView(group.getChildAt(index), candidate, target);
        }
    }

    private void applySkin() {
        if (keyboardRoot == null) return;
        keyboardRoot.setBackgroundColor(Color.parseColor(skin.background()));
        if (keyboardSurface != null) applySkinBackground(keyboardSurface);
        if (candidateViewport != null)
            candidateViewport.setBackgroundColor(candidateAppearance.surface());
        if (candidatePaging != null)
            candidatePaging.setBackgroundColor(candidateAppearance.surface());
        if (expandedCandidates != null)
            expandedCandidates.setBackgroundColor(candidateAppearance.surface());
        if (clipboardPanel != null)
            applySkinBackground(clipboardPanel);
        if (schemePanel != null)
            applySkinBackground(schemePanel);
        if (skinPanel != null)
            applySkinBackground(skinPanel);
        if (layoutSettingsPanel != null)
            applySkinBackground(layoutSettingsPanel);
        if (moreToolsPanel != null)
            applySkinBackground(moreToolsPanel);
        if (emojiPanel != null)
            applySkinBackground(emojiPanel, emojiSkin);
        if (voiceResultPanel != null)
            applySkinBackground(voiceResultPanel);
        if (aiPolishPanel != null)
            applySkinBackground(aiPolishPanel);
        if (aiPolishContainer != null)
            applySkinBackground(aiPolishContainer);
        if (replyKeyboard != null)
            applySkinBackground(replyKeyboard);
        if (handwritingCanvas != null) handwritingCanvas.applySkin(handwritingSkin);
        applySkinToView(keyboardRoot);
        // The keyboard-wide pass already styled these subtrees; re-walk the two that carry their
        // own light/dark setting so only their faces change.
        if (emojiPanel != null) applySkinToView(emojiPanel, emojiSkin);
        if (handwritingActive() && keyRows != null) applySkinToView(keyRows, handwritingSkin);
        applySidebarRail();
        if (preedit != null) {
            // Idle, this is the brand badge the shared design draws as an outlined pill; composing, it is the reading itself, set in the strip's typeface and its secondary colour above the candidates.
            preedit.setTextColor(brandPillVisible
                ? Color.parseColor(skin.accent()) : candidateAppearance.number());
            preedit.setTypeface(candidateTypeface());
            preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP,
                brandPillVisible ? 12 : candidatePreeditFontSize);
            preedit.setBackground(brandPillVisible ? brandPillDrawable() : null);
            preedit.setPadding(pixels(brandPillVisible ? 12 : 2), pixels(brandPillVisible ? 4 : 0),
                pixels(brandPillVisible ? 12 : 2), pixels(brandPillVisible ? 4 : 0));
        }
        if (candidateBrandMark != null) candidateBrandMark.invalidate();
        if (status != null) status.setTextColor(fade(skin.accent(), .55));
        if (candidatePage != null) {
            candidatePage.setTextColor(candidateAppearance.accent());
            candidatePage.setTypeface(candidateTypeface());
        }
        if (candidatePaging != null) {
            for (int index = 0; index < candidatePaging.getChildCount(); index++) {
                View child = candidatePaging.getChildAt(index);
                if (child instanceof Button) styleButton((Button) child, KeyboardKeyRole.GLYPH, skin);
            }
        }
        if (layoutAdjustView != null) layoutAdjustView.updateSkin(skin);
    }

    private void applySkinBackground(View node) { applySkinBackground(node, skin); }

    private void applySkinBackground(View node, KeyboardSkin target) {
        node.setBackground(new KeyboardSkinBackgroundDrawable(
            target, getResources().getDisplayMetrics().density));
    }

    private boolean systemDark() {
        int mode = getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK;
        return mode == Configuration.UI_MODE_NIGHT_YES;
    }

    private KeyboardSkin keyboardSkin(JSONObject preferences) {
        return surfaceSkin(preferences, "screen_keyboard_theme");
    }

    /** Keep phone keys edge-to-edge while a tablet or two-in-one gets a bounded, centred surface. */
    private void applyKeyboardSurfaceGeometry() {
        if (keyboardSurface == null) return;
        Configuration configuration = getResources().getConfiguration();
        int widthDp = KeyboardFormFactorPolicy.surfaceWidthDp(
            configuration.smallestScreenWidthDp, configuration.screenWidthDp);
        int width = widthDp == 0 ? FrameLayout.LayoutParams.MATCH_PARENT : pixels(widthDp);
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
            width, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        keyboardSurface.setLayoutParams(params);
        keyboardSurface.setElevation(widthDp == 0 ? 0 : pixels(10));
    }

    /**
     * The global theme's keyboard resolved for one panel's own light/dark setting.
     *
     * <p>An explicit `dark` or `light` on the surface wins; `follow` inherits the app mode (`theme`), and a `system` app mode follows the Android night mode. A missing or unknown value is `follow`, so an older snapshot keeps the keyboard's appearance rather than jumping to light. A theme with a fixed appearance then overrides that mode, which is what the shared resolver's `appearance` says.
     */
    private KeyboardSkin surfaceSkin(JSONObject preferences, String key) {
        String surfaceMode = preferences == null ? "follow" : preferences.optString(key, "follow");
        String appMode = preferences == null ? "system"
            : preferences.optString("theme", "system");
        boolean dark = KeyboardSkin.resolveDark(surfaceMode, appMode, systemDark());
        String globalTheme = preferences == null ? "system"
            : preferences.optString("global_theme", "system");
        JSONObject customTheme = preferences == null ? null
            : preferences.optJSONObject("custom_theme");
        return themeSkin(globalTheme, customTheme, dark);
    }

    /**
     * One global theme's keyboard in one host mode, through `msime_client_resolve_theme` (see {@link KeyboardSkin#themeRequest}). Anything the resolver refuses draws the Material 3 keyboard rather than a stale theme.
     */
    private KeyboardSkin themeSkin(String globalTheme, JSONObject customTheme, boolean dark) {
        JSONObject design = customTheme == null ? null : customTheme.optJSONObject("keyboard");
        String request;
        try {
            request = KeyboardSkin.themeRequest(globalTheme, customTheme, dark);
        } catch (JSONException error) {
            return KeyboardSkin.system(dark);
        }
        String cacheKey = request + (design == null ? "" : ":" + design.toString().hashCode());
        KeyboardSkin cached = resolvedThemes.get(cacheKey);
        if (cached != null) return cached;
        KeyboardSkin resolved;
        try {
            resolved = KeyboardSkin.resolved(value(NativeClient.resolveTheme(request)),
                themeTitle(globalTheme), dark, design);
        } catch (JSONException | LinkageError error) {
            return KeyboardSkin.system(dark);
        }
        if (resolvedThemes.size() >= 8) resolvedThemes.clear();
        resolvedThemes.put(cacheKey, resolved);
        return resolved;
    }

    /** The picker entries in the shared order: system, the built-ins, then custom. */
    private JSONArray themeCatalog() {
        if (themeCatalog != null) return themeCatalog;
        try {
            themeCatalog = value(NativeClient.themeCatalog()).getJSONArray("themes");
        } catch (JSONException | LinkageError error) {
            return new JSONArray();
        }
        return themeCatalog;
    }

    private String themeTitle(String id) {
        return KeyboardSkin.themeTitle(themeCatalog(), id);
    }

    private void loadFeedbackPreferences() {
        KeyboardFeedbackStore.Settings settings = KeyboardFeedbackStore.load(this);
        soundEnabled = settings.soundEnabled();
        hapticsEnabled = settings.hapticsEnabled();
        hapticStrength = settings.hapticStrength();
        vibrator = getSystemService(Vibrator.class);
    }

    /** Width for the text this host commits itself, which never passes through the runtime. */
    private String fullWidthOutput(String text) {
        // Korean writes half-width ASCII whatever the width setting says, as the runtime does for its own Korean commits.
        return FullWidthInputPolicy.output(text, fullWidthInput && !koreanSchemeActive());
    }

    /**
     * Hand the width to the runtime and take the answer back from the view it returns.
     *
     * <p>Without a session there is nothing to tell, and the value is simply remembered: the
     * next session start replays it, and the host's own commits already honour it.
     */
    private void applyCharacterWidth(boolean fullwidth) {
        fullWidthInput = fullwidth;
        if (session == 0) return;
        try {
            view = value(NativeClient.setCharacterWidth(session, fullwidth));
            fullWidthInput = CharacterWidthPolicy.viewIsFullWidth(
                view.optString(CharacterWidthPolicy.VIEW_KEY, "Halfwidth"));
        } catch (JSONException | LinkageError error) {
            // The width the host applies to its own commits stands; the runtime keeps the old one.
            showDiagnostic("全角状态未能同步到输入引擎");
        }
    }

    /** Hand the punctuation state to the runtime and take the answer from the view it returns. */
    private void applyChinesePunctuation(boolean enabled) {
        chinesePunctuation = enabled;
        if (session == 0) return;
        try {
            view = value(NativeClient.setChinesePunctuation(session, enabled));
        } catch (JSONException | LinkageError error) {
            showDiagnostic("标点状态未能同步到输入引擎");
        }
    }

    /**
     * 中文标点 for this session only, from the toolbar card or 「Ctrl + .」.
     *
     * <p>The shared setting is the state a session starts in, the same way the width is; changing
     * it there is what makes a new value stick.
     */
    private void toggleChinesePunctuation() {
        applyChinesePunctuation(!chinesePunctuation);
        rebuildKeyRows();
        render();
        renderMoreTools();
    }

    /** Switch width for this session; the shared setting supplies the next session's default. */
    private void toggleFullWidthInput() {
        applyCharacterWidth(!fullWidthInput);
        render();
        renderMoreTools();
    }

    private void saveFeedbackPreferences() {
        try {
            KeyboardFeedbackStore.save(this, new KeyboardFeedbackStore.Settings(
                soundEnabled, hapticsEnabled, hapticStrength));
        } catch (Exception ignored) {
            // Keep the current in-memory feedback state; the next save retries the file.
        }
    }

    private void playFeedback(View source) {
        if (soundEnabled) {
            AudioManager audio = (AudioManager) getSystemService(AUDIO_SERVICE);
            if (audio != null) audio.playSoundEffect(AudioManager.FX_KEYPRESS_STANDARD);
        }
        if (!hapticsEnabled) return;
        if (Build.VERSION.SDK_INT >= 26 && vibrator != null && vibrator.hasVibrator()) {
            vibrator.vibrate(VibrationEffect.createOneShot(10, hapticStrength.amplitude()));
        } else {
            source.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
        }
    }

    private boolean supportsLocalTools() {
        if (view == null) return false;
        int scheme = view.optInt("scheme", 0);
        // Korean has no local modes: Shift+letter is a double consonant there.
        return !dedicatedEnglish && scheme != 2 && scheme != 3
            && scheme != KoreanInputPolicy.KOREAN_SCHEME;
    }

    private void showLocalInputMenu() {
        if (preedit == null || !supportsLocalTools() || view == null
                || !view.optString("editing_text", "").isEmpty()
                || !"none".equals(view.optString("local_mode", "none"))) return;
        PopupMenu popup = new PopupMenu(this, preedit);
        for (LocalInputMode mode : LocalInputMode.values()) {
            MenuItem item = popup.getMenu().add(mode.title());
            item.setEnabled(localModeEnabled(mode));
            item.setOnMenuItemClickListener(ignored -> {
                openLocalInputMode(mode);
                return true;
            });
        }
        popup.show();
    }

    private boolean localModeEnabled(LocalInputMode mode) {
        return localModes.optBoolean(mode.preferenceKey(), true);
    }

    private void openLocalInputMode(LocalInputMode mode) {
        if (session == 0 || !supportsLocalTools() || !localModeEnabled(mode)) return;
        playFeedback(moreButton);
        character(mode.trigger().charAt(0), true);
    }

    private void closeClipboardHistory() {
        if (clipboardScroll != null) clipboardScroll.setVisibility(View.GONE);
        // Cloud entries live only as long as the panel that fetched them, so a later field - possibly a password one - never starts with them in memory.
        cloudClipboardGeneration++;
        cloudClipboardItems = java.util.List.of();
        cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    }

    private void closeSchemePicker() {
        if (schemeScroll != null) schemeScroll.setVisibility(View.GONE);
        synchronizeReplyKeyboard();
    }

    private void closeSkinPicker() {
        if (skinScroll != null) skinScroll.setVisibility(View.GONE);
    }

    private void closeLayoutSettings() {
        if (layoutSettingsScroll != null) layoutSettingsScroll.setVisibility(View.GONE);
        if (layoutAdjustView != null) layoutAdjustView.setVisibility(View.GONE);
    }

    private void closeMoreTools() {
        localInputToolsOpen = false;
        if (moreToolsScroll != null) moreToolsScroll.setVisibility(View.GONE);
    }

    private void closeEmojiPicker() {
        emojiLoadGeneration++;
        emojiLoading = false;
        emojiSelectedCategory = Integer.MIN_VALUE;
        emojiItems = java.util.List.of();
        if (emojiPanel != null) emojiPanel.setVisibility(View.GONE);
        synchronizeReplyKeyboard();
    }

    private void closeSymbolPanel() {
        if (symbolPanel != null) symbolPanel.setVisibility(View.GONE);
        synchronizeReplyKeyboard();
    }

    private java.util.List<String> loadEmojiRecents() {
        if (emojiPreferences == null) return java.util.List.of();
        String document = emojiPreferences.getString(EMOJI_RECENTS_KEY, "[]");
        if (document == null || document.length() > 16_384) return java.util.List.of();
        try {
            JSONArray values = new JSONArray(document);
            java.util.ArrayList<String> stored = new java.util.ArrayList<>();
            int count = Math.min(values.length(), EmojiCatalogModel.RECENTS_LIMIT * 2);
            for (int index = 0; index < count; index++) {
                Object value = values.opt(index);
                if (value instanceof String) stored.add((String) value);
            }
            return EmojiCatalogModel.normalizeRecents(stored);
        } catch (JSONException error) {
            return java.util.List.of();
        }
    }

    private void saveEmojiRecents() {
        if (emojiPreferences == null) return;
        emojiPreferences.edit().putString(
            EMOJI_RECENTS_KEY, new JSONArray(emojiRecents).toString()).apply();
    }

    private boolean emojiPickerVisible() {
        return emojiPanel != null && emojiPanel.getVisibility() == View.VISIBLE;
    }

    private void selectEmojiCategory(int category) {
        if (!emojiPickerVisible()) return;
        if (category < -1 || category >= EmojiCatalogModel.categories().size()) return;
        emojiLoadGeneration++;
        emojiSelectedCategory = category;
        emojiNextOffset = 0;
        emojiComplete = category == -1;
        emojiLoading = false;
        emojiItems = java.util.List.of();
        renderEmojiTabs();
        if (category == -1) {
            java.util.ArrayList<EmojiCatalogModel.Item> recent = new java.util.ArrayList<>();
            for (String text : emojiRecents)
                recent.add(new EmojiCatalogModel.Item(text, "", "最近"));
            emojiItems = java.util.List.copyOf(recent);
            renderEmojiGrid();
        } else {
            renderEmojiGrid();
            loadEmojiPage();
        }
    }

    private EmojiCatalogModel.Page decodeEmojiPage(
            String response, int offset, EmojiCatalogModel.Category category) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!envelope.getBoolean("ok")) throw new JSONException("Emoji catalog unavailable");
        JSONObject value = envelope.getJSONObject("value");
        JSONArray entries = value.getJSONArray("items");
        if (entries.length() > EmojiCatalogModel.PAGE_SIZE)
            throw new JSONException("Emoji catalog page too large");
        java.util.ArrayList<EmojiCatalogModel.Item> items = new java.util.ArrayList<>();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.getJSONObject(index);
            EmojiCatalogModel.Item item;
            try {
                item = new EmojiCatalogModel.Item(entry.getString("text"),
                    entry.getString("annotation"), entry.getString("group"));
            } catch (IllegalArgumentException error) {
                throw new JSONException("Invalid emoji catalog item");
            }
            if (!category.group().equals(item.group()))
                throw new JSONException("Unexpected emoji catalog group");
            items.add(item);
        }
        try {
            return EmojiCatalogModel.validatePage(items, offset, EmojiCatalogModel.PAGE_SIZE,
                value.getLong("next_offset"), value.getBoolean("complete"));
        } catch (IllegalArgumentException error) {
            throw new JSONException("Invalid emoji catalog cursor");
        }
    }

    private void loadEmojiPage() {
        if (!emojiPickerVisible() || emojiSelectedCategory < 0 || emojiLoading || emojiComplete
                || emojiResources.isEmpty()) return;
        int categoryIndex = emojiSelectedCategory;
        EmojiCatalogModel.Category category = EmojiCatalogModel.categories().get(categoryIndex);
        int offset = emojiNextOffset;
        long generation = ++emojiLoadGeneration;
        String resources = emojiResources;
        String query;
        try {
            query = new JSONObject().put("category", "").put("group", category.group())
                .put("offset", offset).put("limit", EmojiCatalogModel.PAGE_SIZE)
                .put("cursor", true).toString();
        } catch (JSONException error) {
            return;
        }
        emojiLoading = true;
        renderEmojiStatus();
        emojiWorker.execute(() -> {
            EmojiCatalogModel.Page page = null;
            try { page = decodeEmojiPage(NativeClient.emojiCatalog(query, resources), offset, category); }
            catch (JSONException | RuntimeException | LinkageError ignored) {
                // The UI reports a sanitized catalog error; never expose resource paths or rows.
            }
            EmojiCatalogModel.Page result = page;
            main.post(() -> {
                if (!emojiPickerVisible() || generation != emojiLoadGeneration
                        || categoryIndex != emojiSelectedCategory) return;
                emojiLoading = false;
                if (result == null) {
                    emojiComplete = true;
                    if (emojiStatus != null) emojiStatus.setText("表情目录暂时不可用；点分类重试");
                    return;
                }
                java.util.ArrayList<EmojiCatalogModel.Item> combined =
                    new java.util.ArrayList<>(emojiItems);
                combined.addAll(result.items());
                emojiItems = java.util.List.copyOf(combined);
                emojiNextOffset = result.nextOffset();
                emojiComplete = result.complete();
                renderEmojiGrid();
                if (!emojiComplete && result.items().isEmpty()) {
                    loadEmojiPage();
                } else if (!emojiComplete && emojiGridScroll != null) {
                    emojiGridScroll.post(() -> {
                        if (emojiPickerVisible() && generation == emojiLoadGeneration
                                && categoryIndex == emojiSelectedCategory
                                && !emojiGridScroll.canScrollVertically(1)) loadEmojiPage();
                    });
                }
            });
        });
    }

    private void renderEmojiStatus() {
        if (emojiStatus == null) return;
        if (emojiLoading) emojiStatus.setText("正在加载表情…");
        else if (emojiItems.isEmpty()) emojiStatus.setText("暂无表情");
        else if (emojiComplete) emojiStatus.setText(emojiItems.size() + " 个表情");
        else emojiStatus.setText(emojiItems.size() + " 个表情 · 继续滚动加载");
    }

    private void renderEmojiTabs() {
        if (emojiTabs == null) return;
        emojiTabs.removeAllViews();
        if (!emojiRecents.isEmpty()) addEmojiTab("最近", -1);
        for (int index = 0; index < EmojiCatalogModel.categories().size(); index++)
            addEmojiTab(EmojiCatalogModel.categories().get(index).title(), index);
    }

    private void addEmojiTab(String title, int category) {
        Button tab = new Button(this);
        tab.setAllCaps(false);
        tab.setText(title);
        tab.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        tab.setSelected(emojiSelectedCategory == category);
        tab.setContentDescription("表情分类 " + title);
        if (Build.VERSION.SDK_INT >= 30)
            tab.setStateDescription(tab.isSelected() ? "已选中" : "未选中");
        styleButton(tab, true);
        tab.setOnClickListener(ignored -> {
            playFeedback(tab);
            selectEmojiCategory(category);
        });
        emojiTabs.addView(tab, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, pixels(38)));
    }

    private void renderEmojiGrid() {
        if (emojiGrid == null) return;
        emojiGrid.removeAllViews();
        for (EmojiCatalogModel.Item item : emojiItems) {
            Button cell = keyboardKey(item.text(), "表情 " + item.text(),
                () -> insertEmoji(item.text()));
            cell.setTextSize(TypedValue.COMPLEX_UNIT_SP, 24);
            cell.setPadding(0, 0, 0, 0);
            GridLayout.LayoutParams params = new GridLayout.LayoutParams(
                GridLayout.spec(GridLayout.UNDEFINED), GridLayout.spec(GridLayout.UNDEFINED, 1f));
            params.width = 0;
            params.height = pixels(48);
            emojiGrid.addView(cell, params);
        }
        renderEmojiStatus();
        applySkin();
    }

    private void insertEmoji(String text) {
        if (!emojiPickerVisible() || connection == null) return;
        if (!commitText(text, TypingSource.LOCAL)) return;
        emojiRecents = EmojiCatalogModel.recordRecent(emojiRecents, text);
        saveEmojiRecents();
    }

    private void deleteFromEmojiPicker() {
        if (connection != null && !command(0))
            connection.deleteSurroundingTextInCodePoints(1, 0);
    }

    private void showEmojiPicker() {
        if (session == 0 || connection == null || emojiPanel == null || emojiResources.isEmpty()) {
            Toast.makeText(this, "表情目录尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        command(2);
        if (session == 0 || connection == null) return;
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeMoreTools();
        closeSymbolPanel();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        emojiRecents = loadEmojiRecents();
        emojiPanel.setVisibility(View.VISIBLE);
        emojiPanel.requestFocus();
        selectEmojiCategory(emojiRecents.isEmpty() ? 0 : -1);
    }

    private void showSymbolPanel() {
        if (session == 0 || connection == null || symbolPanel == null) {
            Toast.makeText(this, "符号面板尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        command(2);
        if (session == 0 || connection == null) return;
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeMoreTools();
        closeEmojiPicker();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        symbolPanel.resetForPresentation();
        symbolPanel.setVisibility(View.VISIBLE);
        symbolPanel.requestFocus();
    }

    private void closeVoiceResult() {
        if (voiceResultScroll != null) voiceResultScroll.setVisibility(View.GONE);
        voiceResultEntry = null;
        voiceTarget = null;
    }

    private void cancelAiRequest() {
        if (aiOperation != null) aiOperation.cancel();
        aiOperation = null;
        aiBusy = false;
    }

    private void closeAiPolish() {
        cancelAiRequest();
        if (aiPolishContainer != null) aiPolishContainer.setVisibility(View.GONE);
        aiTarget = null;
        aiRequestConfiguration = null;
        aiSourceText = "";
        aiOutputText = "";
        aiError = "";
    }

    private void clearReplyRequestReferences() {
        replyOperation = null;
        replyTarget = null;
        replyRequestConfiguration = null;
    }

    private void closeReplyKeyboard() {
        replyModel.resetResults();
        clearReplyRequestReferences();
        setReplyKeyboardVisible(false);
    }

    /** Keep the shared candidate/shortcut bar visible while the reply surface owns the key area. */
    private void setReplyKeyboardVisible(boolean visible) {
        if (replyKeyboard != null)
            replyKeyboard.setVisibility(visible ? View.VISIBLE : View.GONE);
        if (keyRows != null)
            keyRows.setVisibility(visible ? View.GONE : View.VISIBLE);
        if (actionRow != null)
            actionRow.setVisibility(visible ? View.GONE : View.VISIBLE);
    }

    private void invalidateReplyContext(String message) {
        replyModel.invalidate(message);
        clearReplyRequestReferences();
        renderReplyKeyboard();
    }

    private boolean replyTargetMatches() {
        return replyTarget != null && replyTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    private boolean replyReady() {
        return selectedScheme == KeyboardScheme.THOUGHTFUL_REPLY && thoughtfulReplyEnabled()
            && aiPolishReady();
    }

    private void synchronizeReplyKeyboard() {
        if (replyKeyboard == null) return;
        if (selectedScheme != KeyboardScheme.THOUGHTFUL_REPLY || !thoughtfulReplyEnabled()) {
            closeReplyKeyboard();
            replySuppressed = false;
            return;
        }
        if (replySuppressed) {
            setReplyKeyboardVisible(false);
            return;
        }
        renderReplyKeyboard();
        setReplyKeyboardVisible(true);
    }

    private void showReplyKeyboard() {
        if (selectedScheme != KeyboardScheme.THOUGHTFUL_REPLY) return;
        replySuppressed = false;
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeVoiceResult();
        closeAiPolish();
        synchronizeReplyKeyboard();
    }

    private void pasteReplySource() {
        try {
            ClipboardManager manager = getSystemService(ClipboardManager.class);
            if (manager == null || !manager.hasPrimaryClip() || manager.getPrimaryClip() == null
                    || manager.getPrimaryClip().getItemCount() == 0
                    || manager.getPrimaryClipDescription() == null
                    || !(manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                        || manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML))) {
                replyModel.setSource("");
            } else {
                CharSequence value = manager.getPrimaryClip().getItemAt(0).getText();
                replyModel.setSource(value == null ? "" : value.toString());
            }
        } catch (SecurityException | IllegalStateException error) {
            replyModel.invalidate("无法读取剪贴板，请重试");
        }
        clearReplyRequestReferences();
        renderReplyKeyboard();
    }

    private void generateReply(String style) {
        if (aiPolishConfiguration == null) {
            replyModel.showStatus("请先在共享设置中启用并配置 AI 辅助");
            renderReplyKeyboard();
            return;
        }
        if (!replyReady()) {
            replyModel.showStatus("请先完成输入，再选择回复方式");
            renderReplyKeyboard();
            return;
        }
        java.util.List<CommunityReplyLibrary.Template> templates = java.util.List.of();
        if (style != null && style.startsWith("community:")) {
            try { templates = communityReplyLibrary == null ? java.util.List.of() : communityReplyLibrary.read(); }
            catch (java.io.IOException error) {
                replyModel.showStatus("回复模板无法读取，请重试");
                renderReplyKeyboard();
                return;
            }
        }
        ReplyKeyboardModel.Request request = replyModel.begin(style, templates);
        if (request == null) {
            renderReplyKeyboard();
            return;
        }
        replyRequestConfiguration = aiPolishConfiguration;
        replyTarget = new EditorContextSnapshot(connection, editorContextRevision, editorContext(true),
            selectedEditorText(), editorContext(false));
        renderReplyKeyboard();
        try {
            AiPolishConfiguration requestConfiguration = aiPolishConfiguration.withPrompt(request.prompt());
            replyOperation = aiPolishClient.request(requestConfiguration, request.source(),
                (generation, result, failure) -> main.post(
                    () -> finishReply(request.generation(), generation, result, failure)));
            replyModel.attachCancellation(request.generation(), replyOperation::cancel);
        } catch (AiPolishClient.Failure | IllegalArgumentException error) {
            replyModel.fail(request.generation(), "无法启动 AI 请求，请检查配置");
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }
    }

    private void finishReply(long modelGeneration, long operationGeneration, String result,
                             AiPolishClient.Failure failure) {
        if (replyOperation == null || replyOperation.generation() != operationGeneration) return;
        replyOperation = null;
        if (!replyTargetMatches() || replyRequestConfiguration == null
                || !replyRequestConfiguration.equals(aiPolishConfiguration)
                || selectedScheme != KeyboardScheme.THOUGHTFUL_REPLY) {
            invalidateReplyContext("输入位置或 AI 配置已变化，请重新选择回复方式");
            return;
        }
        if (failure == null) replyModel.finish(modelGeneration, result);
        else replyModel.fail(modelGeneration, failure.reason() == AiPolishClient.Reason.INVALID
            ? "服务返回的文字为空或超过一万字" : "AI 请求失败，请检查网络、地址、模型和密钥");
        if (!replyModel.busy()) replyOperation = null;
        renderReplyKeyboard();
    }

    private void useReply(String text) {
        boolean inserted = replyModel.use(text, value -> {
            if (!replyReady() || !replyTargetMatches() || replyRequestConfiguration == null
                    || !replyRequestConfiguration.equals(aiPolishConfiguration)
                    || connection == null) return false;
            try {
                if (!commitText(value, TypingSource.REPLY)) return false;
            } catch (RuntimeException error) { return false; }
            replySuppressed = true;
            return true;
        });
        clearReplyRequestReferences();
        renderReplyKeyboard();
        if (inserted) setReplyKeyboardVisible(false);
    }

    private void showReplyTemplates() {
        if (replyTemplateButton == null || replyModel.busy()) return;
        final java.util.List<CommunityReplyLibrary.Template> templates;
        try { templates = communityReplyLibrary == null ? java.util.List.of() : communityReplyLibrary.read(); }
        catch (java.io.IOException error) {
            replyModel.showStatus("回复模板无法读取，请重试");
            renderReplyKeyboard();
            return;
        }
        if (templates.isEmpty()) {
            Toast.makeText(this, "请先在 App 社区收藏并添加回复模板", Toast.LENGTH_SHORT).show();
            return;
        }
        PopupMenu popup = new PopupMenu(this, replyTemplateButton);
        for (CommunityReplyLibrary.Template template : templates) {
            popup.getMenu().add(template.name()).setOnMenuItemClickListener(ignored -> {
                generateReply("community:" + template.id());
                return true;
            });
        }
        popup.show();
    }

    private boolean canSaveKeyboardSkin() {
        return !skinSaving && !traditionalOutputSaving
            && session != 0 && preferencesSnapshot != null
            && !preferencesDirectory.isEmpty();
    }

    private void showSkinMenu(Button anchor) {
        if (anchor == null || !canSaveKeyboardSkin() || skinScroll == null) return;
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeMoreTools();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        renderSkinPicker();
        skinScroll.setVisibility(View.VISIBLE);
    }

    private void renderSkinPicker() {
        if (skinPanel == null) return;
        skinPanel.removeAllViews();
        LinearLayout header = new LinearLayout(this);
        TextView title = new TextView(this);
        title.setText("选择皮肤");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button close = button(header, "返回键盘", this::closeSkinPicker);
        close.setContentDescription("返回键盘");
        skinPanel.addView(header);

        java.util.List<SkinChoice> saved = new java.util.ArrayList<>();
        JSONObject preferences = preferencesSnapshot == null ? null
            : preferencesSnapshot.optJSONObject("preferences");
        boolean hostDark = KeyboardSkin.resolveDark(
            preferences == null ? "follow" : preferences.optString("screen_keyboard_theme", "follow"),
            preferences == null ? "system" : preferences.optString("theme", "system"), systemDark());
        try {
            for (CustomSkinLibrary.Item item : CustomSkinLibrary.read(java.nio.file.Paths.get(preferencesDirectory))) {
                JSONObject design = item.design();
                saved.add(new SkinChoice("custom", item.name(), KeyboardSkin.custom(design, hostDark), design));
            }
        } catch (Exception ignored) {
            // A partially written library must not hide the themes.
        }
        if (!saved.isEmpty()) addSkinSection(skinPanel, "我的设计", saved);

        // The global themes in the shared catalog's order. A built-in card draws its catalog palette in the theme's own fixed mode; 跟随系统 draws the Material 3 tokens in this keyboard's mode; the custom card draws the custom theme as it stands, which is 我的皮肤 once a keyboard design exists.
        java.util.List<SkinChoice> builtIns = new java.util.ArrayList<>();
        JSONObject customTheme = preferences == null ? null : preferences.optJSONObject("custom_theme");
        JSONArray themes = themeCatalog();
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = entry.optString("id", "");
            if (id.isEmpty()) continue;
            String themeName = entry.optString("title", id);
            KeyboardSkin choice = "custom".equals(id) ? themeSkin(id, customTheme, hostDark)
                : KeyboardSkin.resolved(entry, themeName, hostDark, null);
            builtIns.add(new SkinChoice(id, choice.title(), choice, null));
        }
        if (builtIns.isEmpty()) {
            KeyboardSkin system = KeyboardSkin.system(hostDark);
            builtIns.add(new SkinChoice(system.id(), system.title(), system, null));
        }
        addSkinSection(skinPanel, null, builtIns);
        applySkin();
    }

    private void addSkinSection(LinearLayout parent, String heading, java.util.List<SkinChoice> choices) {
        JSONObject stored = preferencesSnapshot == null ? null
            : preferencesSnapshot.optJSONObject("preferences");
        String globalTheme = stored == null ? "system" : stored.optString("global_theme", "system");
        if (heading != null) {
            TextView label = new TextView(this);
            label.setText(heading);
            label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
            parent.addView(label, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int start = 0; start < choices.size(); start += 2) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.HORIZONTAL);
            // 卡片是小布局，没有文字基线可对齐。
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 2; slot++) {
                int index = start + slot;
                if (index >= choices.size()) {
                    row.addView(new View(this), new LinearLayout.LayoutParams(0, pixels(124), 1));
                    continue;
                }
                SkinChoice choice = choices.get(index);
                KeyboardSkinCard card = new KeyboardSkinCard(this, choice.skin(), choice.title());
                // A theme card is selected by the stored global theme; a saved design only while the custom theme draws exactly that design.
                card.setSelected(choice.design() == null ? choice.id().equals(globalTheme)
                    : "custom".equals(globalTheme) && skin.key().equals(choice.skin().key()));
                card.setContentDescription("屏幕键盘皮肤 " + choice.title());
                if (Build.VERSION.SDK_INT >= 30)
                    card.setStateDescription(card.isSelected() ? "已选中" : "未选中");
                card.setOnClickListener(ignored -> {
                    playFeedback(card);
                    closeSkinPicker();
                    saveKeyboardSkin(choice.id(), choice.design());
                });
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, pixels(124), 1);
                params.setMargins(pixels(4), pixels(4), pixels(4), pixels(4));
                row.addView(card, params);
            }
            parent.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, pixels(132)));
        }
    }

    private void showKeyboardSkinStatus(String value) {
        if (replyKeyboard != null && replyKeyboard.getVisibility() == View.VISIBLE) {
            replyModel.showStatus(value);
            renderReplyKeyboard();
        } else {
            Toast.makeText(this, value, Toast.LENGTH_SHORT).show();
        }
    }

    /**
     * Select one global theme from the keyboard's picker, or, with a `design`, store it as `custom_theme.keyboard` and select `custom`.
     *
     * <p>This copies the settings page: when another theme was on screen it becomes `custom_theme.base` and `custom_theme.candidate_skin` is cleared, so the candidate strip keeps the theme the user was looking at; while `custom` is already selected only the keyboard changes.
     */
    private void saveKeyboardSkin(String identifier, JSONObject design) {
        if (skinSaving || traditionalOutputSaving || session == 0
                || preferencesSnapshot == null || preferencesDirectory.isEmpty()) return;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        final JSONObject preferences;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = pending.getLong("revision");
            preferences = pending.getJSONObject("preferences");
            String current = preferences.optString("global_theme", "system");
            if (design == null) {
                if (identifier.equals(current)) return;
                preferences.put("global_theme", identifier);
            } else {
                JSONObject customTheme = preferences.optJSONObject("custom_theme");
                if (customTheme == null) customTheme = new JSONObject();
                JSONObject stored = customTheme.optJSONObject("keyboard");
                if ("custom".equals(current) && stored != null
                        && stored.toString().equals(design.toString())) return;
                if (!"custom".equals(current)) {
                    customTheme.put("base", current);
                    customTheme.remove("candidate_skin");
                }
                customTheme.put("keyboard", new JSONObject(design.toString()));
                preferences.put("custom_theme", customTheme);
                preferences.put("global_theme", "custom");
            }
        } catch (JSONException error) {
            showKeyboardSkinStatus("皮肤切换失败，保留当前皮肤");
            return;
        }
        skin = keyboardSkin(preferences);
        emojiSkin = surfaceSkin(preferences, "emoji_theme");
        handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
        skinSaving = true;
        final long operation = ++preferenceSaveGeneration;
        applySkin();
        render();
        try {
            preferencesWorker.execute(() -> {
                String response;
                try { response = NativeClient.savePreferences(targetDirectory, expectedRevision, pending.toString()); }
                catch (Exception | LinkageError error) { response = null; }
                final String savedResponse = response;
                main.post(() -> finishKeyboardSkinSave(operation, targetSession, targetDirectory, savedResponse));
            });
        } catch (RuntimeException error) {
            finishKeyboardSkinSave(operation, targetSession, targetDirectory, null);
        }
    }

    private void finishKeyboardSkinSave(long operation, long targetSession, String targetDirectory,
                                        String response) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        skinSaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            applyPreferencesSnapshot(value(response));
            showKeyboardSkinStatus("皮肤已切换");
        } catch (JSONException | LinkageError error) {
            JSONObject accepted = preferencesSnapshot == null ? null
                : preferencesSnapshot.optJSONObject("preferences");
            skin = keyboardSkin(accepted);
            emojiSkin = surfaceSkin(accepted, "emoji_theme");
            handwritingSkin = surfaceSkin(accepted, "handwriting_theme");
            showKeyboardSkinStatus("皮肤切换失败，已恢复原皮肤");
        }
        applySkin();
        render();
    }

    /** Finish the Engine composition before handing the input connection to another IME. */
    private void switchToNextInputMethodAfterCommit() {
        if (session != 0) command(2);
        switchToNextInputMethod(false);
    }

    private boolean canSaveChineseOutput() {
        return !traditionalOutputSaving && !schemeSaving && !touchGeometrySaving && !skinSaving
            && session != 0 && preferencesSnapshot != null && !preferencesDirectory.isEmpty();
    }

    private void toggleChineseOutput() {
        if (!canSaveChineseOutput()) {
            Toast.makeText(this, "简繁设置尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        final boolean targetTraditional = !traditionalChineseOutput;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = pending.getLong("revision");
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            pending.getJSONObject("preferences")
                .put("traditional_chinese_output", targetTraditional);
        } catch (JSONException error) {
            preferencesNotice = " · 简繁设置保存失败，保留原设置";
            render();
            return;
        }
        traditionalChineseOutput = targetTraditional;
        traditionalOutputSaving = true;
        preferencesNotice = " · 正在保存简繁设置";
        final long operation = ++preferenceSaveGeneration;
        render();
        try {
            preferencesWorker.execute(() -> {
                String response;
                try {
                    response = NativeClient.savePreferences(
                        targetDirectory, expectedRevision, pending.toString());
                } catch (Exception | LinkageError error) {
                    response = null;
                }
                final String savedResponse = response;
                main.post(() -> finishChineseOutputSave(
                    operation, targetSession, targetDirectory, savedResponse));
            });
        } catch (RuntimeException error) {
            finishChineseOutputSave(operation, targetSession, targetDirectory, null);
        }
    }

    private void finishChineseOutputSave(long operation, long targetSession,
                                         String targetDirectory, String response) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        traditionalOutputSaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long savedRevision = saved.getLong("revision");
            if (preferencesSnapshot != null
                    && preferencesSnapshot.optLong("revision", -1) > savedRevision) {
                applyChineseOutputPreference(preferencesSnapshot.optJSONObject("preferences"));
                preferencesNotice = "";
            } else {
                applyPreferencesSnapshot(saved);
                preferencesNotice = traditionalChineseOutput
                    ? " · 已切换为繁体输出" : " · 已切换为简体输出";
            }
        } catch (JSONException | LinkageError error) {
            JSONObject accepted = preferencesSnapshot == null ? null
                : preferencesSnapshot.optJSONObject("preferences");
            applyChineseOutputPreference(accepted);
            preferencesNotice = " · 简繁设置保存失败，已恢复原设置";
            Toast.makeText(this, "简繁设置未能保存", Toast.LENGTH_SHORT).show();
        }
        render();
    }

    private LinearLayout createReplyKeyboard() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(pixels(6), pixels(5), pixels(6), pixels(5));
        root.setBackgroundColor(Color.parseColor(skin.background()));
        root.setContentDescription("高情商回复键盘");

        LinearLayout header = new LinearLayout(this);
        replyReplyModeButton = role(button(header, "帮你回", () -> {
            replyModel.setMode(ReplyKeyboardModel.Mode.REPLY);
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.KEY);
        replyReplyModeButton.setContentDescription("帮你回模式");
        replyPolishModeButton = role(button(header, "帮润色", () -> {
            replyModel.setMode(ReplyKeyboardModel.Mode.POLISH);
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.KEY);
        replyPolishModeButton.setContentDescription("帮润色模式");
        replyTemplateButton = role(button(header, "模板", this::showReplyTemplates),
            KeyboardKeyRole.KEY);
        replyTemplateButton.setContentDescription("回复模板");
        View spacer = new View(this);
        header.addView(spacer, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 1));
        root.addView(header, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(42)));

        LinearLayout sourceRow = new LinearLayout(this);
        replySourceButton = role(button(sourceRow, "+ 粘贴 TA 的话帮你回", this::pasteReplySource),
            KeyboardKeyRole.KEY);
        replySourceButton.setSingleLine(true);
        replySourceButton.setEllipsize(android.text.TextUtils.TruncateAt.END);
        replySourceButton.setContentDescription("回复源文字");
        Button paste = role(button(sourceRow, "粘贴", this::pasteReplySource),
            KeyboardKeyRole.KEY);
        paste.setContentDescription("粘贴回复源文字");
        paste.setLayoutParams(new LinearLayout.LayoutParams(pixels(64),
            LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(sourceRow, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(44)));

        LinearLayout body = new LinearLayout(this);
        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        replyMain = new LinearLayout(this);
        replyMain.setOrientation(LinearLayout.VERTICAL);
        replyMain.setContentDescription("回复风格与候选");
        scroll.addView(replyMain);
        body.addView(scroll, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        replyActions = new LinearLayout(this);
        replyActions.setOrientation(LinearLayout.VERTICAL);
        body.addView(replyActions, new LinearLayout.LayoutParams(pixels(68),
            LinearLayout.LayoutParams.MATCH_PARENT));
        root.addView(body, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));

        LinearLayout footer = new LinearLayout(this);
        replyStatus = new TextView(this);
        replyStatus.setSingleLine(true);
        replyStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11);
        replyStatus.setContentDescription("回复键盘状态");
        footer.addView(replyStatus, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button styles = role(button(footer, "选风格", () -> {
            replyModel.chooseStyle();
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.KEY);
        styles.setContentDescription("重新选择回复风格");
        root.addView(footer, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(36)));
        return root;
    }

    private void renderReplyKeyboard() {
        if (replyKeyboard == null || replyMain == null || replyActions == null) return;
        replyMain.removeAllViews();
        replyActions.removeAllViews();
        replyReplyModeButton.setSelected(replyModel.mode() == ReplyKeyboardModel.Mode.REPLY);
        replyPolishModeButton.setSelected(replyModel.mode() == ReplyKeyboardModel.Mode.POLISH);
        // 键帽，不是两个常亮的实心块：选中哪个由 isSelected 决定，那才是这一对要表达的东西。
        styleButton(replyReplyModeButton, KeyboardKeyRole.KEY, skin);
        styleButton(replyPolishModeButton, KeyboardKeyRole.KEY, skin);
        replyTemplateButton.setEnabled(!replyModel.busy());
        replySourceButton.setText(replyModel.source().isEmpty()
            ? "+ 粘贴 TA 的话帮你回" : replyModel.source());
        if (replyModel.replies().isEmpty()) {
            for (int start = 0; start < ReplyKeyboardModel.STYLES.size(); start += 3) {
                LinearLayout row = new LinearLayout(this);
                for (int column = 0; column < 3; column++) {
                    int index = start + column;
                    ReplyKeyboardModel.Style style = ReplyKeyboardModel.STYLES.get(index);
                    // 十二个风格是一组可选项，不是十二个强调动作。
                    Button choice = role(button(row, style.emoji() + " " + style.label(),
                        () -> generateReply(style.label())), KeyboardKeyRole.KEY);
                    choice.setContentDescription("回复风格 " + style.label());
                    choice.setEnabled(!replyModel.busy());
                }
                replyMain.addView(row, new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, pixels(52)));
            }
        } else {
            for (String reply : replyModel.replies()) {
                Button candidate = role(button(replyMain, reply, () -> useReply(reply)),
                    KeyboardKeyRole.KEY);
                candidate.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
                candidate.setContentDescription("回复候选，点按插入");
                candidate.setMinHeight(pixels(48));
                candidate.setLayoutParams(new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            }
        }
        Button delete = role(button(replyActions, "⌫", () -> {
            replyModel.deleteLastCodePoint();
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.KEY);
        delete.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        delete.setContentDescription("删除源文字");
        Button clear = role(button(replyActions, "清空", () -> {
            replyModel.setSource("");
            clearReplyRequestReferences();
            renderReplyKeyboard();
        }), KeyboardKeyRole.KEY);
        clear.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        clear.setContentDescription("清空源文字");
        if (replyModel.busy()) {
            Button cancel = role(button(replyActions, "取消", () -> {
                replyModel.cancel();
                clearReplyRequestReferences();
                renderReplyKeyboard();
            }), KeyboardKeyRole.KEY);
            cancel.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
            cancel.setContentDescription("取消回复生成");
        } else if (replyModel.replies().isEmpty()) {
            Button generate = button(replyActions, "生成", () -> generateReply(replyModel.style()));
            generate.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
            generate.setContentDescription("生成回复");
        } else {
            Button regenerate = button(replyActions, "换一句", () -> generateReply(replyModel.style()));
            regenerate.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
            regenerate.setContentDescription("换一句回复");
        }
        replyStatus.setText(replyModel.status());
        applySkinToView(replyKeyboard);
    }

    private boolean voiceInsertionReady() {
        return session != 0 && connection != null && view != null
            && view.optString("editing_text", "").isEmpty()
            && view.optString("local_mode", "none").equals("none");
    }

    private boolean aiPolishReady() { return voiceInsertionReady(); }

    private String editorContext(boolean before) {
        if (connection == null) return null;
        CharSequence text = before ? connection.getTextBeforeCursor(64, 0)
            : connection.getTextAfterCursor(64, 0);
        return text == null ? null : text.toString();
    }

    private String selectedEditorText() {
        if (connection == null) return null;
        CharSequence text = connection.getSelectedText(0);
        return text == null ? null : text.toString();
    }

    private void captureVoiceTarget() {
        voiceTarget = new EditorContextSnapshot(connection, editorContextRevision, editorContext(true),
            selectedEditorText(), editorContext(false));
    }

    private boolean voiceTargetMatches() {
        return voiceTarget != null && voiceTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    private boolean aiTargetMatches() {
        return aiTarget != null && aiTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    private void showAiPolish() {
        if (aiPolishConfiguration == null) {
            Toast.makeText(this, "请先在共享设置中启用并配置 AI 辅助", Toast.LENGTH_SHORT).show();
            return;
        }
        String selected = selectedEditorText();
        if (!aiPolishReady() || !AiPolishConfiguration.acceptableText(selected)) {
            Toast.makeText(this, "请先完成当前输入，再选择一万字以内的文字", Toast.LENGTH_SHORT).show();
            return;
        }
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        aiRequestConfiguration = aiPolishConfiguration;
        aiSourceText = selected;
        aiTarget = new EditorContextSnapshot(connection, editorContextRevision, editorContext(true),
            selected, editorContext(false));
        renderAiPolish();
        aiPolishContainer.setVisibility(View.VISIBLE);
    }

    private void sendAiPolish() {
        if (aiBusy || aiRequestConfiguration == null || !aiTargetMatches()
                || !aiRequestConfiguration.equals(aiPolishConfiguration)) {
            aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
            renderAiPolish();
            return;
        }
        aiBusy = true;
        aiError = "";
        renderAiPolish();
        try {
            aiOperation = aiPolishClient.request(aiRequestConfiguration, aiSourceText,
                (generation, result, failure) -> main.post(
                    () -> finishAiPolish(generation, result, failure)));
        } catch (AiPolishClient.Failure error) {
            aiBusy = false;
            aiError = error.reason() == AiPolishClient.Reason.BUSY
                ? "已有 AI 请求正在处理，请稍后重试。" : "无法启动 AI 请求，请检查配置。";
            renderAiPolish();
        }
    }

    private void finishAiPolish(long generation, String result, AiPolishClient.Failure failure) {
        if (aiOperation == null || aiOperation.generation() != generation
                || aiPolishContainer == null
                || aiPolishContainer.getVisibility() != View.VISIBLE) return;
        aiOperation = null;
        aiBusy = false;
        if (!aiTargetMatches() || aiRequestConfiguration == null
                || !aiRequestConfiguration.equals(aiPolishConfiguration)) {
            aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
        } else if (failure != null) {
            aiError = failure.reason() == AiPolishClient.Reason.INVALID
                ? "服务返回的文字为空或超过一万字。"
                : "AI 请求失败，请检查网络、地址、模型和密钥。";
        } else {
            aiOutputText = result;
            aiError = "";
        }
        renderAiPolish();
    }

    private void replaceAiSelection() {
        if (aiOutputText.isEmpty() || !aiPolishReady() || !aiTargetMatches()
                || aiRequestConfiguration == null
                || !aiRequestConfiguration.equals(aiPolishConfiguration)) {
            aiError = "输入位置或 AI 配置已变化，请返回键盘后重试。";
            renderAiPolish();
            return;
        }
        boolean committed;
        committed = commitText(aiOutputText, TypingSource.AI);
        if (committed) closeAiPolish();
        else {
            aiError = "编辑器拒绝替换，请返回键盘后重试。";
            renderAiPolish();
        }
    }

    private void renderAiPolish() {
        if (aiPolishPanel == null || aiPolishActions == null) return;
        aiPolishPanel.removeAllViews();
        aiPolishActions.removeAllViews();
        LinearLayout header = new LinearLayout(this);
        TextView title = new TextView(this);
        title.setText("AI 润色");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        button(header, "返回键盘", this::closeAiPolish);
        aiPolishPanel.addView(header);
        if (!aiError.isEmpty()) {
            TextView error = new TextView(this);
            error.setText(aiError);
            error.setTextColor(Color.RED);
            error.setContentDescription("AI 润色状态");
            aiPolishPanel.addView(error);
        }
        if (aiRequestConfiguration != null) {
            TextView destination = new TextView(this);
            destination.setText("发送到 " + aiRequestConfiguration.destination() + " · "
                + aiRequestConfiguration.model());
            destination.setContentDescription("AI 请求目标和模型");
            aiPolishPanel.addView(destination);
        }
        TextView label = new TextView(this);
        label.setText(aiOutputText.isEmpty() ? "待发送的选中文字" : "润色结果");
        aiPolishPanel.addView(label);
        TextView content = new TextView(this);
        content.setText(aiOutputText.isEmpty() ? aiSourceText : aiOutputText);
        content.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        content.setContentDescription(aiOutputText.isEmpty() ? "待润色文字" : "AI 润色结果");
        aiPolishPanel.addView(content);
        if (aiBusy) {
            TextView progress = new TextView(this);
            progress.setText("正在请求…");
            aiPolishPanel.addView(progress);
            Button cancel = button(aiPolishActions, "取消请求", () -> {
                cancelAiRequest();
                renderAiPolish();
            });
            cancel.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        } else if (aiOutputText.isEmpty()) {
            Button send = button(aiPolishActions, "发送选中文字", this::sendAiPolish);
            send.setEnabled(aiTargetMatches() && aiRequestConfiguration != null
                && aiRequestConfiguration.equals(aiPolishConfiguration));
            send.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        } else {
            Button replace = button(aiPolishActions, "替换选中文字", this::replaceAiSelection);
            replace.setEnabled(aiTargetMatches() && aiRequestConfiguration != null
                && aiRequestConfiguration.equals(aiPolishConfiguration));
            replace.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        applySkin();
    }

    private void startVoiceRecognition() {
        if (!voiceInputEnabled) {
            Toast.makeText(this, "请先在共享设置中启用语音输入", Toast.LENGTH_SHORT).show();
            return;
        }
        String requestId = "ime-" + Long.toUnsignedString(SystemClock.uptimeMillis());
        // The configuration the settings app resolves, read through the same shared entry. This
        // keyboard's voice button used to launch the platform recogniser unconditionally, so a
        // user who had configured a provider got it from the settings panel and not from here.
        VoiceConfiguration configured = VoiceConfiguration.read(preferencesDirectory, requestId);
        if (configured.provider() == null && !VoiceRecognitionActivity.available(this)) {
            // Only reached with no provider configured, so name the way out rather than leaving
            // the user with a device limitation and nothing to do about it.
            Toast.makeText(this, "设备没有可用的系统语音识别服务，可在设置中配置识别服务商",
                Toast.LENGTH_LONG).show();
            return;
        }
        closeVoiceResult();
        try {
            VoiceRecognitionActivity.launch(this, requestId, voiceLanguage,
                configured.providerName(), configured.endpoint(), configured.model(),
                configured.token(), configured.streaming(), configured.polish(),
                configured.localModel());
        } catch (RuntimeException error) {
            VoiceRecognitionActivity.clearRequest(requestId);
            Toast.makeText(this, "语音识别服务无法启动", Toast.LENGTH_SHORT).show();
        }
    }

    private void insertVoiceResult() {
        VoiceResultStore.Entry entry = voiceResultEntry;
        if (entry == null || voiceResultStore == null) return;
        if (!voiceInsertionReady() || !voiceTargetMatches()) {
            Toast.makeText(this, "输入位置已变化，请关闭后重新打开语音结果", Toast.LENGTH_SHORT).show();
            return;
        }
        try {
            String text = voiceResultStore.consume(entry.id(), System.currentTimeMillis());
            boolean committed;
            committed = commitText(text, TypingSource.VOICE);
            closeVoiceResult();
            if (!committed)
                Toast.makeText(this, "编辑器拒绝插入；结果已安全清除", Toast.LENGTH_SHORT).show();
        } catch (VoiceResultStore.Failure error) {
            voiceResultEntry = null;
            renderVoiceResult();
            Toast.makeText(this, error.reason() == VoiceResultStore.Reason.BUSY
                ? "语音结果正在更新，请稍后重试" : "语音结果已过期、已使用或不可读取",
                Toast.LENGTH_SHORT).show();
        }
    }

    /**
     * Opens the client app from the keyboard.
     *
     * 走包管理器要启动 Intent，而不是直接 new Intent(this, HomeActivity.class)：输入法这一侧单独编译，
     * classpath 上没有 app.msime.android.home，引类就编不过。它同时也更对路 —— 主屏图标那四个主题是
     * activity-alias，启动项是哪一个由当时启用的那一个决定，这里问的就是它。
     *
     * 输入法是 Service，不属于任何任务栈，所以必须自己起一个新任务；不带 FLAG_ACTIVITY_NEW_TASK 时
     * 这一调用直接抛 AndroidRuntimeException。CLEAR_TOP 是为了让重复点击回到已经开着的那一份，而不是
     * 在栈上再叠一个首页。
     *
     * 失败时说出来。系统可能因为后台启动限制拒掉它，而那种拒绝是静默的——不提示的话，用户看到的是
     * 点了没反应。
     */
    private void openClientApp() {
        Intent intent = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (intent == null) {
            Toast.makeText(this, "无法打开水杉输入法，请从主屏幕进入", Toast.LENGTH_SHORT).show();
            return;
        }
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        try {
            startActivity(intent);
        } catch (RuntimeException error) {
            Toast.makeText(this, "无法打开水杉输入法，请从主屏幕进入", Toast.LENGTH_SHORT).show();
        }
    }

    private void showVoiceResult() {
        if (!voiceInsertionReady()) {
            Toast.makeText(this, "请先完成当前输入，再插入语音结果", Toast.LENGTH_SHORT).show();
            return;
        }
        if (voiceResultStore == null || voiceResultScroll == null) {
            Toast.makeText(this, "语音结果存储尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        try { voiceResultEntry = voiceResultStore.read(System.currentTimeMillis()); }
        catch (VoiceResultStore.Failure error) {
            Toast.makeText(this, error.reason() == VoiceResultStore.Reason.BUSY
                ? "语音结果正在更新，请稍后重试" : "语音结果无法读取",
                Toast.LENGTH_SHORT).show();
            return;
        }
        captureVoiceTarget();
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeAiPolish();
        renderVoiceResult();
        voiceResultScroll.setVisibility(View.VISIBLE);
    }

    private void renderVoiceResult() {
        if (voiceResultPanel == null) return;
        voiceResultPanel.removeAllViews();
        LinearLayout header = new LinearLayout(this);
        TextView title = new TextView(this);
        title.setText("语音结果");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        button(header, "返回键盘", this::closeVoiceResult);
        voiceResultPanel.addView(header);
        if (voiceResultEntry == null) {
            TextView empty = new TextView(this);
            // Neutral about which engine runs: since the keyboard entry honours a configured
            // provider, naming the system recognizer here was wrong exactly for the users who had
            // configured one. Which service is used is the settings page's to explain.
            empty.setText("暂无待插入结果。点击下方按钮开始语音识别；只保留最新一条，10 分钟内有效。");
            voiceResultPanel.addView(empty);
        } else {
            TextView recognized = new TextView(this);
            recognized.setText(voiceResultEntry.text());
            recognized.setContentDescription("待插入语音结果");
            voiceResultPanel.addView(recognized);
            TextView hint = new TextView(this);
            hint.setText("点击插入后清除待插入结果；输入位置变化时会拒绝插入。");
            voiceResultPanel.addView(hint);
            Button insert = button(voiceResultPanel, "插入语音结果", this::insertVoiceResult);
            insert.setContentDescription("插入并清除语音结果");
            insert.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        Button recognize = button(voiceResultPanel, "开始语音识别",
            this::startVoiceRecognition);
        recognize.setLayoutParams(new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        recognize.setEnabled(voiceInputEnabled && VoiceRecognitionActivity.available(this));
        applySkin();
    }

    private void renderLayoutSettingsState() {
        if (keySpacingSlider != null && rowSpacingSlider != null && keyboardHeightSlider != null
                && keySpacingValue != null && rowSpacingValue != null && keyboardHeightValue != null
                && voiceShortcutSwitch != null && resetLayoutSettingsButton != null) {
            keySpacingSlider.setProgress(touchKeySpacingTenths);
            rowSpacingSlider.setProgress(touchRowSpacingTenths);
            keyboardHeightSlider.setProgress(touchKeyboardHeightAdjustment);
            keySpacingSlider.setEnabled(!touchGeometrySaving && !traditionalOutputSaving);
            rowSpacingSlider.setEnabled(!touchGeometrySaving && !traditionalOutputSaving);
            keyboardHeightSlider.setEnabled(!touchGeometrySaving && !traditionalOutputSaving);
            voiceShortcutSwitch.setChecked(touchVoiceShortcutEnabled);
            voiceShortcutSwitch.setEnabled(!touchGeometrySaving && !traditionalOutputSaving);
            resetLayoutSettingsButton.setEnabled(!touchGeometrySaving && !traditionalOutputSaving);
            keySpacingValue.setText(KeyboardGeometry.display(touchKeySpacingTenths) + " dp");
            rowSpacingValue.setText(KeyboardGeometry.display(touchRowSpacingTenths) + " dp");
            keyboardHeightValue.setText(KeyboardGeometry.displayHeight(
                touchKeyboardHeightAdjustment) + " dp");
        }
        if (layoutAdjustView != null) {
            layoutAdjustView.update(touchKeySpacingTenths, touchRowSpacingTenths,
                touchKeyboardHeightAdjustment, touchVoiceShortcutEnabled);
            layoutAdjustView.setAdjustmentsEnabled(
                !touchGeometrySaving && !traditionalOutputSaving);
            layoutAdjustView.updateSkin(skin);
        }
    }

    private void previewTouchGeometry(boolean keySpacing, int value) {
        if (touchGeometrySaving || traditionalOutputSaving) return;
        if (keySpacing) touchKeySpacingTenths = KeyboardGeometry.keySpacing(value);
        else touchRowSpacingTenths = KeyboardGeometry.rowSpacing(value);
        renderLayoutSettingsState();
        applyKeyboardGeometry();
    }

    private void previewTouchHeight(int value) {
        if (touchGeometrySaving || traditionalOutputSaving) return;
        touchKeyboardHeightAdjustment = KeyboardGeometry.heightAdjustment(value);
        renderLayoutSettingsState();
        applyKeyboardGeometry();
    }

    private void configureSpacingSlider(SeekBar slider, boolean keySpacing) {
        slider.setMin(keySpacing ? KeyboardGeometry.MIN_KEY_SPACING_TENTHS
            : KeyboardGeometry.MIN_ROW_SPACING_TENTHS);
        slider.setMax(keySpacing ? KeyboardGeometry.MAX_KEY_SPACING_TENTHS
            : KeyboardGeometry.MAX_ROW_SPACING_TENTHS);
        slider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar source, int progress, boolean fromUser) {
                if (fromUser) {
                    previewTouchGeometry(keySpacing, progress);
                    if (!source.isPressed()) saveTouchGeometry();
                }
            }
            @Override public void onStartTrackingTouch(SeekBar source) { }
            @Override public void onStopTrackingTouch(SeekBar source) { saveTouchGeometry(); }
        });
    }

    private void configureHeightSlider(SeekBar slider) {
        slider.setMin(KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_DP);
        slider.setMax(KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_DP);
        slider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar source, int progress, boolean fromUser) {
                if (fromUser) {
                    previewTouchHeight(progress);
                    if (!source.isPressed()) saveTouchGeometry();
                }
            }
            @Override public void onStartTrackingTouch(SeekBar source) { }
            @Override public void onStopTrackingTouch(SeekBar source) { saveTouchGeometry(); }
        });
    }

    private void showLayoutSettings() {
        if (touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) {
            Toast.makeText(this, "键盘设置尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeVoiceResult();
        closeAiPolish();
        renderLayoutSettingsState();
        if (layoutAdjustView != null) {
            layoutSettingsScroll.setVisibility(View.GONE);
            layoutAdjustView.setVisibility(View.VISIBLE);
            layoutAdjustView.requestFocus();
        } else {
            layoutSettingsScroll.setVisibility(View.VISIBLE);
        }
    }

    private void saveTouchGeometry() {
        saveTouchGeometry(false);
    }

    private void resetTouchGeometry() {
        if (touchGeometrySaving || traditionalOutputSaving || session == 0
                || preferencesSnapshot == null || preferencesDirectory.isEmpty()) return;
        touchKeySpacingTenths = KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS;
        touchRowSpacingTenths = KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS;
        touchKeyboardHeightAdjustment = KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_DP;
        touchVoiceShortcutEnabled = false;
        applyKeyboardGeometry();
        saveTouchGeometry(true);
    }

    private void saveTouchGeometry(boolean reset) {
        if (touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) return;
        JSONObject acceptedPreferences = preferencesSnapshot.optJSONObject("preferences");
        if (!reset && acceptedPreferences != null
                && KeyboardGeometry.keySpacing(acceptedPreferences.optInt(
                    "touch_key_spacing_tenths", -1)) == touchKeySpacingTenths
                && KeyboardGeometry.rowSpacing(acceptedPreferences.optInt(
                    "touch_row_spacing_tenths", -1)) == touchRowSpacingTenths
                && KeyboardGeometry.heightAdjustment(acceptedPreferences.optInt(
                    "touch_keyboard_height_adjustment", Integer.MIN_VALUE))
                    == touchKeyboardHeightAdjustment
                && acceptedPreferences.optBoolean("touch_voice_shortcut", false)
                    == touchVoiceShortcutEnabled) return;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = pending.getLong("revision");
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            JSONObject preferences = pending.getJSONObject("preferences");
            if (reset) {
                preferences.remove("touch_key_spacing_tenths");
                preferences.remove("touch_row_spacing_tenths");
                preferences.remove("touch_keyboard_height_adjustment");
                preferences.remove("touch_voice_shortcut");
            } else {
                preferences.put("touch_key_spacing_tenths", touchKeySpacingTenths);
                preferences.put("touch_row_spacing_tenths", touchRowSpacingTenths);
                preferences.put("touch_keyboard_height_adjustment", touchKeyboardHeightAdjustment);
                preferences.put("touch_voice_shortcut", touchVoiceShortcutEnabled);
            }
        } catch (JSONException error) {
            if (reset && preferencesSnapshot != null) {
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                applyKeyboardGeometry();
            }
            preferencesNotice = reset ? " · 恢复默认失败，保留原设置" : " · 键盘设置保存失败，保留原设置";
            render();
            return;
        }
        touchGeometrySaving = true;
        preferencesNotice = " · 正在保存键盘设置";
        final long operation = ++preferenceSaveGeneration;
        renderLayoutSettingsState();
        render();
        Runnable save = () -> {
            String response;
            try {
                response = NativeClient.savePreferences(targetDirectory, expectedRevision,
                    pending.toString());
            } catch (Exception | LinkageError error) {
                response = null;
            }
            final String savedResponse = response;
            main.post(() -> finishTouchGeometrySave(operation, targetSession, targetDirectory,
                reset, savedResponse));
        };
        try {
            preferencesWorker.execute(save);
        } catch (RuntimeException error) {
            if (operation == preferenceSaveGeneration) {
                touchGeometrySaving = false;
                if (reset && preferencesSnapshot != null) {
                    applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                    applyKeyboardGeometry();
                }
                preferencesNotice = reset ? " · 恢复默认失败，保留原设置" : " · 键盘设置保存失败，保留原设置";
                renderLayoutSettingsState();
                render();
            }
        }
    }

    private void finishTouchGeometrySave(long operation, long targetSession,
                                         String targetDirectory, boolean reset, String response) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        touchGeometrySaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long savedRevision = saved.getLong("revision");
            if (preferencesSnapshot != null
                    && preferencesSnapshot.optLong("revision", -1) > savedRevision) {
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                applyKeyboardGeometry();
                preferencesNotice = "";
            } else {
                applyPreferencesSnapshot(saved);
                preferencesNotice = reset ? " · 键盘设置已恢复默认" : " · 键盘设置已保存";
            }
        } catch (JSONException | LinkageError error) {
            if (preferencesSnapshot != null)
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
            applyKeyboardGeometry();
            preferencesNotice = reset ? " · 恢复默认失败，已恢复原设置" : " · 键盘设置保存失败，已恢复原设置";
            Toast.makeText(this, reset ? "键盘设置未能恢复默认" : "键盘设置未能保存", Toast.LENGTH_SHORT).show();
        }
        renderLayoutSettingsState();
        render();
    }

    private void showSchemePicker() {
        if (touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) {
            Toast.makeText(this, "输入方案尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeLayoutSettings();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        renderSchemePicker();
        schemeScroll.setVisibility(View.VISIBLE);
    }

    private void renderSchemePicker() {
        if (schemePanel == null) return;
        schemePanel.removeAllViews();
        LinearLayout header = new LinearLayout(this);
        // 标题去掉了：这一屏只有方案卡片，左上返回、右上设置，和母版一致。写着「输入方案」的那行
        // 字和那颗「返回键盘」按钮，占的是卡片的位置，说的却是用户已经看见的事。
        Button close = borderlessButton(header, "‹", this::closeSchemePicker);
        close.setContentDescription("返回键盘");
        // 高度写 0，不写 WRAP_CONTENT：裸 View 的默认测量在 AT_MOST 下取满可用空间，这一条
        // 占位会把标题栏撑到整屏高，卡片区就一点高度都分不到了。
        header.addView(new View(this), new LinearLayout.LayoutParams(0, 0, 1));
        Button settings = borderlessButton(header, "⚙", this::showFeedbackMenu);
        settings.setContentDescription("键盘设置");
        schemePanel.addView(header);
        LinearLayout schemeSurface = new LinearLayout(this);
        schemeSurface.setOrientation(LinearLayout.VERTICAL);
        schemeSurface.setPadding(pixels(8), pixels(6), pixels(8), pixels(6));
        schemeSurface.setContentDescription("输入方案卡片区域");
        // 卡面按内容高度收，不再撑满标题以下的全部空间。撑满原本是为了「短列表下面不要露出
        // 键盘底纹」，但十三张卡片也填不满一屏，结果是一大块什么都没有的白。露出的是选择器
        // 自己的底色，与卡片同一套配色，比那块空白好看。
        schemePanel.addView(schemeSurface, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        java.util.List<KeyboardScheme> schemes = enabledSchemes;
        int cardCount = schemes.size() + 1;
        // The English card sits third when there are enough schemes to put it there, and last
        // otherwise. Pinning it to index 2 made a single enabled scheme index past the end of
        // the list, which threw on the main thread and took the IME down with it.
        final int englishIndex = Math.min(2, schemes.size());
        java.util.List<KeyboardSchemeCard> schemeCards = new java.util.ArrayList<>();
        java.util.List<Boolean> cardSelection = new java.util.ArrayList<>();
        for (int start = 0; start < cardCount; start += 4) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.HORIZONTAL);
            // 卡片是小布局，没有文字基线可对齐。
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 4; slot++) {
                int index = start + slot;
                if (index >= cardCount) {
                    View spacer = new View(this);
                    row.addView(spacer, new LinearLayout.LayoutParams(0, pixels(72), 1));
                    continue;
                }
                if (index == englishIndex) {
                    // English is a platform text mode, not a second persisted Engine scheme.
                    KeyboardSchemeCard card = new KeyboardSchemeCard(
                        this, "EN", "26", "英文 26 键");
                    card.setOnClickListener(ignored -> {
                        playFeedback(card);
                        selectEnglishScheme();
                    });
                    row.addView(card, new LinearLayout.LayoutParams(0, pixels(72), 1));
                    card.setEnabled(!schemeSaving);
                    card.setContentDescription("输入方案卡片 英文 26 键");
                    if (Build.VERSION.SDK_INT >= 30)
                        card.setStateDescription(dedicatedEnglish ? "已选中" : "未选中");
                    schemeCards.add(card);
                    cardSelection.add(dedicatedEnglish);
                    continue;
                }
                int schemeIndex = index > englishIndex ? index - 1 : index;
                KeyboardScheme scheme = schemes.get(schemeIndex);
                // Apple renders scheme cards with the same press-feedback surface as keys. Keep
                // the Android-specific scheme persistence and selection guards in the callback.
                KeyboardSchemeCard card = new KeyboardSchemeCard(
                    this, scheme.glyph(), scheme.badge(), scheme.title());
                card.setOnClickListener(ignored -> {
                    playFeedback(card);
                    selectKeyboardScheme(scheme);
                });
                row.addView(card, new LinearLayout.LayoutParams(0, pixels(72), 1));
                card.setEnabled(!schemeSaving);
                card.setContentDescription("输入方案卡片 " + scheme.title());
                if (Build.VERSION.SDK_INT >= 30)
                    card.setStateDescription(scheme == selectedScheme ? "已选中" : "未选中");
                schemeCards.add(card);
                cardSelection.add(scheme == selectedScheme);
            }
            schemeSurface.addView(row);
        }
        if (!sharedSchemePreferences) {
            Button toggleReply = button(schemeSurface, thoughtfulReplyEnabled()
                ? "禁用高情商回复" : "启用高情商回复", this::toggleThoughtfulReplyScheme);
            toggleReply.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            toggleReply.setContentDescription(thoughtfulReplyEnabled()
                ? "禁用高情商回复输入方案" : "启用高情商回复输入方案");
        }
        // 共享设置在管方案时不再解释这件事：那句话说的是宿主之间怎么协作，不是用户在这一屏要
        // 做的决定，而它占着选择器底部——列表短的时候，它比方案本身还显眼。
        if (!sharedSchemePreferences) {
            TextView hint = new TextView(this);
            hint.setText("切换会先完成当前组词，并迁移到共享输入方案设置");
            schemeSurface.addView(hint);
        }
        applySkin();
        // Apple keeps the selectable scheme area on a filled key surface, so a short list does not
        // leave a bare keyboard backdrop below the cards. Apply this after the recursive skin pass:
        // the picker itself remains the patterned backdrop while this inner surface follows the
        // selected skin's key material, including custom Android skins.
        int cardSurface = Color.parseColor(skin.keyBackground());
        schemeSurface.setBackground(new KeyboardSkinKeyDrawable(skin, cardSurface, false,
            getResources().getDisplayMetrics().density));
        // 也必须在那一趟之后：它会把每个 TextView 重新刷成 keyForeground，卡片的强调色先上就没了。
        int cardAccent = Color.parseColor(skin.accent());
        for (int index = 0; index < schemeCards.size(); index++) {
            schemeCards.get(index).paint(cardAccent, cardSurface, cardSelection.get(index));
        }
    }

    private void selectEnglishScheme() {
        if (schemeSaving || touchGeometrySaving || traditionalOutputSaving || session == 0) return;
        if (!dedicatedEnglish) toggleInputLanguage();
        if (session == 0) return;
        closeSchemePicker();
        render();
    }

    private void selectKeyboardScheme(KeyboardScheme scheme) {
        if (schemeSaving || touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) return;
        // Match Apple's scheme picker: choosing a Chinese scheme from the English card
        // returns to Chinese mode before applying the persisted Engine scheme.
        if (dedicatedEnglish) {
            toggleInputLanguage();
            if (session == 0) return;
        }
        if (scheme == selectedScheme) {
            closeSchemePicker();
            if (scheme == KeyboardScheme.THOUGHTFUL_REPLY) showReplyKeyboard();
            return;
        }
        replyModel.resetResults();
        clearReplyRequestReferences();
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        try {
            // Scheme replacement is never deferred: complete Engine composition first.
            apply(NativeClient.command(targetSession, 2));
            if (session != targetSession || preferencesSnapshot == null
                    || !targetDirectory.equals(preferencesDirectory)) return;
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = pending.getLong("revision");
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            JSONObject preferences = pending.getJSONObject("preferences");
            String currentScheme = preferences.optString("scheme", "quanpin");
            String lastChinese = preferences.optString("last_chinese_scheme", currentScheme);
            KeyboardScheme.PreferenceMapping mapping = scheme.mapping(lastChinese,
                preferences.optString("shuangpin_profile", "xiaohe"));
            preferences.put("scheme", mapping.scheme());
            preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
            preferences.put("shuangpin_profile", mapping.shuangpinProfile());
            preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
            JSONArray enabled = new JSONArray();
            for (KeyboardScheme candidate : enabledSchemes) {
                enabled.put(candidate.preferenceId());
            }
            preferences.put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", enabled).put("selected", scheme.preferenceId()));
        } catch (JSONException | LinkageError error) {
            preferencesNotice = " · 输入方案切换失败，保留当前设置";
            closeSchemePicker();
            render();
            return;
        }
        closeSchemePicker();
        schemeSaving = true;
        preferencesNotice = " · 正在切换输入方案";
        final long operation = ++preferenceSaveGeneration;
        render();
        Runnable save = () -> {
            String response;
            try {
                response = NativeClient.savePreferences(targetDirectory, expectedRevision, pending.toString());
            } catch (Exception | LinkageError error) {
                response = null;
            }
            final String savedResponse = response;
            main.post(() -> finishSchemeSave(operation, targetSession, targetDirectory, savedResponse));
        };
        try {
            preferencesWorker.execute(save);
        } catch (RuntimeException error) {
            if (operation == preferenceSaveGeneration) {
                schemeSaving = false;
                preferencesNotice = " · 输入方案切换失败，保留当前设置";
                render();
            }
        }
    }

    private void finishSchemeSave(long operation, long targetSession, String targetDirectory,
                                  String response) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        schemeSaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long savedRevision = saved.getLong("revision");
            if (preferencesSnapshot != null
                    && preferencesSnapshot.optLong("revision", -1) > savedRevision) {
                preferencesNotice = "";
            } else {
                applyPreferencesSnapshot(saved);
                preferencesNotice = " · 输入方案已切换";
            }
        } catch (JSONException | LinkageError error) {
            // A conflict or storage failure leaves the working session unchanged.
            preferencesNotice = " · 输入方案切换失败，保留当前设置";
            Toast.makeText(this, "输入方案未能保存", Toast.LENGTH_SHORT).show();
        }
        synchronizeReplyKeyboard();
        render();
    }

    private void toggleThoughtfulReplyScheme() {
        if (sharedSchemePreferences || schemeHostPreferences == null
                || schemeSaving || traditionalOutputSaving) return;
        boolean enabled = thoughtfulReplyEnabled();
        schemeHostPreferences.edit().putBoolean(THOUGHTFUL_REPLY_ENABLED, !enabled).apply();
        if (enabled && selectedScheme == KeyboardScheme.THOUGHTFUL_REPLY) {
            selectedScheme = KeyboardScheme.QUANPIN;
            saveHostScheme(selectedScheme);
            replySuppressed = false;
            closeReplyKeyboard();
        }
        renderSchemePicker();
        render();
    }

    private void insertClipboardText(String text) {
        // Only that there is text. Re-checking the shared store's own bounds here is what made
        // an entry another mobile host saved listable but not insertable on this one.
        if (connection == null || !ClipboardHistoryPolicy.hasText(text)) return;
        command(2);
        commitText(text);
        closeClipboardHistory();
    }

    private void captureClipboardText() {
        if (!clipboardHistoryEnabled || clipboardHistory == null) return;
        try {
            ClipboardManager manager = getSystemService(ClipboardManager.class);
            if (manager == null || !manager.hasPrimaryClip() || manager.getPrimaryClip() == null
                    || manager.getPrimaryClip().getItemCount() == 0
                    || manager.getPrimaryClipDescription() == null
                    || !(manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                        || manager.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML))) {
                Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.Rejection.EMPTY), Toast.LENGTH_SHORT).show();
                return;
            }
            CharSequence value = manager.getPrimaryClip().getItemAt(0).getText();
            if (!ClipboardHistoryPolicy.hasText(value == null ? null : value.toString())) {
                Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.Rejection.EMPTY), Toast.LENGTH_SHORT).show();
                return;
            }
            // The shared store refuses rather than throws, and says which refusal it is. Deciding
            // that here as well is what made this host disagree with the store it writes into.
            String reason = clipboardHistory.add(value.toString());
            if (reason != null) {
                Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.rejectionFor(reason)), Toast.LENGTH_SHORT).show();
                return;
            }
            renderClipboardHistory();
        } catch (IllegalArgumentException | IllegalStateException | SecurityException error) {
            Toast.makeText(this, "无法保存当前剪贴板", Toast.LENGTH_SHORT).show();
        }
    }

    private void manageClipboardItem(Button anchor, ClipboardHistory.Item item) {
        PopupMenu popup = new PopupMenu(this, anchor);
        MenuItem pin = popup.getMenu().add(item.pinned() ? "取消固定" : "固定");
        MenuItem remove = popup.getMenu().add("删除");
        // Offered wherever the cloud half is, so the action is discoverable; it only runs once this panel's fetch said the account is signed in with the cloud clipboard on.
        boolean cloudAllowed = cloudClipboardAllowed();
        MenuItem upload = cloudAllowed ? popup.getMenu().add(CloudClipboardPanelPolicy.UPLOAD_ACTION) : null;
        if (upload != null) upload.setEnabled(CloudClipboardPanelPolicy.canUpload(
            cloudAllowed, cloudClipboardStatus, item.text()));
        popup.setOnMenuItemClickListener(selected -> {
            if (upload != null && selected == upload) {
                uploadClipboardText(item.text());
                return true;
            }
            if (clipboardHistory == null) return false;
            try {
                if (selected == pin) clipboardHistory.setPinned(item.text(), !item.pinned());
                else if (selected == remove) clipboardHistory.remove(item.text());
                else return false;
            } catch (IllegalStateException error) {
                Toast.makeText(this, "无法修改剪贴板历史", Toast.LENGTH_SHORT).show();
                return true;
            }
            renderClipboardHistory();
            return true;
        });
        popup.show();
    }

    private void confirmClearClipboardHistory() {
        new AlertDialog.Builder(this)
            .setTitle("清空剪贴板历史")
            .setMessage("将删除全部历史，包括固定项。")
            .setNegativeButton("取消", null)
            .setPositiveButton("清空", (dialog, which) -> {
                // The user asked for this one, so a refusal is reported rather than swallowed.
                try {
                    if (clipboardHistory != null) clipboardHistory.clear();
                } catch (IllegalStateException error) {
                    Toast.makeText(this, "无法清空剪贴板历史", Toast.LENGTH_SHORT).show();
                }
                renderClipboardHistory();
            })
            .show();
    }

    private void showClipboardHistory() {
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!CloudClipboardPanelPolicy.panelAvailable(clipboardHistoryEnabled, cloudAllowed)
                || clipboardScroll == null) return;
        // Clipboard entries are independent editor text. Finish the active composition when the
        // panel opens, matching the symbol and emoji panels instead of leaving stale preedit behind
        // while the user browses history.
        command(2);
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeSchemePicker();
        closeLayoutSettings();
        closeVoiceResult();
        closeAiPolish();
        clipboardTab = CloudClipboardPanelPolicy.initialTab(
            clipboardTab, clipboardHistoryEnabled, cloudAllowed);
        renderClipboardHistory();
        clipboardScroll.setVisibility(View.VISIBLE);
        // Fetched on every opening, whichever half is showing: the local half's 发到云剪贴板 needs to know the account is signed in with the cloud clipboard on.
        if (cloudAllowed) refreshCloudClipboard();
    }

    private boolean clipboardPanelOpen() {
        return clipboardScroll != null && clipboardScroll.getVisibility() == View.VISIBLE;
    }

    private boolean cloudClipboardAllowed() {
        return CloudClipboardPanelPolicy.cloudAllowed(editorInputType, allowLearning);
    }

    private void selectClipboardTab(CloudClipboardPanelPolicy.Tab tab) {
        if (tab == CloudClipboardPanelPolicy.Tab.CLOUD && !cloudClipboardAllowed()) return;
        clipboardTab = tab;
        renderClipboardHistory();
    }

    /**
     * Ask the service for this account's cloud list, off the main thread.
     *
     * <p>The answer is drawn only if the field and the panel are still the ones it was asked for; otherwise it is dropped. Errors carry no response text, and nothing about the request is logged.
     */
    private void refreshCloudClipboard() {
        if (!cloudClipboardAllowed()) return;
        long generation = ++cloudClipboardGeneration;
        cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
        cloudClipboardItems = java.util.List.of();
        if (clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
        try {
            cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status status;
                java.util.List<BackendAccount.ClipboardItem> items = java.util.List.of();
                try {
                    BackendAccount account = new BackendAccount(this);
                    // Throws when the session owner cannot tell right now, which is a retry, not a sign-in.
                    if (account.currentAccessToken().isEmpty()) {
                        status = CloudClipboardPanelPolicy.Status.SIGNED_OUT;
                    } else {
                        BackendAccount.ClipboardPage page = account.clipboard("");
                        status = CloudClipboardPanelPolicy.loaded(page.enabled(), page.items().size());
                        if (CloudClipboardPanelPolicy.showsItems(status)) items = page.items();
                    }
                } catch (BackendAccount.RequestException error) {
                    status = CloudClipboardPanelPolicy.failed(error.status);
                } catch (Exception | LinkageError error) {
                    status = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status answer = status;
                java.util.List<BackendAccount.ClipboardItem> answered = items;
                main.post(() -> {
                    if (!CloudClipboardPanelPolicy.accepts(generation, cloudClipboardGeneration)
                            || !clipboardPanelOpen() || !cloudClipboardAllowed()) return;
                    cloudClipboardStatus = answer;
                    cloudClipboardItems = answered;
                    if (clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD) renderClipboardHistory();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            cloudClipboardStatus = CloudClipboardPanelPolicy.Status.FAILED;
        }
    }

    /** Send one local entry to the account's cloud clipboard, because the user asked for exactly this one. */
    private void uploadClipboardText(String text) {
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!CloudClipboardPanelPolicy.canUpload(cloudAllowed, cloudClipboardStatus, text)) {
            Toast.makeText(this, cloudClipboardStatus == CloudClipboardPanelPolicy.Status.SIGNED_OUT
                    || cloudClipboardStatus == CloudClipboardPanelPolicy.Status.DISABLED
                    ? CloudClipboardPanelPolicy.message(cloudClipboardStatus, 0)
                    : "这条记录无法发到云剪贴板", Toast.LENGTH_SHORT).show();
            return;
        }
        long generation = cloudClipboardGeneration;
        try {
            cloudClipboardWorker.execute(() -> {
                CloudClipboardPanelPolicy.Status failure = null;
                try {
                    new BackendAccount(this).addClipboard(text);
                } catch (BackendAccount.RequestException error) {
                    failure = CloudClipboardPanelPolicy.failed(error.status);
                } catch (Exception | LinkageError error) {
                    failure = CloudClipboardPanelPolicy.Status.FAILED;
                }
                CloudClipboardPanelPolicy.Status result = failure;
                main.post(() -> {
                    Toast.makeText(this, result == null ? "已发到云剪贴板"
                        : result == CloudClipboardPanelPolicy.Status.SIGNED_OUT
                            ? CloudClipboardPanelPolicy.SIGNED_OUT_MESSAGE
                            : "未能发到云剪贴板，请稍后重试", Toast.LENGTH_SHORT).show();
                    if (!CloudClipboardPanelPolicy.accepts(generation, cloudClipboardGeneration)
                            || !clipboardPanelOpen()) return;
                    // Re-read rather than splice the entry in: the service deduplicates and orders the list.
                    refreshCloudClipboard();
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            Toast.makeText(this, "未能发到云剪贴板，请稍后重试", Toast.LENGTH_SHORT).show();
        }
    }

    private void insertCloudClipboardText(String text) {
        // Re-checked at the tap: the list was drawn for this field, but a field never gets cloud text once it has turned sensitive.
        if (!cloudClipboardAllowed()) return;
        insertClipboardText(text);
    }

    private void renderClipboardHistory() {
        if (clipboardPanel == null || clipboardHistory == null) return;
        clipboardPanel.removeAllViews();
        boolean cloudAllowed = cloudClipboardAllowed();
        if (!cloudAllowed) clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
        boolean cloud = clipboardTab == CloudClipboardPanelPolicy.Tab.CLOUD;
        LinearLayout header = new LinearLayout(this);
        TextView title = new TextView(this);
        title.setText("剪贴板历史");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        header.addView(title, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        if (cloud) {
            Button refresh = button(header, "刷新", this::refreshCloudClipboard);
            refresh.setEnabled(cloudClipboardStatus != CloudClipboardPanelPolicy.Status.LOADING);
            refresh.setContentDescription("刷新云剪贴板");
        } else if (clipboardHistoryEnabled) {
            button(header, "清空", this::confirmClearClipboardHistory);
        }
        button(header, "返回", this::closeClipboardHistory);
        clipboardPanel.addView(header);
        if (cloudAllowed) {
            LinearLayout tabs = new LinearLayout(this);
            addClipboardTab(tabs, CloudClipboardPanelPolicy.TAB_LOCAL, CloudClipboardPanelPolicy.Tab.LOCAL);
            addClipboardTab(tabs, CloudClipboardPanelPolicy.TAB_CLOUD, CloudClipboardPanelPolicy.Tab.CLOUD);
            clipboardPanel.addView(tabs);
        }
        if (cloud) {
            renderCloudClipboard();
            applySkin();
            return;
        }
        if (!clipboardHistoryEnabled) {
            TextView status = new TextView(this);
            status.setText("剪贴板历史未开启，可在设置中开启");
            clipboardPanel.addView(status);
            applySkin();
            return;
        }
        Button capture = button(clipboardPanel, "保存当前剪贴板", this::captureClipboardText);
        capture.setContentDescription("保存当前剪贴板文本");
        try {
            java.util.List<ClipboardHistory.Item> items = clipboardHistory.load();
            TextView status = new TextView(this);
            // Apple names the affordance next to the count; on Android the pin and delete actions
            // are behind the row's 管理 button, so that is what the hint points at.
            status.setText(items.isEmpty() ? "暂无历史 · 保存后点按插入 · 记录仅保存在本机"
                : items.size() + "/" + ClipboardHistoryPolicy.LIMIT
                    + " 条 · 点按插入 · 管理可固定或删除");
            clipboardPanel.addView(status);
            for (ClipboardHistory.Item item : items) {
                LinearLayout row = new LinearLayout(this);
                Button insert = button(row, item.text(), () -> insertClipboardText(item.text()));
                insert.setContentDescription((item.pinned() ? "已固定；" : "") + "点按插入剪贴板记录");
                Button manage = button(row, item.pinned() ? "已固定" : "管理", () -> {});
                manage.setOnClickListener(ignored -> manageClipboardItem(manage, item));
                row.getChildAt(0).setLayoutParams(new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1));
                row.getChildAt(1).setLayoutParams(new LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
                clipboardPanel.addView(row);
            }
        } catch (IllegalStateException error) {
            TextView status = new TextView(this);
            status.setText("历史记录无法读取，请清空后重试");
            clipboardPanel.addView(status);
        }
        applySkin();
    }

    private void addClipboardTab(LinearLayout tabs, String title, CloudClipboardPanelPolicy.Tab tab) {
        Button button = button(tabs, title, () -> selectClipboardTab(tab));
        button.setSelected(clipboardTab == tab);
        button.setContentDescription("剪贴板分类 " + title);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(button.isSelected() ? "已选中" : "未选中");
        styleButton(button, true);
    }

    private void renderCloudClipboard() {
        TextView status = new TextView(this);
        status.setText(CloudClipboardPanelPolicy.message(cloudClipboardStatus, cloudClipboardItems.size()));
        clipboardPanel.addView(status);
        if (!CloudClipboardPanelPolicy.showsItems(cloudClipboardStatus)) return;
        for (BackendAccount.ClipboardItem item : cloudClipboardItems) {
            LinearLayout row = new LinearLayout(this);
            Button insert = button(row, item.text(), () -> insertCloudClipboardText(item.text()));
            insert.setContentDescription("点按插入云剪贴板记录");
            clipboardPanel.addView(row);
        }
    }

    private void showFeedbackMenu() {
        if (moreButton == null || moreToolsPanel == null || moreToolsScroll == null) return;
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeVoiceResult();
        closeAiPolish();
        closeReplyKeyboard();
        localInputToolsOpen = false;
        renderMoreTools();
        moreToolsScroll.setVisibility(View.VISIBLE);
        moreToolsScroll.requestFocus();
    }

    private Button moreToolsCard(String title, MoreToolsLayout.Section section, boolean active,
                                 boolean enabled, boolean playBeforeAction, Runnable action) {
        return moreToolsCard(title, section, active, enabled, playBeforeAction, null, action);
    }

    private Button moreToolsCard(String title, MoreToolsLayout.Section section, boolean active,
                                 boolean enabled, boolean playBeforeAction, String caption,
                                 Runnable action) {
        // Apple renders every tool card with the same press-feedback surface as a key. Keep the
        // Android card's existing state, accessibility and navigation behavior unchanged.
        Button card = new KeyboardPressButton(this);
        card.setAllCaps(false);
        String state = enabled ? MoreToolsLayout.state(section, active) : "不可用";
        String label = MoreToolsLayout.icon(title) + "  " + title;
        boolean tile = section.tiles();
        boolean navigates = section == MoreToolsLayout.Section.LOCAL_INPUT_BACK;
        // The design's function panel is a grid of icon-over-title tiles; an on setting is told by the tile's tint and its state description, and a caption (振动强度's level) follows the title.
        if (tile) card.setText(MoreToolsLayout.icon(title) + "\n" + title
            + (caption == null ? "" : " " + caption));
        else if (navigates) card.setText(label + "  ›");
        else card.setText(label);
        card.setTextSize(TypedValue.COMPLEX_UNIT_SP, tile ? 12 : 14);
        card.setGravity(navigates
            ? Gravity.CENTER_VERTICAL | Gravity.START : Gravity.CENTER);
        card.setPadding(pixels(tile ? 4 : 12), pixels(tile ? 4 : 5), pixels(tile ? 4 : 12),
            pixels(tile ? 4 : 5));
        if (tile) {
            card.setMaxLines(2);
            card.setLineSpacing(0, .95f);
        }
        card.setContentDescription(title);
        card.setSelected(active);
        card.setEnabled(enabled);
        // A disabled card swallows the press, and the 工具 section draws no state text, so without
        // this 剪贴板历史 and AI 润色 looked exactly like the cards that work and did nothing when
        // pressed. A screen reader was told "不可用"; nobody else was.
        card.setAlpha(enabled ? 1f : .45f);
        if (Build.VERSION.SDK_INT >= 30) card.setStateDescription(state);
        if (tile && card instanceof KeyboardPressButton press)
            press.setKeyboardRole(KeyboardKeyRole.TILE);
        styleButton(card, tile ? KeyboardKeyRole.TILE : KeyboardKeyRole.ACCENT, skin);
        card.setOnClickListener(ignored -> {
            if (playBeforeAction) playFeedback(card);
            action.run();
        });
        return card;
    }

    private void appendMoreToolsSection(MoreToolsLayout.Section section, Button... cards) {
        if (!section.title().isEmpty()) {
            TextView label = new TextView(this);
            label.setText(section.title());
            label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11);
            label.setGravity(Gravity.CENTER_VERTICAL);
            moreToolsPanel.addView(label, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, pixels(20)));
        }
        int columns = section.columns();
        for (int start = 0; start < cards.length; start += columns) {
            LinearLayout row = new LinearLayout(this);
            row.setWeightSum(columns);
            for (int column = 0; column < columns; column++) {
                int index = start + column;
                View child = index < cards.length ? cards[index] : new View(this);
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                    0, pixels(section.height()), 1);
                if (column > 0) params.setMarginStart(pixels(MoreToolsLayout.CARD_SPACING_DP));
                row.addView(child, params);
            }
            LinearLayout.LayoutParams rowParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, pixels(section.height()));
            rowParams.bottomMargin = pixels(MoreToolsLayout.ROW_SPACING_DP);
            moreToolsPanel.addView(row, rowParams);
        }
    }

    private void installShortcutBar(Button dismissButton) {
        shortcutBar.removeAllViews();
        // The design's idle row: the brand mark first, then the scheme pill, the content tools and 收起, with ⚙ at the far end.
        Button[] buttons = {moreButton, schemeButton, replyShortcutButton, emojiShortcutButton,
            voiceShortcutButton, skinButton, dismissButton, layoutSettingsButton};
        for (Button button : buttons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                0, pixels(44), 1);
            params.setMarginStart(pixels(2));
            params.setMarginEnd(pixels(2));
            shortcutBar.addView(button, params);
            button.setMinWidth(pixels(44));
            button.setMinimumWidth(pixels(44));
        }
        // Traditional output and AI are persistent settings/actions in the Apple layout; keep
        // their Android controls detached from the shortcut strip rather than duplicating them.
        scriptShortcutButton.setVisibility(View.GONE);
        aiPolishShortcutButton.setVisibility(View.GONE);
    }

    private Button actionRowKey(KeyboardActionRow.Slot slot) {
        return switch (slot) {
            case SYMBOL_PANEL -> symbolPanelButton;
            case LAYER -> layerButton;
            case GLOBE -> globeButton;
            case PUNCTUATION -> quickPunctuationButton;
            case SPACE -> spaceButton;
            case LANGUAGE -> languageButton;
            case RETURN -> enterButton;
        };
    }

    /**
     * Lay the bottom row out for the surface on screen.
     *
     * <p>Every control here is a long-lived field with its own listeners and state, so the row is
     * re-parented rather than rebuilt: a fresh set of buttons each time would drop the space key's
     * cursor gesture and the delete key's repeat.
     */
    private void updateActionRow() {
        if (actionRow == null) return;
        int layout = displayedTouchLayout(view);
        boolean globe = shouldOfferSwitchingToNextInputMethod();
        // Every keystroke reaches render(), and re-parenting eight keys under the pressed one is a
        // relayout the user can see. The row only changes when the surface does.
        String signature = layout + ":" + globe;
        java.util.List<KeyboardActionRow.Entry> entries =
            KeyboardActionRow.entries(layout, globe);
        // Visibility is re-asserted every time: the reply surface hides this row and restores it
        // without the surface itself having changed.
        actionRow.setVisibility(entries.isEmpty() ? View.GONE : View.VISIBLE);
        if (signature.equals(actionRowSignature)) return;
        actionRowSignature = signature;
        actionRow.removeAllViews();
        for (KeyboardActionRow.Entry entry : entries) {
            Button key = actionRowKey(entry.slot());
            if (key == null) continue;
            if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
            // The shared design tints every function key in this row (123, 中, 换行 and the
            // symbol and globe keys); only the punctuation and space keys wear plain key caps.
            boolean function = entry.slot() != KeyboardActionRow.Slot.SPACE
                && entry.slot() != KeyboardActionRow.Slot.PUNCTUATION;
            KeyboardKeyRole role = entry.slot() == KeyboardActionRow.Slot.RETURN ? returnKeyRole()
                : function ? KeyboardKeyRole.ACCENT : KeyboardKeyRole.KEY;
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(role);
            key.setVisibility(View.VISIBLE);
            actionRow.addView(key, new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.MATCH_PARENT, entry.weight()));
        }
        // The quick punctuation key hides itself when the scheme has no punctuation to offer, and
        // the loop above just told every slot it was visible.
        updateQuickPunctuation();
        applyKeyboardGeometry();
    }

    private void toggleSoundFromMoreTools() {
        soundEnabled = !soundEnabled;
        saveFeedbackPreferences();
        if (soundEnabled) playFeedback(moreButton);
        renderMoreTools();
    }

    private void toggleHapticsFromMoreTools() {
        hapticsEnabled = !hapticsEnabled;
        saveFeedbackPreferences();
        if (hapticsEnabled) playFeedback(moreButton);
        renderMoreTools();
    }

    private void selectHapticStrength(KeyboardFeedbackPreferences.HapticStrength strength) {
        hapticStrength = strength;
        saveFeedbackPreferences();
        if (hapticsEnabled) playFeedback(moreButton);
        renderMoreTools();
    }

    private String hapticStrengthTitle() {
        return switch (hapticStrength) {
            case LIGHT -> "轻";
            case MEDIUM -> "中";
            case STRONG -> "强";
        };
    }

    private void cycleHapticStrength() {
        KeyboardFeedbackPreferences.HapticStrength[] values =
            KeyboardFeedbackPreferences.HapticStrength.values();
        int next = (hapticStrength.ordinal() + 1) % values.length;
        selectHapticStrength(values[next]);
    }

    private boolean traditionalOutputToolAvailable() {
        int scheme = view == null ? -1 : view.optInt("scheme", -1);
        return scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && canSaveChineseOutput();
    }

    private void renderMoreTools() {
        if (moreToolsPanel == null) return;
        moreToolsPanel.removeAllViews();
        LinearLayout header = new LinearLayout(this);
        header.setGravity(Gravity.CENTER_VERTICAL);
        Button close = button(header, "返回", this::closeMoreTools);
        close.setContentDescription("返回键盘");
        close.setLayoutParams(new LinearLayout.LayoutParams(
            pixels(84), pixels(MoreToolsLayout.HEADER_HEIGHT_DP)));
        TextView title = new TextView(this);
        title.setText("工具");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        title.setGravity(Gravity.CENTER);
        header.addView(title, new LinearLayout.LayoutParams(
            0, pixels(MoreToolsLayout.HEADER_HEIGHT_DP), 1));
        View balance = new View(this);
        header.addView(balance, new LinearLayout.LayoutParams(
            pixels(84), pixels(MoreToolsLayout.HEADER_HEIGHT_DP)));
        moreToolsPanel.addView(header);

        if (localInputToolsOpen) {
            appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                moreToolsCard("返回工具", MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                    false, true, true, () -> {
                        localInputToolsOpen = false;
                        renderMoreTools();
                    }));
            LocalInputMode[] modes = LocalInputMode.values();
            Button[] localCards = new Button[modes.length];
            for (int index = 0; index < modes.length; index++) {
                LocalInputMode mode = modes[index];
                localCards[index] = moreToolsCard(mode.title(), MoreToolsLayout.Section.LOCAL_INPUT,
                    false, supportsLocalTools() && localModeEnabled(mode), false, () -> {
                        closeMoreTools();
                        openLocalInputMode(mode);
                    });
            }
            appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT, localCards);
            applySkin();
            return;
        }

        appendMoreToolsSection(MoreToolsLayout.Section.TOOLS,
            moreToolsCard("表情", MoreToolsLayout.Section.TOOLS, false,
                session != 0 && !emojiResources.isEmpty(), true, () -> {
                    closeMoreTools();
                    showEmojiPicker();
                }),
            moreToolsCard("剪贴板历史", MoreToolsLayout.Section.TOOLS, false,
                CloudClipboardPanelPolicy.panelAvailable(clipboardHistoryEnabled,
                    cloudClipboardAllowed()), true, () -> {
                    closeMoreTools();
                    showClipboardHistory();
                }),
            moreToolsCard("AI 润色", MoreToolsLayout.Section.TOOLS, false,
                aiPolishConfiguration != null && aiPolishReady(), true, () -> {
                    closeMoreTools();
                    showAiPolish();
                }),
            moreToolsCard("本地输入", MoreToolsLayout.Section.TOOLS, false,
                supportsLocalTools(), true, () -> {
                    localInputToolsOpen = true;
                    renderMoreTools();
                }),
            moreToolsCard("语音结果", MoreToolsLayout.Section.TOOLS, false,
                true, true, () -> {
                    closeMoreTools();
                    showVoiceResult();
                }),
            // 键盘里改得了的只有这个面板上这些。皮肤、词库、账号、统计都在应用里，而用户正打着字，
            // 没有别的路走过去。
            moreToolsCard("应用设置", MoreToolsLayout.Section.TOOLS, false,
                true, true, () -> {
                    closeMoreTools();
                    openClientApp();
                }));
        appendMoreToolsSection(MoreToolsLayout.Section.SETTINGS,
            moreToolsCard("繁体输出", MoreToolsLayout.Section.SETTINGS, traditionalChineseOutput,
                traditionalOutputToolAvailable(), true, null, this::toggleChineseOutput),
            moreToolsCard("全角输入", MoreToolsLayout.Section.SETTINGS, fullWidthInput,
                true, false, this::toggleFullWidthInput),
            moreToolsCard("中文标点", MoreToolsLayout.Section.SETTINGS, chinesePunctuation,
                true, false, this::toggleChinesePunctuation),
            moreToolsCard("按键音", MoreToolsLayout.Section.SETTINGS, soundEnabled,
                true, false, this::toggleSoundFromMoreTools),
            moreToolsCard("按键振动", MoreToolsLayout.Section.SETTINGS, hapticsEnabled,
                true, false, this::toggleHapticsFromMoreTools),
            moreToolsCard("振动强度", MoreToolsLayout.Section.SETTINGS, hapticsEnabled,
                hapticsEnabled, false, hapticStrengthTitle(), this::cycleHapticStrength));
        applySkin();
    }

    /**
     * Run one maintenance chord, or decline so the key goes on to the application.
     *
     * <p>Deleting declines when the slot is empty or the candidate is not one the Engine will drop
     * — a cloud or AI suggestion is not in the user dictionary to remove — and says so rather than
     * swallowing the chord silently.
     */
    private boolean runMaintenanceChord(int action) {
        try {
            if (action == HardwareMaintenancePolicy.RESET_CACHE) {
                apply(NativeClient.resetCache(session));
                showDiagnostic("已清除候选缓存");
                return true;
            }
            if (!candidateManagementEnabled()) return false;
            JSONObject candidate = visibleCandidate(action);
            JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
            if (id == null || id.optLong("session") != session) return false;
            if (!apply(NativeClient.removeCandidate(session, id.getLong("generation"),
                                                    id.getLong("index")))) {
                showDiagnostic("当前候选不支持此操作");
            }
            return true;
        } catch (JSONException | LinkageError error) {
            fail();
            return true;
        }
    }

    private boolean candidateManagementEnabled() {
        if (view == null || !view.optString("local_mode", "none").equals("none")) return false;
        int scheme = view.optInt("scheme", 0);
        return scheme != 2 && scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME;
    }

    private void editCandidate(JSONObject id, CandidateManagementAction action) {
        if (session == 0 || id == null || id.optLong("session") != session) return;
        try {
            long generation = id.getLong("generation");
            long index = id.getLong("index");
            String result = switch (action) {
                case PROMOTE -> NativeClient.pinCandidate(session, generation, index);
                case FIX_FIRST -> NativeClient.fixCandidatePosition(
                    session, generation, index, action.fixedPosition());
                case CLEAR_POSITION -> NativeClient.clearCandidatePosition(
                    session, generation, index);
                case REMOVE -> NativeClient.removeCandidate(session, generation, index);
            };
            if (!apply(result)) {
                showDiagnostic("当前候选不支持此操作");
            } else if (keyboardRoot != null) {
                keyboardRoot.announceForAccessibility(action.announcement());
            }
        } catch (JSONException | LinkageError error) { fail(); }
    }

    private void confirmCandidateRemoval(JSONObject id, String text) {
        new AlertDialog.Builder(this)
            .setTitle("删除词条")
            .setMessage("确认删除“" + text + "”？")
            .setNegativeButton("取消", null)
            .setPositiveButton("删除", (dialog, which) -> {
                playFeedback(moreButton);
                editCandidate(id, CandidateManagementAction.REMOVE);
            })
            .show();
    }

    private boolean candidateIsCurrent(int slot, JSONObject id, String text) {
        JSONObject current = visibleCandidate(slot);
        JSONObject currentId = current == null ? null : current.optJSONObject("id");
        return current != null && currentId != null && id != null
            && currentId.optLong("session") == id.optLong("session")
            && currentId.optLong("generation") == id.optLong("generation")
            && currentId.optLong("index") == id.optLong("index")
            && text.equals(chineseOutput(current.optString("text"), view));
    }

    private boolean candidateGlossInsertionEnabled() {
        if (view == null || !"none".equals(view.optString("local_mode", "none"))) return false;
        int scheme = view.optInt("scheme", 0);
        return scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME;
    }

    private void insertCandidateGloss(int slot, JSONObject id, String text, String gloss) {
        if (!candidateIsCurrent(slot, id, text) || connection == null) return;
        if (!commitText(gloss, TypingSource.LOCAL)) return;
        if (session != 0) command(3);
    }

    private void insertExpandedCandidateGloss(JSONObject candidate, JSONObject id,
                                              String text, String gloss) {
        if (!expandedCandidateIsCurrent(candidate, id, text) || connection == null) return;
        if (!commitText(gloss, TypingSource.LOCAL)) return;
        if (session != 0) command(3);
    }

    private boolean expandedCandidateIsCurrent(JSONObject candidate, JSONObject id, String text) {
        if (!candidatePanelOpen || candidatePanelSnapshot == null || view == null || candidate == null
                || id == null || candidatePanelSnapshot.optLong("session") != session
                || candidatePanelSnapshot.optLong("session") != view.optLong("session")
                || candidatePanelSnapshot.optLong("generation") != view.optLong("generation"))
            return false;
        JSONObject candidateId = candidate.optJSONObject("id");
        return candidateId != null
            && candidateId.optLong("session") == id.optLong("session")
            && candidateId.optLong("generation") == id.optLong("generation")
            && candidateId.optLong("index") == id.optLong("index")
            && text.equals(chineseOutput(candidate.optString("text"), view));
    }

    private boolean showCandidateMenu(Button button, int slot, JSONObject id, String text) {
        if (!candidateGlossInsertionEnabled() && !candidateManagementEnabled()) return false;
        return showCandidateMenu(button, slot, id, text, visibleCandidate(slot), false);
    }

    private boolean showCandidateMenu(Button button, int slot, JSONObject id, String text,
                                      JSONObject candidate, boolean expanded) {
        if (!candidateGlossInsertionEnabled() && !candidateManagementEnabled()) return false;
        PopupMenu popup = new PopupMenu(this, button);
        String translation = candidate == null || candidate.isNull("translation")
            ? "" : candidate.optString("translation", "");
        java.util.List<String> glosses = candidateGlossInsertionEnabled()
            ? CandidateTranslationPolicy.insertionGlosses(translation) : java.util.List.of();
        for (int index = 0; index < glosses.size(); index++)
            popup.getMenu().add(Menu.NONE, 2000 + index, Menu.NONE, glosses.get(index));
        boolean management = candidateManagementEnabled();
        if (glosses.isEmpty() && !management) return false;
        if (management) {
            for (CandidateManagementAction action : CandidateManagementAction.values()) {
                popup.getMenu().add(Menu.NONE, action.menuItemId(), action.ordinal(), action.title());
            }
        }
        popup.setOnMenuItemClickListener(item -> {
            playFeedback(button);
            int glossIndex = item.getItemId() - 2000;
            if (glossIndex >= 0 && glossIndex < glosses.size()) {
                if (expanded) insertExpandedCandidateGloss(candidate, id, text, glosses.get(glossIndex));
                else insertCandidateGloss(slot, id, text, glosses.get(glossIndex));
                return true;
            }
            CandidateManagementAction action;
            try {
                action = CandidateManagementAction.fromMenuItemId(item.getItemId());
            } catch (IllegalArgumentException error) {
                return false;
            }
            if (action.confirmationRequired()) {
                confirmCandidateRemoval(id, text);
                return true;
            }
            editCandidate(id, action);
            return true;
        });
        popup.show();
        return true;
    }

    /** {@code ownRow} starts the annotation on its own row under the candidate, as Korean Hanja rows draw their 훈음; otherwise it follows the candidate inline. */
    private CharSequence candidateLabel(String prefix, String text, String annotation,
                                        boolean highlighted, boolean ownRow) {
        if (annotation.isEmpty() && prefix.isEmpty()) return text;
        String primary = prefix + text;
        SpannableString label = new SpannableString(primary + (ownRow ? "\n" : " ") + annotation);
        if (!prefix.isEmpty()) {
            label.setSpan(new ForegroundColorSpan(candidateAppearance.number()), 0, prefix.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        if (annotation.isEmpty()) return label;
        int annotationStart = primary.length() + 1;
        label.setSpan(new RelativeSizeSpan(0.72f), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        int foreground = candidateAppearance.textFor(highlighted);
        int secondary = Color.argb(Math.round(Color.alpha(foreground) * 0.58f),
            Color.red(foreground), Color.green(foreground), Color.blue(foreground));
        label.setSpan(new ForegroundColorSpan(secondary), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return label;
    }

    /** Keep a candidate word intact; the surrounding strip/panel owns scrolling and wrapping. */
    private void configureCandidateTextLayout(Button button, int lines) {
        // Reserve extra rows only when this candidate actually carries a gloss. A globally enabled translation target is not evidence that every candidate has one; keeping the empty case single-line prevents long candidate words from wrapping inside the chip. Likewise, a second requested language may be unavailable for this particular result; only an actual newline in the rendered label (a second gloss, or a Korean 훈음 row under its Hanja) warrants another row.
        // A multi-row label deliberately occupies those rows. Do not turn the whole label into a single-line TextView in that case, or the rows after the first are silently clipped. With no extra row the chip can scroll horizontally as one intact candidate word.
        button.setSingleLine(lines == 1);
        button.setEllipsize(null);
        button.setHorizontallyScrolling(lines == 1);
    }

    /** Rows of one rendered candidate label; see {@link #candidateLabel}. */
    private static int candidateLabelLines(String annotation, boolean ownRow) {
        return ownRow ? CandidateTranslationPolicy.renderedOwnRowLines(annotation)
            : CandidateTranslationPolicy.renderedGlossLines(annotation);
    }

    /** Whether the candidates on the strip are the Hanja of a composing Korean syllable, whose annotation is the 훈음 drawn on its own row. */
    private boolean koreanHanjaRows() {
        return koreanSchemeActive() && "none".equals(view.optString("local_mode", "none"));
    }

    private String candidateAnnotation(JSONObject candidate) {
        if (koreanHanjaRows())
            return CandidateGlossPolicy.hanjaAnnotation(candidate.optString("annotation", ""),
                candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
                candidateEnglishGloss || candidateTranslationsEnabled);
        return CandidateGlossPolicy.annotation(candidate.optString("annotation", ""),
            candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    private String wubiCodeHint(JSONObject candidate, JSONObject context, String typed) {
        return WubiCodeHintPolicy.hint(candidate.optString("code", ""), typed, wubiCodeHint,
            context == null ? -1 : context.optInt("scheme", -1),
            context == null ? "none" : context.optString("local_mode", "none"),
            context != null && context.optBoolean("answered_by_pinyin_fallback", false));
    }

    private String candidateAnnotation(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        return hint.isEmpty() ? candidateAnnotation(candidate) : hint;
    }

    private String candidateAccessibilitySuffix(JSONObject candidate) {
        if (koreanHanjaRows())
            return CandidateGlossPolicy.hanjaAccessibilitySuffix(candidate.optString("annotation", ""),
                candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
                candidateEnglishGloss || candidateTranslationsEnabled);
        return CandidateGlossPolicy.accessibilitySuffix(candidate.optString("annotation", ""),
            candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    private String candidateAccessibilitySuffix(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        return hint.isEmpty() ? candidateAccessibilitySuffix(candidate) : "，还需输入 " + hint;
    }

    private Button makeCandidateButton(int slot) {
        // Apple uses the same press-feedback button for candidate chips as for keys. Android's
        // HorizontalScrollView cancels the child on a drag, so the button keeps immediate tap
        // feedback without changing the existing scroll-versus-select boundary.
        Button button = new KeyboardPressButton(this);
        button.setAllCaps(false);
        button.setOnClickListener(ignored -> selectVisibleCandidate(button, slot));
        button.setOnLongClickListener(ignored -> {
            JSONObject current = visibleCandidate(slot);
            JSONObject id = current == null ? null : current.optJSONObject("id");
            if (id == null || (!candidateManagementEnabled() && !candidateGlossInsertionEnabled())) return false;
            String text = chineseOutput(current.optString("text"), view);
            return showCandidateMenu(button, slot, id, text);
        });
        return button;
    }

    /** The shared `navigation` switches, with the shared defaults for anything absent. */
    private static CandidateNavigationPolicy.Bindings candidateNavigationFrom(JSONObject navigation) {
        CandidateNavigationPolicy.Bindings defaults = CandidateNavigationPolicy.Bindings.defaults();
        if (navigation == null) return defaults;
        return new CandidateNavigationPolicy.Bindings(
            navigation.optBoolean("minus_equal", defaults.minusEqual()),
            navigation.optBoolean("comma_period", defaults.commaPeriod()),
            navigation.optBoolean("brackets", defaults.brackets()),
            navigation.optBoolean("tab", defaults.tab()),
            navigation.optBoolean("page_up_down", defaults.pageUpDown()),
            navigation.optBoolean("arrows", defaults.arrows()));
    }

    private static String wordCharacterBindingFrom(JSONObject preferences) {
        JSONObject wordCharacter = preferences == null ? null
            : preferences.optJSONObject("word_character");
        if (wordCharacter == null) return WordCharacterPolicy.DISABLED;
        return WordCharacterPolicy.binding(wordCharacter.optBoolean("enabled", true),
            wordCharacter.optString("keys", WordCharacterPolicy.BRACKETS));
    }

    /** The candidate the strip is showing as highlighted, which 以词定字 takes its character from. */
    private JSONObject highlightedCandidate() {
        JSONArray entries = view == null ? null : view.optJSONArray("candidates");
        if (entries == null) return null;
        for (int index = 0; index < entries.length(); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null && candidate.optBoolean("highlighted")) return candidate;
        }
        return null;
    }

    /**
     * 以词定字: commit one Han character from the highlighted candidate.
     *
     * <p>Two steps, because the Engine only owns the first. It takes a candidate that has Han text,
     * commits the requested end and clears the composition. A candidate with none — the English
     * word offered for the same spelling — it refuses and leaves the composition alone, and the
     * host resolves that case itself: extract the character from the candidate's own text, or
     * commit the whole candidate when it has no Han character at all. The text used is the one the
     * strip is showing, so a user reading 繁体 candidates gets the 繁体 character.
     *
     * <p>Returns false when there is nothing to act on, which lets the key type its own symbol.
     */
    private boolean selectCandidateEdge(WordCharacterPolicy.Edge edge) {
        JSONObject candidate = highlightedCandidate();
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null || session == 0 || id.optLong("session") != session) return false;
        String displayed = chineseOutput(candidate.optString("text"), view);
        try {
            candidatePanelOpen = false;
            if (apply(NativeClient.selectEdge(session, id.getLong("generation"),
                                              id.getLong("index"), edge.code()))) return true;
            // Declined: the Engine kept the composition, so end it here before the host commits.
            String fallback = CandidateTextPolicy.fallbackCommit(displayed, edge);
            if (fallback == null || fallback.isEmpty()) return false;
            command(3);
            commitText(fallback);
            render();
            return true;
        } catch (JSONException | LinkageError error) {
            fail();
            return true;
        }
    }

    private JSONObject visibleCandidate(int slot) {
        if (view == null || slot < 0) return null;
        JSONArray entries = view.optJSONArray("candidates");
        return entries == null ? null : entries.optJSONObject(slot);
    }

    private void selectVisibleCandidate(Button button, int slot) {
        JSONObject candidate = visibleCandidate(slot);
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null) return;
        playFeedback(button);
        if (session == 0 || id.optLong("session") != session) return;
        candidatePanelOpen = false;
        try {
            apply(NativeClient.select(session, id.getLong("generation"), id.getLong("index")));
        } catch (JSONException | LinkageError error) { fail(); }
    }

    private boolean selectHardwareCandidate(int slot) {
        JSONObject candidate = visibleCandidate(slot);
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null || session == 0 || id.optLong("session") != session) return false;
        candidatePanelOpen = false;
        try {
            apply(NativeClient.select(session, id.getLong("generation"), id.getLong("index")));
            return true;
        } catch (JSONException | LinkageError error) {
            fail();
            return true;
        }
    }

    private void updateCandidateButton(Button button, JSONObject candidate, int slot) {
        String text = chineseOutput(candidate.optString("text"), view);
        boolean highlighted = candidate.optBoolean("highlighted");
        String typed = view == null ? "" : view.optString("preedit", "");
        String annotation = candidateAnnotation(candidate, typed);
        // Touch candidates follow Apple's chip surface: the word itself is shown without a
        // numeric prefix. The slot remains available through contentDescription and the shared
        // session/generation/index identity for accessibility and hardware number-row selection.
        boolean ownRow = koreanHanjaRows();
        button.setText(candidateLabel("", text, annotation, highlighted, ownRow));
        int labelLines = candidateLabelLines(annotation, ownRow);
        button.setMinLines(labelLines);
        button.setMaxLines(labelLines);
        configureCandidateTextLayout(button, labelLines);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidateFontSize);
        button.setSelected(highlighted);
        styleCandidateButton(button);
        String description = "候选 " + (slot + 1) + "：" + text
            + candidateAccessibilitySuffix(candidate, typed);
        JSONObject id = candidate.optJSONObject("id");
        button.setContentDescription(id != null && candidateManagementEnabled()
            ? description + "；长按管理" : description);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        button.setEnabled(id != null);
    }

    private Button expandedCandidateButton(JSONObject candidate) {
        JSONObject id = candidate.optJSONObject("id");
        Button button = new KeyboardPressButton(this);
        String text = chineseOutput(candidate.optString("text"), view);
        boolean highlighted = candidate.optBoolean("highlighted");
        String typed = candidatePanelSnapshot == null ? ""
            : candidatePanelSnapshot.optString("preedit", "");
        String annotation = candidateAnnotation(candidate, typed);
        button.setAllCaps(false);
        boolean ownRow = koreanHanjaRows();
        button.setText(candidateLabel("", text, annotation, highlighted, ownRow));
        int labelLines = candidateLabelLines(annotation, ownRow);
        button.setMinLines(labelLines);
        button.setMaxLines(labelLines);
        configureCandidateTextLayout(button, labelLines);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidateFontSize);
        button.setSelected(highlighted);
        styleCandidateButton(button);
        long index = id == null ? -1 : id.optLong("index", -1);
        button.setContentDescription(index < 0 ? "候选" : "候选 " + (index + 1) + "："
            + text + candidateAccessibilitySuffix(candidate, typed));
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        if (id == null || index < 0) {
            button.setEnabled(false);
        } else {
            button.setOnClickListener(ignored -> {
                playFeedback(button);
                if (session == 0 || id.optLong("session") != session) return;
                candidatePanelOpen = false;
                candidatePanelSnapshot = null;
                try {
                    apply(NativeClient.selectAnyCandidate(
                        session, id.getLong("generation"), id.getLong("index")));
                } catch (JSONException | LinkageError error) { fail(); }
            });
            button.setOnLongClickListener(ignored -> {
                if (!candidateManagementEnabled() && !candidateGlossInsertionEnabled()) return false;
                return showCandidateMenu(button, -1, id, text, candidate, true);
            });
        }
        return button;
    }

    private void closeCandidatePanel() {
        candidatePanelOpen = false;
        candidatePanelSnapshot = null;
        if (keyboardRoot != null && expandedCandidates != null) {
            expandedCandidates.setVisibility(View.GONE);
            if (expandedCandidateScroll != null)
                expandedCandidateScroll.setVisibility(View.GONE);
        }
    }

    private void openCandidatePanel() {
        if (session == 0 || view == null || view.optInt("page_count", 0) <= 1) return;
        try {
            JSONObject snapshot = value(NativeClient.allCandidates(session));
            if (snapshot.optLong("session") != view.optLong("session")
                    || snapshot.optLong("generation") != view.optLong("generation")) return;
            JSONArray entries = snapshot.optJSONArray("candidates");
            JSONArray visible = view.optJSONArray("candidates");
            if (entries == null || visible == null || entries.length() <= visible.length()) return;
            candidatePanelSnapshot = snapshot;
            candidatePanelOpen = true;
            renderExpandedCandidates();
            applySkin();
        } catch (JSONException | LinkageError error) { fail(); }
    }

    private void renderExpandedCandidates() {
        if (expandedCandidates == null || expandedCandidateScroll == null) return;
        expandedCandidates.removeAllViews();
        if (!candidatePanelOpen || view == null || candidatePanelSnapshot == null
                || candidatePanelSnapshot.optLong("session") != view.optLong("session")
                || candidatePanelSnapshot.optLong("generation") != view.optLong("generation")) {
            candidatePanelOpen = false;
            candidatePanelSnapshot = null;
            expandedCandidates.setVisibility(View.GONE);
            expandedCandidateScroll.setVisibility(View.GONE);
            return;
        }
        expandedCandidateScroll.setVisibility(View.VISIBLE);
        expandedCandidates.setVisibility(View.VISIBLE);
        LinearLayout header = new LinearLayout(this);
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView composition = new TextView(this);
        String reading = candidatePanelSnapshot.optString("reading", "");
        String compositionText = reading.isEmpty()
            ? candidatePanelSnapshot.optString("preedit", "") : reading;
        composition.setText(compositionText);
        composition.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidatePreeditFontSize);
        composition.setContentDescription("当前组合文本：" + compositionText);
        header.addView(composition, new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        JSONArray entries = candidatePanelSnapshot.optJSONArray("candidates");
        int count = entries == null ? 0 : entries.length();
        TextView countView = new TextView(this);
        countView.setText(count + " 个候选");
        countView.setContentDescription(count + " 个候选");
        header.addView(countView, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        Button close = new Button(this);
        close.setAllCaps(false);
        close.setText("收起");
        close.setContentDescription("收起候选面板");
        close.setOnClickListener(ignored -> {
            playFeedback(close);
            closeCandidatePanel();
        });
        header.addView(close, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT));
        expandedCandidates.addView(header);
        CandidateWrapLayout list = new CandidateWrapLayout(this, pixels(8));
        list.setPadding(0, pixels(8), 0, 0);
        list.setContentDescription("完整候选列表");
        if (entries != null) {
            for (int index = 0; index < entries.length(); index++) {
                JSONObject candidate = entries.optJSONObject(index);
                if (candidate == null) continue;
                Button button = expandedCandidateButton(candidate);
                list.addView(button, new android.view.ViewGroup.LayoutParams(
                    android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
                    android.view.ViewGroup.LayoutParams.WRAP_CONTENT));
            }
        }
        expandedCandidates.addView(list);
    }

    private boolean handwritingActive() {
        String localMode = view == null ? "none" : view.optString("local_mode", "none");
        return session != 0 && !dedicatedEnglish
            && "none".equals(localMode)
            && keyboardLayer == KeyboardLayout.Layer.LETTERS
            && displayedTouchLayout(view) == HANDWRITING_LAYOUT && handwritingCanvas != null;
    }

    private void deactivateHandwriting() {
        if (handwritingRecognitionTask != null) main.removeCallbacks(handwritingRecognitionTask);
        if (handwritingAvailabilityTask != null) main.removeCallbacks(handwritingAvailabilityTask);
        handwritingRecognitionTask = null;
        handwritingAvailabilityTask = null;
        handwritingRequests.invalidate();
        if (handwritingRecognizer != null) {
            try { handwritingRecognizer.cancelPending(); } catch (RuntimeException ignored) { }
            try { handwritingRecognizer.close(); } catch (RuntimeException ignored) { }
        }
        handwritingRecognizer = null;
        handwritingCanvas = null;
        handwritingCandidates = null;
        handwritingStatus = null;
        handwritingDownload = null;
        handwritingDownloading = false;
        handwritingResults = java.util.List.of();
        handwritingCandidateToken = null;
    }

    private void showHandwritingStatus(String text) {
        if (handwritingStatus == null) return;
        handwritingStatus.setText(text);
        handwritingStatus.setVisibility(text == null || text.isEmpty() ? View.GONE : View.VISIBLE);
        if (handwritingStatus.getLayoutParams() instanceof FrameLayout.LayoutParams params) {
            boolean downloadVisible = handwritingDownload != null
                && handwritingDownload.getVisibility() == View.VISIBLE;
            params.gravity = downloadVisible
                ? Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL : Gravity.CENTER;
            params.bottomMargin = downloadVisible ? pixels(8) : 0;
            handwritingStatus.setLayoutParams(params);
        }
        if (candidates != null) render();
    }

    private void refreshHandwritingAvailability() {
        if (!handwritingActive() || handwritingRecognizer == null || handwritingDownload == null) return;
        if (handwritingAvailabilityTask != null) {
            main.removeCallbacks(handwritingAvailabilityTask);
            handwritingAvailabilityTask = null;
        }
        HandwritingRecognizer.Availability availability = handwritingRecognizer.availability();
        switch (availability) {
            case UNAVAILABLE -> {
                handwritingCanvas.setAcceptsInk(false);
                handwritingDownload.setVisibility(View.GONE);
                showHandwritingStatus("此构建不含手写识别");
            }
            case DOWNLOAD_REQUIRED -> {
                handwritingCanvas.setAcceptsInk(false);
                handwritingDownload.setText("下载中文手写模型");
                handwritingDownload.setContentDescription("下载中文手写模型；完成后可离线识别");
                handwritingDownload.setEnabled(true);
                handwritingDownload.setVisibility(View.VISIBLE);
                if (!handwritingDownloading) showHandwritingStatus("首次下载后可离线手写");
            }
            case DOWNLOADING -> {
                handwritingCanvas.setAcceptsInk(false);
                handwritingDownload.setText(handwritingDownloading ? "正在下载…" : "正在检查模型…");
                handwritingDownload.setEnabled(false);
                handwritingDownload.setVisibility(View.VISIBLE);
                showHandwritingStatus(handwritingDownloading
                    ? "正在下载中文手写模型…" : "正在检查中文手写模型…");
                HandwritingRecognizer expected = handwritingRecognizer;
                handwritingAvailabilityTask = () -> {
                    if (handwritingRecognizer == expected) refreshHandwritingAvailability();
                };
                main.postDelayed(handwritingAvailabilityTask, 250);
            }
            case READY -> {
                handwritingDownloading = false;
                handwritingCanvas.setAcceptsInk(true);
                handwritingDownload.setVisibility(View.GONE);
                if (!handwritingCanvas.hasInk() && handwritingResults.isEmpty()) {
                    showHandwritingStatus("在此手写，停笔后选字");
                }
            }
        }
    }

    private void downloadHandwritingModel() {
        if (!handwritingActive() || handwritingRecognizer == null
                || handwritingRecognizer.availability() != HandwritingRecognizer.Availability.DOWNLOAD_REQUIRED) {
            return;
        }
        HandwritingRecognizer expected = handwritingRecognizer;
        handwritingDownloading = true;
        try {
            handwritingRecognizer.download(new HandwritingRecognizer.DownloadListener() {
                @Override public void onProgress(int percent) {
                    main.post(() -> {
                        if (handwritingRecognizer != expected || !handwritingActive()) return;
                        showHandwritingStatus(percent > 0
                            ? "模型下载中 " + percent + "%" : "正在连接模型服务…");
                    });
                }

                @Override public void onComplete() {
                    main.post(() -> {
                        if (handwritingRecognizer != expected || !handwritingActive()) return;
                        handwritingDownloading = false;
                        refreshHandwritingAvailability();
                    });
                }

                @Override public void onFailure() {
                    main.post(() -> {
                        if (handwritingRecognizer != expected || !handwritingActive()) return;
                        handwritingDownloading = false;
                        refreshHandwritingAvailability();
                        showHandwritingStatus("下载失败，请检查网络后重试");
                    });
                }
            });
        } catch (RuntimeException error) {
            handwritingDownloading = false;
            showHandwritingStatus("下载失败，请检查网络后重试");
        }
        refreshHandwritingAvailability();
    }

    private void invalidateHandwritingRecognition() {
        if (handwritingRecognitionTask != null) main.removeCallbacks(handwritingRecognitionTask);
        handwritingRecognitionTask = null;
        handwritingRequests.invalidate();
        handwritingResults = java.util.List.of();
        handwritingCandidateToken = null;
        if (handwritingRecognizer != null) {
            try { handwritingRecognizer.cancelPending(); } catch (RuntimeException ignored) { }
        }
    }

    private void handwritingInkChanged(long revision,
                                       java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
        invalidateHandwritingRecognition();
        if (!handwritingActive() || handwritingCanvas == null) return;
        if (strokes.isEmpty()) {
            showHandwritingStatus("在此手写，停笔后选字");
            return;
        }
        if (handwritingRecognizer == null
                || handwritingRecognizer.availability() != HandwritingRecognizer.Availability.READY) {
            refreshHandwritingAvailability();
            return;
        }
        showHandwritingStatus("停笔后识别…");
        HandwritingRequestTracker.Token token = handwritingRequests.begin(session, revision);
        handwritingRecognitionTask = () -> recognizeHandwriting(token, strokes);
        main.postDelayed(handwritingRecognitionTask, HANDWRITING_DEBOUNCE_MILLIS);
    }

    private boolean acceptsHandwriting(HandwritingRequestTracker.Token token) {
        return handwritingCanvas != null && handwritingRequests.accepts(token, session,
            handwritingCanvas.revision(), handwritingActive());
    }

    private void recognizeHandwriting(HandwritingRequestTracker.Token token,
                                      java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
        handwritingRecognitionTask = null;
        if (!acceptsHandwriting(token) || handwritingRecognizer == null
                || handwritingRecognizer.availability() != HandwritingRecognizer.Availability.READY) {
            return;
        }
        final HandwritingRecognizer expected = handwritingRecognizer;
        final HandwritingRecognizer.Request request;
        try {
            request = new HandwritingRecognizer.Request(token.revision(), strokes,
                handwritingCanvas.getWidth(), handwritingCanvas.getHeight());
        } catch (IllegalArgumentException error) {
            showHandwritingStatus("书写区域不可用，请重试");
            return;
        }
        showHandwritingStatus("正在识别…");
        try {
            handwritingRecognizer.recognize(request, new HandwritingRecognizer.RecognitionListener() {
                @Override public void onResult(long revision, java.util.List<String> values) {
                    main.post(() -> {
                        if (handwritingRecognizer != expected || revision != token.revision()
                                || !acceptsHandwriting(token)) return;
                        handwritingResults = HandwritingRecognizer.sanitizeCandidates(values);
                        handwritingCandidateToken = handwritingResults.isEmpty() ? null : token;
                        renderHandwritingCandidates(token);
                    });
                }

                @Override public void onFailure(long revision) {
                    main.post(() -> {
                        if (handwritingRecognizer != expected || revision != token.revision()
                                || !acceptsHandwriting(token)) return;
                        handwritingResults = java.util.List.of();
                        handwritingCandidateToken = null;
                        showHandwritingStatus("识别失败，请撤销或重新书写");
                    });
                }
            });
        } catch (RuntimeException error) {
            if (acceptsHandwriting(token)) showHandwritingStatus("识别失败，请撤销或重新书写");
        }
    }

    private void renderHandwritingCandidates(HandwritingRequestTracker.Token token) {
        render();
    }

    private void clearHandwriting() {
        invalidateHandwritingRecognition();
        if (handwritingCanvas != null) handwritingCanvas.clear();
        showHandwritingStatus("在此手写，停笔后选字");
    }

    private void deleteFromHandwriting() {
        if (handwritingCanvas != null && handwritingCanvas.hasInk()) {
            handwritingCanvas.undo();
        } else if (directEnglishActive() && connection != null) {
            clearEnglishSuggestions();
            connection.deleteSurroundingTextInCodePoints(1, 0);
            refreshEnglishSuggestions();
        } else if (connection != null && !command(0)) {
            connection.deleteSurroundingTextInCodePoints(1, 0);
        }
    }

    private boolean commitFirstHandwritingCandidate() {
        if (!handwritingActive() || handwritingCanvas == null || !handwritingCanvas.hasInk()
                || handwritingCandidateToken == null || handwritingResults.isEmpty()) return false;
        return commitHandwritingCandidate(handwritingCandidateToken, handwritingResults.get(0));
    }

    private boolean commitHandwritingCandidate(HandwritingRequestTracker.Token token, String candidate) {
        if (!acceptsHandwriting(token) || !handwritingResults.contains(candidate)
                || connection == null) return false;
        long targetSession = session;
        command(2);
        if (targetSession != session || !acceptsHandwriting(token) || connection == null) return false;
        if (!commitText(chineseOutput(candidate, view), TypingSource.HANDWRITING)) return false;
        clearHandwriting();
        return true;
    }

    private void rebuildHandwritingRows() {
        handwritingStatus = new TextView(this);
        handwritingStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        handwritingStatus.setGravity(Gravity.CENTER);
        handwritingStatus.setText("在此手写，停笔后选字");
        handwritingStatus.setContentDescription("手写状态");
        handwritingStatus.setClickable(false);
        handwritingStatus.setFocusable(false);

        FrameLayout row = new FrameLayout(this);
        handwritingCanvas = new HandwritingCanvas(this);
        handwritingCanvas.applySkin(skin);
        row.addView(handwritingCanvas, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));

        FrameLayout cardFrame = new FrameLayout(this);
        cardFrame.setClickable(false);
        cardFrame.setFocusable(false);
        FrameLayout.LayoutParams cardParams = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT);
        cardParams.rightMargin = pixels(64);
        row.addView(cardFrame, cardParams);
        cardFrame.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> handwritingCanvas.setCardRect(
                    left, top, right, bottom));
        cardFrame.addView(handwritingStatus, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT,
            Gravity.CENTER));

        handwritingDownload = new Button(this);
        handwritingDownload.setAllCaps(false);
        handwritingDownload.setOnClickListener(ignored -> {
            playFeedback(handwritingDownload);
            downloadHandwritingModel();
        });
        FrameLayout.LayoutParams downloadParams = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, pixels(48));
        downloadParams.gravity = Gravity.CENTER;
        downloadParams.leftMargin = pixels(16);
        downloadParams.rightMargin = pixels(16);
        cardFrame.addView(handwritingDownload, downloadParams);

        LinearLayout tools = new LinearLayout(this);
        tools.setOrientation(LinearLayout.VERTICAL);
        addNineKey(tools, keyboardKey("撤销", "撤销最后一笔", () -> {
            if (handwritingCanvas != null) handwritingCanvas.undo();
        }));
        addNineKey(tools, keyboardKey("清空", "清空手写", this::clearHandwriting));
        addNineKey(tools, keyId(keyboardKey("⌫", "删除", this::deleteFromHandwriting), "Backspace"));
        row.addView(tools, new FrameLayout.LayoutParams(pixels(64),
            FrameLayout.LayoutParams.MATCH_PARENT, Gravity.END));
        adjustFixedHeight(row, KeyboardGeometry.HANDWRITING_BODY_HEIGHT_DP);
        keyRows.addView(row);

        handwritingRecognizer = HandwritingRecognizerFactory.create(this);
        handwritingCanvas.setListener(new HandwritingCanvas.Listener() {
            @Override public void onStrokeBegan() {
                invalidateHandwritingRecognition();
                showHandwritingStatus("书写中…");
            }

            @Override public void onInkChanged(long revision,
                    java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
                handwritingInkChanged(revision, strokes);
            }
        });
        refreshHandwritingAvailability();
    }

    private void rebuildKeyRows() {
        if (keyRows == null) return;
        hideJapaneseFlickPreview();
        deactivateHandwriting();
        symbolKeyButtons.clear();
        symbolKeyInputs.clear();
        shuangpinKeyButtons.clear();
        shuangpinKeyInputs.clear();
        microsoftFinalKey = null;
        nineKeySidebar = null;
        japaneseSpaceKey = null;
        japaneseReturnKey = null;
        japaneseSymbolsKey = null;
        japaneseVariantsButton = null;
        keyRows.removeAllViews();
        if (displayedTouchLayout(view) == JAPANESE_NINE_KEY_LAYOUT) {
            rebuildJapaneseNineKeyRows();
            applyKeyboardGeometry();
            return;
        }
        // 九键切数字仍然是九键。The digit layer keeps the grid the user picked three columns for;
        // only handwriting hands its panel over to the 26-key symbol rows.
        if (displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT) {
            rebuildNineKeyRows();
            applyKeyboardGeometry();
            return;
        }
        if (keyboardLayer == KeyboardLayout.Layer.LETTERS
            && displayedTouchLayout(view) == HANDWRITING_LAYOUT) {
            rebuildHandwritingRows();
            applyKeyboardGeometry();
            return;
        }
        boolean chineseMode = !dedicatedEnglish;
        boolean localMode = view != null
            && !"none".equals(view.optString("local_mode", "none"));
        boolean shifted = letterCase.usesUppercase();
        boolean koreanKeycaps = keyboardLayer == KeyboardLayout.Layer.LETTERS
            && displayedTouchLayout(view) == KeyboardLayout.KOREAN_LAYOUT;
        // The face is the policy's job; the key itself always sends its canonical lowercase form.
        java.util.List<java.util.List<String>> rows = KeyboardLayout.rows(keyboardLayer);
        for (int rowIndex = 0; rowIndex < rows.size(); rowIndex++) {
            java.util.List<String> keys = rows.get(rowIndex);
            LinearLayout row = new LinearLayout(this);
            row.setTag(new KeyboardHeightRole(KeyboardGeometry.STANDARD_ROW_HEIGHT_DP,
                rows.size(), rowIndex, true));
            keyRows.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            for (String key : keys) {
                final String input = key;
                String face = keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                    ? ChineseSymbolFaces.face(key, sendsChinesePunctuation())
                    : koreanKeycaps ? KoreanKeyboardLayout.face(key, shifted)
                    : LetterKeyFacePolicy.face(key, chineseMode, localMode, shifted);
                Button keyButton;
                if (koreanKeycaps) {
                    // Jamo keycaps over the same QWERTY letters: type() sends the letter, and the Engine composes the syllable.
                    keyButton = keyboardKey(face, face, () -> type(input.charAt(0)));
                    keyButton.setContentDescription(
                        KoreanKeyboardLayout.accessibilityLabel(key, shifted));
                    if (keyButton instanceof KeyboardPressButton press)
                        press.setKeyboardRole(KeyboardKeyRole.KEY);
                } else if (keyboardLayer == KeyboardLayout.Layer.LETTERS) {
                    ShuangpinHintButton hintButton = shuangpinKeyboardKey(
                        face, face, () -> type(input.charAt(0)));
                    keyButton = hintButton;
                    shuangpinKeyButtons.add(hintButton);
                    shuangpinKeyInputs.add(input);
                } else {
                    keyButton = keyboardKey(face, face, () -> type(input.charAt(0)));
                }
                keyId(keyButton, KeyPressIds.forCharacter(input.charAt(0)));
                if (keyboardLayer == KeyboardLayout.Layer.LETTERS && !koreanKeycaps) {
                    keyButton.setContentDescription(LetterKeyFacePolicy.accessibilityLabel(
                        input, chineseMode, localMode, shifted));
                    // 字母键读作「字母 Q」而不是「按键 Q」，所以描述推导一直把它判成 action 面，
                    // 26 键的字母因此是实心深绿的。角色说了算之后就不必靠描述去猜。
                    if (keyButton instanceof KeyboardPressButton press)
                        press.setKeyboardRole(KeyboardKeyRole.KEY);
                }
                if (keyboardLayer == KeyboardLayout.Layer.SYMBOLS) {
                    symbolKeyButtons.add(keyButton);
                    symbolKeyInputs.add(input);
                }
                row.addView(keyButton, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1));
            }
            if (keyboardLayer == KeyboardLayout.Layer.LETTERS && rowIndex == 1) {
                microsoftFinalKey = keyId(shuangpinKeyboardKey(";", "微软双拼 ing", () -> type(';')),
                    "Semicolon");
                shuangpinKeyButtons.add((ShuangpinHintButton) microsoftFinalKey);
                shuangpinKeyInputs.add(";");
                row.addView(microsoftFinalKey, new LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1));
            }
            // 大小写和删除属于最后一行的两端，不属于底部功能行。Leaving them in a strip below the keys
            // is what pushed every other control out of reach of a thumb.
            if (rowIndex == rows.size() - 1) {
                int layout = displayedTouchLayout(view);
                boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
                float edge = KeyboardActionRow.letterRowEdgeWeight(symbols);
                if (KeyboardActionRow.rowsCarryCase(layout, symbols))
                    addLetterRowEdgeKey(row, shiftButton, 0, edge);
                if (KeyboardActionRow.rowsCarryDelete(layout, symbols))
                    addLetterRowEdgeKey(row, deleteButton, row.getChildCount(), edge);
            }
        }
        applyKeyboardGeometry();
    }

    /** Re-parent a long-lived control into one end of the last letter row. */
    private void addLetterRowEdgeKey(LinearLayout row, Button key, int index, float weight) {
        if (key == null) return;
        if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
        // ⇧ and ⌫ are function keys: the design tints them like 123 and 中.
        if (key instanceof KeyboardPressButton press)
            press.setKeyboardRole(KeyboardKeyRole.ACCENT);
        key.setVisibility(View.VISIBLE);
        row.addView(key, index, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, weight));
    }

    private void addNineKey(LinearLayout parent, Button key) {
        boolean horizontal = parent.getOrientation() == LinearLayout.HORIZONTAL;
        parent.addView(key, new LinearLayout.LayoutParams(
            horizontal ? 0 : LinearLayout.LayoutParams.MATCH_PARENT,
            horizontal ? LinearLayout.LayoutParams.MATCH_PARENT : 0, 1));
    }

    /** The rail behind the capless punctuation column; it is not a Button, so the skin pass misses it. */
    private void applySidebarRail() {
        if (nineKeySidebar == null) return;
        GradientDrawable rail = new GradientDrawable();
        rail.setColor(Color.parseColor(skin.sidebarBackground()));
        rail.setCornerRadius(pixels(skin.cornerRadius()));
        nineKeySidebar.setBackground(rail);
    }

    private void rebuildNineKeyRows() {
        dismissNineKeyHoldOptions();
        LinearLayout container = new LinearLayout(this);
        container.setOrientation(LinearLayout.HORIZONTAL);
        adjustFixedHeight(container, KeyboardGeometry.NINE_KEY_HEIGHT_DP);
        keyRows.addView(container, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(180)));

        LinearLayout punctuation = new LinearLayout(this);
        punctuation.setOrientation(LinearLayout.VERTICAL);
        for (String symbol : NineKeyLayout.punctuation()) {
            Button key = keyId(keyboardKey(symbol, "符号 " + symbol,
                () -> commitNineKeyLiteral(symbol)), "SoftPunctuation");
            // The four punctuation keys share one rail rather than wearing four caps of their own.
            if (key instanceof KeyboardPressButton press)
                press.setKeyboardRole(KeyboardKeyRole.PLAIN);
            punctuation.addView(key, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        FrameLayout sidebar = new FrameLayout(this);
        nineKeySidebar = sidebar;
        applySidebarRail();
        sidebar.addView(punctuation, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        if (nineKeySpellingScroll != null) {
            sidebar.addView(nineKeySpellingScroll, new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        }
        container.addView(sidebar, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.7f));

        LinearLayout grid = new LinearLayout(this);
        grid.setOrientation(LinearLayout.VERTICAL);
        boolean digits = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        for (java.util.List<NineKeyLayout.Key> keys : NineKeyLayout.rows()) {
            LinearLayout row = new LinearLayout(this);
            for (NineKeyLayout.Key key : keys) {
                String description = NineKeyLayout.description(key, digits);
                // On the digit layer the grid is a numeric keypad, so a tap commits the number
                // instead of feeding it to the pinyin session.
                NineKeyDigitButton keyButton = nineKeyGridKey(
                    NineKeyLayout.face(key, digits), description,
                    digits ? () -> commitNineKeyLiteral(NineKeyLayout.digitInput(key))
                        : () -> character(key.input()));
                keyId(keyButton, KeyPressIds.forNineKeyDigit(key.digit()));
                // 字母键面上印着它送进引擎的数字；数字键面本身就是那个数字，不必再印一次。
                // 分词键送的是拼音分隔符而不是 1，所以它没有可印的数字。
                keyButton.setDigitText(digits || !Character.isDigit(key.input())
                    ? "" : NineKeyLayout.digitInput(key));
                if (!digits && Character.isDigit(key.input()) && key.label().length() > 1) {
                    keyButton.setContentDescription("按键 " + description + "；长按输入数字或字母");
                    keyButton.setOnLongClickListener(ignored -> {
                        // The hold is this cell's press; picking from the popup is not another key.
                        countKey(keyButton);
                        showNineKeyHoldOptions(keyButton, key);
                        return true;
                    });
                }
                addNineKey(row, keyButton);
            }
            grid.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        container.addView(grid, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 3));

        LinearLayout actions = new LinearLayout(this);
        actions.setOrientation(LinearLayout.VERTICAL);
        Runnable deleteAction = () -> {
            if (connection != null && !command(0)) connection.deleteSurroundingTextInCodePoints(1, 0);
        };
        Button delete = keyId(keyboardKey("⌫", "删除", deleteAction), "Backspace");
        bindBackspaceRepeat(delete, deleteAction);
        addNineKey(actions, delete);
        addNineKey(actions, keyId(keyboardKey(".", "句点", this::commitNineKeyPeriod), "Period"));
        addNineKey(actions, keyId(keyboardKey("0", "数字 0", () -> commitNineKeyLiteral("0")),
            "Nine0"));
        container.addView(actions, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.8f));
    }

    /** Show the digit and literal letters printed on a nine-key key, like Apple's hold popup. */
    private void showNineKeyHoldOptions(Button anchor, NineKeyLayout.Key key) {
        dismissNineKeyHoldOptions();
        if (keyboardRoot == null) return;
        playFeedback(anchor);
        LinearLayout options = new LinearLayout(this);
        options.setOrientation(LinearLayout.HORIZONTAL);
        int padding = pixels(5);
        options.setPadding(padding, padding, padding, padding);
        GradientDrawable surface = new GradientDrawable();
        surface.setColor(Color.parseColor(skin.background()));
        surface.setCornerRadius(pixels(10));
        surface.setStroke(Math.max(1, pixels(1)), Color.parseColor(skin.accent()));
        options.setBackground(surface);

        String letters = key.label().toLowerCase(java.util.Locale.ROOT);
        String[] choices = new String[letters.length() + 1];
        choices[0] = String.valueOf(key.input());
        for (int index = 0; index < letters.length(); index++)
            choices[index + 1] = String.valueOf(letters.charAt(index));
        for (String choice : choices) {
            Button option = keyboardKey(choice, "输入 " + choice,
                () -> commitNineKeyHoldOption(choice));
            option.setContentDescription("输入 " + choice);
            option.setPadding(0, 0, 0, 0);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                pixels(36), pixels(38));
            if (options.getChildCount() > 0) params.setMarginStart(pixels(2));
            options.addView(option, params);
        }

        options.measure(View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        final PopupWindow[] holder = new PopupWindow[1];
        PopupWindow popup = new PopupWindow(options, options.getMeasuredWidth(),
            options.getMeasuredHeight(), true);
        holder[0] = popup;
        popup.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
        popup.setOutsideTouchable(true);
        popup.setClippingEnabled(true);
        popup.setElevation(pixels(4));
        popup.setOnDismissListener(() -> {
            if (nineKeyHoldPopup == holder[0]) nineKeyHoldPopup = null;
        });
        nineKeyHoldPopup = popup;
        int xOffset = (anchor.getWidth() - options.getMeasuredWidth()) / 2;
        int yOffset = -anchor.getHeight() - options.getMeasuredHeight() - pixels(6);
        popup.showAsDropDown(anchor, xOffset, yOffset);
    }

    private void commitNineKeyHoldOption(String text) {
        dismissNineKeyHoldOptions();
        if (connection == null) return;
        command(2);
        commitText(fullWidthOutput(text));
    }

    private void dismissNineKeyHoldOptions() {
        if (nineKeyHoldPopup != null) {
            nineKeyHoldPopup.dismiss();
            nineKeyHoldPopup = null;
        }
    }

    private static String japaneseKeyLabel(JapaneseNineKeyLayout.Key key) {
        return key.kana().get(0) + "\n" + key.kana().subList(1, 5).stream()
            .filter(label -> !label.isEmpty()).collect(java.util.stream.Collectors.joining(" "));
    }

    private void inputJapaneseStroke(String input) {
        if (session == 0 || input.isEmpty()) return;
        for (int index = 0; index < input.length(); index++) character(input.charAt(index));
    }

    private void selectJapaneseKey(JapaneseNineKeyLayout.Key key, int direction) {
        if (direction < 0 || direction >= key.kana().size()) return;
        if (key.kana().get(direction).isEmpty()) return;
        String stroke = key.strokes().get(direction);
        if (stroke.isEmpty()) commitNineKeyLiteral(key.kana().get(direction));
        else inputJapaneseStroke(stroke);
    }

    private void showJapaneseFlickPreview(Button button, JapaneseNineKeyLayout.Key key, int direction) {
        if (japaneseFlickPreview != null && keyboardRoot != null)
            japaneseFlickPreview.show(button, key, direction, keyboardRoot);
    }

    private void hideJapaneseFlickPreview() {
        if (japaneseFlickPreview != null) japaneseFlickPreview.hide();
    }

    private void bindJapaneseFlick(Button button, JapaneseNineKeyLayout.Key key) {
        final float[] origin = new float[2];
        final int[] direction = new int[1];
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    direction[0] = 0;
                    button.setPressed(true);
                    showJapaneseFlickPreview(button, key, 0);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    direction[0] = JapaneseNineKeyLayout.direction(
                        event.getX() - origin[0], event.getY() - origin[1], pixels(12));
                    showJapaneseFlickPreview(button, key, direction[0]);
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.setPressed(false);
                    hideJapaneseFlickPreview();
                    if (direction[0] == 0) button.performClick();
                    else {
                        playFeedback(button);
                        countKey(button);
                        selectJapaneseKey(key, direction[0]);
                    }
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    button.setPressed(false);
                    hideJapaneseFlickPreview();
                    return true;
                }
                default -> {
                    return true;
                }
            }
        });
    }

    private Button japaneseKey(JapaneseNineKeyLayout.Key key) {
        String description = key.kana().stream().filter(label -> !label.isEmpty())
            .collect(java.util.stream.Collectors.joining("、"));
        Button button = keyboardKey(japaneseKeyLabel(key), description,
            () -> selectJapaneseKey(key, 0));
        button.setContentDescription("轻点输入" + key.kana().get(0)
            + "；左、上、右、下滑动选择其他假名");
        bindJapaneseFlick(button, key);
        return button;
    }

    private void showJapaneseBracketOptions(Button anchor) {
        PopupMenu popup = new PopupMenu(this, anchor);
        for (String bracket : JapaneseNineKeyLayout.digitBrackets()) {
            popup.getMenu().add(bracket).setOnMenuItemClickListener(ignored -> {
                playFeedback(anchor);
                commitNineKeyLiteral(bracket);
                return true;
            });
        }
        popup.show();
    }

    private Button japaneseVariantsKey() {
        boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
        Button variants = keyboardKey(symbols ? "（）" : "小゛゜",
            symbols ? "括号；长按选择其他括号" : "小假名、浊音和半浊音", () -> {});
        japaneseVariantsButton = variants;
        variants.setOnClickListener(ignored -> {
            playFeedback(variants);
            if (keyboardLayer == KeyboardLayout.Layer.SYMBOLS) showJapaneseBracketOptions(variants);
            else command(CYCLE_KANA_VARIANT_COMMAND);
        });
        return variants;
    }

    private void addJapaneseSideKey(LinearLayout column, Button button, float weight) {
        column.addView(button, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, weight));
    }

    private void rebuildJapaneseNineKeyRows() {
        LinearLayout container = new LinearLayout(this);
        container.setOrientation(LinearLayout.HORIZONTAL);
        adjustFixedHeight(container, KeyboardGeometry.NINE_KEY_HEIGHT_DP);
        keyRows.addView(container, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(180)));

        LinearLayout modeColumn = new LinearLayout(this);
        modeColumn.setOrientation(LinearLayout.VERTICAL);
        japaneseSymbolsKey = keyId(keyboardKey("123", "切换到数字和符号", () -> {
            keyboardLayer = keyboardLayer == KeyboardLayout.Layer.SYMBOLS
                ? KeyboardLayout.Layer.LETTERS : KeyboardLayout.Layer.SYMBOLS;
            rebuildKeyRows();
            render();
        }), "SoftLayer");
        addJapaneseSideKey(modeColumn, japaneseSymbolsKey, 1);
        addJapaneseSideKey(modeColumn, keyId(keyboardKey("☺", "打开表情浏览", this::showEmojiPicker),
            "SoftEmoji"), 1);
        Button language = keyId(keyboardKey("英", "切换到英文输入", this::toggleInputLanguage),
            "SoftLanguage");
        addJapaneseSideKey(modeColumn, language,
            shouldOfferSwitchingToNextInputMethod() ? 1 : 2);
        if (shouldOfferSwitchingToNextInputMethod()) {
            addJapaneseSideKey(modeColumn, keyId(keyboardKey("切换", "切换到下一个输入法",
                this::switchToNextInputMethodAfterCommit), "SoftGlobe"), 1);
        }
        container.addView(modeColumn, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.17f));

        LinearLayout grid = new LinearLayout(this);
        grid.setOrientation(LinearLayout.VERTICAL);
        java.util.List<JapaneseNineKeyLayout.Key> keys = keyboardLayer == KeyboardLayout.Layer.SYMBOLS
            ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys();
        for (int rowIndex = 0; rowIndex < 4; rowIndex++) {
            LinearLayout row = new LinearLayout(this);
            if (rowIndex < 3) {
                for (int column = 0; column < 3; column++) {
                    int index = rowIndex * 3 + column;
                    addNineKey(row, keyId(japaneseKey(keys.get(index)),
                        KeyPressIds.forJapaneseKeyIndex(index)));
                }
            } else {
                addNineKey(row, japaneseVariantsKey());
                addNineKey(row, keyId(japaneseKey(keys.get(9)), KeyPressIds.forJapaneseKeyIndex(9)));
                addNineKey(row, keyId(japaneseKey(keys.get(10)), KeyPressIds.forJapaneseKeyIndex(10)));
            }
            grid.addView(row, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        }
        container.addView(grid, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.64f));

        LinearLayout side = new LinearLayout(this);
        side.setOrientation(LinearLayout.VERTICAL);
        Runnable deleteAction = () -> {
            if (connection != null && !command(0)) connection.deleteSurroundingTextInCodePoints(1, 0);
        };
        Button delete = keyId(keyboardKey("⌫", "删除", deleteAction), "Backspace");
        bindBackspaceRepeat(delete, deleteAction);
        addJapaneseSideKey(side, delete, 1);
        japaneseSpaceKey = keyId(keyboardKey("空白", "空白；左右滑动移动光标", this::space), "Space");
        bindSpaceCursor(japaneseSpaceKey);
        addJapaneseSideKey(side, japaneseSpaceKey, 1);
        japaneseReturnKey = keyId(keyboardKey("改行", "改行", this::enter), "Enter");
        addJapaneseSideKey(side, japaneseReturnKey, 2);
        container.addView(side, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 0.19f));
        updateReturnKey();
    }

    /** Non-interactive overlay showing the five choices while a Japanese key is being flicked. */
    private final class JapaneseFlickPreview extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private static final int[] X_OFFSETS = {0, -1, 0, 1, 0};
        private static final int[] Y_OFFSETS = {0, 0, -1, 0, 1};
        private String[] labels = new String[5];
        private int selectedDirection;
        private float centerX;
        private float centerY;
        private float cellWidth;
        private float cellHeight;
        private float gap;

        JapaneseFlickPreview(android.content.Context context) {
            super(context);
            setVisibility(View.GONE);
            setClickable(false);
            setFocusable(false);
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        }

        void show(Button anchor, JapaneseNineKeyLayout.Key key, int direction, FrameLayout root) {
            labels = key.kana().toArray(String[]::new);
            selectedDirection = KeyboardGeometry.bounded(direction, 0, labels.length - 1);
            int[] rootLocation = new int[2];
            int[] anchorLocation = new int[2];
            root.getLocationOnScreen(rootLocation);
            anchor.getLocationOnScreen(anchorLocation);
            centerX = anchorLocation[0] - rootLocation[0] + anchor.getWidth() / 2f;
            centerY = anchorLocation[1] - rootLocation[1] + anchor.getHeight() / 2f;
            float density = getResources().getDisplayMetrics().density;
            cellWidth = Math.max(anchor.getWidth(), Math.round(40 * density));
            cellHeight = Math.max(anchor.getHeight(), Math.round(36 * density));
            gap = Math.round(6 * density);
            root.bringChildToFront(this);
            setVisibility(View.VISIBLE);
            invalidate();
        }

        void hide() { setVisibility(View.GONE); }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            float textSize = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, 22,
                getResources().getDisplayMetrics());
            float radius = 8 * getResources().getDisplayMetrics().density;
            paint.setTextSize(textSize);
            paint.setTypeface(skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            Paint.FontMetrics metrics = paint.getFontMetrics();
            float stepX = cellWidth + gap;
            float stepY = cellHeight + gap;
            for (int index = 0; index < labels.length; index++) {
                String label = labels[index];
                if (label == null || label.isEmpty()) continue;
                boolean selected = index == selectedDirection;
                float x = centerX + X_OFFSETS[index] * stepX - cellWidth / 2;
                float y = centerY + Y_OFFSETS[index] * stepY - cellHeight / 2;
                paint.setColor(Color.parseColor(selected ? skin.accent() : skin.keyBackground()));
                canvas.drawRoundRect(x, y, x + cellWidth, y + cellHeight,
                    radius, radius, paint);
                paint.setColor(Color.parseColor(
                    selected ? skin.actionForeground() : skin.keyForeground()));
                float baseline = y + (cellHeight - metrics.bottom - metrics.top) / 2
                    - metrics.top;
                float textWidth = paint.measureText(label);
                canvas.drawText(label, x + (cellWidth - textWidth) / 2, baseline, paint);
            }
        }

        @Override public boolean onTouchEvent(MotionEvent event) { return false; }
    }

    private void commitNineKeyLiteral(String text) {
        if (connection == null) return;
        command(2);
        commitText(fullWidthOutput(text));
    }

    private void commitNineKeyPeriod() {
        if (dedicatedEnglish || !punctuation('.')) commitNineKeyLiteral(".");
    }

    private void chooseNineKeySpelling(long generation, int index) {
        if (session == 0) return;
        try { apply(NativeClient.chooseNineKeySpelling(session, generation, index)); }
        catch (JSONException | LinkageError error) { fail(); }
    }

    private void renderNineKeySpellings() {
        if (nineKeySpellings == null || nineKeySpellingScroll == null) return;
        JSONArray spellings = view == null ? null : view.optJSONArray("nine_key_spellings");
        boolean visible = displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT
            && spellings != null && spellings.length() > 0;
        nineKeySpellingScroll.setVisibility(visible ? View.VISIBLE : View.GONE);
        if (!visible) {
            nineKeySpellingIndices = java.util.List.of();
            nineKeySpellingGeneration = -1;
            for (Button key : nineKeySpellingButtons) key.setVisibility(View.GONE);
            return;
        }
        nineKeySpellingGeneration = view.optLong("generation", -1);
        java.util.List<String> values = new java.util.ArrayList<>();
        java.util.List<Integer> indices = new java.util.ArrayList<>();
        for (int index = 0; index < spellings.length(); index++) {
            String spelling = spellings.optString(index, "");
            if (!spelling.isEmpty()) {
                values.add(spelling);
                indices.add(index);
            }
        }
        nineKeySpellingIndices = java.util.List.copyOf(indices);
        while (nineKeySpellingButtons.size() < values.size()) {
            int slot = nineKeySpellingButtons.size();
            Button key = button(nineKeySpellings, "", () -> {
                if (slot < nineKeySpellingIndices.size()) {
                    chooseNineKeySpelling(nineKeySpellingGeneration,
                        nineKeySpellingIndices.get(slot));
                }
            });
            key.setContentDescription("选择拼音");
            LinearLayout.LayoutParams params = (LinearLayout.LayoutParams) key.getLayoutParams();
            params.width = LinearLayout.LayoutParams.WRAP_CONTENT;
            params.weight = 0;
            key.setLayoutParams(params);
            nineKeySpellingButtons.add(key);
        }
        for (int slot = 0; slot < nineKeySpellingButtons.size(); slot++) {
            Button key = nineKeySpellingButtons.get(slot);
            boolean slotVisible = slot < values.size();
            key.setVisibility(slotVisible ? View.VISIBLE : View.GONE);
            if (slotVisible) {
                String spelling = values.get(slot);
                key.setText(spelling);
                key.setContentDescription("选择拼音 " + spelling);
            }
        }
    }

    @Override public View onCreateInputView() {
        cancelInputViewRefresh();
        deactivateHandwriting();
        candidateButtons.clear();
        englishSuggestionButtons.clear();
        nineKeySpellingButtons.clear();
        nineKeySpellingIndices = java.util.List.of();
        nineKeySpellingGeneration = -1;
        loadFeedbackPreferences();
        File files = getFilesDir();
        // The shared store keeps its file under this directory, which is the same one the settings
        // page hands the shared entry; both sides therefore read one history.
        clipboardHistory = new ClipboardHistoryStore(this, files);
        voiceResultStore = files == null ? null
            : new VoiceResultStore(files.toPath().resolve("voice-handoff"));
        communityReplyLibrary = files == null ? null : new CommunityReplyLibrary(files.toPath());
        if (!clipboardHistoryEnabled) clipboardHistory.clearQuietly();
        keyboardRoot = new FrameLayout(this);
        keyboardSurface = new FrameLayout(this);
        keyboardRoot.addView(keyboardSurface);
        applyKeyboardSurfaceGeometry();
        LinearLayout keyboard = new LinearLayout(this);
        keyboard.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(keyboard);
        keyboardSurface.addView(keyboard, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        japaneseFlickPreview = new JapaneseFlickPreview(this);
        keyboardSurface.addView(japaneseFlickPreview, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        LinearLayout candidateRegion = new LinearLayout(this);
        candidateRegion.setOrientation(LinearLayout.VERTICAL);
        LinearLayout candidateHeader = new LinearLayout(this);
        candidateHeader.setGravity(Gravity.CENTER_VERTICAL);
        candidateHeader.setPadding(pixels(10), pixels(6), pixels(6), pixels(2));
        // The brand mark leads the header the way it leads the macOS candidate window's top row: 16dp, then a 6dp gap before the reading. The header is never hidden, so the mark is always present; it is decorative, since the idle pill beside it already reads 水杉输入法.
        candidateBrandMark = new KeyboardBrandMark(this, () -> Color.parseColor(skin.accent()));
        LinearLayout.LayoutParams brandMarkLayout = new LinearLayout.LayoutParams(
            pixels(16), pixels(16));
        brandMarkLayout.setMarginEnd(pixels(6));
        candidateHeader.addView(candidateBrandMark, brandMarkLayout);
        preedit = new TextView(this);
        preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidatePreeditFontSize);
        preedit.setMaxLines(1);
        preedit.setEllipsize(android.text.TextUtils.TruncateAt.END);
        preedit.setOnClickListener(ignored -> {
            playFeedback(preedit);
            showLocalInputMenu();
        });
        // The pill hugs its own text, so it needs a parent that bounds it: a weighted TextView would
        // stretch the outline the whole width of the keyboard.
        LinearLayout preeditFrame = new LinearLayout(this);
        preeditFrame.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
        preeditFrame.addView(preedit, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateHeader.addView(preeditFrame, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        // The host notice channel: idle it names the build, and a deferred or failed preference load
        // is the only thing the user ever reads here. It keeps the caption weight the design gives a
        // secondary label rather than the headline it used to be at the top of the keyboard.
        status = new TextView(this);
        status.setTextSize(TypedValue.COMPLEX_UNIT_SP, 10);
        status.setMaxLines(1);
        status.setEllipsize(android.text.TextUtils.TruncateAt.END);
        status.setGravity(Gravity.CENTER_VERTICAL | Gravity.END);
        status.setPadding(pixels(6), 0, pixels(2), 0);
        candidateHeader.addView(status, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidatePage = new TextView(this);
        candidateHeader.addView(candidatePage, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        shortcutBar = new LinearLayout(this);
        shortcutBar.setOrientation(LinearLayout.HORIZONTAL);
        shortcutBar.setGravity(Gravity.CENTER_VERTICAL);
        shortcutBar.setContentDescription("键盘快捷栏");
        shortcutBar.setPadding(pixels(8), 0, pixels(8), 0);
        shortcutScroll = new HorizontalScrollView(this);
        shortcutScroll.setHorizontalScrollBarEnabled(false);
        shortcutScroll.setContentDescription("键盘快捷栏");
        shortcutScroll.setFillViewport(true);
        // The glyphs share the width evenly instead of queueing from the left edge; the scroll view
        // stays as the fallback for a narrow screen that cannot give each a 44dp target.
        shortcutScroll.addView(shortcutBar, new HorizontalScrollView.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT));
        scriptShortcutButton = button(shortcutBar, "简", this::toggleChineseOutput);
        scriptShortcutButton.setContentDescription("切换到繁体");
        emojiShortcutButton = shortcutButton(shortcutBar, "☺",
            KeyboardShortcutIconPolicy.Icon.EMOJI, this::showEmojiPicker);
        emojiShortcutButton.setContentDescription("打开表情浏览");
        keyId(emojiShortcutButton, "SoftEmoji");
        voiceShortcutButton = shortcutButton(shortcutBar, "语音",
            KeyboardShortcutIconPolicy.Icon.VOICE, this::showVoiceResult);
        voiceShortcutButton.setContentDescription("打开语音结果");
        keyId(voiceShortcutButton, "SoftVoice");
        aiPolishShortcutButton = button(shortcutBar, "AI", this::showAiPolish);
        aiPolishShortcutButton.setContentDescription("打开 AI 润色");
        replyShortcutButton = shortcutButton(shortcutBar, "回复",
            KeyboardShortcutIconPolicy.Icon.REPLY, this::showReplyKeyboard);
        replyShortcutButton.setContentDescription("生成高情商回复");
        // 漢 is the touch counterpart of a Korean keyboard's Hanja key: it lists the Hanja of the composing syllable on the strip below and closes the list again. It sits in the header so it stays put while the list fills the strip, and render() shows it only while a Korean syllable composes; the filled face says the list is open.
        KeyboardPressButton hanja = new KeyboardPressButton(this);
        hanja.setKeyboardRole(KeyboardKeyRole.GLYPH);
        hanjaButton = hanja;
        hanjaButton.setAllCaps(false);
        hanjaButton.setText("漢");
        hanjaButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        hanjaButton.setContentDescription("转换为汉字");
        hanjaButton.setVisibility(View.GONE);
        hanjaButton.setOnClickListener(ignored -> {
            playFeedback(hanjaButton);
            command(KoreanInputPolicy.CONVERT_HANJA_COMMAND);
        });
        candidateHeader.addView(hanjaButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        KeyboardPressButton expand = new KeyboardPressButton(this);
        expand.setKeyboardRole(KeyboardKeyRole.GLYPH);
        expandCandidates = expand;
        expandCandidates.setAllCaps(false);
        expandCandidates.setText("展开");
        expandCandidates.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        expandCandidates.setContentDescription("展开候选面板");
        expandCandidates.setOnClickListener(ignored -> {
            playFeedback(expandCandidates);
            openCandidatePanel();
        });
        candidateHeader.addView(expandCandidates, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        exitLocalModeButton = new KeyboardBorderlessButton(this);
        exitLocalModeButton.setAllCaps(false);
        exitLocalModeButton.setText("×");
        exitLocalModeButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 22);
        exitLocalModeButton.setContentDescription("退出本地模式");
        exitLocalModeButton.setPadding(0, 0, 0, 0);
        styleButton(exitLocalModeButton, true);
        exitLocalModeButton.setOnClickListener(ignored -> {
            playFeedback(exitLocalModeButton);
            command(3);
        });
        candidateHeader.addView(exitLocalModeButton, new LinearLayout.LayoutParams(
            pixels(40), LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateRegion.addView(candidateHeader);
        diagnosticView = new TextView(this);
        diagnosticView.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        diagnosticView.setContentDescription("输入提示");
        diagnosticView.setVisibility(View.GONE);
        candidateRegion.addView(diagnosticView, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateRegion.addView(shortcutScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(KeyboardGeometry.CANDIDATE_ROW_HEIGHT_DP)));
        nineKeySpellings = new LinearLayout(this);
        nineKeySpellings.setOrientation(LinearLayout.HORIZONTAL);
        nineKeySpellingScroll = new HorizontalScrollView(this);
        nineKeySpellingScroll.setHorizontalScrollBarEnabled(false);
        nineKeySpellingScroll.setContentDescription("九键拼音选择");
        nineKeySpellingScroll.addView(nineKeySpellings);
        nineKeySpellingScroll.setVisibility(View.GONE);
        candidates = new LinearLayout(this);
        candidates.setOrientation(LinearLayout.HORIZONTAL);
        horizontalCandidateScroll = new HorizontalScrollView(this);
        horizontalCandidateScroll.addView(candidates);
        verticalCandidates = new LinearLayout(this);
        verticalCandidates.setOrientation(LinearLayout.VERTICAL);
        verticalCandidateScroll = new ScrollView(this);
        verticalCandidateScroll.addView(verticalCandidates);
        candidateViewport = new FrameLayout(this);
        candidateViewport.addView(horizontalCandidateScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        candidateViewport.addView(verticalCandidateScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        candidateViewport.setVisibility(View.GONE);
        candidateRegion.addView(candidateViewport, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(KeyboardGeometry.CANDIDATE_ROW_HEIGHT_DP)));
        candidatePaging = new LinearLayout(this);
        candidatePaging.setOrientation(LinearLayout.HORIZONTAL);
        candidatePaging.setGravity(Gravity.CENTER_VERTICAL);
        candidatePagingScroll = new HorizontalScrollView(this);
        candidatePagingScroll.setHorizontalScrollBarEnabled(false);
        candidatePagingScroll.setContentDescription("组词与候选控制");
        candidatePagingScroll.addView(candidatePaging);
        candidateRegion.addView(candidatePagingScroll);
        keyboard.addView(candidateRegion);
        // Like Apple, keep the shared candidate/shortcut strip above the reply surface. The
        // ordinary key rows and controls are hidden while this weighted child is visible.
        replyKeyboard = createReplyKeyboard();
        replyKeyboard.setVisibility(View.GONE);
        keyboard.addView(replyKeyboard, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        keyRows = new LinearLayout(this);
        keyRows.setOrientation(LinearLayout.VERTICAL);
        keyboard.addView(keyRows);
        actionRow = new LinearLayout(this);
        actionRow.setOrientation(LinearLayout.HORIZONTAL);
        actionRow.setContentDescription("键盘功能行");
        actionRowSignature = "";
        keyboard.addView(actionRow, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            pixels(KeyboardGeometry.STANDARD_ROW_HEIGHT_DP)));
        // Staging only: every control below is created here and then moved to the row that owns it.
        // The case and delete keys go to the last of the 26 key rows, the shortcut glyphs to the
        // toolbar, and what the action row keeps is whatever KeyboardActionRow lists for the surface.
        LinearLayout controls = new LinearLayout(this);
        shiftButton = button(controls, "⇧", () -> {
            // On the Korean keycaps Shift is the double consonant row, not the way into English.
            if (!dedicatedEnglish && session != 0 && !helpcodeCompositionEligible()
                    && !koreanSchemeActive()) {
                toggleInputLanguage();
                if (!dedicatedEnglish) return;
                // Match Apple: the Shift that entered English starts from lowercase even if the
                // editor would otherwise request automatic capitalization.
                letterCase.reset();
            }
            letterCase.toggle(SystemClock.uptimeMillis());
            rebuildKeyRows();
            render();
        });
        shiftButton.setContentDescription("切换到英文大写");
        keyId(shiftButton, "ShiftLeft");
        languageButton = keyId(button(controls, "中/英", this::toggleInputLanguage), "SoftLanguage");
        languageButton.setContentDescription("切换中英文");
        layerButton = button(controls, "123", () -> {
            keyboardLayer = keyboardLayer == KeyboardLayout.Layer.LETTERS
                ? KeyboardLayout.Layer.SYMBOLS : KeyboardLayout.Layer.LETTERS;
            rebuildKeyRows();
            render();
        });
        layerButton.setContentDescription("切换到数字和符号");
        keyId(layerButton, "SoftLayer");
        symbolPanelButton = keyId(button(controls, "符", this::showSymbolPanel), "SoftSymbol");
        symbolPanelButton.setContentDescription("打开符号面板");
        quickPunctuationButton = keyId(button(controls, ",", this::insertQuickPunctuation), "Comma");
        quickPunctuationButton.setOnLongClickListener(ignored -> {
            showQuickPunctuationMenu();
            return true;
        });
        deleteButton = keyId(button(controls, "⌫", this::deleteFromHandwriting), "Backspace");
        // The same 按键 form the nine-key and kana grids give their own delete: the bare 删除 is the
        // emoji panel's, and two nodes answering to it would make either one ambiguous.
        deleteButton.setContentDescription("按键 删除");
        bindBackspaceRepeat(deleteButton, this::deleteFromHandwriting);
        spaceButton = keyId(button(controls, "空格", this::space), "Space");
        spaceButton.setContentDescription(SPACE_CURSOR_DESCRIPTION);
        bindSpaceCursor(spaceButton);
        enterButton = keyId(button(controls, "换行", this::enter), "Enter");
        enterButton.setContentDescription("换行");
        globeButton = shortcutButton(controls, "切换",
            KeyboardShortcutIconPolicy.Icon.GLOBE, this::switchToNextInputMethodAfterCommit);
        globeButton.setContentDescription("切换到下一个输入法");
        keyId(globeButton, "SoftGlobe");
        schemeButton = pillButton(controls, "方案", this::showSchemePicker);
        schemeButton.setContentDescription("选择输入方案");
        skinButton = shortcutButton(controls, "皮肤",
            KeyboardShortcutIconPolicy.Icon.SKIN, () -> showSkinMenu(skinButton));
        skinButton.setContentDescription("切换键盘皮肤");
        layoutSettingsButton = shortcutButton(controls, "设置",
            KeyboardShortcutIconPolicy.Icon.SETTINGS, this::showLayoutSettings);
        layoutSettingsButton.setContentDescription("键盘设置");
        moreButton = brandButton(controls, this::showFeedbackMenu);
        moreButton.setContentDescription("更多快捷设置");
        Button dismissButton = shortcutButton(controls, "收起",
            KeyboardShortcutIconPolicy.Icon.DISMISS, () -> requestHideSelf(0));
        dismissButton.setContentDescription("收起键盘");
        installShortcutBar(dismissButton);
        // The rows are built after the controls exist: the case and delete keys are laid into the
        // last of them, and they are the same long-lived instances the rest of the host talks to.
        rebuildKeyRows();
        updateActionRow();
        expandedCandidates = new LinearLayout(this);
        expandedCandidates.setOrientation(LinearLayout.VERTICAL);
        expandedCandidates.setPadding(24, 16, 24, 16);
        expandedCandidates.setBackgroundColor(0xfff5f5f5);
        expandedCandidates.setContentDescription("候选面板");
        expandedCandidates.setVisibility(View.GONE);
        expandedCandidateScroll = new ScrollView(this);
        expandedCandidateScroll.addView(expandedCandidates);
        expandedCandidateScroll.setVisibility(View.GONE);
        keyboardSurface.addView(expandedCandidateScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        clipboardPanel = new LinearLayout(this);
        clipboardPanel.setOrientation(LinearLayout.VERTICAL);
        clipboardPanel.setPadding(24, 16, 24, 16);
        clipboardPanel.setBackgroundColor(Color.parseColor(skin.background()));
        clipboardPanel.setContentDescription("剪贴板历史");
        clipboardScroll = new ScrollView(this);
        clipboardScroll.addView(clipboardPanel);
        clipboardScroll.setVisibility(View.GONE);
        keyboardSurface.addView(clipboardScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        schemePanel = new LinearLayout(this);
        schemePanel.setOrientation(LinearLayout.VERTICAL);
        schemePanel.setPadding(24, 16, 24, 16);
        schemePanel.setBackgroundColor(Color.parseColor(skin.background()));
        schemePanel.setContentDescription("输入方案选择器");
        schemeScroll = new ScrollView(this);
        schemeScroll.addView(schemePanel, new ScrollView.LayoutParams(
            ScrollView.LayoutParams.MATCH_PARENT, ScrollView.LayoutParams.MATCH_PARENT));
        schemeScroll.setFillViewport(true);
        schemeScroll.setVisibility(View.GONE);
        keyboardSurface.addView(schemeScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        skinPanel = new LinearLayout(this);
        skinPanel.setOrientation(LinearLayout.VERTICAL);
        skinPanel.setPadding(24, 16, 24, 16);
        skinPanel.setBackgroundColor(Color.parseColor(skin.background()));
        skinPanel.setContentDescription("键盘皮肤选择器");
        skinScroll = new ScrollView(this);
        skinScroll.addView(skinPanel, new ScrollView.LayoutParams(
            ScrollView.LayoutParams.MATCH_PARENT, ScrollView.LayoutParams.MATCH_PARENT));
        skinScroll.setFillViewport(true);
        skinScroll.setVisibility(View.GONE);
        keyboardSurface.addView(skinScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        layoutSettingsPanel = new LinearLayout(this);
        layoutSettingsPanel.setOrientation(LinearLayout.VERTICAL);
        layoutSettingsPanel.setPadding(24, 16, 24, 16);
        layoutSettingsPanel.setBackgroundColor(Color.parseColor(skin.background()));
        layoutSettingsPanel.setContentDescription("键盘设置");
        LinearLayout layoutHeader = new LinearLayout(this);
        TextView layoutTitle = new TextView(this);
        layoutTitle.setText("键盘设置");
        layoutTitle.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18);
        layoutHeader.addView(layoutTitle, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button closeLayout = button(layoutHeader, "返回键盘", this::closeLayoutSettings);
        closeLayout.setContentDescription("返回键盘");
        layoutSettingsPanel.addView(layoutHeader);
        LinearLayout keyboardHeightHeader = new LinearLayout(this);
        TextView keyboardHeightLabel = new TextView(this);
        keyboardHeightLabel.setText("键盘高度");
        keyboardHeightHeader.addView(keyboardHeightLabel, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        keyboardHeightValue = new TextView(this);
        keyboardHeightHeader.addView(keyboardHeightValue);
        layoutSettingsPanel.addView(keyboardHeightHeader);
        keyboardHeightSlider = new SeekBar(this);
        keyboardHeightSlider.setContentDescription("键盘高度");
        configureHeightSlider(keyboardHeightSlider);
        layoutSettingsPanel.addView(keyboardHeightSlider);
        LinearLayout keySpacingHeader = new LinearLayout(this);
        TextView keySpacingLabel = new TextView(this);
        keySpacingLabel.setText("按键间距");
        keySpacingHeader.addView(keySpacingLabel, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        keySpacingValue = new TextView(this);
        keySpacingHeader.addView(keySpacingValue);
        layoutSettingsPanel.addView(keySpacingHeader);
        keySpacingSlider = new SeekBar(this);
        keySpacingSlider.setContentDescription("按键间距");
        configureSpacingSlider(keySpacingSlider, true);
        layoutSettingsPanel.addView(keySpacingSlider);
        LinearLayout rowSpacingHeader = new LinearLayout(this);
        TextView rowSpacingLabel = new TextView(this);
        rowSpacingLabel.setText("行间距");
        rowSpacingHeader.addView(rowSpacingLabel, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        rowSpacingValue = new TextView(this);
        rowSpacingHeader.addView(rowSpacingValue);
        layoutSettingsPanel.addView(rowSpacingHeader);
        rowSpacingSlider = new SeekBar(this);
        rowSpacingSlider.setContentDescription("行间距");
        configureSpacingSlider(rowSpacingSlider, false);
        layoutSettingsPanel.addView(rowSpacingSlider);
        voiceShortcutSwitch = new Switch(this);
        voiceShortcutSwitch.setText("顶部语音入口");
        voiceShortcutSwitch.setContentDescription("顶部语音入口");
        voiceShortcutSwitch.setOnCheckedChangeListener((button, checked) -> {
            if (checked == touchVoiceShortcutEnabled
                    || touchGeometrySaving || traditionalOutputSaving) return;
            touchVoiceShortcutEnabled = checked;
            renderLayoutSettingsState();
            render();
            saveTouchGeometry();
        });
        layoutSettingsPanel.addView(voiceShortcutSwitch);
        resetLayoutSettingsButton = button(layoutSettingsPanel, "恢复默认", this::resetTouchGeometry);
        resetLayoutSettingsButton.setContentDescription("恢复默认");
        TextView layoutHint = new TextView(this);
        layoutHint.setText("高度和间距只改变键位外观，不改变输入方案；松手后自动保存。");
        layoutSettingsPanel.addView(layoutHint);
        layoutSettingsScroll = new ScrollView(this);
        layoutSettingsScroll.addView(layoutSettingsPanel);
        layoutSettingsScroll.setVisibility(View.GONE);
        keyboardSurface.addView(layoutSettingsScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        layoutAdjustView = new KeyboardLayoutAdjustView(this,
            new KeyboardLayoutAdjustView.Listener() {
                @Override public void keySpacing(int tenths) {
                    previewTouchGeometry(true, tenths);
                }

                @Override public void rowSpacing(int tenths) {
                    previewTouchGeometry(false, tenths);
                }

                @Override public void height(int adjustment) {
                    previewTouchHeight(adjustment);
                }

                @Override public void voiceShortcut(boolean enabled) {
                    if (enabled == touchVoiceShortcutEnabled
                            || touchGeometrySaving || traditionalOutputSaving) return;
                    touchVoiceShortcutEnabled = enabled;
                    renderLayoutSettingsState();
                    render();
                    saveTouchGeometry();
                }

                @Override public void commit() { saveTouchGeometry(); }

                @Override public void reset() { resetTouchGeometry(); }

                @Override public void close() { closeLayoutSettings(); }
            });
        layoutAdjustView.setVisibility(View.GONE);
        keyboardSurface.addView(layoutAdjustView, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        voiceResultPanel = new LinearLayout(this);
        voiceResultPanel.setOrientation(LinearLayout.VERTICAL);
        voiceResultPanel.setPadding(24, 16, 24, 16);
        voiceResultPanel.setBackgroundColor(Color.parseColor(skin.background()));
        voiceResultPanel.setContentDescription("语音结果面板");
        voiceResultScroll = new ScrollView(this);
        voiceResultScroll.addView(voiceResultPanel);
        voiceResultScroll.setVisibility(View.GONE);
        keyboardSurface.addView(voiceResultScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        aiPolishContainer = new LinearLayout(this);
        aiPolishContainer.setOrientation(LinearLayout.VERTICAL);
        aiPolishContainer.setBackgroundColor(Color.parseColor(skin.background()));
        aiPolishPanel = new LinearLayout(this);
        aiPolishPanel.setOrientation(LinearLayout.VERTICAL);
        aiPolishPanel.setPadding(24, 16, 24, 16);
        aiPolishPanel.setBackgroundColor(Color.parseColor(skin.background()));
        aiPolishPanel.setContentDescription("AI 润色面板");
        aiPolishScroll = new ScrollView(this);
        aiPolishScroll.setFillViewport(true);
        aiPolishScroll.addView(aiPolishPanel);
        aiPolishContainer.addView(aiPolishScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        aiPolishActions = new LinearLayout(this);
        aiPolishActions.setOrientation(LinearLayout.VERTICAL);
        aiPolishActions.setPadding(24, 0, 24, 16);
        aiPolishContainer.addView(aiPolishActions, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        aiPolishContainer.setVisibility(View.GONE);
        keyboardSurface.addView(aiPolishContainer, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        moreToolsPanel = new LinearLayout(this);
        moreToolsPanel.setOrientation(LinearLayout.VERTICAL);
        moreToolsPanel.setPadding(pixels(12), 0, pixels(12), pixels(10));
        moreToolsPanel.setBackgroundColor(Color.parseColor(skin.background()));
        moreToolsScroll = new ScrollView(this);
        moreToolsScroll.setFillViewport(true);
        moreToolsScroll.setVerticalScrollBarEnabled(false);
        moreToolsScroll.setContentDescription("更多工具");
        moreToolsScroll.addView(moreToolsPanel);
        moreToolsScroll.setVisibility(View.GONE);
        keyboardSurface.addView(moreToolsScroll, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        emojiPreferences = getSharedPreferences(EMOJI_RECENTS_PREFERENCES, MODE_PRIVATE);
        emojiRecents = loadEmojiRecents();
        emojiPanel = new LinearLayout(this);
        emojiPanel.setOrientation(LinearLayout.VERTICAL);
        emojiPanel.setPadding(pixels(8), 0, pixels(8), pixels(6));
        emojiPanel.setBackgroundColor(Color.parseColor(skin.background()));
        emojiPanel.setContentDescription("表情面板");
        emojiPanel.setFocusable(true);
        LinearLayout emojiHeader = new LinearLayout(this);
        emojiHeader.setGravity(Gravity.CENTER_VERTICAL);
        Button closeEmoji = button(emojiHeader, "‹", this::closeEmojiPicker);
        closeEmoji.setContentDescription("返回键盘");
        closeEmoji.setLayoutParams(new LinearLayout.LayoutParams(pixels(56), pixels(40)));
        TextView emojiTitle = new TextView(this);
        emojiTitle.setText("表情");
        emojiTitle.setTextSize(TypedValue.COMPLEX_UNIT_SP, 17);
        emojiTitle.setGravity(Gravity.CENTER);
        emojiHeader.addView(emojiTitle, new LinearLayout.LayoutParams(0, pixels(40), 1));
        Button deleteEmoji = button(emojiHeader, "⌫", this::deleteFromEmojiPicker);
        deleteEmoji.setContentDescription("删除");
        deleteEmoji.setLayoutParams(new LinearLayout.LayoutParams(pixels(56), pixels(40)));
        emojiPanel.addView(emojiHeader);
        emojiTabs = new LinearLayout(this);
        emojiTabs.setOrientation(LinearLayout.HORIZONTAL);
        emojiTabsScroll = new HorizontalScrollView(this);
        emojiTabsScroll.setHorizontalScrollBarEnabled(false);
        emojiTabsScroll.setContentDescription("表情分类");
        emojiTabsScroll.addView(emojiTabs);
        emojiPanel.addView(emojiTabsScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(40)));
        emojiStatus = new TextView(this);
        emojiStatus.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11);
        emojiStatus.setGravity(Gravity.CENTER_VERTICAL);
        emojiPanel.addView(emojiStatus, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, pixels(20)));
        emojiGrid = new GridLayout(this);
        emojiGrid.setColumnCount(EmojiCatalogModel.COLUMNS);
        emojiGrid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        emojiGrid.setUseDefaultMargins(false);
        emojiGridScroll = new ScrollView(this);
        emojiGridScroll.setFillViewport(false);
        emojiGridScroll.setVerticalScrollBarEnabled(false);
        emojiGridScroll.setContentDescription("表情网格；每行八个");
        emojiGridScroll.addView(emojiGrid, new ScrollView.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        emojiGridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            if (scrollY > oldY && !view.canScrollVertically(1)) loadEmojiPage();
        });
        emojiPanel.addView(emojiGridScroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        emojiPanel.setVisibility(View.GONE);
        keyboardSurface.addView(emojiPanel, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        symbolPanel = new SymbolPanelView(this,
            (title, description, action, actionStyle) -> {
                Button button = new KeyboardPressButton(this);
                button.setAllCaps(false);
                button.setText(title);
                button.setContentDescription(actionStyle ? description : "按键 " + description);
                styleButton(button, actionStyle);
                button.setOnClickListener(ignored -> {
                    playFeedback(button);
                    action.run();
                });
                return button;
            },
            new SymbolPanelView.Listener() {
                @Override public void insert(String text) {
                    if (connection != null) commitText(text, TypingSource.LOCAL);
                }

                @Override public void delete() {
                    if (connection != null && !command(0))
                        connection.deleteSurroundingTextInCodePoints(1, 0);
                }

                @Override public void close() { closeSymbolPanel(); }
            });
        symbolPanel.setVisibility(View.GONE);
        keyboardSurface.addView(symbolPanel, new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        renderLayoutSettingsState();
        render();
        synchronizeReplyKeyboard();
        return keyboardRoot;
    }

    private void render() {
        updateSymbolKeyFaces();
        updateShuangpinKeyHints();
        updateQuickPunctuation();
        String currentEditingText = view == null ? "" : view.optString("editing_text", "");
        if (!japaneseSchemeActive() || currentEditingText.isEmpty()) {
            japaneseConversionIndex = null;
            japaneseConversionEditingText = "";
        } else if (japaneseConversionIndex != null
                && !currentEditingText.equals(japaneseConversionEditingText)) {
            japaneseConversionIndex = null;
            japaneseConversionEditingText = "";
        }
        String page = "";
        if (view != null && view.optInt("page_count", 0) > 0)
            page = " · " + (view.optInt("page", 0) + 1) + "/" + view.optInt("page_count");
        String localMode = "";
        if (view != null) {
            String modeKey = view.optString("local_mode", "none");
            for (LocalInputMode mode : LocalInputMode.values()) {
                if (mode.preferenceKey().equals(modeKey)) {
                    localMode = " · " + mode.title();
                    break;
                }
            }
        }
        // 分页不再进这一行：candidatePage 就在它旁边，两个 1/33 挨着显示是同一件事说了两遍。
        if (status != null) status.setText(message + preferencesNotice
            + (dedicatedEnglish ? " · 英文输入" : "") + localMode
            + switch (letterCase.mode()) {
                case LOWERCASE -> "";
                case SHIFTED -> " · Shift";
                case CAPS_LOCK -> " · Caps Lock";
            });
        if (candidatePage != null) candidatePage.setText(page.isEmpty() ? "" : page.substring(3));
        JSONArray visibleCandidates = view == null ? null : view.optJSONArray("candidates");
        boolean hasEnglishSuggestions = englishSuggestionsActive() && !englishSuggestions.isEmpty();
        boolean handwriting = handwritingActive();
        boolean hasHandwritingResults = handwriting && !handwritingResults.isEmpty()
            && handwritingCandidateToken != null;
        boolean idle = view == null || (view.optString("editing_text", "").isEmpty()
            && "none".equals(view.optString("local_mode", "none"))
            && (visibleCandidates == null || visibleCandidates.length() == 0)
            && !hasEnglishSuggestions
            && !hasHandwritingResults);
        if (preedit != null) {
            preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidatePreeditFontSize);
            String editingText = view == null ? "" : view.optString("editing_text", "");
            boolean offersLocalModes = idle && supportsLocalTools();
            String localModeKey = view == null ? "none" : view.optString("local_mode", "none");
            String reading = view == null ? "" : view.optString("reading", "");
            String localModeTitle = "none".equals(localModeKey)
                ? (reading.isEmpty() ? editingText : reading) : editingText;
            // A mode's own name is a label saying which mode is running, not composed input, so it
            // survives 「不显示」; anything the mode is spelling beyond its trigger does not.
            boolean localModeName = false;
            for (LocalInputMode mode : LocalInputMode.values()) {
                if (mode.preferenceKey().equals(localModeKey)
                        && mode.trigger().equals(editingText)) {
                    localModeTitle = mode.title();
                    localModeName = true;
                    break;
                }
            }
            localModeTitle = CandidatePreeditStylePolicy.composedText(
                candidatePreeditStyle, localModeTitle, localModeName);
            boolean idleTitle = idle && editingText.isEmpty();
            brandPillVisible = idleTitle;
            String phrasePrefix = view == null ? "" : view.optString("phrase_prefix", "");
            String displayText = idleTitle
                ? (dedicatedEnglish ? "英文输入" : "水杉输入法")
                : PhrasePreeditPolicy.title(phrasePrefix, localModeTitle,
                                            !"none".equals(localModeKey));
            preedit.setText(displayText);
            preedit.setContentDescription(offersLocalModes ? "本地输入模式" : displayText);
            preedit.setClickable(offersLocalModes);
            preedit.setFocusable(offersLocalModes);
        }
        if (exitLocalModeButton != null) {
            boolean localModeActive = view != null
                && !"none".equals(view.optString("local_mode", "none"));
            exitLocalModeButton.setVisibility(localModeActive ? View.VISIBLE : View.GONE);
            exitLocalModeButton.setEnabled(localModeActive && session != 0);
            exitLocalModeButton.setContentDescription("退出本地模式");
            styleButton(exitLocalModeButton, KeyboardKeyRole.GLYPH, skin);
        }
        if (hanjaButton != null) {
            boolean offersHanja = session != 0 && koreanConvertsHanja();
            boolean hanjaListOpen = offersHanja && koreanHanjaListOpen();
            hanjaButton.setVisibility(offersHanja ? View.VISIBLE : View.GONE);
            hanjaButton.setEnabled(offersHanja);
            hanjaButton.setSelected(hanjaListOpen);
            hanjaButton.setContentDescription(hanjaListOpen ? "关闭汉字列表" : "转换为汉字");
        }
        boolean hasDiagnostic = InputDiagnosticPolicy.visible(diagnosticMessage);
        if (diagnosticView != null) {
            diagnosticView.setText(diagnosticMessage);
            diagnosticView.setContentDescription("提示：" + diagnosticMessage);
            diagnosticView.setTextColor(Color.parseColor(skin.accent()));
            diagnosticView.setVisibility(hasDiagnostic ? View.VISIBLE : View.GONE);
        }
        if (shortcutScroll != null)
            shortcutScroll.setVisibility(idle && !hasDiagnostic ? View.VISIBLE : View.GONE);
        if (replyKeyboard == null || replyKeyboard.getVisibility() != View.VISIBLE)
            updateActionRow();
        if (candidateViewport != null)
            candidateViewport.setVisibility(!idle && !hasDiagnostic ? View.VISIBLE : View.GONE);
        updateCandidateViewportHeight();
        if (scriptShortcutButton != null) {
            scriptShortcutButton.setVisibility(View.GONE);
            scriptShortcutButton.setText(traditionalChineseOutput ? "繁" : "简");
            scriptShortcutButton.setSelected(traditionalChineseOutput);
            styleButton(scriptShortcutButton, true);
            int scheme = view == null
                ? ((selectedScheme == KeyboardScheme.JAPANESE
                    || selectedScheme == KeyboardScheme.JAPANESE_NINE_KEY) ? 3
                    : selectedScheme == KeyboardScheme.KOREAN ? KoreanInputPolicy.KOREAN_SCHEME : -1)
                : view.optInt("scheme", -1);
            boolean japanese = scheme == 3;
            boolean korean = scheme == KoreanInputPolicy.KOREAN_SCHEME;
            scriptShortcutButton.setEnabled(!japanese && !korean && canSaveChineseOutput());
            String label = traditionalChineseOutput ? "切换到简体" : "切换到繁体";
            String outputState = japanese ? "日语不使用简繁转换"
                : korean ? "韩语不使用简繁转换"
                : traditionalOutputSaving ? "正在保存"
                : traditionalChineseOutput ? "繁体" : "简体";
            scriptShortcutButton.setContentDescription(
                Build.VERSION.SDK_INT >= 30 ? label : label + "，" + outputState);
            if (Build.VERSION.SDK_INT >= 30)
                scriptShortcutButton.setStateDescription(outputState);
        }
        if (emojiShortcutButton != null) {
            emojiShortcutButton.setVisibility(idle && session != 0 && !emojiResources.isEmpty()
                && selectedScheme != KeyboardScheme.THOUGHTFUL_REPLY ? View.VISIBLE : View.GONE);
            emojiShortcutButton.setEnabled(session != 0 && !emojiResources.isEmpty());
        }
        if (voiceShortcutButton != null) {
            voiceShortcutButton.setVisibility(touchVoiceShortcutEnabled
                && selectedScheme != KeyboardScheme.THOUGHTFUL_REPLY ? View.VISIBLE : View.GONE);
            voiceShortcutButton.setEnabled(voiceInsertionReady());
        }
        if (aiPolishShortcutButton != null) {
            aiPolishShortcutButton.setVisibility(
                aiPolishConfiguration == null ? View.GONE : View.VISIBLE);
            aiPolishShortcutButton.setEnabled(aiPolishReady());
        }
        if (replyShortcutButton != null) {
            replyShortcutButton.setVisibility(selectedScheme == KeyboardScheme.THOUGHTFUL_REPLY
                ? View.VISIBLE : View.GONE);
            replyShortcutButton.setEnabled(replyReady());
        }
        if (microsoftFinalKey != null) {
            String currentLocalMode = view == null ? "none" : view.optString("local_mode", "none");
            boolean visible = MicrosoftShuangpinKeyPolicy.visible(
                dedicatedEnglish, selectedScheme, currentLocalMode);
            microsoftFinalKey.setVisibility(visible ? View.VISIBLE : View.GONE);
            microsoftFinalKey.setEnabled(visible && session != 0);
            microsoftFinalKey.setContentDescription("微软双拼 ing");
        }
        if (layerButton != null) {
            boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
            layerButton.setText(KeyboardActionRow.layerTitle(displayedTouchLayout(view), symbols));
            layerButton.setContentDescription(KeyboardActionRow.layerDescription(symbols));
        }
        if (symbolPanelButton != null) {
            symbolPanelButton.setEnabled(session != 0 && connection != null
                && keyboardLayer == KeyboardLayout.Layer.LETTERS);
            symbolPanelButton.setContentDescription("打开符号面板");
        }
        updateReturnKey();
        if (shiftButton != null) {
            // 九键切数字仍然是九键，没有大小写可切。Shift only returns when the symbol layer actually
            // hands over to the 26-key rows, which the nine-key grids never do.
            int shiftLayout = displayedTouchLayout(view);
            boolean keepsOwnGrid = shiftLayout == QUANPIN_NINE_KEY_LAYOUT
                || shiftLayout == JAPANESE_NINE_KEY_LAYOUT;
            shiftButton.setVisibility(!KeyboardLayout.carriesLetterCase(shiftLayout)
                && (keepsOwnGrid || keyboardLayer == KeyboardLayout.Layer.LETTERS)
                ? View.GONE : View.VISIBLE);
            shiftButton.setText(letterCase.keyText());
            shiftButton.setSelected(letterCase.usesUppercase());
            shiftButton.setActivated(letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
            // The tinted function face in the letter row, and the filled accent only while it is on.
            styleButton(shiftButton, KeyboardKeyRole.ACCENT, skin);
            String caseLabel = shiftLayout == KeyboardLayout.KOREAN_LAYOUT
                ? KoreanKeyboardLayout.SHIFT_LABEL
                : letterCase.accessibilityLabel(dedicatedEnglish || session == 0);
            String caseValue = letterCase.accessibilityValue();
            shiftButton.setContentDescription(Build.VERSION.SDK_INT >= 30
                ? caseLabel : caseLabel + "，" + caseValue);
            if (Build.VERSION.SDK_INT >= 30) shiftButton.setStateDescription(caseValue);
        }
        if (languageButton != null) {
            languageButton.setText(dedicatedEnglish ? "英" : "中");
            languageButton.setEnabled(session != 0);
            languageButton.setContentDescription(
                dedicatedEnglish ? "切换到所选输入方案" : "切换到英文输入");
            if (Build.VERSION.SDK_INT >= 30) {
                languageButton.setStateDescription(dedicatedEnglish ? "英文输入" : "中文输入");
            }
        }
        if (schemeButton != null) {
            schemeButton.setText(selectedScheme.glyph() + selectedScheme.badge());
            schemeButton.setContentDescription("输入方案：" + selectedScheme.title());
            boolean schemeReady = session != 0 && preferencesSnapshot != null
                && !schemeSaving && !touchGeometrySaving && !traditionalOutputSaving;
            schemeButton.setEnabled(schemeReady);
            if (Build.VERSION.SDK_INT >= 30) {
                String schemeState = session == 0 ? "输入会话未就绪"
                    : preferencesSnapshot == null ? "设置加载中"
                    : schemeSaving ? "正在切换输入方案"
                    : touchGeometrySaving || traditionalOutputSaving ? "正在保存其他设置"
                    : "可用";
                schemeButton.setStateDescription(schemeState);
            }
        }
        if (skinButton != null) {
            skinButton.setEnabled(canSaveKeyboardSkin());
            skinButton.setContentDescription("切换键盘皮肤；当前" + skin.title());
            if (Build.VERSION.SDK_INT >= 30) skinButton.setStateDescription(skin.title());
        }
        synchronizeReplyKeyboard();
        if (layoutSettingsButton != null)
            layoutSettingsButton.setEnabled(session != 0 && preferencesSnapshot != null
                && !schemeSaving && !touchGeometrySaving && !traditionalOutputSaving);
        renderNineKeySpellings();
        if (hasDiagnostic && nineKeySpellingScroll != null)
            nineKeySpellingScroll.setVisibility(View.GONE);
        scheduleCandidateGlosses();
        scheduleCandidateTranslations();
        scheduleOnlineProviders();
        if (candidates == null) {
            applySkin();
            return;
        }
        if (horizontalCandidateScroll != null)
            horizontalCandidateScroll.setVisibility(
                candidateHorizontal && !hasDiagnostic ? View.VISIBLE : View.GONE);
        if (verticalCandidateScroll != null)
            verticalCandidateScroll.setVisibility(
                !candidateHorizontal && !hasDiagnostic ? View.VISIBLE : View.GONE);
        LinearLayout activeCandidates = candidateHorizontal ? candidates : verticalCandidates;
        candidates.removeAllViews();
        if (verticalCandidates != null) verticalCandidates.removeAllViews();
        if (candidatePaging != null) candidatePaging.removeAllViews();
        if (expandCandidates != null) expandCandidates.setVisibility(View.GONE);
        // 这一行是组词时才有意义的控制：光标在组词串里移动、翻候选页、取消这次组词。空闲时它是
        // 十个按不出结果的按钮，占掉候选行上方一整行。
        if (candidatePagingScroll != null)
            candidatePagingScroll.setVisibility(idle || hasDiagnostic ? View.GONE : View.VISIBLE);
        if (view == null) {
            closeCandidatePanel();
            renderEnglishSuggestions(activeCandidates);
            for (Button button : candidateButtons) button.setVisibility(View.GONE);
            applySkin();
            return;
        }
        JSONArray entries = view.optJSONArray("candidates");
        if (handwriting) {
            renderSharedHandwritingCandidates(activeCandidates);
        } else if (englishSuggestionsActive()) {
            renderEnglishSuggestions(activeCandidates);
        } else if (entries != null) {
            for (int slot = 0; slot < entries.length(); slot++) {
                JSONObject candidate = entries.optJSONObject(slot);
                if (candidate == null) continue;
                while (candidateButtons.size() <= slot)
                    candidateButtons.add(makeCandidateButton(candidateButtons.size()));
                Button candidateView = candidateButtons.get(slot);
                candidateView.setVisibility(View.VISIBLE);
                updateCandidateButton(candidateView, candidate, slot);
                activeCandidates.addView(candidateView, new LinearLayout.LayoutParams(
                    candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                        : LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT));
            }
            if (!hasDiagnostic && view.optInt("page_count", 0) > 1 && expandCandidates != null)
                expandCandidates.setVisibility(View.VISIBLE);
        }
        int visibleSlots = entries == null ? 0 : entries.length();
        for (int slot = visibleSlots; slot < candidateButtons.size(); slot++)
            candidateButtons.get(slot).setVisibility(View.GONE);
        if (directEnglishActive()) {
            for (Button button : candidateButtons) button.setVisibility(View.GONE);
        }
        if (!handwriting && !directEnglishActive() && candidatePaging != null) {
            // 这些键只在组词时有意义，所以它们跟着候选分页行一起出现和消失。They used to sit in the
            // strip under the keys, where they were permanent and did nothing most of the time.
            pagingKey("首", "组词光标移到开头", () -> command(6));
            pagingKey("←", "组词光标左移", () -> command(4));
            pagingKey("→", "组词光标右移", () -> command(5));
            pagingKey("尾", "组词光标移到结尾", () -> command(7));
            pagingKey("上词", "上一个候选", () -> command(103));
            pagingKey("下词", "下一个候选", () -> command(102));
            pagingKey("上一页", "上一页候选", () -> command(101));
            pagingKey("下一页", "下一页候选", () -> command(100));
            pagingKey("删除", "删除光标后一个字符", () -> command(8));
            pagingKey("取消", "取消本次组词", this::discardComposition);
        }
        if (hasDiagnostic) closeCandidatePanel();
        resetCandidateScrollIfViewChanged();
        renderExpandedCandidates();
        if (moreToolsScroll != null && moreToolsScroll.getVisibility() == View.VISIBLE)
            renderMoreTools();
        applySkin();
    }

    /**
     * A new Engine page must start at its first visible candidate. Keeping the old scroll offset
     * makes a page change look like a reordered or missing candidate list, especially when the
     * previous page had a long sentence at the leading edge. Gloss-only redraws keep the offset
     * because session/generation/page are unchanged.
     */
    private void resetCandidateScrollIfViewChanged() {
        if (view == null) {
            candidateScrollSession = -1;
            candidateScrollGeneration = -1;
            candidateScrollPage = -1;
            if (horizontalCandidateScroll != null) horizontalCandidateScroll.scrollTo(0, 0);
            if (verticalCandidateScroll != null) verticalCandidateScroll.scrollTo(0, 0);
            return;
        }
        long nextSession = view.optLong("session", session);
        long nextGeneration = view.optLong("generation", -1);
        int nextPage = view.optInt("page", -1);
        if (!CandidateScrollPolicy.changed(candidateScrollSession, candidateScrollGeneration,
                candidateScrollPage, nextSession, nextGeneration, nextPage)) return;
        candidateScrollSession = nextSession;
        candidateScrollGeneration = nextGeneration;
        candidateScrollPage = nextPage;
        if (horizontalCandidateScroll != null)
            horizontalCandidateScroll.post(() -> horizontalCandidateScroll.scrollTo(0, 0));
        if (verticalCandidateScroll != null)
            verticalCandidateScroll.post(() -> verticalCandidateScroll.scrollTo(0, 0));
    }

    private void renderSharedHandwritingCandidates(LinearLayout activeCandidates) {
        if (handwritingStatus == null) return;
        if (handwritingResults.isEmpty() || handwritingCandidateToken == null) {
            handwritingStatus.setVisibility(View.VISIBLE);
            return;
        }
        handwritingStatus.setVisibility(View.GONE);
        for (int index = 0; index < handwritingResults.size(); index++) {
            String candidate = handwritingResults.get(index);
            HandwritingRequestTracker.Token token = handwritingCandidateToken;
            Button choice = keyboardKey(chineseOutput(candidate, view),
                "手写候选 " + (index + 1), () -> commitHandwritingCandidate(token, candidate));
            choice.setTextSize(TypedValue.COMPLEX_UNIT_SP, candidateFontSize);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
            activeCandidates.addView(choice, params);
        }
    }
}
