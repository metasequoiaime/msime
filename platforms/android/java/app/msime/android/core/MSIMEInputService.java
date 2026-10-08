package app.msime.android;

import android.inputmethodservice.InputMethodService;
import app.msime.android.core.Telemetry;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.ClipDescription;
import android.content.Context;
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
import app.msime.android.core.InputViewValuePolicy;
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
    static final int QUANPIN_NINE_KEY_LAYOUT = 1;
    static final int JAPANESE_NINE_KEY_LAYOUT = 2;
    static final int HANDWRITING_LAYOUT = 3;
    private static final long HANDWRITING_DEBOUNCE_MILLIS = 550;
    static final long BACKSPACE_REPEAT_DELAY_MILLIS = 400;
    static final long BACKSPACE_REPEAT_INTERVAL_MILLIS = 75;
    private static final long PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS = 500;
    private static final long INPUT_VIEW_REFRESH_DELAY_MILLIS = 32;
    /** How long counted key presses wait in memory before they are written anyway. */
    private static final long KEY_PRESS_FLUSH_DELAY_MILLIS = 30_000;
    private static final String INPUT_MODE_PREFERENCES = "android-input-modes";
    static final String EMOJI_RECENTS_PREFERENCES = "android-emoji-recents";
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
    static final int CYCLE_KANA_VARIANT_COMMAND = 10;
    // 键盘的构建、渲染与扩展点（见各类说明）；onCreate 里创建。
    ImeToolbar imeToolbar;
    ImeCandidates imeCandidates;
    ImeFunctionPanel imeFunctionPanel;
    ImePanels imePanels;
    ImeLetterRows imeLetterRows;
    ImeGlideTyping imeGlideTyping;
    ImeBottomRow imeBottomRow;
    ImeLayoutRows imeLayoutRows;
    ImeStyler imeStyler;
    ImeFrame imeFrame;
    ImePrivacyGate imePrivacyGate;
    ImeVoiceEntry imeVoiceEntry;
    ImeKeyFeedback imeKeyFeedback;
    ImeDebugOverlay imeDebugOverlay;
    long session;
    InputConnection connection;
    private EditorBridge bridge = new EditorBridge();
    JSONObject view;
    FrameLayout keyboardRoot;
    FrameLayout keyboardSurface;
    private LinearLayout candidates;
    private LinearLayout verticalCandidates;
    FrameLayout candidateViewport;
    private HorizontalScrollView horizontalCandidateScroll;
    private ScrollView verticalCandidateScroll;
    private final java.util.List<Button> candidateButtons = new java.util.ArrayList<>();
    private final java.util.List<Button> englishSuggestionButtons = new java.util.ArrayList<>(32);
    LinearLayout expandedCandidates;
    ScrollView expandedCandidateScroll;
    TextView preedit;
    TextView candidatePage;
    KeyboardBrandMark candidateBrandMark;
    Button exitLocalModeButton;
    /** 漢 in the candidate header: converts the composing Korean syllable to Hanja, or closes its list. */
    Button hanjaButton;
    LinearLayout nineKeySpellings;
    HorizontalScrollView nineKeySpellingScroll;
    final java.util.List<Button> nineKeySpellingButtons = new java.util.ArrayList<>();
    java.util.List<Integer> nineKeySpellingIndices = java.util.List.of();
    long nineKeySpellingGeneration = -1;
    private long candidateScrollSession = -1;
    private long candidateScrollGeneration = -1;
    private int candidateScrollPage = -1;
    Button expandCandidates;
    boolean candidatePanelOpen;
    JSONObject candidatePanelSnapshot;
    PopupWindow nineKeyHoldPopup;
    ScrollView clipboardScroll;
    LinearLayout clipboardPanel;
    ScrollView schemeScroll;
    LinearLayout schemePanel;
    ScrollView skinScroll;
    LinearLayout skinPanel;
    private ScrollView layoutSettingsScroll;
    LinearLayout layoutSettingsPanel;
    KeyboardLayoutAdjustView layoutAdjustView;
    ScrollView moreToolsScroll;
    LinearLayout moreToolsPanel;
    boolean localInputToolsOpen;
    LinearLayout emojiPanel;
    LinearLayout emojiTabs;
    ScrollView emojiGridScroll;
    LinearLayout emojiGrid;
    private SeekBar keySpacingSlider;
    private SeekBar rowSpacingSlider;
    private SeekBar keyboardHeightSlider;
    private Switch voiceShortcutSwitch;
    private Button resetLayoutSettingsButton;
    private TextView keySpacingValue;
    private TextView rowSpacingValue;
    private TextView keyboardHeightValue;
    ClipboardHistoryStore clipboardHistory;
    boolean clipboardHistoryEnabled;
    CloudClipboardPanelPolicy.Tab clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
    CloudClipboardPanelPolicy.Status cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    java.util.List<BackendAccount.ClipboardItem> cloudClipboardItems = java.util.List.of();
    // Bumped whenever the field or the open panel changes; a cloud answer started under an older value is dropped rather than drawn into a field it was not fetched for.
    long cloudClipboardGeneration;
    private boolean candidateEnglishGloss;
    private boolean candidateTranslationsEnabled;
    // Gates only the account network path (fetch, apply, reserved rows); candidateTranslationsEnabled stays the display gate for translations a candidate already carries.
    private boolean candidateTranslationAccount;
    private boolean englishSuggestionsEnabled = true;
    private java.util.List<String> candidateTranslationTargets = java.util.List.of("en");
    private CandidateTranslationStore candidateTranslationStore;
    private boolean wubiCodeHint = true;
    private boolean wubiMixedPinyin;
    // 五笔版本只决定方案卡片和工具栏上的「86」「98」字样；选表由引擎按同一份偏好里的 `wubi_profile` 决定。
    String wubiProfile = KeyboardScheme.WUBI_86;
    private String candidateGlossResources = "";
    // 会话的 `preferences_directory`（state_root）：随包没有的非英文离线释义从它下面下载的 offline-glosses 资源包里读。
    private String candidateGlossStateRoot = "";
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
    int candidateFontSize = 16;
    int candidatePreeditFontSize = 16;
    CandidateAppearance.Palette candidateAppearance =
        CandidateAppearance.fromSkin(KeyboardSkin.system(false));
    int touchKeySpacingTenths = KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS;
    int touchRowSpacingTenths = KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS;
    int touchKeyboardHeightAdjustment = KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_DP;
    private boolean touchVoiceShortcutEnabled;
    private boolean voiceInputEnabled = true;
    private String voiceLanguage = "zh-CN";
    KeyboardSkin skin = KeyboardSkin.system(false);
    /** 表情面板有自己的明暗设置，跟随时才继承键盘皮肤解析出的明暗。 */
    KeyboardSkin emojiSkin = KeyboardSkin.system(false);
    KeyboardSkin handwritingSkin = KeyboardSkin.system(false);
    /** The shared theme catalog (`msime_client_theme_catalog`), read once: ids, titles and palettes are fixed per build. */
    JSONArray themeCatalog;
    /** Resolved keyboards by request, so the four surfaces of one snapshot cost at most two native calls. */
    private final java.util.Map<String, KeyboardSkin> resolvedThemes = new java.util.HashMap<>(8);
    /** 输入法自己写进编辑器后预期的选区，用来认出 `onUpdateSelection` 里迟到的回声。 */
    final SelectionEchoTracker selectionEcho = new SelectionEchoTracker();
    private JSONObject localModes = new JSONObject();
    Button moreButton;
    Button schemeButton;
    Button skinButton;
    Button layoutSettingsButton;
    Button scriptShortcutButton;
    Button emojiShortcutButton;
    Button voiceShortcutButton;
    Button aiPolishShortcutButton;
    Button replyShortcutButton;
    Button phraseShortcutButton;
    Button clipboardShortcutButton;
    Button dismissShortcutButton;
    /** 顶部一行里组词时的读音行与候选行。 */
    LinearLayout candidateHeader;
    LinearLayout candidateLine;
    /** 内联键盘高度条；进入时记下原值，取消时恢复。 */
    InlineHeightBar inlineHeightBar;
    boolean inlineHeightActive;
    private int inlineHeightOriginal;
    /** 功能面板（新设计的菜单）与它的「AI 回复与润色」分段页。 */
    FunctionPanelView functionPanel;
    boolean aiAssistChooserOpen;
    /** 常用语面板。 */
    ScrollView phraseScroll;
    LinearLayout phrasePanel;
    /** Android 本地设置（{@link AndroidLocalSettings}）：单手、隐私、按键细节、手写、工具栏的常用语/输入方式/隐藏等不在共享偏好里的设置。onStartInputView 与每次偏好生效时按文件身份重读。 */
    AndroidLocalSettings.Snapshot localSettings = AndroidLocalSettings.defaults();
    /** `touch_toolbar` 的各个按钮开关；常用语、输入方式与「显示方式：隐藏」来自本地设置。 */
    boolean toolbarEmoji = true;
    boolean toolbarPhrase = true;
    boolean toolbarClipboard = true;
    boolean toolbarSkin = true;
    boolean toolbarScheme = true;
    boolean toolbarHidden;
    /** 功能面板上直接切换的三项：模糊音（共享偏好）、单手模式（off / left / right）与隐私模式（本地设置）。 */
    boolean fuzzyPinyinEnabled;
    String oneHandedMode = "off";
    /** 本地设置「横屏分离式键盘」；实际画不画还要看设备形态、方向和布局，见 {@link #splitKeyboardDrawn}。 */
    boolean splitKeyboardEnabled;
    boolean incognitoEnabled;
    boolean panelPreferenceSaving;
    Button microsoftFinalKey;
    final java.util.List<ShuangpinHintButton> shuangpinKeyButtons =
        new java.util.ArrayList<>(27);
    final java.util.List<String> shuangpinKeyInputs = new java.util.ArrayList<>(27);
    private String shuangpinHintsProfile = "";
    private java.util.Map<String, String> shuangpinHints = java.util.Map.of();
    /** 本包所属的版本：键盘只列出本版本提供的方案入口，偏好里的方案本版本没有时回退到本版本的默认方案。 */
    private final AppEdition edition = AppEdition.current();
    /** 空闲时候选栏左侧显示的产品名，取自本版本的应用名（full 是「水杉输入法」，五笔版是「水杉五笔」）。 */
    private String productName = "";
    KeyboardScheme selectedScheme = KeyboardScheme.fallback(edition);
    private java.util.List<KeyboardScheme> enabledSchemes =
        KeyboardScheme.enabledFromPreferenceIds(null, edition);
    // The schemes the picker offers: `enabledSchemes` without those whose dictionary `languageDictionaries` lacks. `enabledSchemes` stays the stored list, so a picker save does not drop a scheme the user turned on before its dictionary arrived.
    java.util.List<KeyboardScheme> visibleSchemes = enabledSchemes;
    // The runtime options' `language_dictionaries` directory, read with them in onStartInput; empty when the configuration names none.
    private String languageDictionaries = "";
    // 本机已具备的方案资源包（日文词典、语言词库），在 onStartInput 里和运行时选项一起重读：设置页下载完后，下一次进入输入框切换器就出现新语言，与 host-api 获得焦点时刷新资源包的时机一致。
    private java.util.Set<String> resourcePacks = java.util.Set.of();
    boolean soundEnabled = true;
    boolean hapticsEnabled;
    KeyboardFeedbackPreferences.HapticStrength hapticStrength =
        KeyboardFeedbackPreferences.HapticStrength.MEDIUM;
    Vibrator vibrator;
    LinearLayout keyRows;
    /** The fixed bottom row; {@link KeyboardActionRow} decides what it carries. */
    LinearLayout actionRow;
    Button globeButton;
    Button deleteButton;
    View nineKeySidebar;
    /** 笔画网格的通配键：只在组字中可用，render 时按组字状态更新。 */
    Button strokeWildcardKey;
    String actionRowSignature = "";
    boolean brandPillVisible;
    JapaneseFlickPreview japaneseFlickPreview;
    LinearLayout shortcutBar;
    HorizontalScrollView shortcutScroll;
    final java.util.List<Button> symbolKeyButtons = new java.util.ArrayList<>(30);
    final java.util.List<String> symbolKeyInputs = new java.util.ArrayList<>(30);
    HandwritingCanvas handwritingCanvas;
    private LinearLayout handwritingCandidates;
    TextView handwritingStatus;
    Button handwritingDownload;
    HandwritingRecognizer handwritingRecognizer;
    private Runnable handwritingRecognitionTask;
    private Runnable handwritingAvailabilityTask;
    private boolean handwritingDownloading;
    private java.util.List<String> handwritingResults = java.util.List.of();
    private HandwritingRequestTracker.Token handwritingCandidateToken;
    private final HandwritingRequestTracker handwritingRequests = new HandwritingRequestTracker();
    final SpaceCursorMovement cursorMovement = new SpaceCursorMovement();
    Button layerButton;
    Button symbolPanelButton;
    Button quickPunctuationButton;
    Button shiftButton;
    Button languageButton;
    Button enterButton;
    Button spaceButton;
    Button japaneseSpaceKey;
    Button japaneseReturnKey;
    Button japaneseSymbolsKey;
    Button japaneseVariantsButton;
    private Integer japaneseConversionIndex;
    private String japaneseConversionEditingText = "";
    TextView status;
    TextView diagnosticView;
    private String message = "";
    String diagnosticMessage = "";
    long diagnosticGeneration;
    Runnable diagnosticDismissTask;
    final EnglishLetterCaseState letterCase = new EnglishLetterCaseState();
    boolean dedicatedEnglish;
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
    boolean fullWidthInput;
    /** The shared preference this session started from, so only a change to it overrides the toggle. */
    private boolean fullWidthPreference;
    /** Mirrors the runtime's Chinese/English punctuation state; the card and the chord move it. */
    boolean chinesePunctuation = true;
    private boolean chinesePunctuationPreference = true;
    boolean traditionalChineseOutput;
    int editorInputType;
    private long currentDocumentIdentifier;
    private long nextDocumentIdentifier = 1;
    private final KeyboardInputContext inputContext = new KeyboardInputContext();
    KeyboardLayout.Layer keyboardLayer = KeyboardLayout.Layer.LETTERS;
    boolean allowLearning;
    private String preferencesNotice = "";
    /** 上次以 Toast 说过的空闲提示，同一条不重复弹。 */
    private String announcedIdleNotice = "";

    /** 空闲时的临时提示不占状态行；其中的失败（失败、未能、无法）以 Toast 告诉用户。进度、成功和「稍后重试」这类会自己恢复的提示不弹，免得每次弹出键盘都冒一条。 */
    private void announceIdleNotice(String notice) {
        String text = notice.startsWith(" · ") ? notice.substring(3) : notice;
        if (text.equals(announcedIdleNotice)) return;
        announcedIdleNotice = text;
        if (text.contains("失败") || text.contains("未能") || text.contains("无法")) {
            Toast.makeText(this, text, Toast.LENGTH_SHORT).show();
        }
    }
    String preferencesDirectory = "";
    private long appearanceLoadGeneration;
    private String runtimeOptionsForSnapshot = "";
    /** 本输入框的运行时选项在强制关闭学习之前的原样，隐私模式切换时据此重建会话。 */
    private String runtimeOptionsBase = "";
    JSONObject preferencesSnapshot;
    private long preferenceSaveGeneration;
    /** 功能面板本地设置（单手、隐私）自己的写入代数，与几何、方案、繁体那些偏好保存互不作废，否则一方的 saving 标志会卡住。 */
    private long localSettingSaveGeneration;
    boolean schemeSaving;
    boolean touchGeometrySaving;
    private boolean skinSaving;
    boolean traditionalOutputSaving;
    static final class KeyboardHeightRole {
        final int baseHeight;
        final int rowCount;
        final int rowIndex;
        final boolean includesRowSpacing;
        /** 要加上的行距份数：一行键加一份；一个占三行的九键块加三份。 */
        final int rowSpacings;

        KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex,
                           boolean includesRowSpacing) {
            this(baseHeight, rowCount, rowIndex, includesRowSpacing ? 1 : 0);
        }

        KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex, int rowSpacings) {
            this.baseHeight = baseHeight;
            this.rowCount = rowCount;
            this.rowIndex = rowIndex;
            this.includesRowSpacing = rowSpacings > 0;
            this.rowSpacings = rowSpacings;
        }
    }
    private ScrollView voiceResultScroll;
    LinearLayout voiceResultPanel;
    private VoiceResultStore voiceResultStore;
    private VoiceResultStore.Entry voiceResultEntry;
    private EditorContextSnapshot voiceTarget;
    private ScrollView aiPolishScroll;
    LinearLayout aiPolishContainer;
    LinearLayout aiPolishPanel;
    LinearLayout aiPolishActions;
    AiPolishConfiguration aiPolishConfiguration;
    AiPolishConfiguration aiRequestConfiguration;
    AiPolishClient.Operation aiOperation;
    EditorContextSnapshot aiTarget;
    String aiSourceText = "";
    String aiOutputText = "";
    String aiError = "";
    boolean aiBusy;
    LinearLayout replyKeyboard;
    LinearLayout replyMain;
    LinearLayout replyActions;
    TextView replyStatus;
    Button replyReplyModeButton;
    Button replyPolishModeButton;
    Button replySourceButton;
    Button replyTemplateButton;
    /** 回复面板里不随皮肤遍历自动上色的部件：分段控件、源文字卡片、行内「粘贴」、底部进度与「选风格」。 */
    LinearLayout replyHeader;
    LinearLayout replyModeControl;
    LinearLayout replySourceCard;
    LinearLayout replyBody;
    ScrollView replyScroll;
    Button replyPasteButton;
    android.widget.ProgressBar replyProgress;
    Button replyStyleResetButton;
    /** 右侧操作列里当前的主操作（生成或换一句），用强调色画；忙碌时的「取消」不是主操作，为 null。 */
    Button replyPrimaryAction;
    CommunityReplyLibrary communityReplyLibrary;
    final ReplyKeyboardModel replyModel = new ReplyKeyboardModel();
    AiPolishClient.Operation replyOperation;
    EditorContextSnapshot replyTarget;
    AiPolishConfiguration replyRequestConfiguration;
    /** 高情商回复面板是否打开。它是工具栏「回复」按钮开关的工具面板，与当前输入方案无关；关闭后回到原来的键盘。 */
    boolean replyOpen;
    private boolean statisticsFailureReported;
    /** Key presses since the last write, per key and local day. Main thread only. */
    private final KeyPressBatch keyPresses = new KeyPressBatch();
    /** Each soft key's heatmap id. Weak, so the keys of a rebuilt row leave with it. */
    private final java.util.Map<View, String> keyIds = new java.util.WeakHashMap<>();
    /** The store's statistics switch as last read; until it has been read, nothing is counted. */
    boolean keyStatisticsEnabled;
    /** A password or no-learning field, where no key press is counted. */
    boolean keyStatisticsExcluded = true;
    private String keyStatisticsDirectory = "";
    private long keyStatisticsGeneration;
    private Runnable keyPressFlushTask;
    long editorContextRevision;
    /** Editor-owned smart-punctuation snapshots; never persisted or sent to the UI. */
    private JSONObject smartRepeatSnapshot;
    private JSONObject smartSpaceSnapshot;
    SharedPreferences emojiPreferences;
    java.util.List<String> emojiRecents = java.util.List.of();
    java.util.List<EmojiCatalogModel.Item> emojiItems = java.util.List.of();
    String emojiResources = "";
    SymbolPanelView symbolPanel;
    int emojiSelectedCategory = Integer.MIN_VALUE;
    private int emojiNextOffset;
    private boolean emojiComplete;
    boolean emojiLoading;
    private long emojiLoadGeneration;
    final Handler main = new Handler(Looper.getMainLooper());
    Runnable backspaceRepeatTask;
    Button backspaceRepeatButton;
    boolean backspaceRepeated;
    boolean backspaceClearedComposition;
    private long personalDictionarySyncGeneration;
    private Runnable personalDictionarySyncTask;
    /** 用户每复制一次就记进本机剪贴板历史；只在本服务（当前默认输入法）存活期间监听，关掉剪贴板历史或命中隐私规则时什么也不记。 */
    private final ClipboardManager.OnPrimaryClipChangedListener clipboardWatcher = () -> captureClipboard(false);
    private long engineStartGeneration;
    private Runnable inputViewRefreshTask;
    final ExecutorService preferencesWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService typingStatisticsWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(32),
        new ThreadPoolExecutor.AbortPolicy());
    private final ExecutorService emojiWorker = Executors.newSingleThreadExecutor();
    // 只在 `emojiWorker` 线程上使用；`hasGlyph` 会走系统字体回退链，能判断当前设备能否画出某个表情。
    private final Paint emojiGlyphPaint = new Paint();
    final ExecutorService cloudClipboardWorker = Executors.newSingleThreadExecutor();
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
    final AiPolishClient aiPolishClient = new AiPolishClient(new AiPolishHttpTransport());
    private final PreferencesReloader preferencesReloader = new PreferencesReloader(
        (task, delay) -> main.postDelayed(task, delay), preferencesWorker, NativeClient::loadPreferences);

    /** 候选区标题行（品牌标记、预编辑、状态、页码、展开）的固定高度。 */
    static final int CANDIDATE_HEADER_HEIGHT_DP = 34;

    private record SchemeConfiguration(
        java.util.List<KeyboardScheme> enabled, java.util.List<KeyboardScheme> visible,
        KeyboardScheme selected) {}

    record SkinChoice(String id, String title, KeyboardSkin skin, JSONObject design) {}

    private SchemeConfiguration schemeConfiguration(
            JSONObject preferences, KeyboardScheme engineScheme) {
        // client-core 总是把 `touch_keyboard_schemes` 写进它给出的文档；缺少这个对象或 `enabled` 列表的只有默认值改成只有中文之前的版本写的文档，按那时的默认方案、没有选中项读（见 `KeyboardScheme.optIn`）。
        JSONObject shared = preferences == null ? null
            : preferences.optJSONObject("touch_keyboard_schemes");
        JSONArray values = shared == null ? null : shared.optJSONArray("enabled");
        java.util.List<String> ids = null;
        if (values != null) {
            ids = new java.util.ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                String value = values.isNull(index) ? null : values.optString(index, null);
                if (value != null) ids.add(value);
            }
        }
        java.util.List<KeyboardScheme> enabled = KeyboardScheme.enabledFromPreferenceIds(ids, edition);
        // 切换器列出和设置 → 输入相同的方案：所有词典已安装的方案。Android 上没有启用开关，只按 `enabled` 过滤会让粤拼、注音、越南语这些默认不启用的方案在键盘上永远找不到。词典缺失的方案照旧不列，host-api 也会从它回退。
        java.util.List<KeyboardScheme> visible =
            KeyboardScheme.installedOf(java.util.List.of(KeyboardScheme.values()), languageDictionaries, resourcePacks, edition);
        String selected = shared == null || shared.isNull("selected")
            ? null : shared.optString("selected", null);
        return new SchemeConfiguration(enabled, visible,
            KeyboardScheme.resolveEnabledSelection(engineScheme, selected, visible, edition));
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
        applyEditorPreferences(preferences, true);
    }

    /**
     * @param appearance 是否用这份偏好重算皮肤。runtime-options.json 里的偏好是宿主早先准备时写下的副本，主题字段可能已经过时（例如仍是默认的薄荷设计）；用它重算会把按上次皮肤画好的第一帧刷成旧配色，一两秒后真正的偏好到了又换回来。那条路径传 false，皮肤只认启动缓存和真正读到的偏好。
     */
    private void applyEditorPreferences(JSONObject preferences, boolean appearance) throws JSONException {
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
            preferences == null ? edition.defaultScheme()
                : preferences.optString("scheme", edition.defaultScheme()),
            preferences == null ? "xiaohe" : preferences.optString("shuangpin_profile", "xiaohe"),
            preferences == null ? "twenty_six_key"
                : preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        SchemeConfiguration schemeConfiguration = schemeConfiguration(preferences, engineScheme);
        alignEngineSchemeWithSelection(preferences, engineScheme, schemeConfiguration);
        enabledSchemes = schemeConfiguration.enabled();
        visibleSchemes = schemeConfiguration.visible();
        selectedScheme = schemeConfiguration.selected();
        // 只用真正读到的偏好重算皮肤：runtime-options.json 的副本（appearance 为假）和缺主题字段的偏好都保留当前皮肤，也就是 onCreate 按上次换上的皮肤画好的那一份。
        if (appearance && preferences != null && preferences.has("global_theme")) {
            skin = keyboardSkin(preferences);
            emojiSkin = surfaceSkin(preferences, "emoji_theme");
            handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
            rememberSkinHint(preferences);
        }
        localModes = preferences == null ? new JSONObject()
            : preferences.optJSONObject("local_modes");
        if (localModes == null) localModes = new JSONObject();
        applyCandidateAppearance(preferences);
        applyTouchGeometry(preferences);
        applyToolbarPreferences(preferences);
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
        wubiProfile = KeyboardScheme.normalizedWubiProfile(
            preferences == null ? null : preferences.optString("wubi_profile", KeyboardScheme.WUBI_86));
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
        if (preferences == null || configuration.selected() == engineScheme) return;
        KeyboardScheme.PreferenceMapping mapping = KeyboardScheme.mappingForRuntimeSelection(
            engineScheme, configuration.selected(),
            preferences.optString("last_chinese_scheme",
                preferences.optString("scheme", edition.defaultScheme())),
            preferences.optString("shuangpin_profile", "xiaohe"), edition);
        preferences.put("scheme", mapping.scheme());
        preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
        preferences.put("shuangpin_profile", mapping.shuangpinProfile());
        preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
    }

    private JSONObject value(String response) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new JSONException("Shared runtime rejected operation");
        return envelope.getJSONObject("value");
    }

    /** Read a native protocol flag without accepting org.json's string coercion. */
    private static boolean strictBoolean(JSONObject object, String key) throws JSONException {
        Object value = object.opt(key);
        if (!(value instanceof Boolean)) throw new JSONException("Invalid boolean: " + key);
        return (Boolean) value;
    }

    private EditorBridge.Sink sink(TypingSource source) {
        final InputConnection target = connection;
        return new EditorBridge.Sink() {
            public void begin() { target.beginBatchEdit(); }
            public boolean commit(String text) {
                boolean committed = target.commitText(text, 1);
                if (committed) {
                    selectionEcho.commit(text.length());
                    recordTypingStatistics(text, source);
                } else {
                    selectionEcho.invalidate();
                }
                return committed;
            }
            public boolean compose(String text) {
                boolean composed = target.setComposingText(text, 1);
                if (composed) selectionEcho.compose(text.length());
                else selectionEcho.invalidate();
                return composed;
            }
            public boolean finish() {
                boolean finished = target.finishComposingText();
                if (finished) selectionEcho.finish();
                else selectionEcho.invalidate();
                return finished;
            }
            public boolean select(int start, int end) {
                // 恢复用户点选的位置之后，回报的选区由编辑器决定，这里不去预测，退回到旧的保守行为。
                selectionEcho.invalidate();
                return target.setSelection(start, end);
            }
            public void end() {
                target.endBatchEdit();
                // 编辑器在批量编辑结束后才回报一次选区，所以整批写完再记预期。
                selectionEcho.expect();
            }
        };
    }

    private TypingSource typingSource() {
        return TypingSource.resolve(selectedScheme, dedicatedEnglish,
            view == null || view.isNull("local_mode") ? null : view.optString("local_mode", null));
    }

    private String typingStatisticsDirectory() {
        if (!preferencesDirectory.isEmpty()) return preferencesDirectory;
        File files = getFilesDir();
        return files == null ? "" : HostOptionsPolicy.bootstrapStateDirectory(files);
    }

    private void recordTypingStatistics(String text, TypingSource source) {
        String directory = typingStatisticsDirectory();
        if (!ImePrivacyGate.recordsTyping(directory, text)) return;
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
        submitTypingStatistics(request, null, engineStartGeneration);
    }

    /**
     * Send one statistics request on the worker; {@code nothingRecorded}, when given, runs on the main thread if the store took none of a non-empty batch, which is how it answers once statistics are off.
     */
    private void submitTypingStatistics(String request, Runnable nothingRecorded,
                                        long lifecycleGeneration) {
        try {
            typingStatisticsWorker.execute(() -> {
                try {
                    JSONObject result = new JSONObject(NativeClient.typingStatistics(request));
                    if (!JsonPolicy.strictTrue(result.opt("ok"))) {
                        reportTypingStatisticsFailure(lifecycleGeneration);
                    } else if (nothingRecorded != null) {
                        JSONObject value = result.getJSONObject("value");
                        long recorded = KeyboardGeometry.strictLong(value.opt("recorded"), -1);
                        if (recorded < 0) throw new JSONException("Invalid recorded count");
                        if (recorded == 0) main.post(nothingRecorded);
                    }
                } catch (Exception | LinkageError error) {
                    reportTypingStatisticsFailure(lifecycleGeneration);
                }
            });
        } catch (RuntimeException error) {
            reportTypingStatisticsFailure(lifecycleGeneration);
        }
    }

    /**
     * Where key counts go: the preferences directory the settings page reads them back from, or the bootstrap state root when there is none, in the order HostStore.statisticsDirectory reads them.
     */
    private String keyStatisticsDirectory(String preferences) {
        if (!preferences.isEmpty() && new File(preferences).isAbsolute()) return preferences;
        File files = getFilesDir();
        return files == null ? "" : HostOptionsPolicy.bootstrapStateDirectory(files);
    }

    /**
     * Decide whether this editor's key presses are counted, and read the statistics switch again.
     *
     * <p>The switch lives in the shared store, which the settings app writes from another process, so it is read on the worker when a new editor starts rather than per key. Until the answer arrives nothing is buffered: off is the store's default and the user's choice must hold before anything is kept, even in memory.
     */
    private void refreshKeyStatistics(EditorInfo info, boolean restarting, String directory) {
        keyStatisticsExcluded = ImePrivacyGate.excludesKeyStatistics(info);
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
    <T extends View> T keyId(T key, String id) {
        if (id != null) keyIds.put(key, id);
        return key;
    }

    void countKey(View key) { countKey(keyIds.get(key)); }

    /** 按键的键位 id（`Backspace`、`Space`、`Enter`…）；没有登记的控件为 null。按键反馈靠它给键分类，不去猜无障碍描述。 */
    String keyIdOf(View key) { return key == null ? null : keyIds.get(key); }

    /**
     * Count one key press for the heatmap: its id and its local day, nothing else.
     *
     * <p>Presses are batched here and written by the worker -- on {@link KeyPressBatch#FLUSH_PRESSES} presses, at the first press of a new day (the old day first, under its own date), when the editor or the keyboard goes away, and otherwise half a minute after the batch opened. The worker's queue is short and every write rewrites the whole document under the file lock, so it never sees a single key.
     */
    void countKey(String id) {
        if (id == null || !imePrivacyGate.countsKeys()) return;
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
        }, engineStartGeneration);
    }

    private void reportTypingStatisticsFailure() {
        reportTypingStatisticsFailure(engineStartGeneration);
    }

    private void reportTypingStatisticsFailure(long lifecycleGeneration) {
        main.post(() -> {
            if (!TypingStatisticsLifecyclePolicy.acceptsFailure(
                    lifecycleGeneration, engineStartGeneration)) return;
            if (statisticsFailureReported) return;
            statisticsFailureReported = true;
            preferencesNotice = " · 打字统计未能写入";
            render();
        });
    }

    boolean commitText(String text, TypingSource source) {
        if (connection == null) return false;
        boolean committed;
        try { committed = connection.commitText(text, 1); }
        catch (RuntimeException error) {
            selectionEcho.invalidate();
            return false;
        }
        if (committed) {
            selectionEcho.commit(text.length());
            selectionEcho.expect();
            recordTypingStatistics(text, source);
        } else {
            selectionEcho.invalidate();
        }
        return committed;
    }

    /** 删光标前 `length` 个 UTF-16 单元，并记下删完后的选区预期，免得这次删除迟到的回报把紧接着开始的新组字取消掉。有选区或组字区时删的位置由编辑器决定，追踪器会自己作废预期。 */
    private boolean deleteBeforeCursor(int length) {
        boolean deleted = connection.deleteSurroundingText(length, 0);
        if (deleted) {
            selectionEcho.deleteBefore(length);
            selectionEcho.expect();
        } else {
            selectionEcho.invalidate();
        }
        return deleted;
    }

    /** 按码位删光标前一个字符，同样记下预期；删掉的是一个还是两个 UTF-16 单元由追踪器按回声确定。 */
    void deleteCodePointBeforeCursor() {
        if (connection.deleteSurroundingTextInCodePoints(1, 0)) {
            selectionEcho.deleteCodePointBefore();
            selectionEcho.expect();
        } else {
            selectionEcho.invalidate();
        }
    }

    /** 编辑器动作可能改文字也可能不改，选区预期先作废。 */
    private boolean performEditorAction(int action) {
        selectionEcho.invalidate();
        return connection.performEditorAction(action);
    }

    boolean commitText(String text) { return commitText(text, typingSource()); }

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
        // 只有接下来要建引擎会话的输入框才停掉空闲同步：建会话前 scheduleEngineStartup 会先在同一个工作线程上同步一轮。焦点落到不打字的窗口（桌面、设置页）时系统同样会调这里，原先一律停掉，离开输入框 500 ms 后的空闲同步几乎总被取消，导入词库的词条就一直等不到写入。
        if (info != null && EditorPolicy.useEngine(info.inputType)) cancelPersonalDictionarySynchronization();
        boolean newDocument = !restarting || currentDocumentIdentifier == 0;
        if (newDocument) {
            currentDocumentIdentifier = nextDocumentIdentifier++;
        }
        imeBottomRow.resetSpaceCursor();
        stop(false);
        connection = getCurrentInputConnection();
        selectionEcho.reset(info == null ? -1 : info.initialSelStart,
            info == null ? -1 : info.initialSelEnd);
        ensureCandidateTranslationStore();
        editorContextRevision++;
        clearSmartPunctuationSnapshots();
        bridge = new EditorBridge();
        if (inputModeStore == null) {
            inputModeStore = InputModeStore.from(
                getSharedPreferences(INPUT_MODE_PREFERENCES, MODE_PRIVATE));
        }
        enabledSchemes = KeyboardScheme.enabledFromPreferenceIds(null, edition);
        visibleSchemes = enabledSchemes;
        languageDictionaries = "";
        resourcePacks = java.util.Set.of();
        letterCase.reset();
        clearEnglishSuggestions();
        keyboardLayer = KeyboardLayout.Layer.LETTERS;
        editorInputType = info == null ? 0 : info.inputType;
        currentEditorPackage = info == null || info.packageName == null ? "" : info.packageName;
        allowLearning = info != null && EditorPolicy.allowLearning(info.imeOptions);
        refreshLocalSettings();
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
            JSONObject options = new JSONObject(
                HostOptionsPolicy.readRuntimeOptions(getFilesDir()));
            statisticsPreferences = options.optString("preferences_directory", "");
            languageDictionaries = options.optString("language_dictionaries", "");
            resourcePacks = KeyboardScheme.availablePacks(
                ResourcePacks.installedIds(this)::contains, options.optString("resources", ""));
            JSONObject preferences = options.optJSONObject("preferences");
            applyEditorPreferences(preferences, false);
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
            runtimeOptionsBase = "";
            if (engineWanted) {
                runtimeOptionsBase = options.toString();
                if (learningSuppressed()) options.getJSONObject("preferences").put("learning", false);
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
        imeLetterRows.rebuildKeyRows();
        render();
        synchronizeReplyKeyboard();
    }

    // One reporting session per keyboard process: a crash here is recorded against it, and a session that ends through onDestroy counts as a normal one.
    @Override public void onCreate() {
        // 拆出去的构建与渲染类持有本服务；在 onCreate 里创建，构造期间不把 this 交出去。
        imeToolbar = new ImeToolbar(this);
        imeCandidates = new ImeCandidates(this);
        imeFunctionPanel = new ImeFunctionPanel(this);
        imePanels = new ImePanels(this);
        imeLetterRows = new ImeLetterRows(this);
        imeGlideTyping = new ImeGlideTyping(this);
        imeBottomRow = new ImeBottomRow(this);
        imeLayoutRows = new ImeLayoutRows(this);
        imeStyler = new ImeStyler(this);
        imeFrame = new ImeFrame(this);
        imePrivacyGate = new ImePrivacyGate(this);
        imeVoiceEntry = new ImeVoiceEntry(this);
        imeKeyFeedback = new ImeKeyFeedback(this);
        imeDebugOverlay = new ImeDebugOverlay(this);
        // 必须在 super.onCreate() 之前：InputMethodService 在那里按这个主题建输入法窗口，之后再设会抛异常。按名字查是因为 core/ 要能脱离 Gradle 生成的 R 编译（check-host.sh 的 JVM 冒烟）；res/values/themes.xml 说明了这个主题为什么存在。
        // 五笔、拼音等版本的 applicationId 带后缀，资源表的包名仍是命名空间，两个都试。
        int theme = getResources().getIdentifier("Theme.MSIME.InputMethod", "style", getPackageName());
        if (theme == 0)
            theme = getResources().getIdentifier("Theme.MSIME.InputMethod", "style", "app.msime.android");
        if (theme != 0) setTheme(theme);
        super.onCreate();
        productName = getApplicationInfo().loadLabel(getPackageManager()).toString();
        // 偏好要等引擎准备好才读到；先按上次换上的皮肤画，免得每次弹出键盘都先闪一两秒内置的淡绿配色。
        JSONObject hint = readSkinHint();
        if (hint != null) {
            skin = keyboardSkin(hint);
            emojiSkin = surfaceSkin(hint, "emoji_theme");
            handwritingSkin = surfaceSkin(hint, "handwriting_theme");
        }
        Telemetry.beginInputSession(this);
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        if (clipboard != null) clipboard.addPrimaryClipChangedListener(clipboardWatcher);
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
            markPrivateSession();
            loadFeedbackPreferences();
            refreshLocalSettings();
            refreshPreferencesOnInputView();
            updateAutomaticCapitalization();
            render();
        };
        main.postDelayed(inputViewRefreshTask, INPUT_VIEW_REFRESH_DELAY_MILLIS);
    }

    @Override public void onFinishInput() {
        flushKeyPresses();
        imeLetterRows.cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        cloudClipboardGeneration++;
        imeBottomRow.resetSpaceCursor();
        // 也覆盖 onFinishInputView(true)：那条路径不经过 finishInputViewPresentation。
        imeVoiceEntry.cancel();
        stop(true);
        schedulePersonalDictionarySynchronization(false);
        scheduleDictionarySnapshotProcessing();
        connection = null;
        selectionEcho.reset(-1, -1);
        currentDocumentIdentifier = 0;
        super.onFinishInput();
    }

    @Override public void onFinishInputView(boolean finishingInput) {
        flushKeyPresses();
        cancelInputViewRefresh();
        // 键盘收起时还在调整高度就当作取消：没点「完成」的预览不保存。
        finishInlineHeight(false);
        if (!finishingInput) finishInputViewPresentation();
        super.onFinishInputView(finishingInput);
    }

    /** Match Apple's viewWillDisappear boundary while keeping the editor session alive. */
    private void finishInputViewPresentation() {
        // 键盘收起就停掉键盘内的语音识别：聆听面板挂在已隐藏的窗口上不会被移除，录音（豆包还在往云端推流）会一直持续到上限。
        imeVoiceEntry.cancel();
        imeLetterRows.cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        imeBottomRow.resetSpaceCursor();
        imeLayoutRows.dismissNineKeyHoldOptions();
        imeLayoutRows.hideJapaneseFlickPreview();
        closeCandidatePanel();
        // 工具栏面板一律走 closeToolbarPanels：逐个列举时漏掉过常用语，A 应用里打开的常用语会盖在 B 应用密码框的键盘上，一点就把短语上屏进密码框。
        closeToolbarPanels();
        deactivateHandwriting();
        imeDebugOverlay.clearDiagnostic();
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
        imeLayoutRows.dismissNineKeyHoldOptions();
        imeLayoutRows.hideJapaneseFlickPreview();
        if (keyboardRoot == null) return;
        imeStyler.applyKeyboardSurfaceGeometry();
        if (skinChanged) imeStyler.applySkin();
        imeStyler.applyKeyboardGeometry();
        renderLayoutSettingsState();
        render();
    }
    @Override public void onDestroy() {
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        if (clipboard != null) clipboard.removePrimaryClipChangedListener(clipboardWatcher);
        imeLetterRows.cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        localSettingSaveGeneration++;
        cancelPersonalDictionarySynchronization();
        stop(false);
        schedulePersonalDictionarySynchronization(true);
        preferencesWorker.shutdown();
        // Queued before the shutdown, so the worker still writes it.
        flushKeyPresses();
        typingStatisticsWorker.shutdown();
        emojiWorker.shutdown();
        imeVoiceEntry.shutdown();
        imeKeyFeedback.shutdown();
        imeLayoutRows.shutdown();
        imeDebugOverlay.shutdown();
        cloudClipboardWorker.shutdownNow();
        candidateGlossWorker.shutdownNow();
        candidateTranslationWorker.shutdownNow();
        englishSuggestionWorker.shutdownNow();
        onlineCandidateWorker.shutdownNow();
        aiPolishClient.close();
        connection = null;
        Telemetry.endInputSession(this);
        imePrivacyGate.release();
        super.onDestroy();
    }
    @Override public boolean onEvaluateFullscreenMode() { return false; }

    private void stop(boolean finish) {
        imeLetterRows.cancelBackspaceRepeat();
        imeDebugOverlay.clearDiagnostic();
        clearSmartPunctuationSnapshots();
        deactivateHandwriting();
        invalidateOnlineProviders();
        invalidateCandidateGlosses();
        preferencesReloader.stop();
        preferenceSaveGeneration++;
        preferencesDirectory = "";
        emojiResources = "";
        candidateGlossResources = "";
        candidateGlossStateRoot = "";
        candidateEnglishGloss = false;
        candidateTranslationsEnabled = false;
        candidateTranslationAccount = false;
        candidateTranslationTargets = java.util.List.of("en");
        clearEnglishSuggestions();
        if (candidateTranslationStore != null) candidateTranslationStore.clear();
        wubiCodeHint = true;
        wubiMixedPinyin = false;
        wubiProfile = KeyboardScheme.WUBI_86;
        preferencesSnapshot = null;
        schemeSaving = false;
        touchGeometrySaving = false;
        panelPreferenceSaving = false;
        skinSaving = false;
        traditionalOutputSaving = false;
        if (session != 0) {
            try { if (finish && connection != null) apply(NativeClient.command(session, 2)); }
            catch (Exception | LinkageError ignored) { /* Never log editor text or native responses. */ }
            try { NativeClient.destroy(session); } catch (LinkageError ignored) { }
            session = 0;
        }
        // A Stroke composition still marked here (the session stopped without CommitRaw) is stroke glyphs, not text: remove it rather than finish it into the document.
        if (connection != null && strokeCompositionMarked()) bridge.discard(sink(typingSource()));
        else if (connection != null) bridge.abandon(sink(typingSource()));
        view = null;
        closeCandidatePanel();
        // 换编辑器时不让任何工具栏面板（含常用语）活下来，理由同 finishInputViewPresentation。
        closeToolbarPanels();
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
        boolean suppressLearning = learningSuppressed();
        try {
            preferencesWorker.execute(() -> {
                String startOptions = withLivePreferences(options, suppressLearning);
                String notice = "";
                try {
                    JSONObject sync = value(NativeClient.personalDictionarySync(options));
                    if (sync.optString("snapshot_error", "").length() > 0) {
                        notice = " · 个人词库同步稍后重试";
                    }
                    // 刚处理完的这批腾出了队列，把导入词库的下一批送进去，键盘收起后的空闲同步会接着写完。
                    DictionaryCollectionsStore.flushSent(this);
                } catch (Exception | LinkageError ignored) {
                    // Personal dictionary maintenance is optional; session startup continues.
                }
                String finalNotice = notice;
                main.post(() -> {
                    if (generation != engineStartGeneration || connection == null || session != 0) return;
                    if (!finalNotice.isEmpty()) preferencesNotice = finalNotice;
                    startEngineSession(startOptions);
                });
            });
        } catch (RuntimeException ignored) {
            // A worker shutdown must not leave a still-valid editor without its session.
            main.post(complete);
        }
    }

    /**
     * runtime-options.json 里的偏好是首次安装时写下的出厂默认（见 Bootstrap.prepare），拿它建会话，引擎先按默认方案（全拼 26 键）起来，过一两秒实时偏好重载后才换成用户的方案，九键用户每次都看到键盘从 26 键跳成九键。建会话前在工作线程上读一次实时偏好换进去；读不到时照旧用原来那份。不允许学习的输入框照样把 learning 关掉。
     */
    private static String withLivePreferences(String optionsText, boolean suppressLearning) {
        try {
            JSONObject options = new JSONObject(optionsText);
            String directory = options.optString("preferences_directory", "");
            if (directory.isEmpty() || !new File(directory).isAbsolute()) return optionsText;
            JSONObject envelope = new JSONObject(NativeClient.loadPreferences(directory));
            JSONObject live = JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.getJSONObject("value").optJSONObject("preferences") : null;
            if (live == null) return optionsText;
            if (suppressLearning) live.put("learning", false);
            options.put("preferences", live);
            return options.toString();
        } catch (JSONException | RuntimeException | LinkageError error) {
            return optionsText;
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
            session = strictCandidateLong(view, "session");
            String resources = options.optString("resources", "");
            if (new File(resources).isAbsolute()) {
                emojiResources = resources;
                candidateGlossResources = resources;
            }
            String stateRoot = options.optString("preferences_directory", "");
            candidateGlossStateRoot = new File(stateRoot).isAbsolute() ? stateRoot : "";
            apply(NativeClient.focus(session, true));
            markPrivateSession();
            view = value(NativeClient.setEnglishMode(session, dedicatedEnglish));
            applyCharacterWidth(fullWidthPreference);
            applyChinesePunctuation(chinesePunctuationPreference);
            // The rows were drawn before the session existed, from no view at all; a scheme with its own surface (the Korean keycaps) has to replace them now that the view says which one applies.
            if (displayedTouchLayout(view) != drawnLayout) {
                letterCase.reset();
                imeLetterRows.rebuildKeyRows();
            }
            refreshEnglishSuggestions();
            // 先清掉「准备中」再画，否则这次 render 还会把过期的模式标签留在读音行上。
            message = "";
            render();
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
                    int queued;
                    try {
                        queued = value(NativeClient.personalDictionarySync(options)).optInt("pending_count", 0);
                    } catch (Exception | LinkageError ignored) {
                        // The next idle boundary retries a busy or unavailable journal.
                        return;
                    }
                    // 个人词库每同步一次只写 4 条，导入的词库又按队列空位分批送进来。这里把下一批送进去，只要还有没写完的（队列里还有，或者刚送进了新的）就马上再来一轮，直到全部写完。每一轮都回到主线程重新确认此刻没有输入会话：键盘一弹出就停，下次空闲时接着写。
                    DictionaryCollectionsStore.Result<Integer> sent = DictionaryCollectionsStore.flushSent(this);
                    if (queued > 0 || (sent.value() != null && sent.value() > 0))
                        main.post(() -> schedulePersonalDictionarySynchronization(true));
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
        candidateAppearance = candidateAppearanceFor(preferences);
        if (preferences == null) {
            candidateHorizontal = true;
            candidateFontSize = 16;
            candidatePreeditFontSize = 16;
            return;
        }
        // 共享的 `candidate_layout` 是桌面候选窗的横排/竖排，默认竖排；触屏候选条只有一行高，竖排时每个候选占满整行往下排，一屏只露出第一个。和 iOS 一样，触屏键盘始终横排，更多候选在展开面板里看。
        candidateHorizontal = true;
        candidateFontSize = CandidateAppearance.fontSize(
            KeyboardGeometry.strictInt(preferences, "candidate_font_size", 16));
        candidatePreeditFontSize = CandidateAppearance.fontSize(
            KeyboardGeometry.strictInt(preferences, "candidate_preedit_font_size", candidateFontSize));
    }

    private void applyTouchGeometry(JSONObject preferences) {
        touchKeySpacingTenths = KeyboardGeometry.keySpacing(preferences == null ? -1
            : KeyboardGeometry.strictInt(preferences, "touch_key_spacing_tenths", -1));
        touchRowSpacingTenths = KeyboardGeometry.rowSpacing(preferences == null ? -1
            : KeyboardGeometry.strictInt(preferences, "touch_row_spacing_tenths", -1));
        touchKeyboardHeightAdjustment = heightAdjustmentFrom(preferences);
        touchVoiceShortcutEnabled = preferences != null
            && preferences.optBoolean("touch_voice_shortcut", false);
    }

    /** 日语九键侧列的 ☺：顶部工具栏有表情按钮时两处入口重复，不放；工具栏关掉表情或整条隐藏时才放回来。 */
    boolean japaneseSideEmojiKey() {
        return !toolbarEmoji || toolbarHidden;
    }

    /** `touch_toolbar` 的按钮开关，以及功能面板直接切换的模糊音、单手、隐私三项；缺键时按 Android 的默认值读。 */
    private void applyToolbarPreferences(JSONObject preferences) {
        JSONObject toolbar = preferences == null ? null : preferences.optJSONObject("touch_toolbar");
        toolbarEmoji = toolbar == null || toolbar.optBoolean("emoji", true);
        toolbarClipboard = toolbar == null || toolbar.optBoolean("clipboard", true);
        toolbarSkin = toolbar == null || toolbar.optBoolean("skin", true);
        JSONObject fuzzy = preferences == null ? null : preferences.optJSONObject("fuzzy_pinyin");
        fuzzyPinyinEnabled = fuzzy != null && fuzzy.optBoolean("enabled", false);
        applyLocalSettings();
    }

    /** 重读 Android 本地设置；文件没变时拿到的是缓存。 */
    void refreshLocalSettings() {
        localSettings = AndroidLocalSettings.load(this);
        applyLocalSettings();
    }

    /** 本地设置里由服务直接持有的几项。 */
    private void applyLocalSettings() {
        toolbarPhrase = localSettings.bool(AndroidLocalSettings.TOOLBAR_PHRASE);
        toolbarScheme = localSettings.bool(AndroidLocalSettings.TOOLBAR_SCHEME);
        toolbarHidden = localSettings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN);
        oneHandedMode = localSettings.choice(AndroidLocalSettings.ONE_HANDED);
        splitKeyboardEnabled = localSettings.bool(AndroidLocalSettings.SPLIT_KEYBOARD);
        incognitoEnabled = localSettings.bool(AndroidLocalSettings.INCOGNITO);
    }

    /** 键盘高度：本地设置里有设计范围（-46..55）的值就用它，否则沿用共享偏好的 `touch_keyboard_height_adjustment`（-12..48）。 */
    private int heightAdjustmentFrom(JSONObject preferences) {
        if (localSettings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)) {
            return KeyboardGeometry.designHeightAdjustment(
                localSettings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT));
        }
        return KeyboardGeometry.designHeightAdjustment(preferences == null ? Integer.MIN_VALUE
            : KeyboardGeometry.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
    }

    /** 会话是否不学习：输入框不许个性化学习，或者隐私模式开着。 */
    private boolean learningSuppressed() {
        return !allowLearning || incognitoEnabled;
    }

    /** 把隐私模式和不允许学习的输入框告诉共享层：这样的会话不记选词位置和上屏效率。用户在设置里关掉学习不算，那时统计照常。 */
    private void markPrivateSession() {
        if (session != 0) NativeClient.setPrivateSession(session, learningSuppressed());
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
                String prompt = ai.optString(
                    AiPolishConfiguration.promptSlotKey(ai.optString("prompt_id", "")), "");
                if (TextPolicy.trimmed(prompt).isEmpty()) prompt = AiPolishConfiguration.DEFAULT_PROMPT;
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
            imePanels.renderAiPolish();
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
        String key = candidateGlossResources + "\n" + candidateGlossStateRoot + "\n" + candidateTranslationTargets;
        if (!key.equals(candidateOfflineTargetsKey)) {
            candidateOfflineTargetsKey = key;
            candidateOfflineTargets = CandidateTranslationPolicy.offlineTargets(
                candidateTranslationTargets, candidateGlossResources, candidateGlossStateRoot);
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

    String chineseOutput(String text, JSONObject context) {
        int scheme = context == null
            ? (view == null ? -1 : InputViewValuePolicy.scheme(view, -1))
            : InputViewValuePolicy.scheme(context, -1);
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
        java.util.List<String> previousTranslationTargets = candidateTranslationTargets;
        boolean previousEnglishSuggestions = englishSuggestionsEnabled;
        boolean previousWubiCodeHint = wubiCodeHint;
        boolean previousWubiMixedPinyin = wubiMixedPinyin;
        String previousWubiProfile = wubiProfile;
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
                || !previousWubiProfile.equals(wubiProfile)
                || previousScheme != selectedScheme
                || !previousView.equals(view == null ? "" : view.toString())) render();
    }

    private void applyPreferencesSnapshot(JSONObject snapshot) throws JSONException {
        refreshLocalSettings();
        JSONObject previousPreferences = preferencesSnapshot == null
            ? null : preferencesSnapshot.optJSONObject("preferences");
        boolean previousCloudCandidates = previousPreferences == null
            || previousPreferences.optBoolean("cloud_candidates", true);
        JSONObject previousAiAssistant = previousPreferences == null
            ? null : previousPreferences.optJSONObject("ai_assistant");
        boolean previousAiCandidates = previousAiAssistant != null
            && previousAiAssistant.optBoolean("enabled", false);
        JSONObject accepted = new JSONObject(snapshot.toString());
        long revision = PreferencesRevisionPolicy.read(accepted.opt("revision"), -1);
        if (revision < 0) throw new JSONException("Invalid preferences revision");
        accepted.put("revision", revision);
        JSONObject preferences = accepted.getJSONObject("preferences");
        KeyboardSkin nextSkin = keyboardSkin(preferences);
        JSONObject nextLocalModes = preferences.optJSONObject("local_modes");
        if (nextLocalModes == null) nextLocalModes = new JSONObject();
        CandidateAppearance.Palette nextCandidateAppearance = candidateAppearanceFor(preferences);
        boolean nextHorizontal = true;
        int nextFontSize = CandidateAppearance.fontSize(
            KeyboardGeometry.strictInt(preferences, "candidate_font_size", 16));
        int nextPreeditFontSize = CandidateAppearance.fontSize(
            KeyboardGeometry.strictInt(preferences, "candidate_preedit_font_size", nextFontSize));
        int nextKeySpacing = KeyboardGeometry.keySpacing(
            KeyboardGeometry.strictInt(preferences, "touch_key_spacing_tenths", -1));
        int nextRowSpacing = KeyboardGeometry.rowSpacing(
            KeyboardGeometry.strictInt(preferences, "touch_row_spacing_tenths", -1));
        int nextHeightAdjustment = heightAdjustmentFrom(preferences);
        boolean nextVoiceShortcut = preferences.optBoolean("touch_voice_shortcut", false);
        JSONObject nextVoice = preferences.optJSONObject("voice_input");
        boolean nextVoiceEnabled = nextVoice == null || nextVoice.optBoolean("enabled", true);
        String nextVoiceLanguage = nextVoice == null ? "zh-CN"
            : nextVoice.optString("language", "zh-CN");
        boolean nextClipboard = preferences.optBoolean("clipboard_history", false);
        boolean nextCloudCandidates = preferences.optBoolean("cloud_candidates", true);
        JSONObject nextAiAssistant = preferences.optJSONObject("ai_assistant");
        boolean nextAiCandidates = nextAiAssistant != null
            && nextAiAssistant.optBoolean("enabled", false);
        boolean aiConfigurationChanged = previousAiAssistant == null
            ? nextAiAssistant != null
            : nextAiAssistant == null
                || !previousAiAssistant.toString().equals(nextAiAssistant.toString());
        boolean nextTraditional = preferences.optBoolean("traditional_chinese_output", false);
        boolean nextCandidateGloss = preferences.optBoolean("candidate_english_gloss", true);
        boolean nextCandidateTranslations = preferences.optBoolean("candidate_translations", true);
        boolean nextCandidateTranslationAccount = candidateTranslationAccountFrom(preferences);
        boolean nextEnglishSuggestions = preferences.optBoolean("english_suggestions", true);
        java.util.List<String> nextTranslationTargets = translationTargetsFrom(preferences);
        boolean nextWubiCodeHint = preferences.optBoolean("wubi_code_hint", true);
        boolean nextWubiMixedPinyin = preferences.optBoolean("wubi_mixed_pinyin", false);
        String nextWubiProfile = KeyboardScheme.normalizedWubiProfile(
            preferences.optString("wubi_profile", KeyboardScheme.WUBI_86));
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
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        SchemeConfiguration nextSchemeConfiguration = schemeConfiguration(preferences, nextScheme);
        JSONObject sessionSnapshot = new JSONObject(accepted.toString());
        // Keep the accepted disk snapshot intact while enforcing editor privacy in this session.
        if (learningSuppressed()) sessionSnapshot.getJSONObject("preferences").put("learning", false);
        JSONObject result = value(NativeClient.updatePreferences(session, sessionSnapshot.toString()));
        markPrivateSession();
        boolean geometryChanged = touchKeySpacingTenths != nextKeySpacing
            || touchRowSpacingTenths != nextRowSpacing
            || touchKeyboardHeightAdjustment != nextHeightAdjustment;
        skin = nextSkin;
        rememberSkinHint(preferences);
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
        boolean previousJapaneseEmojiKey = japaneseSideEmojiKey();
        applyToolbarPreferences(preferences);
        boolean japaneseEmojiKeyChanged = previousJapaneseEmojiKey != japaneseSideEmojiKey();
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
        wubiProfile = nextWubiProfile;
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
        boolean translationDisplayChanged = CandidateTranslationPolicy.displayInvalidated(
            candidateEnglishGloss, nextCandidateGloss, candidateTranslationsEnabled,
            nextCandidateTranslations, candidateTranslationAccount,
            nextCandidateTranslationAccount, candidateTranslationTargets,
            nextTranslationTargets);
        boolean rebuildLayout = displayedTouchLayout(view) != displayedTouchLayout(nextView)
            || japaneseEmojiKeyChanged;
        enabledSchemes = nextSchemeConfiguration.enabled();
        visibleSchemes = nextSchemeConfiguration.visible();
        selectedScheme = nextSchemeConfiguration.selected();
        preferencesSnapshot = accepted;
        if (!clipboardHistoryEnabled && clipboardHistory != null) {
            clipboardHistory.clearQuietly();
            // The cloud half does not depend on this switch; only a panel left with nothing to show closes.
            if (imePanels.clipboardPanelOpen() && imePanels.cloudClipboardAllowed()) imePanels.renderClipboardHistory();
            else closeClipboardHistory();
        }
        view = nextView;
        if (previousCloudCandidates && !nextCloudCandidates) clearOnlineProvider(0);
        if (previousAiCandidates && (!nextAiCandidates || aiConfigurationChanged)) {
            clearOnlineProvider(1);
        }
        if (previousCloudCandidates != nextCloudCandidates
                || previousAiCandidates != nextAiCandidates || aiConfigurationChanged) {
            invalidateOnlineProviders();
        }
        if (translationDisplayChanged) {
            long generation = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
            if (generation >= 0) {
                try {
                    JSONObject cleared = value(NativeClient.applyTranslations(
                        session, generation, "[]"));
                    if (CandidateGlossPolicy.isApplied(cleared.opt("applied")))
                        view = cleared.getJSONObject("view");
                } catch (JSONException | RuntimeException | LinkageError ignored) {
                    // 翻译是可选的显示状态，清除失败不能影响正在输入的会话。
                }
            }
        }
        // After the view is in place, because this replaces it with the runtime's answer.
        if (characterWidthChanged) applyCharacterWidth(nextFullWidthPreference);
        if (punctuationChanged) applyChinesePunctuation(nextChinesePunctuation);
        // A Shift latched on the Korean keycaps means a double consonant, so it must not outlive the surface it was set on.
        if (rebuildLayout || !KeyboardLayout.carriesLetterCase(displayedTouchLayout(view)))
            letterCase.reset();
        if (rebuildLayout) imeLetterRows.rebuildKeyRows();
        else if (geometryChanged) imeStyler.applyKeyboardGeometry();
        renderLayoutSettingsState();
        if (voiceResultScroll != null && voiceResultScroll.getVisibility() == View.VISIBLE)
            renderVoiceResult();
        preferencesNotice = strictBoolean(result, "deferred")
            ? " · 设置将在组词结束后应用" : "";
    }

    boolean apply(String response) throws JSONException {
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
        // Korean marks the composing Hangul, not the key letters editing_text holds; a transition may carry the syllable the key finished and the next one together, and the bridge writes the commit first. Zhuyin's editing_text is the Dachen keys too, and it marks the reading (the conversion and the pending bopomofo) by the same rule.
        int nextViewScheme = InputViewValuePolicy.scheme(next, -1);
        boolean nextDedicatedEnglish = InputViewValuePolicy.booleanValue(
            next, "dedicated_english", dedicatedEnglish);
        // 笔画的 editing_text 是字母 hspnzx，reading 才是用户按下的笔画字形（一丨丿丶乛＊），所以同样标记 reading。日语的 editing_text 是罗马字（九键的 ち 送的是 chi），reading 才是假名。哪些方案这样做由引擎的 `draws_reading` 决定。
        String composing = KoreanInputPolicy.composing(
            InputSchemeTraits.drawsReading(nextViewScheme) && !nextDedicatedEnglish,
            next.optString("phrase_prefix", ""), next.getString("editing_text"),
            next.optString("reading", ""));
        // 九键的 editing_text 是按下的数字键（64426），写进输入框对用户没有意义；和 iOS 默认一样不在输入框里标记组词，组词只显示在键盘自己的预编辑栏上（选过的音节显示为拼音，如 ni'426）。注音 9 键例外：上面已经按大千的规则标记 reading（转换结果加未完成的数字），照常留在输入框里。
        if (ZhuyinInputPolicy.hidesNineKeyComposing(InputViewValuePolicy.booleanValue(
                next, "nine_key", false),
                ZhuyinInputPolicy.active(nextViewScheme, nextDedicatedEnglish))) composing = "";
        if (connection != null
                && !bridge.apply(sink(typingSource()), commit, composing)) {
            throw new JSONException("Editor rejected update");
        }
        view = next;
        imeDebugOverlay.showDiagnostic(result.isNull("diagnostic") ? null
            : result.optString("diagnostic", ""));
        if (rebuildLayout || !KeyboardLayout.carriesLetterCase(displayedTouchLayout(view)))
            letterCase.reset();
        if (rebuildLayout) imeLetterRows.rebuildKeyRows();
        render();
        return strictBoolean(result, "handled");
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
            if (!deleteBeforeCursor(replacement.deleteCount())) return;
            if (!commitText(fullWidthOutput(replacement.insert()))) return;
        } finally {
            connection.endBatchEdit();
        }
        clearEnglishSuggestions();
        render();
    }

    private Button makeEnglishSuggestionButton(int slot) {
        Button button = new KeyboardPressButton(this);
        ViewPolicy.setAllCapsFalse(button);
        button.setOnClickListener(ignored -> {
            imeKeyFeedback.playFeedback(button);
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
            ViewPolicy.show(button);
            button.setText(text);
            KeyboardGeometry.setKeyTextSize(button, candidateFontSize);
            button.setContentDescription("英文建议 " + (slot + 1) + "：" + text);
            imeStyler.styleButton(button, false);
            button.setTypeface(imeStyler.candidateTypeface());
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = englishSuggestions.size(); slot < englishSuggestionButtons.size(); slot++)
            ViewPolicy.hide(englishSuggestionButtons.get(slot));
    }

    /** Copies Engine candidates on the IME thread, then performs only session-free IO off-thread. */
    private void scheduleCandidateGlosses() {
        if (!candidateEnglishGloss || session == 0 || view == null
                || candidateGlossResources.isEmpty()
                || !schemeShowsGlosses(InputViewValuePolicy.scheme(view, -1))) return;
        long generation = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        if (generation < 0 || (candidateGlossRequestedSession == session
                && candidateGlossRequestedGeneration == generation)) return;
        JSONArray visible = view.optJSONArray("candidates");
        if (visible == null || visible.length() == 0) return;
        final long targetSession = session;
        final long targetEpoch = candidateGlossEpoch;
        final String targetResources = candidateGlossResources;
        final String targetStateRoot = candidateGlossStateRoot.isEmpty() ? null : candidateGlossStateRoot;
        final java.util.List<String> offlineTargets = candidateOfflineTargets();
        final String request;
        final java.util.Map<String, String> targetRequests =
            new java.util.LinkedHashMap<>(offlineTargets.size());
        try {
            JSONObject snapshot = value(NativeClient.allCandidates(targetSession));
            if (CandidateGlossPolicy.strictOr(snapshot.opt("session"), Long.MIN_VALUE) != targetSession
                    || CandidateGlossPolicy.strictOr(snapshot.opt("generation"), -1) != generation) return;
            request = CandidateGlossModel.request(generation,
                snapshot.getJSONArray("candidates"));
            // The account path's scheme gate: a Japanese composition is not glossed into other languages. Korean Hanja rows are, as the shared translation query answers them.
            if ("none".equals(view.optString("local_mode", "none")) && InputViewValuePolicy.scheme(view, -1) != 3) {
                for (String language : offlineTargets) {
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
                        offline = new java.util.HashMap<>(targetRequests.size() + 1);
                        offline.put("en", glossMap(result));
                        for (java.util.Map.Entry<String, String> target : targetRequests.entrySet()) {
                            try {
                                CandidateGlossModel.Result glosses = CandidateGlossModel.decode(
                                    NativeClient.candidateGlosses(target.getValue(), targetResources, targetStateRoot));
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
        long currentGeneration = view == null ? -1
            : CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
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
            if (!CandidateGlossPolicy.isApplied(applied.opt("applied"))) return;
            JSONObject next = applied.getJSONObject("view");
            if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != token.session()
                    || CandidateGlossPolicy.strictOr(next.opt("generation"), -1) != token.generation()) return;
            view = next;
            if (candidatePanelOpen) {
                JSONObject snapshot = value(NativeClient.allCandidates(token.session()));
                if (CandidateGlossPolicy.strictOr(snapshot.opt("session"), Long.MIN_VALUE) == token.session()
                        && CandidateGlossPolicy.strictOr(snapshot.opt("generation"), -1) == token.generation()) {
                    candidatePanelSnapshot = snapshot;
                }
            }
            render();
        } catch (JSONException | RuntimeException | LinkageError error) {
            // Candidate glosses are optional display state; keep the current Engine view.
        }
    }

    /** 粤拼、注音、越南语、藏文和笔画的候选不带任何释义（`shows_glosses`）。它们之前的方案沿用本宿主原有的规则，日语候选仍附英文释义。 */
    private static boolean schemeShowsGlosses(int scheme) {
        return scheme != InputSchemeTraits.CANTONESE && scheme != InputSchemeTraits.ZHUYIN
            && scheme != InputSchemeTraits.VIETNAMESE && scheme != InputSchemeTraits.TIBETAN
            && scheme != InputSchemeTraits.STROKE;
    }

    private void scheduleCandidateTranslations() {
        if (!candidateTranslationAccount || session == 0 || view == null
                || candidateTranslationStore == null
                || !"none".equals(view.optString("local_mode", "none"))) return;
        if (InputViewValuePolicy.scheme(view, -1) == 3 || !schemeShowsGlosses(InputViewValuePolicy.scheme(view, -1))) return;
        JSONArray entries = view.optJSONArray("candidates");
        long generation = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        if (entries == null || entries.length() == 0 || generation < 0) return;
        int candidateCount = Math.min(entries.length(), 32);
        java.util.ArrayList<String> words = new java.util.ArrayList<>(candidateCount);
        for (int index = 0; index < candidateCount; index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null) words.add(candidate.optString("text", ""));
        }
        candidateTranslationStore.refresh(words, candidateTranslationTargets, generation);
    }

    private void applyCandidateTranslations(long generation) {
        if (!candidateTranslationAccount || session == 0 || view == null
                || CandidateGlossPolicy.strictOr(view.opt("generation"), -1) != generation) return;
        JSONArray entries = view.optJSONArray("candidates");
        if (entries == null || entries.length() == 0) return;
        JSONArray translations = mergedCandidateGlosses(generation);
        if (translations.length() == 0) return;
        try {
            JSONObject applied = value(NativeClient.applyTranslations(session, generation,
                translations.toString()));
            if (!CandidateGlossPolicy.isApplied(applied.opt("applied"))) return;
            JSONObject next = applied.getJSONObject("view");
            if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != session
                    || CandidateGlossPolicy.strictOr(next.opt("generation"), -1) != generation) return;
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
        JSONArray entries = view == null ? null : view.optJSONArray("candidates");
        int textCapacity = entries == null ? 0 : Math.min(entries.length(), 32);
        for (java.util.Map<String, String> glosses : offline.values()) textCapacity += glosses.size();
        java.util.LinkedHashSet<String> texts = new java.util.LinkedHashSet<>(textCapacity);
        for (int index = 0; entries != null && index < BoundsPolicy.atMost(entries.length(), 32); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null) texts.add(candidate.optString("text", ""));
        }
        for (java.util.Map<String, String> glosses : offline.values()) texts.addAll(glosses.keySet());
        boolean account = candidateTranslationAccount && candidateTranslationStore != null;
        JSONArray translations = new JSONArray();
        for (String text : texts) {
            java.util.HashMap<String, String> offlineRows =
                new java.util.HashMap<>(candidateTranslationTargets.size());
            java.util.HashMap<String, String> accountRows =
                new java.util.HashMap<>(candidateTranslationTargets.size());
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
        java.util.LinkedHashMap<String, String> glosses = new java.util.LinkedHashMap<>(entries.length());
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
            return JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    private boolean requestsCloud(JSONObject query) {
        return OnlineCandidatePolicy.requestsCloud(
            InputViewValuePolicy.booleanValue(query, "cloud_candidates", false),
            InputViewValuePolicy.booleanValue(query, "cloud_eligible", false));
    }

    private boolean requestsAi(JSONObject query) {
        JSONObject assistant = query.optJSONObject("ai_assistant");
        return OnlineCandidatePolicy.requestsAi(
            InputViewValuePolicy.booleanValue(query, "ai_eligible", false),
            InputViewValuePolicy.booleanValue(assistant, "enabled", false));
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

    /** Remove rows already accepted by a provider before its setting is disabled. */
    private void clearOnlineProvider(int source) {
        if (session == 0) return;
        try {
            JSONObject envelope = new JSONObject(NativeClient.clearOnlineCandidates(session, source));
            if (JsonPolicy.strictTrue(envelope.opt("ok"))) {
                JSONObject next = envelope.optJSONObject("value");
                if (next != null) view = next;
            }
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // Optional provider rows are display state; a failed cleanup must not end the IME.
        }
    }

    /**
     * The candidate texts inside a Chat Completions reply, in provider order.
     *
     * <p>An empty list means the reply supplies nothing, whether it was rejected or simply had no
     * candidates; either way nothing reaches Engine. Bounds and the per-entry rules belong to
     * {@link OnlineCandidatePolicy}, so only the envelope shape is read here.
     */
    private java.util.List<String> aiCandidateTexts(String body) {
        java.util.List<String> texts = java.util.List.of();
        if (!OnlineCandidatePolicy.acceptsAiBody(body)) return texts;
        try {
            JSONObject envelope = new JSONObject(body);
            if (!envelope.isNull("error")) return texts;
            JSONArray choices = envelope.optJSONArray("choices");
            if (choices == null || choices.length() == 0) return texts;
            JSONObject message = choices.getJSONObject(0).optJSONObject("message");
            if (message == null) return texts;
            String content = OnlineCandidatePolicy.strictText(message.opt("content"));
            if (content == null) return texts;
            if (!OnlineCandidatePolicy.acceptsAiContent(content)) return texts;
            JSONArray entries = new JSONObject(content).optJSONArray("candidates");
            if (entries == null) return texts;
            texts = new java.util.ArrayList<>(entries.length());
            for (int index = 0; index < entries.length(); index++) {
                JSONObject entry = entries.optJSONObject(index);
                if (entry != null) {
                    String text = OnlineCandidatePolicy.strictText(entry.opt("text"));
                    if (text != null) texts.add(text);
                }
            }
            return texts;
        } catch (JSONException | RuntimeException error) {
            return java.util.List.of();
        }
    }

    /** The cloud service URL the shared host built for this query, or empty when unavailable. */
    private String cloudRequestUrl(String document) {
        try {
            JSONObject envelope = new JSONObject(NativeClient.cloudRequestUrl(document));
            return JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.optString("value", "") : "";
        } catch (JSONException | RuntimeException | LinkageError error) {
            return "";
        }
    }

    /** The AI HTTPS descriptor in this envelope, or null when it no longer matches the settings. */
    private static JSONObject aiRequestDescriptor(String raw) {
        if (raw == null) return null;
        try {
            JSONObject envelope = new JSONObject(raw);
            return JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.optJSONObject("value") : null;
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
        long querySession = OnlineCandidatePolicy.sessionId(query.opt("session_id"), -1);
        if (querySession < 0) {
            invalidateOnlineProviders();
            return;
        }
        JSONObject assistant = query.optJSONObject("ai_assistant");
        String signature = OnlineCandidatePolicy.signature(querySession,
            query.optString("cache_key", ""), query.optString("identity", ""),
            InputViewValuePolicy.booleanValue(query, "cloud_candidates", false),
            InputViewValuePolicy.booleanValue(assistant, "enabled", false) ? assistant.toString() : "");
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
                aiAssistant == null ? 0 : KeyboardGeometry.strictInt(aiAssistant, "candidate_limit", 0));
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
                if (!CandidateGlossPolicy.isApplied(applied.opt("applied"))) return;
                JSONObject next = applied.getJSONObject("view");
                if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE)
                        != targetSession) return;
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
        if (candidateLine == null) return;
        // 42 dp 的候选行容下候选字、一行释义和选中 chip 的留白；第二行起每行再加高一些。
        int reserved = CandidateTranslationPolicy.reservedGlossRows(candidateGlossLineCount(), koreanHanjaRows());
        int extraRows = BoundsPolicy.nonNegative(reserved - 1);
        int line = ImeToolbar.CANDIDATE_LINE_DP + extraRows * ImeToolbar.EXTRA_GLOSS_ROW_DP;
        setFixedHeight(candidateLine, pixels(line));
        // 空闲时的工具栏和组词时的读音行 + 候选行占同一个位置，两者同高，打字时键盘才不会变高。空闲时读音行若在显示常驻的模式标签（直接输入、准备中），它已经占了那 14 dp，工具栏只取候选行的高度，总高不变。
        boolean idleHeader = candidateHeader != null
            && candidateHeader.getVisibility() == View.VISIBLE;
        if (shortcutScroll != null)
            setFixedHeight(shortcutScroll,
                pixels((idleHeader ? 0 : ImeToolbar.READING_ROW_DP) + line));
    }

    private static void setFixedHeight(View view, int height) {
        android.view.ViewGroup.LayoutParams params = view.getLayoutParams();
        if (params == null || params.height == height) return;
        params.height = height;
        view.setLayoutParams(params);
    }

    void fail() { stop(false); message = "输入连接失败：仅直接输入"; render(); }

    boolean character(int ascii) {
        return character(ascii, letterCase.usesUppercase());
    }

    boolean character(int ascii, boolean shifted) {
        if (session == 0) return false;
        try { return apply(NativeClient.character(session, ascii, shifted)); }
        catch (JSONException | LinkageError error) { fail(); return true; }
    }

    private boolean helpcodeCompositionEligible() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.eligible(dedicatedEnglish,
            view.optString("editing_text", ""), InputViewValuePolicy.scheme(view, -1),
            view.optString("local_mode", "none"));
    }

    private boolean entersHelpcode() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.entersHelpcode(dedicatedEnglish, letterCase.usesUppercase(),
            view.optString("editing_text", ""), InputViewValuePolicy.scheme(view, -1),
            view.optString("local_mode", "none"));
    }

    /** 一笔滑行抬手（{@link ImeGlideTyping}）：请求见 {@link GlideTypingPolicy#request}。引擎不收（handled=false）时什么也不输入。 */
    void glide(String request) {
        if (session == 0) return;
        try { apply(NativeClient.glide(session, request)); }
        catch (JSONException | LinkageError error) { fail(); }
    }

    boolean command(int code) {
        if (session == 0) return false;
        try { return apply(NativeClient.command(session, code)); }
        catch (JSONException | LinkageError error) { fail(); return true; }
    }

    void type(char key) {
        if (connection == null) return;
        if (directEnglishActive()) {
            char output = letterCase.usesUppercase() ? Character.toUpperCase(key) : key;
            if (isAsciiLetter(output)) {
                commitEnglishLiteral(output);
                if (letterCase.consumeLetter()) {
                    imeLetterRows.rebuildKeyRows();
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
                imeLetterRows.rebuildKeyRows();
                render();
            }
            return;
        }
        if (entersHelpcode() && isAsciiLetter(key)) {
            character(Character.toUpperCase(key), true);
            if (letterCase.consumeLetter()) {
                imeLetterRows.rebuildKeyRows();
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
            imeLetterRows.rebuildKeyRows();
            render();
        }
    }

    boolean punctuation(int ascii) {
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
            // Zhuyin's marks are bopomofo keys or its Shift overlay, which the Engine writes in any state, so smart punctuation neither replaces them nor arms on what they commit (as on macOS).
            if (zhuyinSchemeActive()) {
                clearSmartPunctuationSnapshots();
                return apply(NativeClient.punctuationWithContext(session, ascii, preceding));
            }
            JSONObject decision = smartPunctuationDecision((char) ascii, before);
            // `isNull` first: org.json's optString hands back the four-letter string "null" for a
            // JSON null, not the fallback. Reading it without this asked the editor to delete the
            // character before the cursor and commit "null" - on every punctuation key.
            String replacement = decision == null || decision.isNull("replace_with")
                ? null : decision.optString("replace_with", null);
            if (replacement != null && !replacement.isEmpty()) {
                if (connection == null || !deleteBeforeCursor(1)) return false;
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

    void space() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        // Space on a composing Zhuyin conversion is tone 1 or opens its list, as on a hardware keyboard; the commit command below would end the conversion and drop the pending syllable.
        if (session != 0 && view != null && ZhuyinInputPolicy.spaceIsEngineKey(zhuyinSchemeActive(),
                view.optString("spelling_symbols", "")) && character(' ', false)) return;
        JSONObject spaceDecision = smartPunctuationDecision(' ', getTextBeforeCursor());
        if (spaceDecision != null && !spaceDecision.isNull("space_ascii")) {
            int ascii = InputViewValuePolicy.integer(spaceDecision, "space_ascii", 0);
            if (ascii >= 32 && ascii <= 126 && deleteBeforeCursor(1)) {
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
            int firstSource = InputViewValuePolicy.integer(first, "source", -1);
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
        return view != null && InputViewValuePolicy.scheme(view, -1) == 3;
    }

    private boolean koreanSchemeActive() {
        return view != null
            && KoreanInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    /** Whether the Hanja list of the composing Korean syllable is on the strip. */
    private boolean koreanHanjaListOpen() {
        if (view == null) return false;
        JSONArray entries = view.optJSONArray("candidates");
        return KoreanInputPolicy.hanjaListOpen(koreanSchemeActive(),
            view.optString("local_mode", "none"), entries == null ? 0 : entries.length());
    }

    private boolean zhuyinSchemeActive() {
        return view != null
            && ZhuyinInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    /** A Stroke composition whose glyphs the editor holds as its composing region (apply marks View.reading for Stroke). */
    private boolean strokeCompositionMarked() {
        return view != null && !view.optString("editing_text", "").isEmpty()
            && StrokeInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish)
            && !InputViewValuePolicy.booleanValue(view, "nine_key", false);
    }

    private boolean vietnameseSchemeActive() {
        return view != null
            && VietnameseInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    boolean tibetanSchemeActive() {
        return view != null
            && TibetanInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    /** 越南语或藏文：字母按敲下的大小写写进组字，所以键面显示大小写，和英文键一样，而不是中文键盘的大写键面。 */
    boolean letterCaseSchemeActive() {
        return vietnameseSchemeActive() || tibetanSchemeActive();
    }

    /** 韩语、越南语或藏文：字母直接拼成书写的文字，所以 Shift 是它们的大小写。韩语和越南语的回车上屏后还执行编辑器动作；藏文的回车只确认组字，见 `returnKeyConfirms`。 */
    private boolean letterCompositionActive() {
        return koreanSchemeActive() || letterCaseSchemeActive();
    }

    /** 韩语、注音、越南语或藏文（`locks_caret`、`commits_on_blur`）：组字是用户已经写下的文字，里面没有光标，打开列表之前也没有候选列表。 */
    private boolean writtenCompositionActive() {
        return view != null && !dedicatedEnglish
            && InputSchemeTraits.locksCaret(InputViewValuePolicy.scheme(view, -1));
    }

    /** Whether the candidate list of the composing Zhuyin conversion is on the strip. */
    private boolean zhuyinListOpen() {
        if (view == null) return false;
        JSONArray entries = view.optJSONArray("candidates");
        return ZhuyinInputPolicy.listOpen(zhuyinSchemeActive(),
            view.optString("local_mode", "none"), entries == null ? 0 : entries.length());
    }

    /** Whether the Zhuyin open-list command applies now: a conversion is composing, with or without its list open. */
    private boolean zhuyinOpensList() {
        return view != null && ZhuyinInputPolicy.opensList(zhuyinSchemeActive(),
            view.optString("local_mode", "none"), view.optString("editing_text", ""));
    }

    /** Drops the composition without writing it. With a Korean Hanja list open the first cancel only closes the list, so this sends as many as KoreanInputPolicy.cancelsToDiscard says; Zhuyin's first cancel likewise only closes its list and Vietnamese's only takes the word back to its raw keys (`cancel_keeps_composition`), so a composition they leave standing takes one more. */
    void discardComposition() {
        boolean keepsComposition = !koreanSchemeActive() && writtenCompositionActive();
        for (int cancels = KoreanInputPolicy.cancelsToDiscard(koreanHanjaListOpen()); cancels > 0; cancels--)
            command(3);
        if (keepsComposition && hasEngineComposition()) command(3);
    }

    /** Whether the Hanja command applies now: a Korean syllable is composing, with or without its list open. */
    private boolean koreanConvertsHanja() {
        return view != null && KoreanInputPolicy.convertsHanja(koreanSchemeActive(),
            view.optString("local_mode", "none"), view.optString("editing_text", ""));
    }

    private boolean japaneseNineKeyActive() {
        return displayedTouchLayout(view) == JAPANESE_NINE_KEY_LAYOUT;
    }

    String spaceKeyTitle() {
        return japaneseSchemeActive()
            ? JapaneseNineKeyActions.spaceTitle(view != null
                && !view.optString("editing_text", "").isEmpty()) : "空格";
    }

    String spaceKeyDescription() {
        return japaneseSchemeActive()
            ? spaceKeyTitle() + "；左右滑动移动光标" : SPACE_CURSOR_DESCRIPTION;
    }

    /** 现在是否画成分离式键盘：开关打开、大屏、横屏，并且正在画的是 26 键一族或韩文键盘（{@link SplitKeyboardPolicy#drawn}）。每次都按当前配置重算，旋转后不需要另外通知。 */
    boolean splitKeyboardDrawn() {
        Configuration configuration = getResources().getConfiguration();
        return SplitKeyboardPolicy.drawn(splitKeyboardEnabled, configuration.smallestScreenWidthDp,
            configuration.orientation == Configuration.ORIENTATION_LANDSCAPE, displayedTouchLayout(view));
    }

    int displayedTouchLayout(JSONObject value) {
        if (dedicatedEnglish) return STANDARD_TOUCH_LAYOUT;
        if (value == null) return touchLayoutHint();
        int layout = touchLayout(value);
        rememberTouchLayout(layout);
        return layout;
    }

    // 键盘弹出时引擎会话还在后台起（scheduleEngineStartup），view 要一两秒后才到；那之前按 view 算布局只能是默认的 26 键，九键用户每次都先看到 26 键再跳成九键。所以记住上次引擎给出的布局，view 还没到时先按它画。记在 SharedPreferences 里，键盘进程重启后也还在。
    private static final String TOUCH_LAYOUT_HINT_PREFERENCES = "keyboard-layout-hint";
    private static final String TOUCH_LAYOUT_HINT_KEY = "touch_layout";
    private int touchLayoutHint = -1;

    private int touchLayoutHint() {
        if (touchLayoutHint < 0) {
            touchLayoutHint = getSharedPreferences(TOUCH_LAYOUT_HINT_PREFERENCES, MODE_PRIVATE)
                .getInt(TOUCH_LAYOUT_HINT_KEY, STANDARD_TOUCH_LAYOUT);
        }
        return touchLayoutHint;
    }

    private void rememberTouchLayout(int layout) {
        if (layout == touchLayoutHint()) return;
        touchLayoutHint = layout;
        getSharedPreferences(TOUCH_LAYOUT_HINT_PREFERENCES, MODE_PRIVATE).edit()
            .putInt(TOUCH_LAYOUT_HINT_KEY, layout).apply();
    }

    boolean sendsChinesePunctuation() {
        return view != null && ChineseSymbolFaces.shouldUseChineseFaces(dedicatedEnglish,
            InputViewValuePolicy.scheme(view, -1), view.optString("local_mode", "none"), chinesePunctuation);
    }

    private java.util.List<QuickPunctuationPolicy.Entry> quickPunctuationEntries() {
        return QuickPunctuationPolicy.entries(dedicatedEnglish,
            view == null ? -1 : InputViewValuePolicy.scheme(view, -1),
            view == null ? "none" : view.optString("local_mode", "none"));
    }

    private boolean quickPunctuationVisible() {
        int layout = displayedTouchLayout(view);
        return keyboardLayer == KeyboardLayout.Layer.LETTERS
            && layout != QUANPIN_NINE_KEY_LAYOUT && layout != JAPANESE_NINE_KEY_LAYOUT
            && layout != KeyboardLayout.STROKE_LAYOUT
            && layout != KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT
            && selectedScheme != KeyboardScheme.QUANPIN_NINE_KEY
            && selectedScheme != KeyboardScheme.JAPANESE_NINE_KEY
            && selectedScheme != KeyboardScheme.ZHUYIN_NINE_KEY;
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
                imeKeyFeedback.playFeedback(quickPunctuationButton);
                type(entry.input());
                return true;
            });
        }
        popup.show();
    }

    void updateQuickPunctuation() {
        if (quickPunctuationButton == null) return;
        java.util.List<QuickPunctuationPolicy.Entry> entries = quickPunctuationEntries();
        boolean visible = quickPunctuationVisible() && !entries.isEmpty();
        ViewPolicy.setVisible(quickPunctuationButton, visible);
        if (!visible) return;
        String face = entries.get(0).face();
        // 与句号键一样按墨迹居中放大画：全角「，」原样居中时只剩键底一个小点。
        CenteredGlyphSpan.apply(quickPunctuationButton, face, 1.3f);
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
        int scheme = view == null ? -1 : InputViewValuePolicy.scheme(view, -1);
        String profile = view == null ? "" : view.optString("shuangpin_profile", "");
        String localMode = view == null ? "none" : view.optString("local_mode", "none");
        boolean chineseMode = !dedicatedEnglish && !letterCaseSchemeActive();
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
            // 韩语键面上 Shift 是用户选的双辅音而不是大写；越南语和藏文里它是下一个字母的大小写或 Caps Lock（威利转写区分大小写），所以编辑器的更新不能把它清掉。
            if (!letterCompositionActive()) letterCase.reset();
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
        if (letterCase.applyAutomatic(next)) imeLetterRows.rebuildKeyRows();
        render();
    }

    void toggleInputLanguage() {
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
            if (previousLayout != displayedTouchLayout(view) || previousUppercase) imeLetterRows.rebuildKeyRows();
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
            InputViewValuePolicy.booleanValue(value, "nine_key", false), InputViewValuePolicy.scheme(value, -1),
            value.optString("touch_keyboard_layout"));
    }

    void enter() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        // With a Korean Hanja list open Return chooses the highlighted Hanja, as Space does: only the session knows the highlight, so it is the candidate command (msime_client.h). With no list Return writes the syllable out and then does its editor action below.
        if (koreanHanjaListOpen() && command(1)) return;
        // An open Zhuyin list is chosen from the same way.
        if (zhuyinListOpen() && command(1)) return;
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
                && performEditorAction(action)) return;
        commitText("\n");
    }

    /**
     * 回车键是否只确认组字。韩语音节由回车上屏，按键随后照常执行自己的动作，所以那里保留编辑器动作，除非音节的汉字列表打开：这时回车只选汉字。越南语单词也一样：回车写出单词再执行编辑器动作。藏文不同：Engine 吞掉组字时的回车（handled），只上屏藏文、不加音节点，所以组字时回车键显示「确认」。
     */
    boolean returnKeyConfirms() {
        return view != null && !view.optString("editing_text", "").isEmpty()
            && (!letterCompositionActive() || koreanHanjaListOpen() || tibetanSchemeActive());
    }

    void updateReturnKey() {
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
            KeyboardKeyRole role = imeBottomRow.returnKeyRole();
            if (enterButton instanceof KeyboardPressButton press && press.keyboardRole() != role) {
                press.setKeyboardRole(role);
                imeStyler.styleButton(enterButton, role, skin);
            }
        }
        if (japaneseReturnKey != null) {
            String japaneseTitle = JapaneseNineKeyActions.returnTitle(composing);
            japaneseReturnKey.setText(japaneseTitle);
            japaneseReturnKey.setContentDescription(japaneseTitle);
            // 与其他布局底栏的回车同一个角色，配色才一致。
            KeyboardKeyRole role = imeBottomRow.returnKeyRole();
            if (japaneseReturnKey instanceof KeyboardPressButton press && press.keyboardRole() != role) {
                press.setKeyboardRole(role);
                imeStyler.styleButton(japaneseReturnKey, role, skin);
            }
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
            ViewPolicy.setEnabled(japaneseVariantsButton, enabled);
            japaneseVariantsButton.setContentDescription(
                symbols ? "括号；长按选择其他括号"
                    : JapaneseVariantPolicy.accessibilityLabel(enabled));
        }
        if (spaceButton != null && !cursorMovement.isActive()) {
            spaceButton.setText(spaceKeyTitle());
            spaceButton.setContentDescription(spaceKeyDescription());
        }
        imeBottomRow.syncSplitSpace();
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
        // A key the Dachen editor claims in its current state (a bopomofo or tone key, Space while a syllable is pending) or one of its Shift marks is Zhuyin input, decided before the number row picks a candidate or a mark pages the list, as on macOS.
        if (session != 0 && view != null && !event.isCtrlPressed() && !event.isAltPressed()
                && !event.isMetaPressed() && ZhuyinInputPolicy.engineKey(zhuyinSchemeActive(),
                    event.getUnicodeChar(), view.optString("spelling_symbols", ""))) {
            return character(event.getUnicodeChar(), event.isShiftPressed())
                || super.onKeyDown(keyCode, event);
        }
        // 组字中或本地模式里 Engine 列为拼写的字符是输入，要在数字选词、翻页键和标点之前送给 Engine：网址模式的 `.` `=` 和数字、`www` 之后的 `.`、V 模式的运算符。
        if (session != 0 && view != null && !event.isCtrlPressed() && !event.isAltPressed()
                && !event.isMetaPressed() && NumberRowSelectionPolicy.engineSpells(
                    view.optString("local_mode", "none"), view.optString("editing_text", ""),
                    view.optString("spelling_symbols", ""), event.getUnicodeChar())) {
            return character(event.getUnicodeChar(), event.isShiftPressed())
                || super.onKeyDown(keyCode, event);
        }
        // 哪一面数字键选词由策略决定：Engine 把数字列为拼写时（U、V、网址模式）裸数字是输入，选词移到 Shift 那一面。
        int candidateSlot = NumberRowSelectionPolicy.slotForKeyCode(keyCode,
            event.isShiftPressed(), numberRowSelection,
            view == null ? "none" : view.optString("local_mode", "none"),
            view == null ? "" : view.optString("spelling_symbols", ""), event.getUnicodeChar());
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
                    if (letterCase.consumeLetter()) imeLetterRows.rebuildKeyRows();
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
        // A Korean Hanja is one character already, so there is no word to take one from: with the list open the pair is punctuation, which closes the list and writes the Hangul with the mark. A Zhuyin list keeps its marks for the Engine the same way, and Vietnamese has no list.
        WordCharacterPolicy.Edge wordCharacterEdge = WordCharacterPolicy.edgeFor(
            keyCode, event.isShiftPressed(), wordCharacterBinding,
            highlightedCandidate() != null && !writtenCompositionActive());
        if (wordCharacterEdge != WordCharacterPolicy.Edge.NONE
                && selectCandidateEdge(wordCharacterEdge)) return true;
        // Paging only means something while there is a candidate list. With nothing composed these
        // keys are the editor's: Tab moves focus, Page Down scrolls, and a comma is a comma.
        // Korean has no candidate list until its Hanja list opens: until then its punctuation follows the syllable, and Home/End end the syllable through HardwareKeyPolicy below and then move the caret. With the list open these keys page and move the highlight as for any list, except that the marks stay punctuation and Left/Right, which have no caret inside a syllable to move, move the highlight; Escape closes the list and keeps the syllable.
        // 注音、越南语和藏文按同样的方式区分：没有打开列表时（越南语和藏文从来没有列表）这些键结束组字并执行自己的动作，打开的注音列表则像汉字列表一样翻页和移动高亮。不论方向键绑定是什么，↓ 都打开关闭着的注音列表（这是 libchewing 打开列表的键），因为列表关闭时没有高亮可以移动。
        boolean koreanHanjaList = koreanHanjaListOpen();
        boolean openedList = koreanHanjaList || zhuyinListOpen();
        if (ZhuyinInputPolicy.listDownKey(keyCode, event.isShiftPressed(), zhuyinOpensList(),
                zhuyinListOpen())) {
            command(ZhuyinInputPolicy.OPEN_CANDIDATE_LIST_COMMAND);
            return true;
        }
        if (openedList && keyCode == KeyEvent.KEYCODE_ESCAPE)
            return command(3) || super.onKeyDown(keyCode, event);
        if (hasEngineComposition() && (!writtenCompositionActive() || openedList)
                && !(koreanHanjaList && KoreanInputPolicy.hanjaListMark(keyCode))) {
            int navigationCommand = CandidateNavigationPolicy.commandFor(
                keyCode, event.isShiftPressed(), candidateNavigation, japaneseSchemeActive());
            if (navigationCommand == CandidateNavigationPolicy.NONE && openedList) {
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
            imePanels.renderAiPolish();
        }
        boolean replyVisible = replyKeyboard != null
            && replyKeyboard.getVisibility() == View.VISIBLE;
        if (replyTarget != null || (selectionChanged && replyVisible)) {
            invalidateReplyContext("输入位置已变化，请重新选择回复方式");
        }
        // 迟到的回报可能只是输入法自己上一次写入的回声（例如上屏之后用户已经按下了下一个键），那不是光标移动，不能把新开始的组字取消掉；对不上任何预期的才按原来的规则处理。
        boolean ownEcho = selectionEcho.acknowledge(newStart, newEnd, composingStart, composingEnd);
        if (!ownEcho && session != 0 && view != null && !view.optString("editing_text").isEmpty()
                && (newStart != composingEnd || newEnd != composingEnd)) {
            // Don't apply an empty composition over the editor's newly moved selection.
            // 韩语音节已经是最终的韩文并内联标记，下面结束组字区域后它留在文档里，所以算作已输入。注音转换、越南语单词和藏文音节同样是已书写的文字（`commits_on_blur`）；藏文记的是 `editing_text` 里转换后的藏文。
            if (koreanSchemeActive() || zhuyinSchemeActive())
                recordTypingStatistics(view.optString("reading", ""), typingSource());
            else if (letterCaseSchemeActive())
                recordTypingStatistics(view.optString("editing_text", ""), typingSource());
            boolean keepsComposition = !koreanSchemeActive() && writtenCompositionActive();
            try {
                // 韩语汉字列表打开时第一次取消只关闭列表（msime_client.h），要再取消一次才丢掉编辑器里已作为文字保留的音节。注音的第一次取消只关闭列表，越南语和藏文的只退回原始按键，所以它们留下的组字还要再取消一次。
                JSONObject cancelled = null;
                for (int cancels = KoreanInputPolicy.cancelsToDiscard(koreanHanjaListOpen()); cancels > 0; cancels--)
                    cancelled = value(NativeClient.command(session, 3));
                JSONObject left = cancelled == null ? null : cancelled.optJSONObject("view");
                if (keepsComposition && left != null && !left.optString("editing_text", "").isEmpty())
                    value(NativeClient.command(session, 3));
            } catch (JSONException | LinkageError error) { fail(); }
            // Stroke marks its stroke glyphs, which are not text the user wrote (`commits_on_blur` is false): finishing the region would leave 一丨 in the document, so the region is removed and the tapped selection put back.
            if (connection != null && strokeCompositionMarked()) {
                bridge.discard(sink(typingSource()), composingStart, composingEnd, newStart, newEnd);
            } else if (connection != null) {
                bridge.abandon(sink(typingSource()));
                // 结束组字区也会有一条回报；记下它，免得它晚到时把用户紧接着开始的新组字当成光标移动取消掉。
                selectionEcho.expect();
            }
            view = null;
            render();
        }
        updateAutomaticCapitalization();
        if (directEnglishActive()) refreshEnglishSuggestions();
    }

    Button button(LinearLayout row, String label, Runnable action) {
        return addRowButton(row, new KeyboardPressButton(this), label, action, true);
    }

    private <T extends Button> T addRowButton(LinearLayout row, T button, String label,
            Runnable action, boolean counted) {
        KeyboardGeometry.setKeyTextSize(button, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        imeStyler.styleButton(button, true);
        if (counted) bindCountedAction(button, action);
        else bindAction(button, action);
        row.addView(button, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        return button;
    }

    /** Give a control an explicit face and hand it back, for use inline where it is created. */
    static Button role(Button button, KeyboardKeyRole face) {
        if (button instanceof KeyboardPressButton press) press.setKeyboardRole(face);
        return button;
    }

    Button shortcutButton(LinearLayout row, String label,
            KeyboardShortcutIconPolicy.Icon icon, Runnable action) {
        KeyboardShortcutButton button = new KeyboardShortcutButton(this, icon);
        return addRowButton(row, button, label, action, true);
    }

    private Button brandButton(LinearLayout row, Runnable action) {
        KeyboardBrandButton button = new KeyboardBrandButton(this,
            () -> Color.parseColor(skin.accent()));
        ViewPolicy.setAllCapsFalse(button);
        button.setText("更多");
        imeStyler.styleButton(button, true);
        bindAction(button, action);
        row.addView(button, KeyboardGeometry.weightedWrapParams(1));
        return button;
    }

    /** The idle top row's scheme pill (the design's 全拼). */
    private Button pillButton(LinearLayout row, String label, Runnable action) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        button.setKeyboardRole(KeyboardKeyRole.PILL);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        KeyboardGeometry.setKeyTextSize(button, 13);
        imeStyler.styleButton(button, KeyboardKeyRole.PILL, skin);
        bindAction(button, action);
        row.addView(button, KeyboardGeometry.weightedWrapParams(1));
        return button;
    }

    Button borderlessButton(LinearLayout row, String label, Runnable action) {
        Button button = new KeyboardBorderlessButton(this);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        imeStyler.styleButton(button, true);
        bindAction(button, action);
        row.addView(button, KeyboardGeometry.weightedWrapParams(1));
        return button;
    }

    Button keyboardKey(String label, String description, Runnable action) {
        Button button = new KeyboardPressButton(this);
        KeyboardGeometry.normalizeKeyCap(button);
        KeyboardGeometry.setKeyTextSize(button, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        return bindCountedAction(button, action);
    }

    /** 九键、注音、笔画、手写和日语九键的 ⌫：和 {@link #keyboardKey} 一样的键，只是画 26 键那个 22 dp 的删除图标，不再用排版字号的「⌫」字符，那样比 26 键的小一圈。 */
    Button backspaceKey(Runnable action) {
        return iconKey(KeyboardIconKey.Kind.BACKSPACE, "⌫", "删除", action);
    }

    /** {@link #keyboardKey} 的图标版：节点文字仍是 `label`，键面画 `kind` 的描边图标。 */
    Button iconKey(KeyboardIconKey.Kind kind, String label, String description, Runnable action) {
        KeyboardIconKey button = new KeyboardIconKey(this, kind);
        KeyboardGeometry.setKeyTextSize(button, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        button.setOnClickListener(ignored -> {
            imeKeyFeedback.playFeedback(button);
            countKey(button);
            action.run();
        });
        return button;
    }

    /** A nine-key grid cap: the same key as {@link #keyboardKey}, plus room for its digit. */
    NineKeyDigitButton nineKeyGridKey(String label, String description, Runnable action) {
        NineKeyDigitButton button = new NineKeyDigitButton(this);
        KeyboardGeometry.setKeyTextSize(button, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        return bindCountedAction(button, action);
    }

    <T extends Button> T bindCountedAction(T button, Runnable action) {
        button.setOnClickListener(ignored -> {
            imeKeyFeedback.playFeedback(button);
            countKey(button);
            action.run();
        });
        return button;
    }

    private <T extends Button> T bindAction(T button, Runnable action) {
        button.setOnClickListener(ignored -> {
            imeKeyFeedback.playFeedback(button);
            action.run();
        });
        return button;
    }

    ShuangpinHintButton shuangpinKeyboardKey(
            String label, String description, Runnable action) {
        ShuangpinHintButton button = new ShuangpinHintButton(this);
        KeyboardGeometry.setKeyTextSize(button, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        return bindCountedAction(button, action);
    }

    /**
     * 键盘里要不要放切换输入法的地球键（底栏的 🌐、日语九键侧列的「切换」）。最低支持 Android 9（API 28）：从 9 起，只要能切换到别的输入法，系统导航栏右下角就有切换按钮，键盘里再放一个是重复的，还占掉底栏一个键位。所以只在系统不给按钮的版本上才放，目前支持的版本都不放。
     */
    boolean offersGlobeKey() {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.P && shouldOfferSwitchingToNextInputMethod();
    }

    boolean hasEngineComposition() {
        return view != null && !view.optString("editing_text", "").isEmpty();
    }

    int pixels(int value) {
        return KeyboardGeometry.pixels(this, value);
    }

    int pixels(double value) {
        if (value <= 0) return 0;
        return KeyboardGeometry.atLeastOnePixel(this, (float) value);
    }

    int halfSpacingPixels(int tenths) {
        return KeyboardGeometry.halfGapPixels(tenths, KeyboardGeometry.density(this));
    }

    /**
     * Whether the key spacing setting insets this control inside its row.
     *
     * <p>The accessibility description used to be the only marker, because every control it applied
     * to was a key cap. The action row holds caps whose descriptions read as sentences, so the role
     * answers first and the description remains the fallback for everything that never asked.
     */
    boolean followsKeySpacing(View node) {
        if (node instanceof KeyboardPressButton press) {
            KeyboardKeyRole role = press.keyboardRole();
            return role == null || role.followsKeySpacing();
        }
        CharSequence description = node.getContentDescription();
        return node instanceof Button && description != null
            && description.toString().startsWith("按键 ");
    }

    boolean systemDark() {
        return KeyboardGeometry.isNight(this);
    }

    private KeyboardSkin keyboardSkin(JSONObject preferences) {
        return surfaceSkin(preferences, "screen_keyboard_theme");
    }

    /** 决定键盘、表情与手写面板皮肤的偏好字段；{@link #rememberSkinHint} 只记这几项。 */
    private static final String[] SKIN_HINT_KEYS = {"global_theme", "custom_theme", "theme",
        "screen_keyboard_theme", "emoji_theme", "handwriting_theme"};
    /** 上次换上的皮肤所用的偏好片段，存在键盘进程自己的 filesDir 里。 */
    private static final String SKIN_HINT_FILE = "keyboard-skin-hint.json";
    private String writtenSkinHint;

    /**
     * 记下这次换上的皮肤所依据的偏好片段，下次键盘进程启动时在偏好读到之前就用它画第一帧。
     *
     * <p>键盘视图在偏好读到之前就建好了：原先那一两秒里画的是内置的跟随系统配色（淡绿），偏好到了才换成用户的皮肤。片段与上次写的相同时不写；写在偏好线程上。
     */
    private void rememberSkinHint(JSONObject preferences) {
        if (preferences == null) return;
        JSONObject hint = new JSONObject();
        try {
            for (String key : SKIN_HINT_KEYS) {
                if (preferences.has(key) && !preferences.isNull(key)) hint.put(key, preferences.get(key));
            }
        } catch (JSONException error) {
            return;
        }
        String text = hint.toString();
        if (text.equals(writtenSkinHint)) return;
        writtenSkinHint = text;
        File target = new File(getFilesDir(), SKIN_HINT_FILE);
        preferencesWorker.execute(() -> {
            File pending = new File(getFilesDir(), SKIN_HINT_FILE + ".pending");
            try {
                java.nio.file.Files.write(pending.toPath(), TextPolicy.utf8Bytes(text));
                java.nio.file.Files.move(pending.toPath(), target.toPath(),
                    java.nio.file.StandardCopyOption.REPLACE_EXISTING, java.nio.file.StandardCopyOption.ATOMIC_MOVE);
            } catch (java.io.IOException | RuntimeException error) {
                android.util.Log.w("MSIMESkin", "Keyboard skin hint was not written", error);
            }
        });
    }

    /** 读上次的皮肤片段；没有或损坏时返回 null，键盘照旧先用内置配色。 */
    private JSONObject readSkinHint() {
        File file = new File(getFilesDir(), SKIN_HINT_FILE);
        if (!file.isFile() || file.length() > 1_000_000) return null;
        try (java.io.InputStream input = java.nio.file.Files.newInputStream(file.toPath(), java.nio.file.LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, 1_000_000);
            if (bytes == null) return null;
            String text = TextPolicy.utf8(bytes);
            writtenSkinHint = text;
            return new JSONObject(text);
        } catch (Exception ignored) {
            return null;
        }
    }

    /**
     * 候选条的配色。皮肤和键盘、表情、手写一样要经 {@link ImeStyler#themed} 按应用主题着色：「跟随系统」不着色时取的是经典绿种子，暖色主题下候选条铺一层淡绿底、页码也是绿的。键盘里换皮肤时也要跟着重算，否则候选条停在旧皮肤上。
     */
    CandidateAppearance.Palette candidateAppearanceFor(JSONObject preferences) {
        KeyboardSkin strip = surfaceSkin(preferences, "candidate_theme");
        return CandidateAppearance.from(preferences, imeStyler == null ? strip : imeStyler.themed(strip));
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
    KeyboardSkin themeSkin(String globalTheme, JSONObject customTheme, boolean dark) {
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
    JSONArray themeCatalog() {
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
    String fullWidthOutput(String text) {
        // 韩语、越南语和藏文无论全角设置怎样都写半角 ASCII（`widens_full_width`），和 runtime 对它们自己的上屏一样。
        return FullWidthInputPolicy.output(text, fullWidthInput && !letterCompositionActive());
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
            imeDebugOverlay.showDiagnostic("全角状态未能同步到输入引擎");
        }
    }

    /** Hand the punctuation state to the runtime and take the answer from the view it returns. */
    private void applyChinesePunctuation(boolean enabled) {
        chinesePunctuation = enabled;
        if (session == 0) return;
        try {
            view = value(NativeClient.setChinesePunctuation(session, enabled));
        } catch (JSONException | LinkageError error) {
            imeDebugOverlay.showDiagnostic("标点状态未能同步到输入引擎");
        }
    }

    /**
     * 中文标点 for this session only, from the toolbar card or 「Ctrl + .」.
     *
     * <p>The shared setting is the state a session starts in, the same way the width is; changing
     * it there is what makes a new value stick.
     */
    void toggleChinesePunctuation() {
        applyChinesePunctuation(!chinesePunctuation);
        imeLetterRows.rebuildKeyRows();
        render();
        imeFunctionPanel.renderMoreTools();
    }

    /** Switch width for this session; the shared setting supplies the next session's default. */
    void toggleFullWidthInput() {
        applyCharacterWidth(!fullWidthInput);
        render();
        imeFunctionPanel.renderMoreTools();
    }

    private void saveFeedbackPreferences() {
        try {
            KeyboardFeedbackStore.save(this, new KeyboardFeedbackStore.Settings(
                soundEnabled, hapticsEnabled, hapticStrength));
            // 按键音和振动随「设置」同步，键盘里改的也要标记，否则传不上去，下次下载还会被盖回。
            SyncSignals.markDirty(this, SyncSwitch.SETTINGS);
        } catch (Exception ignored) {
            // Keep the current in-memory feedback state; the next save retries the file.
        }
    }

    boolean supportsLocalTools() {
        if (view == null) return false;
        int scheme = InputViewValuePolicy.scheme(view, 0);
        // 韩语没有本地模式：那里 Shift+字母是双辅音。粤拼、注音、越南语、藏文和笔画也没有（`opens_local_modes`）。
        return !dedicatedEnglish && scheme != 2 && scheme != 3
            && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && (!InputSchemeTraits.known(scheme) || InputSchemeTraits.opensLocalModes(scheme));
    }

    /** 本版本提供的本地模式：不带临时日语的版本（五笔版）不列出它，其余与 {@link LocalInputMode#values()} 相同。 */
    java.util.List<LocalInputMode> localInputModes() {
        LocalInputMode[] allModes = LocalInputMode.values();
        java.util.List<LocalInputMode> modes = new java.util.ArrayList<>(allModes.length);
        for (LocalInputMode mode : allModes) {
            if (mode != LocalInputMode.TEMPORARY_JAPANESE || edition.temporaryJapanese()) modes.add(mode);
        }
        return modes;
    }

    /** 本地模式可用：偏好打开着，临时日语还要日文词典已在本机（随包或下载的资源包）。没有词典时 host-api 本来就把它关掉，菜单里置灰，免得点了没反应；去设置 → 输入下载。 */
    boolean localModeEnabled(LocalInputMode mode) {
        if (mode == LocalInputMode.TEMPORARY_JAPANESE && !resourcePacks.contains(KeyboardScheme.JAPANESE_PACK)) return false;
        return localModes.optBoolean(mode.preferenceKey(), true);
    }

    void openLocalInputMode(LocalInputMode mode) {
        if (session == 0 || !supportsLocalTools() || !localModeEnabled(mode)) return;
        imeKeyFeedback.playFeedback(moreButton);
        character(mode.trigger().charAt(0), true);
    }

    void closeClipboardHistory() {
        if (clipboardScroll != null) ViewPolicy.hide(clipboardScroll);
        // Cloud entries live only as long as the panel that fetched them, so a later field - possibly a password one - never starts with them in memory.
        cloudClipboardGeneration++;
        cloudClipboardItems = java.util.List.of();
        cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    }

    void closeSchemePicker() {
        if (schemeScroll != null) ViewPolicy.hide(schemeScroll);
        synchronizeReplyKeyboard();
    }

    void closeSkinPicker() {
        if (skinScroll != null) ViewPolicy.hide(skinScroll);
    }

    void closeLayoutSettings() {
        if (layoutSettingsScroll != null) ViewPolicy.hide(layoutSettingsScroll);
        if (layoutAdjustView != null) ViewPolicy.hide(layoutAdjustView);
    }

    void closeMoreTools() {
        localInputToolsOpen = false;
        aiAssistChooserOpen = false;
        if (moreToolsScroll != null) ViewPolicy.hide(moreToolsScroll);
    }

    void closeEmojiPicker() {
        emojiLoadGeneration++;
        emojiLoading = false;
        emojiSelectedCategory = Integer.MIN_VALUE;
        emojiItems = java.util.List.of();
        if (emojiPanel != null) ViewPolicy.hide(emojiPanel);
        synchronizeReplyKeyboard();
    }

    void closeSymbolPanel() {
        if (symbolPanel != null) ViewPolicy.hide(symbolPanel);
        synchronizeReplyKeyboard();
    }

    java.util.List<String> loadEmojiRecents() {
        if (emojiPreferences == null) return java.util.List.of();
        String document = emojiPreferences.getString(EMOJI_RECENTS_KEY, "[]");
        if (document == null || document.length() > 16_384) return java.util.List.of();
        try {
            JSONArray values = new JSONArray(document);
            int count = Math.min(values.length(), EmojiCatalogModel.RECENTS_LIMIT * 2);
            java.util.ArrayList<String> stored = new java.util.ArrayList<>(count);
            for (int index = 0; index < count; index++) {
                Object value = values.opt(index);
                if (value instanceof String) stored.add((String) value);
            }
            return EmojiCatalogModel.normalizeRecents(stored);
        } catch (JSONException error) {
            return java.util.List.of();
        }
    }

    void saveEmojiRecents() {
        if (emojiPreferences == null) return;
        emojiPreferences.edit().putString(
            EMOJI_RECENTS_KEY, new JSONArray(emojiRecents).toString()).apply();
    }

    boolean emojiPickerVisible() {
        return emojiPanel != null && emojiPanel.getVisibility() == View.VISIBLE;
    }

    void selectEmojiCategory(int category) {
        if (!emojiPickerVisible()) return;
        if (category < -1 || category >= EmojiCatalogModel.categories().size()) return;
        emojiLoadGeneration++;
        emojiSelectedCategory = category;
        emojiNextOffset = 0;
        emojiComplete = category == -1;
        emojiLoading = false;
        emojiItems = java.util.List.of();
        imePanels.renderEmojiTabs();
        if (category == -1) {
            java.util.ArrayList<EmojiCatalogModel.Item> recent =
                new java.util.ArrayList<>(emojiRecents.size());
            for (String text : emojiRecents)
                recent.add(new EmojiCatalogModel.Item(text, "", "最近"));
            emojiItems = java.util.List.copyOf(recent);
            imePanels.renderEmojiGrid();
        } else {
            imePanels.renderEmojiGrid();
            loadEmojiPage();
        }
    }

    private EmojiCatalogModel.Page decodeEmojiPage(
            String response, int offset, EmojiCatalogModel.Category category) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new JSONException("Emoji catalog unavailable");
        JSONObject value = envelope.getJSONObject("value");
        JSONArray entries = value.getJSONArray("items");
        if (entries.length() > EmojiCatalogModel.PAGE_SIZE)
            throw new JSONException("Emoji catalog page too large");
        java.util.ArrayList<EmojiCatalogModel.Item> items = new java.util.ArrayList<>(entries.length());
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
                nextOffset(value), strictBoolean(value, "complete"));
        } catch (IllegalArgumentException error) {
            throw new JSONException("Invalid emoji catalog cursor");
        }
    }

    private static long nextOffset(JSONObject value) throws JSONException {
        long offset = KeyboardGeometry.strictLong(value.opt("next_offset"), -1);
        if (offset < 0) throw new JSONException("Invalid emoji catalog cursor");
        return offset;
    }

    void loadEmojiPage() {
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
        imePanels.renderEmojiStatus();
        emojiWorker.execute(() -> {
            EmojiCatalogModel.Page page = null;
            try {
                page = EmojiCatalogModel.renderable(decodeEmojiPage(
                    NativeClient.emojiCatalog(query, resources), offset, category),
                    emojiGlyphPaint::hasGlyph);
            }
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
                    imePanels.showEmojiStatus("表情目录暂时不可用；点分类重试");
                    return;
                }
                java.util.ArrayList<EmojiCatalogModel.Item> combined =
                    new java.util.ArrayList<>(emojiItems);
                combined.addAll(result.items());
                emojiItems = java.util.List.copyOf(combined);
                emojiNextOffset = result.nextOffset();
                emojiComplete = result.complete();
                imePanels.renderEmojiGrid();
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

    void closeVoiceResult() {
        if (voiceResultScroll != null) ViewPolicy.hide(voiceResultScroll);
        voiceResultEntry = null;
        voiceTarget = null;
    }

    void cancelAiRequest() {
        if (aiOperation != null) aiOperation.cancel();
        aiOperation = null;
        aiBusy = false;
    }

    void closeAiPolish() {
        cancelAiRequest();
        if (aiPolishContainer != null) ViewPolicy.hide(aiPolishContainer);
        aiTarget = null;
        aiRequestConfiguration = null;
        aiSourceText = "";
        aiOutputText = "";
        aiError = "";
    }

    void clearReplyRequestReferences() {
        replyOperation = null;
        replyTarget = null;
        replyRequestConfiguration = null;
    }

    void closeReplyKeyboard() {
        replyOpen = false;
        replyModel.resetResults();
        clearReplyRequestReferences();
        setReplyKeyboardVisible(false);
    }

    /** Keep the shared candidate/shortcut bar visible while the reply surface owns the key area. */
    void setReplyKeyboardVisible(boolean visible) {
        // 面板顶替按键区，就该和按键区一样高：键盘外层按内容定高，只靠权重占「剩下的空间」时，按键行一隐藏键盘就整体变矮，面板里的风格格子被压扁。打开前量一次按键区的实际高度给面板；还没布局过（高度为 0）时保留权重。
        if (visible && replyKeyboard != null && keyRows != null && actionRow != null
                && keyRows.getVisibility() == View.VISIBLE) {
            int keyArea = actionRow.getBottom() - keyRows.getTop();
            if (keyArea > 0 && replyKeyboard.getLayoutParams() instanceof LinearLayout.LayoutParams params) {
                params.height = keyArea;
                params.weight = 0;
                replyKeyboard.setLayoutParams(params);
            }
        }
        if (replyKeyboard != null)
            ViewPolicy.setVisible(replyKeyboard, visible);
        if (keyRows != null)
            ViewPolicy.setVisible(keyRows, !visible);
        // 收起回复面板时底栏不是一律恢复：日语九键没有底栏，强行设回 VISIBLE 会让一条空底栏占掉一行高度。由 updateActionRow 按当前布局决定。
        if (actionRow != null) {
            if (visible) ViewPolicy.hide(actionRow);
            else imeBottomRow.updateActionRow();
        }
    }

    private void invalidateReplyContext(String message) {
        replyModel.invalidate(message);
        clearReplyRequestReferences();
        imePanels.renderReplyKeyboard();
    }

    boolean replyTargetMatches() {
        return replyTarget != null && replyTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    boolean replyReady() {
        return replyOpen && aiPolishReady();
    }

    void synchronizeReplyKeyboard() {
        if (replyKeyboard == null) return;
        if (!replyOpen) {
            setReplyKeyboardVisible(false);
            return;
        }
        imePanels.renderReplyKeyboard();
        setReplyKeyboardVisible(true);
    }

    /** 工具栏「回复」按钮：面板关着就打开，开着就收起回到原来的键盘。任何输入方案下都可用。 */
    void toggleReplyKeyboard() {
        if (replyOpen) hideReplyKeyboard();
        else imePanels.showReplyKeyboard();
    }

    /** 收起面板但保留已生成的回复，再次打开时仍能看到；与插入回复后的收起相同。还在生成的请求随收起取消，免得结果在面板关着时到达。 */
    private void hideReplyKeyboard() {
        if (replyModel.busy()) invalidateReplyContext("面板已收起，请重新选择回复方式");
        replyOpen = false;
        setReplyKeyboardVisible(false);
        render();
    }

    void finishReply(long modelGeneration, long operationGeneration, String result,
                             AiPolishClient.Failure failure) {
        if (replyOperation == null || replyOperation.generation() != operationGeneration) return;
        replyOperation = null;
        if (!replyTargetMatches() || replyRequestConfiguration == null
                || !replyRequestConfiguration.equals(aiPolishConfiguration)
                || !replyOpen) {
            invalidateReplyContext("输入位置或 AI 配置已变化，请重新选择回复方式");
            return;
        }
        if (failure == null) replyModel.finish(modelGeneration, result);
        else replyModel.fail(modelGeneration, failure.reason() == AiPolishClient.Reason.INVALID
            ? "服务返回的文字为空或超过一万字" : "AI 请求失败，请检查网络、地址、模型和密钥");
        if (!replyModel.busy()) replyOperation = null;
        imePanels.renderReplyKeyboard();
    }

    boolean canSaveKeyboardSkin() {
        return !skinSaving && !traditionalOutputSaving
            && session != 0 && preferencesSnapshot != null
            && !preferencesDirectory.isEmpty();
    }

    private void showKeyboardSkinStatus(String value) {
        if (replyKeyboard != null && replyKeyboard.getVisibility() == View.VISIBLE) {
            replyModel.showStatus(value);
            imePanels.renderReplyKeyboard();
        } else {
            Toast.makeText(this, value, Toast.LENGTH_SHORT).show();
        }
    }

    /**
     * Select one global theme from the keyboard's picker, or, with a `design`, store it as `custom_theme.keyboard` and select `custom`.
     *
     * <p>This copies the settings page: when another theme was on screen it becomes `custom_theme.base` and `custom_theme.candidate_skin` is cleared, so the candidate strip keeps the theme the user was looking at; while `custom` is already selected only the keyboard changes.
     */
    void saveKeyboardSkin(String identifier, JSONObject design) {
        if (skinSaving || traditionalOutputSaving || session == 0
                || preferencesSnapshot == null || preferencesDirectory.isEmpty()) return;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        final JSONObject preferences;
        String designAnimation = null;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
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
                applyDesignFeedback(preferences, design);
                designAnimation = CustomKeyboardSkin.from(design).pressAnimation();
            }
        } catch (JSONException error) {
            showKeyboardSkinStatus("皮肤切换失败，保留当前皮肤");
            return;
        }
        skin = keyboardSkin(preferences);
        emojiSkin = surfaceSkin(preferences, "emoji_theme");
        handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
        candidateAppearance = candidateAppearanceFor(preferences);
        rememberSkinHint(preferences);
        skinSaving = true;
        final long operation = ++preferenceSaveGeneration;
        imeStyler.applySkin();
        render();
        try {
            final String animation = designAnimation == null ? null
                : (String) AndroidLocalSettings.spec(AndroidLocalSettings.KEY_ANIMATION).accept(designAnimation);
            preferencesWorker.execute(() -> {
                String response;
                try {
                    response = NativeClient.savePreferences(targetDirectory, expectedRevision, pending.toString());
                    // 本地的按键动画只在偏好写入成功后再写：CAS 冲突时界面回到原皮肤，磁盘上也不能留下新动画。
                    if (animation != null && response != null
                            && JsonPolicy.strictTrue(new JSONObject(response).opt("ok"))) {
                        AndroidLocalSettings.put(this, AndroidLocalSettings.KEY_ANIMATION, animation);
                    }
                }
                catch (Exception | LinkageError error) { response = null; }
                final String savedResponse = response;
                main.post(() -> finishKeyboardSkinSave(operation, targetSession, targetDirectory, savedResponse));
            });
        } catch (RuntimeException error) {
            finishKeyboardSkinSave(operation, targetSession, targetDirectory, null);
        }
    }

    /** 自定义设计带的按键音（P23）写进偏好的 `plugins.key_sound.pack`；「静音」不改音效包，只是不播（本地按键音开关）。按键动画在本地设置里，由 saveKeyboardSkin 另外写。 */
    private static void applyDesignFeedback(JSONObject preferences, JSONObject design) throws JSONException {
        CustomKeyboardSkin feedback = CustomKeyboardSkin.from(design);
        if (CustomKeyboardSkin.SILENT_SOUND_PACK.equals(feedback.soundPack())) return;
        JSONObject plugins = preferences.optJSONObject("plugins");
        if (plugins == null) return;
        JSONObject keySound = plugins.optJSONObject("key_sound");
        if (keySound == null) return;
        keySound.put("pack", feedback.soundPack());
    }

    /** 用户在键盘里换了一款皮肤：记进打字统计（徽章「换装达人」），隐私情况下不记。 */
    void recordSkinStatistics(String id) {
        String directory = typingStatisticsDirectory();
        if (id == null || id.isEmpty() || !ImePrivacyGate.recordsTyping(directory, id)) return;
        final String request;
        try {
            request = new JSONObject().put("directory", directory).put("action",
                new JSONObject().put("operation", "record_skin").put("id", id)).toString();
        } catch (JSONException error) {
            return;
        }
        submitTypingStatistics(request, null, engineStartGeneration);
    }

    private void finishKeyboardSkinSave(long operation, long targetSession, String targetDirectory,
                                        String response) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        skinSaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long currentRevision = preferencesSnapshot == null
                ? -1 : PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1);
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (PreferencesSavePolicy.shouldApplyResponse(currentRevision, savedRevision)) {
                // The worker just wrote the key animation; reload it so the snapshot is applied with the new value, not the stale in-memory one.
                refreshLocalSettings();
                applyPreferencesSnapshot(saved);
                showKeyboardSkinStatus("皮肤已切换");
            } else {
                // A preferences reload won the race while this save was in flight. Its newer
                // snapshot is already applied; the stale save response must not roll it back.
                showKeyboardSkinStatus("设置已更新");
            }
        } catch (JSONException | LinkageError error) {
            JSONObject accepted = preferencesSnapshot == null ? null
                : preferencesSnapshot.optJSONObject("preferences");
            skin = keyboardSkin(accepted);
            emojiSkin = surfaceSkin(accepted, "emoji_theme");
            handwritingSkin = surfaceSkin(accepted, "handwriting_theme");
            candidateAppearance = candidateAppearanceFor(accepted);
            showKeyboardSkinStatus("皮肤切换失败，已恢复原皮肤");
        }
        imeStyler.applySkin();
        render();
        imePanels.finishSkinPick();
    }

    /** Finish the Engine composition before handing the input connection to another IME. */
    void switchToNextInputMethodAfterCommit() {
        if (session != 0) command(2);
        switchToNextInputMethod(false);
    }

    private boolean canSaveChineseOutput() {
        return !traditionalOutputSaving && !schemeSaving && !touchGeometrySaving && !skinSaving
            && session != 0 && preferencesSnapshot != null && !preferencesDirectory.isEmpty();
    }

    void toggleChineseOutput() {
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
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
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
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (preferencesSnapshot != null
                    && PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1)
                        > savedRevision) {
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

    static final String REPLY_SOURCE_PLACEHOLDER = "+ 粘贴 TA 的话帮你回";

    boolean voiceInsertionReady() {
        return session != 0 && connection != null && view != null
            && view.optString("editing_text", "").isEmpty()
            && view.optString("local_mode", "none").equals("none");
    }

    boolean aiPolishReady() { return voiceInsertionReady(); }

    String editorContext(boolean before) {
        if (connection == null) return null;
        CharSequence text = before ? connection.getTextBeforeCursor(64, 0)
            : connection.getTextAfterCursor(64, 0);
        return text == null ? null : text.toString();
    }

    String selectedEditorText() {
        if (connection == null) return null;
        CharSequence text = connection.getSelectedText(0);
        return text == null ? null : text.toString();
    }

    void captureVoiceTarget() {
        voiceTarget = new EditorContextSnapshot(connection, editorContextRevision, editorContext(true),
            selectedEditorText(), editorContext(false));
    }

    boolean voiceTargetMatches() {
        return voiceTarget != null && voiceTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    /** 键盘内识别的结果放不进原来的位置时，先存进语音结果，由用户在结果面板里决定插到哪儿。 */
    boolean stashVoiceResult(String text) {
        if (voiceResultStore == null) return false;
        try {
            voiceResultStore.save(text, System.currentTimeMillis());
        } catch (VoiceResultStore.Failure error) {
            return false;
        }
        return true;
    }

    boolean aiTargetMatches() {
        return aiTarget != null && aiTarget.matches(connection, editorContextRevision,
            editorContext(true), selectedEditorText(), editorContext(false));
    }

    void startVoiceRecognition() {
        if (!voiceInputEnabled) {
            Toast.makeText(this, "请先在共享设置中启用语音输入", Toast.LENGTH_SHORT).show();
            return;
        }
        // 键盘内识别（扩展点）接手时不再打开识别窗口。
        if (imeVoiceEntry.startInKeyboard(keyRows)) return;
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
    void openClientApp() {
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

    void showVoiceResult() {
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
        ViewPolicy.show(voiceResultScroll);
    }

    private void renderVoiceResult() {
        if (voiceResultPanel == null) return;
        voiceResultPanel.removeAllViews();
        LinearLayout header = KeyboardGeometry.row(this);
        TextView title = ViewPolicy.textLabel(this, "语音结果", 18);
        KeyboardGeometry.setKeyTextSize(title, 18);
        header.addView(title, KeyboardGeometry.weightedWrapParams(1));
        button(header, "返回键盘", this::closeVoiceResult);
        voiceResultPanel.addView(header);
        if (voiceResultEntry == null) {
            TextView empty = textView("暂无待插入结果。点击下方按钮开始语音识别；只保留最新一条，10 分钟内有效。");
            // Neutral about which engine runs: since the keyboard entry honours a configured
            // provider, naming the system recognizer here was wrong exactly for the users who had
            // configured one. Which service is used is the settings page's to explain.
            voiceResultPanel.addView(empty);
        } else {
            TextView recognized = textView(voiceResultEntry.text());
            recognized.setContentDescription("待插入语音结果");
            voiceResultPanel.addView(recognized);
            TextView hint = textView("点击插入后清除待插入结果；输入位置变化时会拒绝插入。");
            voiceResultPanel.addView(hint);
            Button insert = button(voiceResultPanel, "插入语音结果", this::insertVoiceResult);
            insert.setContentDescription("插入并清除语音结果");
            insert.setLayoutParams(KeyboardGeometry.matchWidthWrapParams());
        }
        Button recognize = button(voiceResultPanel, "开始语音识别",
            this::startVoiceRecognition);
        recognize.setLayoutParams(KeyboardGeometry.matchWidthWrapParams());
        VoiceConfiguration configured = VoiceConfiguration.read(preferencesDirectory, "ime-preview");
        boolean platformRecognizerAvailable = VoiceRecognitionActivity.available(this);
        ViewPolicy.setEnabled(recognize, voiceInputEnabled
            && (platformRecognizerAvailable || configured.provider() != null));
        imeStyler.applySkin();
    }

    private TextView textView(CharSequence text) {
        TextView view = ViewPolicy.newTextView(this, text);
        KeyboardGeometry.setKeyTextSize(view, KeyboardGeometry.DEFAULT_KEY_TEXT_SP);
        return view;
    }
    private void renderLayoutSettingsState() {
        if (keySpacingSlider != null && rowSpacingSlider != null && keyboardHeightSlider != null
                && keySpacingValue != null && rowSpacingValue != null && keyboardHeightValue != null
                && voiceShortcutSwitch != null && resetLayoutSettingsButton != null) {
            keySpacingSlider.setProgress(touchKeySpacingTenths);
            rowSpacingSlider.setProgress(touchRowSpacingTenths);
            keyboardHeightSlider.setProgress(touchKeyboardHeightAdjustment);
            boolean settingsEditable = !touchGeometrySaving && !traditionalOutputSaving;
            ViewPolicy.setEnabled(keySpacingSlider, settingsEditable);
            ViewPolicy.setEnabled(rowSpacingSlider, settingsEditable);
            ViewPolicy.setEnabled(keyboardHeightSlider, settingsEditable);
            voiceShortcutSwitch.setChecked(touchVoiceShortcutEnabled);
            ViewPolicy.setEnabled(voiceShortcutSwitch, settingsEditable);
            ViewPolicy.setEnabled(resetLayoutSettingsButton, settingsEditable);
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
        imeStyler.applyKeyboardGeometry();
    }

    private void previewTouchHeight(int value) {
        if (touchGeometrySaving || traditionalOutputSaving) return;
        touchKeyboardHeightAdjustment = KeyboardGeometry.heightAdjustment(value);
        renderLayoutSettingsState();
        imeStyler.applyKeyboardGeometry();
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
            ViewPolicy.hide(layoutSettingsScroll);
            ViewPolicy.show(layoutAdjustView);
            layoutAdjustView.requestFocus();
        } else {
            ViewPolicy.show(layoutSettingsScroll);
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
        imeStyler.applyKeyboardGeometry();
        saveTouchGeometry(true);
    }

    private void saveTouchGeometry(boolean reset) {
        if (touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) return;
        JSONObject acceptedPreferences = preferencesSnapshot.optJSONObject("preferences");
        if (!reset && acceptedPreferences != null
                && KeyboardGeometry.keySpacing(KeyboardGeometry.strictInt(
                    acceptedPreferences, "touch_key_spacing_tenths", -1)) == touchKeySpacingTenths
                && KeyboardGeometry.rowSpacing(KeyboardGeometry.strictInt(
                    acceptedPreferences, "touch_row_spacing_tenths", -1)) == touchRowSpacingTenths
                && heightAdjustmentFrom(acceptedPreferences) == touchKeyboardHeightAdjustment
                && acceptedPreferences.optBoolean("touch_voice_shortcut", false)
                    == touchVoiceShortcutEnabled) return;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
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
                preferences.put("touch_voice_shortcut", touchVoiceShortcutEnabled);
            }
        } catch (JSONException error) {
            if (reset && preferencesSnapshot != null) {
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                imeStyler.applyKeyboardGeometry();
            }
            preferencesNotice = reset ? " · 恢复默认失败，保留原设置" : " · 键盘设置保存失败，保留原设置";
            render();
            return;
        }
        touchGeometrySaving = true;
        preferencesNotice = " · 正在保存键盘设置";
        final long operation = ++preferenceSaveGeneration;
        // 高度是设计范围（75%..130%），共享偏好放不下，写进本地设置；恢复默认时删掉本地值。
        final Integer height = reset ? null : touchKeyboardHeightAdjustment;
        renderLayoutSettingsState();
        render();
        Runnable save = () -> {
            String response;
            try {
                response = NativeClient.savePreferences(targetDirectory, expectedRevision,
                    pending.toString());
                // 本地高度只在偏好写入成功后再写：CAS 冲突时界面回到原高度，磁盘上也不能留下新高度。
                if (response != null && JsonPolicy.strictTrue(new JSONObject(response).opt("ok"))) {
                    AndroidLocalSettings.put(this, AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT, height);
                }
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
                    imeStyler.applyKeyboardGeometry();
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
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (preferencesSnapshot != null
                    && PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1)
                        > savedRevision) {
                refreshLocalSettings();
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                imeStyler.applyKeyboardGeometry();
                preferencesNotice = "";
            } else {
                // The worker just wrote the height; reload it so heightAdjustmentFrom reads the new value, not the stale in-memory snapshot.
                refreshLocalSettings();
                applyPreferencesSnapshot(saved);
                preferencesNotice = reset ? " · 键盘设置已恢复默认" : " · 键盘设置已保存";
            }
        } catch (JSONException | LinkageError error) {
            if (preferencesSnapshot != null)
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
            imeStyler.applyKeyboardGeometry();
            preferencesNotice = reset ? " · 恢复默认失败，已恢复原设置" : " · 键盘设置保存失败，已恢复原设置";
            Toast.makeText(this, reset ? "键盘设置未能恢复默认" : "键盘设置未能保存", Toast.LENGTH_SHORT).show();
        }
        renderLayoutSettingsState();
        render();
    }

    void selectEnglishScheme() {
        if (schemeSaving || touchGeometrySaving || traditionalOutputSaving || session == 0) return;
        if (!dedicatedEnglish) toggleInputLanguage();
        if (session == 0) return;
        closeSchemePicker();
        render();
    }

    void selectKeyboardScheme(KeyboardScheme scheme) {
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
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            JSONObject preferences = pending.getJSONObject("preferences");
            String currentScheme = preferences.optString("scheme", edition.defaultScheme());
            String lastChinese = preferences.optString("last_chinese_scheme", currentScheme);
            KeyboardScheme.PreferenceMapping mapping = scheme.mapping(lastChinese,
                preferences.optString("shuangpin_profile", "xiaohe"), edition);
            preferences.put("scheme", mapping.scheme());
            preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
            preferences.put("shuangpin_profile", mapping.shuangpinProfile());
            preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
            JSONArray enabled = new JSONArray();
            for (KeyboardScheme candidate : enabledSchemes) {
                enabled.put(candidate.preferenceId());
            }
            // 切换器也列出未启用的方案，选中时把它加入启用列表；否则 `selected` 指向未启用的方案，下次读取会被回退掉。
            if (!enabledSchemes.contains(scheme)) enabled.put(scheme.preferenceId());
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
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (preferencesSnapshot != null
                    && PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1)
                        > savedRevision) {
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

    void insertClipboardText(String text) {
        // Only that there is text. Re-checking the shared store's own bounds here is what made
        // an entry another mobile host saved listable but not insertable on this one.
        if (connection == null || !ClipboardHistoryPolicy.hasText(text)) return;
        command(2);
        commitText(text);
        closeClipboardHistory();
    }

    /**
     * 把当前剪贴板文本记进本机历史。
     *
     * <p>Android 的默认输入法本来就能读剪贴板，所以和 Gboard 一样，复制之后自动记下（{@link #clipboardWatcher}），打开面板时再补读一次（键盘进程没在运行时复制的那一条）。`announce` 为假时一律不弹提示：自动记录被隐私规则挡下、内容为空或重复都是正常情况。系统标记为敏感的内容（密码管理器复制的密码，Android 13 起的 `EXTRA_IS_SENSITIVE`）从不记录。
     */
    void captureClipboard(boolean announce) {
        if (!imePrivacyGate.allows(ImePrivacyGate.Record.CLIPBOARD_HISTORY)) {
            if (announce) Toast.makeText(this, "隐私模式或当前输入框下不保存剪贴板", Toast.LENGTH_SHORT).show();
            return;
        }
        if (!imePrivacyGate.capturesClipboard()) return;
        try {
            ClipboardManager manager = getSystemService(ClipboardManager.class);
            ClipData clip = manager == null || !manager.hasPrimaryClip() ? null : manager.getPrimaryClip();
            ClipDescription description = clip == null ? null : clip.getDescription();
            if (clip == null || clip.getItemCount() == 0 || description == null
                    || !(description.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                        || description.hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML))) {
                if (announce) Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.Rejection.EMPTY), Toast.LENGTH_SHORT).show();
                return;
            }
            if (description.getExtras() != null
                    && description.getExtras().getBoolean(ClipDescription.EXTRA_IS_SENSITIVE, false)) return;
            CharSequence value = clip.getItemAt(0).getText();
            if (!ClipboardHistoryPolicy.hasText(value == null ? null : value.toString())) {
                if (announce) Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.Rejection.EMPTY), Toast.LENGTH_SHORT).show();
                return;
            }
            // The shared store refuses rather than throws, and says which refusal it is. Deciding
            // that here as well is what made this host disagree with the store it writes into.
            String reason = clipboardHistory.add(value.toString());
            if (reason != null) {
                if (announce) Toast.makeText(this, ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.rejectionFor(reason)), Toast.LENGTH_SHORT).show();
                return;
            }
            if (imePanels.clipboardPanelOpen()) imePanels.renderClipboardHistory();
        } catch (IllegalArgumentException | IllegalStateException | SecurityException error) {
            if (announce) Toast.makeText(this, "无法保存当前剪贴板", Toast.LENGTH_SHORT).show();
        }
    }

    void manageClipboardItem(Button anchor, ClipboardHistory.Item item) {
        PopupMenu popup = new PopupMenu(this, anchor);
        MenuItem pin = popup.getMenu().add(item.pinned() ? "取消固定" : "固定");
        MenuItem remove = popup.getMenu().add("删除");
        // Offered wherever the cloud half is, so the action is discoverable; it only runs once this panel's fetch said the account is signed in with the cloud clipboard on.
        boolean cloudAllowed = imePanels.cloudClipboardAllowed();
        MenuItem upload = cloudAllowed ? popup.getMenu().add(CloudClipboardPanelPolicy.UPLOAD_ACTION) : null;
        if (upload != null) upload.setEnabled(CloudClipboardPanelPolicy.canUpload(
            cloudAllowed, cloudClipboardStatus, item.text()));
        popup.setOnMenuItemClickListener(selected -> {
            if (upload != null && selected == upload) {
                imePanels.uploadClipboardText(item.text());
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
            imePanels.renderClipboardHistory();
            return true;
        });
        popup.show();
    }

    void confirmClearClipboardHistory() {
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
                imePanels.renderClipboardHistory();
            })
            .show();
    }

    void toggleSoundFromMoreTools() {
        soundEnabled = !soundEnabled;
        saveFeedbackPreferences();
        if (soundEnabled) imeKeyFeedback.playFeedback(moreButton);
        imeFunctionPanel.renderMoreTools();
    }

    void toggleHapticsFromMoreTools() {
        hapticsEnabled = !hapticsEnabled;
        saveFeedbackPreferences();
        if (hapticsEnabled) imeKeyFeedback.playFeedback(moreButton);
        imeFunctionPanel.renderMoreTools();
    }

    private void selectHapticStrength(KeyboardFeedbackPreferences.HapticStrength strength) {
        hapticStrength = strength;
        saveFeedbackPreferences();
        if (hapticsEnabled) imeKeyFeedback.playFeedback(moreButton);
        imeFunctionPanel.renderMoreTools();
    }

    String hapticStrengthTitle() {
        return switch (hapticStrength) {
            case LIGHT -> "轻";
            case MEDIUM -> "中";
            case STRONG -> "强";
        };
    }

    void cycleHapticStrength() {
        KeyboardFeedbackPreferences.HapticStrength[] values =
            KeyboardFeedbackPreferences.HapticStrength.values();
        int next = (hapticStrength.ordinal() + 1) % values.length;
        selectHapticStrength(values[next]);
    }

    boolean traditionalOutputToolAvailable() {
        int scheme = view == null ? -1 : InputViewValuePolicy.scheme(view, -1);
        // 粤拼和注音本来就写繁体字，笔画的候选就是字本身，越南语和藏文不是中文（`script_conversion_applies`）。
        return scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && (!InputSchemeTraits.known(scheme) || InputSchemeTraits.scriptConversionApplies(scheme))
            && canSaveChineseOutput();
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
                imeDebugOverlay.showDiagnostic("已清除候选缓存");
                return true;
            }
            if (!candidateManagementEnabled()) return false;
            JSONObject candidate = visibleCandidate(action);
            JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
            if (id == null || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE)
                    != session) return false;
            if (!apply(NativeClient.removeCandidate(session, strictCandidateLong(id, "generation"),
                                                    strictCandidateLong(id, "index")))) {
                imeDebugOverlay.showDiagnostic("当前候选不支持此操作");
            }
            return true;
        } catch (JSONException | LinkageError error) {
            fail();
            return true;
        }
    }

    boolean candidateManagementEnabled() {
        if (view == null || !view.optString("local_mode", "none").equals("none")) return false;
        int scheme = InputViewValuePolicy.scheme(view, 0);
        // 粤拼、注音、越南语、藏文和笔画的候选不属于拼音用户词库，不能固定、删除或调整顺序。
        return scheme != 2 && scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && scheme != InputSchemeTraits.CANTONESE && scheme != InputSchemeTraits.ZHUYIN
            && scheme != InputSchemeTraits.VIETNAMESE && scheme != InputSchemeTraits.TIBETAN
            && scheme != InputSchemeTraits.STROKE;
    }

    /** 候选身份字段是协议整数，禁止 JSONObject 把小数或布尔值静默转换。 */
    static long strictCandidateLong(JSONObject value, String key) throws JSONException {
        try {
            return CandidateGlossPolicy.strictInteger(value.opt(key));
        } catch (IllegalArgumentException error) {
            throw new JSONException("Invalid candidate identity: " + key);
        }
    }

    private static int strictCandidatePage(JSONObject value, String key) {
        long page = CandidateGlossPolicy.strictOr(value.opt(key), Long.MIN_VALUE);
        return page < 0 || page > Integer.MAX_VALUE ? -1 : (int) page;
    }

    private static boolean sameCandidateIdentity(JSONObject left, JSONObject right) {
        if (left == null || right == null) return false;
        try {
            return strictCandidateLong(left, "session") == strictCandidateLong(right, "session")
                && strictCandidateLong(left, "generation")
                    == strictCandidateLong(right, "generation")
                && strictCandidateLong(left, "index") == strictCandidateLong(right, "index");
        } catch (JSONException error) {
            return false;
        }
    }

    static boolean sameCandidateVersion(JSONObject left, JSONObject right) {
        if (left == null || right == null) return false;
        try {
            return strictCandidateLong(left, "session") == strictCandidateLong(right, "session")
                && strictCandidateLong(left, "generation")
                    == strictCandidateLong(right, "generation");
        } catch (JSONException error) {
            return false;
        }
    }

    void editCandidate(JSONObject id, CandidateManagementAction action) {
        if (session == 0 || id == null
                || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) != session)
            return;
        try {
            long generation = strictCandidateLong(id, "generation");
            long index = strictCandidateLong(id, "index");
            String result = switch (action) {
                case PROMOTE -> NativeClient.pinCandidate(session, generation, index);
                case FIX_FIRST -> NativeClient.fixCandidatePosition(
                    session, generation, index, action.fixedPosition());
                case CLEAR_POSITION -> NativeClient.clearCandidatePosition(
                    session, generation, index);
                case REMOVE -> NativeClient.removeCandidate(session, generation, index);
            };
            if (!apply(result)) {
                imeDebugOverlay.showDiagnostic("当前候选不支持此操作");
            } else if (keyboardRoot != null) {
                keyboardRoot.announceForAccessibility(action.announcement());
            }
        } catch (JSONException | LinkageError error) { fail(); }
    }

    void confirmCandidateRemoval(JSONObject id, String text) {
        new AlertDialog.Builder(this)
            .setTitle("删除词条")
            .setMessage("确认删除“" + text + "”？")
            .setNegativeButton("取消", null)
            .setPositiveButton("删除", (dialog, which) -> {
                imeKeyFeedback.playFeedback(moreButton);
                editCandidate(id, CandidateManagementAction.REMOVE);
            })
            .show();
    }

    private boolean candidateIsCurrent(int slot, JSONObject id, String text) {
        JSONObject current = visibleCandidate(slot);
        JSONObject currentId = current == null ? null : current.optJSONObject("id");
        return current != null && currentId != null && id != null
            && sameCandidateIdentity(currentId, id)
            && text.equals(chineseOutput(current.optString("text"), view));
    }

    boolean candidateGlossInsertionEnabled() {
        if (view == null || !"none".equals(view.optString("local_mode", "none"))) return false;
        int scheme = InputViewValuePolicy.scheme(view, 0);
        return scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && schemeShowsGlosses(scheme);
    }

    void insertCandidateGloss(int slot, JSONObject id, String text, String gloss) {
        if (!candidateIsCurrent(slot, id, text) || connection == null) return;
        if (!commitText(gloss, TypingSource.LOCAL)) return;
        if (session != 0) command(3);
    }

    void insertExpandedCandidateGloss(JSONObject candidate, JSONObject id,
                                              String text, String gloss) {
        if (!expandedCandidateIsCurrent(candidate, id, text) || connection == null) return;
        if (!commitText(gloss, TypingSource.LOCAL)) return;
        if (session != 0) command(3);
    }

    private boolean expandedCandidateIsCurrent(JSONObject candidate, JSONObject id, String text) {
        if (!candidatePanelOpen || candidatePanelSnapshot == null || view == null || candidate == null
                || id == null
                || CandidateGlossPolicy.strictOr(candidatePanelSnapshot.opt("session"), Long.MIN_VALUE)
                    != session
                || !sameCandidateVersion(candidatePanelSnapshot, view))
            return false;
        JSONObject candidateId = candidate.optJSONObject("id");
        return candidateId != null
            && sameCandidateIdentity(candidateId, id)
            && text.equals(chineseOutput(candidate.optString("text"), view));
    }

    /** 释义（以及韩语汉字的 훈음）总是从候选下面另起一行，和 iOS 候选条一致；放在同一行会把候选撑宽，一屏只剩一两个候选。 */
    CharSequence candidateLabel(String prefix, String text, String annotation,
                                        boolean highlighted) {
        if (annotation.isEmpty() && prefix.isEmpty()) return text;
        String primary = prefix + text;
        SpannableString label = new SpannableString(primary + "\n" + annotation);
        if (!prefix.isEmpty()) {
            label.setSpan(new ForegroundColorSpan(candidateAppearance.number()), 0, prefix.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        if (annotation.isEmpty()) return label;
        int annotationStart = primary.length() + 1;
        // 0.62 倍：候选字与一行释义要一起放进 36 dp 的候选行，0.72 倍时释义下半截被裁掉。
        label.setSpan(new RelativeSizeSpan(0.62f), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        int foreground = candidateAppearance.textFor(highlighted);
        int secondary = ColorPolicy.withAlpha(foreground, 0.58f);
        label.setSpan(new ForegroundColorSpan(secondary), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return label;
    }

    /** Keep a candidate word intact; the surrounding strip/panel owns scrolling and wrapping. */
    void configureCandidateTextLayout(Button button, int lines) {
        // Reserve extra rows only when this candidate actually carries a gloss. A globally enabled translation target is not evidence that every candidate has one; keeping the empty case single-line prevents long candidate words from wrapping inside the chip. Likewise, a second requested language may be unavailable for this particular result; only an actual newline in the rendered label (a second gloss, or a Korean 훈음 row under its Hanja) warrants another row.
        // A multi-row label deliberately occupies those rows. Do not turn the whole label into a single-line TextView in that case, or the rows after the first are silently clipped. With no extra row the chip can scroll horizontally as one intact candidate word.
        ViewPolicy.setSingleLine(button, lines == 1);
        button.setEllipsize(null);
        button.setHorizontallyScrolling(lines == 1);
    }

    /** Rows of one rendered candidate label; see {@link #candidateLabel}. */
    static int candidateLabelLines(String annotation) {
        return CandidateTranslationPolicy.renderedOwnRowLines(annotation);
    }

    /** Whether the candidates on the strip are the Hanja of a composing Korean syllable, whose annotation is the 훈음 drawn on its own row. */
    private boolean koreanHanjaRows() {
        return koreanSchemeActive() && "none".equals(view.optString("local_mode", "none"));
    }

    String candidateAnnotation(JSONObject candidate) {
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
            context == null ? -1 : InputViewValuePolicy.scheme(context, -1),
            context == null ? "none" : context.optString("local_mode", "none"),
            InputViewValuePolicy.booleanValue(context, "answered_by_pinyin_fallback", false));
    }

    String candidateAnnotation(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        return hint.isEmpty() ? candidateAnnotation(candidate) : hint;
    }

    String candidateAccessibilitySuffix(JSONObject candidate) {
        if (koreanHanjaRows())
            return CandidateGlossPolicy.hanjaAccessibilitySuffix(candidate.optString("annotation", ""),
                candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
                candidateEnglishGloss || candidateTranslationsEnabled);
        return CandidateGlossPolicy.accessibilitySuffix(candidate.optString("annotation", ""),
            candidate.isNull("translation") ? "" : candidate.optString("translation", ""),
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    String candidateAccessibilitySuffix(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        return hint.isEmpty() ? candidateAccessibilitySuffix(candidate) : "，还需输入 " + hint;
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
            if (candidate != null && InputViewValuePolicy.booleanValue(candidate, "highlighted", false)) return candidate;
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
        if (id == null || session == 0
                || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) != session)
            return false;
        String displayed = chineseOutput(candidate.optString("text"), view);
        try {
            candidatePanelOpen = false;
            if (apply(NativeClient.selectEdge(session, strictCandidateLong(id, "generation"),
                                              strictCandidateLong(id, "index"), edge.code()))) return true;
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

    JSONObject visibleCandidate(int slot) {
        if (view == null || slot < 0) return null;
        JSONArray entries = view.optJSONArray("candidates");
        return entries == null ? null : entries.optJSONObject(slot);
    }

    void selectVisibleCandidate(Button button, int slot) {
        JSONObject candidate = visibleCandidate(slot);
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null) return;
        imeKeyFeedback.playFeedback(button);
        if (session == 0
                || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) != session)
            return;
        candidatePanelOpen = false;
        try {
            apply(NativeClient.select(session, strictCandidateLong(id, "generation"),
                                      strictCandidateLong(id, "index")));
        } catch (JSONException | LinkageError error) { fail(); }
    }

    private boolean selectHardwareCandidate(int slot) {
        JSONObject candidate = visibleCandidate(slot);
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null || session == 0
                || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) != session)
            return false;
        candidatePanelOpen = false;
        try {
            apply(NativeClient.select(session, strictCandidateLong(id, "generation"),
                                      strictCandidateLong(id, "index")));
            return true;
        } catch (JSONException | LinkageError error) {
            fail();
            return true;
        }
    }

    private void updateCandidateButton(Button button, JSONObject candidate, int slot) {
        String text = chineseOutput(candidate.optString("text"), view);
        boolean highlighted = InputViewValuePolicy.booleanValue(candidate, "highlighted", false);
        String typed = view == null ? "" : view.optString("preedit", "");
        String annotation = candidateAnnotation(candidate, typed);
        // 开着释义时每个候选都占两行：还没有释义（或这个候选没有）的那行用不换行空格占住。否则带释义的 chip 是两行、不带的是一行，居中后候选字一高一低，释义异步到达或候选一换，候选字就上下跳。
        if (annotation.isEmpty() && candidateGlossLineCount() > 0) annotation = "\u00A0";
        // Touch candidates follow Apple's chip surface: the word itself is shown without a
        // numeric prefix. The slot remains available through contentDescription and the shared
        // session/generation/index identity for accessibility and hardware number-row selection.
        button.setText(candidateLabel("", text, annotation, highlighted));
        int labelLines = candidateLabelLines(annotation);
        ViewPolicy.setFixedLines(button, labelLines);
        configureCandidateTextLayout(button, labelLines);
        KeyboardGeometry.setKeyTextSize(button, candidateFontSize);
        ViewPolicy.setSelected(button, highlighted);
        // render() attaches the button and applies the complete skin tree once below.
        // Avoid creating its candidate drawables before that pass.
        String description = "候选 " + (slot + 1) + "：" + text
            + candidateAccessibilitySuffix(candidate, typed);
        JSONObject id = candidate.optJSONObject("id");
        button.setContentDescription(id != null && candidateManagementEnabled()
            ? description + "；长按管理" : description);
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        ViewPolicy.setEnabled(button, id != null);
    }

    void closeCandidatePanel() {
        candidatePanelOpen = false;
        candidatePanelSnapshot = null;
        if (keyboardRoot != null && expandedCandidates != null) {
            ViewPolicy.hide(expandedCandidates);
            if (expandedCandidateScroll != null)
                ViewPolicy.hide(expandedCandidateScroll);
        }
    }

    void openCandidatePanel() {
        if (session == 0 || view == null || strictCandidatePage(view, "page_count") <= 1) return;
        try {
            JSONObject snapshot = value(NativeClient.allCandidates(session));
            if (CandidateGlossPolicy.strictOr(snapshot.opt("session"), Long.MIN_VALUE) != session
                    || !sameCandidateVersion(snapshot, view))
                return;
            JSONArray entries = snapshot.optJSONArray("candidates");
            JSONArray visible = view.optJSONArray("candidates");
            if (entries == null || visible == null || entries.length() <= visible.length()) return;
            candidatePanelSnapshot = snapshot;
            candidatePanelOpen = true;
            imeCandidates.renderExpandedCandidates();
            imeStyler.applySkin();
        } catch (JSONException | LinkageError error) { fail(); }
    }

    boolean handwritingActive() {
        String localMode = view == null ? "none" : view.optString("local_mode", "none");
        return session != 0 && !dedicatedEnglish
            && "none".equals(localMode)
            && keyboardLayer == KeyboardLayout.Layer.LETTERS
            && displayedTouchLayout(view) == HANDWRITING_LAYOUT && handwritingCanvas != null;
    }

    void deactivateHandwriting() {
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

    void showHandwritingStatus(String text) {
        if (handwritingStatus == null) return;
        handwritingStatus.setText(text);
        ViewPolicy.setVisible(handwritingStatus, text != null && !text.isEmpty());
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

    void refreshHandwritingAvailability() {
        if (!handwritingActive() || handwritingRecognizer == null || handwritingDownload == null) return;
        if (handwritingAvailabilityTask != null) {
            main.removeCallbacks(handwritingAvailabilityTask);
            handwritingAvailabilityTask = null;
        }
        HandwritingRecognizer.Availability availability = handwritingRecognizer.availability();
        switch (availability) {
            case UNAVAILABLE -> {
                handwritingCanvas.setAcceptsInk(false);
                ViewPolicy.hide(handwritingDownload);
                showHandwritingStatus("此构建不含手写识别");
            }
            case DOWNLOAD_REQUIRED -> {
                handwritingCanvas.setAcceptsInk(false);
                handwritingDownload.setText("下载中文手写模型");
                handwritingDownload.setContentDescription("下载中文手写模型；完成后可离线识别");
                ViewPolicy.setEnabled(handwritingDownload, true);
                ViewPolicy.show(handwritingDownload);
                if (!handwritingDownloading) showHandwritingStatus("首次下载后可离线手写");
            }
            case DOWNLOADING -> {
                handwritingCanvas.setAcceptsInk(false);
                handwritingDownload.setText(handwritingDownloading ? "正在下载…" : "正在检查模型…");
                ViewPolicy.setEnabled(handwritingDownload, false);
                ViewPolicy.show(handwritingDownload);
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
                ViewPolicy.hide(handwritingDownload);
                if (!handwritingCanvas.hasInk() && handwritingResults.isEmpty()) {
                    showHandwritingStatus("在此手写，停笔后选字");
                }
            }
        }
    }

    void downloadHandwritingModel() {
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

    void invalidateHandwritingRecognition() {
        if (handwritingRecognitionTask != null) main.removeCallbacks(handwritingRecognitionTask);
        handwritingRecognitionTask = null;
        handwritingRequests.invalidate();
        handwritingResults = java.util.List.of();
        handwritingCandidateToken = null;
        if (handwritingRecognizer != null) {
            try { handwritingRecognizer.cancelPending(); } catch (RuntimeException ignored) { }
        }
    }

    void handwritingInkChanged(long revision,
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

    void clearHandwriting() {
        invalidateHandwritingRecognition();
        if (handwritingCanvas != null) handwritingCanvas.clear();
        showHandwritingStatus("在此手写，停笔后选字");
    }

    void deleteFromHandwriting() {
        if (handwritingCanvas != null && handwritingCanvas.hasInk()) {
            handwritingCanvas.undo();
        } else if (directEnglishActive() && connection != null) {
            clearEnglishSuggestions();
            deleteCodePointBeforeCursor();
            refreshEnglishSuggestions();
        } else if (connection != null && !command(0)) {
            deleteCodePointBeforeCursor();
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

    /** Non-interactive overlay showing the five choices while a Japanese key is being flicked. */
    final class JapaneseFlickPreview extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private static final int[] X_OFFSETS = {0, -1, 0, 1, 0};
        private static final int[] Y_OFFSETS = {0, 0, -1, 0, 1};
        private final String[] labels = new String[5];
        private int labelCount;
        private int selectedDirection;
        private float centerX;
        private float centerY;
        private float cellWidth;
        private float cellHeight;
        private float gap;
        private String paletteKey;
        private int keyColor;
        private int accentColor;
        private int hairlineColor;
        private int onAccentColor;
        private int foregroundColor;
        private Typeface previewTypeface;
        private final int[] rootLocation = new int[2];
        private final int[] anchorLocation = new int[2];

        JapaneseFlickPreview(android.content.Context context) {
            super(context);
            ViewPolicy.hide(this);
            ViewPolicy.setNonInteractive(this);
            ViewPolicy.hideFromAccessibility(this);
        }

        void show(Button anchor, JapaneseNineKeyLayout.Key key, int direction, FrameLayout root) {
            java.util.List<String> kana = key.kana();
            labelCount = Math.min(labels.length, kana.size());
            for (int index = 0; index < labelCount; index++) labels[index] = kana.get(index);
            for (int index = labelCount; index < labels.length; index++) labels[index] = null;
            selectedDirection = KeyboardGeometry.bounded(direction, 0, labelCount - 1);
            root.getLocationOnScreen(rootLocation);
            anchor.getLocationOnScreen(anchorLocation);
            centerX = anchorLocation[0] - rootLocation[0] + anchor.getWidth() / 2f;
            centerY = anchorLocation[1] - rootLocation[1] + anchor.getHeight() / 2f;
            cellWidth = BoundsPolicy.atLeast(anchor.getWidth(), KeyboardGeometry.pixels(getContext(), 40));
            cellHeight = BoundsPolicy.atLeast(anchor.getHeight(), KeyboardGeometry.pixels(getContext(), 36));
            // 五格紧挨着拼成一个十字浮层；原先各隔 6 dp、和底下的键同色同大，看起来像键盘被挤乱了，而不是一个弹框。
            gap = 0;
            root.bringChildToFront(this);
            ViewPolicy.show(this);
            invalidate();
        }

        void hide() { ViewPolicy.hide(this); }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            float density = KeyboardGeometry.density(getContext());
            float textSize = KeyboardGeometry.keySp(getContext(), 24);
            float radius = KeyboardGeometry.floatPixels(10, density);
            float stepX = cellWidth + gap;
            float stepY = cellHeight + gap;
            KeyboardSkin previewSkin = imeStyler.themed(skin);
            String nextPaletteKey = previewSkin.key();
            if (!nextPaletteKey.equals(paletteKey)) {
                paletteKey = nextPaletteKey;
                keyColor = Color.parseColor(previewSkin.keyBackground());
                accentColor = Color.parseColor(previewSkin.accent());
                hairlineColor = Color.parseColor(previewSkin.hairline());
                onAccentColor = Color.parseColor(previewSkin.onAccent());
                foregroundColor = Color.parseColor(previewSkin.keyForeground());
                previewTypeface = previewSkin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT;
            }
            // 先整体画一层投影，再盖上格子：浮层要看得出是压在键盘上面的，而不是键盘本身的一部分。
            paint.setStyle(Paint.Style.FILL);
            paint.setColor(keyColor);
            paint.setShadowLayer(KeyboardGeometry.floatPixels(10, density), 0,
                KeyboardGeometry.floatPixels(3, density), 0x40000000);
            for (int index = 0; index < labelCount; index++) {
                if (labels[index] == null || labels[index].isEmpty()) continue;
                float x = centerX + X_OFFSETS[index] * stepX - cellWidth / 2;
                float y = centerY + Y_OFFSETS[index] * stepY - cellHeight / 2;
                canvas.drawRoundRect(x, y, x + cellWidth, y + cellHeight, radius, radius, paint);
            }
            paint.clearShadowLayer();
            paint.setTextSize(textSize);
            paint.setTypeface(previewTypeface);
            Paint.FontMetrics metrics = paint.getFontMetrics();
            for (int index = 0; index < labelCount; index++) {
                String label = labels[index];
                if (label == null || label.isEmpty()) continue;
                boolean selected = index == selectedDirection;
                float x = centerX + X_OFFSETS[index] * stepX - cellWidth / 2;
                float y = centerY + Y_OFFSETS[index] * stepY - cellHeight / 2;
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(selected ? accentColor : keyColor);
                canvas.drawRoundRect(x, y, x + cellWidth, y + cellHeight, radius, radius, paint);
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(BoundsPolicy.bounded(density, 1f, Float.MAX_VALUE));
                paint.setColor(hairlineColor);
                canvas.drawRoundRect(x, y, x + cellWidth, y + cellHeight, radius, radius, paint);
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(selected ? onAccentColor : foregroundColor);
                paint.setFakeBoldText(selected);
                // 字身中线对准格子中线。原式多减了一次 top，字整体下移大半个字高，落到格子下沿、被下一格盖住。
                float baseline = y + cellHeight / 2 - (metrics.ascent + metrics.descent) / 2;
                float textWidth = paint.measureText(label);
                canvas.drawText(label, x + (cellWidth - textWidth) / 2, baseline, paint);
                paint.setFakeBoldText(false);
            }
        }

        @Override public boolean onTouchEvent(MotionEvent event) { return false; }
    }

    /**
     * 键盘外框：高度只由第一个子视图（键区本身）决定，其余子视图都是盖在键区上的面板，按键区的高度排布，底边让出键区的系统栏内边距。
     *
     * <p>表情面板曾经覆盖整屏，根因在这里：外框原是普通 FrameLayout，输入法窗口给它 AT_MOST 整屏高度，它取所有可见子视图里最高的那个；表情面板是竖排 LinearLayout，里面权重为 1、高度为 0 的网格 ScrollView 在非 EXACTLY 测量下按 WRAP_CONTENT 测，ScrollView 再以 UNSPECIFIED 测整张表情表，于是面板想要几千像素高，被截到整屏，外框跟着变成整屏，键盘窗口也就盖满了屏幕。现在面板永远拿到键区的 EXACTLY 高度，内容再多也只能在面板里滚动。
     */
    static final class PanelSurface extends FrameLayout {
        /** 需要铺满整块键区（含系统栏内边距）的覆盖层，例如按键气泡；其余面板不压在手势条上。 */
        final java.util.Set<View> fullBleed = new java.util.HashSet<>(2);

        PanelSurface(Context context) {
            super(context);
        }

        @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
            View base = getChildCount() == 0 ? null : getChildAt(0);
            if (base == null || base.getVisibility() == GONE) {
                super.onMeasure(widthMeasureSpec, heightMeasureSpec);
                return;
            }
            measureChildWithMargins(base, widthMeasureSpec, 0, heightMeasureSpec, 0);
            int height = base.getMeasuredHeight() + getPaddingTop() + getPaddingBottom();
            int inset = base.getPaddingBottom();
            for (int index = 1; index < getChildCount(); index++) {
                View child = getChildAt(index);
                if (fullBleed.contains(child)
                        || !(child.getLayoutParams() instanceof FrameLayout.LayoutParams params)) continue;
                params.bottomMargin = inset;
            }
            super.onMeasure(widthMeasureSpec, MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY));
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
        refreshLocalSettings();
        File files = getFilesDir();
        // The shared store keeps its file under this directory, which is the same one the settings
        // page hands the shared entry; both sides therefore read one history.
        clipboardHistory = new ClipboardHistoryStore(files);
        voiceResultStore = files == null ? null
            : new VoiceResultStore(files.toPath().resolve("voice-handoff"));
        communityReplyLibrary = files == null ? null : new CommunityReplyLibrary(files.toPath());
        if (!clipboardHistoryEnabled) clipboardHistory.clearQuietly();
        keyboardRoot = new FrameLayout(this);
        PanelSurface surface = new PanelSurface(this);
        keyboardSurface = surface;
        keyboardRoot.addView(keyboardSurface);
        imeStyler.applyKeyboardSurfaceGeometry();
        LinearLayout keyboard = KeyboardGeometry.column(this);
        WindowLayout.fitSystemBars(keyboard);
        keyboardSurface.addView(keyboard, KeyboardGeometry.frameMatchParentParams());
        japaneseFlickPreview = new JapaneseFlickPreview(this);
        keyboardSurface.addView(japaneseFlickPreview, KeyboardGeometry.frameMatchParentParams());
        // 按键气泡的覆盖层：盖在整个键盘上、初始为空，空的 FrameLayout 不拦截触摸，由 ImeLetterRows 持有。
        surface.fullBleed.add(japaneseFlickPreview);
        imeLetterRows.keyPreviewLayer = new FrameLayout(this);
        surface.fullBleed.add(imeLetterRows.keyPreviewLayer);
        keyboardSurface.addView(imeLetterRows.keyPreviewLayer, KeyboardGeometry.frameMatchParentParams());
        LinearLayout candidateRegion = KeyboardGeometry.column(this);
        imeToolbar.buildCandidateHeader(candidateRegion);
        diagnosticView = ViewPolicy.textLabel(this, "", 12);
        KeyboardGeometry.setKeyTextSize(diagnosticView, 12);
        diagnosticView.setContentDescription("输入提示");
        ViewPolicy.hide(diagnosticView);
        candidateRegion.addView(diagnosticView, KeyboardGeometry.matchWidthWrapParams());
        candidateRegion.addView(shortcutScroll, KeyboardGeometry.matchWidthHeightPx(
            pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
        nineKeySpellings = KeyboardGeometry.row(this);
        nineKeySpellingScroll = new HorizontalScrollView(this);
        nineKeySpellingScroll.setHorizontalScrollBarEnabled(false);
        nineKeySpellingScroll.setContentDescription("九键拼音选择");
        nineKeySpellingScroll.addView(nineKeySpellings);
        ViewPolicy.hide(nineKeySpellingScroll);
        candidates = KeyboardGeometry.row(this);
        horizontalCandidateScroll = new HorizontalScrollView(this);
        horizontalCandidateScroll.addView(candidates);
        verticalCandidates = KeyboardGeometry.column(this);
        verticalCandidateScroll = new ScrollView(this);
        verticalCandidateScroll.addView(verticalCandidates);
        candidateViewport = new FrameLayout(this);
        candidateViewport.addView(horizontalCandidateScroll, KeyboardGeometry.frameMatchParentParams());
        candidateViewport.addView(verticalCandidateScroll, KeyboardGeometry.frameMatchParentParams());
        ViewPolicy.hide(candidateViewport);
        imeToolbar.addCandidateLine(candidateRegion, candidateViewport,
            pixels(ImeToolbar.CANDIDATE_LINE_DP));
        imeToolbar.addInlineHeightBar(candidateRegion);
        inlineHeightBar.setListener(new InlineHeightBar.Listener() {
            @Override public void onPercentChanged(int percent, boolean fromUser) {
                previewInlineHeight(percent);
            }

            @Override public void onCancel() { finishInlineHeight(false); }

            @Override public void onReset() { previewInlineHeight(KeyboardGeometry.DEFAULT_HEIGHT_PERCENT); }

            @Override public void onDone() { finishInlineHeight(true); }
        });
        keyboard.addView(candidateRegion);
        // 功能面板和候选展开网格盖住键区但不盖顶部一行，设计里打开它们时工具栏或候选条仍在上面；顶部一行高度会变（读音行、释义行），所以跟着它的实际底边走。
        candidateRegion.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> alignOverlaysBelowTopRow(view));
        imeFrame.keyboard = keyboard;
        // Like Apple, keep the shared candidate/shortcut strip above the reply surface. The
        // ordinary key rows and controls are hidden while this weighted child is visible.
        replyKeyboard = imePanels.createReplyKeyboard();
        ViewPolicy.hide(replyKeyboard);
        keyboard.addView(replyKeyboard, KeyboardGeometry.weightedWidthParams(1));
        // 键距是键的外边距；这两个容器把落在空隙里的按下交给拥有那段空隙的键，画面不变（见 KeyboardKeyArea）。
        KeyboardKeyArea letterArea = new KeyboardKeyArea(this, this::followsKeySpacing);
        letterArea.setGlideTracker(imeGlideTyping);
        keyRows = letterArea;
        keyRows.setOrientation(LinearLayout.VERTICAL);
        imeFrame.wrap(keyRows);
        actionRow = new KeyboardKeyArea(this, this::followsKeySpacing);
        actionRow.setOrientation(LinearLayout.HORIZONTAL);
        actionRow.setContentDescription("键盘功能行");
        actionRowSignature = "";
        imeFrame.wrap(actionRow, KeyboardGeometry.matchWidthHeightPx(
            pixels(KeyboardGeometry.STANDARD_ROW_HEIGHT_DP)));
        // Staging only: every control below is created here and then moved to the row that owns it.
        // The case and delete keys go to the last of the 26 key rows, the shortcut glyphs to the
        // toolbar, and what the action row keeps is whatever KeyboardActionRow lists for the surface.
        LinearLayout controls = KeyboardGeometry.row(this);
        shiftButton = button(controls, "⇧", () -> {
            // 韩语键面上 Shift 是双辅音那一排，不是切到英文的入口；越南语和藏文里它是字母大小写。
            if (!dedicatedEnglish && session != 0 && !helpcodeCompositionEligible()
                    && !letterCompositionActive()) {
                toggleInputLanguage();
                if (!dedicatedEnglish) return;
                // Match Apple: the Shift that entered English starts from lowercase even if the
                // editor would otherwise request automatic capitalization.
                letterCase.reset();
            }
            letterCase.toggle(SystemClock.uptimeMillis());
            imeLetterRows.rebuildKeyRows();
            render();
        });
        shiftButton.setContentDescription("切换到英文大写");
        keyId(shiftButton, "ShiftLeft");
        languageButton = keyId(button(controls, "中/英", this::toggleInputLanguage), "SoftLanguage");
        languageButton.setContentDescription("切换中英文");
        layerButton = button(controls, "123", () -> {
            keyboardLayer = keyboardLayer == KeyboardLayout.Layer.LETTERS
                ? KeyboardLayout.Layer.SYMBOLS : KeyboardLayout.Layer.LETTERS;
            imeLetterRows.rebuildKeyRows();
            render();
        });
        layerButton.setContentDescription("切换到数字和符号");
        keyId(layerButton, "SoftLayer");
        symbolPanelButton = keyId(button(controls, "符", imePanels::showSymbolPanel), "SoftSymbol");
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
        imeLetterRows.bindBackspaceRepeat(deleteButton, this::deleteFromHandwriting);
        spaceButton = keyId(button(controls, "空格", this::space), "Space");
        spaceButton.setContentDescription(SPACE_CURSOR_DESCRIPTION);
        imeBottomRow.bindSpaceCursor(spaceButton);
        enterButton = keyId(button(controls, "换行", this::enter), "Enter");
        enterButton.setContentDescription("换行");
        globeButton = shortcutButton(controls, "切换",
            KeyboardShortcutIconPolicy.Icon.GLOBE, this::switchToNextInputMethodAfterCommit);
        globeButton.setContentDescription("切换到下一个输入法");
        keyId(globeButton, "SoftGlobe");
        schemeButton = shortcutButton(controls, "输入方式",
            KeyboardShortcutIconPolicy.Icon.SCHEME,
            imeToolbar.panelToggle(() -> schemeScroll, imePanels::showSchemePicker));
        schemeButton.setContentDescription("输入方案");
        skinButton = shortcutButton(controls, "皮肤",
            KeyboardShortcutIconPolicy.Icon.SKIN,
            imeToolbar.panelToggle(() -> skinScroll, () -> imePanels.showSkinMenu(skinButton)));
        skinButton.setContentDescription("切换键盘皮肤");
        layoutSettingsButton = shortcutButton(controls, "设置",
            KeyboardShortcutIconPolicy.Icon.SETTINGS, this::showLayoutSettings);
        layoutSettingsButton.setContentDescription("键盘设置");
        moreButton = brandButton(controls, imeFunctionPanel::toggleFunctionPanel);
        moreButton.setContentDescription("更多");
        // 收起键：面板开着时先回到键盘（描述随之变为「返回键盘」），没有面板时收起整个键盘。
        Button dismissButton = shortcutButton(controls, "收起",
            KeyboardShortcutIconPolicy.Icon.DISMISS, () -> {
                if (anyToolbarPanelOpen()) {
                    closeToolbarPanels();
                    render();
                } else requestHideSelf(0);
            });
        dismissButton.setContentDescription("收起键盘");
        imeToolbar.installShortcutBar(dismissButton);
        // The rows are built after the controls exist: the case and delete keys are laid into the
        // last of them, and they are the same long-lived instances the rest of the host talks to.
        imeLetterRows.rebuildKeyRows();
        imeBottomRow.updateActionRow();
        expandedCandidates = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(expandedCandidates, 24, 16);
        ViewPolicy.setBackgroundColor(expandedCandidates, 0xfff5f5f5);
        expandedCandidates.setContentDescription("候选面板");
        ViewPolicy.hide(expandedCandidates);
        expandedCandidateScroll = new ScrollView(this);
        expandedCandidateScroll.setFillViewport(true);
        expandedCandidateScroll.addView(expandedCandidates,
            KeyboardGeometry.scrollMatchParentParams());
        ViewPolicy.hide(expandedCandidateScroll);
        keyboardSurface.addView(expandedCandidateScroll, KeyboardGeometry.frameMatchParentParams());
        clipboardPanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(clipboardPanel, 24, 16);
        ViewPolicy.setBackgroundColor(clipboardPanel, Color.parseColor(skin.background()));
        clipboardPanel.setContentDescription("剪贴板历史");
        clipboardScroll = new ScrollView(this);
        // 和候选、方案面板一样撑满整个键区：原先内容少时滚动视图本身是透明的，下面的键从空白处露出来。
        clipboardScroll.setFillViewport(true);
        clipboardScroll.addView(clipboardPanel, KeyboardGeometry.scrollMatchParentParams());
        ViewPolicy.hide(clipboardScroll);
        keyboardSurface.addView(clipboardScroll, KeyboardGeometry.frameMatchParentParams());
        schemePanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(schemePanel, 24, 16);
        ViewPolicy.setBackgroundColor(schemePanel, Color.parseColor(skin.background()));
        schemePanel.setContentDescription("输入方案选择器");
        schemeScroll = new ScrollView(this);
        schemeScroll.addView(schemePanel, KeyboardGeometry.scrollMatchParentParams());
        schemeScroll.setFillViewport(true);
        ViewPolicy.hide(schemeScroll);
        keyboardSurface.addView(schemeScroll, KeyboardGeometry.frameMatchParentParams());
        skinPanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(skinPanel, 24, 16);
        ViewPolicy.setBackgroundColor(skinPanel, Color.parseColor(skin.background()));
        skinPanel.setContentDescription("键盘皮肤选择器");
        skinScroll = new ScrollView(this);
        skinScroll.addView(skinPanel, KeyboardGeometry.scrollMatchParentParams());
        skinScroll.setFillViewport(true);
        ViewPolicy.hide(skinScroll);
        keyboardSurface.addView(skinScroll, KeyboardGeometry.frameMatchParentParams());
        layoutSettingsPanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(layoutSettingsPanel, 24, 16);
        ViewPolicy.setBackgroundColor(layoutSettingsPanel, Color.parseColor(skin.background()));
        layoutSettingsPanel.setContentDescription("键盘设置");
        LinearLayout layoutHeader = KeyboardGeometry.row(this);
        TextView layoutTitle = ViewPolicy.textLabel(this, "键盘设置", 18);
        KeyboardGeometry.setKeyTextSize(layoutTitle, 18);
        layoutHeader.addView(layoutTitle, KeyboardGeometry.weightedWrapParams(1));
        Button closeLayout = button(layoutHeader, "返回键盘", this::closeLayoutSettings);
        closeLayout.setContentDescription("返回键盘");
        layoutSettingsPanel.addView(layoutHeader);
        LinearLayout keyboardHeightHeader = KeyboardGeometry.row(this);
        TextView keyboardHeightLabel = textView("键盘高度");
        keyboardHeightHeader.addView(keyboardHeightLabel, KeyboardGeometry.weightedWrapParams(1));
        keyboardHeightValue = textView("");
        keyboardHeightHeader.addView(keyboardHeightValue);
        layoutSettingsPanel.addView(keyboardHeightHeader);
        keyboardHeightSlider = new SeekBar(this);
        keyboardHeightSlider.setContentDescription("键盘高度");
        configureHeightSlider(keyboardHeightSlider);
        layoutSettingsPanel.addView(keyboardHeightSlider);
        LinearLayout keySpacingHeader = KeyboardGeometry.row(this);
        TextView keySpacingLabel = textView("按键间距");
        keySpacingHeader.addView(keySpacingLabel, KeyboardGeometry.weightedWrapParams(1));
        keySpacingValue = textView("");
        keySpacingHeader.addView(keySpacingValue);
        layoutSettingsPanel.addView(keySpacingHeader);
        keySpacingSlider = new SeekBar(this);
        keySpacingSlider.setContentDescription("按键间距");
        configureSpacingSlider(keySpacingSlider, true);
        layoutSettingsPanel.addView(keySpacingSlider);
        LinearLayout rowSpacingHeader = KeyboardGeometry.row(this);
        TextView rowSpacingLabel = textView("行间距");
        rowSpacingHeader.addView(rowSpacingLabel, KeyboardGeometry.weightedWrapParams(1));
        rowSpacingValue = textView("");
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
        TextView layoutHint = textView("高度和间距只改变键位外观，不改变输入方案；松手后自动保存。");
        layoutSettingsPanel.addView(layoutHint);
        layoutSettingsScroll = new ScrollView(this);
        layoutSettingsScroll.addView(layoutSettingsPanel);
        ViewPolicy.hide(layoutSettingsScroll);
        keyboardSurface.addView(layoutSettingsScroll, KeyboardGeometry.frameMatchParentParams());
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
        ViewPolicy.hide(layoutAdjustView);
        keyboardSurface.addView(layoutAdjustView, KeyboardGeometry.frameMatchParentParams());
        voiceResultPanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(voiceResultPanel, 24, 16);
        ViewPolicy.setBackgroundColor(voiceResultPanel, Color.parseColor(skin.background()));
        voiceResultPanel.setContentDescription("语音结果面板");
        voiceResultScroll = new ScrollView(this);
        voiceResultScroll.addView(voiceResultPanel);
        ViewPolicy.hide(voiceResultScroll);
        keyboardSurface.addView(voiceResultScroll, KeyboardGeometry.frameMatchParentParams());
        aiPolishContainer = KeyboardGeometry.column(this);
        ViewPolicy.setBackgroundColor(aiPolishContainer, Color.parseColor(skin.background()));
        aiPolishPanel = KeyboardGeometry.column(this);
        ViewPolicy.setSymmetricPadding(aiPolishPanel, 24, 16);
        ViewPolicy.setBackgroundColor(aiPolishPanel, Color.parseColor(skin.background()));
        aiPolishPanel.setContentDescription("AI 润色面板");
        aiPolishScroll = new ScrollView(this);
        aiPolishScroll.setFillViewport(true);
        aiPolishScroll.addView(aiPolishPanel);
        aiPolishContainer.addView(aiPolishScroll, KeyboardGeometry.weightedWidthParams(1));
        aiPolishActions = KeyboardGeometry.column(this);
        ViewPolicy.setPadding(aiPolishActions, 24, 0, 24, 16);
        aiPolishContainer.addView(aiPolishActions, KeyboardGeometry.matchWidthWrapParams());
        ViewPolicy.hide(aiPolishContainer);
        keyboardSurface.addView(aiPolishContainer, KeyboardGeometry.frameMatchParentParams());
        moreToolsPanel = KeyboardGeometry.column(this);
        ViewPolicy.setPadding(moreToolsPanel, pixels(12), 0, pixels(12), pixels(10));
        ViewPolicy.setBackgroundColor(moreToolsPanel, Color.parseColor(skin.background()));
        moreToolsScroll = new ScrollView(this);
        moreToolsScroll.setFillViewport(true);
        moreToolsScroll.setVerticalScrollBarEnabled(false);
        moreToolsScroll.addView(moreToolsPanel, KeyboardGeometry.scrollMatchParentParams());
        ViewPolicy.hide(moreToolsScroll);
        keyboardSurface.addView(moreToolsScroll, KeyboardGeometry.frameMatchParentParams());
        phrasePanel = KeyboardGeometry.column(this);
        phrasePanel.setContentDescription("常用语面板");
        phraseScroll = new ScrollView(this);
        // 与皮肤、剪贴板面板一样铺满键盘区：只按内容高度时，没有常用语的那一行提示只盖住第一排键，空白处的触摸还会穿到下面的键上。
        phraseScroll.addView(phrasePanel, KeyboardGeometry.scrollMatchParentParams());
        phraseScroll.setFillViewport(true);
        ViewPolicy.setClickable(phraseScroll, true);
        ViewPolicy.hide(phraseScroll);
        keyboardSurface.addView(phraseScroll, KeyboardGeometry.frameMatchParentParams());
        imePanels.buildEmojiPanel();
        imePanels.buildSymbolPanel();
        renderLayoutSettingsState();
        render();
        synchronizeReplyKeyboard();
        return keyboardRoot;
    }

    /** 把从工具栏打开的面板（功能、候选展开、常用语、表情、符号、剪贴板、皮肤、输入方式、AI）的顶边对齐到顶部一行的底边：设计里这些面板打开时工具栏仍在上面。 */
    private void alignOverlaysBelowTopRow(View region) {
        View parent = (View) region.getParent();
        int top = (parent == null ? 0 : parent.getTop()) + region.getBottom();
        for (View overlay : new View[] {moreToolsScroll, expandedCandidateScroll, phraseScroll,
                emojiPanel, symbolPanel, clipboardScroll, skinScroll, schemeScroll, aiPolishContainer}) {
            if (overlay == null) continue;
            if (!(overlay.getLayoutParams() instanceof FrameLayout.LayoutParams params)
                    || params.topMargin == top) continue;
            params.topMargin = top;
            overlay.post(() -> overlay.setLayoutParams(params));
        }
    }

    private static boolean shown(View view) {
        return view != null && view.getVisibility() == View.VISIBLE;
    }

    /** 是否有从工具栏打开的面板（功能面板、表情、常用语、剪贴板、皮肤、方案、符号、AI、语音结果、旧的键盘设置）开着；开着时品牌键垫 accentSoft、收起键变为「返回键盘」。 */
    boolean anyToolbarPanelOpen() {
        return shown(moreToolsScroll) || shown(emojiPanel) || shown(phraseScroll)
            || shown(clipboardScroll) || shown(skinScroll) || shown(schemeScroll)
            || shown(symbolPanel) || shown(aiPolishContainer) || shown(voiceResultScroll)
            || shown(layoutSettingsScroll) || shown(layoutAdjustView) || replyOpen;
    }

    void closeToolbarPanels() {
        closeMoreTools();
        closeEmojiPicker();
        closeCommonPhrases();
        closeClipboardHistory();
        closeSkinPicker();
        closeSchemePicker();
        closeSymbolPanel();
        closeAiPolish();
        closeVoiceResult();
        closeLayoutSettings();
        closeReplyKeyboard();
    }

    void closeCommonPhrases() {
        if (phraseScroll != null) ViewPolicy.hide(phraseScroll);
    }

    /** 功能面板「键盘高度」：顶部一行换成内联高度条，拖动时实时预览，完成才保存，取消恢复原值。 */
    void showInlineHeight() {
        if (touchGeometrySaving || traditionalOutputSaving || session == 0
                || preferencesSnapshot == null || preferencesDirectory.isEmpty()
                || inlineHeightBar == null) {
            Toast.makeText(this, "键盘设置尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        closeToolbarPanels();
        closeCandidatePanel();
        inlineHeightOriginal = touchKeyboardHeightAdjustment;
        inlineHeightActive = true;
        inlineHeightBar.setPercent(
            KeyboardGeometry.heightAdjustmentToPercent(touchKeyboardHeightAdjustment));
        render();
        inlineHeightBar.requestFocus();
    }

    private void previewInlineHeight(int percent) {
        if (!inlineHeightActive || touchGeometrySaving || traditionalOutputSaving) return;
        touchKeyboardHeightAdjustment = KeyboardGeometry.designHeightAdjustment(
            KeyboardGeometry.heightPercentToAdjustment(percent));
        imeStyler.applyKeyboardGeometry();
    }

    private void finishInlineHeight(boolean commit) {
        if (!inlineHeightActive) return;
        inlineHeightActive = false;
        if (commit) {
            saveTouchGeometry();
        } else {
            touchKeyboardHeightAdjustment = inlineHeightOriginal;
            imeStyler.applyKeyboardGeometry();
        }
        render();
    }

    /** 功能面板开关对偏好文档的一处修改。 */
    interface PreferenceEdit {
        void apply(JSONObject preferences) throws JSONException;
    }

    /** 功能面板上直接写共享偏好的开关（模糊音、单手、隐私）：按 revision 比较并交换写回，成功后整份快照重新生效，失败时保留原值并提示。 */
    void savePanelPreference(String label, PreferenceEdit edit) {
        if (panelPreferenceSaving || traditionalOutputSaving || schemeSaving || touchGeometrySaving
                || session == 0 || preferencesSnapshot == null || preferencesDirectory.isEmpty()) {
            Toast.makeText(this, label + "设置尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        final JSONObject pending;
        final long expectedRevision;
        try {
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            edit.apply(pending.getJSONObject("preferences"));
        } catch (JSONException error) {
            preferencesNotice = " · " + label + "保存失败，保留原设置";
            render();
            return;
        }
        // 先在面板上显示新值，写回失败时由快照恢复。
        applyToolbarPreferences(pending.optJSONObject("preferences"));
        panelPreferenceSaving = true;
        final long operation = ++preferenceSaveGeneration;
        imeFunctionPanel.renderMoreTools();
        Runnable save = () -> {
            String response;
            try {
                response = NativeClient.savePreferences(targetDirectory, expectedRevision,
                    pending.toString());
            } catch (Exception | LinkageError error) {
                response = null;
            }
            final String savedResponse = response;
            main.post(() -> finishPanelPreferenceSave(operation, targetSession, targetDirectory,
                label, savedResponse));
        };
        try {
            preferencesWorker.execute(save);
        } catch (RuntimeException error) {
            finishPanelPreferenceSave(operation, targetSession, targetDirectory, label, null);
        }
    }

    private void finishPanelPreferenceSave(long operation, long targetSession,
                                           String targetDirectory, String label, String response) {
        if (!PreferencesSavePolicy.isCurrentOperation(operation, preferenceSaveGeneration,
                targetSession, session, targetDirectory, preferencesDirectory)) return;
        panelPreferenceSaving = false;
        try {
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (preferencesSnapshot != null
                    && PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1)
                        > savedRevision) {
                applyToolbarPreferences(preferencesSnapshot.optJSONObject("preferences"));
            } else {
                applyPreferencesSnapshot(saved);
            }
            preferencesNotice = "";
        } catch (JSONException | LinkageError error) {
            applyToolbarPreferences(preferencesSnapshot == null ? null
                : preferencesSnapshot.optJSONObject("preferences"));
            preferencesNotice = " · " + label + "保存失败，已恢复原设置";
            Toast.makeText(this, label + "未能保存", Toast.LENGTH_SHORT).show();
        }
        render();
    }

    void toggleFuzzyPinyin() {
        boolean target = !fuzzyPinyinEnabled;
        savePanelPreference("模糊音", preferences -> {
            JSONObject fuzzy = preferences.optJSONObject("fuzzy_pinyin");
            if (fuzzy == null) {
                fuzzy = new JSONObject();
                preferences.put("fuzzy_pinyin", fuzzy);
            }
            fuzzy.put("enabled", target);
        });
    }

    /** 单手模式：关着时打开到右侧，开着时关闭；长按在左右两侧之间换边。贴边与收窄由键盘框架按偏好实现。 */
    void toggleOneHanded(boolean swapSide) {
        String target = swapSide
            ? ("left".equals(oneHandedMode) ? "right" : "left")
            : ("off".equals(oneHandedMode) ? "right" : "off");
        saveLocalPanelSetting("单手模式", AndroidLocalSettings.ONE_HANDED, target);
    }

    void toggleIncognito() {
        saveLocalPanelSetting("隐私模式", AndroidLocalSettings.INCOGNITO, !incognitoEnabled);
    }

    /** 功能面板上写本地设置的开关（单手、隐私）：在偏好线程上写文件，写好后在主线程生效；隐私模式变了还要把会话的学习开关重新推给引擎。 */
    private void saveLocalPanelSetting(String label, String key, Object value) {
        if (panelPreferenceSaving) {
            Toast.makeText(this, label + "设置尚未就绪", Toast.LENGTH_SHORT).show();
            return;
        }
        panelPreferenceSaving = true;
        final long operation = ++localSettingSaveGeneration;
        imeFunctionPanel.renderMoreTools();
        Runnable save = () -> {
            AndroidLocalSettings.Snapshot saved;
            try {
                saved = AndroidLocalSettings.put(this, key, value);
            } catch (java.io.IOException | RuntimeException error) {
                saved = null;
            }
            final AndroidLocalSettings.Snapshot result = saved;
            main.post(() -> finishLocalPanelSetting(operation, label, result));
        };
        try {
            preferencesWorker.execute(save);
        } catch (RuntimeException error) {
            finishLocalPanelSetting(operation, label, null);
        }
    }

    private void finishLocalPanelSetting(long operation, String label, AndroidLocalSettings.Snapshot saved) {
        if (!PreferencesSavePolicy.isCurrentOperation(operation, localSettingSaveGeneration)) return;
        panelPreferenceSaving = false;
        if (saved == null) {
            preferencesNotice = " · " + label + "保存失败，保留原设置";
            Toast.makeText(this, label + "未能保存", Toast.LENGTH_SHORT).show();
            render();
            return;
        }
        boolean wasIncognito = incognitoEnabled;
        localSettings = saved;
        applyLocalSettings();
        preferencesNotice = "";
        if (wasIncognito != incognitoEnabled && session != 0) restartSessionForPrivacy();
        render();
    }

    /**
     * 隐私模式中途切换时重建引擎会话。同一修订号的快照只改 learning 会被 host-api 当作冲突拒绝（Session::update），所以不能靠 applyPreferencesSnapshot 把学习开关推给现有会话；新会话按当前的学习开关创建，随后 preferencesReloader 的首次应用与它一致，不会冲突。
     */
    private void restartSessionForPrivacy() {
        if (session == 0 || runtimeOptionsBase.isEmpty()) return;
        if (connection != null && view != null && !view.optString("editing_text", "").isEmpty())
            command(FINISH_COMPOSITION_COMMAND);
        boolean panelOpen = moreToolsScroll != null && moreToolsScroll.getVisibility() == View.VISIBLE;
        stop(false);
        // stop 会收起所有面板；用户是在功能面板上点的隐私，面板应该留在原处。
        if (panelOpen) imePanels.showFeedbackMenu();
        try {
            JSONObject options = new JSONObject(runtimeOptionsBase);
            if (learningSuppressed()) options.getJSONObject("preferences").put("learning", false);
            runtimeOptionsForSnapshot = options.toString();
            message = "共享运行时准备中";
            scheduleEngineStartup(runtimeOptionsForSnapshot, ++engineStartGeneration);
        } catch (JSONException error) {
            message = "共享运行时未就绪：仅直接输入";
        }
    }

    /** 功能面板的手写：切到手写方案；已经是手写时切回上一个中文方案。 */
    void toggleHandwritingScheme() {
        if (selectedScheme == KeyboardScheme.HANDWRITING) {
            KeyboardScheme back = KeyboardScheme.fallback(edition);
            for (KeyboardScheme scheme : enabledSchemes) {
                if (scheme != KeyboardScheme.HANDWRITING) {
                    back = scheme;
                    break;
                }
            }
            selectKeyboardScheme(back);
        } else {
            selectKeyboardScheme(KeyboardScheme.HANDWRITING);
        }
    }

    /** 词库、设置、反馈、关于：经 HostDeepLink 打开宿主的对应页面；系统拒绝从后台启动时说出来。 */
    void openHostPage(String page) {
        openHostPage(page, null);
    }

    /** 同上，带页面参数（例如输入方式面板「+ 添加语言」打开 TYPING 并带 add_language=true）。 */
    void openHostPage(String page, android.os.Bundle args) {
        Intent intent = page == null ? HostDeepLink.tab(this, HostDeepLink.TAB_SETTINGS)
            : HostDeepLink.page(this, page, args);
        try {
            startActivity(intent);
            requestHideSelf(0);
        } catch (RuntimeException error) {
            Toast.makeText(this, "无法打开设置，请从主屏幕进入", Toast.LENGTH_SHORT).show();
        }
    }

    // A disabled card swallows the press, and the 工具 section draws no state text, so without
    // this 剪贴板历史 and AI 润色 looked exactly like the cards that work and did nothing when
    // pressed. A screen reader was told "不可用"; nobody else was.
    void applyToolCardState(View card, boolean enabled) {
        ViewPolicy.setActiveAlpha(card, enabled, .45f);
    }

    void render() {
        // 旋转、设置变化或布局切换让分离式键盘该画与否变了，而键行还是按旧状态建的：先按新状态重建，下面的底行排布也会跟着换。
        if (imeLetterRows.splitStale()) imeLetterRows.rebuildKeyRows();
        updateSymbolKeyFaces();
        updateShuangpinKeyHints();
        updateQuickPunctuation();
        imeLayoutRows.updateStrokeWildcardKey();
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
        if (view != null && strictCandidatePage(view, "page_count") > 0) {
            int currentPage = strictCandidatePage(view, "page");
            int pageCount = strictCandidatePage(view, "page_count");
            if (currentPage >= 0 && pageCount > 0)
                page = " · " + (currentPage + 1) + "/" + pageCount;
        }
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
        // 正常状态下 message 为空，只有准备中、失败或提示时才有文字；后面的各项都以「 · 」开头，没有 message 时去掉打头的分隔符。
        String statusText = message + preferencesNotice
            + (dedicatedEnglish ? " · 英文输入" : "") + localMode
            + switch (letterCase.mode()) {
                case LOWERCASE -> "";
                case SHIFTED -> " · Shift";
                case CAPS_LOCK -> " · Caps Lock";
            };
        if (status != null) status.setText(statusText.startsWith(" · ") ? statusText.substring(3) : statusText);
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
            KeyboardGeometry.setKeyTextSize(preedit, candidatePreeditFontSize);
            String editingText = view == null ? "" : view.optString("editing_text", "");
            boolean offersLocalModes = idle && supportsLocalTools();
            String localModeKey = view == null ? "none" : view.optString("local_mode", "none");
            String reading = view == null ? "" : view.optString("reading", "");
            // 九键的 editing_text 只是按下的数字。读音行显示引擎给的首选读法拼音（94'26 显示 xi'an），首行是英文词时退回 preedit（带拆分分界和选过的拼音，如 ni'426）。
            String nineKeyPreedit = "";
            if (view != null && "none".equals(localModeKey)
                    && displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT) {
                nineKeyPreedit = view.optString("nine_key_reading", "");
                if (nineKeyPreedit.isEmpty()) nineKeyPreedit = view.optString("preedit", "");
            }
            String localModeTitle = "none".equals(localModeKey)
                ? (!nineKeyPreedit.isEmpty() ? nineKeyPreedit : reading.isEmpty() ? editingText : reading)
                : editingText;
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
            // 新设计去掉了空闲时的品牌药丸：空闲时读音行整行隐藏，品牌标在工具栏最左。
            brandPillVisible = false;
            String phrasePrefix = view == null ? "" : view.optString("phrase_prefix", "");
            String displayText = idleTitle
                ? (dedicatedEnglish ? "英文输入" : productName)
                : PhrasePreeditPolicy.title(phrasePrefix, localModeTitle,
                                            !"none".equals(localModeKey));
            preedit.setText(displayText);
            preedit.setContentDescription(offersLocalModes ? "长按打开本地输入模式" : displayText);
            preedit.setLongClickable(offersLocalModes);
            ViewPolicy.setFocusable(preedit, offersLocalModes);
        }
        if (exitLocalModeButton != null) {
            boolean localModeActive = view != null
                && !"none".equals(view.optString("local_mode", "none"));
            ViewPolicy.setVisible(exitLocalModeButton, localModeActive);
            ViewPolicy.setEnabled(exitLocalModeButton, localModeActive && session != 0);
            exitLocalModeButton.setContentDescription("退出本地模式");
            // The final applySkin() traversal styles this attached button once.
        }
        if (hanjaButton != null) {
            boolean offersHanja = session != 0 && koreanConvertsHanja();
            boolean offersZhuyinList = session != 0 && zhuyinOpensList();
            boolean listOpen = (offersHanja && koreanHanjaListOpen())
                || (offersZhuyinList && zhuyinListOpen());
            hanjaButton.setText(offersZhuyinList ? "選" : "漢");
            ViewPolicy.setVisible(hanjaButton, offersHanja || offersZhuyinList);
            ViewPolicy.setEnabled(hanjaButton, offersHanja || offersZhuyinList);
            ViewPolicy.setSelected(hanjaButton, listOpen);
            hanjaButton.setContentDescription(offersZhuyinList
                ? (listOpen ? "关闭候选列表" : "打开候选列表")
                : (listOpen ? "关闭汉字列表" : "转换为汉字"));
        }
        boolean hasDiagnostic = InputDiagnosticPolicy.visible(diagnosticMessage);
        imeDebugOverlay.updateDiagnosticView(hasDiagnostic);
        boolean heightMode = inlineHeightActive;
        if (inlineHeightBar != null)
            ViewPolicy.setVisible(inlineHeightBar, heightMode);
        if (candidateHeader != null) {
            // 空闲时这一行只给常驻的模式标签（直接输入、准备中）：简繁切换、设置保存、同步重试、Shift 这类一闪而过的提示若也占这一行，每次出现和消失都把整副键盘顶上去又落回来。临时提示只在失败时以 Toast 说出来。
            boolean modeLabel = !message.isEmpty();
            ViewPolicy.setVisible(candidateHeader, !heightMode && !hasDiagnostic && (!idle || modeLabel));
            if (idle && !modeLabel) announceIdleNotice(preferencesNotice);
        }
        if (shortcutScroll != null)
            ViewPolicy.setVisible(shortcutScroll, !heightMode && idle && !hasDiagnostic && !toolbarHidden);
        if (candidateLine != null)
            ViewPolicy.setVisible(candidateLine, !heightMode && !idle && !hasDiagnostic);
        if (replyKeyboard == null || replyKeyboard.getVisibility() != View.VISIBLE)
            imeBottomRow.updateActionRow();
        if (candidateViewport != null)
            ViewPolicy.setVisible(candidateViewport, !idle && !hasDiagnostic);
        updateCandidateViewportHeight();
        if (scriptShortcutButton != null) {
            ViewPolicy.hide(scriptShortcutButton);
            scriptShortcutButton.setText(traditionalChineseOutput ? "繁" : "简");
            ViewPolicy.setSelected(scriptShortcutButton, traditionalChineseOutput);
            imeStyler.styleButton(scriptShortcutButton, true);
            int scheme = view == null
                ? ((selectedScheme == KeyboardScheme.JAPANESE
                    || selectedScheme == KeyboardScheme.JAPANESE_NINE_KEY) ? 3
                    : selectedScheme == KeyboardScheme.KOREAN ? KoreanInputPolicy.KOREAN_SCHEME
                    : selectedScheme == KeyboardScheme.CANTONESE ? InputSchemeTraits.CANTONESE
                    : selectedScheme == KeyboardScheme.ZHUYIN
                        || selectedScheme == KeyboardScheme.ZHUYIN_NINE_KEY ? InputSchemeTraits.ZHUYIN
                    : selectedScheme == KeyboardScheme.VIETNAMESE ? InputSchemeTraits.VIETNAMESE
                    : selectedScheme == KeyboardScheme.TIBETAN ? InputSchemeTraits.TIBETAN
                    : selectedScheme == KeyboardScheme.STROKE ? InputSchemeTraits.STROKE : -1)
                : InputViewValuePolicy.scheme(view, -1);
            boolean japanese = scheme == 3;
            boolean korean = scheme == KoreanInputPolicy.KOREAN_SCHEME;
            boolean cantonese = scheme == InputSchemeTraits.CANTONESE;
            boolean zhuyin = scheme == InputSchemeTraits.ZHUYIN;
            boolean vietnamese = scheme == InputSchemeTraits.VIETNAMESE;
            boolean tibetan = scheme == InputSchemeTraits.TIBETAN;
            boolean stroke = scheme == InputSchemeTraits.STROKE;
            ViewPolicy.setEnabled(scriptShortcutButton,
                !japanese && !korean && !cantonese && !zhuyin
                    && !vietnamese && !tibetan && !stroke && canSaveChineseOutput());
            String label = traditionalChineseOutput ? "切换到简体" : "切换到繁体";
            String outputState = japanese ? "日语不使用简繁转换"
                : korean ? "韩语不使用简繁转换"
                : cantonese ? "粤拼直接输出繁体"
                : zhuyin ? "注音直接输出繁体"
                : vietnamese ? "越南语不使用简繁转换"
                : tibetan ? "藏文不使用简繁转换"
                : stroke ? "笔画不使用简繁转换"
                : traditionalOutputSaving ? "正在保存"
                : traditionalChineseOutput ? "繁体" : "简体";
            scriptShortcutButton.setContentDescription(
                Build.VERSION.SDK_INT >= 30 ? label : label + "，" + outputState);
            if (Build.VERSION.SDK_INT >= 30)
                scriptShortcutButton.setStateDescription(outputState);
        }
        if (emojiShortcutButton != null) {
            ViewPolicy.setVisible(emojiShortcutButton, toolbarEmoji);
            ViewPolicy.setEnabled(emojiShortcutButton, session != 0 && !emojiResources.isEmpty());
            ViewPolicy.setSelected(emojiShortcutButton, emojiPanel != null
                && emojiPanel.getVisibility() == View.VISIBLE);
        }
        if (phraseShortcutButton != null) {
            ViewPolicy.setVisible(phraseShortcutButton, toolbarPhrase);
            ViewPolicy.setEnabled(phraseShortcutButton, session != 0 && !preferencesDirectory.isEmpty());
            ViewPolicy.setSelected(phraseShortcutButton, phraseScroll != null
                && phraseScroll.getVisibility() == View.VISIBLE);
        }
        if (clipboardShortcutButton != null) {
            ViewPolicy.setVisible(clipboardShortcutButton, toolbarClipboard);
            ViewPolicy.setEnabled(clipboardShortcutButton, CloudClipboardPanelPolicy.panelAvailable(
                clipboardHistoryEnabled, imePanels.cloudClipboardAllowed()));
            ViewPolicy.setSelected(clipboardShortcutButton, imePanels.clipboardPanelOpen());
        }
        if (dismissShortcutButton != null) {
            dismissShortcutButton.setContentDescription(anyToolbarPanelOpen() ? "返回键盘" : "收起键盘");
        }
        if (voiceShortcutButton != null) {
            ViewPolicy.hide(voiceShortcutButton);
            ViewPolicy.setEnabled(voiceShortcutButton, voiceInsertionReady());
        }
        if (aiPolishShortcutButton != null) {
            ViewPolicy.hide(aiPolishShortcutButton);
            ViewPolicy.setEnabled(aiPolishShortcutButton, aiPolishReady());
        }
        if (replyShortcutButton != null) {
            // 回复面板不属于任何输入方案，每个方案都显示这个入口；未配置 AI 时面板里会提示去设置。开着时始终可点，用来收起面板。
            ViewPolicy.hide(replyShortcutButton);
            ViewPolicy.setEnabled(replyShortcutButton, replyOpen || aiPolishReady());
            ViewPolicy.setSelected(replyShortcutButton, replyOpen);
            imeStyler.styleButton(replyShortcutButton, KeyboardKeyRole.GLYPH, skin);
            replyShortcutButton.setContentDescription(replyOpen ? "收起高情商回复" : "生成高情商回复");
        }
        if (microsoftFinalKey != null) {
            String currentLocalMode = view == null ? "none" : view.optString("local_mode", "none");
            boolean visible = MicrosoftShuangpinKeyPolicy.visible(
                dedicatedEnglish, selectedScheme, currentLocalMode);
            ViewPolicy.setVisible(microsoftFinalKey, visible);
            ViewPolicy.setEnabled(microsoftFinalKey, visible && session != 0);
            microsoftFinalKey.setContentDescription("微软双拼 ing");
        }
        if (layerButton != null) {
            boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
            layerButton.setText(KeyboardActionRow.layerTitle(displayedTouchLayout(view), symbols));
            layerButton.setContentDescription(KeyboardActionRow.layerDescription(symbols));
        }
        if (symbolPanelButton != null) {
            ViewPolicy.setEnabled(symbolPanelButton, session != 0 && connection != null
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
            ViewPolicy.setVisible(shiftButton, KeyboardLayout.carriesLetterCase(shiftLayout)
                || (!keepsOwnGrid && keyboardLayer != KeyboardLayout.Layer.LETTERS));
            shiftButton.setText(letterCase.keyText());
            ViewPolicy.setSelected(shiftButton, letterCase.usesUppercase());
            shiftButton.setActivated(letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
            // The final applySkin() traversal styles this attached button once.
            String caseLabel = shiftLayout == KeyboardLayout.KOREAN_LAYOUT
                ? KoreanKeyboardLayout.SHIFT_LABEL
                : letterCase.accessibilityLabel(dedicatedEnglish || session == 0
                    || letterCaseSchemeActive());
            String caseValue = letterCase.accessibilityValue();
            shiftButton.setContentDescription(Build.VERSION.SDK_INT >= 30
                ? caseLabel : caseLabel + "，" + caseValue);
            if (Build.VERSION.SDK_INT >= 30) shiftButton.setStateDescription(caseValue);
        }
        if (languageButton != null) {
            languageButton.setText(dedicatedEnglish ? "英" : "中");
            ViewPolicy.setEnabled(languageButton, session != 0);
            languageButton.setContentDescription(
                dedicatedEnglish ? "切换到所选输入方案" : "切换到英文输入");
            if (Build.VERSION.SDK_INT >= 30) {
                languageButton.setStateDescription(dedicatedEnglish ? "英文输入" : "中文输入");
            }
        }
        if (schemeButton != null) {
            ViewPolicy.setVisible(schemeButton, toolbarScheme);
            ViewPolicy.setSelected(schemeButton,
                schemeScroll != null && schemeScroll.getVisibility() == View.VISIBLE);
            schemeButton.setText(selectedScheme.glyph() + selectedScheme.badge(wubiProfile));
            schemeButton.setContentDescription("输入方案：" + selectedScheme.title(wubiProfile));
            // 只按「会话与偏好是否就绪」决定可用：简繁、键高、方案的保存都在一瞬间完成，若跟着保存状态禁用，图标每切一次简繁就变灰再变回来。保存进行中的点按由 showSchemePicker 忽略。
            ViewPolicy.setEnabled(schemeButton, session != 0 && preferencesSnapshot != null);
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
            ViewPolicy.setVisible(skinButton, toolbarSkin);
            ViewPolicy.setSelected(skinButton,
                skinScroll != null && skinScroll.getVisibility() == View.VISIBLE);
            // 同上：保存进行中的点按由 ImePanels.showSkinPicker 按 canSaveKeyboardSkin 忽略，图标不跟着变灰。
            ViewPolicy.setEnabled(skinButton, session != 0 && preferencesSnapshot != null
                && !preferencesDirectory.isEmpty());
            skinButton.setContentDescription("切换键盘皮肤；当前" + skin.title());
            if (Build.VERSION.SDK_INT >= 30) skinButton.setStateDescription(skin.title());
        }
        synchronizeReplyKeyboard();
        if (layoutSettingsButton != null) ViewPolicy.hide(layoutSettingsButton);
        if (layoutSettingsButton != null)
            ViewPolicy.setEnabled(layoutSettingsButton, session != 0 && preferencesSnapshot != null
                && !schemeSaving && !touchGeometrySaving && !traditionalOutputSaving);
        imeLayoutRows.renderNineKeySpellings();
        if (hasDiagnostic && nineKeySpellingScroll != null)
            ViewPolicy.hide(nineKeySpellingScroll);
        scheduleCandidateGlosses();
        scheduleCandidateTranslations();
        scheduleOnlineProviders();
        if (candidates == null) {
            imeStyler.applySkin();
            imeToolbar.styleTopRow();
            return;
        }
        // 注音 9 键的读音选择条叠在候选区上：选择条只在候选列表关着时出现，那时候选行本来就是空的，所以把候选滚动区让成不可见（仍占位，键盘高度不变）。
        boolean zhuyinSpellingsShown = nineKeySpellingScroll != null
            && nineKeySpellingScroll.getVisibility() == View.VISIBLE
            && displayedTouchLayout(view) == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
        int candidateScrollShown = zhuyinSpellingsShown ? View.INVISIBLE : View.VISIBLE;
        if (horizontalCandidateScroll != null) {
            if (!candidateHorizontal || hasDiagnostic) ViewPolicy.hide(horizontalCandidateScroll);
            else if (candidateScrollShown == View.VISIBLE) ViewPolicy.show(horizontalCandidateScroll);
            else ViewPolicy.setInvisible(horizontalCandidateScroll);
        }
        if (verticalCandidateScroll != null) {
            if (candidateHorizontal || hasDiagnostic) ViewPolicy.hide(verticalCandidateScroll);
            else if (candidateScrollShown == View.VISIBLE) ViewPolicy.show(verticalCandidateScroll);
            else ViewPolicy.setInvisible(verticalCandidateScroll);
        }
        LinearLayout activeCandidates = candidateHorizontal ? candidates : verticalCandidates;
        candidates.removeAllViews();
        if (verticalCandidates != null) verticalCandidates.removeAllViews();
        if (expandCandidates != null) ViewPolicy.hide(expandCandidates);
        if (view == null) {
            closeCandidatePanel();
            renderEnglishSuggestions(activeCandidates);
            for (Button button : candidateButtons) ViewPolicy.hide(button);
            imeStyler.applySkin();
            imeToolbar.styleTopRow();
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
                    candidateButtons.add(imeCandidates.makeCandidateButton(candidateButtons.size()));
                Button candidateView = candidateButtons.get(slot);
                ViewPolicy.show(candidateView);
                updateCandidateButton(candidateView, candidate, slot);
                activeCandidates.addView(candidateView, new LinearLayout.LayoutParams(
                    candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                        : LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT));
            }
            if (!hasDiagnostic && strictCandidatePage(view, "page_count") > 1
                    && expandCandidates != null)
                ViewPolicy.show(expandCandidates);
        }
        int visibleSlots = entries == null ? 0 : entries.length();
        for (int slot = visibleSlots; slot < candidateButtons.size(); slot++)
            ViewPolicy.hide(candidateButtons.get(slot));
        if (directEnglishActive()) {
            for (Button button : candidateButtons) ViewPolicy.hide(button);
        }
        if (hasDiagnostic) closeCandidatePanel();
        resetCandidateScrollIfViewChanged();
        imeCandidates.renderExpandedCandidates();
        if (moreToolsScroll != null && moreToolsScroll.getVisibility() == View.VISIBLE)
            imeFunctionPanel.renderMoreTools();
        imeStyler.applySkin();
        imeToolbar.styleTopRow();
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
        long nextSession = CandidateGlossPolicy.strictOr(view.opt("session"), session);
        long nextGeneration = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        int nextPage = strictCandidatePage(view, "page");
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
            ViewPolicy.show(handwritingStatus);
            return;
        }
        ViewPolicy.hide(handwritingStatus);
        for (int index = 0; index < handwritingResults.size(); index++) {
            String candidate = handwritingResults.get(index);
            HandwritingRequestTracker.Token token = handwritingCandidateToken;
            Button choice = keyboardKey(chineseOutput(candidate, view),
                "手写候选 " + (index + 1), () -> commitHandwritingCandidate(token, candidate));
            KeyboardGeometry.setKeyTextSize(choice, candidateFontSize);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
            activeCandidates.addView(choice, params);
        }
    }
}
