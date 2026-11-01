package app.msime.android;

import app.msime.android.LayoutPolicy;


import android.inputmethodservice.InputMethodService;
import app.msime.android.core.Telemetry;
import app.msime.android.core.InputViewValuePolicy;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ClipDescription;
import android.content.Context;
import android.database.ContentObserver;
import android.provider.Settings;
import android.content.Intent;
import android.content.ClipboardManager;
import android.content.SharedPreferences;
import android.content.res.Configuration;
import app.msime.android.CustomSkinLibrary;
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
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.os.Build;
import android.os.Bundle;
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
import android.view.KeyCharacterMap;
import android.view.Menu;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewGroup;
import android.util.TypedValue;
import android.widget.PopupMenu;
import android.widget.PopupWindow;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputContentInfo;
import android.view.inputmethod.InputMethodManager;
import android.webkit.MimeTypeMap;
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
import java.io.IOException;
import java.io.InputStream;
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
    static final int FOURTEEN_KEY_LAYOUT = KeyboardLayout.FOURTEEN_KEY_LAYOUT;
    private static final long HANDWRITING_DEBOUNCE_MILLIS = 550;
    /** 会让「键盘调整」记下的键盘屏幕上沿作废的配置变化：方向、屏幕尺寸（含分屏）和密度（显示大小）。 */
    private static final int KEYBOARD_ADJUST_SCREEN_CHANGES = android.content.pm.ActivityInfo.CONFIG_ORIENTATION
        | android.content.pm.ActivityInfo.CONFIG_SCREEN_SIZE
        | android.content.pm.ActivityInfo.CONFIG_SMALLEST_SCREEN_SIZE
        | android.content.pm.ActivityInfo.CONFIG_DENSITY;
    static final long BACKSPACE_REPEAT_DELAY_MILLIS = 400;
    static final long BACKSPACE_REPEAT_INTERVAL_MILLIS = 75;
    private static final long PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS = 500;
    private static final long INPUT_VIEW_REFRESH_DELAY_MILLIS = 32;
    /** How long counted key presses wait in memory before they are written anyway. */
    private static final long KEY_PRESS_FLUSH_DELAY_MILLIS = 30_000;
    private static final String INPUT_MODE_PREFERENCES = "android-input-modes";
    static final String EMOJI_RECENTS_PREFERENCES = "android-emoji-recents";
    /** 剪贴板补读用的「上一次处理过的那一条」，只存身份（复制时刻和文字散列），见 {@link ClipboardCapturePolicy}。 */
    private static final String CLIPBOARD_CAPTURE_PREFERENCES = "android-clipboard-capture";
    private static final String HANDLED_CLIP_KEY = "handled_clip";
    private static final String EMOJI_RECENTS_KEY = "items";
    private static final String SPACE_CURSOR_DESCRIPTION =
        "空格；轻点输入空格或选词，左右滑动移动光标";
    private static final int CAPITALIZATION_CONTEXT_LIMIT = 128;
    /**
     * Shared host command 9, `Action::Finish`: commit the highlighted candidate and whatever the
     * composition still holds. This is Apple's `finishComposition`, not `CommitRaw`, so a
     * composition ended this way reaches the editor as 你好 rather than as the pinyin letters.
     */
    static final int FINISH_COMPOSITION_COMMAND = 9;
    /** Shared host command 10 delegates small-kana, dakuten and handakuten cycling to Engine. */
    static final int CYCLE_KANA_VARIANT_COMMAND = 10;
    // 键盘的构建、渲染与扩展点（见各类说明）；onCreate 里创建。
    ImeToolbar imeToolbar;
    ImeCompositionEditor imeCompositionEditor;
    ImeCandidates imeCandidates;
    ImeNineKeyPanel imeNineKeyPanel;
    ImeFunctionPanel imeFunctionPanel;
    ImeWorldClock imeWorldClock;
    ImeOcr imeOcr;
    ImePanels imePanels;
    ImeLetterRows imeLetterRows;
    ImeGlideTyping imeGlideTyping;
    ImeTouchPoints imeTouchPoints;
    ImeBottomRow imeBottomRow;
    ImeBottomBar imeBottomBar;
    ImeLayoutRows imeLayoutRows;
    ImeStyler imeStyler;
    ImeFrame imeFrame;
    ImePrivacyGate imePrivacyGate;
    ImeVoiceEntry imeVoiceEntry;
    ImeKeyFeedback imeKeyFeedback;
    ImeDebugOverlay imeDebugOverlay;
    private final ImeOperationNotice operationNotice = new ImeOperationNotice(this);
    ImeTextEditPanel imeTextEditPanel;
    ImeCalculator imeCalculator;
    ImeEmailSuffixes imeEmailSuffixes;
    ImeNineKeyQuickPunctuation imeNineKeyQuickPunctuation;
    ImeTranslationSenses imeTranslationSenses;
    ImeStylusHandwriting imeStylusHandwriting;
    ImeStickers imeStickers;
    /** 「我的词条」经 WebDAV 网盘同步在键盘进程里的触发（改词后记录、按 Rust 给的间隔定时），见 {@link ImeDictionarySync}。 */
    private ImeDictionarySync imeDictionarySync;
    long session;
    InputConnection connection;
    private EditorBridge bridge = new EditorBridge();
    // 隐藏状态属于当前编辑器的整段组字，不能随每次 apply 创建的 Sink 重置。
    private boolean editorCompositionSuppressed;
    JSONObject view;
    FrameLayout keyboardRoot;
    FrameLayout keyboardSurface;
    private LinearLayout candidates;
    private LinearLayout verticalCandidates;
    FrameLayout candidateViewport;
    CandidateCanvasColumn candidateCanvasRegion;
    CandidateCanvasFrame expandedCandidateCanvas;
    private HorizontalScrollView horizontalCandidateScroll;
    private ScrollView verticalCandidateScroll;
    private final java.util.List<Button> candidateButtons = new java.util.ArrayList<>();
    final java.util.List<CandidateCanvasAssets.Word> candidateDecorationWords = new java.util.ArrayList<>();
    /**
     * 候选条上有手指按着，这一帧的候选内容推迟到松开之后再画。
     *
     * <p>候选条每次 render 都 `removeAllViews` 再逐个挂回去，候选对象本身是复用的，但摘下来的那一瞬间框架就把这次按压变成 CANCEL，点击不再发出，用户看到的是「候选偶尔点不上」。候选内容又几乎每帧都在变（组字的异步回包、译文角标），所以这个窗口并不罕见。按钮通过 {@link #bindCandidateRenderRelease} 在按下状态真正清除后补一次 render，避免 ACTION_UP 早于点击和延迟松开时漏掉补帧。
     */
    boolean candidateRenderDeferred;
    /** 候选触摸保护延续到框架排队的点击之后；每次按下用独立标记，旧松开不能清掉下一次按下。 */
    private final java.util.Map<View, Object> candidateTouches = new java.util.WeakHashMap<>();
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
    /** 拼音九键与 14 键的侧栏纵向滚动，注音 9 键候选行横向滚动。 */
    FrameLayout nineKeySpellingScroll;
    final java.util.List<Button> nineKeySpellingButtons = new java.util.ArrayList<>();
    java.util.List<Integer> nineKeySpellingIndices = java.util.List.of();
    long nineKeySpellingGeneration = -1;
    private long candidateScrollSession = -1;
    private long candidateScrollGeneration = -1;
    private int candidateScrollPage = -1;
    Button expandCandidates;
    /** 上屏后联想时展开键左边的关闭键（#10078），显示与否由 {@link PredictionButtonsPolicy} 和 {@link #predictionButtons} 决定。 */
    Button closePrediction;
    boolean candidatePanelOpen;
    JSONObject candidatePanelSnapshot;
    final ImeCandidateStrip imeCandidateStrip = new ImeCandidateStrip(this);
    PopupWindow nineKeyHoldPopup;
    ScrollView clipboardScroll;
    FrameLayout clipboardSurface;
    LinearLayout clipboardHeader;
    Button clipboardBackToTop;
    /** 删除（单条或多选）后浮在剪贴板面板右下角的大号「撤销」，带倒数（#10073、#10080）；没有可撤销的记录时收起。 */
    Button clipboardUndoButton;
    LinearLayout clipboardPanel;
    ScrollView schemeScroll;
    LinearLayout schemePanel;
    ScrollView skinScroll;
    LinearLayout skinPanel;
    private ScrollView layoutSettingsScroll;
    LinearLayout layoutSettingsPanel;
    KeyboardLayoutAdjustView layoutAdjustView;
    ScrollView moreToolsScroll;
    /** 功能面板容器的底部内边距，面板选行数时算进网格以外的高度。 */
    static final int MORE_TOOLS_BOTTOM_PADDING_DP = 10;
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
    ClipboardModePolicy.Mode clipboardMode = ClipboardModePolicy.Mode.MAIN;
    boolean clipboardHistoryEnabled;
    CloudClipboardPanelPolicy.Tab clipboardTab = CloudClipboardPanelPolicy.Tab.LOCAL;
    CloudClipboardPanelPolicy.Status cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    java.util.List<BackendAccount.ClipboardItem> cloudClipboardItems = java.util.List.of();
    /** 当前画出来的云端条目归谁：水杉账号编号，或者配置了自建中继时中继的绑定（`relay:` 开头，见 CloudClipboardRelayPolicy#binding）；空串表示没有。 */
    String cloudClipboardAccountId = "";
    /** 账号的登录代次；中继为 0。 */
    long cloudClipboardBindingGeneration = -1L;
    /** 供出当前这一页的账号会话（中继时是中继的绑定）；上传只发给它。 */
    String cloudClipboardSessionId = "";
    // Bumped whenever the field or the open panel changes; a cloud answer started under an older value is dropped rather than drawn into a field it was not fetched for.
    long cloudClipboardGeneration;
    private boolean candidateEnglishGloss;
    private boolean candidatePronunciationEnabled;
    private boolean candidatePronunciationAvailable;
    private boolean candidatePronunciationPackInstalled;
    private java.util.Map<String, String> candidatePronunciations = java.util.Map.of();
    private String candidatePronunciationRequest = "";
    private long candidatePronunciationEpoch;
    private JSONObject candidatePronunciationSourceView;
    private JSONObject candidatePronunciationSourcePanel;
    private java.util.List<String> candidatePronunciationSourceSuggestions = java.util.List.of();
    private long candidatePronunciationSourceSession;
    private String candidatePronunciationPreferences = "";
    private boolean candidateTranslationsEnabled;
    // Gates only the account network path (fetch, apply, reserved rows); candidateTranslationsEnabled stays the display gate for translations a candidate already carries.
    private boolean candidateTranslationAccount;
    private boolean englishSuggestionsEnabled = true;
    private String englishCorrectionRejectedPrefix = "";
    private java.util.List<String> candidateTranslationTargets = java.util.List.of("en");
    // 候选词翻译的范围（`candidate_translation_scope`）。生效时（见 candidateTranslationScopeLimited）账号、用户自己的服务和 AI 只为视图里高亮的那个候选请求，离线释义照旧整页。
    private CandidateTranslationPolicy.Scope candidateTranslationScope = CandidateTranslationPolicy.Scope.HIGHLIGHTED;
    private CandidateTranslationStore candidateTranslationStore;
    private long candidateTranslationBindingEpoch;
    private long candidateTranslationVerifiedGeneration = -1L;
    private long candidateTranslationAppliedGeneration = -1L;
    // 候选翻译这一次走哪个服务（含凭据），由 CandidateTranslationPolicy.route 从偏好算出；选的是用户自己的服务（小牛、自定义、腾讯）时，译文由 userTranslationStore 取，它与账号的 candidateTranslationStore 并行、互不复用。路由不可变，变了就整个换掉 userTranslationStore。
    private CandidateTranslationRoute candidateTranslationRoute = CandidateTranslationRoute.NONE;
    /** 用户自己的服务一次最多翻译的候选数，与共享层的每页候选上限（`MAX_CANDIDATE_PAGE_SIZE`）相同。 */
    private static final int USER_TRANSLATION_PAGE = 10;
    private CandidateTranslationStore userTranslationStore;
    // 用户服务的客户端连同它记下的回答和退避（UserTranslationMemory），只在偏好算出另一条路由时才换掉；输入会话结束时 stop() 只把路由暂置为 NONE，下一次会话算出同一路由就接着用它，换输入框、收起再弹出键盘都不会把问过的词重新发一遍。
    private UserTranslationClient userTranslationClient;
    private long userTranslationAppliedGeneration = -1L;
    private boolean wubiCodeHint = true;
    private boolean wubiMixedPinyin;
    // 五笔版本只决定方案卡片和工具栏上的「86」「98」字样；选表由引擎按同一份偏好里的 `wubi_profile` 决定。
    String wubiProfile = KeyboardScheme.WUBI_86;
    private String candidateGlossResources = "";
    // 会话的 `preferences_directory`（state_root）：随包没有的非英文离线释义从它下面下载的 offline-glosses 资源包里读。
    private String candidateGlossStateRoot = "";
    private String onlineSignature = "";
    /**
     * 问过 AI 却没有结果的那次请求的签名：同一组字不再向 AI 发请求（#6888）。
     *
     * <p>失败后释放 {@link #onlineSignature} 是为了让云候选下次渲染时重试；没有这一条，AI 会跟着在每次渲染时重问；释义到达、设置轮询都会重画，模拟器上一个回答为空的组字因此每秒发一次 AI 请求。组字一变就清掉，回到这个组字时重新问。
     */
    private volatile String aiResolvedSignature = "";
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
    /** 第二行是否为纠错；等待查询时只显示原文，点它仍按拒绝纠错处理。 */
    private boolean englishSuggestionCorrected;
    private String clearableEnglishWord = "";
    private int clearableEnglishCursor = -1;
    private InputConnection clearableEnglishConnection;
    /** 英文直输一次按键或一次选区回报里，光标位置和光标前后文每样只跨进程读一次，见 EditorReadCache。 */
    private final EditorReadCache editorReads = new EditorReadCache();
    /** 大于 0 时 render() 只记下要画，最外层的 endRenderBatch 再画一次，见 beginRenderBatch。 */
    private int renderBatchDepth;
    private boolean renderBatchPending;
    /** 最近一次 render() 开始时选区回声追踪器记下过的预期数，见 updateEditorSelection 里的 renderedEcho。 */
    private long renderedEchoExpectations = -1;
    private long englishSuggestionEpoch;
    private long englishSuggestionRequestedEpoch = -1;
    private String englishSuggestionRequestedPrefix = "";
    private boolean candidateHorizontal = true;
    int candidateFontSize = 16;
    int candidatePreeditFontSize = 16;
    int candidateScalePercent = 100;
    /** 拼音显示在候选栏还是输入框（`touch_spelling_position`）；共享偏好缺失或非法时沿用输入框。 */
    private String spellingPosition = CandidatePreeditStylePolicy.EDITOR;
    CandidateAppearance.Palette candidateAppearance =
        CandidateAppearance.fromSkin(KeyboardSkin.system(false));
    int touchKeySpacingTenths = KeyboardSpacingPolicy.DEFAULT_KEY_SPACING_TENTHS;
    int touchRowSpacingTenths = KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS;
    int touchKeyboardHeightAdjustment = KeyboardHeightPolicy.DEFAULT_HEIGHT_ADJUSTMENT_DP;
    /** 工具栏语音按钮，来自共享偏好 `touch_voice_shortcut`，缺省为关；在「键盘 → 工具栏」打开「语音」或键盘内打开「顶部语音入口」时为真（#10142）；「语音输入 → 启动方式」选「无」会关掉它，选「工具栏/长按空格」不改它（#10309）。 */
    boolean touchVoiceShortcutEnabled;
    /** 26 键双拼的字母键画不画声母/韵母提示，来自共享偏好 `touch_shuangpin_key_hints`，缺省为关。 */
    private boolean shuangpinKeyHintsEnabled = false;

    /** 全拼 26 键组字时 ⇧ 保留（臂辅助码）而不换成「分词」，来自共享偏好 `touch_quanpin_shift`，缺省为开。 */
    private boolean quanpinShiftEnabled = true;
    /** 上屏后联想时候选栏右端的按键样式，来自共享偏好 `next_word_prediction_buttons`，缺省为现有的只有展开键。 */
    private PredictionButtonsPolicy.Style predictionButtons = PredictionButtonsPolicy.Style.EXPAND_ONLY;
    /** 26 键、14/18 键与九键各自的底行样式，来自共享偏好 `touch_*_key_style`，缺省都是标准底行。 */
    TouchKeyStyle.Styles touchKeyStyles = TouchKeyStyle.Styles.DEFAULT;
    /** 26 键底行 符 排在 123 左边，来自共享偏好 `touch_twenty_six_symbol_first`，缺省为关。 */
    boolean twentySixSymbolFirst;
    /** 九宫格数字键盘里空格的位置，来自共享偏好 `touch_number_pad_space`，缺省是 Android 原来的右列。 */
    TouchKeyStyle.NumberPadSpace numberPadSpace = TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT;
    /** 九键左侧符号栏字母键面那张表，来自共享偏好 `touch_nine_key_sidebar_symbols`；空表表示用出厂表（{@link NineKeySidebarPolicy}）。 */
    java.util.List<String> nineKeySidebarSymbols = java.util.List.of();
    /** 九宫格数字键盘左列那张表，来自共享偏好 `touch_nine_key_digit_sidebar_symbols`；空表表示用出厂表。 */
    java.util.List<String> nineKeyDigitSidebarSymbols = java.util.List.of();
    /** 大屏单手模式空出来那一侧放什么（#10203），来自共享偏好 `touch_one_handed_side_panel`，缺省空着；画不画还要看单手、屏宽和键面，见 {@link OneHandSidePadPolicy#shown}。 */
    OneHandSidePadPolicy.Panel oneHandedSidePanel = OneHandSidePadPolicy.Panel.OFF;
    /** 中/英键排在空格左边，来自共享偏好 `touch_language_key_side`（只有 `left` 算），缺省在右边、紧挨回车。 */
    boolean languageKeyLeft;
    /** 拼音九键「九键滑动输入数字」的方向，来自共享偏好 `touch_nine_key_swipe_digits`（{@link NineKeySwipePolicy#mode}），缺省关闭。只影响按键的触摸判定，不改键面，所以不进 touchGeometryKey；每次触摸判定时读当前值，改了下一次触摸就生效。 */
    String nineKeySwipeDigits = NineKeySwipePolicy.OFF;
    /** 日语九键连点同键时是否循环假名，旧设置和新装默认都开启。 */
    boolean japaneseToggleInputEnabled = InputFeatureToggle.JAPANESE_TOGGLE_INPUT.enabledByDefault();
    private boolean voiceInputEnabled = true;
    private String voiceLanguage = "zh-CN";
    KeyboardSkin skin = KeyboardSkin.system(false);
    private java.util.List<CustomSkinLibrary.Item> modeSkinLibrary = java.util.List.of();
    private int modeSkinLibraryGeneration;
    /** 表情面板有自己的明暗设置，跟随时才继承键盘皮肤解析出的明暗。 */
    KeyboardSkin emojiSkin = KeyboardSkin.system(false);
    KeyboardSkin handwritingSkin = KeyboardSkin.system(false);
    /** The shared theme catalog (`msime_client_theme_catalog`), read once: ids, titles and palettes are fixed per build. */
    JSONArray themeCatalog;
    /** Resolved keyboards by request, so the four surfaces of one snapshot cost at most two native calls. */
    private final java.util.Map<String, KeyboardSkin> resolvedThemes = new java.util.HashMap<>(8);
    /** 键盘设计按对象记下的内容散列：设计里可能有半兆以上的 base64 照片，同一份快照的四块面板各自 toString 一遍才能查缓存。键盘进程不原地改设计对象，换了内容就是换了对象。 */
    private final java.util.WeakHashMap<JSONObject, Integer> designHashes = new java.util.WeakHashMap<>();
    /** 输入法自己写进编辑器后预期的选区，用来认出 `onUpdateSelection` 里迟到的回声。 */
    final SelectionEchoTracker selectionEcho = new SelectionEchoTracker();
    private JSONObject localModes = new JSONObject();
    Button moreButton;
    Button schemeButton;
    /** 工具栏上的「浮动键盘」按钮（本地设置 TOOLBAR_FLOATING，默认不显示）。 */
    Button floatingShortcutButton;
    Button textEditShortcutButton;
    Button undoShortcutButton;
    Button redoShortcutButton;
    Button skinButton;
    Button layoutSettingsButton;
    Button scriptShortcutButton;
    Button emojiShortcutButton;
    Button symbolShortcutButton;
    Button voiceShortcutButton;
    Button randomDigitsShortcutButton;
    Button aiPolishShortcutButton;
    Button replyShortcutButton;
    Button phraseShortcutButton;
    Button clipboardShortcutButton;
    Button dismissShortcutButton;
    /** 顶部一行里组词时的读音行与候选行。 */
    LinearLayout candidateHeader;
    LinearLayout candidateLine;
    /** 「键盘调整」遮罩（功能面板「键盘调整」）：盖在整块键盘上，拖四条边和「移动」实时预览高度、底部抬高、宽度和水平位置，确认或任何离开方式都保存。 */
    KeyboardAdjustOverlay keyboardAdjustOverlay;
    boolean keyboardAdjustActive;
    /** 调整中预览的底部抬高（dp，键区下沿到底栏或导航区的额外距离）与宽度、水平位置；高度预览直接写在 {@link #touchKeyboardHeightAdjustment}。 */
    private int keyboardAdjustLiftDp;
    private KeyboardAdjustPolicy.Horizontal keyboardAdjustHorizontal = KeyboardAdjustPolicy.Horizontal.DEFAULT;
    /** 打开时的抬高与宽度、位置：保存时只写变了的项。 */
    private int keyboardAdjustOriginalLiftDp;
    private KeyboardAdjustPolicy.Horizontal keyboardAdjustOriginalHorizontal = KeyboardAdjustPolicy.Horizontal.DEFAULT;
    /** 一次拖动按下时的三组值：拖动中的每一步都从它们重新算，手指来回拖不会累积误差。 */
    private int keyboardAdjustDragHeight;
    private int keyboardAdjustDragLiftDp;
    private KeyboardAdjustPolicy.Horizontal keyboardAdjustDragHorizontal = KeyboardAdjustPolicy.Horizontal.DEFAULT;
    /** 「键盘调整」打开时键盘外框在屏幕上的上沿：调整期间输入法窗口铺满，应用的避让边界固定在这里（#10025）。转屏后作废，等外框下一次布局重新记下。 */
    private int keyboardAdjustKeyboardTop = KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP;
    /** 记下上沿时的配置：调整期间只有屏幕尺寸、方向或密度变了，记下的屏幕坐标才作废；深浅模式、字号这类变化不动避让边界。 */
    private Configuration keyboardAdjustConfiguration;
    /** 功能面板（新设计的菜单）与它的「AI 回复与润色」分段页。 */
    FunctionPanelView functionPanel;
    /** 隐私会话重启时保留已打开的功能菜单。 */
    private boolean preserveMoreToolsOnStop;
    /** 无痕输入切换后等键盘窗口收起再重建引擎，避免当前键盘整窗闪烁。 */
    private boolean privacySessionRestartPending;
    boolean aiAssistChooserOpen;
    /** 功能菜单的「设备信息」子面板（#10045）开着。 */
    boolean deviceInfoOpen;
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
    /** 剪贴板面板一行排几条，来自本地设置 `platform.android.clipboard_columns`。 */
    int clipboardColumns = 1;
    /** 本地设置「撤销时长」，删除时读取；改了设置只影响之后的删除。 */
    int clipboardUndoSeconds = ClipboardUndoPolicy.DEFAULT_SECONDS;
    /** 本地设置「剪贴板删除数据验证」；访客模式下即使关闭也要求认证。 */
    boolean clipboardDeleteAuthenticationEnabled;
    private ImeInlineAutofill inlineAutofill;
    /** 本地设置 `platform.android.clipboard_suggestion` 的值；工具栏是否真的显示最近复制，还要看剪贴板历史开没开，一律问 {@link #recentClipEnabled()}。 */
    boolean clipboardSuggestionEnabled = true;
    /** 本地设置 `platform.android.clipboard_suggestion_action`：最近复制那一行是否带分词或分类快捷键（#10079）。 */
    boolean clipboardSuggestionActionEnabled = true;
    /** 本地设置 `platform.android.clipboard_suggestion_action_kind`：快捷键的功能，取值见 {@link RecentClipboardSuggestion#quickAction}。 */
    String clipboardSuggestionActionKind = RecentClipboardSuggestion.ACTION_CATEGORY;
    /** 工具栏上的「最近复制」（#5692）。 */
    final RecentClipboardSuggestion recentClip = new RecentClipboardSuggestion();
    /** 占工具栏那一行位置的「最近复制」：粘贴按钮和关闭按钮；有内容可显示时替换工具栏。 */
    LinearLayout recentClipRow;
    Button recentClipButton;
    private final Runnable recentClipExpiry = this::render;
    /** 发送「最近复制」图片（#6781）时读剪贴板图片、写缓存副本和清理副本都在这条线程上。 */
    private final ExecutorService clipboardImageWorker = Executors.newSingleThreadExecutor();
    /** 换输入框或收起输入时加一：点了发送之后输入框变了，读完的图片就不再提交给新的输入框。表情包的贴纸（{@link ImeStickers}）发图时也看它。 */
    long clipboardImageGeneration;
    /** 发出 {@link ClipboardImagePolicy#RETENTION_MS} 之后删掉缓存里的副本。 */
    private final Runnable clipboardImageCleanup = this::clearClipboardImages;
    boolean toolbarFloating;
    /** 本地设置 `platform.android.toolbar_text_edit`：工具栏上显示「文本编辑」按钮。 */
    boolean toolbarTextEdit;
    boolean toolbarUndo;
    boolean toolbarRedo;
    /** 浮动键盘（本地设置 FLOATING_KEYBOARD，{@link FloatingKeyboardPolicy}）与它在可移动范围里的位置（千分比）。 */
    boolean floatingKeyboard;
    private int floatingX = FloatingKeyboardPolicy.DEFAULT_X_FRACTION;
    private int floatingY = FloatingKeyboardPolicy.DEFAULT_Y_FRACTION;
    /** 浮动面板顶部的拖动条，只在浮动时显示。 */
    FloatingKeyboardBar floatingBar;
    /** 上一次窗口是否铺满透明区域；API 28 从首次显示起固定全高，避免预览扩窗时使用旧 insets。 */
    private boolean fullWindowLayoutApplied;
    private boolean overlayLayoutApplied;
    /** 全文预览期间停靠键区沿用打开前的像素高度。 */
    boolean clipboardPreviewExpanded;
    int clipboardPreviewKeyboardHeight;
    private int clipboardPreviewKeyboardTop;
    boolean textTrackpadPreviewExpanded;
    int editorPreviewKeyboardHeight;
    /** 上一次套上的浮动布局是不是跟随光标的候选栏：它和浮动键盘的外框宽度、拖动条不同，在两者之间切换也要重新套。 */
    private boolean caretLayoutApplied;
    /** 第一次浮动前窗口与系统 inputArea 原来的高度，停靠时原样恢复；{@link Integer#MIN_VALUE} 表示还没记下。 */
    private int dockedWindowHeight = Integer.MIN_VALUE;
    private int dockedInputAreaHeight = Integer.MIN_VALUE;
    private boolean floatingDragging;
    private float floatingDragStartX;
    private float floatingDragStartY;
    private long floatingPositionSaveGeneration;
    /** 窗口的系统栏内边距（像素），浮动时把面板限制在状态栏与导航栏之间。 */
    private final android.graphics.Rect systemBarInsets = new android.graphics.Rect();
    private final int[] floatingLocation = new int[2];
    /** 「物理键盘时候选栏跟随光标」（本地设置 HARDWARE_CARET_CANDIDATES，{@link CaretCandidatePolicy}）。 */
    boolean caretCandidates;
    /** 向哪个 InputConnection 要过光标位置（`requestCursorUpdates`）；换了输入框或不再需要时重新要或撤回。 */
    private InputConnection caretAnchorConnection;
    /** 这个输入框答应报告光标位置（`requestCursorUpdates` 返回真）；拒绝的输入框候选栏照旧停在底部。 */
    private boolean caretAnchorsReported;
    /** 最近一次收到的光标位置（屏幕坐标，像素）：有组字时左边取组字第一个字，上下取整段组字和插入点所占的行；没有组字时取插入点。 */
    private boolean caretAnchorKnown;
    private float caretAnchorLeft;
    private float caretAnchorTop;
    private float caretAnchorBottom;
    /** 刚要过光标位置，还在等第一次报告（{@link CaretCandidatePolicy#ANCHOR_WAIT_MS}）；等的期间要显示的候选栏先不显示，不退回底部。 */
    private boolean caretAnchorPending;
    /** 等第一次报告的期限到了：还没收到就不再等，下一次 render 让要显示的候选栏退回底部。 */
    private final Runnable caretAnchorWaitExpired = () -> {
        caretAnchorPending = false;
        if (keyboardRoot != null && !caretAnchorKnown) render();
    };
    /** 这一次因为显示候选栏时还不知道光标在哪儿而退回底部（{@link CaretCandidatePolicy#docked}）。 */
    private boolean caretDocked;
    /** 上一次 render 时候选栏（组字、候选或提示）是否该显示，供光标位置回调判断此刻是否空闲。 */
    private boolean caretStripShown;
    /** 输入法窗口是跟随光标的候选栏空闲时自己收起的（{@link CaretCandidatePolicy#hidesWindow}）：从发出收起请求起，到窗口再显示出来或离开输入框为止。候选栏又有内容时只叫回这样收起的窗口。 */
    private boolean caretWindowHidden;
    /** 输入法窗口此刻显示着（`onWindowShown` 到 `onWindowHidden` 之间）。偏好轮询只在这段时间里跑。 */
    private boolean imeWindowVisible;
    /** 发出过空闲收起请求（{@link CaretCandidatePolicy#ownHide}），离开输入框时清掉。窗口在落地前可能又被应用叫出来（`onWindowShown`），系统也可能把一次收起分几次落地，所以与 {@link #caretWindowHidden} 分开记、落地时不清：请求后 {@link CaretCandidatePolicy#HIDE_LANDING_MS} 之内落地的收起都认作自己的，不当作用户收起键盘。 */
    private boolean caretHidePending;
    /** 上一次因为空闲收起窗口的时刻（`uptimeMillis`），限制多久才能再收一次（{@link CaretCandidatePolicy#WINDOW_REHIDE_MS}）。 */
    private long caretWindowHiddenAt = Long.MIN_VALUE;
    private final Runnable caretWindowSync = this::synchronizeCaretWindow;
    private final int[] caretRootLocation = new int[2];
    /** 功能面板上直接切换的三项：模糊音（共享偏好）、单手模式（off / left / right）与隐私模式（本地设置）。 */
    boolean fuzzyPinyinEnabled;
    String oneHandedMode = "off";
    /** 本地设置的分离主开关；实际画不画还要看窗口大小、方向和布局，见 {@link #splitKeyboardDrawn}。 */
    boolean splitKeyboardEnabled;
    int splitKeyboardThresholdDp;
    int splitKeyboardGapPercent;
    boolean splitKeyboardPortrait;
    /** 本地设置「手机横屏分离式键盘」（#10440）；实际画不画还要看方向、窗口和分离后的键宽，见 {@link #splitKeyboardDrawn}。 */
    boolean splitKeyboardPhoneLandscape;
    /** 键盘列上一次布局时扣掉左右内边距（系统栏）后给键行的宽度（像素），以及那次布局时的 `screenWidthDp`；配置变了而还没重新布局时不用它。 */
    private int keyboardContentWidthPx;
    private int keyboardContentScreenWidthDp = -1;
    /** 共享偏好 `touch_number_row`；实际画不画还要看键面和布局，见 {@link #numberRowDrawn}。 */
    boolean numberRowEnabled;
    private final AutomaticNumberRow automaticNumberRow = new AutomaticNumberRow();
    boolean incognitoEnabled;
    /** 「防止截屏」（#10098）：开着时输入法窗口和它自己开的弹窗都带 FLAG_SECURE，见 {@link ScreenCaptureProtection}。 */
    boolean screenCaptureProtected;
    boolean panelPreferenceSaving;
    Button microsoftFinalKey;
    final java.util.List<ShuangpinHintButton> shuangpinKeyButtons =
        new java.util.ArrayList<>(27);
    final java.util.List<String> shuangpinKeyInputs = new java.util.ArrayList<>(27);
    private String shuangpinHintsProfile = "";
    private java.util.Map<String, String> shuangpinHints = java.util.Map.of();
    /** 本包所属的版本：键盘只列出本版本提供的方案入口，偏好里的方案本版本没有时回退到本版本的默认方案。 */
    final AppEdition edition = AppEdition.current();
    /** 空闲时候选栏左侧显示的产品名，取自本版本的应用名（full 是「水杉输入法」，五笔版是「水杉五笔」）。 */
    private String productName = "";
    KeyboardScheme selectedScheme = KeyboardScheme.fallback(edition);
    /** 最近一次用的中文入口（不是其他语言键盘的那个），中英键轮换回到中文时切回它（{@link LanguageKeyCyclePolicy#chineseReturn}）。 */
    private KeyboardScheme lastChineseScheme;
    /** 经功能面板「手写」磁贴进入手写前的入口和中英状态，再点一次磁贴回到它们（#9703）；经其他途径换方案时清掉，那时离开手写按 {@link KeyboardScheme#handwritingReturn} 取启用列表里第一个非手写入口。 */
    private KeyboardScheme schemeBeforeHandwriting;
    private boolean englishBeforeHandwriting;
    private java.util.List<KeyboardScheme> enabledSchemes =
        KeyboardScheme.enabledFromPreferenceIds(null, edition);
    // 「输入方式」面板列出的方案：已启用且词典已装好的方案，双拼只留用户设置的那一种（见 `schemeConfiguration` 和 `KeyboardScheme.pickerSchemes`）。`enabledSchemes` 仍是存下的列表，面板里的一次保存不会丢掉用户在词典到达之前打开的方案。
    java.util.List<KeyboardScheme> visibleSchemes = enabledSchemes;
    // The runtime options' `language_dictionaries` directory, read with them in onStartInput; empty when the configuration names none.
    private String languageDictionaries = "";
    // 本机已具备的方案资源包（日文词典、语言词库），在 onStartInput 里和运行时选项一起重读：设置页下载完后，下一次进入输入框切换器就出现新语言，与 host-api 获得焦点时刷新资源包的时机一致。
    private java.util.Set<String> resourcePacks = java.util.Set.of();
    // 算出 resourcePacks 和 candidatePronunciationPackInstalled 时本机已装好的资源包 id。同一输入框重启连接时只有它没变才留着会话（keepsSessionAcrossRestart），变了就照常重建，让上面这两样和引擎会话一起跟上新装或删掉的资源包。
    private java.util.Set<String> installedResourcePackIds = java.util.Set.of();
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
    /** 拼音九键的 1 键：组字时显示「分词」，否则「@#」（点按把常用标点放进候选条），render 时按组字状态更新。 */
    Button nineKeySymbolKey;
    /** 笔画网格的通配键：只在组字中可用，render 时按组字状态更新。 */
    Button strokeWildcardKey;
    String actionRowSignature = "";
    boolean brandPillVisible;
    JapaneseFlickPreview japaneseFlickPreview;
    /** 删除键上滑时弹出的「快速删除」框，见 {@link BackspaceSwipePolicy}。 */
    QuickDeleteOverlay quickDeleteOverlay;
    /** 文本编辑面板（方向键、选择、全选、复制、剪切、粘贴），见 {@link ImeTextEditPanel}。 */
    LinearLayout textEditPanel;
    LinearLayout shortcutToolbar;
    View candidateHeightSpacer;
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
    /** 日语 9 键左列的中英键，键面写这一下要切到的语言（{@link #japaneseLanguageKeyLabel}）。 */
    Button japaneseLanguageKey;
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
    private boolean temporaryEnglishShift;
    /** 方案异步保存完成前暂不绘制中间的中文视图。 */
    private boolean renderingSuppressed;
    private boolean hardwareLanguageShift = true;
    private boolean hardwareLanguageCtrlAltSpace = true;
    private boolean hardwareCharacterSet = true;
    private boolean hardwareFullWidth = true;
    private boolean numberRowSelection = true;
    /** 外接键盘的候选条模式（{@link HardwareKeyboardModePolicy}）：实体键盘打字时打开，点候选条上的展开键或键盘拔掉时关闭。 */
    boolean hardwareKeyboardMode;
    /** 上一次配置是否报告接着实体键盘，用来认出「拔掉 / 合上」这一下。 */
    private boolean hardwareKeyboardAttached;
    /** 这块实体键盘接着期间用户点过展开键：系统再问要不要显示（挪光标、换输入框）时不再收回候选条（{@link HardwareKeyboardModePolicy#afterSystemDecision}）。 */
    private boolean hardwareKeyboardUserExpanded;
    /** Resolved 以词定字 binding: `disabled`, `brackets` or `minus_equal`. */
    private String wordCharacterBinding = WordCharacterPolicy.DISABLED;
    /** Which hardware keys page the candidate list, from the shared `navigation` preferences. */
    private CandidateNavigationPolicy.Bindings candidateNavigation =
        CandidateNavigationPolicy.Bindings.defaults();
    private boolean hardwareLanguageCtrl;
    private long modifierTapStartedAt = -1;
    private int modifierTapKeyCode = -1;
    private boolean modifierTapInvalid;
    /** Ctrl+Enter 上屏译文和义项页吃掉的那个键：它按住时的自动重复和随后的抬起也吃掉，免得上屏之后重复的回车或数字落到已经空闲的输入框里（聊天应用会把消息发出去），没有时为 -1。 */
    private int translationConsumedKeyCode = -1;
    private String defaultImeMode = "chinese";
    private String imeModeScope = "app";
    private boolean liveInputModePreferencesLoaded;
    private String currentEditorPackage = "";
    private InputModeStore inputModeStore;
    /** Mirrors the runtime's own character width; the toolbar card and the chord move it. */
    boolean fullWidthInput;
    /** The shared preference this session started from, so only a change to it overrides the toggle. */
    private boolean fullWidthPreference;
    /** Mirrors the runtime's Chinese/English punctuation state; the card and the chord move it. */
    boolean chinesePunctuation = true;
    private boolean chinesePunctuationPreference = true;
    /** 共享偏好 `paired_punctuation`（设置里的「自动补全成对标点」）：输入前括号、前引号时补上后半个，光标停在中间。 */
    boolean pairedPunctuation = true;
    /** 补上的后半个还在光标右边的那些对，见 {@link PairedPunctuationPolicy.Stack}。 */
    private final PairedPunctuationPolicy.Stack pairedPunctuationStack = new PairedPunctuationPolicy.Stack();
    boolean traditionalChineseOutput;
    int editorInputType;
    private long currentDocumentIdentifier;
    private long nextDocumentIdentifier = 1;
    private final KeyboardInputContext inputContext = new KeyboardInputContext();
    KeyboardLayout.Layer keyboardLayer = KeyboardLayout.Layer.LETTERS;

    boolean directNumberPad() {
        return EditorPolicy.numberInput(editorInputType);
    }
    boolean allowLearning;
    private String preferencesNotice = "";
    /** 上次提示「五笔码表要下载」的时刻（{@link android.os.SystemClock#elapsedRealtime}），见 {@link #announceWubiTableMissing}；还没提示过时为 0。 */
    private long wubiTableNoticeAt;
    /** 最近一次实时偏好里的 `scheme`、`last_chinese_scheme` 和方案列表 `touch_keyboard_schemes.enabled`，在 alignEngineSchemeWithSelection 改写之前记下，给 {@link #announceWubiTableMissing} 用；还没读到实时偏好时 scheme 为 null。 */
    private String wubiNoticeScheme;
    private String wubiNoticeLastChinese;
    private java.util.List<String> wubiNoticeEnabledIds;
    /** 同一个键盘进程里两次「五笔码表要下载」提示至少隔这么久：提示在诊断行里只停几秒、按一下键就消失，只说一次容易错过，每次弹出键盘都说又太吵。 */
    private static final long WUBI_TABLE_NOTICE_INTERVAL_MILLIS = 60 * 60 * 1000L;
    /** 上次以 Toast 说过的空闲提示，同一条不重复弹。 */
    private String announcedIdleNotice = "";

    /** 空闲时的临时提示不占状态行；其中的失败（失败、未能、无法）以 Toast 告诉用户。进度、成功和「稍后重试」这类会自己恢复的提示不弹，免得每次弹出键盘都冒一条。 */
    private void announceIdleNotice(String notice) {
        String text = notice.startsWith(" · ") ? notice.substring(3) : notice;
        if (text.equals(announcedIdleNotice)) return;
        announcedIdleNotice = text;
        if (text.contains("失败") || text.contains("未能") || text.contains("无法")) {
            notice(text);
        }
    }
    /**
     * 键盘里的一句提示（失败、尚未就绪、操作结果）。键盘在屏幕上时显示在诊断行，几秒后或下一次按键时消失，不改变键盘高度；键盘不在屏幕上时才用 Toast。安卓 13 起没有通知权限的应用从后台发的 Toast 会被系统吞掉（logcat: Suppressing toast from package app.msime.android by user request），输入法服务就算后台，而应用只在「关于」页检查更新时才请求通知权限，原先这些提示多数用户根本看不到。
     */
    void notice(String text) {
        if (text == null || text.isEmpty()) return;
        if (diagnosticView != null && isInputViewShown()) {
            imeDebugOverlay.showMessage(text);
            // 投递而不是直接重画：announceIdleNotice 在 render() 里面调用到这里。
            main.post(this::render);
            return;
        }
        Toast.makeText(this, text, Toast.LENGTH_SHORT).show();
    }

    /** 记下实时偏好里 {@link #announceWubiTableMissing} 要看的三项，再判断一次。有会话时实时偏好经 applyPreferencesSnapshot 到达，没有会话的输入框经 applyEditorPreferences，两处都调用。 */
    private void rememberWubiNoticePreferences(JSONObject preferences) {
        wubiNoticeScheme = InputViewValuePolicy.textOr(preferences, "scheme", "");
        wubiNoticeLastChinese = InputViewValuePolicy.textOr(preferences, "last_chinese_scheme", "");
        wubiNoticeEnabledIds = SchemePreferences.enabledIds(preferences);
        announceWubiTableMissing();
    }

    /**
     * 偏好里在用五笔、本机却没有五笔码表（{@link KeyboardScheme#wubiTableWanted}）时，在键盘上提示一句去设置应用下载。
     *
     * <p>full 和拼音版的瘦包不带五笔码表，安装包换了词库锁时已装的 wubi 资源包也随之过期。这时键盘不列五笔、host-api 回退到别的方案，而输入法进程从不联网，下载只能由设置应用发起（{@code HomeActivity} 回到前台时会问）。不提示的话，在那里点了「以后再说」、之后只用键盘的人只会看到五笔不见了。偏好取最近一次读到的实时偏好，码表在不在取 onStartInput 刚算的 {@link #resourcePacks}：键盘进程常常比设置应用活得久，码表消失（设置应用启动时换了代次）之后偏好并没有变，所以读到实时偏好时、每次开始输入时和键盘窗口显示时都判断一次。键盘窗口在屏幕上才提示，同一个进程里至少隔 {@link #WUBI_TABLE_NOTICE_INTERVAL_MILLIS}。
     */
    private void announceWubiTableMissing() {
        if (wubiNoticeScheme == null || !imeWindowVisible) return;
        if (!KeyboardScheme.wubiTableWanted(wubiNoticeScheme, wubiNoticeLastChinese, wubiNoticeEnabledIds,
                resourcePacks, edition)) return;
        long now = android.os.SystemClock.elapsedRealtime();
        if (wubiTableNoticeAt != 0 && now - wubiTableNoticeAt < WUBI_TABLE_NOTICE_INTERVAL_MILLIS) return;
        wubiTableNoticeAt = now;
        // 投递而不是直接显示：调用方随后才 render()，onStartInput 的 render 会清掉提前显示的诊断行。
        main.post(() -> notice("五笔要先下载码表：打开水杉输入法即可下载"));
    }

    long beginClipboardPhraseNotice() { return operationNotice.begin(); }

    void showClipboardPhraseNotice(long request, String text) { operationNotice.show(request, text); }

    /** 复制的代次：每次「复制全部」加一，最后一次说了算；服务销毁时也加一。 */
    private long deviceInfoCopyGeneration;
    /**
     * 插入的代次：只在服务销毁时加一。插入不互相取消：子面板开着就是为了连着插几项，前一项还在读时点下一项，两项都要插进去；工作线程串行，结果仍按点按顺序回来。
     */
    private long deviceInfoInsertGeneration;

    /** 只在用户点击时读取白名单信息；后台收集不阻塞按键，也不触碰组字会话。{@code current} 在主线程上判断结果是否还该用。 */
    private void collectDeviceInfo(java.util.function.BooleanSupplier current,
                                   java.util.function.Consumer<java.util.List<DeviceInfoReport.Item>> done,
                                   Runnable unavailable) {
        // 屏幕参数要从已附着的视图取 Display，在主线程读好再交给工作线程。
        DeviceInfoReport.ScreenFacts screen = DeviceInfoCollector.screenFacts(moreButton);
        try {
            preferencesWorker.execute(() -> {
                var items = DeviceInfoCollector.keyboard(getApplicationContext(), screen);
                main.post(() -> {
                    if (!current.getAsBoolean() || preferencesWorker.isShutdown()) return;
                    done.accept(items);
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException rejected) {
            unavailable.run();
        }
    }

    /** 子面板里的「复制全部」：整段按所选语言放进剪贴板。 */
    void copyDeviceInfo(DeviceInfoReport.Language language) {
        // 复制完成后直接回到键盘，提示会在键盘内部显示，避免用户还要手动关闭工具面板。
        closeMoreTools();
        render();
        long generation = ++deviceInfoCopyGeneration;
        collectDeviceInfo(() -> generation == deviceInfoCopyGeneration, items -> {
            try {
                ClipboardManager clipboard = getSystemService(ClipboardManager.class);
                if (clipboard == null) {
                    notice("无法复制设备信息");
                    return;
                }
                clipboard.setPrimaryClip(ClipData.newPlainText("设备信息",
                    DeviceInfoCollector.text(this, items, language)));
                // 剪贴板监听会把刚复制的内容放进键盘顶部预览；预览可用时不要再用诊断行盖住它。
                offerRecentClip(true);
                if (recentClipRow == null || !recentClipRow.isShown())
                    notice("已复制设备信息");
            } catch (RuntimeException unavailable) {
                notice("无法复制设备信息");
            }
        }, () -> notice("无法复制设备信息"));
    }

    /** 插入设备信息要有输入框，密码框里不插。 */
    boolean deviceInfoInsertable() {
        return connection != null && !EditorPolicy.password(editorInputType);
    }

    /**
     * 子面板里的单项或「全部」（{@code field} 为 null）：读完后直接写进当前输入框，不经过剪贴板（#10045）。子面板保持打开，方便接着插下一项；读的过程中换了输入框就不写。
     *
     * <p>插入格不按输入框状态置灰（子面板开着时输入框会变），这里是唯一的判断处，插不了就说明原因，不悄悄忽略这次点按。
     */
    void insertDeviceInfo(DeviceInfoReport.Field field, DeviceInfoReport.Language language) {
        if (connection == null) {
            notice("没有可插入的输入框");
            return;
        }
        if (!deviceInfoInsertable()) {
            notice("密码框里不插入设备信息");
            return;
        }
        InputConnection target = connection;
        long generation = deviceInfoInsertGeneration;
        collectDeviceInfo(() -> generation == deviceInfoInsertGeneration, items -> {
            if (connection != target || !deviceInfoInsertable()) return;
            String text = field == null ? DeviceInfoCollector.text(this, items, language)
                : DeviceInfoReport.line(DeviceInfoReport.item(items, field), language);
            if (session != 0 && hasEngineComposition()) command(2);
            if (connection != target) return;
            // 只看光标前一个字符决定要不要先换行，读到的内容不保存、不进报告。
            boolean multiLine = (editorInputType & android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE) != 0;
            String inserted = DeviceInfoReport.afterText(
                InputConnectionGuard.textBeforeCursor(target, 1), multiLine, text);
            if (!commitText(inserted, TypingSource.LOCAL)) notice("无法插入设备信息");
            render();
        }, () -> notice("无法插入设备信息"));
    }

    String preferencesDirectory = "";
    private long appearanceLoadGeneration;
    private String runtimeOptionsForSnapshot = "";
    /** 本输入框的运行时选项在强制关闭学习之前的原样，隐私模式切换时据此重建会话。 */
    private String runtimeOptionsBase = "";
    JSONObject preferencesSnapshot;
    /** 上次整套应用成功的 loadPreferences 原始响应，和应用前取的 {@link #preferencesApplyInputs}；每次 {@link #applyPreferencesSnapshot} 开头清空，只由每秒重读和建会话的首份快照记下。 */
    private String appliedPreferencesResponse;
    private java.util.List<Object> appliedPreferencesInputs;
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
        /** 底行（功能行、123 / #+= 层自带的底行）：和一行键同高、带一份行距，分到四行均分后底行的那一份调整（{@link KeyboardGeometry#bottomRowAdjustment}）。 */
        final boolean bottomRow;
        /** 整块里还包着底行（日语九键、注音九键没有单独的底行，四行都在块里）：`baseHeight` 已经是四行，块拿整份调整。其余的键行和键块分摊底行以外的那部分（{@link KeyboardGeometry#keyRowsAdjustment}）。 */
        final boolean withBottomRow;
        /** 九宫格主体；上方补充数字行仍按原字母行分摊，避免切层改变总高。 */
        final boolean nineKeyGrid;

        KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex,
                           boolean includesRowSpacing) {
            this(baseHeight, rowCount, rowIndex, includesRowSpacing ? 1 : 0);
        }

        KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex, int rowSpacings) {
            this(baseHeight, rowCount, rowIndex, rowSpacings, false, false, false);
        }

        private KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex, int rowSpacings,
                                   boolean bottomRow, boolean withBottomRow, boolean nineKeyGrid) {
            this.baseHeight = baseHeight;
            this.rowCount = rowCount;
            this.rowIndex = rowIndex;
            this.includesRowSpacing = rowSpacings > 0;
            this.rowSpacings = rowSpacings;
            this.bottomRow = bottomRow;
            this.withBottomRow = withBottomRow;
            this.nineKeyGrid = nineKeyGrid;
        }

        /** 底行这一行。 */
        static KeyboardHeightRole bottomRow() {
            return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP, 1, 0, 1, true, false, false);
        }

        /** 九宫格三行外框，单独标记以免上方的补充符号行跟着加高。 */
        static KeyboardHeightRole nineKeyGrid() {
            return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3,
                1, 0, 3, false, false, true);
        }

        /** 三行键加底行的整块：四行键高、四份行距，拿整份高度调整，总高和三行键加一条底行的布局相同。 */
        static KeyboardHeightRole threeRowsWithBottomRow() {
            return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT,
                1, 0, KeyboardHeightPolicy.KEYBOARD_ROW_COUNT, false, true, false);
        }
    }
    private ScrollView voiceResultScroll;
    LinearLayout voiceResultPanel;
    private VoiceResultStore voiceResultStore;
    private VoiceResultStore.Entry voiceResultEntry;
    /** 不为 null 时语音面板画的是「语音输入还不能用」的引导（{@link #showVoiceSetup}），内容是原因。 */
    private String voiceSetupReason;
    private EditorContextSnapshot voiceTarget;
    private long voiceActivityGeneration;
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
    /** The directory whose committed text the native store may still hold unwritten. Main thread only. */
    private final PendingTypingStatistics pendingTypingStatistics = new PendingTypingStatistics();
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
    /** 当前输入框的一次快速删除备份，超时或输入上下文变化立即清除。 */
    private QuickDeleteUndo quickDeleteUndo;
    private final Runnable expireQuickDeleteUndo = this::clearQuickDeleteUndo;

    private void clearQuickDeleteUndo() {
        quickDeleteUndo = null;
        main.removeCallbacks(expireQuickDeleteUndo);
    }

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
    private final ContentObserver inputMethodWatcher = new ContentObserver(main) {
        @Override public void onChange(boolean selfChange) {
            automaticNumberRow.inputMethodChanged();
            // 回调异步到达：切回本输入法后仍在密码/终端中时，按当前编辑器重新开启。
            String selected;
            try { selected = Settings.Secure.getString(getContentResolver(), Settings.Secure.DEFAULT_INPUT_METHOD); }
            catch (SecurityException unavailable) { selected = null; }
            EditorInfo info = getCurrentInputEditorInfo();
            if (selected != null && selected.startsWith(getPackageName() + "/") && isInputViewShown() && info != null)
                automaticNumberRow.enterEditor(info.inputType, info.packageName);
            if (keyboardRoot != null) render();
        }
    };
    Runnable backspaceRepeatTask;
    Button backspaceRepeatButton;
    boolean backspaceRepeated;
    boolean backspaceClearedComposition;
    private volatile long personalDictionarySyncGeneration;
    private Runnable personalDictionarySyncTask;
    /** 离开输入框后的词库快照处理与空闲同步一样延后、可取消：它每次都要把整份用户词库扫一遍算版本，原先失焦就直接排进工作线程，紧接着的下一个输入框建会话要排在它后面。 */
    private volatile long dictionarySnapshotGeneration;
    private Runnable dictionarySnapshotTask;
    /** 被取消的快照处理欠了多久；欠满一分钟后下一次失焦不再延后。主线程上用。 */
    private final DictionarySnapshotBacklog dictionarySnapshotBacklog = new DictionarySnapshotBacklog();
    /** 用户每复制一次就记进本机剪贴板历史；只在本服务（当前默认输入法）存活期间监听，关掉剪贴板历史或命中隐私规则时什么也不记。 */
    private final ClipboardManager.OnPrimaryClipChangedListener clipboardWatcher = () -> {
        captureClipboard(ClipboardCapturePolicy.Trigger.COPIED, false);
        offerRecentClip(true);
    };
    /** 只让当前输入框的启动任务占用工作线程；旧任务可能还在收尾，但不能再排队读偏好或同步词库。 */
    private volatile long engineStartGeneration;
    /** 一次输入会话最多自动重建一次；持续失败时保留直接输入，避免在坏配置上循环重启。 */
    private int inputFailureRecoveryAttempts;
    private Runnable inputViewRefreshTask;
    final ExecutorService preferencesWorker = Executors.newSingleThreadExecutor();
    private final ExecutorService typingStatisticsWorker = new ThreadPoolExecutor(
        1, 1, 0, TimeUnit.MILLISECONDS, new ArrayBlockingQueue<>(32),
        new ThreadPoolExecutor.AbortPolicy());
    /** 表情目录和符号面板里的颜文字、符号目录都在这条线程上读。 */
    final ExecutorService emojiWorker = Executors.newSingleThreadExecutor();
    // 只在 `emojiWorker` 线程上使用（表情目录和符号面板的符号目录都在那条线程上读）；`hasGlyph` 会走系统字体回退链，能判断当前设备能否画出某个表情或符号。
    final Paint emojiGlyphPaint = new Paint();
    private java.util.List<EmojiCatalogModel.Item> emojiCombinationItems;
    final ExecutorService cloudClipboardWorker = Executors.newSingleThreadExecutor();
    /** 键盘里常用语的读取和「添加到常用语」都在这条线程上排队（#10086）：`preferencesWorker` 在每个新输入框里先做建会话前的偏好读取和个人词库同步，常用语排在它后面，会话在建时点开的常用语面板要一直显示「正在读取」到会话快建好。读写放在同一条线程上，刚添加的那条在随后打开的面板里一定读得到。 */
    final ExecutorService commonPhrasesWorker = Executors.newSingleThreadExecutor();
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
    private final CandidateSkinResolver candidateSkinResolver = new CandidateSkinResolver();
    private final PreferencesReloader preferencesReloader = new PreferencesReloader(
        (task, delay) -> main.postDelayed(task, delay), preferencesWorker, candidateSkinResolver::read);

    /** 候选区标题行（品牌标记、预编辑、状态、页码、展开）的固定高度。 */
    static final int CANDIDATE_HEADER_HEIGHT_DP = 34;

    private record SchemeConfiguration(
        java.util.List<KeyboardScheme> enabled, java.util.List<KeyboardScheme> visible,
        KeyboardScheme selected) {}

    record SkinChoice(String id, String title, KeyboardSkin skin, JSONObject design, String libraryId) {}

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
        // 切换器只列已启用且词典已安装的方案；设置页另有「添加语言」入口。把所有已安装方案都放进来会让用户移除的外语仍被中英键轮换到。
        java.util.List<KeyboardScheme> visible =
            KeyboardScheme.installedOf(enabled, languageDictionaries, resourcePacks, edition);
        String selected = shared == null || shared.isNull("selected")
            ? null : shared.optString("selected", null);
        KeyboardScheme resolved = KeyboardScheme.resolveEnabledSelection(engineScheme, selected, visible, edition);
        // 选中项按全部已安装方案解析，面板里的双拼再只留用户设置的那一种（#6450）。
        String profile = preferences == null ? null
            : InputViewValuePolicy.textOr(preferences, "shuangpin_profile", null);
        return new SchemeConfiguration(enabled,
            KeyboardScheme.pickerSchemes(visible, resolved, profile), resolved);
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
        applyEditorPreferences(preferences, ClipboardHistoryRetentionPolicy.Source.LIVE);
    }

    /**
     * @param source 这份偏好从哪里来，由调用处明说。只有实时读到的（`LIVE`）才重算皮肤和候选条外观、才改剪贴板历史开关。runtime-options.json 里的偏好是宿主早先准备时写下的副本（`RUNTIME_OPTIONS_COPY`）：主题字段可能已经过时（例如仍是默认的薄荷设计），用它重算会把按上次皮肤画好的第一帧刷成旧配色，一两秒后真正的偏好到了又换回来；它的 `clipboard_history` 永远是出厂默认的关，按它清空就是每换一个输入框历史都被清掉的原因（#5602）。皮肤因此只认启动缓存和真正读到的偏好。
     */
    private void applyEditorPreferences(JSONObject preferences,
            ClipboardHistoryRetentionPolicy.Source source) throws JSONException {
        boolean live = source == ClipboardHistoryRetentionPolicy.Source.LIVE;
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
        applyPairedPunctuation(preferences == null || preferences.optBoolean("paired_punctuation", true));
        wordCharacterBinding = wordCharacterBindingFrom(preferences);
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
        // 进框会重读首次准备的旧副本；已有实时模式偏好时保留默认值和记忆范围，避免换框后查错记忆。
        if (live || !liveInputModePreferencesLoaded) {
            defaultImeMode = preferences != null
                && "english".equals(InputViewValuePolicy.textOr(preferences, "default_ime_mode", "chinese"))
                ? "english" : "chinese";
            imeModeScope = preferences != null
                && "global".equals(InputViewValuePolicy.textOr(preferences, "ime_mode_scope", "app"))
                ? "global" : "app";
            if (live) liveInputModePreferencesLoaded = true;
        }
        KeyboardScheme engineScheme = SchemePreferences.storedScheme(preferences, edition);
        // 必须在 alignEngineSchemeWithSelection 改写内存里的 scheme 之前判断：它把缺码表的五笔换成了回退的方案。只认实时偏好，副本里是出厂值。
        if (live && preferences != null) rememberWubiNoticePreferences(preferences);
        SchemeConfiguration schemeConfiguration = schemeConfiguration(preferences, engineScheme);
        alignEngineSchemeWithSelection(preferences, engineScheme, schemeConfiguration);
        enabledSchemes = schemeConfiguration.enabled();
        visibleSchemes = schemeConfiguration.visible();
        selectedScheme = schemeConfiguration.selected();
        // 出厂副本只用于启动，不代表最近使用的中文入口；否则日语中换输入框会把九键记忆覆盖成 26 键。
        if (live) rememberChineseScheme();
        // 只用真正读到的偏好重算皮肤：runtime-options.json 的副本（来源不是 LIVE）和缺主题字段的偏好都保留当前皮肤，也就是 onCreate 按上次换上的皮肤画好的那一份。
        if (live && preferences != null && preferences.has("global_theme")) {
            skin = keyboardSkin(preferences);
            emojiSkin = surfaceSkin(preferences, "emoji_theme");
            handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
            rememberSkinHint(preferences);
        }
        localModes = preferences == null ? new JSONObject()
            : preferences.optJSONObject("local_modes");
        if (localModes == null) localModes = new JSONObject();
        // 候选条的配色和字号与皮肤同理：按副本重算，候选条会先铺一层出厂薄荷底、字号回到出厂的 18/15，实时偏好到了才换回来（#5933）。副本这条路径保留当前外观，也就是上次真正读到的偏好或 onCreate 按皮肤片段算好的那一份。
        if (live) applyCandidateAppearance(preferences);
        // 键距、行距、语音快捷键、数字键顺序和双拼键位提示同理：副本里是出厂值，onStartInput 紧接着就按它重建键行，调过键距的用户每换一个输入框，键盘都先按出厂间距排一帧，实时偏好到了才跳回来。副本这条路径保留当前几何，也就是上次真正读到的偏好或 onCreate 按皮肤片段算好的那一份。键高例外：本地设置里有键高时那份是实时的，照旧跟着刷新；没有时 heightAdjustmentFrom 会退回副本里的出厂值，所以要先判断。
        if (live) applyTouchGeometry(preferences);
        else if (localSettings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT))
            adoptSavedHeightAdjustment(heightAdjustmentFrom(preferences));
        // 工具栏按钮开关与皮肤同理：runtime-options.json 那份出厂默认里剪贴板按钮是关的，拿它画，新打开的应用里工具栏先少一格、其余按钮跟着挪位，一两秒后实时偏好到了才补回来（#5680）。那条路径改用上次真正读到的开关，没有时才退回这份副本。
        JSONObject toolbar = live || rememberedToolbar == null
            ? (preferences == null ? null : preferences.optJSONObject("touch_toolbar"))
            : rememberedToolbar;
        applyToolbarPreferences(preferences, toolbar);
        if (imeTextEditPanel != null) imeTextEditPanel.applyPreferences(preferences, live);
        applyVoicePreferences(preferences);
        applyAiPreferences(preferences);
        applyClipboardPreference(preferences, source);
        applyChineseOutputPreference(preferences);
        applyCandidateGlossPreference(preferences);
        spellingPosition = CandidatePreeditStylePolicy.normalize(
            InputViewValuePolicy.textOr(preferences, "touch_spelling_position", null));
        applyEnglishSuggestionsPreference(preferences);
        applyCandidateTranslationPreference(preferences);
        applyWubiCodeHintPreference(preferences);
        wubiMixedPinyin = preferences != null
            && preferences.optBoolean("wubi_mixed_pinyin", false);
        wubiProfile = KeyboardScheme.normalizedWubiProfile(
            preferences == null ? null : InputViewValuePolicy.textOr(
                preferences, "wubi_profile", KeyboardScheme.WUBI_86));
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
                    response = candidateSkinResolver.read(directory);
                } catch (RuntimeException | LinkageError error) {
                    return;
                }
                main.post(() -> {
                    if (generation != appearanceLoadGeneration || session != 0) return;
                    try {
                        boolean previousClipboard = clipboardHistoryEnabled;
                        preferencesDirectory = directory;
                        applyEditorPreferences(value(response).optJSONObject("preferences"));
                        offerRecentClipOnHistoryEnabled(previousClipboard);
                        render();
                        imeWorldClock.resumePicker(getCurrentInputEditorInfo());
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
            InputViewValuePolicy.textOr(preferences, "last_chinese_scheme",
                InputViewValuePolicy.textOr(preferences, "scheme", edition.defaultScheme())),
            InputViewValuePolicy.textOr(preferences, "shuangpin_profile", "xiaohe"), edition);
        preferences.put("scheme", mapping.scheme());
        preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
        preferences.put("shuangpin_profile", mapping.shuangpinProfile());
        preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
    }

    /** 解开共享层的响应信封，给服务之外的键盘部件用（如九键展开面板重新取完整候选）。 */
    JSONObject nativeValue(String response) throws JSONException {
        return value(response);
    }

    private JSONObject value(String response) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        Object ok = envelope.opt("ok");
        if (!(ok instanceof Boolean)) throw new JSONException("Invalid runtime envelope");
        if (!((Boolean) ok))
            throw new InputFailurePolicy.Failure(InputFailurePolicy.Reason.RUNTIME_REJECTED);
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
            private String pendingComposition;
            public void begin() { target.beginBatchEdit(); }
            public boolean commit(String text) {
                editorCompositionSuppressed = false;
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
                if (text.isEmpty()) {
                    if (editorCompositionSuppressed) return true;
                    boolean cleared = target.setComposingText(text, 1);
                    if (cleared) selectionEcho.compose(0);
                    else selectionEcho.invalidate();
                    return cleared;
                }
                if (editorCompositionSuppressed) return true;
                boolean composed = target.setComposingText(text, 1);
                if (!composed) {
                    selectionEcho.invalidate();
                    return false;
                }
                pendingComposition = text;
                selectionEcho.compose(text.length());
                return true;
            }
            public boolean verify() {
                if (pendingComposition == null) return true;
                // Compose 等编辑器在 endBatchEdit 才应用写入；批量内读回的可能是上一拍快照。
                CharSequence before;
                try { before = target.getTextBeforeCursor(pendingComposition.length(), 0); }
                catch (RuntimeException error) { before = null; }
                int actualLength = CompositionTextPolicy.composingLength(before, pendingComposition);
                if (actualLength < 0 || actualLength == pendingComposition.length()) return true;
                // 截短批次的回声可能先于清理回声到达，且 LengthFilter 可能已移除组字标记。
                selectionEcho.compose(actualLength);
                selectionEcho.expect();
                selectionEcho.finish();
                selectionEcho.expect();
                target.beginBatchEdit();
                try {
                    // 空 setComposingText 无法删除已失去标记的前缀，按实际长度清理。
                    if (!target.finishComposingText()
                            || (actualLength > 0 && !target.deleteSurroundingText(actualLength, 0))) {
                        selectionEcho.invalidate();
                        return false;
                    }
                    selectionEcho.deleteBefore(actualLength);
                    editorCompositionSuppressed = true;
                    return true;
                } finally {
                    target.endBatchEdit();
                    selectionEcho.expect();
                }
            }
            public boolean finish() {
                boolean finished = target.finishComposingText();
                editorCompositionSuppressed = false;
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
            view == null || view.isNull("local_mode")
                ? null : InputViewValuePolicy.textOr(view, "local_mode", null));
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
        String earlier = pendingTypingStatistics.record(directory);
        if (earlier != null) submitTypingStatisticsFlush(earlier);
        submitTypingStatistics(request, null, engineStartGeneration);
    }

    /**
     * Ask the native store to write the commits it is still holding in memory.
     *
     * <p>{@code record} no longer rewrites and fsyncs the whole document per commit: the store keeps the counts and writes them at most every half minute, checked on the next commit. So whatever was typed last waits here until the editor loses focus, the keyboard is hidden or the service goes away. It runs on the statistics worker, after every record already queued there.
     */
    private void flushTypingStatistics() {
        String directory = pendingTypingStatistics.drain();
        if (directory != null) submitTypingStatisticsFlush(directory);
    }

    /**
     * Queue one flush for {@code directory}; its result is ignored. A native library older than the flush operation answers with an error, and it wrote every commit already. When the queue is full the counts stay in the store, which writes them on the next commit after half a minute or before any other request for that directory.
     */
    private void submitTypingStatisticsFlush(String directory) {
        final String request;
        try {
            request = new JSONObject().put("directory", directory)
                .put("action", new JSONObject().put("operation", "flush")).toString();
        } catch (JSONException error) {
            return;
        }
        try {
            typingStatisticsWorker.execute(() -> {
                try {
                    NativeClient.typingStatistics(request);
                } catch (RuntimeException | LinkageError ignored) {
                }
            });
        } catch (RuntimeException ignored) {
        }
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
                        long recorded = NumberPolicy.strictLong(value.opt("recorded"), -1);
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
        clearQuickDeleteUndo();
        dropDeferredToolbarTaps();
        editorReads.invalidate();
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
    boolean deleteBeforeCursor(int length) {
        clearQuickDeleteUndo();
        editorReads.invalidate();
        pairedPunctuationStack.clear();
        boolean deleted = InputConnectionGuard.deleteSurroundingText(connection, length, 0);
        if (deleted) {
            selectionEcho.deleteBefore(length);
            selectionEcho.expect();
        } else {
            selectionEcho.invalidate();
        }
        return deleted;
    }

    /** 删除键先删选区；没有前文可删时交给编辑器处理引用等控件，其余按码位删字并追踪回声。 */
    void deleteCodePointBeforeCursor() {
        clearQuickDeleteUndo();
        dropDeferredToolbarTaps();
        editorReads.invalidate();
        pairedPunctuationStack.clear();
        if (deleteSelection()) return;
        CharSequence before = InputConnectionGuard.textBeforeCursor(connection, 2);
        if (before != null && before.length() == 0) {
            // 空框外的艾特与引用不属于周边文字，应用只有收到退格事件才能处理；读取失败不当作空框。
            selectionEcho.invalidate();
            imeLetterRows.invalidateBackspaceScrub();
            sendEditorKeyEvents(KeyEvent.KEYCODE_DEL);
            return;
        }
        if (InputConnectionGuard.deleteSurroundingTextInCodePoints(connection, 1, 0)) {
            imeLetterRows.recordBackspaceScrubDelete(before);
            selectionEcho.deleteCodePointBefore();
            selectionEcho.expect();
        } else {
            imeLetterRows.invalidateBackspaceScrub();
            selectionEcho.invalidate();
            // 部分远程编辑器（例如桌面控制类应用）拒绝 surrounding-text 删除，但仍处理原始退格键。
            // 连接已明确返回失败时才回退，避免普通编辑器重复删除。
            sendEditorKeyEvents(KeyEvent.KEYCODE_DEL);
        }
    }

    /** `deleteSurroundingText` 只删选区以外的字：选中文字后按删除，选区在开头时毫无反应，在中间时删掉的是选区前一个字。有选区就用空串替换它，和系统键盘一致；返回是否有选区，有选区时不再删光标前的字。 */
    private boolean deleteSelection() {
        CharSequence selected = InputConnectionGuard.selectedText(connection);
        if (selected == null || selected.length() == 0) return false;
        imeLetterRows.invalidateBackspaceScrub();
        selectionEcho.invalidate();
        InputConnectionGuard.commitText(connection, "", 1);
        return true;
    }

    /** 与系统辅助方法保持相同的按键标记，但两次发键分别保护，连接断开时松键仍尽力发送。 */
    void sendEditorKeyEvents(int keyCode) {
        editorReads.invalidate();
        InputConnection target = connection;
        if (target == null) return;
        long down = SystemClock.uptimeMillis();
        InputConnectionGuard.sendKeyEvent(target, new KeyEvent(down, down,
            KeyEvent.ACTION_DOWN, keyCode, 0, 0, KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
            KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE));
        InputConnectionGuard.sendKeyEvent(target, new KeyEvent(down, SystemClock.uptimeMillis(),
            KeyEvent.ACTION_UP, keyCode, 0, 0, KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
            KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE));
    }

    /** 编辑器动作可能改文字也可能不改，选区预期先作废。 */
    private boolean performEditorAction(int action) {
        selectionEcho.invalidate();
        editorReads.invalidate();
        return InputConnectionGuard.performEditorAction(connection, action);
    }

    boolean commitText(String text) { return commitText(text, typingSource()); }

    /** 完整符号对是用户明确选择的模板，不受自动补半个的开关影响；分两次写入，让光标停在中间。 */
    boolean commitLiteralText(String text) {
        String closing = PairedPunctuationPolicy.literalPairClosing(text);
        if (closing == null) return commitText(text);
        TypingSource source = typingSource();
        return commitText(text.substring(0, text.length() - closing.length()), source)
            && commitClosingMark(closing, source);
    }

    private void applyPairedPunctuation(boolean enabled) {
        pairedPunctuation = enabled;
        if (!enabled) pairedPunctuationStack.clear();
    }

    /**
     * 补上一对的后半个：写在光标右边，光标留在两半之间。`commitText` 的第二个参数不大于 0 时按新文字的开头算，0 就是停在它前面；写成「整对一起上屏再左移一格」做不到，因为大于 0 的值从末尾减一算起，左移一个字符要的正是 0。
     *
     * <p>个别应用不认这个 0，光标照样落在后半个后面（#6458）。写完马上读一次光标两侧的文字（同步查询编辑器当前内容，不等异步的选区回报），确认光标跑到了后半个后面才发左方向键退回去；照规矩处理的编辑器里光标后面就是后半个，不发任何按键，行为和原来一样。
     */
    boolean commitClosingMark(String closing, TypingSource source) {
        if (connection == null || closing == null || closing.isEmpty()) return false;
        boolean committed;
        try { committed = connection.commitText(closing, 0); }
        catch (RuntimeException error) {
            selectionEcho.invalidate();
            return false;
        }
        if (!committed) {
            selectionEcho.invalidate();
            return false;
        }
        CharSequence after = textAfterCursor(closing.length());
        // 光标后面可能是外层同名后半个，要检查刚写出的后半个有没有跑到光标前面。
        CharSequence before = after == null ? null : textBeforeCursor(closing.length());
        if (PairedPunctuationPolicy.caretPassedClosing(closing, after, before)) {
            selectionEcho.commit(closing.length());
            selectionEcho.expect();
            for (int index = TextPolicy.codePointLength(closing); index > 0; index--)
                sendEditorKeyEvents(KeyEvent.KEYCODE_DPAD_LEFT);
            selectionEcho.caretLeft(closing.length());
        } else {
            selectionEcho.commit(0);
        }
        selectionEcho.expect();
        recordTypingStatistics(closing, source);
        pairedPunctuationStack.push(closing, currentDocumentIdentifier);
        return true;
    }

    /** 光标右边紧跟着的 `length` 个 UTF-16 单元；读不出来时为 null。 */
    private CharSequence textAfterCursor(int length) {
        if (connection == null) return null;
        try { return connection.getTextAfterCursor(length, 0); }
        catch (RuntimeException ignored) { return null; }
    }

    /** 光标左边紧挨着的 `length` 个 UTF-16 单元；读不出来时为 null。 */
    private CharSequence textBeforeCursor(int length) {
        if (connection == null) return null;
        try { return connection.getTextBeforeCursor(length, 0); }
        catch (RuntimeException ignored) { return null; }
    }

    /** 跨过光标右边已经补好的后半个：在一次批量编辑里删掉它再原样上屏，文字不变，光标到了它后面。不算一次输入，不记打字统计。 */
    private void stepOverClosingMark(String closing) {
        InputConnection target = connection;
        if (!InputConnectionGuard.beginBatchEdit(target)) {
            selectionEcho.invalidate();
            return;
        }
        try {
            boolean moved = InputConnectionGuard.deleteSurroundingText(target, 0, closing.length())
                && InputConnectionGuard.commitText(target, closing, 1);
            if (moved) selectionEcho.commit(closing.length());
            else selectionEcho.invalidate();
        } finally {
            InputConnectionGuard.endBatchEdit(target);
        }
        selectionEcho.expect();
    }

    private void cancelInputViewRefresh() {
        if (inputViewRefreshTask != null) main.removeCallbacks(inputViewRefreshTask);
        inputViewRefreshTask = null;
    }

    /** Re-read host settings at the appearance boundary, matching Apple's viewWillAppear path. */
    private void refreshPreferencesOnInputView() {
        if (session == 0 || preferencesDirectory.isEmpty() || hasEngineComposition() || !imeWindowVisible) return;
        preferencesReloader.start(preferencesDirectory, this::reloadPreferences);
    }

    private int currentEditorFieldId;
    private int currentEditorImeOptions;
    private Runnable pendingCompositionMarkerLoss;

    private void cancelPendingCompositionMarkerLoss() {
        if (pendingCompositionMarkerLoss != null) main.removeCallbacks(pendingCompositionMarkerLoss);
        pendingCompositionMarkerLoss = null;
    }

    /** 新按键不能在失去标记的旧区间上继续追加；先按原规则处理尚未被重启接手的回报。 */
    private void finishPendingCompositionMarkerLoss() {
        Runnable pending = pendingCompositionMarkerLoss;
        cancelPendingCompositionMarkerLoss();
        if (pending != null) pending.run();
    }

    /** 重启连接不等于换文档；只有新连接确认还保存自己的拼音组字时才延续原会话。 */
    private boolean retainRestartedComposition(EditorInfo info, InputConnection next) {
        if (info == null || next == null || currentDocumentIdentifier == 0 || session == 0
                || view == null || editorCompositionSuppressed
                || !EditorPolicy.useEngine(info.inputType)
                || info.inputType != editorInputType || info.fieldId != currentEditorFieldId
                || info.imeOptions != currentEditorImeOptions
                || !currentEditorPackage.equals(info.packageName)) return false;
        return retainCompositionAtSelection(next, info.initialSelStart, info.initialSelEnd);
    }

    /** 编辑器可能先移除组字标记、再通知连接重启；只重标记已核对的原区间，不重新写文字。 */
    private boolean retainCompositionAtSelection(InputConnection next, int start, int end) {
        if (next == null || session == 0 || view == null || editorCompositionSuppressed) return false;
        int scheme = InputViewValuePolicy.scheme(view, -1);
        if (scheme != 0 && scheme != 1) return false;
        String marked = composingText(view, InputViewValuePolicy.editingText(view), true);
        if (!selectionEcho.matchesRestartComposition(start, end, marked.length())) return false;
        try {
            CharSequence before = next.getTextBeforeCursor(marked.length(), 0);
            if (!android.text.TextUtils.equals(before, marked)) return false;
            // 重启可能去掉标记但留下文字；先恢复已核对的区间，下一次写入才不会追加第二份组字。
            return next.setComposingRegion(end - marked.length(), end);
        } catch (RuntimeException disconnected) {
            return false;
        }
    }

    @Override public void onStartInput(EditorInfo info, boolean restarting) {
        // restart 保留组字也属于新的焦点边界，旧照片和识别回调不能跨过去。
        if (imeOcr != null) imeOcr.close();
        cancelPendingCompositionMarkerLoss();
        super.onStartInput(info, restarting);
        if (info != null) automaticNumberRow.enterEditor(info.inputType, info.packageName);
        if (inlineAutofill != null) inlineAutofill.startInput();
        cancelInputViewRefresh();
        InputConnection nextConnection = getCurrentInputConnection();
        InputConnection previousConnection = connection;
        // 同一输入框重启连接时文本编辑面板留着（#10141）：它直接作用于编辑器，不依赖引擎会话；换了输入框才关。包名、fieldId、输入类型和 imeOptions 分不出同一个 WebView 或 Compose 视图里的两个输入框，所以还要求新连接的初始选区就是编辑器最近报来的选区；对不上或选区未知时按换输入框处理。
        boolean sameEditorRestart = restarting && nextConnection != null && previousConnection != null
            && sameEditor(info) && imeTextEditPanel.reportedSelection(info.initialSelStart, info.initialSelEnd);
        if (restarting && retainRestartedComposition(info, nextConnection)) {
            imeDebugOverlay.recordEvent(DiagnosticEventKind.IME_START);
            connection = nextConnection;
            if (sameEditorRestart) {
                imeTextEditPanel.editorRestarted(previousConnection, info.initialSelStart, info.initialSelEnd);
            } else {
                imeTextEditPanel.editorChanged(info.initialSelStart, info.initialSelEnd);
            }
            resetCaretAnchor();
            updateAutomaticCapitalization();
            render();
            return;
        }
        // 应用对同一个输入框 restartInput 而引擎里没有组字时（聊天应用发送后清空输入框），留着原会话，只在焦点边界上复位：原来每发一条消息都要销毁会话、在工作线程上重读偏好和皮肤、同步个人词库（每次都重写并 fsync 一个文件），再在主线程上新建会话。
        boolean keepSession = keepsSessionAcrossRestart(info, restarting, nextConnection);
        long startGeneration = ++engineStartGeneration;
        cloudClipboardGeneration++;
        clipboardImageGeneration++;
        boolean deferEngine = !keepSession && info != null && nextConnection != null
            && InputViewRefreshPolicy.defersEngineStartup(EditorPolicy.useEngine(info.inputType),
                info.inputType, isInputViewShown(), hardwareKeyboardAttached);
        // 只有接下来要建引擎会话的输入框才停掉空闲同步：建会话前 scheduleEngineStartup 会先在同一个工作线程上同步一轮。焦点落到不打字的窗口（桌面、设置页）时系统同样会调这里，原先一律停掉，离开输入框 500 ms 后的空闲同步几乎总被取消，导入词库的词条就一直等不到写入。这类窗口报的是 TYPE_NULL，会话推迟到键盘真显示时才建（deferEngine），到那时再停。
        if (info != null && EditorPolicy.useEngine(info.inputType) && !deferEngine) {
            cancelPersonalDictionarySynchronization();
            cancelDictionarySnapshotProcessing();
        }
        boolean newDocument = !restarting || currentDocumentIdentifier == 0;
        if (newDocument) {
            currentDocumentIdentifier = nextDocumentIdentifier++;
        }
        imeBottomRow.resetSpaceCursor();
        if (keepSession) stopKeepingSession(sameEditorRestart);
        else stop(false, sameEditorRestart);
        inputFailureRecoveryAttempts = 0;
        connection = getCurrentInputConnection();
        if (sameEditorRestart) {
            imeTextEditPanel.editorRestarted(previousConnection, info.initialSelStart, info.initialSelEnd);
        } else {
            imeTextEditPanel.editorChanged(info == null ? -1 : info.initialSelStart,
                info == null ? -1 : info.initialSelEnd);
        }
        resetCaretAnchor();
        selectionEcho.reset(info == null ? -1 : info.initialSelStart,
            info == null ? -1 : info.initialSelEnd);
        ensureCandidateTranslationStore();
        editorContextRevision++;
        clearSmartPunctuationSnapshots();
        pairedPunctuationStack.clear();
        bridge = new EditorBridge();
        editorCompositionSuppressed = false;
        if (inputModeStore == null) {
            inputModeStore = InputModeStore.from(
                getSharedPreferences(INPUT_MODE_PREFERENCES, MODE_PRIVATE));
        }
        // 方案列表、语言词库和资源包是会话的偏好和运行时选项算出来的；留着会话时仍是它的，不退回出厂值。
        if (!keepSession) {
            enabledSchemes = KeyboardScheme.enabledFromPreferenceIds(null, edition);
            visibleSchemes = enabledSchemes;
            languageDictionaries = "";
            resourcePacks = java.util.Set.of();
            installedResourcePackIds = java.util.Set.of();
        }
        letterCase.reset();
        clearEnglishSuggestions();
        int nextInputType = info == null ? 0 : info.inputType;
        String nextPackage = info == null || info.packageName == null ? "" : info.packageName;
        // 同一编辑器重启连接时保留用户选择的数字层；换输入框或输入类型时重新按编辑器决定。
        boolean keepKeyboardLayer = !newDocument && editorInputType == nextInputType
            && currentEditorPackage.equals(nextPackage);
        if (!keepKeyboardLayer) keyboardLayer = KeyboardLayout.Layer.LETTERS;
        editorInputType = nextInputType;
        currentEditorFieldId = info == null ? 0 : info.fieldId;
        currentEditorImeOptions = info == null ? 0 : info.imeOptions;
        if (directNumberPad()) keyboardLayer = KeyboardLayout.Layer.SYMBOLS;
        currentEditorPackage = nextPackage;
        allowLearning = info != null && EditorPolicy.allowLearning(info.imeOptions);
        refreshLocalSettings();
        imeDebugOverlay.recordEvent(DiagnosticEventKind.IME_START);
        imeDebugOverlay.flushServiceCreate();
        preferencesNotice = "";
        statisticsFailureReported = false;
        // 外观和引擎是两件事。皮肤、方案角标和键高来自同一份偏好，但它们该跟着用户走，而不是跟着
        // 这个输入框用不用引擎走：密码框和数字框不走引擎，TYPE_NULL 的自定义聊天框仍要走引擎，
        // 否则九键和 26 键会落入「直接输入」而无法组字。
        boolean engineWanted = info != null && connection != null
            && EditorPolicy.useEngine(info.inputType);
        if (keepSession) {
            resumeKeptSession(info, restarting);
            return;
        }
        startInputRuntime(info, restarting, newDocument, startGeneration, engineWanted, true, deferEngine);
    }

    /** 新连接与当前记下的是不是同一个输入框：包名、fieldId、输入类型和 imeOptions 都没变。 */
    private boolean sameEditor(EditorInfo info) {
        return info != null && info.inputType == editorInputType && info.fieldId == currentEditorFieldId
            && info.imeOptions == currentEditorImeOptions && currentEditorPackage.equals(info.packageName);
    }

    /** 同一输入框重启连接时能否留着引擎会话（{@link InputViewRefreshPolicy#keepsSessionAcrossRestart}）。 */
    private boolean keepsSessionAcrossRestart(EditorInfo info, boolean restarting, InputConnection next) {
        if (!restarting || info == null || next == null || session == 0) return false;
        boolean sameEditor = sameEditor(info);
        if (!sameEditor) return false;
        // 资源包可能刚在设置应用里装好或删掉：方案切换器、日文临时模式和音标可用与否都是建会话时按资源包算的，留着会话就一直是旧的。原来每次重启连接都重建、重读一遍，这里同样一次 JNI 列出，变了就不留会话。
        if (!ResourcePacks.installedIds(this).equals(installedResourcePackIds)) return false;
        // 隐私模式可能刚在设置应用里开关过：先按文件重读本地设置，再比这个输入框此刻该不该学习。
        refreshLocalSettings();
        boolean suppressed = !EditorPolicy.allowLearning(info.imeOptions) || incognitoEnabled;
        // 选中只吃掉部分输入的候选后，已选的那一段留在 phrase_prefix 里；失焦会把它上屏，所以和组字一样不留会话。
        boolean composing = hasEngineComposition()
            || !InputViewValuePolicy.textOr(view, "phrase_prefix", "").isEmpty();
        return InputViewRefreshPolicy.keepsSessionAcrossRestart(restarting, session, view != null,
            currentDocumentIdentifier, sameEditor, EditorPolicy.useEngine(info.inputType), composing,
            privacySessionRestartPending, suppressed, sessionLearningSuppressed);
    }

    /**
     * 留着会话的重启连接：按换输入框的规矩复位引擎，而不是销毁重建。失焦写下这一段的选词和延迟学习、给脚本插件 focus_out；聚焦丢掉组字、上下文和联想，重读插件表、资源包和音效。之后与新会话建好时一样，把隐私标记、中英、全半角和中文标点这些「只对本次会话」的状态按偏好重新交给引擎。
     */
    private void resumeKeptSession(EditorInfo info, boolean restarting) {
        message = "";
        refreshKeyStatistics(info, restarting, keyStatisticsDirectory);
        // 与 startInputRuntime 相同：同一文档、同一输入类型时这里不改中英，照样经它记下这次输入开始。
        Boolean englishOverride = inputContext.englishOverride(
            EditorPolicy.prefersLatin(editorInputType), currentDocumentIdentifier, dedicatedEnglish);
        if (englishOverride != null) dedicatedEnglish = englishOverride;
        try {
            apply(NativeClient.focus(session, false));
            apply(NativeClient.focus(session, true));
            markPrivateSession();
            // markPrivateSession 失败时已经经 fail 停掉会话并排上恢复，不能再碰已销毁的句柄。
            if (session == 0) return;
            view = value(NativeClient.setEnglishMode(session, dedicatedEnglish));
        } catch (JSONException | LinkageError error) {
            // 与会话中途的引擎失败同样处理：停掉会话，按这个输入框的运行时选项重建。
            fail(error);
            return;
        }
        applyCharacterWidth(fullWidthPreference);
        applyChinesePunctuation(chinesePunctuationPreference);
        refreshEnglishSuggestions();
        updateAutomaticCapitalization();
        imeLetterRows.refreshKeyRowsForEditor();
        render();
        // 保留会话的重启也可能只复用已有键行；会话切换后仍要重新把当前布局注册给引擎。
        imeTouchPoints.registerAfterLayout();
        synchronizeReplyKeyboard();
        imeWorldClock.resumePicker(getCurrentInputEditorInfo());
    }

    /** 接上 onStartInput 推迟的键行更新和引擎启动（{@link InputViewRefreshPolicy#defersEngineStartup}）。 */
    private void resumeDeferredInputStart(EditorInfo info) {
        if (editorKeyRowsPending) {
            editorKeyRowsPending = false;
            imeLetterRows.refreshKeyRowsForEditor();
            render();
        }
        long deferred = deferredEngineStartup;
        deferredEngineStartup = -1;
        if (!InputViewRefreshPolicy.shouldResumeDeferredStartup(deferred, engineStartGeneration, session,
                connection, getCurrentInputConnection(), info != null && EditorPolicy.useEngine(info.inputType)))
            return;
        // 到这时才知道这个输入框真要建会话，与 onStartInput 里一样先停掉空闲同步和快照处理。
        cancelPersonalDictionarySynchronization();
        cancelDictionarySnapshotProcessing();
        scheduleEngineStartup(runtimeOptionsForSnapshot, engineStartGeneration);
    }

    /**
     * 表情目录和常用语只用运行时选项里的资源目录与偏好目录，不经引擎会话（#10086）：建会话前先按 startEngineSession 的同一套校验（必须是绝对路径）填上这两项，键盘一弹出表情、常用语就能点。stop 时照旧随 stopSessionState 清掉，会话建好时 startEngineSession 按同一份选项再写一次。
     */
    private void prefillSessionIndependentDirectories(JSONObject options) {
        String resources = options.optString("resources", "");
        if (new File(resources).isAbsolute()) emojiResources = resources;
        String directory = InputViewValuePolicy.textOr(options, "preferences_directory", "");
        if (!directory.isEmpty() && new File(directory).isAbsolute()) preferencesDirectory = directory;
    }

    /** 配置准备完成后仍初始化原输入框；不重复触发生命周期，也不重放已直出的文字。 */
    private void startInputRuntime(EditorInfo info, boolean restarting, boolean newDocument,
            long startGeneration, boolean engineWanted, boolean allowBootstrap, boolean deferEngine) {
        message = EngineStartupStatusPolicy.initialMessage(engineWanted);
        String statisticsPreferences = "";
        boolean deferred = deferEngine && engineWanted;
        try {
            if (engineWanted && allowBootstrap
                    && !new File(getFilesDir(), "runtime-options.json").exists()) {
                // 系统可直接启用刚安装的键盘；设置应用不是准备词库的必要入口。
                runtimeOptionsBase = "";
                runtimeOptionsForSnapshot = "";
                pendingRuntimeBootstrap = new PendingRuntimeBootstrap(info, restarting, newDocument);
                if (deferred) deferredEngineStartup = startGeneration;
                else scheduleEngineStartup("", startGeneration);
            } else {
                pendingRuntimeBootstrap = null;
                JSONObject options = new JSONObject(
                    HostOptionsPolicy.readRuntimeOptions(getFilesDir()));
                statisticsPreferences = InputViewValuePolicy.textOr(options, "preferences_directory", "");
                languageDictionaries = options.optString("language_dictionaries", "");
                java.util.Set<String> installedPacks = ResourcePacks.installedIds(this);
                resourcePacks = KeyboardScheme.availablePacks(
                    installedPacks::contains, options.optString("resources", ""));
                candidatePronunciationPackInstalled = installedPacks.contains(ResourcePacks.PRONUNCIATIONS);
                installedResourcePackIds = installedPacks;
                announceWubiTableMissing();
                JSONObject preferences = options.optJSONObject("preferences");
                applyEditorPreferences(preferences, ClipboardHistoryRetentionPolicy.Source.RUNTIME_OPTIONS_COPY);
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
                    loadAppearanceWithoutSession(InputViewValuePolicy.textOr(options, "preferences_directory", ""));
                }
                runtimeOptionsBase = "";
                if (engineWanted) {
                    prefillSessionIndependentDirectories(options);
                    runtimeOptionsBase = options.toString();
                    if (learningSuppressed()) options.getJSONObject("preferences").put("learning", false);
                    runtimeOptionsForSnapshot = options.toString();
                    if (deferred) deferredEngineStartup = startGeneration;
                    else scheduleEngineStartup(runtimeOptionsForSnapshot, startGeneration);
                }
            }
        } catch (Exception | LinkageError error) {
            stop(false);
            // 这句说的是引擎起不来。没打算起引擎的输入框读不到偏好，只是外观退回默认，说「未就绪」
            // 会把一个不存在的故障摆到用户面前。
            if (engineWanted) message = "共享运行时未就绪：仅直接输入";
        }
        refreshKeyStatistics(info, restarting, keyStatisticsDirectory(statisticsPreferences));
        updateAutomaticCapitalization();
        // 推迟建会话的窗口键盘没显示，键行和重画也留给 onStartInputView（resumeDeferredInputStart）。
        editorKeyRowsPending = deferred;
        if (!deferred) {
            imeLetterRows.refreshKeyRowsForEditor();
            render();
        }
        synchronizeReplyKeyboard();
    }

    // One reporting session per keyboard process: a crash here is recorded against it, and a session that ends through onDestroy counts as a normal one.
    @Override public void onCreate() {
        long createStartedAt = SystemClock.uptimeMillis();
        // 拆出去的构建与渲染类持有本服务；在 onCreate 里创建，构造期间不把 this 交出去。
        imeToolbar = new ImeToolbar(this);
        imeCompositionEditor = new ImeCompositionEditor(this);
        imeCandidates = new ImeCandidates(this);
        imeNineKeyPanel = new ImeNineKeyPanel(this);
        imeFunctionPanel = new ImeFunctionPanel(this);
        imeWorldClock = new ImeWorldClock(this);
        imeOcr = new ImeOcr(this);
        imePanels = new ImePanels(this);
        imeLetterRows = new ImeLetterRows(this);
        imeGlideTyping = new ImeGlideTyping(this);
        imeTouchPoints = new ImeTouchPoints(this);
        imeBottomRow = new ImeBottomRow(this);
        imeBottomBar = new ImeBottomBar(this);
        imeLayoutRows = new ImeLayoutRows(this);
        imeStyler = new ImeStyler(this);
        imeFrame = new ImeFrame(this);
        imePrivacyGate = new ImePrivacyGate(this);
        imeVoiceEntry = new ImeVoiceEntry(this);
        imeKeyFeedback = new ImeKeyFeedback(this);
        imeDebugOverlay = new ImeDebugOverlay(this);
        imeTextEditPanel = new ImeTextEditPanel(this);
        imeCalculator = new ImeCalculator(this);
        imeEmailSuffixes = new ImeEmailSuffixes(this);
        imeNineKeyQuickPunctuation = new ImeNineKeyQuickPunctuation(this);
        imeTranslationSenses = new ImeTranslationSenses(this);
        imeStylusHandwriting = new ImeStylusHandwriting(this, HANDWRITING_DEBOUNCE_MILLIS);
        imeStickers = new ImeStickers(this);
        // 必须在 super.onCreate() 之前：InputMethodService 在那里按这个主题建输入法窗口，之后再设会抛异常。按名字查是因为 core/ 要能脱离 Gradle 生成的 R 编译（check-host.sh 的 JVM 冒烟）；res/values/themes.xml 说明了这个主题为什么存在。
        // 五笔、拼音等版本的 applicationId 带后缀，资源表的包名仍是命名空间，两个都试。
        int theme = getResources().getIdentifier("Theme.MSIME.InputMethod", "style", getPackageName());
        if (theme == 0)
            theme = getResources().getIdentifier("Theme.MSIME.InputMethod", "style", "app.msime.android");
        if (theme != 0) setTheme(theme);
        super.onCreate();
        getContentResolver().registerContentObserver(
            Settings.Secure.getUriFor(Settings.Secure.DEFAULT_INPUT_METHOD), false, inputMethodWatcher);
        if (Build.VERSION.SDK_INT >= 30) inlineAutofill = new ImeInlineAutofill(this);
        LauncherIcons.normalize(this);
        hardwareKeyboardAttached = hardwareKeyboardAttached(getResources().getConfiguration());
        productName = getApplicationInfo().loadLabel(getPackageManager()).toString();
        // 偏好要等引擎准备好才读到；先按上次换上的皮肤画，免得每次弹出键盘都先闪一两秒内置的淡绿配色。
        // 候选条外观要经 ImeStyler.themed 按应用主题着色，第一次着色会把主题种子缓存约一分钟；先读本地设置，种子才是用户选的应用主题而不是默认的四季。
        refreshLocalSettings();
        JSONObject hint = readSkinHint();
        if (hint != null) {
            skin = keyboardSkin(hint);
            emojiSkin = surfaceSkin(hint, "emoji_theme");
            handwritingSkin = surfaceSkin(hint, "handwriting_theme");
            applyCandidateAppearance(hint);
            applyTouchGeometry(hint);
        }
        Telemetry.beginInputSession(this);
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        if (clipboard != null) clipboard.addPrimaryClipChangedListener(clipboardWatcher);
        // 键盘进程上次没走到 onDestroy 就被回收时，发出去的图片副本还留在缓存里。
        clearClipboardImages();
        imeDictionarySync = new ImeDictionarySync(this, main);
        imeDictionarySync.start();
        // 键盘进程被系统回收后，下一次弹出键盘先要走这里（#9916）。这时还没进输入框，隐私判断没有依据，耗时先留着，进了允许记录的输入框再写（onStartInput）。
        imeDebugOverlay.holdServiceCreate(SystemClock.uptimeMillis() - createStartedAt);
    }

    @android.annotation.TargetApi(30)
    @Override public android.view.inputmethod.InlineSuggestionsRequest onCreateInlineSuggestionsRequest(Bundle extras) {
        return inlineAutofill == null ? null : inlineAutofill.request(extras);
    }

    @android.annotation.TargetApi(30)
    @Override public boolean onInlineSuggestionsResponse(android.view.inputmethod.InlineSuggestionsResponse response) {
        return inlineAutofill != null && inlineAutofill.response(response);
    }

    @Override public void onStartInputView(EditorInfo info, boolean restarting) {
        super.onStartInputView(info, restarting);
        armFirstFrameWait();
        // 反馈必须在第一下按键前生效，不能被延迟刷新的组字保护跳过。
        loadFeedbackPreferences();
        refreshLocalSettings();
        imeKeyFeedback.prepare();
        if (info != null) automaticNumberRow.enterEditor(info.inputType, info.packageName);
        if (inlineAutofill != null) inlineAutofill.showInput();
        Telemetry.inputViewShown(this);
        cancelInputViewRefresh();
        resumeDeferredInputStart(info);
        // 同一编辑器的键盘视图可以单独收起、再显示。收起时启动代次作废，但这不会触发新的
        // onStartInput；把那次尚未建成的会话重新排上，避免一直停在「准备中」的直接输入状态。
        if (InputViewRefreshPolicy.shouldResumeCancelledStartup(
                pendingEngineStartup, engineStartGeneration, session,
                connection, getCurrentInputConnection(),
                info != null && EditorPolicy.useEngine(info.inputType))) {
            scheduleEngineStartup(runtimeOptionsForSnapshot, engineStartGeneration);
        }
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
            refreshLocalSettings();
            refreshPreferencesOnInputView();
            updateAutomaticCapitalization();
            // 键盘进程不在时复制的那一条，监听收不到；弹出键盘时看一眼是不是刚复制的。
            offerRecentClip(false);
            render();
            imeWorldClock.resumePicker(effectiveInfo);
        };
        main.postDelayed(inputViewRefreshTask, INPUT_VIEW_REFRESH_DELAY_MILLIS);
    }

    @Override public void onFinishInput() {
        operationNotice.cancel();
        if (inlineAutofill != null) inlineAutofill.finishInput();
        imeDebugOverlay.recordEvent(DiagnosticEventKind.IME_FINISH);
        cancelPendingCompositionMarkerLoss();
        flushKeyPresses();
        flushTypingStatistics();
        clearQuickDeleteUndo();
        imeLetterRows.cancelBackspaceRepeat();
        cancelInputViewRefresh();
        // 新输入会话会按当前隐私设置创建，不把上一个输入框的待重建请求带过去。
        privacySessionRestartPending = false;
        engineStartGeneration++;
        cloudClipboardGeneration++;
        clipboardImageGeneration++;
        imeBottomRow.resetSpaceCursor();
        imeCalculator.clear();
        imeEmailSuffixes.clear();
        imeNineKeyQuickPunctuation.clear();
        imeTranslationSenses.clear();
        translationConsumedKeyCode = -1;
        imeStylusHandwriting.inputFinished();
        // 也覆盖 onFinishInputView(true)：那条路径不经过 finishInputViewPresentation。
        imeVoiceEntry.cancel();
        stop(true);
        schedulePersonalDictionarySynchronization(false);
        scheduleDictionarySnapshotProcessing(false);
        // 输入会话刚结束，Rust 这时拿得到词库维护权：还有没写进本机的网盘改动就趁现在写。
        if (imeDictionarySync != null) imeDictionarySync.idle();
        connection = null;
        resetCaretAnchor();
        main.removeCallbacks(caretWindowSync);
        caretWindowHidden = false;
        caretHidePending = false;
        selectionEcho.reset(-1, -1);
        currentDocumentIdentifier = 0;
        super.onFinishInput();
    }

    @Override public void onFinishInputView(boolean finishingInput) {
        operationNotice.cancel();
        if (inlineAutofill != null) inlineAutofill.finishInput();
        imePanels.dismissClipboardPreview();
        flushKeyPresses();
        cancelInputViewRefresh();
        // 键盘收起时还在「键盘调整」就照「确认」保存：用户拖到满意后直接收起键盘或切到别的应用，是把这些值留下来的意思。调整界面没有取消，任何离开方式都保存（见 2026-10-08-touch-keyboard-height-reverts）。
        finishKeyboardAdjust();
        // 跟随光标的候选栏空闲时自己收起的窗口不是用户收起键盘：空闲、键区收起时本来没有要关的面板，而收起请求落地前用户可能已经又开始组字，照常收尾（finishInputViewPresentation 和系统默认实现里的 finishComposingText）会把这段组字上屏、引擎里的组字也随之丢掉。所以两样都不做，落地后再按此刻的状态决定要不要叫回窗口。
        boolean ownHide = !finishingInput && CaretCandidatePolicy.ownHide(caretHidePending, caretWindowHiddenAt,
            android.os.SystemClock.uptimeMillis());
        if (ownHide) {
            caretWindowHidden = true;
            main.post(caretWindowSync);
            return;
        }
        if (!finishingInput) finishInputViewPresentation();
        super.onFinishInputView(finishingInput);
    }

    @Override public void onWindowHidden() {
        // 窗口收起后不再每秒重读偏好：会话要一直活到输入框失焦，按返回键收起键盘后输入框通常还留着焦点，原先这段时间里每秒都在后台读偏好、解析皮肤、把快照推给引擎，电池统计里全算后台耗电（#9916）。窗口再显示时 onWindowShown 先读一次再接着轮询。
        imeWindowVisible = false;
        // 收起前第一帧还没交出去：不再等它，也不记这次弹出的耗时。排着的建会话马上放行，不空等兜底：它自己核对启动代次，用户收起时 onFinishInputView 已经先换了代，放行即作废；跟随光标的候选栏自己收起窗口时不换代，会话照常马上建好。
        firstFrameArmedAt = -1;
        runSessionStartAfterFirstFrame();
        preferencesReloader.stop();
        flushTypingStatistics();
        super.onWindowHidden();
    }

    @Override public void onWindowShown() {
        super.onWindowShown();
        imeWindowVisible = true;
        announceWubiTableMissing();
        if (session != 0 && !preferencesDirectory.isEmpty())
            preferencesReloader.start(preferencesDirectory, this::reloadPreferences);
        caretWindowHidden = false;
        // 系统或应用把窗口叫出来时可能正空闲（例如输入框获得焦点），按此刻的状态再收起。
        main.post(caretWindowSync);
        // 选择页返回时，输入连接刷新可能先于窗口显示；等窗口可见再补一次原编辑器恢复。
        main.post(() -> imeWorldClock.resumePicker(getCurrentInputEditorInfo()));
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
        // 会话在建时记下的那一下「中」键属于这次弹出的键盘：收起后同一输入框重新弹出时启动会重新排上，不能在新键盘上自己执行（#10086）。
        pendingLanguageTap = false;
        deactivateHandwriting();
        imeDebugOverlay.clearDiagnostic();
        // macOS resolves the same focus-loss boundary with finish_composition, so a keyboard put
        // away mid-composition leaves 你好 behind rather than the letters `nihao`. This was
        // CommitRaw only because command 9 was unmapped in the FFI when the path was written.
        if (session != 0 && connection != null && view != null
                && !InputViewValuePolicy.editingText(view).isEmpty()) {
            command(FINISH_COMPOSITION_COMMAND);
        }
        if (privacySessionRestartPending && session != 0) {
            privacySessionRestartPending = false;
            restartSessionForPrivacy();
        }
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        operationNotice.cancel();
        super.onConfigurationChanged(configuration);
        boolean attached = hardwareKeyboardAttached(configuration);
        hardwareKeyboardMode = HardwareKeyboardModePolicy.afterConfiguration(
            hardwareKeyboardMode, hardwareKeyboardAttached, attached);
        hardwareKeyboardUserExpanded = HardwareKeyboardModePolicy.userExpandedAfterConfiguration(
            hardwareKeyboardUserExpanded, hardwareKeyboardAttached, attached);
        boolean hardwareKeyboardNewlyAttached = attached && !hardwareKeyboardAttached;
        hardwareKeyboardAttached = attached;
        // 停在推迟建会话的 TYPE_NULL 窗口上时接上了实体键盘：实体键盘在键盘没显示时就要经引擎组字，现在就建。
        if (hardwareKeyboardNewlyAttached && deferredEngineStartup >= 0)
            resumeDeferredInputStart(getCurrentInputEditorInfo());
        // 旋转、分屏后光标在屏幕上的位置变了：让编辑器立即重报一次，候选栏先按旧位置夹在新窗口里，新位置到了再挪过去。
        if (caretAnchorsAvailable())
            InputConnectionGuard.requestCursorUpdates(connection,
                InputConnection.CURSOR_UPDATE_MONITOR | InputConnection.CURSOR_UPDATE_IMMEDIATE);
        JSONObject preferences = preferencesSnapshot == null ? null
            : preferencesSnapshot.optJSONObject("preferences");
        // 输入法服务可能在深浅模式切换时保留同一个窗口，所有表面都按回调收到的配置重算。
        // 只更新键盘会让候选栏、表情和手写面板停在旧模式。
        boolean systemDark = DimensionPolicy.isNight(configuration.uiMode);
        skin = keyboardSkin(preferences, systemDark);
        emojiSkin = surfaceSkin(preferences, "emoji_theme", systemDark);
        handwritingSkin = surfaceSkin(preferences, "handwriting_theme", systemDark);
        candidateAppearance = candidateAppearanceFor(preferences, systemDark);
        // The input service can survive rotation and window/inset changes. Reapply the
        // geometry even when the palette is unchanged; otherwise the existing key tree keeps
        // margins and height adjustments calculated with the previous configuration. A held
        // nine-key popup is anchored to the old tree, so it must not outlive the reflow.
        imeLayoutRows.dismissNineKeyHoldOptions();
        imeLayoutRows.hideJapaneseFlickPreview();
        // 「键盘调整」中转屏、改分屏尺寸或显示大小：旧屏幕坐标下的键盘上沿已经不对，等外框在新窗口里布局后重新记下。深浅模式、字号等其他变化不清，否则下一次布局会记下拖动后的上沿，应用在调整中途跳一次。
        if (keyboardAdjustActive) {
            if (keyboardAdjustConfiguration == null
                    || (keyboardAdjustConfiguration.diff(configuration) & KEYBOARD_ADJUST_SCREEN_CHANGES) != 0)
                keyboardAdjustKeyboardTop = KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP;
            keyboardAdjustConfiguration = new Configuration(configuration);
        }
        if (keyboardRoot == null) return;
        imeStyler.applyKeyboardSurfaceGeometry();
        imeStyler.applyKeyboardGeometry();
        renderLayoutSettingsState();
        render();
        if (ViewPolicy.isVisible(skinScroll)) imePanels.renderSkinPicker();
        // 输入方式和功能面板的行数随横竖屏变（pickerPhoneLandscape），转屏后按新配置重排，横屏不会留着竖屏的三行；行高和行距由网格按新的面板高度重新量。render 没有会话视图时会提前返回，所以这里自己重排功能面板。
        if (ViewPolicy.isVisible(schemeScroll)) imePanels.renderSchemePicker();
        if (ViewPolicy.isVisible(moreToolsScroll)) imeFunctionPanel.renderMoreTools();
    }

    @Override public void onTrimMemory(int level) {
        super.onTrimMemory(level);
        if (!AndroidMemoryPolicy.shouldReleaseProcessCache(level)) return;
        try {
            if (session != 0) {
                // A live session owns its provider until destroy; reset its Engine caches as well
                // as dropping the process-level pin, matching the iOS memory-warning boundary.
                apply(NativeClient.resetCache(session));
            }
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // Memory callbacks must never break the IME; the process cache is still released below.
        } finally {
            try { NativeClient.releaseProcessCache(); }
            catch (RuntimeException | LinkageError ignored) { }
        }
    }

    @Override public void onLowMemory() {
        super.onLowMemory();
        try {
            if (session != 0) apply(NativeClient.resetCache(session));
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // Memory callbacks must never break the IME; the process cache is still released below.
        } finally {
            try { NativeClient.releaseProcessCache(); }
            catch (RuntimeException | LinkageError ignored) { }
        }
    }

    @Override public void onDestroy() {
        if (imeOcr != null) imeOcr.destroy();
        operationNotice.cancel();
        if (imeWorldClock != null) imeWorldClock.destroy();
        if (imeOcr != null) imeOcr.close();
        voiceActivityGeneration++;
        getContentResolver().unregisterContentObserver(inputMethodWatcher);
        if (inlineAutofill != null) inlineAutofill.finishInput();
        clearQuickDeleteUndo();
        deviceInfoCopyGeneration++;
        deviceInfoInsertGeneration++;
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        if (clipboard != null) clipboard.removePrimaryClipChangedListener(clipboardWatcher);
        main.removeCallbacks(recentClipExpiry);
        main.removeCallbacks(clipboardImageCleanup);
        // 撤销倒数每秒重排自己，不取消的话服务销毁后还会跑到期限，并去刷新已经脱离窗口的按钮。
        main.removeCallbacks(clipboardUndoTick);
        clipboardImageGeneration++;
        imeLetterRows.cancelBackspaceRepeat();
        cancelInputViewRefresh();
        engineStartGeneration++;
        localSettingSaveGeneration++;
        cancelPersonalDictionarySynchronization();
        stop(false);
        schedulePersonalDictionarySynchronization(true);
        // 还欠着的快照处理（刚离开输入框还在等空闲的，或被后面的输入框取消、一直没做成的）在关掉工作线程前排进去，与原来失焦即排队时一样不丢这一轮。
        if (dictionarySnapshotBacklog.owed()) scheduleDictionarySnapshotProcessing(true);
        if (imeDictionarySync != null) imeDictionarySync.stop();
        preferencesWorker.shutdown();
        // Queued before the shutdown, so the worker still writes it.
        flushKeyPresses();
        flushTypingStatistics();
        typingStatisticsWorker.shutdown();
        emojiWorker.shutdown();
        // 已排上的「添加到常用语」照样写完。
        commonPhrasesWorker.shutdown();
        // 同样排在 shutdown 之前：正在复制的那张写完后，这一步把它和更早的副本一起删掉。
        clearClipboardImages();
        clipboardImageWorker.shutdown();
        imeStickers.shutdown();
        imeVoiceEntry.shutdown();
        imeStylusHandwriting.shutdown();
        imeKeyFeedback.shutdown();
        imeTextEditPanel.shutdown();
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

    @Override public void onPrepareStylusHandwriting() {
        imeStylusHandwriting.prepare();
    }

    /** 用户拿手写笔在输入框上写字，系统问要不要接（Android 14 起，#7080）；接不接见 {@link StylusHandwritingPolicy#start}。 */
    @Override public boolean onStartStylusHandwriting() {
        return imeStylusHandwriting.start();
    }

    /** 笔的事件直接交给墨迹视图（原因见 {@link StylusInkView}）；仍交给系统默认实现，它在落笔时排上「闲置五分钟回收书写窗口」的计时，派发到视图的那份视图不处理。 */
    @Override public void onStylusHandwritingMotionEvent(android.view.MotionEvent event) {
        imeStylusHandwriting.motion(event);
        super.onStylusHandwritingMotionEvent(event);
    }

    @Override public void onFinishStylusHandwriting() {
        imeStylusHandwriting.finished();
    }

    /**
     * 接着实体键盘时系统按「使用实体键盘时显示虚拟键盘」决定显不显示输入法窗口。关着时系统不显示，组词的候选就无处可看：走引擎的输入框照样显示窗口，但只给候选条（#5584）。
     */
    @Override public boolean onEvaluateInputViewShown() {
        boolean systemShows = super.onEvaluateInputViewShown();
        boolean attached = hardwareKeyboardAttached(getResources().getConfiguration());
        EditorInfo info = getCurrentInputEditorInfo();
        boolean engineEditor = info != null && EditorPolicy.useEngine(info.inputType);
        boolean next = HardwareKeyboardModePolicy.afterSystemDecision(
            hardwareKeyboardMode, systemShows, attached && engineEditor, hardwareKeyboardUserExpanded);
        if (next != hardwareKeyboardMode) {
            hardwareKeyboardMode = next;
            if (keyboardRoot != null) render();
        }
        return HardwareKeyboardModePolicy.showInputView(systemShows, attached, engineEditor);
    }

    private static boolean hardwareKeyboardAttached(Configuration configuration) {
        return configuration != null
            && HardwareKeyboardModePolicy.attached(configuration.keyboard, configuration.hardKeyboardHidden);
    }

    /** 这次按键是否来自实体键盘（{@link HardwareKeyboardModePolicy#physicalTyping}）。 */
    private static boolean physicalKeyboardEvent(KeyEvent event) {
        android.view.InputDevice device = event.getDevice();
        return HardwareKeyboardModePolicy.physicalTyping(device != null,
            device != null && device.isVirtual(),
            device != null && device.getKeyboardType() == android.view.InputDevice.KEYBOARD_TYPE_ALPHABETIC);
    }

    /**
     * 实体键盘按下一键：进入候选条模式。输入法窗口被系统收着时（接着实体键盘、「显示虚拟键盘」关着），这一键处理完若开始了组词，就把窗口叫出来显示候选条。
     */
    private void noteHardwareTyping() {
        // 推迟建会话的 TYPE_NULL 窗口上来了实体键盘的按键（键盘没被系统报成已接上时会这样）：现在就建，之后的键经引擎组字。
        if (deferredEngineStartup >= 0) resumeDeferredInputStart(getCurrentInputEditorInfo());
        if (!hardwareKeyboardMode) {
            hardwareKeyboardMode = true;
            if (keyboardRoot != null) render();
        }
        long expectedGeneration = engineStartGeneration;
        main.post(() -> {
            // A physical key can arrive just before the editor changes or this service is
            // destroyed. Do not let the queued show request escape its input session.
            if (expectedGeneration != engineStartGeneration) return;
            if (hardwareKeyboardMode && !isInputViewShown() && hasEngineComposition()) requestShowSelf(0);
        });
    }

    /** 键区此刻是否收起成候选条：候选条模式开着，且没有要占用键区高度的面板。 */
    boolean hardwareKeysCollapsed() {
        return HardwareKeyboardModePolicy.keysCollapsed(hardwareKeyboardMode,
            anyToolbarPanelOpen() || candidatePanelOpen || keyboardAdjustActive || handwritingActive());
    }

    /** 候选条上的展开键：退出候选条模式，回到完整的软键盘；下一次实体键盘打字时再收起。 */
    private void expandFromHardwareKeyboardMode() {
        hardwareKeyboardMode = false;
        hardwareKeyboardUserExpanded = true;
        render();
    }

    private void stop(boolean finish) {
        stop(finish, false);
    }

    /** @param keepTextEdit 同一输入框重启连接时留着文本编辑面板（#10141） */
    private void stop(boolean finish, boolean keepTextEdit) {
        boolean keepMoreTools = preserveMoreToolsOnStop;
        preserveMoreToolsOnStop = false;
        stop(finish, keepMoreTools, false, keepTextEdit);
    }

    /** 同一输入框重启连接、留着引擎会话时的收尾（{@link InputViewRefreshPolicy#keepsSessionAcrossRestart}）：只做编辑器边界上的复位。 */
    private void stopKeepingSession(boolean keepTextEdit) {
        boolean keepMoreTools = preserveMoreToolsOnStop;
        preserveMoreToolsOnStop = false;
        stop(false, keepMoreTools, true, keepTextEdit);
    }

    /**
     * 停止并重启输入会话时保留已打开的功能菜单，避免隐私开关切换时闪退式收起再弹出。
     *
     * @param keepSession 为真时不销毁引擎会话，会话带着的偏好快照、皮肤、翻译路由和保存中的状态都不动，只关面板、收起组字区、作废这个输入框上还在路上的请求，与输入框一起结束的那些照常复位。
     */
    private void stop(boolean finish, boolean keepMoreTools, boolean keepSession, boolean keepTextEdit) {
        if (imeWorldClock != null) imeWorldClock.close();
        if (imeOcr != null) imeOcr.close();
        imeCompositionEditor.close();
        pendingEngineStartup = -1;
        deferredEngineStartup = -1;
        pendingRuntimeBootstrap = null;
        engineStartupRequestedAt = -1;
        // 会话在建时推迟的点按只属于这个输入框的这一次启动（#10086）。
        pendingToolbarPanel = null;
        pendingLanguageTap = false;
        // Rebuilding the Engine (including an in-place privacy-mode restart) invalidates any
        // clipboard image copy still running on the worker. Its completion must not submit an
        // image after the user has enabled the privacy gate.
        clipboardImageGeneration++;
        if (!keepSession) {
            candidateTranslationBindingEpoch++;
            candidateTranslationVerifiedGeneration = -1L;
            candidateTranslationAppliedGeneration = -1L;
            userTranslationAppliedGeneration = -1L;
        }
        imeLetterRows.cancelBackspaceRepeat();
        imeDebugOverlay.clearDiagnostic();
        clearSmartPunctuationSnapshots();
        deactivateHandwriting();
        invalidateOnlineProviders();
        invalidateCandidateGlosses();
        if (!keepSession) stopSessionState();
        else clearEnglishSuggestions();
        if (!keepSession && session != 0) {
            // 换输入框、切应用时旧会话在主线程上销毁（写出选词和延迟学习），正排在新输入框的键盘弹出前面，单独记下耗时（#9916）。
            long destroyStartedAt = SystemClock.uptimeMillis();
            try { if (finish && connection != null) apply(NativeClient.command(session, 2)); }
            catch (Exception | LinkageError ignored) { /* Never log editor text or native responses. */ }
            try { NativeClient.destroy(session); } catch (LinkageError ignored) { }
            session = 0;
            imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_SESSION_DESTROY,
                SystemClock.uptimeMillis() - destroyStartedAt);
        }
        // A Stroke composition still marked here (the session stopped without CommitRaw) is stroke glyphs, not text: remove it rather than finish it into the document.
        if (connection != null && strokeCompositionMarked()) bridge.discard(sink(typingSource()));
        // 会话停下时不知道编辑器里是否还有组字区（restartInput 前应用常已清空或改写了文字），换成确定读音可能把它插进文档，所以只结束组字区，不写文字。
        else if (connection != null) bridge.abandon(sink(typingSource()));
        if (!keepSession) view = null;
        if (temporaryEnglishShift) {
            dedicatedEnglish = false;
            temporaryEnglishShift = false;
            letterCase.reset();
        }
        closeCandidatePanel();
        // 换编辑器时不让任何工具栏面板（含常用语）活下来，理由同 finishInputViewPresentation。
        if (keepMoreTools || keepTextEdit) {
            if (!keepMoreTools) closeMoreTools();
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
            if (!keepTextEdit) imeTextEditPanel.close();
        } else {
            closeToolbarPanels();
        }
    }

    /** 跟着引擎会话走的状态：偏好快照和由它算出的皮肤、释义、翻译、五笔设置，以及偏好轮询和保存中的标记。会话销毁时一起清掉，新会话建好后由实时偏好重新填上。 */
    private void stopSessionState() {
        preferencesReloader.stop();
        preferenceSaveGeneration++;
        preferencesDirectory = "";
        emojiResources = "";
        candidateGlossResources = "";
        candidatePronunciationEnabled = false;
        candidatePronunciationAvailable = false;
        candidatePronunciationPackInstalled = false;
        invalidateCandidatePronunciations();
        candidateGlossStateRoot = "";
        candidateEnglishGloss = false;
        candidateTranslationsEnabled = false;
        candidateTranslationAccount = false;
        candidateTranslationTargets = java.util.List.of("en");
        candidateTranslationScope = CandidateTranslationPolicy.Scope.HIGHLIGHTED;
        clearEnglishSuggestions();
        if (candidateTranslationStore != null) candidateTranslationStore.clear();
        suspendCandidateTranslationRoute();
        wubiCodeHint = true;
        wubiMixedPinyin = false;
        wubiProfile = KeyboardScheme.WUBI_86;
        preferencesSnapshot = null;
        appliedPreferencesResponse = null;
        appliedPreferencesInputs = null;
        schemeSaving = false;
        touchGeometrySaving = false;
        panelPreferenceSaving = false;
        skinSaving = false;
        traditionalOutputSaving = false;
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
        if (pendingEngineStartup != generation) {
            dictionaryStartupRetryDeadline = SystemClock.uptimeMillis() + 5_000;
            // 同一代的重试（词库维护忙、首次安装准备完再排一次）不重新计时，记下的是用户从这一代启动开始等了多久。
            engineStartupRequestedAt = SystemClock.uptimeMillis();
        }
        pendingEngineStartup = generation;
        Runnable complete = () -> {
            if (generation != engineStartGeneration || connection == null || session != 0) return;
            startEngineSession(options, null);
        };
        boolean suppressLearning = learningSuppressed();
        startupLearningSuppressed = suppressLearning;
        PendingRuntimeBootstrap bootstrap = pendingRuntimeBootstrap;
        InputConnection startupConnection = connection;
        try {
            preferencesWorker.execute(() -> {
                // onStartInput/finishInputViewPresentation 会使上一代任务失效。工作线程是
                // 单线程的，若让这些任务继续做实时偏好读取和个人词库同步，频繁
                // restartInput 会把新的会话堵在「共享运行时准备中」。
                if (generation != engineStartGeneration) return;
                if (options.isEmpty() && bootstrap != null) {
                    boolean prepared;
                    try {
                        Bootstrap.prepare(this);
                        prepared = true;
                    } catch (Exception | LinkageError error) {
                        prepared = false;
                    }
                    boolean ready = prepared;
                    main.post(() -> {
                        if (generation != engineStartGeneration || connection == null
                                || connection != startupConnection || session != 0
                                || pendingRuntimeBootstrap != bootstrap) return;
                        if (ready) {
                            startInputRuntime(bootstrap.info(), bootstrap.restarting(),
                                bootstrap.newDocument(), generation, true, false, false);
                        } else {
                            stop(false);
                            message = "共享运行时未就绪：仅直接输入";
                            render();
                        }
                    });
                    return;
                }
                long preferencesStartedAt = SystemClock.uptimeMillis();
                EngineStartOptions startOptions = withLivePreferences(options, suppressLearning);
                long preferencesMillis = SystemClock.uptimeMillis() - preferencesStartedAt;
                if (generation != engineStartGeneration) return;
                String notice = "";
                long syncStartedAt = SystemClock.uptimeMillis();
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
                long syncMillis = SystemClock.uptimeMillis() - syncStartedAt;
                if (generation != engineStartGeneration) return;
                String finalNotice = notice;
                main.post(() -> {
                    if (generation != engineStartGeneration || connection == null || session != 0) return;
                    imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_STARTUP_PREFERENCES, preferencesMillis);
                    imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_STARTUP_DICTIONARY_SYNC, syncMillis);
                    if (!finalNotice.isEmpty()) preferencesNotice = finalNotice;
                    startEngineSessionAfterFirstFrame(() -> {
                        if (generation != engineStartGeneration || connection == null || session != 0) return;
                        startEngineSession(startOptions.options(), startOptions.livePreferences());
                    });
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
    private EngineStartOptions withLivePreferences(String optionsText, boolean suppressLearning) {
        try {
            JSONObject options = new JSONObject(optionsText);
            String directory = InputViewValuePolicy.textOr(options, "preferences_directory", "");
            if (directory.isEmpty() || !new File(directory).isAbsolute())
                return new EngineStartOptions(optionsText, null);
            // 读实时偏好之前把本机旧的长空格和数字行设置迁进共享偏好，这次会话就按迁移后的底行和数字行画。
            KeyLayoutStyleMigration.migrate(this, directory);
            String response = candidateSkinResolver.read(directory);
            JSONObject envelope = new JSONObject(response);
            JSONObject live = JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.getJSONObject("value").optJSONObject("preferences") : null;
            if (live == null) return new EngineStartOptions(optionsText, null);
            if (suppressLearning) live.put("learning", false);
            options.put("preferences", live);
            return new EngineStartOptions(options.toString(), response);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return new EngineStartOptions(optionsText, null);
        }
    }

    /** 建会话用的运行时选项，以及换进去的那份实时偏好的原始 loadPreferences 响应（没读到时为 null）。 */
    private record EngineStartOptions(String options, String livePreferences) {}

    /**
     * @param livePreferences 建会话前刚在工作线程上读到的实时偏好（原始响应），会话建好后直接作为第一份偏好快照应用；null 时等 preferencesReloader 读。
     */
    private void startEngineSession(String optionsText, String livePreferences) {
        long startedAt = SystemClock.uptimeMillis();
        try {
            JSONObject options = new JSONObject(optionsText);
            // 选中只吃掉部分输入的候选时，让运行时把已选的那一段留在组字里而不是立刻上屏。这个宿主
            // 会把它画在组字与候选条的读音前面，两件事必须一起做：请求了却不画，用户已经选中的字
            // 既不在文档里也不在屏幕上（scripts/test-phrase-preedit-hosts.py 守的就是这一半状态）。
            options.put("phrase_preedit", true);
            // 没读到实时偏好时（读取失败、建会话的回退路径、没有偏好目录），选项里的偏好是出厂默认的全拼 26 键副本，九键用户的键盘会先跳回 26 键（#10389）。按上次确认的键面把副本换成九键：引擎从九键网格起来，键面照旧只按引擎视图画，画的九键和引擎一致，网格键能打字。实时偏好随后到达时照常换成用户的真实设置。
            JSONObject startupPreferences = livePreferences == null ? options.optJSONObject("preferences") : null;
            if (startupPreferences != null) {
                String copyLayout = InputViewValuePolicy.textOr(startupPreferences, "touch_keyboard_layout", "twenty_six_key");
                String startupLayout = TouchLayoutHintPolicy.startupTouchKeyboardLayout(touchLayoutHint(),
                    InputViewValuePolicy.textOr(startupPreferences, "scheme", "quanpin"), copyLayout);
                if (!startupLayout.equals(copyLayout)) startupPreferences.put("touch_keyboard_layout", startupLayout);
            }
            // 建会话在主线程上，这段时间里点按都要排队（#10086），单独记下它的耗时，与整段等待分开看。
            long createStartedAt = SystemClock.uptimeMillis();
            String created = NativeClient.create(options.toString());
            long createMillis = SystemClock.uptimeMillis() - createStartedAt;
            JSONObject response = new JSONObject(created);
            if ("dictionary maintenance busy".equals(response.optString("error"))
                    && engineStartupPending() && SystemClock.uptimeMillis() < dictionaryStartupRetryDeadline) {
                // 导入页可能刚取得一批词的维护租约。保留「准备中」并稍后重试，不能一次竞争就退回直接输入。
                long generation = engineStartGeneration;
                main.postDelayed(() -> {
                    if (generation == engineStartGeneration && connection != null && engineStartupPending())
                        scheduleEngineStartup(optionsText, generation);
                }, 50);
                return;
            }
            view = value(created);
            session = strictCandidateLong(view, "session");
            // 只有选项里确实写着不学习，才算按不学习建的会话；两边对不上时按会学习记，不学习的输入框重启连接时就会重建而不是留着它。
            JSONObject createdPreferences = options.optJSONObject("preferences");
            sessionLearningSuppressed = startupLearningSuppressed && createdPreferences != null
                && !createdPreferences.optBoolean("learning", true);
            appliedPreferencesResponse = null;
            appliedPreferencesInputs = null;
            String resources = options.optString("resources", "");
            if (new File(resources).isAbsolute()) {
                emojiResources = resources;
                candidateGlossResources = resources;
            }
            String stateRoot = InputViewValuePolicy.textOr(options, "preferences_directory", "");
            candidateGlossStateRoot = new File(stateRoot).isAbsolute() ? stateRoot : "";
            File parent = new File(candidateGlossResources).getParentFile();
            candidatePronunciationAvailable = candidateGlossResources.length() > 0
                && ((parent != null && new File(parent, "pronunciations/en-phonetic.db").isFile())
                    || (candidatePronunciationPackInstalled && !candidateGlossStateRoot.isEmpty()));
            apply(NativeClient.focus(session, true));
            markPrivateSession();
            view = value(NativeClient.setEnglishMode(session, dedicatedEnglish));
            applyCharacterWidth(fullWidthPreference);
            applyChinesePunctuation(chinesePunctuationPreference);
            // focus 可能先按会话的临时中英状态重画键区；恢复编辑器模式后，以实际画出的键区为准，不能比较建会话前预测的布局。
            if (displayedTouchLayout(view) != imeLetterRows.drawnTouchLayout()) {
                letterCase.reset();
                imeLetterRows.rebuildKeyRows();
            }
            refreshEnglishSuggestions();
            String directory = InputViewValuePolicy.textOr(options, "preferences_directory", "");
            boolean hasPreferencesDirectory = !directory.isEmpty() && new File(directory).isAbsolute();
            if (hasPreferencesDirectory) preferencesDirectory = directory;
            // 建会话前刚读过的实时偏好直接作为第一份快照：原先要等 preferencesReloader 在工作线程上再读一遍、回到主线程后才有 preferencesSnapshot，冷启动的应用里这段要一秒左右，其间皮肤和输入方式两个工具栏按钮按「设置加载中」画成灰色（#5680）。应用失败不影响会话，下面的 reloader 马上再读一次并报告。
            boolean liveApplied = false;
            if (hasPreferencesDirectory && livePreferences != null) {
                java.util.List<Object> inputs = preferencesApplyInputs();
                try {
                    applyPreferencesSnapshot(value(livePreferences));
                    rememberAppliedPreferences(livePreferences, inputs);
                    liveApplied = true;
                } catch (JSONException | LinkageError ignored) {
                    // 不记录偏好内容和原生层的返回；下面的 reloader 会再读一次。
                }
            }
            // 先清掉「准备中」再画，否则这次 render 还会把过期的模式标签留在读音行上。
            message = "";
            render();
            recordEngineSessionTiming(createMillis, SystemClock.uptimeMillis() - startedAt);
            // 会话在建时点下的皮肤、输入方式或要轮换语言的「中」键，此刻会话和（多数时候）偏好都有了；偏好还没读到时由 reloadPreferences 第一次读到后再执行。排到这次建会话之后再做：打开面板出的错不能落进下面的 catch 把刚建好的会话停掉。
            main.post(this::runDeferredToolbarTaps);
            // 键行通常在会话建立前已经完成布局；此时外层布局监听不会再次触发，必须在会话就绪后补量一次双拼几何。
            imeTouchPoints.registerAfterLayout();
            // 窗口没显示时（会话在 onStartInput 里先于窗口建好，或在收起后为隐私模式重建）不起轮询，等 onWindowShown 再起。
            // 刚应用的就是几毫秒前读到的实时偏好，不必立刻再整套读一遍，按平常的节奏一秒后再查；没应用上时照旧马上读。
            if (hasPreferencesDirectory && imeWindowVisible) {
                if (liveApplied) preferencesReloader.startAfter(directory, this::reloadPreferences, 1000);
                else preferencesReloader.start(directory, this::reloadPreferences);
            }
            // 输入连接恢复时，时钟数据目录可能尚未恢复，等运行时真正准备好再续接城市选择。
            imeWorldClock.resumePicker(getCurrentInputEditorInfo());
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
                    if (generation != personalDictionarySyncGeneration) return;
                    DictionaryCollectionsStore.Result<DictionaryImportSynchronization.Batch> synchronizedWords =
                        DictionaryCollectionsStore.synchronizeIdle(options);
                    if (!synchronizedWords.ok()) return;
                    DictionaryImportSynchronization.Batch batch = synchronizedWords.value();
                    // 设置页排队的改词写入后立即记录，不依赖两次事后的队列长度；一批内清空也必须通知。
                    if (batch.applied() > 0 || batch.sent() > 0)
                        imeDictionarySync.localChange();
                    // 每 100 毫秒软预算回到主线程确认空闲；失败词挡住待发项且没有进展时停止，避免空转。
                    if (batch.pending() > 0 && (batch.sent() > 0 || batch.failed() == 0))
                        main.post(() -> {
                            if (generation == personalDictionarySyncGeneration && session == 0)
                                schedulePersonalDictionarySynchronization(true);
                        });
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

    private void cancelDictionarySnapshotProcessing() {
        dictionarySnapshotGeneration++;
        if (dictionarySnapshotTask != null) {
            main.removeCallbacks(dictionarySnapshotTask);
            dictionarySnapshotTask = null;
        }
    }

    /**
     * The session has been destroyed, so native maintenance may take the exclusive lock.
     *
     * <p>失焦时不立刻排进工作线程，与空闲同步一样等 {@link #PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS}：换输入框、切应用时紧接着的 onStartInput 会取消它，连着失焦几次也只留最后一次，下一个输入框的建会话不用排在整份词库的扫描后面。只有 onFinishInput 排它：焦点落到桌面这类不打字的窗口时照常处理；按返回键收起键盘不经过 onFinishInput，会话还在，与本改动之前一样不处理。已排进工作线程但还没轮到的，被取消后在线程上直接跳过。被取消的那一轮记在 {@link #dictionarySnapshotBacklog} 里：一直在输入框之间切换、欠满一分钟时，这一次失焦不再延后，立即排队且不可取消；服务销毁时还欠着的在关掉工作线程前补上。
     */
    private void scheduleDictionarySnapshotProcessing(boolean immediate) {
        if (runtimeOptionsForSnapshot.isEmpty()) return;
        String options = runtimeOptionsForSnapshot;
        Context application = getApplicationContext();
        boolean deferrable = dictionarySnapshotBacklog.owe(SystemClock.uptimeMillis()) && !immediate;
        long generation = ++dictionarySnapshotGeneration;
        if (dictionarySnapshotTask != null) main.removeCallbacks(dictionarySnapshotTask);
        Runnable task = () -> {
            if (generation != dictionarySnapshotGeneration) return;
            dictionarySnapshotTask = null;
            // 等的这段时间里又建了会话（没经过 onStartInput 的取消）：这一轮仍记着欠，留给下一次失焦或服务销毁。
            if (session != 0) return;
            // 不可取消的这一轮一定会做，现在就算还清，免得它还没做完时再失焦又排一轮整份扫描。
            if (!deferrable) dictionarySnapshotBacklog.settled();
            try {
                preferencesWorker.execute(() -> {
                    if (deferrable && generation != dictionarySnapshotGeneration) return;
                    try {
                        Path root = DictionarySnapshotQueue.directory(application);
                        DictionarySnapshotWorker.process(root, root.resolve("staging"), options,
                            () -> SyncSignals.accountId(application));
                        if (deferrable) main.post(dictionarySnapshotBacklog::settled);
                    }
                    catch (Exception | LinkageError ignored) { /* Retry at the next idle boundary. */ }
                });
            } catch (RuntimeException ignored) { /* Service shutdown owns the final worker state. */ }
        };
        dictionarySnapshotTask = task;
        if (deferrable) {
            main.postDelayed(task, PERSONAL_DICTIONARY_SYNC_DELAY_MILLIS);
        } else {
            task.run();
        }
    }

    private void applyCandidateAppearance(JSONObject preferences) {
        candidateAppearance = candidateAppearanceFor(preferences);
        if (preferences == null) {
            candidateHorizontal = true;
            candidateFontSize = 16;
            candidatePreeditFontSize = 16;
            candidateScalePercent = 100;
            return;
        }
        // 共享的 `candidate_layout` 是桌面候选窗的横排/竖排，默认竖排；触屏候选条只有一行高，竖排时每个候选占满整行往下排，一屏只露出第一个。和 iOS 一样，触屏键盘始终横排，更多候选在展开面板里看。
        candidateHorizontal = true;
        candidateScalePercent = CandidateScalePolicy.percent(
            NumberPolicy.strictInt(preferences, "candidate_scale_percent", 100));
        candidateFontSize = CandidateScalePolicy.dimension(CandidateAppearance.fontSize(
            NumberPolicy.strictInt(preferences, "candidate_font_size", 16)), candidateScalePercent);
        candidatePreeditFontSize = CandidateScalePolicy.dimension(CandidateAppearance.fontSize(
            NumberPolicy.strictInt(preferences, "candidate_preedit_font_size", candidateFontSize)), candidateScalePercent);
    }

    private void applyTouchGeometry(JSONObject preferences) {
        touchKeySpacingTenths = KeyboardSpacingPolicy.keySpacing(preferences == null ? -1
            : NumberPolicy.strictInt(preferences, "touch_key_spacing_tenths", -1));
        touchRowSpacingTenths = KeyboardSpacingPolicy.rowSpacing(preferences == null ? -1
            : NumberPolicy.strictInt(preferences, "touch_row_spacing_tenths", -1));
        adoptSavedHeightAdjustment(heightAdjustmentFrom(preferences));
        touchVoiceShortcutEnabled = preferences != null
            && preferences.optBoolean("touch_voice_shortcut", false);
        shuangpinKeyHintsEnabled = shuangpinKeyHintsFrom(preferences);
        quanpinShiftEnabled = quanpinShiftFrom(preferences);
        touchKeyStyles = touchKeyStylesFrom(preferences);
        numberRowEnabled = numberRowFrom(preferences);
        twentySixSymbolFirst = symbolFirstFrom(preferences);
        numberPadSpace = numberPadSpaceFrom(preferences);
        nineKeySidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.LETTER_PREFERENCE);
        nineKeyDigitSidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.DIGIT_PREFERENCE);
        oneHandedSidePanel = oneHandedSidePanelFrom(preferences);
        languageKeyLeft = languageKeyLeftFrom(preferences);
        nineKeySwipeDigits = nineKeySwipeDigitsFrom(preferences);
        applyJapaneseToggleInput(japaneseToggleInputFrom(preferences));
        splitKeyboardPortrait = preferences != null
            && preferences.optBoolean(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, false);
    }

    /** 用上保存的键高。键盘里正在「键盘调整」时（应用 `restartInput` 同一个输入框时会走到这里，不经过 onFinishInputView）不动预览，与 applyPreferencesSnapshot 一致；否则预览跳回保存值，确认时什么也存不下。 */
    private void adoptSavedHeightAdjustment(int saved) {
        if (!keyboardAdjustActive) touchKeyboardHeightAdjustment = saved;
    }

    /** 没有偏好或旧文档里没有这个键时按关，与 client-core 的默认值一致。 */
    private static boolean shuangpinKeyHintsFrom(JSONObject preferences) {
        return preferences != null && preferences.optBoolean(ShuangpinKeyHintPolicy.PREFERENCE_KEY, false);
    }

    /** 没有偏好或旧文档里没有这个键时按开（保留 Shift），与 client-core 的默认值一致。 */
    private static boolean quanpinShiftFrom(JSONObject preferences) {
        return preferences == null || preferences.optBoolean(QuanpinSeparatorPolicy.PREFERENCE_KEY, true);
    }

    /** 三类键面的底行样式；没有偏好、缺键或不认识的写法都按标准底行，与 client-core 的宽松读取一致。 */
    private static TouchKeyStyle.Styles touchKeyStylesFrom(JSONObject preferences) {
        if (preferences == null) return TouchKeyStyle.Styles.DEFAULT;
        return TouchKeyStyle.Styles.parse(JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.FOURTEEN_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NINE_KEY_PREFERENCE)));
    }

    /** 顶部数字行；没有偏好或旧文档里没有这个键时按关，与 client-core 的默认值一致。 */
    private static boolean numberRowFrom(JSONObject preferences) {
        return preferences != null && preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false);
    }

    /** 「符」与「123」互换；没有偏好或旧文档里没有这个键时按关，与 client-core 的默认值一致。 */
    private static boolean symbolFirstFrom(JSONObject preferences) {
        return preferences != null && preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false);
    }

    /** 数字键盘空格位置；没有偏好、缺键或不认识的写法都按 `platform_default`（Android 是右列），与 client-core 的宽松读取一致。 */
    private static TouchKeyStyle.NumberPadSpace numberPadSpaceFrom(JSONObject preferences) {
        return preferences == null ? TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT
            : TouchKeyStyle.NumberPadSpace.parse(JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE)));
    }

    /** 九键左侧符号栏的一张表；没有偏好、缺键或不是数组时是空表（出厂表），数组里不是字符串的元素丢掉，与 client-core 的宽松读取一致。 */
    private static java.util.List<String> sidebarSymbolsFrom(JSONObject preferences, String key) {
        return preferences == null ? java.util.List.of() : NineKeySidebarPolicy.fromPreference(preferences.opt(key));
    }

    /** 大屏单手侧边面板；没有偏好、缺键、不是字符串或不认识的写法都按空着，与 client-core 的宽松读取一致。 */
    private static OneHandSidePadPolicy.Panel oneHandedSidePanelFrom(JSONObject preferences) {
        return preferences == null ? OneHandSidePadPolicy.Panel.OFF
            : OneHandSidePadPolicy.parse(JsonPolicy.strictString(preferences.opt(OneHandSidePadPolicy.PREFERENCE_KEY)));
    }

    /** 中/英键在空格左边；没有偏好、缺键、`right` 和不认识的写法都按右边，与 client-core 的宽松读取一致。 */
    private static boolean languageKeyLeftFrom(JSONObject preferences) {
        return preferences != null && TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE));
    }

    /** 九键滑动输入数字；没有偏好、缺键或不认识的写法都按关闭，与 client-core 的宽松读取一致。 */
    private static String nineKeySwipeDigitsFrom(JSONObject preferences) {
        return NineKeySwipePolicy.mode(preferences == null ? null
            : JsonPolicy.strictString(preferences.opt(NineKeySwipePolicy.PREFERENCE)));
    }

    private static boolean japaneseToggleInputFrom(JSONObject preferences) {
        InputFeatureToggle toggle = InputFeatureToggle.JAPANESE_TOGGLE_INPUT;
        return preferences == null ? toggle.enabledByDefault()
            : preferences.optBoolean(toggle.key(), toggle.enabledByDefault());
    }

    /** 开关切换后不继承先前的连点，避免重新开启时替换已经留下的假名。 */
    private void applyJapaneseToggleInput(boolean enabled) {
        if (japaneseToggleInputEnabled == enabled) return;
        japaneseToggleInputEnabled = enabled;
        if (imeLayoutRows != null) {
            imeLayoutRows.resetJapaneseToggle();
            imeLayoutRows.hideJapaneseFlickPreview();
        }
    }

    /** 日语九键左列的 符 长按是否打开表情浏览：顶部工具栏有表情按钮时两处入口重复，不接长按；工具栏关掉表情或整条隐藏时才接上，读屏描述跟着改（{@code ImeLayoutRows.japaneseSymbolPanelKey}）。 */
    boolean japaneseSideEmojiKey() {
        return !toolbarEmoji || toolbarHidden;
    }

    /** 空闲时没有工具按钮就隐藏整行；中文 26 键选了长空格时始终给顶部符号入口留一行，切到英文也留着（{@link #longSpaceToolbarRowKept}）。 */
    boolean toolbarRowHidden() {
        if (longSpaceToolbarRowKept()) return false;
        return toolbarHidden || !(toolbarEmoji || toolbarPhrase || toolbarClipboard || toolbarSkin
            || toolbarScheme || toolbarFloating || toolbarTextEdit || toolbarUndo || toolbarRedo
            || imeLayoutRows.randomDigitsKeepsToolbarRow() || touchVoiceShortcutEnabled);
    }

    /** 工具栏这一行藏着时，打开的面板要不要把它临时显示出来：多数面板（表情等）的返回键就是工具栏上的收起键。剪贴板面板开着、底栏也画着时不用，底栏的剪贴板键再点一次就关上面板。 */
    private boolean toolbarExitNeeded() {
        if (!anyToolbarPanelOpen()) return false;
        return !(ViewPolicy.isVisible(clipboardSurface) && imeBottomBar.shown());
    }

    /** `touch_toolbar` 的按钮开关，以及功能面板直接切换的模糊音、单手、隐私三项；缺键时按 Android 的默认值读。 */
    private void applyToolbarPreferences(JSONObject preferences) {
        applyToolbarPreferences(preferences,
            preferences == null ? null : preferences.optJSONObject("touch_toolbar"));
    }

    /** 同上，按钮开关取自 `toolbar` 而不是 `preferences` 自己的 `touch_toolbar`。 */
    private void applyToolbarPreferences(JSONObject preferences, JSONObject toolbar) {
        toolbarEmoji = toolbar == null || toolbar.optBoolean("emoji", true);
        toolbarClipboard = toolbar == null || toolbar.optBoolean("clipboard", true);
        toolbarSkin = toolbar == null || toolbar.optBoolean("skin", true);
        toolbarUndo = toolbar != null && toolbar.optBoolean("undo", false);
        toolbarRedo = toolbar != null && toolbar.optBoolean("redo", false);
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
        clipboardColumns = ClipboardLayoutPolicy.columns(
            localSettings.choice(AndroidLocalSettings.CLIPBOARD_COLUMNS));
        clipboardSuggestionEnabled = localSettings.bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION);
        clipboardUndoSeconds = localSettings.integer(AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS);
        clipboardDeleteAuthenticationEnabled = localSettings.bool(AndroidLocalSettings.CLIPBOARD_DELETE_AUTHENTICATION);
        clipboardSuggestionActionEnabled = localSettings.bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION);
        clipboardSuggestionActionKind = localSettings.choice(AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION_KIND);
        toolbarFloating = localSettings.bool(AndroidLocalSettings.TOOLBAR_FLOATING);
        toolbarTextEdit = localSettings.bool(AndroidLocalSettings.TOOLBAR_TEXT_EDIT);
        floatingKeyboard = localSettings.bool(AndroidLocalSettings.FLOATING_KEYBOARD);
        caretCandidates = localSettings.bool(AndroidLocalSettings.HARDWARE_CARET_CANDIDATES);
        // 拖动进行中不让重读覆盖手指下的位置；松手后的保存会把它写回文件。
        if (!floatingDragging) {
            floatingX = localSettings.integer(AndroidLocalSettings.FLOATING_KEYBOARD_X);
            floatingY = localSettings.integer(AndroidLocalSettings.FLOATING_KEYBOARD_Y);
        }
        oneHandedMode = localSettings.choice(AndroidLocalSettings.ONE_HANDED);
        splitKeyboardEnabled = localSettings.bool(AndroidLocalSettings.SPLIT_KEYBOARD);
        splitKeyboardThresholdDp = localSettings.integer(AndroidLocalSettings.SPLIT_KEYBOARD_THRESHOLD);
        splitKeyboardGapPercent = localSettings.integer(AndroidLocalSettings.SPLIT_KEYBOARD_GAP);
        splitKeyboardPhoneLandscape = localSettings.bool(AndroidLocalSettings.SPLIT_KEYBOARD_PHONE_LANDSCAPE);
        incognitoEnabled = localSettings.bool(AndroidLocalSettings.INCOGNITO);
        if (incognitoEnabled) clearQuickDeleteUndo();
        // 设置应用里刚切换过时，下一次弹出键盘（onStartInputView 重读本地设置）就跟上；标志没变时不动窗口。
        screenCaptureProtected = localSettings.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION);
        android.app.Dialog imeWindow = getWindow();
        ScreenCaptureProtection.apply(imeWindow == null ? null : imeWindow.getWindow(), screenCaptureProtected);
    }

    /** 键盘高度：本地设置里有设计范围（-46..129）的值就用它，否则沿用共享偏好的 `touch_keyboard_height_adjustment`（-12..48）。 */
    private int heightAdjustmentFrom(JSONObject preferences) {
        if (localSettings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)) {
            return KeyboardHeightPolicy.designHeightAdjustment(
                localSettings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT));
        }
        return KeyboardHeightPolicy.designHeightAdjustment(preferences == null ? Integer.MIN_VALUE
            : NumberPolicy.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
    }

    /** 会话是否不学习：输入框不许个性化学习，或者隐私模式开着。 */
    boolean learningSuppressed() {
        return !allowLearning || incognitoEnabled;
    }

    /** 隐私输入不记录统计，也不显示上屏后联想；同步 Java 视图，旧候选立即失效。用户单独关闭学习时仍记录统计。 */
    private void markPrivateSession() {
        if (session == 0) return;
        try {
            // 隐私设置返回布尔值，先校验信封，再单独读取候选视图。
            JSONObject result = new JSONObject(NativeClient.setPrivateSession(session, learningSuppressed()));
            if (!strictBoolean(result, "ok"))
                throw new InputFailurePolicy.Failure(InputFailurePolicy.Reason.RUNTIME_REJECTED);
            strictBoolean(result, "value");
            view = value(NativeClient.view(session));
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    private void applyVoicePreferences(JSONObject preferences) {
        JSONObject voice = preferences == null ? null : preferences.optJSONObject("voice_input");
        voiceInputEnabled = voice == null || voice.optBoolean("enabled", true);
        voiceLanguage = InputViewValuePolicy.textOr(voice, "language", "zh-CN");
    }

    private void applyAiPreferences(JSONObject preferences) {
        AiPolishConfiguration next = null;
        JSONObject ai = preferences == null ? null : preferences.optJSONObject("ai_assistant");
        if (ai != null && ai.optBoolean("enabled", false)) {
            try {
                String endpoint = InputViewValuePolicy.textOr(ai, "endpoint", "");
                String origin = AiPolishConfiguration.credentialOrigin(endpoint);
                JSONObject tokens = ai.optJSONObject("tokens");
                String token = tokens == null ? "" : tokens.optString(origin, "");
                String prompt = ai.optString(
                    AiPolishConfiguration.promptSlotKey(
                        InputViewValuePolicy.textOr(ai, "prompt_id", "")), "");
                if (!TextPolicy.hasText(prompt)) prompt = AiPolishConfiguration.DEFAULT_PROMPT;
                next = new AiPolishConfiguration(endpoint,
                    InputViewValuePolicy.textOr(ai, "model", ""), prompt, token);
            } catch (IllegalArgumentException ignored) {
                // Invalid settings disable this entry; never log endpoints, models or credentials.
            }
        }
        AiPolishConfiguration previous = aiPolishConfiguration;
        aiPolishConfiguration = next;
        if (ViewPolicy.isVisible(aiPolishContainer)
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

    /**
     * @param source 这份偏好从哪里来。`runtime-options.json` 的副本永远是出厂默认（剪贴板历史关），按它改开关并清空，就是每换一个输入框历史都被清掉的原因（#5602），见 {@link ClipboardHistoryRetentionPolicy}。
     */
    private void applyClipboardPreference(JSONObject preferences,
            ClipboardHistoryRetentionPolicy.Source source) {
        if (clipboardMode == ClipboardModePolicy.Mode.VISITOR) {
            clipboardHistoryEnabled = true;
            return;
        }
        Boolean preference = preferences == null ? null
            : preferences.optBoolean("clipboard_history", false);
        clipboardHistoryEnabled = ClipboardHistoryRetentionPolicy.enabledAfter(
            clipboardHistoryEnabled, source, preference);
        if (ClipboardHistoryRetentionPolicy.clearsHistory(source, preference) && clipboardHistory != null)
            clipboardHistory.clearQuietly();
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
        boolean pronunciation = preferences != null
            && preferences.optBoolean("candidate_pronunciation", false);
        if (candidatePronunciationEnabled != pronunciation) invalidateCandidatePronunciations();
        candidatePronunciationEnabled = pronunciation;
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
        CandidateTranslationRoute nextRoute = candidateTranslationRouteFrom(preferences);
        java.util.List<String> nextTargets = translationTargetsFrom(preferences);
        CandidateTranslationPolicy.Scope nextScope = translationScopeFrom(preferences);
        // 换范围不清缓存（问过的词照样有效），只让离线释义按新的一代重新交给共享层。
        if (candidateTranslationScope != nextScope) invalidateCandidateGlosses();
        if (candidateTranslationsEnabled != nextEnabled
                || candidateTranslationAccount != nextAccount
                || !candidateTranslationTargets.equals(nextTargets)) {
            if (candidateTranslationStore != null) candidateTranslationStore.clear();
            if (userTranslationStore != null) userTranslationStore.clear();
            invalidateCandidateGlosses();
        }
        candidateTranslationsEnabled = nextEnabled;
        candidateTranslationAccount = nextAccount;
        candidateTranslationTargets = nextTargets;
        candidateTranslationScope = nextScope;
        useCandidateTranslationRoute(nextRoute);
    }

    /** Whether the user explicitly chose the MSIME account for candidate translations; absent preferences never choose it. */
    private static boolean candidateTranslationAccountFrom(JSONObject preferences) {
        return candidateTranslationRouteFrom(preferences).provider() == CandidateTranslationRoute.Provider.ACCOUNT;
    }

    /** 候选翻译的路由，规则在 {@link CandidateTranslationPolicy#route}；没有偏好时什么都不发。腾讯缺 `enabled` 时按共享层的默认值算开着，但没有凭据照样选不中。「翻译服务」设置页也用它判断 AI 辅助的配置能不能用，与键盘读同一份规则。 */
    public static CandidateTranslationRoute candidateTranslationRouteFrom(JSONObject preferences) {
        if (preferences == null) return CandidateTranslationRoute.NONE;
        JSONObject niutrans = preferences.optJSONObject("niutrans");
        JSONObject custom = preferences.optJSONObject("custom_translation");
        JSONObject tencent = preferences.optJSONObject("tencent_tmt");
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        String aiEndpoint = translationProviderText(ai, "endpoint");
        String aiOrigin = AiEndpointPolicy.credentialOrigin(aiEndpoint);
        String aiToken = ai == null ? "" : ai.optJSONObject("tokens") == null || aiOrigin == null
            ? translationProviderText(ai, "token")
            : JsonPolicy.strictStringOrEmpty(ai.optJSONObject("tokens").opt(aiOrigin));
        if (aiToken.isEmpty() && ai != null && "deepseek".equals(translationProviderText(ai, "provider")))
            aiToken = translationProviderText(ai, "token");
        return CandidateTranslationPolicy.route(
            preferences.optBoolean("candidate_translations", true),
            preferences.optBoolean("translation_account", false),
            translationProviderEnabled(niutrans, false), translationProviderText(niutrans, "app_id"),
            translationProviderText(niutrans, "apikey"),
            translationProviderEnabled(custom, false), translationProviderText(custom, "endpoint"),
            translationProviderText(custom, "api_key"),
            translationProviderEnabled(tencent, true), translationProviderText(tencent, "secret_id"),
            translationProviderText(tencent, "secret_key"), translationProviderText(tencent, "region"),
            preferences.optBoolean("translation_ai", false), translationProviderText(ai, "provider"),
            aiEndpoint, translationProviderText(ai, "model"), aiToken);
    }

    private static boolean translationProviderEnabled(JSONObject provider, boolean fallback) {
        return provider == null ? fallback : JsonPolicy.strictBoolean(provider.opt("enabled"), fallback);
    }

    private static String translationProviderText(JSONObject provider, String key) {
        return provider == null ? "" : JsonPolicy.strictStringOrEmpty(provider.opt(key));
    }

    /** 换到偏好算出的路由：路由变了就丢掉用户服务的 store 按新路由重建，旧 store 清空后 epoch 前进，正在逐词请求的旧客户端随即停下。客户端和它记下的回答只在路由（服务或凭据）真的换了时才丢掉，stop() 暂置的 NONE 之后回到同一路由时沿用原来的客户端。 */
    private void useCandidateTranslationRoute(CandidateTranslationRoute next) {
        if (next.equals(candidateTranslationRoute)) return;
        candidateTranslationRoute = next;
        if (userTranslationStore != null) userTranslationStore.clear();
        if (userTranslationClient == null || !userTranslationClient.route().equals(next))
            userTranslationClient = next.userService() ? new UserTranslationClient(next) : null;
        userTranslationStore = userTranslationClient != null
            ? new CandidateTranslationStore(userTranslationClient, candidateTranslationWorker, main,
                generation -> applyUserCandidateTranslations(generation))
            : null;
        userTranslationAppliedGeneration = -1L;
        invalidateCandidateGlosses();
    }

    /** 输入会话结束：路由暂置为 NONE，清掉已显示的译文并停下正在进行的请求，但留着用户服务的客户端，下一次会话由偏好算出同一路由时接着用它记下的回答和退避。 */
    private void suspendCandidateTranslationRoute() {
        candidateTranslationRoute = CandidateTranslationRoute.NONE;
        if (userTranslationStore != null) userTranslationStore.clear();
        userTranslationStore = null;
        userTranslationAppliedGeneration = -1L;
    }

    /** 只取主目标语言：触屏键盘不显示第二种候选翻译（见 {@link CandidateTranslationPolicy#MAX_TARGETS}）。 */
    private java.util.List<String> translationTargetsFrom(JSONObject preferences) {
        String primary = preferences == null ? "en"
            : InputViewValuePolicy.textOr(preferences, "translation_target_language", "en");
        return CandidateTranslationPolicy.targets(primary);
    }

    /** 翻译范围；没有偏好、缺键或不认识的值都按「仅当前高亮」。 */
    private static CandidateTranslationPolicy.Scope translationScopeFrom(JSONObject preferences) {
        return CandidateTranslationPolicy.Scope.from(
            preferences == null ? null : preferences.opt(CandidateTranslationPolicy.Scope.KEY));
    }

    /** 范围是否在生效：候选词翻译开着且范围是「仅当前高亮」。 */
    private boolean candidateTranslationScopeLimited() {
        return CandidateTranslationPolicy.scopeLimited(candidateTranslationsEnabled, candidateTranslationScope);
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

    private void invalidateCandidatePronunciations() {
        candidatePronunciationEpoch++;
        candidatePronunciationRequest = "";
        candidatePronunciations = java.util.Map.of();
        candidatePronunciationSourceView = null;
        candidatePronunciationSourcePanel = null;
        candidatePronunciationSourceSuggestions = java.util.List.of();
    }

    boolean candidatePronunciationVisible() {
        return CandidatePronunciationPolicy.visible(candidatePronunciationEnabled,
            candidatePronunciationAvailable);
    }

    String candidatePronunciationAnnotation(String annotation, JSONObject candidate) {
        if (!candidatePronunciationVisible()) return annotation;
        String translation = candidateEnglishGloss || candidateTranslationsEnabled
            ? candidateTranslation(candidate) : "";
        String text = CandidatePronunciationPolicy.lookupText(
            InputViewValuePolicy.textOr(candidate, "text", ""), translation,
            candidateTranslationTargets.contains("en"));
        String reading = candidatePronunciations.getOrDefault(text, "");
        // 保持候选字的基线稳定：音标尚未到达或这个词查不到时，同样占两行次要文字。
        String first = annotation.isEmpty() ? "\u00A0" : annotation;
        return CandidatePronunciationPolicy.withReading(first,
            reading.isEmpty() ? "\u00A0" : reading);
    }

    String candidatePronunciationAccessibilitySuffix(JSONObject candidate) {
        if (!candidatePronunciationVisible()) return "";
        String text = CandidatePronunciationPolicy.lookupText(
            InputViewValuePolicy.textOr(candidate, "text", ""),
            candidateEnglishGloss || candidateTranslationsEnabled ? candidateTranslation(candidate) : "",
            candidateTranslationTargets.contains("en"));
        String reading = candidatePronunciations.getOrDefault(text, "");
        return reading.isEmpty() ? "" : "，音标：" + reading;
    }

    private void scheduleCandidatePronunciations() {
        if (!candidatePronunciationVisible() || session == 0 || view == null
                || candidateGlossResources.isEmpty()) return;
        boolean englishTarget = candidateTranslationTargets.contains("en");
        boolean glossVisible = candidateEnglishGloss || candidateTranslationsEnabled;
        String preferencesKey = englishTarget + ":" + glossVisible;
        java.util.List<String> suggestions = englishSuggestionsActive()
            ? englishSuggestions : java.util.List.of();
        if (candidatePronunciationSourceView == view
                && candidatePronunciationSourcePanel == candidatePanelSnapshot
                && candidatePronunciationSourceSession == session
                && candidatePronunciationSourceSuggestions.equals(suggestions)
                && preferencesKey.equals(candidatePronunciationPreferences)) return;
        try {
            // 横向完整条和展开面板要的比当前一页多，按 displayedCandidateSnapshot 取看得见的那些；普通列表沿用当前视图，避免为音标改变候选代次。
            JSONObject snapshot = !suggestions.isEmpty() && directEnglishActive() ? view
                : candidateHorizontal || candidatePanelOpen || candidatePanelSnapshot != null
                ? displayedCandidateSnapshot() : view;
            long generation = strictCandidateLong(snapshot, "generation");
            JSONArray snapshotCandidates = snapshot.optJSONArray("candidates");
            String request = CandidatePronunciationModel.request(generation,
                snapshotCandidates == null ? new JSONArray() : snapshotCandidates,
                englishTarget, glossVisible, suggestions);
            candidatePronunciationSourceView = view;
            candidatePronunciationSourcePanel = candidatePanelSnapshot;
            candidatePronunciationSourceSession = session;
            candidatePronunciationSourceSuggestions = java.util.List.copyOf(suggestions);
            candidatePronunciationPreferences = preferencesKey;
            String signature = session + ":" + request;
            if (signature.equals(candidatePronunciationRequest)) return;
            candidatePronunciationRequest = signature;
            final long requestedSession = session;
            final long requestedEpoch = candidatePronunciationEpoch;
            final String resources = candidateGlossResources;
            final String supplementaryResources = candidatePronunciationPackInstalled
                ? new File(candidateGlossStateRoot, "resource-packs").getAbsolutePath() : "";
            candidateGlossWorker.execute(() -> {
                try {
                    java.util.Map<String, String> readings = CandidatePronunciationModel.decode(
                        NativeClient.candidatePronunciations(request, resources, supplementaryResources), generation);
                    main.post(() -> {
                        if (!candidatePronunciationVisible() || session != requestedSession
                                || candidatePronunciationEpoch != requestedEpoch || view == null
                                || CandidateGlossPolicy.strictOr(view.opt("generation"), -1) != generation
                                || !signature.equals(candidatePronunciationRequest)) return;
                        candidatePronunciations = readings;
                        render();
                    });
                } catch (JSONException | RuntimeException | LinkageError ignored) {
                    // 音标只是显示信息，离线表失败时不影响输入，也不记录输入内容。
                }
            });
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 输入线程不能因离线音标查询失败而中断。
        }
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

    private static String candidateTranslationBindingKey(SyncSwitch.Binding binding) {
        if (binding == null || binding.accountId() == null) return null;
        return binding.generation() + ":" + binding.accountId();
    }

    private void verifyCandidateTranslationBinding(long generation,
            java.util.function.Consumer<Boolean> onVerified) {
        long epoch = ++candidateTranslationBindingEpoch;
        long expectedSession = session;
        candidateTranslationVerifiedGeneration = -1L;
        try {
            candidateTranslationWorker.execute(() -> {
                app.msime.android.SyncSwitch.Binding binding = SyncSignals.binding(this);
                main.post(() -> {
                    if (epoch != candidateTranslationBindingEpoch || session != expectedSession
                            || view == null || CandidateGlossPolicy.strictOr(view.opt("generation"), -1) != generation
                            || candidateTranslationStore == null) return;
                    // Signed-out users may use the anonymous translation endpoint, but an
                    // unavailable provider cannot prove which account owns cached rows.
                    String key = candidateTranslationBindingKey(binding);
                    String currentKey = candidateTranslationBindingKey(SyncSignals.binding(this));
                    if (!CandidateTranslationPolicy.acceptsBinding(key, currentKey)) {
                        boolean hadEntries = candidateTranslationStore.hasEntries();
                        candidateTranslationStore.bindTo(currentKey);
                        if (hadEntries) applyVerifiedCandidateTranslations(generation, true);
                        onVerified.accept(null);
                        return;
                    }
                    boolean hadEntries = candidateTranslationStore.hasEntries();
                    boolean same = candidateTranslationStore.bindTo(key);
                    if (!same) candidateTranslationAppliedGeneration = -1L;
                    candidateTranslationVerifiedGeneration = generation;
                    if (!same && hadEntries) {
                        applyVerifiedCandidateTranslations(generation, true);
                        return;
                    }
                    onVerified.accept(same);
                });
            });
        } catch (RuntimeException ignored) {
            candidateTranslationStore.bindTo(null);
        }
    }

    String chineseOutput(String text, JSONObject context) {
        if (context != null && context.has("script_conversion")
                && !context.optBoolean("script_conversion", true)) return text;
        int scheme = context == null
            ? (view == null ? -1 : InputViewValuePolicy.scheme(view, -1))
            : InputViewValuePolicy.scheme(context, -1);
        String localMode = context == null
            ? (view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"))
            : InputViewValuePolicy.textOr(context, "local_mode", "none");
        return AndroidChineseTextConversion.outputString(
            text, traditionalChineseOutput, dedicatedEnglish, scheme, localMode);
    }

    String chineseCandidateOutput(String text, JSONObject context, int source) {
        int scheme = context == null ? (view == null ? -1 : InputViewValuePolicy.scheme(view, -1))
            : InputViewValuePolicy.scheme(context, -1);
        String localMode = context == null ? (view == null ? "none"
            : InputViewValuePolicy.textOr(view, "local_mode", "none"))
            : InputViewValuePolicy.textOr(context, "local_mode", "none");
        return AndroidChineseTextConversion.candidateOutput(text, traditionalChineseOutput,
            dedicatedEnglish, scheme, localMode, source);
    }

    private String candidateAppearanceKey() {
        return (candidateHorizontal ? "horizontal" : "vertical") + ":"
            + candidateFontSize + ":" + candidatePreeditFontSize + ":"
            + candidateScalePercent + ":"
            + candidateAppearance.key();
    }

    private String touchGeometryKey() {
        return touchKeySpacingTenths + ":" + touchRowSpacingTenths + ":"
            + touchKeyboardHeightAdjustment + ":"
            + touchVoiceShortcutEnabled + ":" + voiceInputEnabled + ":" + voiceLanguage + ":"
            + shuangpinKeyHintsEnabled + ":" + quanpinShiftEnabled + ":" + splitKeyboardPortrait + ":" + japaneseToggleInputEnabled
            + ":" + oneHandedSidePanel.wire
            + ":" + touchKeyStyles + ":" + numberRowEnabled + ":" + twentySixSymbolFirst + ":" + numberPadSpace.resolved() + ":" + languageKeyLeft
            + ":" + NineKeySidebarPolicy.format(nineKeySidebarSymbols) + ":" + NineKeySidebarPolicy.format(nineKeyDigitSidebarSymbols);
    }

    private void reloadPreferences(String response) {
        if (session == 0) return;
        // 键盘显示期间每秒都会走到这里。文档和它之外的输入都与上次整套应用时相同，就什么也不做：原先每秒都要整份深拷贝两次、经 JNI 推给引擎再读回视图、清一次剪贴板历史、重读设计库，低端机上是持续的发热和每秒一次的卡顿。上次被推迟或失败（preferencesNotice 非空）时照常重试。
        java.util.List<Object> inputs = preferencesApplyInputs();
        if (response != null && preferencesSnapshot != null && preferencesNotice.isEmpty()
                && response.equals(appliedPreferencesResponse) && inputs.equals(appliedPreferencesInputs))
            return;
        boolean previousPreferencesReady = preferencesSnapshot != null;
        String previousView = view == null ? "" : view.toString();
        String previousNotice = preferencesNotice;
        // 比的是实际画出来的皮肤（经应用主题着色后），不是 `skin.key()`：渲染时 `skin` 被换成当季配色，重读时又按偏好重建回 classic 基础色，两者永远不等（见 KeyboardSkin#displayKey）。
        String previousSkin = skin.displayKey(imeStyler.appThemeSeed());
        String previousEmojiSkin = emojiSkin.displayKey(imeStyler.appThemeSeed());
        String previousHandwritingSkin = handwritingSkin.displayKey(imeStyler.appThemeSeed());
        AndroidLocalSettings.Snapshot previousLocalSettings = localSettings;
        String previousAppearance = candidateAppearanceKey();
        String previousGeometry = touchGeometryKey();
        AiPolishConfiguration previousAi = aiPolishConfiguration;
        boolean previousClipboard = clipboardHistoryEnabled;
        boolean previousTraditional = traditionalChineseOutput;
        boolean previousCandidateGloss = candidateEnglishGloss;
        boolean previousCandidatePronunciation = candidatePronunciationEnabled;
        String previousSpellingPosition = spellingPosition;
        boolean previousCandidateTranslations = candidateTranslationsEnabled;
        boolean previousCandidateTranslationAccount = candidateTranslationAccount;
        CandidateTranslationRoute previousCandidateTranslationRoute = candidateTranslationRoute;
        java.util.List<String> previousTranslationTargets = candidateTranslationTargets;
        boolean previousEnglishSuggestions = englishSuggestionsEnabled;
        boolean previousWubiCodeHint = wubiCodeHint;
        boolean previousWubiMixedPinyin = wubiMixedPinyin;
        String previousWubiProfile = wubiProfile;
        KeyboardScheme previousScheme = selectedScheme;
        PredictionButtonsPolicy.Style previousPredictionButtons = predictionButtons;
        try {
            if (response == null) throw new JSONException("Preferences unavailable");
            JSONObject snapshot = value(response);
            applyPreferencesSnapshot(snapshot);
            rememberAppliedPreferences(response, inputs);
        } catch (JSONException | LinkageError error) {
            // Never replace the working session or log preferences/native responses.
            preferencesNotice = " · 设置读取或应用失败，保留当前设置";
        }
        offerRecentClipOnHistoryEnabled(previousClipboard);
        if (previousLocalSettings.bool(AndroidLocalSettings.AUTO_CAPITALIZATION)
                != localSettings.bool(AndroidLocalSettings.AUTO_CAPITALIZATION)) updateAutomaticCapitalization();
        if (previousPreferencesReady != (preferencesSnapshot != null)
                || !previousNotice.equals(preferencesNotice)
                || !previousSkin.equals(skin.displayKey(imeStyler.appThemeSeed()))
                || !previousEmojiSkin.equals(emojiSkin.displayKey(imeStyler.appThemeSeed()))
                || !previousHandwritingSkin.equals(handwritingSkin.displayKey(imeStyler.appThemeSeed()))
                || !previousLocalSettings.equals(localSettings)
                || !previousAppearance.equals(candidateAppearanceKey())
                || !previousGeometry.equals(touchGeometryKey())
                || (previousAi == null ? aiPolishConfiguration != null
                    : !previousAi.equals(aiPolishConfiguration))
                || previousClipboard != clipboardHistoryEnabled
                || previousTraditional != traditionalChineseOutput
                || previousCandidateGloss != candidateEnglishGloss
                || previousCandidatePronunciation != candidatePronunciationEnabled
                || !previousSpellingPosition.equals(spellingPosition)
                || previousCandidateTranslations != candidateTranslationsEnabled
                || previousCandidateTranslationAccount != candidateTranslationAccount
                || !previousCandidateTranslationRoute.equals(candidateTranslationRoute)
                || previousEnglishSuggestions != englishSuggestionsEnabled
                || !previousTranslationTargets.equals(candidateTranslationTargets)
                || previousWubiCodeHint != wubiCodeHint
                || previousWubiMixedPinyin != wubiMixedPinyin
                || !previousWubiProfile.equals(wubiProfile)
                || previousScheme != selectedScheme
                || previousPredictionButtons != predictionButtons
                || !previousView.equals(view == null ? "" : view.toString())) render();
        // 建会话前没读到实时偏好时，会话在建时推迟的点按等到这里第一次有偏好再执行（#10086）。
        if (!previousPreferencesReady && preferencesSnapshot != null) runDeferredToolbarTaps();
    }

    /**
     * {@link #applyPreferencesSnapshot} 在偏好文档之外读的输入：会话、偏好目录、本地设置（含应用主题）、月份（季节配色）、系统明暗、学习开关、键盘内调高度、剪贴板模式与存储、候选包解析结果、设计库文件。其中任何一项变了，即使文档没变也要整套应用。
     */
    private java.util.List<Object> preferencesApplyInputs() {
        Object library = preferencesDirectory.isEmpty() ? null
            : CandidateSkinStamp.file(java.nio.file.Paths.get(preferencesDirectory, "CustomSkins", "library.json"));
        return java.util.Arrays.asList(session, preferencesDirectory, AndroidLocalSettings.load(this),
            java.time.LocalDate.now().getMonthValue(), systemDark(), learningSuppressed(), keyboardAdjustActive,
            clipboardMode, clipboardHistory, candidateSkinResolver.state(), library);
    }

    private void rememberAppliedPreferences(String response, java.util.List<Object> inputs) {
        appliedPreferencesResponse = response;
        appliedPreferencesInputs = inputs;
    }

    private void applyPreferencesSnapshot(JSONObject snapshot) throws JSONException {
        // 任何来源的应用都作废上次的记录：保存后直接应用、触控板、建会话都会换掉会话里的快照，下一次重读必须整套比对。
        appliedPreferencesResponse = null;
        appliedPreferencesInputs = null;
        refreshLocalSettings();
        // 应用主题在本地设置里，季节随月份变：每次重读都按刚读到的本地设置重新解析种子，下面的候选条外观和 reloadPreferences 的皮肤比较才用的是同一个当前种子。渲染时的 refreshSeasonIfNeeded 一分钟才查一次，只靠它的话设置页换了应用主题，键盘最多要等一分钟。
        imeStyler.refreshAppTheme(true);
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
            NumberPolicy.strictInt(preferences, "candidate_font_size", 16));
        int nextPreeditFontSize = CandidateAppearance.fontSize(
            NumberPolicy.strictInt(preferences, "candidate_preedit_font_size", nextFontSize));
        int nextScalePercent = CandidateScalePolicy.percent(
            NumberPolicy.strictInt(preferences, "candidate_scale_percent", 100));
        nextFontSize = CandidateScalePolicy.dimension(nextFontSize, nextScalePercent);
        nextPreeditFontSize = CandidateScalePolicy.dimension(nextPreeditFontSize, nextScalePercent);
        int nextKeySpacing = KeyboardSpacingPolicy.keySpacing(
            NumberPolicy.strictInt(preferences, "touch_key_spacing_tenths", -1));
        int nextRowSpacing = KeyboardSpacingPolicy.rowSpacing(
            NumberPolicy.strictInt(preferences, "touch_row_spacing_tenths", -1));
        int savedHeightAdjustment = heightAdjustmentFrom(preferences);
        // 键盘里正在「键盘调整」时，同步或设置页写来的新快照不能把预览覆盖回去（那样确认时高度已经和保存值相同，什么也不会存下）。
        int nextHeightAdjustment = keyboardAdjustActive ? touchKeyboardHeightAdjustment : savedHeightAdjustment;
        boolean nextVoiceShortcut = preferences.optBoolean("touch_voice_shortcut", false);
        boolean nextShuangpinKeyHints = shuangpinKeyHintsFrom(preferences);
        boolean nextQuanpinShift = quanpinShiftFrom(preferences);
        PredictionButtonsPolicy.Style nextPredictionButtons = PredictionButtonsPolicy.Style.parse(
            JsonPolicy.strictString(preferences.opt(PredictionButtonsPolicy.PREFERENCE_KEY)));
        TouchKeyStyle.Styles nextTouchKeyStyles = touchKeyStylesFrom(preferences);
        boolean nextNumberRow = numberRowFrom(preferences);
        boolean nextSymbolFirst = symbolFirstFrom(preferences);
        TouchKeyStyle.NumberPadSpace nextNumberPadSpace = numberPadSpaceFrom(preferences);
        java.util.List<String> nextSidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.LETTER_PREFERENCE);
        java.util.List<String> nextDigitSidebarSymbols = sidebarSymbolsFrom(preferences, NineKeySidebarPolicy.DIGIT_PREFERENCE);
        OneHandSidePadPolicy.Panel nextOneHandedSidePanel = oneHandedSidePanelFrom(preferences);
        boolean nextLanguageKeyLeft = languageKeyLeftFrom(preferences);
        String nextNineKeySwipeDigits = nineKeySwipeDigitsFrom(preferences);
        boolean nextJapaneseToggleInput = japaneseToggleInputFrom(preferences);
        boolean japaneseToggleInputChanged = japaneseToggleInputEnabled != nextJapaneseToggleInput;
        boolean nextSplitPortrait = preferences.optBoolean(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, false);
        JSONObject nextVoice = preferences.optJSONObject("voice_input");
        boolean nextVoiceEnabled = nextVoice == null || nextVoice.optBoolean("enabled", true);
        String nextVoiceLanguage = nextVoice == null ? "zh-CN"
            : InputViewValuePolicy.textOr(nextVoice, "language", "zh-CN");
        boolean nextClipboard = clipboardMode == ClipboardModePolicy.Mode.VISITOR
            || preferences.optBoolean("clipboard_history", false);
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
        boolean nextCandidatePronunciation = preferences.optBoolean("candidate_pronunciation", false);
        String nextSpellingPosition = CandidatePreeditStylePolicy.normalize(
            InputViewValuePolicy.textOr(preferences, "touch_spelling_position", null));
        boolean nextCandidateTranslations = preferences.optBoolean("candidate_translations", true);
        boolean nextCandidateTranslationAccount = candidateTranslationAccountFrom(preferences);
        CandidateTranslationRoute nextCandidateTranslationRoute = candidateTranslationRouteFrom(preferences);
        boolean nextEnglishSuggestions = preferences.optBoolean("english_suggestions", true);
        java.util.List<String> nextTranslationTargets = translationTargetsFrom(preferences);
        CandidateTranslationPolicy.Scope nextTranslationScope = translationScopeFrom(preferences);
        boolean nextWubiCodeHint = preferences.optBoolean("wubi_code_hint", true);
        boolean nextWubiMixedPinyin = preferences.optBoolean("wubi_mixed_pinyin", false);
        String nextWubiProfile = KeyboardScheme.normalizedWubiProfile(
            InputViewValuePolicy.textOr(preferences, "wubi_profile", KeyboardScheme.WUBI_86));
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
        boolean nextPairedPunctuation = preferences.optBoolean("paired_punctuation", true);
        String nextWordCharacterBinding = wordCharacterBindingFrom(preferences);
        CandidateNavigationPolicy.Bindings nextCandidateNavigation =
            candidateNavigationFrom(preferences.optJSONObject("navigation"));
        boolean nextLanguageCtrl = nextKeybindings != null
            && nextKeybindings.optBoolean("switch_language_ctrl", false);
        String nextDefaultImeMode = "english".equals(
            InputViewValuePolicy.textOr(preferences, "default_ime_mode", "chinese")) ? "english" : "chinese";
        String nextImeModeScope = "global".equals(
            InputViewValuePolicy.textOr(preferences, "ime_mode_scope", "app")) ? "global" : "app";
        KeyboardScheme nextScheme = SchemePreferences.storedScheme(preferences, edition);
        rememberWubiNoticePreferences(preferences);
        SchemeConfiguration nextSchemeConfiguration = schemeConfiguration(preferences, nextScheme);
        JSONObject sessionSnapshot = new JSONObject(accepted.toString());
        // Keep the accepted disk snapshot intact while enforcing editor privacy in this session.
        if (learningSuppressed()) sessionSnapshot.getJSONObject("preferences").put("learning", false);
        JSONObject result = value(NativeClient.updatePreferences(session, sessionSnapshot.toString()));
        markPrivateSession();
        boolean geometryChanged = touchKeySpacingTenths != nextKeySpacing
            || touchRowSpacingTenths != nextRowSpacing
            || touchKeyboardHeightAdjustment != nextHeightAdjustment
            || splitKeyboardPortrait != nextSplitPortrait;
        skin = nextSkin;
        // 实时修改颜色模式和面板明暗时，表情与手写也必须同步；只更新键盘会保留旧配色。
        emojiSkin = surfaceSkin(preferences, "emoji_theme");
        handwritingSkin = surfaceSkin(preferences, "handwriting_theme");
        rememberSkinHint(preferences);
        candidateAppearance = nextCandidateAppearance;
        localModes = nextLocalModes;
        candidateHorizontal = nextHorizontal;
        candidateFontSize = nextFontSize;
        candidatePreeditFontSize = nextPreeditFontSize;
        candidateScalePercent = nextScalePercent;
        spellingPosition = nextSpellingPosition;
        touchKeySpacingTenths = nextKeySpacing;
        touchRowSpacingTenths = nextRowSpacing;
        touchKeyboardHeightAdjustment = nextHeightAdjustment;
        splitKeyboardPortrait = nextSplitPortrait;
        touchVoiceShortcutEnabled = nextVoiceShortcut;
        // 键面提示在 render() 的 updateShuangpinKeyHints 里按它重画；它进了 touchGeometryKey，变了就会触发 render()。
        shuangpinKeyHintsEnabled = nextShuangpinKeyHints;
        // 分词键的键面和点击都按它分支（usesQuanpinSeparator），变了立刻重画。
        quanpinShiftEnabled = nextQuanpinShift;
        // 联想候选栏的关闭键与展开键在 render 里按它显示，变了经 reloadPreferences 的比较重画。
        predictionButtons = nextPredictionButtons;
        // 底行样式和数字行进了 touchGeometryKey，变了就 render()：底行按新样式重排（updateActionRow 的签名带着样式），数字行与样式相关的数字层由 render 里的 numberRowStale、numberPadStale、layerPunctuationStale 重建；只换键的排布，不碰组字。
        touchKeyStyles = nextTouchKeyStyles;
        numberRowEnabled = nextNumberRow;
        // 「符」与「123」互换和数字键盘空格位置同样进了 touchGeometryKey：底行签名带着它们，数字层右列由 render 里的 numberPadSpaceStale 重建。
        twentySixSymbolFirst = nextSymbolFirst;
        numberPadSpace = nextNumberPadSpace;
        // 两张左侧符号表也进了 touchGeometryKey：变了就 render()，render 里的 sidebarStale 按新表重建键行，不打断正在组的字。
        nineKeySidebarSymbols = nextSidebarSymbols;
        nineKeyDigitSidebarSymbols = nextDigitSidebarSymbols;
        // 侧边面板同样进了 touchGeometryKey，变了就 render()，渲染里的 ImeFrame.applyOneHanded 按它摆上或摘下面板。
        oneHandedSidePanel = nextOneHandedSidePanel;
        // 中/英在空格哪一边同样进了 touchGeometryKey：底行签名带着它，右侧大回车外框由 render 里的 tallReturnStale 整块重建。
        languageKeyLeft = nextLanguageKeyLeft;
        // 九键滑动只在触摸判定时读（bindNineKeySwipe），不用重画键盘，下一次触摸就按新方向判定。
        nineKeySwipeDigits = nextNineKeySwipeDigits;
        applyJapaneseToggleInput(nextJapaneseToggleInput);
        voiceInputEnabled = nextVoiceEnabled;
        voiceLanguage = nextVoiceLanguage;
        applyAiPreferences(preferences);
        boolean clipboardTurnedOn = !clipboardHistoryEnabled && nextClipboard;
        clipboardHistoryEnabled = nextClipboard;
        boolean previousJapaneseEmojiKey = japaneseSideEmojiKey();
        applyToolbarPreferences(preferences);
        boolean japaneseEmojiKeyChanged = previousJapaneseEmojiKey != japaneseSideEmojiKey();
        traditionalChineseOutput = nextTraditional;
        // 要在下面改掉当前值之前比较：放在赋值之后时前后两份永远相同，换服务、关翻译后候选下的旧译文要等下一次按键才消失。
        boolean translationDisplayChanged = CandidateTranslationPolicy.displayInvalidated(
            candidateEnglishGloss, nextCandidateGloss, candidateTranslationsEnabled,
            nextCandidateTranslations, candidateTranslationAccount,
            nextCandidateTranslationAccount, candidateTranslationTargets,
            nextTranslationTargets, candidateTranslationRoute, nextCandidateTranslationRoute,
            candidateTranslationScope, nextTranslationScope);
        if (candidateEnglishGloss != nextCandidateGloss || candidateTranslationScope != nextTranslationScope)
            invalidateCandidateGlosses();
        candidateEnglishGloss = nextCandidateGloss;
        if (candidatePronunciationEnabled != nextCandidatePronunciation) invalidateCandidatePronunciations();
        candidatePronunciationEnabled = nextCandidatePronunciation;
        if (englishSuggestionsEnabled != nextEnglishSuggestions) clearEnglishSuggestions();
        englishSuggestionsEnabled = nextEnglishSuggestions;
        if (candidateTranslationsEnabled != nextCandidateTranslations
                || candidateTranslationAccount != nextCandidateTranslationAccount
                || !candidateTranslationTargets.equals(nextTranslationTargets)) {
            if (candidateTranslationStore != null) candidateTranslationStore.clear();
            if (userTranslationStore != null) userTranslationStore.clear();
            invalidateCandidateGlosses();
        }
        candidateTranslationsEnabled = nextCandidateTranslations;
        candidateTranslationAccount = nextCandidateTranslationAccount;
        candidateTranslationTargets = nextTranslationTargets;
        candidateTranslationScope = nextTranslationScope;
        useCandidateTranslationRoute(nextCandidateTranslationRoute);
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
        applyPairedPunctuation(nextPairedPunctuation);
        wordCharacterBinding = nextWordCharacterBinding;
        candidateNavigation = nextCandidateNavigation;
        hardwareLanguageCtrl = nextLanguageCtrl;
        defaultImeMode = nextDefaultImeMode;
        imeModeScope = nextImeModeScope;
        liveInputModePreferencesLoaded = true;
        JSONObject nextView = result.getJSONObject("view");
        // 引号与方头括号样式变了也重建键行：符号键、字母提示和 123 层的键面都跟着它画。
        // 会话快照可能已在模式切换中更新，重建依据是屏幕上实际画出的键区。
        boolean rebuildLayout = imeLetterRows.drawnTouchLayout() != displayedTouchLayout(nextView)
            || japaneseEmojiKeyChanged
            || japaneseToggleInputChanged
            || !InputViewValuePolicy.punctuationMarks(view).equals(
                InputViewValuePolicy.punctuationMarks(nextView));
        enabledSchemes = nextSchemeConfiguration.enabled();
        visibleSchemes = nextSchemeConfiguration.visible();
        selectedScheme = nextSchemeConfiguration.selected();
        rememberChineseScheme();
        preferencesSnapshot = accepted;
        refreshModeSkinLibrary(preferences);
        imeTextEditPanel.applyPreferences(preferences);
        if (!clipboardHistoryEnabled && clipboardHistory != null) {
            clipboardHistory.clearQuietly();
            // The cloud half does not depend on this switch; only a panel left with nothing to show closes.
            if (imePanels.clipboardPanelOpen() && imePanels.cloudClipboardAllowed()) imePanels.renderClipboardHistory();
            else closeClipboardHistory();
        } else if (clipboardTurnedOn && imePanels.clipboardPanelOpen()) {
            // A panel opened before the first live read says the history is off; replace that with the history.
            imePanels.renderClipboardHistory();
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
        if (ViewPolicy.isVisible(voiceResultScroll))
            renderVoiceResult();
        preferencesNotice = strictBoolean(result, "deferred")
            ? " · 设置将在组词结束后应用" : "";
    }

    boolean apply(String response) throws JSONException {
        JSONObject result = value(response);
        JSONObject next = result.getJSONObject("view");
        // 偏好改了样式时，引擎在空闲重建后的那次转换里才换样式，键面在这里跟上。
        // 启动快照可能迟于占位键区：实际已经画成目标布局时不能再重建，免得首次退格摘掉正在按住的键。
        boolean rebuildLayout = imeLetterRows.drawnTouchLayout() != displayedTouchLayout(next)
            || !InputViewValuePolicy.punctuationMarks(view).equals(
                InputViewValuePolicy.punctuationMarks(next));
        String commit = result.isNull("commit") ? null : result.getString("commit");
        if (commit != null) {
            // The runtime widened this already - it holds the character width now, and applies it
            // to everything it finishes. Widening again here would be the second pass over text
            // that is already fullwidth, and would put this host's own rule ahead of the shared one.
            commit = chineseOutput(commit, result.optJSONObject("commit_context"));
        }
        String composing = composingText(next, next.getString("editing_text"), true);
        if (connection != null
                && !bridge.apply(sink(typingSource()), commit, composing)) {
            throw new InputFailurePolicy.Failure(InputFailurePolicy.Reason.EDITOR_REJECTED);
        }
        view = next;
        // 九键 1 键放进候选条的常用标点只留到下一次转换，下面的 render 画这次转换的候选。
        imeNineKeyQuickPunctuation.clear();
        imeDebugOverlay.showDiagnostic(result.isNull("diagnostic") ? null
            : result.optString("diagnostic", ""));
        if (rebuildLayout || !KeyboardLayout.carriesLetterCase(displayedTouchLayout(view)))
            letterCase.reset();
        if (rebuildLayout) imeLetterRows.rebuildKeyRows();
        if (!renderingSuppressed) render();
        return strictBoolean(result, "handled");
    }

    /**
     * 输入框组字区里写的文字。{@code drawn} 为真时取引擎让宿主画的读音 `display_reading`（日语末尾单个 `n` 还是字母），为假时取确定上屏的读音 `reading`（那个 `n` 已是ん）；其余方案两者相同。
     */
    private String composingText(JSONObject next, String editingText, boolean drawn) {
        // Korean marks the composing Hangul, not the key letters editing_text holds; a transition may carry the syllable the key finished and the next one together, and the bridge writes the commit first. Zhuyin's editing_text is the Dachen keys too, and it marks the reading (the conversion and the pending bopomofo) by the same rule.
        int nextViewScheme = InputViewValuePolicy.scheme(next, -1);
        boolean nextDedicatedEnglish = InputViewValuePolicy.booleanValue(
            next, "dedicated_english", dedicatedEnglish);
        // 这些搜索框逐次监听所有文字变化，先写拼音再删除也会触发搜索；确认前只在键盘保留组字。
        if (CandidatePreeditStylePolicy.keepsPinyinOnKeyboard(
                currentEditorPackage, nextViewScheme, nextDedicatedEnglish)
                && "none".equals(InputViewValuePolicy.textOr(next, "local_mode", "none"))) return "";
        String reading = InputViewValuePolicy.textOr(next, "reading", "");
        String phrasePrefix = InputViewValuePolicy.textOr(next, "phrase_prefix", "");
        String preedit = InputViewValuePolicy.textOr(next, "preedit", "");
        // 拼写类方案按 CandidatePreeditStylePolicy 决定把拼写写进输入框还是留在候选栏。写入输入框时画引擎的 `preedit`（双拼按「双拼预编辑」可以是 ni'hao），不是按下的键；光标移走、不经引擎结束组字区时（drawn 为假）留在文档里的仍是按下的键。本地模式和专用英文照旧写 editing_text。
        boolean spellingScheme = !nextDedicatedEnglish
            && CandidatePreeditStylePolicy.writesSpelling(nextViewScheme)
            && "none".equals(InputViewValuePolicy.textOr(next, "local_mode", "none"));
        boolean spelling = spellingScheme && !CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(
            spellingPosition);
        String typed = spellingScheme
            ? spelling
                ? drawn
                    ? CandidatePreeditStylePolicy.inlineSpelling(false, "", preedit, editingText)
                    : editingText
                : ""
            : editingText;
        // 笔画的 editing_text 是字母 hspnzx，reading 才是用户按下的笔画字形（一丨丿丶乛＊），所以同样标记 reading。日语的 editing_text 是罗马字（九键的 ち 送的是 chi），reading 才是假名。哪些方案这样做由引擎的 `draws_reading` 决定。
        String composing = KoreanInputPolicy.composing(
            InputSchemeTraits.drawsReading(nextViewScheme) && !nextDedicatedEnglish,
            phrasePrefix, typed,
            drawn ? InputViewValuePolicy.textOr(next, "display_reading", reading) : reading);
        // 九键的 editing_text 是按下的数字键（64426），不能写进输入框：写的是引擎给的拼音读音 `nine_key_reading`（ni'hao），即候选栏原来画的那段读音。光标移走、不经引擎结束组字区时（drawn 为假）不留下任何文字，与组字不进输入框时相同。注音 9 键例外：上面已经按大千的规则标记 reading（转换结果加未完成的数字），照常留在输入框里。14 键的 editing_text 是组码字母（bugao），不写进输入框，读音画在候选栏上；英文和本地模式画的是 26 键，照常标记。
        boolean letterKeyGrid = !nextDedicatedEnglish && KeyboardLayout.drawsLetterGrid(touchLayout(next));
        // 九键双拼的 editing_text 也是数字（`nine_key` 在它下面为假），同样写读音。
        boolean shuangpinGrid = !nextDedicatedEnglish && nineKeyShuangpinGrid(next);
        if (ZhuyinInputPolicy.hidesNineKeyComposing(InputViewValuePolicy.booleanValue(
                next, "nine_key", false) || letterKeyGrid || shuangpinGrid,
                ZhuyinInputPolicy.active(nextViewScheme, nextDedicatedEnglish))) {
            composing = drawn && spellingScheme && spelling && !letterKeyGrid
                && !CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(spellingPosition)
                ? PhrasePreeditPolicy.composing(phrasePrefix, CandidatePreeditStylePolicy.inlineSpelling(true,
                    InputViewValuePolicy.textOr(next, "nine_key_reading", ""), preedit, editingText))
                : "";
        }
        return composing;
    }

    /**
     * 光标被移走、不经引擎就结束输入框里的组字区时，组字区里的文字就留在文档里。日语末尾单个 `n` 画成字母，确定时却是ん，所以这时先换成确定的读音再结束，不把半截罗马字写进文档；用户点到的选区放回原处。画的和确定的相同时照旧原样结束。只有编辑器回报里还有组字区时才替换（{@link EditorBridge#settle}）：没有组字区时 `setComposingText` 会把读音插到光标处，这时只结束组字。
     */
    private void finishCompositionInPlace(int composingStart, int composingEnd,
            int selectionStart, int selectionEnd) {
        String editingText = view == null ? "" : InputViewValuePolicy.editingText(view);
        String drawn = view == null ? "" : composingText(view, editingText, true);
        String settled = view == null ? "" : composingText(view, editingText, false);
        if (drawn.equals(settled)) bridge.abandon(sink(typingSource()));
        else bridge.settle(sink(typingSource()), settled, composingStart, composingEnd,
            selectionStart, selectionEnd);
    }

    private boolean englishNineKeyActive() {
        return dedicatedEnglish && displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT;
    }

    private boolean directEnglishActive() {
        return dedicatedEnglish && !englishNineKeyActive();
    }

    private boolean englishSuggestionsActive() {
        return directEnglishActive() && englishSuggestionsEnabled
            && EditorPolicy.useEngine(editorInputType) && !EditorPolicy.prefersLatin(editorInputType)
            && !EditorPolicy.uri(editorInputType)
            && !EditorPolicy.password(editorInputType)
            && (editorInputType & android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS) == 0;
    }

    private void clearEnglishSuggestions() {
        englishCorrectionRejectedPrefix = "";
        englishSuggestionEpoch++;
        englishSuggestionRequestedEpoch = -1;
        englishSuggestionRequestedPrefix = "";
        englishSuggestionPrefix = "";
        englishSuggestions = java.util.List.of();
        englishSuggestionCorrected = false;
    }

    private String englishWordBeforeCursor() {
        if (!directEnglishActive() || connection == null) return "";
        return EnglishSuggestionPolicy.currentWord(editorTextBeforeCursor());
    }

    /** 只清除完整的当前英文词，不跨选区、词中光标或被截断的前文。 */
    private String clearableEnglishWord() {
        if (!directEnglishActive() || EditorPolicy.password(editorInputType)
                || !EditorPolicy.useEngine(editorInputType) || quickDeleteCursor(connection) < 0) return "";
        String word = englishWordBeforeCursor();
        if (word.isEmpty() || word.length() >= CAPITALIZATION_CONTEXT_LIMIT) return "";
        CharSequence after = editorReads.after(connection, () -> InputConnectionGuard.textAfterCursor(connection, 1));
        if (after == null || !EnglishSuggestionPolicy.currentWord(after).isEmpty()) return "";
        return word;
    }

    /** 光标前 {@link #CAPITALIZATION_CONTEXT_LIMIT} 个字符，读不到时为 null；英文当前词和自动大写共用这一次读取。 */
    private CharSequence editorTextBeforeCursor() {
        return editorReads.before(connection,
            () -> InputConnectionGuard.textBeforeCursor(connection, CAPITALIZATION_CONTEXT_LIMIT));
    }

    /** 开始一段不写编辑器、可能多处要求重画的处理：期间的 render() 合成 {@link #endRenderBatch()} 时的一次。 */
    private void beginRenderBatch() {
        renderBatchDepth++;
    }

    private void endRenderBatch() {
        if (--renderBatchDepth > 0 || !renderBatchPending) return;
        renderBatchPending = false;
        render();
    }

    /** 一次英文直输的编辑器事件：重画合成一次，光标位置和前后文每样只读一次，最后那次重画也用同一份读取。 */
    private void beginEditorEvent() {
        editorReads.open();
        beginRenderBatch();
    }

    private void endEditorEvent() {
        try {
            endRenderBatch();
        } finally {
            editorReads.close();
        }
    }

    /**
     * 联想候选栏上的关闭键（#10078）：送 Cancel 清掉上屏后联想。引擎对联想的 Cancel 报已处理，不上屏、不往输入框插空格（按空格关闭会多一个空格），已上屏的词序照旧保留；候选清空后 render 换回工具栏和「收起键盘」。展开面板开着时先收起，再取消。
     */
    void dismissPrediction() {
        if (session == 0 || view == null || !view.optBoolean("prediction", false)) {
            render();
            return;
        }
        closeCandidatePanel();
        command(3);
    }

    /** 复用本地模式的取消入口；英文直输必须核对编辑器，再删当前词。 */
    void clearCurrentInput() {
        if (!directEnglishActive()) {
            command(3);
            return;
        }
        String word = clearableEnglishWord;
        if (word.isEmpty() || connection != clearableEnglishConnection
                || clearableEnglishCursor != quickDeleteCursor(connection)
                || !word.equals(clearableEnglishWord())) {
            render();
            return;
        }
        clearEnglishSuggestions();
        deleteBeforeCursor(word.length());
        render();
    }

    private void refreshEnglishClearButton() {
        editorReads.open();
        try {
            String word = clearableEnglishWord();
            if (!clearableEnglishWord.equals(word) || (!word.isEmpty()
                    && (clearableEnglishCursor != quickDeleteCursor(connection)
                        || clearableEnglishConnection != connection))) render();
        } finally {
            editorReads.close();
        }
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
        if (prefix.isEmpty() || !prefix.chars().allMatch(value ->
                TextPolicy.isAsciiLetter(value) || value == '\'')) {
            if (!englishSuggestions.isEmpty() || !englishSuggestionPrefix.isEmpty()) {
                clearEnglishSuggestions();
                render();
            }
            return;
        }
        if (prefix.equals(englishSuggestionRequestedPrefix)
                && englishSuggestionRequestedEpoch == englishSuggestionEpoch) return;
        boolean rejected = prefix.equals(englishCorrectionRejectedPrefix);
        clearEnglishSuggestions();
        if (rejected) englishCorrectionRejectedPrefix = prefix;
        englishSuggestionPrefix = prefix;
        // 共享纠错始终把原文放在第一行；等待查询时先显示当前原文，避免每一键都闪回空闲工具栏。
        englishSuggestions = java.util.List.of(prefix);
        englishSuggestionCorrected = true;
        long epoch = englishSuggestionEpoch;
        String resources = candidateGlossResources;
        englishSuggestionRequestedEpoch = epoch;
        englishSuggestionRequestedPrefix = prefix;
        try {
            englishSuggestionWorker.execute(() -> {
                try {
                    // 共享引擎给出完整展示：有纠错时只有原文与纠错，原文已是完整的词时只有原文，否则原文后接前缀补全；空格提交仍只查询唯一纠错。
                    EnglishSuggestionModel.Result result = EnglishSuggestionModel.decode(
                        NativeClient.englishSuggestions(prefix, resources));
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
        // 核对前文和随后重画里的清除键共用同一次读取。
        editorReads.open();
        try {
            if (epoch != englishSuggestionEpoch || !englishSuggestionsActive()
                    || !prefix.equals(englishWordBeforeCursor())
                    || !prefix.equals(result.prefix())) return;
            englishSuggestions = result.items();
            englishSuggestionCorrected = result.corrected();
            englishSuggestionPrefix = prefix;
            render();
        } finally {
            editorReads.close();
        }
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
        boolean startedCapitalized = !typed.isEmpty() && TextPolicy.isUpperCase(typed.codePointAt(0));
        EnglishSuggestionPolicy.Replacement replacement = EnglishSuggestionPolicy.replacement(
            typed, candidate, startedCapitalized);
        if (replacement == null) {
            // 只有展示了纠错（或答案未到）时点原文才是拒绝纠错；只有补全时点原文仅收起候选。
            boolean rejected = englishSuggestionCorrected;
            clearEnglishSuggestions();
            if (rejected) englishCorrectionRejectedPrefix = typed;
            render();
            return;
        }
        InputConnection target = connection;
        if (!InputConnectionGuard.beginBatchEdit(target)) {
            selectionEcho.invalidate();
            return;
        }
        try {
            if (!deleteBeforeCursor(replacement.deleteCount())) return;
            if (!commitText(fullWidthOutput(replacement.insert()))) return;
        } finally {
            InputConnectionGuard.endBatchEdit(target);
        }
        clearEnglishSuggestions();
        render();
    }

    private Button makeEnglishSuggestionButton(int slot) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        bindCandidateRenderRelease(button);
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
            String reading = candidatePronunciationVisible()
                ? candidatePronunciations.getOrDefault(text, "") : "";
            String annotation = CandidatePronunciationPolicy.suggestionAnnotation(
                candidatePronunciationVisible(), reading);
            button.setText(candidateLabel("", text, annotation, false));
            int labelLines = candidateLabelLines(annotation);
            ViewPolicy.setFixedLines(button, labelLines);
            configureCandidateTextLayout(button, labelLines);
            button.setMaxWidth(candidatePronunciationVisible() ? pixels(240) : Integer.MAX_VALUE);
            if (candidatePronunciationVisible())
                button.setEllipsize(android.text.TextUtils.TruncateAt.END);
            KeyboardTypographyPolicy.setKeyTextSize(button, candidateFontSize);
            button.setContentDescription("英文建议 " + (slot + 1) + "：" + text
                + (reading.isEmpty() ? "" : "，音标：" + reading));
            imeStyler.styleButton(button, false);
            ViewPolicy.setTypeface(button, imeStyler.candidateTypeface());
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = englishSuggestions.size(); slot < englishSuggestionButtons.size(); slot++)
            ViewPolicy.hide(englishSuggestionButtons.get(slot));
    }

    /** 完整候选会放出暂扣尾部并可能重排座位，读取后必须同步同一代视图。 */
    JSONObject currentCandidateSnapshot() throws JSONException {
        return followCandidateGeneration(value(NativeClient.allCandidates(session)));
    }

    /** 横向候选条按需取的一段（{@link NativeClient#candidateRange}）。取到放出来的末尾之后再要会放出暂扣尾部并换一代，同样要同步同一代视图。 */
    JSONObject candidateRangeSnapshot(int start, int count) throws JSONException {
        return followCandidateGeneration(value(NativeClient.candidateRange(session, start, count)));
    }

    /**
     * 释义和音标要覆盖的候选：展开面板（含九键面板）开着时是整代（{@link #currentCandidateSnapshot}），否则是引擎已经放出来的那一段。
     *
     * <p>候选条每键只取一批，从不在打字时放出暂扣的尾部；释义和音标若照旧取整代，首字母打下去引擎仍要把整张声母表查出来、换一代，主线程仍要解析整代的 JSON，开了英文释义或音标的用户就白省了。从 0 起取的这一段不放尾部、不换代，候选条在这一代里滚得到的都在里面；滚过放出来的末尾时候选条自己放尾部、换一代并重画，新的一代再按这里取。上限与释义请求的条数上限相同。
     */
    private JSONObject displayedCandidateSnapshot() throws JSONException {
        if (candidatePanelOpen || candidatePanelSnapshot != null) return currentCandidateSnapshot();
        return candidateRangeSnapshot(0, CandidateGlossModel.MAX_CANDIDATES);
    }

    private JSONObject followCandidateGeneration(JSONObject snapshot) throws JSONException {
        if (strictCandidateLong(snapshot, "session") != session)
            throw new JSONException("Unexpected candidate session");
        if (!sameCandidateVersion(snapshot, view)) {
            JSONObject next = value(NativeClient.view(session));
            if (!sameCandidateVersion(snapshot, next))
                throw new JSONException("Unexpected candidate generation");
            imeTranslationSenses.followOnlineRefresh(next);
            view = next;
        }
        return snapshot;
    }

    /** Copies Engine candidates on the IME thread, then performs only session-free IO off-thread. */
    private void scheduleCandidateGlosses() {
        if (!candidateEnglishGloss || session == 0 || view == null
                || candidateGlossResources.isEmpty()
                || !schemeShowsGlosses(InputViewValuePolicy.scheme(view, -1))) return;
        long requestedGeneration = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        if (requestedGeneration < 0 || (candidateGlossRequestedSession == session
                && candidateGlossRequestedGeneration == requestedGeneration)) return;
        JSONArray visible = view.optJSONArray("candidates");
        if (visible == null || visible.length() == 0) return;
        final long targetSession = session;
        final long targetEpoch = candidateGlossEpoch;
        final String targetResources = candidateGlossResources;
        final String targetStateRoot = candidateGlossStateRoot.isEmpty() ? null : candidateGlossStateRoot;
        final java.util.List<String> offlineTargets = candidateOfflineTargets();
        // 目标语言含英语时，随包的英文释义也要记进 candidateOfflineGlosses：合并在线译文时靠它让离线释义优先。只在装了非英文离线词典时才记，没装的用户（默认配置）在线译文会把「你好 hello」这类离线英文释义盖掉。
        final boolean englishTarget = candidateTranslationTargets.contains("en");
        final String request;
        final long generation;
        final java.util.Map<String, String> targetRequests =
            new java.util.LinkedHashMap<>(offlineTargets.size());
        try {
            JSONObject snapshot = displayedCandidateSnapshot();
            generation = strictCandidateLong(snapshot, "generation");
            request = CandidateGlossModel.request(generation,
                snapshot.getJSONArray("candidates"));
            // The account path's scheme gate: a Japanese composition is not glossed into other languages. Korean Hanja rows are, as the shared translation query answers them.
            if ("none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none")) && InputViewValuePolicy.scheme(view, -1) != 3) {
                for (String language : offlineTargets) {
                    targetRequests.put(language, CandidateGlossModel.request(generation,
                        snapshot.getJSONArray("candidates"), language));
                }
            }
        } catch (JSONException | RuntimeException | LinkageError error) {
            candidateGlossRequestedSession = targetSession;
            candidateGlossRequestedGeneration = requestedGeneration;
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
                    if (!targetRequests.isEmpty() || englishTarget) {
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
            translations = mergedCandidateGlosses(token.generation(), false).toString();
        }
        try {
            if (new JSONArray(translations).length() == 0) return;
            JSONObject applied = value(NativeClient.applyTranslations(
                token.session(), token.generation(), translations));
            if (!CandidateGlossPolicy.isApplied(applied.opt("applied"))) return;
            JSONObject next = applied.getJSONObject("view");
            if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != token.session()
                    || CandidateGlossPolicy.strictOr(next.opt("generation"), -1) != token.generation()) return;
            view = next;
            if (offline != null) {
                candidateTranslationAppliedGeneration = -1L;
                userTranslationAppliedGeneration = -1L;
            }
            if (candidatePanelOpen) {
                candidatePanelSnapshot = currentCandidateSnapshot();
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
                || !"none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"))) return;
        if (InputViewValuePolicy.scheme(view, -1) == 3 || !schemeShowsGlosses(InputViewValuePolicy.scheme(view, -1))) return;
        JSONArray entries = view.optJSONArray("candidates");
        long generation = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        if (entries == null || entries.length() == 0 || generation < 0) return;
        int candidateCount = BoundsPolicy.atMost(entries.length(), 32);
        boolean scopeLimited = candidateTranslationScopeLimited();
        java.util.ArrayList<String> words = new java.util.ArrayList<>(candidateCount);
        for (int index = 0; index < candidateCount; index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate == null || !CandidateTranslationPolicy.userTranslationSourceAllowed(
                    InputViewValuePolicy.integer(candidate, "source", -1))) continue;
            // 「仅当前高亮」只问高亮的那一个；高亮一动代次就换，store.refresh 会丢掉还在路上的旧请求，问过的词从缓存取。
            if (!CandidateTranslationPolicy.requestsTranslation(scopeLimited,
                    InputViewValuePolicy.booleanValue(candidate, "highlighted", false))) continue;
            words.add(InputViewValuePolicy.textOr(candidate, "text", ""));
        }
        verifyCandidateTranslationBinding(generation, same -> {
            if (same != null) {
                candidateTranslationStore.refresh(words, candidateTranslationTargets, generation);
                if (same && candidateTranslationStore.hasEntries()
                        && candidateTranslationAppliedGeneration != generation)
                    applyVerifiedCandidateTranslations(generation);
            }
        });
    }

    private void applyCandidateTranslations(long generation) {
        if (!candidateTranslationAccount || session == 0 || view == null
                || CandidateGlossPolicy.strictOr(view.opt("generation"), -1) != generation) return;
        verifyCandidateTranslationBinding(generation, same -> {
            if (same == null) return;
            if (same) applyVerifiedCandidateTranslations(generation);
            else scheduleCandidateTranslations();
        });
    }

    private void applyVerifiedCandidateTranslations(long generation) {
        applyVerifiedCandidateTranslations(generation, false);
    }

    private void applyVerifiedCandidateTranslations(long generation, boolean allowEmpty) {
        JSONObject next = appliedOnlineTranslations(generation, allowEmpty);
        if (next == null) return;
        view = next;
        candidateTranslationAppliedGeneration = generation;
        render();
    }

    /** 把这一代的离线释义和在线译文合并后交给共享层，返回应用后的 view；没有可应用的、共享层没接受或 view 已不是这一代时为 null。调用方先记下已应用的代次再 render()，否则 render() 里的调度会把同一代再应用一遍。 */
    private JSONObject appliedOnlineTranslations(long generation, boolean allowEmpty) {
        JSONArray entries = view.optJSONArray("candidates");
        if (entries == null || entries.length() == 0) return null;
        JSONArray translations = mergedCandidateGlosses(generation, true);
        if (translations.length() == 0 && !allowEmpty) return null;
        try {
            JSONObject applied = value(NativeClient.applyTranslations(session, generation,
                translations.toString()));
            if (!CandidateGlossPolicy.isApplied(applied.opt("applied"))) return null;
            JSONObject next = applied.getJSONObject("view");
            if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != session
                    || CandidateGlossPolicy.strictOr(next.opt("generation"), -1) != generation) return null;
            return next;
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // Online translations are optional display state.
            return null;
        }
    }

    /**
     * 用户自己的翻译服务（小牛、自定义、腾讯）给当前一页候选取译文。门槛与账号路径相同：没有本地模式、不是日文方案、方案显示释义；另外按共享层的规则跳过表情、颜文字和日文混输候选（source 6、7、13），只取前 {@link #USER_TRANSLATION_PAGE} 个。不经过账号绑定校验：这条路不碰水杉账号。
     */
    private void scheduleUserCandidateTranslations() {
        if (!candidateTranslationRoute.userService() || userTranslationStore == null || session == 0
                || view == null || !"none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"))) return;
        int scheme = InputViewValuePolicy.scheme(view, -1);
        if (scheme == 3 || !schemeShowsGlosses(scheme)) return;
        JSONArray entries = view.optJSONArray("candidates");
        long generation = CandidateGlossPolicy.strictOr(view.opt("generation"), -1);
        if (entries == null || entries.length() == 0 || generation < 0) return;
        int candidateCount = BoundsPolicy.atMost(entries.length(), USER_TRANSLATION_PAGE);
        boolean scopeLimited = candidateTranslationScopeLimited();
        java.util.ArrayList<String> words = new java.util.ArrayList<>(candidateCount);
        for (int index = 0; index < candidateCount; index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate == null) continue;
            int source = InputViewValuePolicy.integer(candidate, "source", -1);
            if (!CandidateTranslationPolicy.userTranslationSourceAllowed(source)) continue;
            // 与账号路径相同：「仅当前高亮」只把高亮的那一个交给用户自己的服务（含复用的 AI 助手）。
            if (!CandidateTranslationPolicy.requestsTranslation(scopeLimited,
                    InputViewValuePolicy.booleanValue(candidate, "highlighted", false))) continue;
            words.add(InputViewValuePolicy.textOr(candidate, "text", ""));
        }
        userTranslationStore.refresh(words, candidateTranslationTargets, generation);
        if (userTranslationStore.hasEntries() && userTranslationAppliedGeneration != generation)
            applyUserCandidateTranslations(generation);
    }

    private void applyUserCandidateTranslations(long generation) {
        if (!candidateTranslationRoute.userService() || userTranslationStore == null || session == 0 || view == null
                || CandidateGlossPolicy.strictOr(view.opt("generation"), -1) != generation) return;
        JSONObject next = appliedOnlineTranslations(generation, false);
        // 没有可应用的也记下这一代，免得每次 render() 都把同一代重新合并一遍；新译文到达时由 store 的回调再进来。
        userTranslationAppliedGeneration = generation;
        if (next == null) return;
        view = next;
        render();
    }

    /**
     * 一代候选的 apply_translations 载荷：逐个候选、按目标语言顺序，每种语言先取离线释义，没有时取在线译文（账号的，或用户自己服务的）。`includeAccount` 同时管这两种在线来源。只来自离线释义的回答标 `"gloss": true`，共享层在「仅当前高亮」时也每行都画；范围生效时在线译文只取视图里高亮的那个候选：共享层这时把不带标记的回答都当作高亮行的候选词翻译，其他候选缓存里的译文交过去也画不出来，高亮移到它们时会在新的一代重新交。
     *
     * <p>The offline rows are the bundled English glosses when English is a target, plus every installed non-English dictionary; without either this is the online translations of the first 32 candidates. It also covers every candidate the offline dictionaries answered, since the payload replaces the one applied before it.
     */
    private JSONArray mergedCandidateGlosses(long generation, boolean includeAccount) {
        java.util.Map<String, java.util.Map<String, String>> offline =
            candidateOfflineGlosses != null && candidateOfflineGlossSession == session
                && candidateOfflineGlossGeneration == generation ? candidateOfflineGlosses : java.util.Map.of();
        JSONArray entries = view == null ? null : view.optJSONArray("candidates");
        int textCapacity = entries == null ? 0 : BoundsPolicy.atMost(entries.length(), 32);
        for (java.util.Map<String, String> glosses : offline.values()) textCapacity += glosses.size();
        java.util.LinkedHashSet<String> texts = new java.util.LinkedHashSet<>(textCapacity);
        for (int index = 0; entries != null && index < BoundsPolicy.atMost(entries.length(), 32); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null && CandidateTranslationPolicy.userTranslationSourceAllowed(
                    InputViewValuePolicy.integer(candidate, "source", -1)))
                texts.add(InputViewValuePolicy.textOr(candidate, "text", ""));
        }
        for (java.util.Map<String, String> glosses : offline.values()) texts.addAll(glosses.keySet());
        boolean scopeLimited = candidateTranslationScopeLimited();
        int highlightedSlot = scopeLimited && view != null ? highlightedCandidateSlot(view) : -1;
        JSONObject highlightedEntry = highlightedSlot < 0 || entries == null ? null : entries.optJSONObject(highlightedSlot);
        String highlightedText = highlightedEntry == null ? null : InputViewValuePolicy.textOr(highlightedEntry, "text", "");
        boolean account = includeAccount && candidateTranslationAccount && candidateTranslationStore != null
            && candidateTranslationVerifiedGeneration == generation;
        // 用户自己的服务没有账号绑定要校验，调用方已保证 generation 是当前 view 的这一代。
        CandidateTranslationStore user = includeAccount && !account && candidateTranslationRoute.userService()
            ? userTranslationStore : null;
        JSONArray translations = new JSONArray();
        for (String text : texts) {
            java.util.HashMap<String, String> offlineRows =
                new java.util.HashMap<>(candidateTranslationTargets.size());
            java.util.HashMap<String, String> onlineRows =
                new java.util.HashMap<>(candidateTranslationTargets.size());
            boolean online = CandidateTranslationPolicy.requestsTranslation(scopeLimited, text.equals(highlightedText));
            for (String target : candidateTranslationTargets) {
                java.util.Map<String, String> glosses = offline.get(target);
                if (glosses != null && glosses.containsKey(text)) offlineRows.put(target, glosses.get(text));
                if (!online) continue;
                if (account) {
                    String translation = candidateTranslationStore.gloss(text, target);
                    if (translation != null) onlineRows.put(target, translation);
                } else if (user != null) {
                    String translation = user.gloss(text, target);
                    if (translation != null) onlineRows.put(target, translation);
                }
            }
            CandidateTranslationPolicy.Answer answer = CandidateTranslationPolicy.answer(
                text, candidateTranslationTargets, offlineRows, onlineRows);
            if (answer == null) continue;
            try {
                translations.put(new JSONObject().put("text", answer.text())
                    .put("translation", answer.translation()).put("gloss", answer.gloss()));
            } catch (JSONException ignored) { return new JSONArray(); }
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
            return JsonPolicy.objectValue(envelope);
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
        aiResolvedSignature = "";
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
                JSONObject result = envelope.optJSONObject("value");
                // 清除候选返回的是 {applied, view}，外层操作结果不能当作候选视图；偏好未变时轮询不会重新应用快照来纠正它。
                if (result != null && JsonPolicy.strictTrue(result.opt("applied"))) {
                    JSONObject next = result.optJSONObject("view");
                    if (next != null) view = next;
                }
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
            String content = JsonPolicy.strictString(message.opt("content"));
            if (content == null) return texts;
            if (!OnlineCandidatePolicy.acceptsAiContent(content)) return texts;
            JSONArray entries = new JSONObject(OnlineCandidatePolicy.stripCodeFence(content))
                .optJSONArray("candidates");
            if (entries == null) return texts;
            texts = new java.util.ArrayList<>(entries.length());
            for (int index = 0; index < entries.length(); index++) {
                JSONObject entry = entries.optJSONObject(index);
                if (entry != null) {
                    String text = JsonPolicy.strictString(entry.opt("text"));
                    if (text != null) texts.add(text);
                }
            }
            return texts;
        } catch (JSONException | RuntimeException error) {
            return java.util.List.of();
        }
    }

    /** The AI HTTPS descriptor in this envelope, or null when it no longer matches the settings. */
    private static JSONObject aiRequestDescriptor(String raw) {
        if (raw == null) return null;
        try {
            JSONObject envelope = new JSONObject(raw);
            return JsonPolicy.objectValue(envelope);
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
            InputViewValuePolicy.booleanValue(query, "cloud_eligible", false),
            InputViewValuePolicy.booleanValue(query, "ai_eligible", false),
            InputViewValuePolicy.booleanValue(assistant, "enabled", false) ? assistant.toString() : "")
            + "|cloud_provider=" + (query.optJSONObject("cloud_request") == null ? ""
                : query.optJSONObject("cloud_request").optString("provider_id", ""));
        if (signature.equals(onlineSignature)) return;
        onlineSignature = signature;
        if (!signature.equals(aiResolvedSignature)) aiResolvedSignature = "";
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
        String aiResolveDocument = null;
        try {
            JSONObject query = new JSONObject(document);
            cloudComplete = !requestsCloud(query);
            String aiDocument = document;
            if (requestsCloud(query)) {
                JSONObject descriptor = query.optJSONObject("cloud_request");
                if (descriptor != null) {
                    String body = OnlineCandidateTransport.cloud(descriptor);
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
                aiAssistant == null ? 0 : NumberPolicy.strictInt(aiAssistant, "candidate_limit", 0));
            if (!requestsAi(aiQuery) || limit == 0) {
                aiComplete = true;
                return;
            }
            final String queryDocument = aiDocument;
            aiResolveDocument = aiDocument;
            // 这个组字问过 AI 而没有结果：不再发请求。
            if (requestSignature.equals(aiResolvedSignature)) return;
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
            // AI 发问了却没有采用任何回答（请求发不出去、失败、空回答、被拒）：告诉 Host API 这次没有回答。共享内核已经不放 AI 占位行，这个调用答 applied:false、不改候选列表；调用和接口留着不变（#6888）。
            if (!aiComplete && aiResolveDocument != null && epoch == onlineEpoch) {
                final String resolveDocument = aiResolveDocument;
                applyOnlineResult(targetSession, epoch,
                    () -> NativeClient.resolveOnlineCandidates(targetSession, resolveDocument, 1));
                // AI 这一边到此为止：只有云候选失败时才释放签名重试，重试也不再问 AI（见 aiResolvedSignature）。
                aiResolvedSignature = requestSignature;
                aiComplete = true;
            }
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
                // 在线结果会推进 generation：组字和高亮候选没变时义项页跟过去，不让它被这次刷新关掉。
                imeTranslationSenses.followOnlineRefresh(next);
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
        int lines = CandidateTranslationPolicy.glossLines(
            candidateTranslationTargets, candidateEnglishGloss,
            candidateTranslationAccount || candidateTranslationRoute.userService(),
            candidateOfflineTargets());
        return candidatePronunciationVisible() ? Math.max(2, lines) : lines;
    }

    void updateCandidateViewportHeight() {
        if (candidateLine == null) return;
        // 42 dp 的候选行容下候选字、一行释义和选中 chip 的留白；第二行起每行再加高一些。
        int reserved = CandidateTranslationPolicy.reservedGlossRows(candidateGlossLineCount(), koreanHanjaRows());
        int extraRows = BoundsPolicy.nonNegative(reserved - 1);
        int line = CandidateScalePolicy.dimension(
            ImeToolbar.CANDIDATE_LINE_DP + extraRows * ImeToolbar.EXTRA_GLOSS_ROW_DP,
            candidateScalePercent);
        ViewPolicy.setFixedHeight(candidateLine, pixels(line));
        // 读音行至少是设计的 14 dp，读音字号放不下时按读音文字的实际高度加上和候选行之间的间距加高，见 ReadingRowPolicy。
        int readingRow = readingRowHeight();
        if (candidateHeader != null) ViewPolicy.setFixedHeight(candidateHeader, readingRow);
        // 未单独设置工具栏尺寸时继续跟随读音行与候选行，「候选栏大小」调小时工具栏这一行跟着变矮，按钮在 applyShortcutSize 里随行高缩小。显式尺寸按固定基准计算，两种状态中较矮的一侧补齐顶部空白，不允许工具栏单独比候选栏矮。
        boolean idleHeader = ViewPolicy.isVisible(candidateHeader);
        int composingHeight = readingRow + pixels(line);
        boolean customSize = localSettings != null && localSettings.has(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT);
        int sizePercent = customSize ? localSettings.integer(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT) : 100;
        int targetIdleHeight = customSize
            ? pixels(ToolbarSizePolicy.heightDp(sizePercent))
            : composingHeight;
        int idleHeight = customSize && idleHeader
            ? Math.max(targetIdleHeight, readingRow + pixels(ToolbarSizePolicy.buttonHeightDp(sizePercent)))
            : targetIdleHeight;
        int idleRowHeight = idleHeight - (idleHeader ? readingRow : 0);
        if (shortcutToolbar != null) ViewPolicy.setFixedHeight(shortcutToolbar, idleRowHeight);
        if (inlineAutofill != null) inlineAutofill.setHeight(idleRowHeight);
        // 「最近复制」占的是工具栏那一行的位置，同高，出现和消失时键盘不跳。
        if (recentClipRow != null) ViewPolicy.setFixedHeight(recentClipRow, idleRowHeight);
        if (candidateHeightSpacer != null) {
            boolean idleRowVisible = ViewPolicy.isVisible(shortcutToolbar)
                || ViewPolicy.isVisible(recentClipRow)
                || inlineAutofill != null && inlineAutofill.visible();
            int spacer = customSize && idleRowVisible
                ? ToolbarSizePolicy.idleSpacerPx(idleHeight, composingHeight)
                : customSize && ViewPolicy.isVisible(candidateLine)
                    ? ToolbarSizePolicy.composingSpacerPx(idleHeight, composingHeight) : 0;
            ViewPolicy.setFixedHeight(candidateHeightSpacer, spacer);
        }
        // 同一状态下「候选栏大小」为 100% 时的行高：读音行显示时只有 42 dp 的候选行，矮于 44 dp 的按钮是原有设计，100% 时按钮和图标不能因此缩小。
        int designLine = pixels(ImeToolbar.CANDIDATE_LINE_DP + extraRows * ImeToolbar.EXTRA_GLOSS_ROW_DP);
        int designRowHeight = idleHeader ? designLine : readingRow + designLine;
        imeToolbar.applyShortcutSize(sizePercent, customSize, idleRowHeight, designRowHeight);
    }

    private int readingRowHeight() {
        int design = pixels(ImeToolbar.READING_ROW_DP);
        if (preedit == null) return design;
        // 读音行自己的上下内边距（下边就是和候选行之间的 READING_GAP_DP）也算进去，否则字号大时间距被读音挤掉。
        int headerPadding = candidateHeader == null ? 0
            : candidateHeader.getPaddingTop() + candidateHeader.getPaddingBottom();
        int height = readingLabelHeight(preedit, design, headerPadding);
        // 「清除」等按钮的字号和字体留白可能比读音大；收起时仍预留同一行高，避免输入时键盘跳动。
        if (candidateHeader != null) for (int index = 0; index < candidateHeader.getChildCount(); index++) {
            View child = candidateHeader.getChildAt(index);
            if (child instanceof TextView label)
                height = Math.max(height, readingLabelHeight(label, design, headerPadding));
        }
        return height;
    }

    private int readingLabelHeight(TextView label, int design, int headerPadding) {
        Paint.FontMetricsInt metrics = label.getPaint().getFontMetricsInt();
        int top = label.getIncludeFontPadding() ? metrics.top : metrics.ascent;
        int bottom = label.getIncludeFontPadding() ? metrics.bottom : metrics.descent;
        return ReadingRowPolicy.heightPx(design, top, bottom,
            label.getCompoundPaddingTop() + label.getCompoundPaddingBottom() + headerPadding);
    }

    private SpellingEditIcon spellingEditIcon;

    /**
     * 读音行左端的笔：边长约等于读音的字号（不超过一行字高，读音行的高度不变），点按区放宽到 40 dp，小图标也点得中。
     */
    private void showSpellingEditIcon(boolean shown) {
        if (!shown) {
            preedit.setCompoundDrawablesRelative(null, null, null, null);
            preedit.setMinWidth(0);
            return;
        }
        if (spellingEditIcon == null) spellingEditIcon = new SpellingEditIcon(preedit);
        int size = Math.round(preedit.getTextSize() * 1.15f);
        spellingEditIcon.setBounds(0, 0, size, size);
        preedit.setCompoundDrawablesRelative(spellingEditIcon, null, null, null);
        preedit.setMinWidth(pixels(40));
    }

    void fail(Throwable error) {
        // 不把异常交给 Log：解析异常可能包含输入原文，固定类别足以区分下一步排查方向。
        ImeLog.e(InputFailurePolicy.logMessage(error), null);
        // 停止之前记录，沿用日志开关与当前输入框的隐私判断。
        imeDebugOverlay.recordEvent(InputFailurePolicy.eventKind(error));
        boolean recover = InputFailureRecoveryPolicy.shouldRetry(
            inputFailureRecoveryAttempts, session != 0, connection != null,
            !runtimeOptionsForSnapshot.isEmpty());
        if (recover) ++inputFailureRecoveryAttempts;
        String recoveryOptions = runtimeOptionsForSnapshot;
        long recoveryGeneration = ++engineStartGeneration;
        stop(false);
        if (recover) {
            message = "输入连接恢复中";
            render();
            scheduleEngineStartup(recoveryOptions, recoveryGeneration);
            return;
        }
        message = "输入连接失败：仅直接输入";
        render();
    }

    boolean character(int ascii) {
        return character(ascii, letterCase.usesUppercase());
    }

    boolean character(int ascii, boolean shifted) {
        finishPendingCompositionMarkerLoss();
        clearQuickDeleteUndo();
        if (session == 0) return false;
        try { return apply(NativeClient.character(session, ascii, shifted)); }
        catch (JSONException | LinkageError error) { fail(error); return true; }
    }

    /** 带按下点的一键（`msime_client_touch_character`）：只由 {@link #type(char, View)} 在 {@link ImeTouchPoints#geometryFor} 给出几何代次时调用，其余与 {@link #character(int, boolean)} 相同；这时没有 Shift。 */
    private boolean touchCharacter(int ascii, int geometryId, float x, float y) {
        finishPendingCompositionMarkerLoss();
        clearQuickDeleteUndo();
        if (session == 0) return false;
        try { return apply(NativeClient.touchCharacter(session, ascii, false, geometryId, x, y)); }
        catch (JSONException | LinkageError error) { fail(error); return true; }
    }

    /** 把 26 键双拼的键位几何注册给当前会话（{@link ImeTouchPoints#register}），返回引擎是否收下。没收下时这个代次不用，按键照常不带按下点。 */
    boolean registerKeyGeometry(String json) {
        if (session == 0) return false;
        try {
            value(NativeClient.setKeyGeometry(session, json));
            return true;
        } catch (JSONException | LinkageError error) {
            return false;
        }
    }

    /** 字母网格的一键：小写送整组首字母，十八键双拼横滑的大写标记限定精确字母，均由引擎在同一网格组字。不走 {@link #type} 的辅助码、分号和英文直出路径；引擎不收时不输入。 */
    boolean gridKey(char letter) {
        clearQuickDeleteUndo();
        if (session == 0) return false;
        try { return apply(NativeClient.gridKey(session, letter)); }
        catch (JSONException | LinkageError error) { fail(error); return true; }
    }

    private boolean helpcodeCompositionEligible() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.eligible(dedicatedEnglish,
            InputViewValuePolicy.editingText(view), InputViewValuePolicy.scheme(view, -1),
            InputViewValuePolicy.textOr(view, "local_mode", "none"));
    }

    private boolean entersHelpcode() {
        if (view == null) return false;
        return ChineseHelpcodePolicy.entersHelpcode(dedicatedEnglish, letterCase.usesUppercase(),
            InputViewValuePolicy.editingText(view), InputViewValuePolicy.scheme(view, -1),
            InputViewValuePolicy.textOr(view, "local_mode", "none"));
    }

    /** 一笔滑行抬手（{@link ImeGlideTyping}）：请求见 {@link GlideTypingPolicy#request}。引擎不收（handled=false）时什么也不输入。 */
    void glide(String request) {
        if (session == 0) return;
        try { apply(NativeClient.glide(session, request)); }
        catch (JSONException | LinkageError error) { fail(error); }
    }

    boolean command(int code) {
        finishPendingCompositionMarkerLoss();
        if (code == 0) clearQuickDeleteUndo();
        if (session == 0) return false;
        try { return apply(NativeClient.command(session, code)); }
        catch (JSONException | LinkageError error) { fail(error); return true; }
    }

    /** 全拼九键组字时的候选筛选（只留单字、首字笔顺前缀），响应照常经 apply 重画。 */
    void setNineKeyFilter(boolean singleCharacter, String strokes) {
        if (session == 0) return;
        try { apply(NativeClient.setNineKeyFilter(session, singleCharacter, strokes)); }
        catch (JSONException | LinkageError error) { fail(error); }
    }

    boolean compositionEditable() {
        return session != 0 && view != null && CompositionCaretPolicy.editable(
            InputViewValuePolicy.scheme(view, -1), InputViewValuePolicy.textOr(view, "local_mode", "none"),
            dedicatedEnglish, InputViewValuePolicy.editingText(view));
    }

    /** 放大条与输入框点击都只移动引擎光标，不在宿主重建拼写。 */
    void moveCompositionCaret(int target) {
        if (!compositionEditable()) return;
        String editing = InputViewValuePolicy.editingText(view);
        if (target < 0 || target > editing.length()) return;
        int current = InputViewValuePolicy.integer(view, "caret_position", editing.length());
        int[] moves = CompositionCaretPolicy.moves(current, target, editing.length());
        try {
            // 位置没变时也重写组字，把系统选区放回末尾，维持编辑器桥的读回契约。
            if (moves.length == 0) {
                if (!bridge.apply(sink(typingSource()), null, composingText(view, editing, true)))
                    throw new InputFailurePolicy.Failure(InputFailurePolicy.Reason.EDITOR_REJECTED);
                render();
                return;
            }
            String response = null;
            for (int step = 0; step < moves[1]; step++) response = NativeClient.command(session, moves[0]);
            apply(response);
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    /** 用户点击仍完整的组字时进入临时编辑；丢标记、选区或文本改写仍走原先的结束路径。 */
    private boolean editCompositionSelection(int start, int end, int newStart, int newEnd) {
        if (!compositionEditable() || connection == null || editorCompositionSuppressed) return false;
        String editing = InputViewValuePolicy.editingText(view);
        String composing = composingText(view, editing, true);
        int prefix = InputViewValuePolicy.textOr(view, "phrase_prefix", "").length();
        int target = CompositionCaretPolicy.editorTapTarget(composing, editing, prefix,
            start, end, newStart, newEnd);
        if (target < 0) return false;
        int offset = newStart - start;
        try {
            CharSequence before = connection.getTextBeforeCursor(offset, 0);
            CharSequence after = connection.getTextAfterCursor(composing.length() - offset, 0);
            if (before == null || after == null
                    || !before.toString().endsWith(composing.substring(0, offset))
                    || !after.toString().startsWith(composing.substring(offset))) return false;
        } catch (RuntimeException disconnected) { return false; }
        moveCompositionCaret(target);
        imeCompositionEditor.open();
        return true;
    }

    void type(char key) {
        type(key, null);
    }

    /**
     * 一个字符键。`source` 是被点击的 26 键字母层的键或微软双拼的 `;`（{@link #shuangpinLetterKey}），其余入口（底行的键、长按选项、实体键盘）为 null。这个键记下的按下点在这里取出并清掉，不论这一键走哪条路；只有走到下面送给引擎的那一步、而且是 26 键双拼的小写字母或 `;` 时才随这一键送出（{@link ImeTouchPoints#geometryFor}），辅助码的大写、英文直出和标点都照旧。无障碍点击也经过键的点击回调，但没有按下，取不到按下点。
     */
    void type(char key, View source) {
        boolean touched = imeTouchPoints.take(source);
        finishPendingCompositionMarkerLoss();
        clearQuickDeleteUndo();
        dropDeferredToolbarTaps();
        if (connection == null) return;
        if (directEnglishActive()) {
            char output = letterCase.usesUppercase() ? Character.toUpperCase(key) : key;
            if (TextPolicy.isAsciiLetter(output)) {
                commitEnglishLiteral(output);
                if (letterCase.consumeLetter()) {
                    finishTemporaryEnglishShift();
                    imeLetterRows.refreshLetterCase();
                    render();
                }
                return;
            }
        }
        if (dedicatedEnglish && !TextPolicy.isAsciiLetter(key)) {
            commitEnglishLiteral(key);
            return;
        }
        if (koreanSchemeActive() && TextPolicy.isAsciiLetter(key)) {
            // Shift picks the double consonant or ㅒ ㅖ; the other keys send their lowercase letter, which types the same jamo.
            char input = KoreanKeyboardLayout.input(key, letterCase.usesUppercase());
            if (!character(input, TextPolicy.isUpperCase(input))) commitText(String.valueOf(input));
            if (letterCase.consumeLetter()) {
                imeLetterRows.refreshLetterCase();
                render();
            }
            return;
        }
        if (entersHelpcode() && TextPolicy.isAsciiLetter(key)) {
            character(Character.toUpperCase(key), true);
            if (letterCase.consumeLetter()) {
                imeLetterRows.refreshLetterCase();
                render();
            }
            return;
        }
        char output = letterCase.usesUppercase() ? Character.toUpperCase(key) : key;
        // 微软双拼的 ; 是韵母 ing：组字中交给 character()，引擎按双拼键处理；空闲时才是标点。#2004 加 ing 键时 type 走的都是 character()，b73a611a8 把 ASCII 标点改走 punctuation() 以后，组字中的 ; 被当成分号：打 b; 上屏「把；」，ing 韵母全打不出来。
        boolean microsoftFinal = MicrosoftShuangpinKeyPolicy.routesAsFinal(output, hasEngineComposition(),
            dedicatedEnglish, selectedScheme,
            view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"));
        boolean punctuationKey = SmartPunctuationContext.isAsciiPunctuation(output) && !microsoftFinal;
        int geometryId = touched && !punctuationKey
            ? imeTouchPoints.geometryFor(output, letterCase.usesUppercase()) : 0;
        boolean handled = punctuationKey ? punctuation(output)
            : geometryId != 0 ? touchCharacter(output, geometryId, imeTouchPoints.x(), imeTouchPoints.y())
            : character(output);
        if (!handled) {
            // 引擎不收的标点和数字（数字行的 0、没有候选时的 1–9）是一次自动上屏，先把组合按首选结束掉。读音是真正的组字区，直接上屏会替换掉拼音，而不是跟在拼音后面。
            if (DeclinedKeyPolicy.finishesComposition(punctuationKey, output, hasEngineComposition())) {
                command(FINISH_COMPOSITION_COMMAND);
            }
            commitText(fullWidthOutput(String.valueOf(output)));
        }
        if (letterCase.consumeLetter()) {
            imeLetterRows.refreshLetterCase();
            render();
        }
    }

    boolean punctuation(int ascii) {
        finishPendingCompositionMarkerLoss();
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
            // 键盘自己补上的后半个已经在光标右边，再按它的键就跨过去而不是再写一个。只在没有组字时：组字中按下的键是拿这个标点结束组字。
            if (pairedPunctuation && connection != null && !hasEngineComposition()) {
                String closing = pairedClosingAhead(ascii);
                if (closing != null) {
                    clearSmartPunctuationSnapshots();
                    stepOverClosingMark(closing);
                    return true;
                }
            }
            String response = reopenPairedQuote(
                NativeClient.punctuationWithContext(session, ascii, preceding), ascii);
            boolean handled = apply(response);
            PairedPunctuationPolicy.Completion completion = pairedCompletion(response);
            if (completion != null && commitClosingMark(completion.closing(), typingSource())) {
                // 补的是书名号时告诉 Engine 这一层已经闭合，否则下一次 < 会被当成嵌套的〈。
                if (completion.opening() == '<') balanceBookTitleAfterAutoClose();
            } else {
                completion = null;
            }
            armSmartPunctuation(ascii, response, completion != null);
            return handled;
        }
        catch (JSONException | LinkageError error) { fail(error); return true; }
    }

    /** 按下 `ascii` 是否该跨过光标右边那个补好的后半个；是的话返回它，并从记录里弹出。 */
    private String pairedClosingAhead(int ascii) {
        if (pairedPunctuationStack.isEmpty()) return null;
        // 跨过是删掉光标后的字再写回去，读不出光标后的文字时不能确定删的就是那个后半个，宁可放弃这份记录照常输入。
        CharSequence following = textAfterCursor(1);
        if (following == null) {
            pairedPunctuationStack.clear();
            return null;
        }
        return pairedPunctuationStack.stepOver(ascii, currentDocumentIdentifier, following);
    }

    /** 符号面板里轻点后半个 `symbol` 时，跨过光标右边补好的同一个后半个而不是再写一个；跨过了返回 true。规则同键盘的 {@link #pairedClosingAhead}。 */
    boolean stepOverPairedSymbol(String symbol) {
        if (!pairedPunctuation || connection == null || pairedPunctuationStack.isEmpty()
                || symbol == null || hasEngineComposition()) return false;
        CharSequence following = textAfterCursor(symbol.length());
        if (following == null) {
            pairedPunctuationStack.clear();
            return false;
        }
        String closing = pairedPunctuationStack.stepOverSymbol(symbol, currentDocumentIdentifier, following);
        if (closing == null) return false;
        clearSmartPunctuationSnapshots();
        stepOverClosingMark(closing);
        return true;
    }

    /** 打开成对补全时，把 Engine 这次上屏末尾的后引号改回前引号，见 {@link PairedPunctuationPolicy#reopenQuote}。 */
    private String reopenPairedQuote(String response, int ascii) throws JSONException {
        if (!pairedPunctuation || (ascii != '"' && ascii != '\'')) return response;
        JSONObject envelope = new JSONObject(response);
        JSONObject result = envelope.optJSONObject("value");
        if (result == null || result.isNull("commit")) return response;
        String commit = result.optString("commit", "");
        String reopened = PairedPunctuationPolicy.reopenQuote(commit, ascii, true);
        if (reopened.equals(commit)) return response;
        result.put("commit", reopened);
        return envelope.toString();
    }

    private PairedPunctuationPolicy.Completion pairedCompletion(String response) throws JSONException {
        if (!pairedPunctuation || connection == null) return null;
        JSONObject result = value(response);
        return result.isNull("commit") ? null
            : PairedPunctuationPolicy.completion(result.optString("commit", ""), true);
    }

    private void balanceBookTitleAfterAutoClose() {
        try {
            value(NativeClient.balancePairedPunctuationAfterAutoClose(session, '<'));
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 平衡失败只影响下一次 < 是《还是〈，不影响已经上屏的文字。
        }
    }

    private void clearSmartPunctuationSnapshots() {
        clearQuickDeleteUndo();
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

    void commitEnglishLiteral(int value) {
        if (connection == null || value < 32 || value > 126) return;
        if (englishSuggestionsActive() && " ,.!?:;".indexOf(value) >= 0
                && commitEnglishBoundary((char) value)) return;
        if (englishNineKeyActive() && session != 0) command(2);
        commitText(fullWidthOutput(String.valueOf((char) value)));
        if (directEnglishActive()) {
            // 上屏之后不再写编辑器：候选条、清除键和它们要求的那次重画共用一次读取，只画一次。
            beginEditorEvent();
            try {
                refreshEnglishSuggestions();
                refreshEnglishClearButton();
            } finally {
                endEditorEvent();
            }
        }
    }

    private boolean commitEnglishBoundary(char boundary) {
        String typed = englishWordBeforeCursor();
        if (typed.isEmpty()) return false;
        String candidate = typed;
        if (!typed.equals(englishCorrectionRejectedPrefix) && !candidateGlossResources.isEmpty()) {
            try {
                EnglishSuggestionModel.Result result = EnglishSuggestionModel.decode(
                    NativeClient.englishCorrections(typed, candidateGlossResources));
                if (result != null && typed.equals(result.prefix()) && result.items().size() == 2
                        && typed.equals(result.items().get(0)))
                    candidate = result.items().get(1);
            } catch (Exception | LinkageError ignored) {
                // 词库查询失败时保留已经输入的文字，边界键仍然照常输入。
            }
        }
        String suffix = boundary == ' ' ? " " : String.valueOf(boundary) + " ";
        EnglishSuggestionPolicy.Replacement replacement = EnglishSuggestionPolicy.replacement(
            typed, candidate, !typed.isEmpty() && TextPolicy.isUpperCase(typed.charAt(0)));
        InputConnection target = connection;
        if (target == null || !typed.equals(englishWordBeforeCursor())
                || !InputConnectionGuard.beginBatchEdit(target)) return false;
        boolean committed;
        try {
            if (replacement != null && !deleteBeforeCursor(replacement.deleteCount())) return false;
            String word = replacement == null ? "" : fullWidthOutput(replacement.insert());
            committed = commitText(word + suffix);
        } finally {
            InputConnectionGuard.endBatchEdit(target);
        }
        clearEnglishSuggestions();
        render();
        return committed;
    }

    void space() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        // Space on a composing Zhuyin conversion is tone 1 or opens its list, as on a hardware keyboard; the commit command below would end the conversion and drop the pending syllable.
        if (session != 0 && view != null && ZhuyinInputPolicy.spaceIsEngineKey(zhuyinSchemeActive(),
                InputViewValuePolicy.textOr(view, "spelling_symbols", "")) && character(' ', false)) return;
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
            String editingText = InputViewValuePolicy.editingText(view);
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

    boolean japaneseSchemeActive() {
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
            InputViewValuePolicy.textOr(view, "local_mode", "none"), entries == null ? 0 : entries.length());
    }

    private boolean zhuyinSchemeActive() {
        return view != null
            && ZhuyinInputPolicy.active(InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    /** A Stroke composition whose glyphs the editor holds as its composing region (apply marks View.reading for Stroke). */
    private boolean strokeCompositionMarked() {
        return view != null && !InputViewValuePolicy.editingText(view).isEmpty()
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
            InputViewValuePolicy.textOr(view, "local_mode", "none"), entries == null ? 0 : entries.length());
    }

    /** Whether the Zhuyin open-list command applies now: a conversion is composing, with or without its list open. */
    private boolean zhuyinOpensList() {
        return view != null && ZhuyinInputPolicy.opensList(zhuyinSchemeActive(),
            InputViewValuePolicy.textOr(view, "local_mode", "none"), InputViewValuePolicy.editingText(view));
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
            InputViewValuePolicy.textOr(view, "local_mode", "none"), InputViewValuePolicy.editingText(view));
    }

    private boolean japaneseNineKeyActive() {
        return displayedTouchLayout(view) == JAPANESE_NINE_KEY_LAYOUT;
    }

    String spaceKeyTitle() {
        return japaneseSchemeActive()
            ? JapaneseNineKeyActions.spaceTitle(view != null
                && !InputViewValuePolicy.editingText(view).isEmpty()) : "空格";
    }

    String spaceKeyDescription() {
        return japaneseSchemeActive()
            ? spaceKeyTitle() + "；左右滑动移动光标" : SPACE_CURSOR_DESCRIPTION;
    }

    int splitKeyboardGapPercent() { return splitKeyboardGapPercent; }

    /** 分离主开关开启、大屏，且方向符合偏好时，26 键一族或韩文键盘分离（{@link SplitKeyboardPolicy#drawn}）；手机横屏另由本机开关和分离后的键宽决定（{@link SplitKeyboardPolicy#phoneDrawn}，#10440）。每次按当前窗口配置重算。 */
    boolean splitKeyboardDrawn() {
        // 浮动键盘是一块窄面板，不分成两半。
        if (floatingDrawn() || imeLetterRows.numberPadDrawn()) return false;
        Configuration configuration = getResources().getConfiguration();
        boolean landscape = configuration.orientation == Configuration.ORIENTATION_LANDSCAPE;
        int touchLayout = displayedTouchLayout(view);
        if (SplitKeyboardPolicy.drawn(splitKeyboardEnabled, configuration.smallestScreenWidthDp, landscape,
                touchLayout, splitKeyboardPortrait, splitKeyboardThresholdDp)) return true;
        return splitKeyboardPhoneLandscape && SplitKeyboardPolicy.phoneDrawn(true,
            configuration.smallestScreenWidthDp, landscape, touchLayout, splitKeyboardThresholdDp,
            keyboardContentWidthDp(configuration), touchKeySpacingTenths, splitKeyboardGapPercent);
    }

    /** 键行实际能用的宽度（dp）：键盘列在当前配置下布局过就用它扣掉左右系统栏内边距后的宽度，分屏、侧边导航栏都已算在里面；刚旋转、还没重新布局时先按 `screenWidthDp` 估，布局完由 {@link #onKeyboardContentLayout} 纠正。 */
    private int keyboardContentWidthDp(Configuration configuration) {
        if (keyboardContentWidthPx > 0 && keyboardContentScreenWidthDp == configuration.screenWidthDp)
            return Math.round(DimensionPolicy.fromPixels(this, (float) keyboardContentWidthPx));
        return configuration.screenWidthDp;
    }

    /** 键盘列布局完记下键行可用宽度；手机横屏分离按这个宽度判断，宽度变了而键行还是按旧判断建的（旋转、分屏、导航栏换边）就再画一次。 */
    private void onKeyboardContentLayout(View column) {
        int width = column.getWidth() - column.getPaddingLeft() - column.getPaddingRight();
        int screenWidthDp = getResources().getConfiguration().screenWidthDp;
        if (width == keyboardContentWidthPx && screenWidthDp == keyboardContentScreenWidthDp) return;
        keyboardContentWidthPx = width;
        keyboardContentScreenWidthDp = screenWidthDp;
        // 在布局回调里不重建视图树，排到下一帧。
        if (splitKeyboardPhoneLandscape && imeLetterRows.splitStale()) column.post(this::render);
    }

    /** 现在字母上方要不要画数字行（#6022）：在字母层、窗口够高，共享偏好打开时画在 26 键一族、韩文、14 键和 18 键上，密码框和终端的自动数字行只画在 26 键一族和韩文上（{@link KeyboardLayout#drawsNumberRow(boolean, boolean, KeyboardLayout.Layer, int, int)}）。 */
    boolean numberRowDrawn() {
        return KeyboardLayout.drawsNumberRow(numberRowEnabled, automaticNumberRow.active(), keyboardLayer,
            displayedTouchLayout(view), getResources().getConfiguration().screenHeightDp);
    }

    /** 数字、符号页保留源字母数字行占用的一排。 */
    boolean symbolExtraRowDrawn() {
        return KeyboardLayout.drawsSymbolExtraRow(numberRowEnabled, automaticNumberRow.active(), keyboardLayer,
            displayedTouchLayout(view), getResources().getConfiguration().screenHeightDp);
    }

    /** 分词键只服务于已就绪的中文全拼组合，密码框和本地直输保留 Shift。用户在共享偏好 `touch_quanpin_shift` 打开「保留 Shift」后也不替换。 */
    boolean usesQuanpinSeparator() {
        return !quanpinShiftEnabled && KeyboardLayout.usesQuanpinSeparator(selectedScheme == KeyboardScheme.QUANPIN,
            session != 0 && view != null && !dedicatedEnglish && !EditorPolicy.password(editorInputType),
            keyboardLayer, displayedTouchLayout(view),
            view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"));
    }

    /** 输入方式和功能面板此刻是不是手机横屏（{@link PagedTileGrid#pickerGrid}）：手机横屏两行，其余三行；行高、行距和放不下三行时退成两行，由网格按面板量到的高度定。 */
    boolean pickerPhoneLandscape() {
        Configuration configuration = getResources().getConfiguration();
        return configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
            && !KeyboardFormFactorPolicy.expanded(configuration.smallestScreenWidthDp);
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

    // 双拼键上的提示同理（#6997）：view 没到时没有方案可查，提示全空、字母居中，会话一到提示和 11dp 下边距同时出现，整排字母往上一跳。记下上次的双拼方案，和布局记在同一个 SharedPreferences 里。
    private static final String SHUANGPIN_PROFILE_HINT_KEY = "shuangpin_profile";
    private String shuangpinProfileHint;

    private String shuangpinProfileHint() {
        if (shuangpinProfileHint == null) {
            shuangpinProfileHint = getSharedPreferences(TOUCH_LAYOUT_HINT_PREFERENCES, MODE_PRIVATE)
                .getString(SHUANGPIN_PROFILE_HINT_KEY, "");
        }
        return shuangpinProfileHint;
    }

    private void rememberShuangpinProfile(String profile) {
        if (profile.equals(shuangpinProfileHint())) return;
        shuangpinProfileHint = profile;
        getSharedPreferences(TOUCH_LAYOUT_HINT_PREFERENCES, MODE_PRIVATE).edit()
            .putString(SHUANGPIN_PROFILE_HINT_KEY, profile).apply();
    }

    /** scheduleEngineStartup 排上、还没建好也没作废的那次会话启动所属的代次；没有时为 -1。 */
    private long pendingEngineStartup = -1;
    /** 只在配置尚不存在的首次准备期间保留；换编辑器与失败沿 stop 清除。 */
    private PendingRuntimeBootstrap pendingRuntimeBootstrap;
    private record PendingRuntimeBootstrap(EditorInfo info, boolean restarting, boolean newDocument) {}
    private long dictionaryStartupRetryDeadline;
    /** onStartInput 推迟到 onStartInputView 的那次会话启动所属的代次（{@link InputViewRefreshPolicy#defersEngineStartup}）；没有时为 -1，stop 时清掉。与 {@link #pendingEngineStartup} 分开记：后者为 -1 表示启动失败或已停下、显示时不再重试，推迟的启动不能混进这个语义。 */
    private long deferredEngineStartup = -1;
    /** 推迟引擎启动的那次 onStartInput 也没有更新键行、没有重画（键盘没显示），等 onStartInputView 补上。 */
    private boolean editorKeyRowsPending;
    /** scheduleEngineStartup 排上那次启动时这个输入框是否不学习；建会话时记到 {@link #sessionLearningSuppressed}。 */
    private boolean startupLearningSuppressed;
    /** 现在这个会话建立时是否按不学习建的。引擎的 learning 只在建会话时定，同一输入框重启连接时只有和现在的要求一致才留着会话。 */
    private boolean sessionLearningSuppressed;

    /** 这次弹出的键盘还在等第一帧交出去时，从 onStartInputView 开始等的时刻（uptime 毫秒）；没有在等时为 -1（#9916，{@link InputViewRefreshPolicy#armsFirstFrameWait}）。 */
    private long firstFrameArmedAt = -1;
    /** 工作线程交回来、等第一帧交出去再在主线程上做的建会话；没有时为 null。它自己核对启动代次，换输入框、收起和销毁换代后放行时直接作废，和原来 post 回主线程的那一步一样。 */
    private Runnable sessionStartAfterFirstFrame;
    /** 第一帧迟迟不来时的兜底：到 {@link InputViewRefreshPolicy#FIRST_FRAME_WAIT_MILLIS} 照常建会话。 */
    private final Runnable firstFrameWaitTimeout = this::runSessionStartAfterFirstFrame;

    /**
     * 键盘从收起到弹出时，开始等这次弹出的第一帧（#9916）：在键盘视图的 ViewTreeObserver 上挂一个只触发一次的画帧前监听，到这一帧时再登记帧提交回调（{@link #awaitFirstFrameCommit}），等这一帧真正交出去后记下 onStartInputView 到这时的耗时，并放行这段时间里工作线程交回来的建会话。窗口本来就显示着时不等。
     */
    private void armFirstFrameWait() {
        View root = keyboardRoot;
        if (!InputViewRefreshPolicy.armsFirstFrameWait(imeWindowVisible, root != null)) return;
        long armedAt = SystemClock.uptimeMillis();
        firstFrameArmedAt = armedAt;
        // 第一次弹出时视图还没挂到窗口上，这里拿到的是临时的观察者，挂上窗口时监听会并进窗口自己的那个；所以摘监听时重新取一次。
        root.getViewTreeObserver().addOnPreDrawListener(new android.view.ViewTreeObserver.OnPreDrawListener() {
            @Override public boolean onPreDraw() {
                android.view.ViewTreeObserver observer = root.getViewTreeObserver();
                if (observer.isAlive()) observer.removeOnPreDrawListener(this);
                awaitFirstFrameCommit(observer, armedAt);
                return true;
            }
        });
    }

    /**
     * 这次弹出的第一帧开始画了，但画帧前离窗口亮出来还早：RenderThread 还要画完、交换缓冲，系统收到画完的报告（finishDrawing）才把窗口亮出来。API 30 上这份报告是 RenderThread 画完后用 postAtFrontOfQueue 排回主线程的，在画帧前监听里 post 出去的建会话会在这一帧遍历结束后马上轮到，往往抢在报告前面，整段建会话就又挡在窗口亮出来之前。所以在这一帧里登记帧提交回调：它和画完的报告在同一个 postAtFrontOfQueue 里、排在报告后面（API 29 不需要报告时直接在 RenderThread 上回调），回调里 post 回主线程再放行。API 28 没有帧提交回调，退一步等下一帧的 Choreographer 回调，那时这一帧多半已经交出去。哪种都由 {@link InputViewRefreshPolicy#FIRST_FRAME_WAIT_MILLIS} 的兜底封顶：这一帧没有画出东西、或者不走硬件渲染时，帧提交回调不会来。又弹出了一次（重新开始等）或已经收起时，这一帧不属于它。
     */
    private void awaitFirstFrameCommit(android.view.ViewTreeObserver observer, long armedAt) {
        if (firstFrameArmedAt != armedAt) return;
        Runnable committed = () -> firstFrameCommitted(armedAt);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && observer.isAlive()) {
            observer.registerFrameCommitCallback(() -> main.post(committed));
        } else {
            android.view.Choreographer.getInstance().postFrameCallback(frameTimeNanos -> main.post(committed));
        }
    }

    /** 这次弹出的第一帧已经交出去：记下耗时，马上放行等着的建会话。又弹出了一次或已经收起时，这一帧不属于它。兜底时间到了先放行了建会话时，晚到的这一帧照样记耗时（这才是第一帧真正的耗时），那时已经没有排着的建会话可放行。 */
    private void firstFrameCommitted(long armedAt) {
        if (firstFrameArmedAt != armedAt) return;
        firstFrameArmedAt = -1;
        imeDebugOverlay.recordPerf(DiagnosticEventKind.KEYBOARD_FIRST_DRAW, SystemClock.uptimeMillis() - armedAt);
        runSessionStartAfterFirstFrame();
    }

    /** 工作线程准备好建会话：键盘这次弹出的第一帧还没交出去时排到它之后（兜底到 {@link InputViewRefreshPolicy#FIRST_FRAME_WAIT_MILLIS}），否则马上建。 */
    private void startEngineSessionAfterFirstFrame(Runnable build) {
        long now = SystemClock.uptimeMillis();
        if (!InputViewRefreshPolicy.waitsForFirstFrame(firstFrameArmedAt, now)) {
            build.run();
            return;
        }
        sessionStartAfterFirstFrame = build;
        main.removeCallbacks(firstFrameWaitTimeout);
        main.postDelayed(firstFrameWaitTimeout, InputViewRefreshPolicy.firstFrameWaitRemaining(firstFrameArmedAt, now));
    }

    private void runSessionStartAfterFirstFrame() {
        main.removeCallbacks(firstFrameWaitTimeout);
        Runnable build = sessionStartAfterFirstFrame;
        sessionStartAfterFirstFrame = null;
        if (build != null) build.run();
    }

    /** 有一次会话启动在路上：排上之后没有换代（换输入框、收起、销毁都会换代），也没有被 stop 取消（建会话失败、输入连接失败都经过 stop）。 */
    private boolean engineStartupPending() {
        return session == 0 && pendingEngineStartup >= 0 && pendingEngineStartup == engineStartGeneration;
    }

    /** 工具栏入口与「中」键看的会话状态（{@link ToolbarReadinessPolicy}）：会话在建时表情、常用语和「中」键照常可用，皮肤和输入方式的点按推迟到会话就绪（#10086）。 */
    ToolbarReadinessPolicy.Session toolbarSession() {
        return ToolbarReadinessPolicy.session(session != 0, engineStartupPending());
    }

    /** 会话还在建时点下的皮肤或输入方式（null 为没有）：会话和偏好就绪后打开。换输入框（stop）、收起键盘或打开别的面板（closeToolbarPanels，以及不经它的功能面板 ImePanels.showFeedbackMenu 和表情面板 ImePanels.showEmojiPicker）、再点一次同一个按钮（deferrablePanelToggle）、开始打字或删字（dropDeferredToolbarTaps）时作废。 */
    private ToolbarOrderPolicy.Item pendingToolbarPanel;
    /** 会话还在建时点下、要轮换到其他语言键盘的那一下「中」键：会话和偏好就绪后照常按一次。换输入框（stop）、收起键盘（finishInputViewPresentation）、开始打字或删字（dropDeferredToolbarTaps）时作废，会话建好后再点一下「中」键时也作废（这一下照常交给引擎，不能等偏好读到后再补按一次）；已记下一下时再点不再记，见 ToolbarReadinessPolicy.languageTap。 */
    private boolean pendingLanguageTap;
    /** 这一代会话启动排上的时刻（uptime 毫秒），会话建好时算出等了多久写进性能记录；-1 为没有在等。 */
    private long engineStartupRequestedAt = -1;

    /**
     * 皮肤、输入方式的点按：会话还在建时记下这一下并返回 true，调用方不再往下走；会话和偏好就绪后由 {@link #runDeferredToolbarTaps} 打开。会话已建好或这个输入框根本没有会话时返回 false，照原来的判断处理。
     */
    boolean deferUntilSessionReady(ToolbarOrderPolicy.Item panel) {
        if (ToolbarReadinessPolicy.preferencePanelTap(toolbarSession(), preferencesSnapshot != null)
                != ToolbarReadinessPolicy.PanelTap.DEFER) return false;
        pendingToolbarPanel = panel;
        render();
        return true;
    }

    /** 皮肤、输入方式按钮的点按：推迟中的按钮画成选中，再点一次就是取消，和面板开着时再点一次回到键盘一致；其余情况照 {@link ImeToolbar#panelToggle} 处理。 */
    private Runnable deferrablePanelToggle(ToolbarOrderPolicy.Item panel, Runnable toggle) {
        return () -> {
            if (pendingToolbarPanel == panel) {
                pendingToolbarPanel = null;
                render();
                return;
            }
            toggle.run();
        };
    }

    /** 推迟着皮肤或输入方式时用户打开了不经 closeToolbarPanels 的面板：推迟的面板不再在会话建好后自己弹出来。 */
    void dropDeferredToolbarPanel() {
        if (pendingToolbarPanel == null) return;
        pendingToolbarPanel = null;
        render();
    }

    /** 推迟着皮肤、输入方式或「中」键时用户开始打字或删字：面板不再在会话建好后弹出来打断输入，键盘也不在一个词打到一半时自己换语言。 */
    private void dropDeferredToolbarTaps() {
        if (pendingToolbarPanel == null && !pendingLanguageTap) return;
        pendingToolbarPanel = null;
        pendingLanguageTap = false;
        render();
    }

    /**
     * 会话建好时写三条性能记录（只在用户打开「记录输入日志」且隐私判断允许时写，见 {@link ImeDebugOverlay#recordPerf}）：主线程上 `NativeClient.create` 的耗时，主线程上从开始建会话到按实时偏好画完的整段（#9916，这一段里点按都在排队），以及从这一代启动排上到会话可用的整段等待。只有毫秒数，不带输入内容和路径。下次有人反馈「键盘弹出来要等」时，从诊断包就能看出慢在建会话本身、应用偏好和重画，还是前面的词库同步和偏好读取（`engine_startup_*`）。
     */
    private void recordEngineSessionTiming(long createMillis, long mainThreadMillis) {
        imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_SESSION_CREATE, createMillis);
        imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_SESSION_MAIN, mainThreadMillis);
        if (engineStartupRequestedAt >= 0) {
            imeDebugOverlay.recordPerf(DiagnosticEventKind.ENGINE_SESSION_READY,
                SystemClock.uptimeMillis() - engineStartupRequestedAt);
        }
        engineStartupRequestedAt = -1;
    }

    /** 会话建好、偏好读到后，执行会话在建时推迟的那一下。用户在等的时候已经打开了别的面板，推迟的面板就不再弹出来盖住它。 */
    private void runDeferredToolbarTaps() {
        if (!ToolbarReadinessPolicy.deferredTapReady(toolbarSession(), preferencesSnapshot != null)) return;
        ToolbarOrderPolicy.Item panel = pendingToolbarPanel;
        boolean language = pendingLanguageTap;
        pendingToolbarPanel = null;
        pendingLanguageTap = false;
        if (language) languageKeyTapped();
        if (panel == null) return;
        if (!anyToolbarPanelOpen() && connection != null) {
            if (panel == ToolbarOrderPolicy.Item.SKIN) imePanels.showSkinMenu(skinButton);
            else if (panel == ToolbarOrderPolicy.Item.SCHEME) imePanels.showSchemePicker();
        }
        render();
    }

    /** 会话还在建时点「中」键：只翻转宿主的中英状态并照常记住；会话建好时 startEngineSession 按 dedicatedEnglish 交给引擎。英文直输本来就不经引擎，翻到英文后马上就能打字。 */
    private void toggleInputLanguageBeforeSession() {
        temporaryEnglishShift = false;
        dedicatedEnglish = !dedicatedEnglish;
        if (inputModeStore != null) inputModeStore.remember(imeModeScope, currentEditorPackage, dedicatedEnglish);
        keyboardLayer = KeyboardLayout.Layer.LETTERS;
        letterCase.reset();
        imeLetterRows.refreshLetterCase();
        clearEnglishSuggestions();
        // 切回中文时 updateAutomaticCapitalization 不重画，这里统一画一次。
        updateAutomaticCapitalization(false);
        render();
    }

    boolean sendsChinesePunctuation() {
        return view != null && ChineseSymbolFaces.shouldUseChineseFaces(dedicatedEnglish,
            InputViewValuePolicy.scheme(view, -1), InputViewValuePolicy.textOr(view, "local_mode", "none"), chinesePunctuation);
    }

    /** 数字九宫格左列符号和右列 ！ 此刻的写法（{@link NumberPadPunctuation#style}）：中文标点生效时是中文，日文方案是日文，其余半角 ASCII。 */
    NumberPadPunctuation.Style numberPadPunctuation() {
        return NumberPadPunctuation.style(chinesePunctuation, dedicatedEnglish,
            view == null ? -1 : InputViewValuePolicy.scheme(view, -1),
            view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"));
    }

    /** 当前 View 报告的引号与方头括号样式；画键面、提示和 123 层时用它，不自己解析偏好。 */
    ChineseSymbolFaces.Marks punctuationMarks() {
        return InputViewValuePolicy.punctuationMarks(view);
    }

    java.util.List<QuickPunctuationPolicy.Entry> quickPunctuationEntries() {
        return QuickPunctuationPolicy.entries(dedicatedEnglish,
            view == null ? -1 : InputViewValuePolicy.scheme(view, -1),
            view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"),
            uriPeriodPrimary());
    }

    private boolean quickPunctuationVisible() {
        int layout = imeLetterRows.drawnTouchLayout();
        if (keyboardLayer != KeyboardLayout.Layer.LETTERS) return false;
        // 九键外框的标点在侧栏里，只有「底栏带标点」样式在底行放逗号。
        if (layout == QUANPIN_NINE_KEY_LAYOUT || layout == KeyboardLayout.STROKE_LAYOUT)
            return bottomRowStyle(layout) == TouchKeyStyle.Row.COMMA_PERIOD;
        return layout != JAPANESE_NINE_KEY_LAYOUT && layout != KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
    }

    private void insertQuickPunctuation() {
        java.util.List<QuickPunctuationPolicy.Entry> entries = quickPunctuationEntries();
        if (!entries.isEmpty()) type(entries.get(0).input());
    }

    /** 26 键字母层的逗号长按输入句号；「逗号句号在空格两侧」有独立句号键，逗号长按改回常用标点菜单。 */
    private boolean commaSecondaryPeriod() {
        int layout = imeLetterRows.drawnTouchLayout();
        return keyboardLayer == KeyboardLayout.Layer.LETTERS
            && KeyboardActionRow.usesCommaSecondaryPeriod(layout)
            && bottomRowStyle(layout) != TouchKeyStyle.Row.COMMA_PERIOD;
    }

    /** 这块键面字母层底行此刻用的样式（{@link TouchKeyStyle.Styles#forLayout}）；数字字段和密码框保留标准底行。 */
    TouchKeyStyle.Row bottomRowStyle(int layout) {
        if (directNumberPad() || EditorPolicy.password(editorInputType)) return TouchKeyStyle.Row.STANDARD;
        return touchKeyStyles.forLayout(layout, view == null ? -1 : InputViewValuePolicy.scheme(view, -1), dedicatedEnglish);
    }

    /** 26 键的长空格样式：底行没有符号键，符号入口移到工具栏，数字层用带逗号的 123 层。 */
    boolean longSpaceLayoutActive() {
        return longSpaceLayoutActive(displayedTouchLayout(view));
    }

    boolean longSpaceLayoutActive(int layout) {
        return KeyboardActionRow.usesCommaSecondaryPeriod(layout)
            && bottomRowStyle(layout) == TouchKeyStyle.Row.LONG_SPACE;
    }

    /** 长空格样式让工具栏那一行一直留着（{@link TouchKeyStyle.Styles#keepsToolbarRow}）：看这块键盘中文时的键面，不看此刻的中/英，切换时键盘高度不跳。英文模式下 {@link #displayedTouchLayout} 一律是 26 键，中文键面取最后一次中文时记下的那块（{@link #touchLayoutHint}，只在中文时更新）。数字字段和密码框保留标准底行，不留。 */
    boolean longSpaceToolbarRowKept() {
        if (directNumberPad() || EditorPolicy.password(editorInputType)) return false;
        int chineseLayout = dedicatedEnglish ? touchLayoutHint() : displayedTouchLayout(view);
        return touchKeyStyles.keepsToolbarRow(chineseLayout, view == null ? -1 : InputViewValuePolicy.scheme(view, -1));
    }

    /** 底行没有句号键的长空格样式（26 键与 14/18 键）：逗号下滑输入句号。 */
    boolean commaSwipesPeriod(int layout) {
        return keyboardLayer == KeyboardLayout.Layer.LETTERS
            && bottomRowStyle(layout) == TouchKeyStyle.Row.LONG_SPACE;
    }

    private boolean uriPeriodPrimary() {
        return QuickPunctuationPolicy.uriPeriodPrimary(dedicatedEnglish, editorInputType,
            commaSecondaryPeriod());
    }

    /** 手写底行在场：逗号长按输入问号、句号长按输入叹号，接替原来书写区左列的 ？ ！。 */
    boolean handwritingMarksShown() {
        return keyboardLayer == KeyboardLayout.Layer.LETTERS
            && KeyboardActionRow.usesHandwritingMarks(imeLetterRows.drawnTouchLayout());
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
        imeBottomRow.updateCommaSwipe();
        java.util.List<QuickPunctuationPolicy.Entry> entries = quickPunctuationEntries();
        boolean visible = quickPunctuationVisible() && !entries.isEmpty();
        ViewPolicy.setVisible(quickPunctuationButton, visible);
        if (!visible) return;
        String face = entries.get(0).face();
        // 与句号键一样按墨迹居中放大画：全角「，」原样居中时只剩键底一个小点。
        CenteredGlyphSpan.apply(quickPunctuationButton, face, 1.3f);
        boolean question = handwritingMarksShown();
        if (quickPunctuationButton instanceof KeyHintButton hintButton) {
            hintButton.setCornerHint(uriPeriodPrimary() ? "," : commaSecondaryPeriod()
                ? ChineseSymbolFaces.face(".", sendsChinesePunctuation(), punctuationMarks())
                : question ? ChineseSymbolFaces.face("?", sendsChinesePunctuation(), punctuationMarks()) : "");
        }
        quickPunctuationButton.setContentDescription("常用标点：" + face
            + (uriPeriodPrimary() ? "；长按输入逗号" : commaSecondaryPeriod() ? "；长按输入句号"
                : question ? "；长按输入问号" : "；长按选择常用标点"));
    }

    private void updateSymbolKeyFaces() {
        boolean chineseMode = sendsChinesePunctuation();
        for (int index = 0; index < symbolKeyButtons.size(); index++) {
            String face = ChineseSymbolFaces.face(symbolKeyInputs.get(index), chineseMode,
                punctuationMarks());
            Button button = symbolKeyButtons.get(index);
            button.setText(face);
            button.setContentDescription("按键 " + face);
        }
    }

    /** 26 键字母右上角的提示键面跟中文标点走（#7069）：设置页改「中文标点」、切换中英文、会话起来后 view 到达，这几种情形都不重建字母层，j k l v b 的键面在这里就地换成中文或 ASCII。工具面板和 Ctrl + . 走 toggleChinesePunctuation()，那里本来就重建键行，建键时已按当时状态取键面。键面相同时 setCornerHint 直接返回。 */
    private void updateLetterCornerHints() {
        boolean chinese = sendsChinesePunctuation();
        for (int index = 0; index < shuangpinKeyButtons.size(); index++) {
            String input = shuangpinKeyInputs.get(index);
            if (";".equals(input)) continue;
            shuangpinKeyButtons.get(index).setCornerHint(LetterHintTable.face(input, dedicatedEnglish, chinese,
                punctuationMarks()));
        }
    }

    private void updateShuangpinKeyHints() {
        int scheme;
        String profile;
        String localMode;
        if (view != null) {
            scheme = InputViewValuePolicy.scheme(view, -1);
            profile = InputViewValuePolicy.textOr(view, "shuangpin_profile", "");
            localMode = InputViewValuePolicy.textOr(view, "local_mode", "none");
            rememberShuangpinProfile(ShuangpinKeyHintPolicy.rememberedProfile(scheme, profile));
        } else {
            // view 还没到时按上次的双拼方案画，和 touchLayoutHint 同理；会话到达后方案不同由这次之后的 render 改正。
            profile = ShuangpinKeyHintPolicy.profileWithoutView(
                session != 0 || engineStartupPending(), shuangpinProfileHint());
            scheme = profile.isEmpty() ? -1 : 1;
            localMode = "none";
        }
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
                shuangpinHints, input, dedicatedEnglish, scheme, localMode, shuangpinKeyHintsEnabled);
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
        updateAutomaticCapitalization(true);
    }

    /** 切换方案前只更新英文大小写状态；调用方要把方案保存和最终渲染合成一次时暂不重画。返回英文键面的大小写是否变了。 */
    private boolean updateAutomaticCapitalization(boolean shouldRender) {
        if (!dedicatedEnglish) {
            // 韩语键面上 Shift 是用户选的双辅音而不是大写；越南语和藏文里它是下一个字母的大小写或 Caps Lock（威利转写区分大小写），所以编辑器的更新不能把它清掉。
            if (!letterCompositionActive()) letterCase.reset();
            return false;
        }
        boolean automaticEnabled = localSettings.bool(AndroidLocalSettings.AUTO_CAPITALIZATION)
            && !englishSuggestionsActive();
        // 光标前文可缺省（读不到时为 null），不记录也不持久化。
        CharSequence context = automaticEnabled && connection != null ? editorTextBeforeCursor() : null;
        boolean next = EnglishCapitalizationPolicy.shouldShift(
            automaticEnabled, EditorPolicy.capitalizationMode(editorInputType), context);
        // 手动按下的单次大写留到下一个字母用掉为止，与 HarmonyOS 一致；否则按下 Shift 后编辑器回报一次光标位置就会把它冲掉。
        boolean changed = !letterCase.isPressedShift() && letterCase.applyAutomatic(next);
        if (changed) imeLetterRows.refreshLetterCase();
        if (shouldRender) render();
        return changed;
    }

    private void rememberChineseScheme() {
        if (selectedScheme != null && !selectedScheme.otherLanguage()) lastChineseScheme = selectedScheme;
    }

    /** 设置「中英键轮换其他语言」是否打开（#6648），默认关。 */
    private boolean languageKeyCycles() {
        return localSettings.bool(AndroidLocalSettings.LANGUAGE_KEY_CYCLE);
    }

    /**
     * 中英键的点按。轮换关着、或没有启用其他语言键盘时就是 {@link #toggleInputLanguage}；打开时按「中 → 英 → 其他语言键盘 → 中」轮换，见 {@link LanguageKeyCyclePolicy}。实体键盘的中英快捷键和 Shift 进英文仍只切中英。
     */
    void languageKeyTapped() {
        if (session == 0) return;
        LanguageKeyCyclePolicy.Target target = nextLanguageTarget();
        if (target.kind() == LanguageKeyCyclePolicy.Kind.BUSY) return;
        if (target.kind() == LanguageKeyCyclePolicy.Kind.SCHEME) selectKeyboardScheme(target.scheme());
        else toggleInputLanguage();
    }

    /** 日语 9 键的语言键键面：轮换关着时是「英」（与原来相同），打开时是这一下要切到的语言。 */
    String japaneseLanguageKeyLabel() {
        return LanguageKeyCyclePolicy.targetLabel(nextLanguageTarget());
    }

    String japaneseLanguageKeyDescription() {
        return LanguageKeyCyclePolicy.description(nextLanguageTarget());
    }

    private LanguageKeyCyclePolicy.Target nextLanguageTarget() {
        JSONObject preferences = preferencesSnapshot == null ? null : preferencesSnapshot.optJSONObject("preferences");
        KeyboardScheme chineseReturn = LanguageKeyCyclePolicy.chineseReturn(lastChineseScheme,
            InputViewValuePolicy.textOr(preferences, "last_chinese_scheme", edition.defaultScheme()),
            selectedScheme != null && selectedScheme.otherLanguage()
                ? selectedScheme.touchKeyboardLayout() : null,
            visibleSchemes, edition, KeyboardScheme.fallback(edition));
        return LanguageKeyCyclePolicy.next(languageKeyCycles(), schemeSaving, selectedScheme, dedicatedEnglish,
            visibleSchemes, chineseReturn);
    }

    void toggleInputLanguage() {
        toggleInputLanguage(true);
    }

    private void toggleInputLanguage(boolean shouldRender) {
        temporaryEnglishShift = false;
        toggleInputLanguage(shouldRender, true);
    }

    /** 临时大写只改变当前会话，不覆盖用户主动选择的中英模式。 */
    private void toggleInputLanguage(boolean shouldRender, boolean rememberMode) {
        if (session == 0) return;
        boolean nextEnglish = !dedicatedEnglish;
        boolean previousSuppressed = renderingSuppressed;
        renderingSuppressed = renderingSuppressed || !shouldRender;
        try {
            if (dedicatedEnglish) command(3); else command(2);
            if (session == 0) return;
            JSONObject nextView = value(NativeClient.setEnglishMode(session, nextEnglish));
            dedicatedEnglish = nextEnglish;
            if (rememberMode && inputModeStore != null) {
                inputModeStore.remember(imeModeScope, currentEditorPackage, nextEnglish);
            }
            view = nextView;
            keyboardLayer = KeyboardLayout.Layer.LETTERS;
            letterCase.reset();
            // 同为小写状态时，中文固定大写键帽与英文小写键帽也不同；布局是否需要重建交给 refreshLetterCase 判断。
            imeLetterRows.refreshLetterCase();
            updateAutomaticCapitalization(shouldRender);
            if (directEnglishActive()) {
                refreshEnglishSuggestions();
                refreshEnglishClearButton();
            } else {
                clearEnglishSuggestions();
                if (shouldRender) render();
            }
        } catch (JSONException | LinkageError error) {
            fail(error);
        } finally {
            renderingSuppressed = previousSuppressed;
        }
    }

    private void finishTemporaryEnglishShift() {
        if (!temporaryEnglishShift || letterCase.usesUppercase()) return;
        temporaryEnglishShift = false;
        if (dedicatedEnglish && session != 0) toggleInputLanguage(true, false);
    }

    private static int touchLayout(JSONObject value) {
        if (value == null) return STANDARD_TOUCH_LAYOUT;
        // 14 键看引擎的 `key_grid`，不看 `nine_key`（14 键下它是假）；本地模式要逐个确定的字母，这时回落 26 键。
        boolean fourteenKey = "fourteen_key".equals(InputViewValuePolicy.textOr(value, "key_grid", "none"))
            && "none".equals(InputViewValuePolicy.textOr(value, "local_mode", "none"));
        boolean eighteenKey = "eighteen_key".equals(InputViewValuePolicy.textOr(value, "key_grid", "none"))
            && "none".equals(InputViewValuePolicy.textOr(value, "local_mode", "none"));
        // 九键双拼画在九键的键面上（键面换成声母和韵母，见 ImeLayoutRows），`nine_key` 在它下面为假，要看 `key_grid`。本地模式要逐个确定的字母，与 14 键一样回落 26 键。
        boolean nineKeyShuangpin = nineKeyShuangpinGrid(value)
            && "none".equals(InputViewValuePolicy.textOr(value, "local_mode", "none"));
        return KeyboardLayout.resolveTouchLayout(
            "handwriting".equals(InputViewValuePolicy.textOr(value, "touch_keyboard_layout", "")),
            InputViewValuePolicy.booleanValue(value, "nine_key", false) || nineKeyShuangpin, fourteenKey, eighteenKey,
            InputViewValuePolicy.scheme(value, -1),
            InputViewValuePolicy.textOr(value, "touch_keyboard_layout", ""));
    }

    /** 画着的是引擎组码网格的键面（拼音九键或 14 键）：读音取 `nine_key_reading`（九键写进输入框，14 键画在读音行上）。 */
    static boolean drawsKeyGrid(int layout) {
        return KeyboardLayout.drawsKeyGrid(layout);
    }

    /** 引擎开着九键双拼的网格（`View.key_grid` 为 `nine_key_shuangpin`）。 */
    static boolean nineKeyShuangpinGrid(JSONObject value) {
        return value != null
            && "nine_key_shuangpin".equals(InputViewValuePolicy.textOr(value, "key_grid", "none"));
    }

    /** 九键的字母键面现在画的是九键双拼：引擎开着九键双拼的网格，并且画的是九键的键面（本地模式回落 26 键）。 */
    boolean nineKeyShuangpinDrawn() {
        return !dedicatedEnglish && nineKeyShuangpinGrid(view)
            && displayedTouchLayout(view) == QUANPIN_NINE_KEY_LAYOUT;
    }

    // 九键双拼的键面按偏好里的方案和自定义表从 host-api 取（NativeClient.nineKeyShuangpinLayout），请求没变时沿用上一次的结果；取不到时为空，键面退回只印数字。
    private String nineKeyShuangpinRequest = "";
    private java.util.List<NineKeyShuangpinKeypad.Face> nineKeyShuangpinFaces = java.util.List.of();

    /** 当前九键双拼表每个数字键的键面，按 1 到 9、0 的顺序，只含表用到的键。 */
    java.util.List<NineKeyShuangpinKeypad.Face> nineKeyShuangpinFaces() {
        JSONObject preferences = preferencesSnapshot == null
            ? null : preferencesSnapshot.optJSONObject("preferences");
        String request;
        try {
            request = nineKeyShuangpinRequest(preferences).toString();
        } catch (JSONException error) {
            return java.util.List.of();
        }
        if (request.equals(nineKeyShuangpinRequest)) return nineKeyShuangpinFaces;
        nineKeyShuangpinRequest = request;
        nineKeyShuangpinFaces = java.util.List.of();
        try {
            JSONObject response = new JSONObject(NativeClient.nineKeyShuangpinLayout(request));
            JSONObject value = response.optBoolean("ok", false) ? response.optJSONObject("value") : null;
            if (value != null) nineKeyShuangpinFaces = nineKeyShuangpinFaces(value.optJSONArray("keys"));
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 取不到键面时只印数字，不猜另一张表的键位。
        }
        return nineKeyShuangpinFaces;
    }

    /** `msime_client_nine_key_shuangpin_layout` 的请求：偏好里的方案，选了自定义时带上那张表。设置页的编辑器用同一个形状。 */
    static JSONObject nineKeyShuangpinRequest(JSONObject preferences) throws JSONException {
        String profile = NineKeyShuangpinKeypad.normalizedProfile(
            InputViewValuePolicy.textOr(preferences, "nine_key_shuangpin_profile", NineKeyShuangpinKeypad.DEFAULT_PROFILE));
        JSONObject request = new JSONObject().put("profile", profile);
        JSONObject table = preferences == null ? null : preferences.optJSONObject("nine_key_shuangpin_custom_profile");
        if (NineKeyShuangpinKeypad.CUSTOM_PROFILE.equals(profile) && table != null) request.put("table", table);
        return request;
    }

    /** `keys` 数组读成键面；格式不对的项跳过。 */
    static java.util.List<NineKeyShuangpinKeypad.Face> nineKeyShuangpinFaces(JSONArray keys) {
        if (keys == null) return java.util.List.of();
        java.util.List<NineKeyShuangpinKeypad.Face> faces = new java.util.ArrayList<>(keys.length());
        for (int index = 0; index < keys.length(); index++) {
            JSONObject key = keys.optJSONObject(index);
            String digit = key == null ? "" : key.optString("key", "");
            if (digit.length() != 1) continue;
            faces.add(new NineKeyShuangpinKeypad.Face(digit.charAt(0), strings(key.optJSONArray("initials")),
                strings(key.optJSONArray("finals")), key.optBoolean("zero_initial", false)));
        }
        return java.util.List.copyOf(faces);
    }

    private static java.util.List<String> strings(JSONArray values) {
        if (values == null) return java.util.List.of();
        java.util.List<String> result = new java.util.ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            String value = values.optString(index, "");
            if (!value.isEmpty()) result.add(value);
        }
        return result;
    }

    void enter() {
        if (connection == null) return;
        if (commitFirstHandwritingCandidate()) return;
        // With a Korean Hanja list open Return chooses the highlighted Hanja, as Space does: only the session knows the highlight, so it is the candidate command (msime_client.h). With no list Return writes the syllable out and then does its editor action below.
        if (koreanHanjaListOpen() && command(1)) return;
        // An open Zhuyin list is chosen from the same way.
        if (zhuyinListOpen() && command(1)) return;
        if (japaneseSchemeActive() && view != null
                && !InputViewValuePolicy.editingText(view).isEmpty()) {
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
        return view != null && !InputViewValuePolicy.editingText(view).isEmpty()
            && (!letterCompositionActive() || koreanHanjaListOpen() || tibetanSchemeActive());
    }

    /** 文本编辑面板里的数字快捷键与普通「123」键一致，但总是进入数字层并关闭编辑面板。 */
    void showNumberKeyboardFromTextEdit() {
        keyboardLayer = KeyboardLayout.Layer.SYMBOLS;
        imeTextEditPanel.close();
        imeLetterRows.rebuildKeyRows();
        render();
        imeCalculator.refresh();
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
        // 长按「确认」能上屏译文时角上标「译」（#6792），没有实体键盘的人也看得到 Ctrl+Enter 的那条路。
        boolean translationHold = composing && returnHoldCommitsTranslation();
        boolean newlineHold = returnHoldInsertsNewline();
        if (enterButton != null) {
            enterButton.setText(shownTitle);
            enterButton.setContentDescription(translationHold ? shownTitle + "；长按上屏译文"
                : newlineHold ? shownTitle + "；长按换行" : shownTitle);
            if (enterButton instanceof KeyboardIconKey icon)
                icon.setCornerHint(translationHold ? "译" : newlineHold ? "换行" : "");
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
        if (japaneseLanguageKey != null) {
            // 要切到哪里取决于已启用的方案、轮换设置和最近的中文入口。换输入框时这些先回到出厂副本、再由后台读到的偏好补上；只在建键盘时写一次的话，键盘若在补上之前建好，键面就停在「英」、读屏也念切到英文，和点下去时按当前状态实际切到的地方对不上。每次重画都按当前状态重写。
            japaneseLanguageKey.setText(japaneseLanguageKeyLabel());
            japaneseLanguageKey.setContentDescription("按键 " + japaneseLanguageKeyDescription());
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
        finishPendingCompositionMarkerLoss();
        // Every physical press counts once, whoever ends up handling it; the OS's auto-repeat does not.
        if (event.getRepeatCount() == 0) countKey(KeyPressIds.forKeyCode(keyCode));
        if (event.getRepeatCount() == 0 && physicalKeyboardEvent(event)) noteHardwareTyping();
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
        if (event.getRepeatCount() == 0) translationConsumedKeyCode = -1;
        else if (keyCode == translationConsumedKeyCode) return true;
        // Ctrl+Enter 打开的义项页（#7079）先于数字选词、以词定字和修饰键分支：页面开着时数字、空格、回车、方向键和 Esc 都归它，其他键关页后照常处理。
        if (imeTranslationSenses.active()) {
            int pageKey = HardwareTranslationCommitPolicy.pageKey(keyCode, event.isShiftPressed(),
                event.isCtrlPressed(), event.isAltPressed(), event.isMetaPressed(), event.getRepeatCount(),
                imeTranslationSenses.size());
            if (pageKey == HardwareTranslationCommitPolicy.PASS) {
                imeTranslationSenses.clear();
                render();
            } else {
                if (event.getRepeatCount() == 0 && HardwareTranslationCommitPolicy.holdsKey(pageKey))
                    translationConsumedKeyCode = keyCode;
                switch (pageKey) {
                    case HardwareTranslationCommitPolicy.IGNORE -> { }
                    case HardwareTranslationCommitPolicy.COMMIT_HIGHLIGHT -> imeTranslationSenses.commitHighlighted();
                    case HardwareTranslationCommitPolicy.CLOSE -> {
                        imeTranslationSenses.clear();
                        render();
                    }
                    case HardwareTranslationCommitPolicy.PREVIOUS -> imeTranslationSenses.move(-1);
                    case HardwareTranslationCommitPolicy.NEXT -> imeTranslationSenses.move(1);
                    case HardwareTranslationCommitPolicy.FIRST -> imeTranslationSenses.moveTo(0);
                    case HardwareTranslationCommitPolicy.LAST -> imeTranslationSenses.moveTo(imeTranslationSenses.size() - 1);
                    default -> imeTranslationSenses.commit(pageKey);
                }
                return true;
            }
        }
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
                    event.getUnicodeChar(), InputViewValuePolicy.textOr(view, "spelling_symbols", ""))) {
            return character(event.getUnicodeChar(), event.isShiftPressed())
                || super.onKeyDown(keyCode, event);
        }
        // 组字中或本地模式里 Engine 列为拼写的字符是输入，要在数字选词、翻页键和标点之前送给 Engine：网址模式的 `.` `=` 和数字、`www` 之后的 `.`、V 模式的运算符。
        if (session != 0 && view != null && !event.isCtrlPressed() && !event.isAltPressed()
                && !event.isMetaPressed() && NumberRowSelectionPolicy.engineSpells(
                    InputViewValuePolicy.textOr(view, "local_mode", "none"), InputViewValuePolicy.editingText(view),
                    InputViewValuePolicy.textOr(view, "spelling_symbols", ""), event.getUnicodeChar())) {
            return character(event.getUnicodeChar(), event.isShiftPressed())
                || super.onKeyDown(keyCode, event);
        }
        // 哪一面数字键选词由策略决定：Engine 把数字列为拼写时（U、V、网址模式）裸数字是输入，选词移到 Shift 那一面。
        int candidateSlot = NumberRowSelectionPolicy.slotForKeyCode(keyCode,
            event.isShiftPressed(), numberRowSelection,
            view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none"),
            InputViewValuePolicy.textOr(view, "spelling_symbols", ""), event.getUnicodeChar());
        if (candidateSlot >= 0 && !dedicatedEnglish
                && !event.isCtrlPressed() && !event.isAltPressed() && !event.isMetaPressed()
                && view != null
                && view.optJSONArray("candidates") != null
                && candidateSlot < view.optJSONArray("candidates").length()
                && selectHardwareCandidate(candidateSlot)) return true;
        // 数字和电话框的按键交给编辑器自己处理，英文模式也一样：输入法上屏走 InputConnection，交还给应用的回车走按键通道，两条通道不保证先后，快速输入「PIN + 回车」时回车可能先到，系统认证界面拿着不完整的 PIN 去校验（数字密码进框临时切英文后暴露，#10481）。
        if (directEnglishActive() && !EditorPolicy.numberInput(editorInputType)
                && !event.isCtrlPressed() && !event.isAltPressed()
                && !event.isMetaPressed()) {
            if (keyCode == KeyEvent.KEYCODE_DEL) {
                clearEnglishSuggestions();
                return super.onKeyDown(keyCode, event);
            }
            int unicode = event.getUnicodeChar();
            if (unicode >= 32 && unicode <= 126) {
                // 末尾总要重画一次，上屏过程中要求的重画并进这一次。
                beginRenderBatch();
                try {
                    if (TextPolicy.isAsciiLetter(unicode)) {
                        char output = letterCase.usesUppercase() || event.isShiftPressed()
                            ? Character.toUpperCase((char) unicode) : Character.toLowerCase((char) unicode);
                        commitEnglishLiteral(output);
                        if (letterCase.consumeLetter()) {
                            finishTemporaryEnglishShift();
                            imeLetterRows.refreshLetterCase();
                        }
                    } else {
                        commitEnglishLiteral(unicode);
                    }
                    render();
                } finally {
                    endRenderBatch();
                }
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
        // 组字中只按着 Ctrl 的回车上屏高亮候选的译文（#7079），也在下面的修饰键分支之前，否则那里会先把原文上屏、再把回车交给应用。
        if (handleTranslationCommitKey(keyCode, event)) return true;
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
            // Tab 在有多页候选时先打开完整候选面板，和 iPad 的全尺寸键盘一致；面板不适用时
            // 继续走下面的普通翻页路径。Shift+Tab 保留上一页，不抢它的反向翻页语义。
            if (CandidateNavigationPolicy.shouldOpenCandidatePanel(keyCode, event.isShiftPressed(),
                    candidateNavigation, candidatePanelOpen)) {
                openCandidatePanel();
                if (candidatePanelOpen) return true;
            }
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
        // 普通组字里未被选词规则认领的硬件数字是字面输入；本地模式仍由引擎决定拼写、选词或原文回退。
        if (unicode >= '0' && unicode <= '9'
                && "none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"))
                && (nineKeyShuangpinGrid(view)
                || InputViewValuePolicy.booleanValue(view, "nine_key", false))) {
            if (hasEngineComposition()) command(FINISH_COMPOSITION_COMMAND);
            commitText(fullWidthOutput(String.valueOf((char) unicode)));
            return true;
        }
        if (dedicatedEnglish && unicode >= 32 && unicode <= 126 && !TextPolicy.isAsciiLetter(unicode)) {
            commitEnglishLiteral(unicode);
            return true;
        }
        if (koreanSchemeActive()) unicode = KoreanInputPolicy.hardwareCharacter(unicode, event.isShiftPressed());
        boolean handled = unicode >= 32 && unicode <= 126
            && character(unicode, event.isShiftPressed());
        return handled || super.onKeyDown(keyCode, event);
    }

    @Override public boolean onKeyUp(int keyCode, KeyEvent event) {
        // 按下归了输入法的键，抬起也不交给应用：先松开 Ctrl 再松开回车时，这个不带修饰键的抬起会让单行输入框执行编辑器动作。
        if (keyCode == translationConsumedKeyCode) {
            translationConsumedKeyCode = -1;
            return true;
        }
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
        imeTextEditPanel.selectionChanged(newStart, newEnd);
        cancelPendingCompositionMarkerLoss();
        int scheme = InputViewValuePolicy.scheme(view, -1);
        // restartInput 前可能先丢失标记，旧连接也已不可读；给紧随的重启一小段接手机会。
        if (composingStart == -1 && composingEnd == -1
                && session != 0 && !editorCompositionSuppressed && (scheme == 0 || scheme == 1)
                && view != null && selectionEcho.matchesRestartComposition(newStart, newEnd,
                    composingText(view, InputViewValuePolicy.editingText(view), true).length())) {
            pendingCompositionMarkerLoss = () -> {
                pendingCompositionMarkerLoss = null;
                updateEditorSelection(oldStart, oldEnd, newStart, newEnd, composingStart, composingEnd);
            };
            main.postDelayed(pendingCompositionMarkerLoss, 100);
            return;
        }
        updateEditorSelection(oldStart, oldEnd, newStart, newEnd, composingStart, composingEnd);
    }

    private void updateEditorSelection(int oldStart, int oldEnd, int newStart, int newEnd,
            int composingStart, int composingEnd) {
        boolean selectionChanged = oldStart != newStart || oldEnd != newEnd;
        // 下面这些面板会因选区变化被关掉或作废；它们本来就没开着时，这次回报不会因此改变界面。
        boolean panelsIdle = !ViewPolicy.isVisible(voiceResultScroll) && !ViewPolicy.isVisible(aiPolishContainer)
            && !ViewPolicy.isVisible(replyKeyboard) && replyTarget == null && aiTarget == null && !handwritingActive();
        if (selectionChanged) {
            editorContextRevision++;
            // Android reports this boundary after the host has moved the selection. Do not carry
            // Apple service-panel or handwriting state into the new editor context.
            clearHandwriting();
            closeVoiceResult();
            closeAiPolish();
        }
        if (ViewPolicy.isVisible(aiPolishContainer)
                && aiTarget != null && !aiTargetMatches()) {
            cancelAiRequest();
            aiError = "输入位置已变化，请返回键盘后重新选择文字。";
            imePanels.renderAiPolish();
        }
        boolean replyVisible = ViewPolicy.isVisible(replyKeyboard);
        if (replyTarget != null || (selectionChanged && replyVisible)) {
            invalidateReplyContext("输入位置已变化，请重新选择回复方式");
        }
        // 迟到的回报可能只是输入法自己上一次写入的回声（例如上屏之后用户已经按下了下一个键），那不是光标移动，不能把新开始的组字取消掉；对不上任何预期的才按原来的规则处理。
        boolean ownEcho = selectionEcho.acknowledge(newStart, newEnd, composingStart, composingEnd);
        if (!ownEcho && selectionChanged && session != 0 && view != null && view.optBoolean("prediction", false)) {
            try {
                // 新位置也切断个人词序与重排上下文，不能把随后选中的词记成旧位置的后续词。
                view = value(NativeClient.focus(session, true)).getJSONObject("view");
                render();
            } catch (JSONException | LinkageError error) { fail(error); }
        }
        if (!ownEcho && selectionChanged) englishCorrectionRejectedPrefix = "";
        if (!ownEcho && quickDeleteUndo != null && !quickDeleteUndo.atSelection(newStart, newEnd)) {
            // 分段删除的中间选区回报可能迟到；按编辑器此刻的真实位置核对，不能仅凭旧回报丢掉备份。
            canUndoQuickDelete();
        }
        // 用户自己移了光标（或应用改了文字），之前补上的后半个不再是「下一个要跨过的」。
        if (!ownEcho && selectionChanged) pairedPunctuationStack.clear();
        boolean editingComposition = !ownEcho && selectionChanged
            && editCompositionSelection(composingStart, composingEnd, newStart, newEnd);
        if (!ownEcho && !editingComposition && session != 0 && view != null && !InputViewValuePolicy.editingText(view).isEmpty()
                && (newStart != composingEnd || newEnd != composingEnd)) {
            // Don't apply an empty composition over the editor's newly moved selection.
            // 韩语音节已经是最终的韩文并内联标记，下面结束组字区域后它留在文档里，所以算作已输入。注音转换、越南语单词和藏文音节同样是已书写的文字（`commits_on_blur`）；藏文记的是 `editing_text` 里转换后的藏文。
            if (koreanSchemeActive() || zhuyinSchemeActive())
                recordTypingStatistics(InputViewValuePolicy.textOr(view, "reading", ""), typingSource());
            else if (letterCaseSchemeActive())
                recordTypingStatistics(InputViewValuePolicy.editingText(view), typingSource());
            boolean keepsComposition = !koreanSchemeActive() && writtenCompositionActive();
            try {
                // 韩语汉字列表打开时第一次取消只关闭列表（msime_client.h），要再取消一次才丢掉编辑器里已作为文字保留的音节。注音的第一次取消只关闭列表，越南语和藏文的只退回原始按键，所以它们留下的组字还要再取消一次。
                JSONObject cancelled = null;
                for (int cancels = KoreanInputPolicy.cancelsToDiscard(koreanHanjaListOpen()); cancels > 0; cancels--)
                    cancelled = value(NativeClient.command(session, 3));
                JSONObject left = cancelled == null ? null : cancelled.optJSONObject("view");
                if (keepsComposition && left != null && !InputViewValuePolicy.editingText(left).isEmpty())
                    value(NativeClient.command(session, 3));
            } catch (JSONException | LinkageError error) { fail(error); }
            // Stroke marks its stroke glyphs, which are not text the user wrote (`commits_on_blur` is false): finishing the region would leave 一丨 in the document, so the region is removed and the tapped selection put back.
            if (connection != null && strokeCompositionMarked()) {
                bridge.discard(sink(typingSource()), composingStart, composingEnd, newStart, newEnd);
            } else if (connection != null) {
                finishCompositionInPlace(composingStart, composingEnd, newStart, newEnd);
                // 结束组字区也会有一条回报；记下它，免得它晚到时把用户紧接着开始的新组字当成光标移动取消掉。
                selectionEcho.expect();
            }
            view = null;
            render();
        }
        // 英文键面上每次回报都重画一次，兜住选区变化带来的各种界面变化。例外是英文直输里输入法自己写入的回声：写完之后界面已经按写完的编辑器重画过（期间没有新的写入），这次回报又没有改变大小写、当前词和英文候选，再画一遍结果相同。英文直输每个字母都会收到这样一条回声，低端机上这次多余的整页重画和它带来的跨进程读取是持续打字时发热、卡顿的来源之一。
        boolean renderedEcho = ownEcho && panelsIdle && !candidateRenderDeferred && directEnglishActive()
            && renderedEchoExpectations == selectionEcho.expectations();
        boolean english = dedicatedEnglish;
        java.util.List<String> shownSuggestions = englishSuggestions;
        String shownSuggestionPrefix = englishSuggestionPrefix;
        beginEditorEvent();
        try {
            boolean letterCaseChanged = updateAutomaticCapitalization(false);
            if (directEnglishActive()) {
                refreshEnglishSuggestions();
                refreshEnglishClearButton();
            }
            if (english && (!renderedEcho || letterCaseChanged || englishSuggestions != shownSuggestions
                    || !englishSuggestionPrefix.equals(shownSuggestionPrefix))) render();
        } finally {
            endEditorEvent();
        }
        // 数字键面上打完算式（或光标挪到算式后面）时，工具栏给出计算结果。
        imeCalculator.refresh();
        // 除密码框外的输入框里打到 `xxx@` 时，候选栏给出邮箱后缀（#6147）。
        imeEmailSuffixes.refresh();
    }

    Button button(LinearLayout row, String label, Runnable action) {
        return addRowButton(row, new KeyboardPressButton(this), label, action, true);
    }

    private <T extends Button> T addRowButton(LinearLayout row, T button, String label,
            Runnable action, boolean counted) {
        KeyboardTypographyPolicy.setKeyTextSize(button, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
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
        imeStyler.styleButton(button, KeyboardKeyRole.GLYPH, skin);
        bindAction(button, action);
        row.addView(button, LayoutPolicy.weightedWrapParams(1));
        return button;
    }

    /** The idle top row's scheme pill (the design's 全拼). */
    private Button pillButton(LinearLayout row, String label, Runnable action) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        button.setKeyboardRole(KeyboardKeyRole.PILL);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        KeyboardTypographyPolicy.setKeyTextSize(button, 13);
        imeStyler.styleButton(button, KeyboardKeyRole.PILL, skin);
        bindAction(button, action);
        row.addView(button, LayoutPolicy.weightedWrapParams(1));
        return button;
    }

    Button borderlessButton(LinearLayout row, String label, Runnable action) {
        Button button = new KeyboardBorderlessButton(this);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        imeStyler.styleButton(button, true);
        bindAction(button, action);
        row.addView(button, LayoutPolicy.weightedWrapParams(1));
        return button;
    }

    KeyboardPressButton keyboardKey(String label, String description, Runnable action) {
        KeyboardPressButton button = new KeyboardPressButton(this);
        KeyboardTypographyPolicy.normalizeKeyCap(button);
        KeyboardTypographyPolicy.setKeyTextSize(button, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
        ViewPolicy.setAllCapsFalse(button);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        bindCountedAction(button, action);
        return button;
    }

    /** 九键、注音、笔画、手写和日语九键的 ⌫：和 {@link #keyboardKey} 一样的键，只是画 26 键那个 22 dp 的删除图标，不再用排版字号的「⌫」字符，那样比 26 键的小一圈。 */
    Button backspaceKey(Runnable action) {
        return iconKey(KeyboardIconKey.Kind.BACKSPACE, "⌫", "删除", action);
    }

    /** {@link #keyboardKey} 的图标版：节点文字仍是 `label`，键面画 `kind` 的描边图标。 */
    Button iconKey(KeyboardIconKey.Kind kind, String label, String description, Runnable action) {
        KeyboardIconKey button = new KeyboardIconKey(this, kind);
        KeyboardTypographyPolicy.setKeyTextSize(button, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
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

    /** 九键网格键帽：沿用 {@link #keyboardKey} 的动作绑定；数字提示由 {@link NineKeyDigitButton} 画在右上角，键面文字仍居中。 */
    NineKeyDigitButton nineKeyGridKey(String label, String description, Runnable action) {
        NineKeyDigitButton button = new NineKeyDigitButton(this);
        KeyboardTypographyPolicy.setKeyTextSize(button, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
        button.setText(label);
        button.setContentDescription("按键 " + description);
        imeStyler.styleButton(button, false);
        button.setOnTouchListener((view, event) -> {
            imeLetterRows.updateKeyPreview(button, event);
            return false;
        });
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

    /** 26 键字母层的字母键和微软双拼的 `;`：点击时把自己交给 {@link #type(char, View)}，它记下的按下点随这一键送给引擎。 */
    ShuangpinHintButton shuangpinLetterKey(String label, String description, char input) {
        ShuangpinHintButton[] key = new ShuangpinHintButton[1];
        key[0] = shuangpinKeyboardKey(label, description, () -> type(input, key[0]));
        return key[0];
    }

    ShuangpinHintButton shuangpinKeyboardKey(
            String label, String description, Runnable action) {
        ShuangpinHintButton button = new ShuangpinHintButton(this);
        KeyboardTypographyPolicy.setKeyTextSize(button, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
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
        return view != null && !InputViewValuePolicy.editingText(view).isEmpty();
    }

    int pixels(int value) {
        return DimensionPolicy.pixels(this, value);
    }

    int pixels(double value) {
        if (value <= 0) return 0;
        return DimensionPolicy.atLeastOnePixel(this, (float) value);
    }

    int halfSpacingPixels(int tenths) {
        return KeyboardSpacingPolicy.halfGapPixels(tenths, DimensionPolicy.density(this));
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
        return DimensionPolicy.isNight(this);
    }

    private KeyboardSkin keyboardSkin(JSONObject preferences) {
        return keyboardSkin(preferences, systemDark());
    }

    private KeyboardSkin keyboardSkin(JSONObject preferences, boolean systemDark) {
        return surfaceSkin(preferences, "screen_keyboard_theme", systemDark);
    }

    /** 决定键盘、表情、手写面板皮肤与候选条外观的偏好字段，{@link #applyTouchGeometry} 读的键盘几何字段，加上工具栏按钮开关 `touch_toolbar`；{@link #rememberSkinHint} 只记这几项。 */
    private static final String[] SKIN_HINT_KEYS = {"global_theme", "custom_theme", "theme",
        "android_keyboard_skin_light", "android_keyboard_skin_dark",
        "candidate_theme", "candidate_font_family", "candidate_english_font", "candidate_fallback_fonts",
        "candidate_font_size", "candidate_preedit_font_size",
        "candidate_scale_percent",
        "touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
        "touch_voice_shortcut", ShuangpinKeyHintPolicy.PREFERENCE_KEY, QuanpinSeparatorPolicy.PREFERENCE_KEY,
        TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, TouchKeyStyle.FOURTEEN_KEY_PREFERENCE,
        TouchKeyStyle.NINE_KEY_PREFERENCE, TouchKeyStyle.NUMBER_ROW_PREFERENCE,
        TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE,
        NineKeySidebarPolicy.LETTER_PREFERENCE, NineKeySidebarPolicy.DIGIT_PREFERENCE,
        OneHandSidePadPolicy.PREFERENCE_KEY,
        TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE,
        NineKeySwipePolicy.PREFERENCE, "screen_keyboard_theme", "emoji_theme", "handwriting_theme", "touch_toolbar"};
    /** 上次换上的皮肤所用的偏好片段，存在键盘进程自己的 filesDir 里。 */
    private static final String SKIN_HINT_FILE = "keyboard-skin-hint.json";
    private String writtenSkinHint;
    /** 最近一次真正读到的偏好里的 `touch_toolbar`（启动时来自皮肤片段文件）；没有时为 null。 */
    private JSONObject rememberedToolbar;

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
        JSONObject toolbar = hint.optJSONObject("touch_toolbar");
        if (toolbar != null) rememberedToolbar = toolbar;
        String text = hint.toString();
        if (text.equals(writtenSkinHint)) return;
        writtenSkinHint = text;
        File target = new File(getFilesDir(), SKIN_HINT_FILE);
        preferencesWorker.execute(() -> {
            java.nio.file.Path pending = null;
            try {
                pending = java.nio.file.Files.createTempFile(getFilesDir().toPath(),
                    SKIN_HINT_FILE + ".", ".pending");
                java.nio.file.Files.write(pending, TextPolicy.utf8Bytes(text),
                    java.nio.file.StandardOpenOption.TRUNCATE_EXISTING,
                    java.nio.file.StandardOpenOption.WRITE,
                    java.nio.file.LinkOption.NOFOLLOW_LINKS);
                java.nio.file.Files.move(pending, target.toPath(),
                    java.nio.file.StandardCopyOption.REPLACE_EXISTING, java.nio.file.StandardCopyOption.ATOMIC_MOVE);
            } catch (java.io.IOException | RuntimeException error) {
                android.util.Log.w("MSIMESkin", "Keyboard skin hint was not written", error);
            } finally {
                if (pending != null) try { java.nio.file.Files.deleteIfExists(pending); }
                catch (java.io.IOException ignored) { }
            }
        });
    }

    /** 读上次的皮肤片段；没有或损坏时返回 null，键盘照旧先用内置配色。 */
    private JSONObject readSkinHint() {
        File file = new File(getFilesDir(), SKIN_HINT_FILE);
        if (!SkinHintFilePolicy.readable(file)) return null;
        try (java.io.InputStream input = java.nio.file.Files.newInputStream(file.toPath(), java.nio.file.LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, SkinHintFilePolicy.MAX_BYTES);
            if (bytes == null) return null;
            String text = TextPolicy.utf8(bytes);
            writtenSkinHint = text;
            JSONObject hint = new JSONObject(text);
            rememberedToolbar = hint.optJSONObject("touch_toolbar");
            return hint;
        } catch (Exception ignored) {
            return null;
        }
    }

    /**
     * 候选条的配色。皮肤和键盘、表情、手写一样要经 {@link ImeStyler#themed} 按应用主题着色：「跟随系统」不着色时取的是经典绿种子，暖色主题下候选条铺一层淡绿底、页码也是绿的。键盘里换皮肤时也要跟着重算，否则候选条停在旧皮肤上。
     */
    CandidateAppearance.Palette candidateAppearanceFor(JSONObject preferences) {
        return candidateAppearanceFor(preferences, systemDark());
    }

    private CandidateAppearance.Palette candidateAppearanceFor(JSONObject preferences, boolean systemDark) {
        KeyboardSkin strip = surfaceSkin(preferences, "candidate_theme", systemDark);
        CandidateAppearance.Palette fallback = CandidateAppearance.from(preferences,
            imeStyler == null ? strip : imeStyler.themed(strip));
        String surfaceMode = preferences == null ? "follow" : preferences.optString("candidate_theme", "follow");
        String appMode = preferences == null ? "system" : preferences.optString("theme", "system");
        return candidateSkinResolver.paletteFor(preferencesDirectory, preferences,
            KeyboardSkin.resolveDark(surfaceMode, appMode, systemDark), fallback);
    }

    /**
     * 按各面板自己的明暗解析主题；屏幕键盘优先使用该明暗专用的皮肤选择。
     *
     * <p>面板自己的 `dark` 或 `light` 优先；`follow` 跟随应用的明暗（`theme`），应用为 `system` 时跟随 Android 的夜间模式。缺失或不认识的值按 `follow` 处理，旧快照因此保持键盘原来的明暗，不会跳回浅色。所选主题属于另一种明暗时按 `system` 画（{@link KeyboardSkin#effectiveTheme}），键盘因此始终和这块面板的明暗一致。
     */
    private KeyboardSkin surfaceSkin(JSONObject preferences, String key) {
        return surfaceSkin(preferences, key, systemDark());
    }

    private KeyboardSkin surfaceSkin(JSONObject preferences, String key, boolean systemDark) {
        String surfaceMode = preferences == null ? "follow" : preferences.optString(key, "follow");
        String appMode = preferences == null ? "system"
            : InputViewValuePolicy.textOr(preferences, "theme", "system");
        boolean dark = KeyboardSkin.resolveDark(surfaceMode, appMode, systemDark);
        String globalTheme = preferences == null ? "system"
            : InputViewValuePolicy.textOr(preferences, "global_theme", "system");
        JSONObject customTheme = preferences == null ? null
            : preferences.optJSONObject("custom_theme");
        if ("screen_keyboard_theme".equals(key)) {
            app.msime.android.KeyboardSkinModeChoice.Selected selected =
                app.msime.android.KeyboardSkinModeChoice.selected(preferences, dark, modeSkinLibrary);
            globalTheme = selected.theme();
            if (selected.libraryId() != null) {
                try {
                    customTheme = customTheme == null ? new JSONObject() : new JSONObject(customTheme.toString());
                    customTheme.put("keyboard", selected.design());
                }
                catch (JSONException error) { return KeyboardSkin.system(dark); }
            }
        }
        return themeSkin(KeyboardSkin.effectiveTheme(themeCatalog(), globalTheme, dark), customTheme, dark);
    }

    /** 在偏好工作线程读取设计库；读取失败时槽位引用自然回退旧主题。 */
    private void refreshModeSkinLibrary(JSONObject preferences) {
        int generation = ++modeSkinLibraryGeneration;
        if (!preferences.optString("android_keyboard_skin_light", "").startsWith("design:")
                && !preferences.optString("android_keyboard_skin_dark", "").startsWith("design:")) {
            modeSkinLibrary = java.util.List.of();
            return;
        }
        String directory = preferencesDirectory;
        if (directory.isEmpty()) return;
        preferencesWorker.execute(() -> {
            java.util.List<CustomSkinLibrary.Item> loaded = java.util.List.of();
            try { loaded = CustomSkinLibrary.read(java.nio.file.Paths.get(directory)); }
            catch (java.io.IOException | RuntimeException error) {
                android.util.Log.w("MSIMESkin", "Mode skin library unreadable", error);
            }
            java.util.List<CustomSkinLibrary.Item> result = loaded;
            main.post(() -> {
                if (generation != modeSkinLibraryGeneration) return;
                modeSkinLibrary = result;
                JSONObject current = preferencesSnapshot == null ? null : preferencesSnapshot.optJSONObject("preferences");
                if (current == null) return;
                KeyboardSkin updated = keyboardSkin(current);
                if (!skin.key().equals(updated.key())) {
                    skin = updated;
                    imeStyler.applySkin();
                    render();
                }
            });
        });
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
        String cacheKey = request + (design == null ? "" : ":" + designHashes.computeIfAbsent(design, value -> value.toString().hashCode()));
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
            if (mode == LocalInputMode.TEMPORARY_JAPANESE && !edition.temporaryJapanese()) continue;
            if (mode.command() && !commandLettersTypable()) continue;
            modes.add(mode);
        }
        return modes;
    }

    /**
     * 「指令」只给 26 键：`/` 模式里键盘仍是原来的键面，九键、14 键、18 键的键发的是数字或分组码，Engine 在 `/` 模式里不收，按第一个键就结束模式、把 `/` 原样上屏（API 35 模拟器上实测），指令字母打不出来。26 键直接发字母，与实体键盘相同。
     */
    private boolean commandLettersTypable() {
        return displayedTouchLayout(view) == STANDARD_TOUCH_LAYOUT;
    }

    /** 本地模式可用：偏好打开着，临时日语还要日文词典已在本机（随包或下载的资源包）。没有词典时 host-api 本来就把它关掉，菜单里置灰，免得点了没反应；去设置 → 输入下载。`/` 指令出厂关着（在插件页指令表的详情里打开），Engine 还只在中文标点下把 `/` 当作入口，英文标点下按了只会打出一个 `/`，所以那时也置灰。 */
    boolean localModeEnabled(LocalInputMode mode) {
        if (mode == LocalInputMode.TEMPORARY_JAPANESE && !resourcePacks.contains(KeyboardScheme.JAPANESE_PACK)) return false;
        if (mode.command() && !chinesePunctuation) return false;
        return localModes.optBoolean(mode.preferenceKey(), mode.enabledByDefault());
    }

    void openLocalInputMode(LocalInputMode mode) {
        if (session == 0 || !supportsLocalTools() || !localModeEnabled(mode)) return;
        imeKeyFeedback.playFeedback(moreButton);
        // Shift+字母进入的模式按大写字母送；`/` 是标点，照没按 Shift 的样子送，与实体键盘上直接按 / 相同。
        character(mode.trigger().charAt(0), !mode.command());
    }

    void closeClipboardHistory() {
        imePanels.resetClipboardCategoryPicker();
        imePanels.dismissClipboardPreview();
        imePanels.clearClipboardDetail();
        if (clipboardSurface != null) ViewPolicy.hide(clipboardSurface);
        // Cloud entries live only as long as the panel that fetched them, so a later field - possibly a password one - never starts with them in memory.
        cloudClipboardGeneration++;
        cloudClipboardAccountId = "";
        cloudClipboardBindingGeneration = -1L;
        cloudClipboardSessionId = "";
        cloudClipboardItems = java.util.List.of();
        cloudClipboardStatus = CloudClipboardPanelPolicy.Status.LOADING;
    }

    /** 切换访客剪贴板并立即换用隔离存储；模式只写布尔状态，不记录剪贴板内容。 */
    void toggleClipboardMode(KeyboardIconKey button) {
        ClipboardModePolicy.Mode next = clipboardMode == ClipboardModePolicy.Mode.VISITOR
            ? ClipboardModePolicy.Mode.MAIN : ClipboardModePolicy.Mode.VISITOR;
        clipboardMode = next;
        ClipboardModeStore.save(this, next);
        clipboardHistory = ClipboardHistoryStore.forMode(this, next);
        closeClipboardHistory();
        if (button != null) {
            button.setKind(next == ClipboardModePolicy.Mode.VISITOR
                ? KeyboardIconKey.Kind.TRASH : KeyboardIconKey.Kind.RETURN);
            final ClipboardModePolicy.Mode changed = next;
            main.postDelayed(() -> {
                if (button == enterButton && clipboardMode == changed)
                    button.setKind(KeyboardIconKey.Kind.RETURN);
            }, 600L);
        }
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
        if (imeFunctionPanel != null) imeFunctionPanel.close();
        if (imeWorldClock != null) imeWorldClock.close();
        localInputToolsOpen = false;
        aiAssistChooserOpen = false;
        deviceInfoOpen = false;
        if (moreToolsScroll != null) ViewPolicy.hide(moreToolsScroll);
    }

    void closeEmojiPicker() {
        emojiLoadGeneration++;
        emojiLoading = false;
        emojiSelectedCategory = Integer.MIN_VALUE;
        emojiItems = java.util.List.of();
        imeStickers.release();
        if (emojiPanel != null) ViewPolicy.hide(emojiPanel);
        synchronizeReplyKeyboard();
    }

    void closeSymbolPanel() {
        if (imePanels != null) imePanels.invalidatePluginSymbolGroupReads();
        if (symbolPanel != null) ViewPolicy.hide(symbolPanel);
        synchronizeReplyKeyboard();
    }

    java.util.List<String> loadEmojiRecents() {
        if (emojiPreferences == null) return java.util.List.of();
        String document = emojiPreferences.getString(EMOJI_RECENTS_KEY, "[]");
        if (document == null || document.length() > 16_384) return java.util.List.of();
        try {
            JSONArray values = new JSONArray(document);
            int count = BoundsPolicy.atMost(values.length(), EmojiCatalogModel.RECENTS_LIMIT * 2);
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
        return ViewPolicy.isVisible(emojiPanel);
    }

    void selectEmojiCategory(int category) {
        if (!emojiPickerVisible()) return;
        if (category < -1 || category > imeStickers.lastEmojiCategory()) return;
        // 离开表情包时放掉它的缩略图；表情包一次摆完，不经目录分页。
        if (category != imeStickers.category()) imeStickers.release();
        emojiLoadGeneration++;
        emojiSelectedCategory = category;
        emojiNextOffset = 0;
        emojiComplete = category == -1 || category == imeStickers.category();
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
        long offset = NumberPolicy.strictLong(value.opt("next_offset"), -1);
        if (offset < 0) throw new JSONException("Invalid emoji catalog cursor");
        return offset;
    }

    void loadEmojiPage() {
        if (!emojiPickerVisible() || emojiSelectedCategory < 0
                || emojiSelectedCategory >= EmojiCatalogModel.categories().size() || emojiLoading || emojiComplete
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
                if (EmojiCombinationCatalog.GROUP.equals(category.group())) {
                    if (emojiCombinationItems == null) {
                        try (java.io.InputStream input = getAssets().open(EmojiCombinationCatalog.ASSET)) {
                            emojiCombinationItems = EmojiCombinationCatalog.decode(new String(
                                HttpBodyPolicy.readRequired(input, EmojiCombinationCatalog.MAX_BYTES),
                                java.nio.charset.StandardCharsets.UTF_8));
                        }
                    }
                    // 组合短语包含多个字形，不能用整串 hasGlyph 的「单字形」判断过滤。
                    page = EmojiCombinationCatalog.page(emojiCombinationItems, offset, EmojiCatalogModel.PAGE_SIZE);
                } else {
                    page = EmojiCatalogModel.renderable(decodeEmojiPage(
                        NativeClient.emojiCatalog(query, resources), offset, category),
                        emojiGlyphPaint::hasGlyph);
                }
            }
            catch (java.io.IOException | JSONException | RuntimeException | LinkageError ignored) {
                // 界面只显示目录加载失败，不暴露资源路径或条目。
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
        voiceSetupReason = null;
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
        imePanels.invalidateReplyTemplateReads();
        replyOpen = false;
        replyModel.resetResults();
        clearReplyRequestReferences();
        setReplyKeyboardVisible(false);
    }

    /** Keep the shared candidate/shortcut bar visible while the reply surface owns the key area. */
    void setReplyKeyboardVisible(boolean visible) {
        // 面板顶替按键区，就该和按键区一样高：键盘外层按内容定高，只靠权重占「剩下的空间」时，按键行一隐藏键盘就整体变矮，面板里的风格格子被压扁。打开前量一次按键区的实际高度给面板；还没布局过（高度为 0）时保留权重。
        if (visible && replyKeyboard != null && actionRow != null && ViewPolicy.isVisible(keyRows)) {
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
        imePanels.invalidateReplyTemplateReads();
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
        if (ViewPolicy.isVisible(replyKeyboard)) {
            replyModel.showStatus(value);
            imePanels.renderReplyKeyboard();
        } else {
            notice(value);
        }
    }

    /** 键盘皮肤面板只保存当前明暗的键盘选择短引用；候选窗和另一种明暗的选择不变。 */
    void saveKeyboardSkin(String identifier, JSONObject design, String libraryId) {
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
            boolean dark = KeyboardSkin.resolveDark(
                InputViewValuePolicy.textOr(preferences, "screen_keyboard_theme", "follow"),
                InputViewValuePolicy.textOr(preferences, "theme", "system"), systemDark());
            String key = app.msime.android.KeyboardSkinModeChoice.key(dark);
            String choice = design == null ? app.msime.android.KeyboardSkinModeChoice.theme(identifier)
                : app.msime.android.KeyboardSkinModeChoice.design(libraryId);
            if (choice.equals(preferences.optString(key))) return;
            preferences.put(key, choice);
            if (design != null) {
                applyDesignFeedback(preferences, design);
                designAnimation = CustomKeyboardSkin.from(design).pressAnimation();
            }
        } catch (JSONException error) {
            showKeyboardSkinStatus("皮肤切换失败，保留当前皮肤");
            return;
        }
        skin = design == null ? keyboardSkin(preferences) : KeyboardSkin.custom(design,
            KeyboardSkin.resolveDark(InputViewValuePolicy.textOr(preferences, "screen_keyboard_theme", "follow"),
                InputViewValuePolicy.textOr(preferences, "theme", "system"), systemDark()));
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

    /**
     * 长按「中/英」（以及地球键、日语九键侧列的英和切换）弹出系统的输入法选择框，用来临时换到密码管理器之类的键盘（#5615）。组字不在这里结束：用户可能只是看一眼就关掉选择框；真的换了输入法时 onFinishInput 会照常收尾。
     */
    void bindInputMethodPicker(Button button) {
        button.setOnLongClickListener(ignored -> {
            InputMethodManager manager = getSystemService(InputMethodManager.class);
            if (manager == null) return false;
            imeKeyFeedback.playFeedback(button);
            manager.showInputMethodPicker();
            return true;
        });
        // 给读屏的长按动作一个名字，否则只会念「双击并按住即可长按」，听不出长按做什么。
        button.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(
                    View host, android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.addAction(new android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                    android.view.accessibility.AccessibilityNodeInfo.ACTION_LONG_CLICK, "切换输入法"));
            }
        });
    }

    /**
     * 长按字母层的 123 直接打开符号面板，不必再经过 #+= 层；记一次「符」键，与点 #+= 层的「符」相同。在符号层里这个键是 ABC / 九键，长按照常当作点按。
     */
    void bindSymbolPanelHold(Button button) {
        button.setOnLongClickListener(ignored -> {
            if (session == 0 || keyboardLayer != KeyboardLayout.Layer.LETTERS) return false;
            imeKeyFeedback.playFeedback(button);
            countKey("SoftSymbol");
            imePanels.showSymbolPanel();
            return true;
        });
        button.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(
                    View host, android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                if (keyboardLayer == KeyboardLayout.Layer.LETTERS)
                    info.addAction(new android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                        android.view.accessibility.AccessibilityNodeInfo.ACTION_LONG_CLICK, "打开符号面板"));
            }
        });
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
            notice("简繁设置尚未就绪");
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
            notice("简繁设置未能保存");
        }
        render();
    }

    static final String REPLY_SOURCE_PLACEHOLDER = "+ 粘贴 TA 的话帮你回";

    boolean voiceInsertionReady() {
        return session != 0 && connection != null && view != null
            && InputViewValuePolicy.editingText(view).isEmpty()
            && InputViewValuePolicy.textOr(view, "local_mode", "none").equals("none");
    }

    boolean aiPolishReady() { return voiceInsertionReady(); }

    String editorContext(boolean before) {
        if (connection == null) return null;
        CharSequence text = before ? InputConnectionGuard.textBeforeCursor(connection, 64)
            : InputConnectionGuard.textAfterCursor(connection, 64);
        return text == null ? null : text.toString();
    }

    String selectedEditorText() {
        if (connection == null) return null;
        CharSequence text = InputConnectionGuard.selectedText(connection);
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
        voiceActivityGeneration++;
        if (!voiceInputEnabled) {
            showVoiceSetup("语音输入已关闭，在设置里打开后，长按空格就能说话。");
            return;
        }
        // 语音结果面板盖在键区上面；从面板里的「开始语音识别」进来时先收起它，键区里的聆听面板才看得见。只在面板开着时收：正在聆听时再按语音键是结束录音，这时清掉记下的输入位置会让结果无法直接上屏。
        if (ViewPolicy.isVisible(voiceResultScroll)) closeVoiceResult();
        // 键盘内识别（扩展点）接手时不再打开识别窗口。
        if (imeVoiceEntry.startInKeyboard(keyRows)) return;
        launchVoiceActivity();
    }

    /** 申请麦克风权限或系统服务需要前台恢复时，打开识别窗口并回传结果。 */
    void launchVoiceActivity() {
        imeDebugOverlay.recordEvent(DiagnosticEventKind.VOICE_ACTIVITY_LAUNCH);
        String requestId = "ime-" + Long.toUnsignedString(SystemClock.uptimeMillis());
        // The configuration the settings app resolves, read through the same shared entry. This
        // keyboard's voice button used to launch the platform recogniser unconditionally, so a
        // user who had configured a provider got it from the settings panel and not from here.
        VoiceConfiguration configured = VoiceConfiguration.read(preferencesDirectory, requestId);
        if (configured.provider() == null && !VoiceRecognitionActivity.available(this)) {
            // 只有没配置服务商时才走到这里。原来只在读音行写一行小字，大多数不带系统识别服务的国产手机上，长按空格看起来就是没反应；现在打开引导面板，一键去设置里下载本地模型或选服务商。
            showVoiceSetup("这台手机没有系统语音识别服务。下载本地离线模型后就能用，录音不离开本机；也可以选择在线识别服务。");
            return;
        }
        closeVoiceResult();
        long operation = ++voiceActivityGeneration;
        android.os.ResultReceiver resultReceiver = voiceActivityResultReceiver(operation);
        try {
            VoiceRecognitionActivity.launch(this, requestId, voiceLanguage,
                configured.providerName(), configured.requestFormat(), configured.endpoint(),
                configured.model(), configured.token(), configured.streaming(), configured.polish(),
                configured.localModel(), null, null, voiceDiagnosticsReceiver(), resultReceiver);
        } catch (RuntimeException error) {
            VoiceRecognitionActivity.clearRequest(requestId);
            // 同一个笼统的提示有两个来源（键区里识别失败、识别窗口起不来），不记类型就分不出来
            // （只记异常类名，不含输入、路径和凭据）。
            imeDebugOverlay.recordEvent(VoiceDiagnostics.unexpected(error));
            ImeLog.w("Voice window launch failed: " + error.getClass().getName());
            notice("语音识别服务无法启动");
        }
    }

    /** 独立保存目标快照，避免窗口收起键盘时关闭面板把它清掉。 */
    private android.os.ResultReceiver voiceActivityResultReceiver(long operation) {
        if (!voiceInsertionReady()) return null;
        String before = editorContext(true);
        String after = editorContext(false);
        // 读不到上下文时无法确认原输入位置，保留原来的手动插入路径。
        if (before == null || after == null) return null;
        EditorContextSnapshot target = new EditorContextSnapshot(connection, editorContextRevision,
            before, selectedEditorText(), after);
        return new android.os.ResultReceiver(main) {
            @Override protected void onReceiveResult(int code, android.os.Bundle payload) {
                if (operation != voiceActivityGeneration || code != android.app.Activity.RESULT_OK
                        || payload == null) return;
                final java.util.UUID id;
                try { id = java.util.UUID.fromString(payload.getString(VoiceRecognitionActivity.RESULT_ID)); }
                catch (IllegalArgumentException | NullPointerException error) { return; }
                insertVoiceActivityResult(operation, target, id, 30);
            }
        };
    }

    /** 等前台窗口归还原编辑连接，再领取本次结果；不能把结果插进后来切换的输入框。 */
    private void insertVoiceActivityResult(long operation, EditorContextSnapshot target,
                                          java.util.UUID id, int attempts) {
        if (operation != voiceActivityGeneration || voiceResultStore == null) return;
        if (voiceInsertionReady() && target.matches(connection, editorContextRevision,
                editorContext(true), selectedEditorText(), editorContext(false))) {
            // 先使重复回传失效，再按编号领取，旧回调不能插入更新后的另一条结果。
            voiceActivityGeneration++;
            try {
                String text = voiceResultStore.consume(id, System.currentTimeMillis());
                if (!commitText(text, TypingSource.VOICE)) notice("编辑器拒绝插入；结果已安全清除");
            } catch (VoiceResultStore.Failure error) {
                notice("语音结果未能自动插入，可在语音结果中查看");
            }
            return;
        }
        if (attempts > 0) {
            main.postDelayed(() -> insertVoiceActivityResult(operation, target, id, attempts - 1), 100);
        } else {
            voiceActivityGeneration++;
            notice("输入位置已变化，结果已保留，可在语音结果中插入");
        }
    }

    /** 两次隐私检查：被抑制的请求不能在切换输入框后补记；当前已关闭也不再记录。 */
    private android.os.ResultReceiver voiceDiagnosticsReceiver() {
        if (!voiceDiagnosticsAllowed()) return null;
        return new android.os.ResultReceiver(main) {
            @Override protected void onReceiveResult(int code, android.os.Bundle ignored) {
                DiagnosticEventKind kind = VoiceDiagnostics.fromCode(code);
                if (kind != null && voiceDiagnosticsAllowed()) imeDebugOverlay.recordEvent(kind);
            }
        };
    }

    private boolean voiceDiagnosticsAllowed() {
        if (!imeDebugOverlay.logging()) return false;
        // 窗口接手后键盘的偏好轮询可能已停；不能只信启动前的内存快照。
        File files = getFilesDir();
        if (files == null) return false;
        AndroidLocalSettings.Snapshot current = AndroidLocalSettings.load(new File(
            HostOptionsPolicy.bootstrapStateDirectory(files), AndroidLocalSettings.FILE_NAME).toPath());
        return current.bool(AndroidLocalSettings.DEVELOPER_INPUT_LOG)
            && !current.bool(AndroidLocalSettings.INCOGNITO);
    }

    private void insertVoiceResult() {
        VoiceResultStore.Entry entry = voiceResultEntry;
        if (entry == null || voiceResultStore == null) return;
        if (!voiceInsertionReady() || !voiceTargetMatches()) {
            notice("输入位置已变化，请关闭后重新打开语音结果");
            return;
        }
        try {
            String text = voiceResultStore.consume(entry.id(), System.currentTimeMillis());
            boolean committed;
            committed = commitText(text, TypingSource.VOICE);
            closeVoiceResult();
            if (!committed)
                notice("编辑器拒绝插入；结果已安全清除");
        } catch (VoiceResultStore.Failure error) {
            voiceResultEntry = null;
            renderVoiceResult();
            notice(error.reason() == VoiceResultStore.Reason.BUSY
                ? "语音结果正在更新，请稍后重试" : "语音结果已过期、已使用或不可读取");
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
            notice("无法打开水杉输入法，请从主屏幕进入");
            return;
        }
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        try {
            startActivity(intent);
        } catch (RuntimeException error) {
            notice("无法打开水杉输入法，请从主屏幕进入");
        }
    }

    /** 只有待插入的识别文字仍有效时才提供菜单入口，不把空结果页当常驻工具。 */
    boolean hasPendingVoiceResult() {
        if (voiceResultStore == null) return false;
        try { return voiceResultStore.read(System.currentTimeMillis()) != null; }
        catch (VoiceResultStore.Failure ignored) { return false; }
    }

    void showVoiceResult() {
        if (!voiceInsertionReady()) {
            notice("请先完成当前输入，再插入语音结果");
            return;
        }
        if (voiceResultStore == null || voiceResultScroll == null) {
            notice("语音结果存储尚未就绪");
            return;
        }
        try { voiceResultEntry = voiceResultStore.read(System.currentTimeMillis()); }
        catch (VoiceResultStore.Failure error) {
            notice(error.reason() == VoiceResultStore.Reason.BUSY
                ? "语音结果正在更新，请稍后重试" : "语音结果无法读取");
            return;
        }
        captureVoiceTarget();
        voiceSetupReason = null;
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

    /** 语音输入还不能用（关着，或者没有任何识别服务）时，在键区上打开引导：说明原因，按钮直达设置的「语音输入」页，本地模型的下载、按流量计费网络的确认和语音运行库都在那一页处理，键盘里不另做一套。 */
    private void showVoiceSetup(String reason) {
        if (voiceResultScroll == null) {
            notice(reason);
            return;
        }
        closeEmojiPicker();
        closeSymbolPanel();
        closeCandidatePanel();
        closeClipboardHistory();
        closeSchemePicker();
        closeLayoutSettings();
        closeAiPolish();
        closeVoiceResult();
        voiceSetupReason = reason;
        renderVoiceResult();
        ViewPolicy.show(voiceResultScroll);
        render();
    }

    private void renderVoiceResult() {
        if (voiceResultPanel == null) return;
        voiceResultPanel.removeAllViews();
        if (voiceSetupReason != null) {
            LinearLayout header = LayoutPolicy.row(this);
            TextView title = ViewPolicy.textLabel(this, "语音输入", 18);
            KeyboardTypographyPolicy.setKeyTextSize(title, 18);
            header.addView(title, LayoutPolicy.weightedWrapParams(1));
            button(header, "返回键盘", this::closeVoiceResult);
            voiceResultPanel.addView(header);
            voiceResultPanel.addView(textView(voiceSetupReason));
            Button setup = button(voiceResultPanel, "设置语音输入", () -> {
                closeVoiceResult();
                // 直接打开「服务」：在那里选「本地模型」后出现模型下载，或者选在线服务商。
                android.os.Bundle args = new android.os.Bundle();
                args.putString(HostDeepLink.ARG_SECTION, "服务");
                openHostPage("VOICE", args);
            });
            setup.setLayoutParams(LayoutPolicy.matchWidthWrapParams());
            imeStyler.applySkin();
            return;
        }
        LinearLayout header = LayoutPolicy.row(this);
        TextView title = ViewPolicy.textLabel(this, "语音结果", 18);
        KeyboardTypographyPolicy.setKeyTextSize(title, 18);
        header.addView(title, LayoutPolicy.weightedWrapParams(1));
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
            insert.setLayoutParams(LayoutPolicy.matchWidthWrapParams());
        }
        Button recognize = button(voiceResultPanel, "开始语音识别",
            this::startVoiceRecognition);
        recognize.setLayoutParams(LayoutPolicy.matchWidthWrapParams());
        VoiceConfiguration configured = VoiceConfiguration.read(preferencesDirectory, "ime-preview");
        boolean platformRecognizerAvailable = VoiceRecognitionActivity.available(this);
        ViewPolicy.setEnabled(recognize, voiceInputEnabled
            && (platformRecognizerAvailable || configured.provider() != null));
        imeStyler.applySkin();
    }

    private TextView textView(CharSequence text) {
        TextView view = ViewPolicy.newTextView(this, text);
        KeyboardTypographyPolicy.setKeyTextSize(view, KeyboardTypographyPolicy.DEFAULT_KEY_TEXT_SP);
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
            keySpacingValue.setText(KeyboardSpacingPolicy.display(touchKeySpacingTenths) + " dp");
            rowSpacingValue.setText(KeyboardSpacingPolicy.display(touchRowSpacingTenths) + " dp");
            keyboardHeightValue.setText(KeyboardHeightPolicy.displayHeight(
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
        if (keySpacing) touchKeySpacingTenths = KeyboardSpacingPolicy.keySpacing(value);
        else touchRowSpacingTenths = KeyboardSpacingPolicy.rowSpacing(value);
        renderLayoutSettingsState();
        imeStyler.applyKeyboardGeometry();
    }

    private void previewTouchHeight(int value) {
        if (touchGeometrySaving || traditionalOutputSaving) return;
        touchKeyboardHeightAdjustment = KeyboardHeightPolicy.heightAdjustment(value);
        renderLayoutSettingsState();
        imeStyler.applyKeyboardGeometry();
    }

    private void configureSpacingSlider(SeekBar slider, boolean keySpacing) {
        slider.setMin(keySpacing ? KeyboardSpacingPolicy.MIN_KEY_SPACING_TENTHS
            : KeyboardSpacingPolicy.MIN_ROW_SPACING_TENTHS);
        slider.setMax(keySpacing ? KeyboardSpacingPolicy.MAX_KEY_SPACING_TENTHS
            : KeyboardSpacingPolicy.MAX_ROW_SPACING_TENTHS);
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
        slider.setMin(KeyboardHeightPolicy.MIN_HEIGHT_ADJUSTMENT_DP);
        slider.setMax(KeyboardHeightPolicy.MAX_HEIGHT_ADJUSTMENT_DP);
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
            notice("键盘设置尚未就绪");
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
        touchKeySpacingTenths = KeyboardSpacingPolicy.DEFAULT_KEY_SPACING_TENTHS;
        touchRowSpacingTenths = KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS;
        touchKeyboardHeightAdjustment = KeyboardHeightPolicy.DEFAULT_HEIGHT_ADJUSTMENT_DP;
        touchVoiceShortcutEnabled = false;
        imeStyler.applyKeyboardGeometry();
        saveTouchGeometry(true);
    }

    private void saveTouchGeometry(boolean reset) {
        saveTouchGeometry(reset, java.util.Map.of());
    }

    /**
     * 保存键盘几何：高度和 {@code localEdits}（「键盘调整」确认时的底部抬高、宽度和位置）在同一次原子写里进本地设置，间距和语音入口进共享文档。
     */
    private void saveTouchGeometry(boolean reset, java.util.Map<String, Object> localEdits) {
        if (touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) return;
        JSONObject acceptedPreferences = preferencesSnapshot.optJSONObject("preferences");
        // 高度在本地设置里，间距和语音入口在共享文档里，各自只在变了时写：只调高度时不去碰共享文档，免得一次无谓的 revision 冲突让保存失败。
        boolean sharedChanged = reset || acceptedPreferences == null
            || KeyboardSpacingPolicy.keySpacing(NumberPolicy.strictInt(
                acceptedPreferences, "touch_key_spacing_tenths", -1)) != touchKeySpacingTenths
            || KeyboardSpacingPolicy.rowSpacing(NumberPolicy.strictInt(
                acceptedPreferences, "touch_row_spacing_tenths", -1)) != touchRowSpacingTenths
            || acceptedPreferences.optBoolean("touch_voice_shortcut", false)
                != touchVoiceShortcutEnabled;
        boolean heightChanged = reset || acceptedPreferences == null
            || heightAdjustmentFrom(acceptedPreferences) != touchKeyboardHeightAdjustment;
        if (!sharedChanged && !heightChanged && localEdits.isEmpty()) return;
        final long targetSession = session;
        final String targetDirectory = preferencesDirectory;
        JSONObject pending = null;
        long expectedRevision = -1;
        if (sharedChanged) {
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
        }
        touchGeometrySaving = true;
        preferencesNotice = " · 正在保存键盘设置";
        final long operation = ++preferenceSaveGeneration;
        // 高度是设计范围（75%..170%），共享偏好放不下，写进本地设置；恢复默认时删掉本地值。
        final Integer height = reset ? null : touchKeyboardHeightAdjustment;
        final java.nio.file.Path localSettingsFile = AndroidLocalSettings.file(this);
        final JSONObject sharedDocument = pending;
        final long sharedRevision = expectedRevision;
        final java.util.Map<String, Object> layoutEdits = new java.util.HashMap<>(localEdits);
        KeyboardHeightSave.HeightWriter writeHeight = heightChanged || !layoutEdits.isEmpty() ? () -> {
            java.util.Map<String, Object> edits = new java.util.HashMap<>(layoutEdits);
            if (heightChanged) edits.put(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT, height);
            AndroidLocalSettings.update(localSettingsFile, edits);
        } : null;
        java.util.function.Supplier<String> writeShared = sharedDocument == null ? null : () -> {
            try {
                return NativeClient.savePreferences(targetDirectory, sharedRevision,
                    sharedDocument.toString());
            } catch (Exception | LinkageError error) {
                return null;
            }
        };
        renderLayoutSettingsState();
        render();
        Runnable save = () -> {
            KeyboardHeightSave.Result result = KeyboardHeightSave.run(writeHeight, writeShared);
            main.post(() -> finishTouchGeometrySave(operation, targetSession, targetDirectory,
                reset, result));
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
                                         String targetDirectory, boolean reset,
                                         KeyboardHeightSave.Result result) {
        if (operation != preferenceSaveGeneration || session != targetSession
                || !targetDirectory.equals(preferencesDirectory)) return;
        touchGeometrySaving = false;
        // 后台已经先写了本地高度，不论共享文档写没写成都先读回来：下面按快照重算布局时，高度取的是刚写下的值，而不是内存里那份旧的本地设置。
        refreshLocalSettings();
        if (!result.heightSaved())
            notice("键盘高度未能保存");
        if (!result.sharedAttempted()) {
            if (preferencesSnapshot != null)
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
            imeStyler.applyKeyboardGeometry();
            preferencesNotice = result.heightSaved() ? " · 键盘设置已保存" : " · 键盘高度保存失败，已恢复原设置";
            renderLayoutSettingsState();
            render();
            return;
        }
        try {
            String response = result.sharedResponse();
            if (response == null) throw new JSONException("Preferences save unavailable");
            JSONObject saved = value(response);
            long savedRevision = PreferencesRevisionPolicy.read(saved.opt("revision"), -1);
            if (savedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (preferencesSnapshot != null
                    && PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1)
                        > savedRevision) {
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
                imeStyler.applyKeyboardGeometry();
                preferencesNotice = "";
            } else {
                applyPreferencesSnapshot(saved);
                preferencesNotice = reset ? " · 键盘设置已恢复默认" : " · 键盘设置已保存";
            }
        } catch (JSONException | LinkageError error) {
            // 间距和语音入口回到已接受的快照；高度已在上面从本地设置读回，保留用户刚调好的值。
            if (preferencesSnapshot != null)
                applyTouchGeometry(preferencesSnapshot.optJSONObject("preferences"));
            imeStyler.applyKeyboardGeometry();
            preferencesNotice = reset ? " · 恢复默认失败，已恢复原设置" : " · 键盘设置保存失败，已恢复原设置";
            notice(reset ? "键盘设置未能恢复默认" : "键盘设置未能保存");
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
        selectKeyboardScheme(scheme, false);
    }

    /**
     * 换到 `scheme`；`english` 为真时保存成功后进英文（手写磁贴回到进入手写前的英文键盘），否则停在中文。
     *
     * @return 是否接受了这次选择：保存进行中、会话或偏好未就绪时什么都不做，返回假。
     */
    private boolean selectKeyboardScheme(KeyboardScheme scheme, boolean english) {
        if (schemeSaving || touchGeometrySaving || traditionalOutputSaving
                || session == 0 || preferencesSnapshot == null
                || preferencesDirectory.isEmpty()) return false;
        // 目标方案不变时只退出英文；更换方案时保留英文键盘到保存成功，避免异步等待期间显示中文中间帧。
        if (scheme == selectedScheme) {
            if (dedicatedEnglish) toggleInputLanguage();
            if (session == 0) return true;
            closeSchemePicker();
            render();
            return true;
        }
        // 换到别的方案：手写磁贴记下的入口作废，磁贴自己在这之后重新记。
        schemeBeforeHandwriting = null;
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
                    || !targetDirectory.equals(preferencesDirectory)) return false;
            pending = new JSONObject(preferencesSnapshot.toString());
            expectedRevision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            if (expectedRevision < 0) throw new JSONException("Invalid preferences revision");
            JSONObject preferences = pending.getJSONObject("preferences");
            String currentScheme = preferences.optString("scheme", edition.defaultScheme());
            String lastChinese = InputViewValuePolicy.textOr(preferences, "last_chinese_scheme", currentScheme);
            KeyboardScheme.PreferenceMapping mapping = scheme.mapping(lastChinese,
                InputViewValuePolicy.textOr(preferences, "shuangpin_profile", "xiaohe"), edition);
            preferences.put("scheme", mapping.scheme());
            preferences.put("last_chinese_scheme", mapping.lastChineseScheme());
            preferences.put("shuangpin_profile", mapping.shuangpinProfile());
            preferences.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
            JSONArray enabled = new JSONArray();
            for (KeyboardScheme candidate : enabledSchemes) {
                enabled.put(candidate.preferenceId());
            }
            // 调用方显式选中新方案时一并启用，保证 `selected` 始终指向启用列表内的方案。
            if (!enabledSchemes.contains(scheme)) enabled.put(scheme.preferenceId());
            preferences.put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", enabled).put("selected", scheme.preferenceId()));
        } catch (JSONException | LinkageError error) {
            preferencesNotice = " · 输入方案切换失败，保留当前设置";
            closeSchemePicker();
            render();
            return false;
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
            main.post(() -> finishSchemeSave(operation, targetSession, targetDirectory, savedResponse, english));
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
        return true;
    }

    private void finishSchemeSave(long operation, long targetSession, String targetDirectory,
                                  String response, boolean english) {
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
                // 英文模式和新方案在同一次主线程回调里应用，只让最终键盘进入下一帧。
                if (dedicatedEnglish) {
                    toggleInputLanguage(false);
                    if (session != targetSession) return;
                }
                applyPreferencesSnapshot(saved);
                // 回到进入手写前的英文键盘：新方案已经应用，再在同一次回调里进英文，中间不画中文帧。
                if (english && !dedicatedEnglish) {
                    toggleInputLanguage(false);
                    if (session != targetSession) return;
                }
                preferencesNotice = " · 输入方案已切换";
            }
        } catch (JSONException | LinkageError error) {
            // A conflict or storage failure leaves the working session unchanged.
            preferencesNotice = " · 输入方案切换失败，保留当前设置";
            notice("输入方案未能保存");
        }
        synchronizeReplyKeyboard();
        render();
    }

    void insertClipboardText(String text) {
        // 这里只判断是否有文字；重复检查共享存储的长度限制会让其他移动宿主保存的条目可见却无法插入。
        if (connection == null || !ClipboardHistoryPolicy.hasText(text)) return;
        command(2);
        commitText(text);
        // 用户已经从剪贴板插入过了（面板里的历史、分词或工具栏上的「最近复制」），工具栏不再提供刚复制的那一条：否则关上面板后工具栏又换成刚插入的文字，剪贴板和表情按钮也跟着被替换掉。
        recentClip.dismiss();
        closeClipboardHistory();
        render();
    }

    /** 系统剪贴板里当前的那一条文字：文字本身、系统给的复制时刻（读不到时为 0）和是否被标为敏感。 */
    record PrimaryClip(String text, long copiedAtMs, boolean sensitive) {
        String identity() { return ClipboardCapturePolicy.identity(copiedAtMs, text); }
    }

    /**
     * 读系统剪贴板当前的那一条文字；没有剪贴板内容、不是文本或文字为空时为 null。
     *
     * <p>系统标记为敏感的内容（密码管理器复制的密码，Android 13 起的 `EXTRA_IS_SENSITIVE`）照样返回，由调用方决定怎么对待。
     */
    PrimaryClip readPrimaryClip() {
        ClipboardManager manager = getSystemService(ClipboardManager.class);
        ClipData clip = manager == null || !manager.hasPrimaryClip() ? null : manager.getPrimaryClip();
        ClipDescription description = clip == null ? null : clip.getDescription();
        if (clip == null || clip.getItemCount() == 0 || description == null
                || !(description.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                    || description.hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML))) return null;
        CharSequence value = clip.getItemAt(0).getText();
        String text = value == null ? null : value.toString();
        if (!ClipboardHistoryPolicy.hasText(text)) return null;
        boolean sensitive = description.getExtras() != null
            && description.getExtras().getBoolean(ClipDescription.EXTRA_IS_SENSITIVE, false);
        return new PrimaryClip(text, description.getTimestamp(), sensitive);
    }

    /** 系统剪贴板里当前的那一张图片：content URI、具体图片类型、复制时刻（读不到时为 0）和是否被标为敏感（#6781）。 */
    record PrimaryImage(Uri uri, String mimeType, long copiedAtMs, boolean sensitive) {
        String identity() {
            return ClipboardCapturePolicy.identity(copiedAtMs,
                ClipboardImagePolicy.identityText(uri.toString(), mimeType));
        }
    }

    /**
     * 读系统剪贴板当前的那一张图片；没有剪贴板内容、第一项不是 content URI、描述里没有具体的图片类型时为 null。只读描述和 URI，不打开图片。
     *
     * <p>和 {@link #readPrimaryClip()} 一样，敏感标记照样返回，由调用方决定怎么对待。
     */
    PrimaryImage readPrimaryImage() {
        ClipboardManager manager = getSystemService(ClipboardManager.class);
        ClipData clip = manager == null || !manager.hasPrimaryClip() ? null : manager.getPrimaryClip();
        ClipDescription description = clip == null ? null : clip.getDescription();
        if (clip == null || clip.getItemCount() == 0 || description == null) return null;
        String[] types = new String[description.getMimeTypeCount()];
        for (int index = 0; index < types.length; index++) types[index] = description.getMimeType(index);
        String mimeType = ClipboardImagePolicy.imageMimeType(types);
        Uri uri = clip.getItemAt(0).getUri();
        if (mimeType == null || uri == null || !"content".equals(uri.getScheme())) return null;
        boolean sensitive = description.getExtras() != null
            && description.getExtras().getBoolean(ClipDescription.EXTRA_IS_SENSITIVE, false);
        return new PrimaryImage(uri, mimeType, description.getTimestamp(), sensitive);
    }

    /**
     * 把当前剪贴板文本记进本机历史。
     *
     * <p>Android 的默认输入法本来就能读剪贴板，所以和 Gboard 一样，复制之后自动记下（{@link #clipboardWatcher}），打开面板时再补读一次（键盘进程没在运行时复制的那一条）。补读只记还没处理过的那一条（{@link ClipboardCapturePolicy}），否则刚清空或删掉的内容会在下一次打开面板时回来（#5605）。`announce` 为假时一律不弹提示：自动记录被隐私规则挡下、内容为空或重复都是正常情况。系统标记为敏感的内容从不记录。
     */
    void captureClipboard(ClipboardCapturePolicy.Trigger trigger, boolean announce) {
        if (!imePrivacyGate.allows(ImePrivacyGate.Record.CLIPBOARD_HISTORY)) {
            if (announce) notice("隐私模式或当前输入框下不保存剪贴板");
            return;
        }
        if (!imePrivacyGate.capturesClipboard()) return;
        try {
            PrimaryClip clip = readPrimaryClip();
            if (clip == null) {
                if (announce) notice(ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.Rejection.EMPTY, clipboardHistory.limit()));
                return;
            }
            if (clip.sensitive()) return;
            String identity = clip.identity();
            if (!ClipboardCapturePolicy.captures(trigger, identity, handledClipIdentity())) return;
            // The shared store refuses rather than throws, and says which refusal it is. Deciding
            // that here as well is what made this host disagree with the store it writes into.
            String reason = clipboardHistory.add(clip.text());
            // 存储有了答复才记为已处理：收下了，或者明确拒收（拒收的内容同样不该在每次打开面板时再试一遍）。存储写不进去时 add 会抛异常，那一条不记，下次打开面板补读时还会再试，否则它就永远进不了历史。
            rememberHandledClip(identity);
            if (reason != null) {
                if (announce) notice(ClipboardHistoryPolicy.message(
                    ClipboardHistoryPolicy.rejectionFor(reason), clipboardHistory.limit()));
                return;
            }
            if (imePanels.clipboardPanelOpen()) imePanels.renderClipboardHistory();
        } catch (IllegalArgumentException | IllegalStateException | SecurityException error) {
            if (announce) notice("无法保存当前剪贴板");
        }
    }

    /** 上一次处理过的剪贴板身份，存在本进程的 SharedPreferences 里，键盘进程被回收后仍然记得；只有身份，没有文字。 */
    private String handledClipIdentity() {
        return getSharedPreferences(CLIPBOARD_CAPTURE_PREFERENCES, MODE_PRIVATE)
            .getString(HANDLED_CLIP_KEY, null);
    }

    private void rememberHandledClip(String identity) {
        getSharedPreferences(CLIPBOARD_CAPTURE_PREFERENCES, MODE_PRIVATE).edit()
            .putString(HANDLED_CLIP_KEY, identity).apply();
    }

    /**
     * 把系统剪贴板里刚复制的那一条交给工具栏的「最近复制」（#5692）；复制的是一张图片时交的是它的身份和类型（#6781），发不发得出去由 {@link #recentImageSendable} 按当前输入框决定。
     *
     * <p>`copiedNow` 为真时是复制监听：这一下就是用户刚复制的，系统给不出复制时刻也按现在算。为假时是弹出键盘时的补看：先只读剪贴板的描述（不读内容）看复制时刻，过了显示窗口就不读内容；读不到复制时刻时不提供，免得一条很久以前的内容被当成刚复制的。系统标为敏感的内容、剪贴板历史或本地开关关着、隐私模式和密码类输入框都不提供，前两种连剪贴板都不读。
     */
    void offerRecentClip(boolean copiedNow) {
        if (!recentClipEnabled() || !imePrivacyGate.allows(ImePrivacyGate.Record.CLIPBOARD_HISTORY)) return;
        try {
            ClipboardManager manager = getSystemService(ClipboardManager.class);
            if (manager == null || !manager.hasPrimaryClip()) return;
            long now = System.currentTimeMillis();
            ClipDescription description = manager.getPrimaryClipDescription();
            if (description == null) return;
            long copiedAt = description.getTimestamp();
            if (copiedAt <= 0 && copiedNow) copiedAt = now;
            if (!RecentClipboardSuggestion.fresh(copiedAt, now)) return;
            PrimaryClip clip = readPrimaryClip();
            // 不是文字时看是不是一张图片（#6781）：同样丢弃敏感内容，只记身份和类型。
            PrimaryImage image = clip == null ? readPrimaryImage() : null;
            if (image != null) {
                if (image.sensitive()) return;
                // 键盘自己放进剪贴板的表情包贴纸不是用户复制的图片，不在工具栏上再提供一次。
                if (imeStickers.ownsClip(image.uri())) return;
                recentClip.offerImage(image.identity(), image.mimeType(), copiedAt);
            } else {
                if (clip == null || clip.sensitive()) return;
                recentClip.offer(clip.identity(), clip.text(), copiedAt);
            }
        } catch (SecurityException error) {
            return;
        }
        // 弹出键盘时的补看紧跟着就会 render，这里只为复制监听重画。
        if (copiedNow) render();
    }

    /** 工具栏是否提供「最近复制」：剪贴板历史和本地开关都开着，见 {@link RecentClipboardSuggestion#enabled}。 */
    private boolean recentClipEnabled() {
        return RecentClipboardSuggestion.enabled(clipboardHistoryEnabled, clipboardSuggestionEnabled);
    }

    /**
     * 剪贴板历史刚由实时偏好变成开着：补看一次刚复制的那一条。键盘进程冷启动时开关在实时偏好到来前是字段初始值（关），弹出键盘时的补看因此跳过了，不补的话键盘进程不在时复制的那一条就不会出现。调用方随后会重画。
     */
    private void offerRecentClipOnHistoryEnabled(boolean previousClipboard) {
        if (!previousClipboard && clipboardHistoryEnabled) offerRecentClip(false);
    }

    /** 点工具栏上的「最近复制」：粘贴它（是图片时发送它），这一条不再出现。 */
    void pasteRecentClip() {
        long now = System.currentTimeMillis();
        if (recentClip.imageMimeType(now) != null) {
            sendRecentImage(now);
            return;
        }
        String text = recentClip.text(now);
        recentClip.dismiss();
        if (text != null) insertClipboardText(text);
        render();
    }

    /**
     * 「最近复制」的图片现在能不能发到当前输入框（#6781）：有一张还没过期的图片，并且输入框经 `EditorInfo.contentMimeTypes` 声明收这种图片。不收的输入框（很多聊天应用、所有不支持富内容的输入框）里不显示，图片仍在系统剪贴板上，长按输入框粘贴照样能用。
     */
    boolean recentImageSendable(long nowMs) {
        String mimeType = recentClip.imageMimeType(nowMs);
        EditorInfo info = getCurrentInputEditorInfo();
        return mimeType != null && info != null
            && ClipboardImagePolicy.accepts(info.contentMimeTypes, mimeType)
            && clipboardImageExtension(mimeType) != null;
    }

    /**
     * 缓存副本用的扩展名；这种图片发出去后收图应用看到的不是图片类型时为 null，这时不提供发送（#6781）。
     *
     * <p>FileProvider 报给收图应用的类型（`ContentResolver.getType`）只按文件扩展名经 `MimeTypeMap` 反查，查不到就是 `application/octet-stream`。所以扩展名先取系统表给这种类型的那个，系统表不认时用 {@link ClipboardImagePolicy#extension} 的常见写法补位，最后按 FileProvider 的做法反查一遍：反查出来不是图片，收图应用拿到的类型就和 commitContent 里声明的对不上，按类型校验的应用会拒收或当成普通文件。这种图仍在系统剪贴板上，长按输入框粘贴照样能用。
     */
    static String clipboardImageExtension(String mimeType) {
        MimeTypeMap types = MimeTypeMap.getSingleton();
        String extension = types.getExtensionFromMimeType(mimeType);
        if (extension == null) extension = ClipboardImagePolicy.extension(mimeType);
        if (extension == null) return null;
        String served = types.getMimeTypeFromExtension(extension);
        return served != null && served.startsWith("image/") ? extension : null;
    }

    /**
     * 把「最近复制」的图片发到当前输入框（#6781）。
     *
     * <p>先在主线程上核对：输入框还收这种图片、隐私规则允许、剪贴板里还是同一张（身份一致，没有被清空或换成别的）、没有被标为敏感。然后在 {@link #clipboardImageWorker} 上把图读进缓存副本（{@link ClipboardImageCache}），回到主线程用 `commitContent` 交给输入框。这一条点过就收起，不论成败。
     */
    private void sendRecentImage(long nowMs) {
        String identity = recentClip.identity(nowMs);
        String mimeType = recentClip.imageMimeType(nowMs);
        boolean sendable = recentImageSendable(nowMs);
        recentClip.dismiss();
        render();
        if (identity == null || mimeType == null || connection == null
                || !imePrivacyGate.allows(ImePrivacyGate.Record.CLIPBOARD_HISTORY)) return;
        if (!sendable) {
            notice("这个输入框不支持直接发送图片，可以长按输入框粘贴");
            return;
        }
        PrimaryImage image;
        try {
            image = readPrimaryImage();
        } catch (SecurityException error) {
            image = null;
        }
        if (image == null || image.sensitive() || !identity.equals(image.identity())) {
            notice("复制的图片已不在剪贴板里");
            return;
        }
        final long generation = clipboardImageGeneration;
        final File directory = new File(getCacheDir(), ClipboardImagePolicy.DIRECTORY);
        final android.content.ContentResolver resolver = getContentResolver();
        final Uri source = image.uri();
        final String extension = clipboardImageExtension(mimeType);
        clipboardImageWorker.execute(() -> {
            File copy = null;
            String failure = null;
            try (InputStream input = resolver.openInputStream(source)) {
                copy = ClipboardImageCache.replace(input, directory, extension, ClipboardImagePolicy.MAX_BYTES);
            } catch (ClipboardImageCache.TooLargeException error) {
                failure = "图片太大，不能直接发送，可以长按输入框粘贴";
            } catch (IOException | RuntimeException error) {
                // 图片来自别的应用的 provider，它在 openFile 里抛的 NullPointerException、IllegalStateException 等会经 binder 原样抛到这里；这是系统边界，漏掉一个就会让整个键盘进程崩掉、这一次也收不到提示。
                failure = "无法读取复制的图片";
            }
            final File sent = copy;
            final String reason = failure;
            main.post(() -> commitClipboardImage(generation, sent, mimeType, reason));
        });
    }

    /** {@link #sendRecentImage} 在主线程上的后半段：输入框没变时把副本经 FileProvider 交给它，并安排过一会儿删掉副本。 */
    private void commitClipboardImage(long generation, File copy, String mimeType, String failure) {
        if (generation != clipboardImageGeneration || connection == null) {
            // 读图期间换了输入框或收起了键盘：这张不再发，副本也不留。
            clearClipboardImages();
            return;
        }
        if (failure != null || copy == null) {
            notice(failure == null ? "无法读取复制的图片" : failure);
            return;
        }
        boolean sent;
        try {
            Uri uri = ClipboardImageCache.contentUri(this, copy);
            InputContentInfo content = new InputContentInfo(uri,
                new ClipDescription(ClipboardImagePolicy.LABEL, new String[] {mimeType}));
            sent = InputConnectionGuard.commitContent(connection, content,
                InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION, null);
        } catch (IOException | IllegalArgumentException error) {
            sent = false;
        }
        main.removeCallbacks(clipboardImageCleanup);
        if (!sent) {
            clearClipboardImages();
            notice("这个输入框没有收下图片，可以长按输入框粘贴");
            return;
        }
        main.postDelayed(clipboardImageCleanup, ClipboardImagePolicy.RETENTION_MS);
    }

    /** 在 {@link #clipboardImageWorker} 上清空缓存里的图片副本；键盘进程正在退出、线程已经停了时什么也不做（退出前已经排过一次清理）。 */
    private void clearClipboardImages() {
        if (clipboardImageWorker.isShutdown()) return;
        File directory = new File(getCacheDir(), ClipboardImagePolicy.DIRECTORY);
        clipboardImageWorker.execute(() -> ClipboardImageCache.clear(directory));
    }

    void dismissRecentClip() {
        recentClip.dismiss();
        render();
    }

    /** 最近复制那一行现在该显示的快捷键（#10079），见 {@link RecentClipboardSuggestion#quickAction}；`image` 为真时这一条是图片。 */
    String recentClipQuickAction(boolean image) {
        return RecentClipboardSuggestion.quickAction(clipboardSuggestionActionEnabled,
            clipboardSuggestionActionKind, image);
    }

    /**
     * 点最近复制那一行的分词或分类快捷键（#10079）：收起别的面板，打开剪贴板面板的本机历史，直接进入这一条的分词点选或「移动到分类」，省去进面板、长按、再选操作这几步。真正进入了分词或分类时这一条算用过，和粘贴一样不再出现；没进去（这一条不在历史里，不能移动分类）时保留它，关掉面板后还能点它粘贴。
     */
    void runRecentClipQuickAction() {
        String text = recentClip.text(System.currentTimeMillis());
        String action = recentClipQuickAction(false);
        if (text == null || action == null) {
            render();
            return;
        }
        closeToolbarPanels();
        if (imePanels.openRecentClipAction(text, RecentClipboardSuggestion.ACTION_SEGMENT.equals(action)))
            recentClip.dismiss();
        render();
    }

    /**
     * 用户删掉或清空历史后，把系统剪贴板里当前那一条记为已处理，下一次打开面板的补读就不会把它记回来。
     *
     * <p>系统标为敏感的内容跳过：补读从不记录它，没有东西需要挡；身份里有文字的散列和长度，短密码、验证码凭这两样就能穷举还原，不能为它落盘。
     */
    private void forgetCurrentClip() {
        try {
            PrimaryClip clip = readPrimaryClip();
            if (clip != null && !clip.sensitive()) rememberHandledClip(clip.identity());
        } catch (SecurityException error) {
            // 读不到剪贴板就没有东西会被补读回来。
        }
    }

    /**
     * 固定或取消固定一条历史。
     *
     * <p>这一条的操作（固定、删除、分词、发到云剪贴板）原来是挂在卡片上的 `PopupMenu`。弹出菜单是可获得焦点的窗口，会把窗口焦点从编辑器拿走：在 Via 这类用系统 WebView 的浏览器里，菜单一出现键盘就被收起（#5653），Firefox 这类自带引擎的不会，应是 WebView 在窗口失焦时让输入框失焦。操作现在画在面板里卡片下方（{@link ImePanels#renderClipboardHistory}），不再弹出任何窗口。
     */
    void setClipboardItemPinned(ClipboardHistory.Item item) {
        if (clipboardHistory == null) return;
        try {
            clipboardHistory.setPinned(item.text(), !item.pinned());
        } catch (IllegalStateException error) {
            notice("无法修改剪贴板历史");
        }
        imePanels.renderClipboardHistory();
    }

    /**
     * 删除一条：长按操作行里的「删除」和左滑露出的垃圾桶都走这里。
     *
     * <p>和多选删除一样走共享存储的 `remove_many`，拿到删除前的完整条目（文字、时间戳、固定状态、分类），删除后同样可以在「撤销时长」内撤销（#10080）；共享层单条的 `remove` 只回答删没删，不交回快照，撤销时只能按文字重新加回，时间戳和固定状态都会丢。
     */
    void removeClipboardItem(ClipboardHistory.Item item) {
        if (clipboardHistory == null) return;
        authenticateClipboardDelete(() -> removeClipboardItemAfterAuthentication(item));
    }

    private void removeClipboardItemAfterAuthentication(ClipboardHistory.Item item) {
        if (clipboardHistory == null) return;
        try {
            holdClipboardUndo(clipboardHistory.removeMany(java.util.List.of(item.text())));
            forgetCurrentClip();
        } catch (IllegalStateException error) {
            notice("无法修改剪贴板历史");
        }
        imePanels.renderClipboardHistory();
    }

    /**
     * 原子删除多选记录；排序、时间戳和固定规则仍由共享存储决定，删除后的撤销见 {@link #holdClipboardUndo}。
     *
     * <p>整批只认证一次。`removed` 只在认证通过（或不需要认证）、真正发起删除之后执行，面板借它退出多选；认证取消或失败时不调用它，选择原样保留。
     */
    void removeClipboardItems(java.util.List<String> texts, Runnable removed) {
        if (clipboardHistory == null || texts == null || texts.isEmpty()) return;
        java.util.List<String> snapshot = java.util.List.copyOf(texts);
        authenticateClipboardDelete(() -> {
            removeClipboardItemsAfterAuthentication(snapshot);
            removed.run();
        });
    }

    /**
     * 单条、左滑和多选删除共用的认证门禁（#10408），规则见 {@link ClipboardDeleteAuthenticationPolicy}。
     *
     * <p>设备没有任何锁屏凭据时，门禁只来自访客模式（用户没有打开「剪贴板删除数据验证」）就照旧直接删除：访客模式本来就要求认证，但没有凭据的设备谁都能解锁，拦下删除保护不了任何东西，只会让访客剪贴板删不掉，比改动之前更糟。用户自己打开过开关时仍然拒绝，并提示去系统设置开启指纹、图案或密码，或回设置页关闭开关（没有认证器时关闭开关不需要认证）。
     */
    private void authenticateClipboardDelete(Runnable delete) {
        boolean enabled = clipboardDeleteAuthenticationEnabled;
        if (!ClipboardDeleteAuthenticationPolicy.requiresAuthentication(enabled,
                clipboardMode == ClipboardModePolicy.Mode.VISITOR)) {
            delete.run();
            return;
        }
        authenticateClipboard("删除", delete,
            ClipboardDeleteAuthenticationPolicy.allowsWithoutAuthenticator(enabled)
                ? delete : () -> notice(CLIPBOARD_DELETE_UNAVAILABLE_NOTICE));
    }

    private void removeClipboardItemsAfterAuthentication(java.util.List<String> texts) {
        if (clipboardHistory == null || texts == null || texts.isEmpty()) return;
        try {
            if (!holdClipboardUndo(clipboardHistory.removeMany(texts))) return;
            forgetCurrentClip();
        } catch (IllegalStateException error) {
            notice("无法修改剪贴板历史");
        }
        imePanels.renderClipboardHistory();
    }

    /**
     * 记下刚删掉的条目供撤销，并从头开始倒数；单条删除和多选删除共用，返回是否真的删掉了记录。
     *
     * <p>删除后在设置的期限（「撤销时长」，5 到 9 秒，默认 5 秒，#10073）内可以撤销：剪贴板面板右下角浮出大号「撤销」按钮，每秒倒数一次，归零时按钮消失、记录不再能恢复。只留最近一次删除，新的删除替换还没到期的上一次。什么都没删掉时（记录已被别处删掉）不碰上一次的撤销，倒数照走。
     */
    private boolean holdClipboardUndo(java.util.List<ClipboardHistory.Item> removed) {
        if (removed.isEmpty()) return false;
        clipboardUndoTexts = removed;
        main.removeCallbacks(clipboardUndoTick);
        int seconds = ClipboardUndoPolicy.seconds(clipboardUndoSeconds);
        long now = SystemClock.uptimeMillis();
        clipboardUndoDeadline = ClipboardUndoPolicy.deadline(now, seconds);
        main.postDelayed(clipboardUndoTick, ClipboardUndoPolicy.millisUntilNextTick(clipboardUndoDeadline, now));
        notice(ClipboardUndoPolicy.deletedNotice(removed.size(), seconds));
        return true;
    }

    private java.util.List<ClipboardHistory.Item> clipboardUndoTexts;
    /** 撤销的截止时刻（`SystemClock.uptimeMillis`），只在 {@link #clipboardUndoTexts} 非空时有意义。 */
    private long clipboardUndoDeadline;
    /** 每秒刷新一次浮动「撤销」上的倒数，到截止时刻丢掉可撤销的记录、收起按钮。 */
    private final Runnable clipboardUndoTick = this::tickClipboardUndo;

    boolean hasClipboardUndo() {
        return clipboardUndoTexts != null && !clipboardUndoTexts.isEmpty();
    }

    /** 浮动「撤销」上显示的剩余秒数；没有可撤销的记录或已到期时为 0。 */
    int clipboardUndoRemainingSeconds() {
        if (!hasClipboardUndo()) return 0;
        return ClipboardUndoPolicy.remainingSeconds(clipboardUndoDeadline, SystemClock.uptimeMillis());
    }

    private void tickClipboardUndo() {
        long now = SystemClock.uptimeMillis();
        if (hasClipboardUndo() && ClipboardUndoPolicy.remainingSeconds(clipboardUndoDeadline, now) > 0) {
            main.postDelayed(clipboardUndoTick, ClipboardUndoPolicy.millisUntilNextTick(clipboardUndoDeadline, now));
        } else {
            clipboardUndoTexts = null;
        }
        // 撤销只在浮动按钮上，顶行和卡片不变，到期和倒数都只刷新这个按钮，不重画整个面板。
        imePanels.updateClipboardUndo();
    }

    void undoClipboardItems() {
        if (!hasClipboardUndo() || clipboardHistory == null) return;
        java.util.List<ClipboardHistory.Item> restore = clipboardUndoTexts;
        clipboardUndoTexts = null;
        main.removeCallbacks(clipboardUndoTick);
        try {
            clipboardHistory.restoreMany(restore);
            notice("已撤销删除");
        } catch (IllegalStateException error) {
            notice("无法撤销剪贴板删除");
        }
        imePanels.renderClipboardHistory();
    }

    /**
     * 长按操作行里的「编辑」（#5971）：打开应用里的编辑页，在那里改这一条的文字，确认后经共享存储的 `replace` 写回，条目留在原位。键盘里没有可输入的文本框，增删改放在应用里，和常用语（#5673）一样；面板里也不能弹对话框（#5605、#5653）。
     *
     * <p>交给编辑页的只是安装级令牌，不是文字本身。打开之前把系统剪贴板当前那一条记为已处理：原文多半还在系统剪贴板里，不记的话改完回来一打开面板，补读又把原文记了回来，编辑等于没生效。
     */
    void editClipboardItem(ClipboardHistory.Item item) {
        forgetCurrentClip();
        String key = ClipboardEditKey.editKey(this, item.timestamp(), item.text());
        if (key == null) {
            notice("剪贴板编辑暂时不可用");
            return;
        }
        android.os.Bundle args = new android.os.Bundle();
        args.putString(ClipboardHistoryPolicy.EDIT_ENTRY_ARG, key);
        putReturnToCaller(args);
        closeClipboardHistory();
        openHostPage(ClipboardHistoryPolicy.EDIT_PAGE, args);
    }

    /** 从键盘进入原生分类设置页，避免回程补读已处理过的系统剪贴板。 */
    void openClipboardCategories() {
        forgetCurrentClip();
        closeClipboardHistory();
        openHostPage("CLIPBOARD_CATEGORIES");
    }

    /**
     * 面板顶行的「搜索」（#5973）：打开应用里可搜索的剪贴板历史页，在那里输入查询、复制、编辑或删除。键盘里没有可输入的文本框，查询框放在应用里，和编辑（#5971）一样；页面读的是同一份共享存储。
     *
     * <p>打开之前和「编辑」一样把系统剪贴板当前那一条记为已处理：用户可能在那一页把它删掉或改掉，已处理身份存在本进程（`:ime`）的 SharedPreferences 里，应用进程写不了；不记的话回来一打开面板，补读又把它记了回来。
     */
    void openClipboardSearch() {
        authenticateClipboard("搜索", () -> {
            forgetCurrentClip();
            android.os.Bundle args = new android.os.Bundle();
            putReturnToCaller(args);
            closeClipboardHistory();
            openHostPage(ClipboardSearchPolicy.SEARCH_PAGE, args);
        });
    }

    private boolean clipboardExportBusy;
    private long clipboardAuthenticationGeneration;
    private boolean clipboardAuthenticationBusy;

    /** 认证桥 `ClipboardAuthenticationActivity` 的结果码；核心编译不依赖那个 Activity 类，这里按值对齐 `RESULT_AUTHENTICATED`、`RESULT_UNAVAILABLE`。 */
    private static final int CLIPBOARD_AUTH_SUCCEEDED = 2001;
    private static final int CLIPBOARD_AUTH_UNAVAILABLE = 2002;
    private static final String CLIPBOARD_AUTH_UNAVAILABLE_NOTICE = "这台设备没有可用的系统认证，请先在系统设置开启指纹、图案或密码";
    private static final String CLIPBOARD_DELETE_UNAVAILABLE_NOTICE = "这台设备没有可用的系统认证，请先在系统设置开启指纹、图案或密码，或在键盘设置的剪贴板里关闭「剪贴板删除数据验证」";

    private void authenticateClipboard(String operation, Runnable after) {
        authenticateClipboard(operation, after, null);
    }

    /** `whenUnavailable` 不为 null 时，设备没有可用认证器（桥返回 `RESULT_UNAVAILABLE`）就执行它，否则提示先设置锁屏凭据。 */
    private void authenticateClipboard(String operation, Runnable after, Runnable whenUnavailable) {
        if (clipboardAuthenticationBusy) return;
        clipboardAuthenticationBusy = true;
        long generation = ++clipboardAuthenticationGeneration;
        android.os.ResultReceiver receiver = new android.os.ResultReceiver(main) {
            @Override protected void onReceiveResult(int code, android.os.Bundle payload) {
                if (generation != clipboardAuthenticationGeneration) return;
                clipboardAuthenticationBusy = false;
                if (code == CLIPBOARD_AUTH_SUCCEEDED) after.run();
                else if (code == CLIPBOARD_AUTH_UNAVAILABLE) {
                    if (whenUnavailable != null) whenUnavailable.run();
                    else notice(CLIPBOARD_AUTH_UNAVAILABLE_NOTICE);
                } else notice("未通过系统认证，剪贴板操作已取消");
            }
        };
        try {
            Intent intent = new Intent().setClassName(getPackageName(),
                "app.msime.android.ClipboardAuthenticationActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
                .putExtra("app.msime.android.clipboard.AUTH_RESULT", receiver)
                .putExtra("app.msime.android.clipboard.AUTH_SUBTITLE", operation + "剪贴板历史");
            startActivity(intent);
        } catch (RuntimeException unavailable) {
            clipboardAuthenticationBusy = false;
            notice("这台设备没有可用的系统认证");
        }
    }

    /** 键盘右上方显式导出；旧系统首次授权交给原生历史页承接。 */
    void exportClipboardHistory() {
        authenticateClipboard("导出", this::exportClipboardHistoryAfterAuthentication);
    }

    /** 输入法面板多选导出；只接收面板当前选中的文字，不把未选记录带出。 */
    void exportClipboardItems(java.util.List<String> texts) {
        if (texts == null || texts.isEmpty()) return;
        if (texts.size() > app.msime.android.ClipboardSelectionPolicy.MAX_SELECTIONS) {
            notice("最多只能选择 " + app.msime.android.ClipboardSelectionPolicy.MAX_SELECTIONS + " 项");
            return;
        }
        java.util.List<String> snapshot = java.util.List.copyOf(texts);
        authenticateClipboard("导出", () -> exportClipboardItemsAfterAuthentication(snapshot));
    }

    private void exportClipboardHistoryAfterAuthentication() {
        if (clipboardExportBusy) return;
        clipboardExportBusy = true;
        Context application = getApplicationContext();
        preferencesWorker.execute(() -> {
            String message;
            try { message = app.msime.android.ClipboardExport.export(application); }
            catch (RuntimeException | LinkageError failed) { message = "剪贴板导出失败，请重试"; }
            String result = message;
            main.post(() -> {
                clipboardExportBusy = false;
                if (app.msime.android.ClipboardExport.NEEDS_DIRECTORY.equals(result)) {
                    openHostPage(ClipboardSearchPolicy.SEARCH_PAGE);
                    notice("首次导出请在历史页点击「导出」，选择并授权文件夹");
                } else notice(result);
            });
        });
    }

    private void exportClipboardItemsAfterAuthentication(java.util.List<String> texts) {
        if (clipboardExportBusy) return;
        clipboardExportBusy = true;
        Context application = getApplicationContext();
        preferencesWorker.execute(() -> {
            String message;
            try { message = app.msime.android.ClipboardExport.export(application, texts); }
            catch (RuntimeException | LinkageError failed) { message = "剪贴板导出失败，请重试"; }
            String result = message;
            main.post(() -> {
                clipboardExportBusy = false;
                if (app.msime.android.ClipboardExport.NEEDS_DIRECTORY.equals(result)) {
                    openHostPage(ClipboardSearchPolicy.SEARCH_PAGE);
                    notice("首次导出请在历史页点击「导出」，选择并授权文件夹");
                } else notice(result);
            });
        });
    }

    /** 分词复制只更新系统剪贴板，回到原键盘后沿用既有最近复制、隐私及历史开关。 */
    void copyClipboardSelection(String text) {
        if (text == null || text.isEmpty()) return;
        ClipboardManager manager = getSystemService(ClipboardManager.class);
        if (manager == null) {
            notice("无法访问系统剪贴板");
            return;
        }
        try {
            manager.setPrimaryClip(ClipData.newPlainText("分词", text));
            closeClipboardHistory();
            offerRecentClip(true);
            render();
        } catch (RuntimeException error) {
            notice("复制失败，请重试");
        }
    }

    /** 从键盘分享用户明确选中的本机记录或分词内容。 */
    void shareClipboardText(String text) {
        if (!ClipboardSharePolicy.canShare(text)) {
            notice("这条记录没有可分享的文本");
            return;
        }
        Intent send = new Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_TEXT, text);
        Intent chooser = Intent.createChooser(send, ClipboardSharePolicy.CHOOSER_TITLE)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        try {
            startActivity(chooser);
            closeClipboardHistory();
            requestHideSelf(0);
        } catch (ActivityNotFoundException unavailable) {
            notice("没有可用的分享应用");
        } catch (RuntimeException unavailable) {
            notice("无法打开分享面板");
        }
    }

    /** 编辑页、剪贴板历史页做完之后要不要回到原来的应用：当前输入框属于别的应用才要；在水杉自己的输入框里打开时留在应用里（{@link ClipboardHistoryPolicy#RETURN_TO_CALLER_ARG}）。 */
    private void putReturnToCaller(android.os.Bundle args) {
        EditorInfo info = getCurrentInputEditorInfo();
        if (ClipboardHistoryPolicy.returnsToCaller(info == null ? null : info.packageName, getPackageName()))
            args.putBoolean(ClipboardHistoryPolicy.RETURN_TO_CALLER_ARG, true);
    }

    /**
     * 面板里「清空」确认之后执行。确认就画在面板里（{@link ImePanels#renderClipboardHistory}），不弹 `AlertDialog`：输入法服务没有 Activity 的窗口令牌，对话框要么加不上窗口、让输入法进程崩掉，要么抢走编辑器的窗口焦点，两种情况键盘都会被收起（#5605）。
     */
    void clearClipboardHistory() {
        authenticateClipboard("清空", this::clearClipboardHistoryAfterAuthentication);
    }

    private void clearClipboardHistoryAfterAuthentication() {
        // 这是用户自己要求的清空，失败要说出来，不能吞掉。
        try {
            if (clipboardHistory != null) clipboardHistory.clear();
            forgetCurrentClip();
        } catch (IllegalStateException error) {
            notice("无法清空剪贴板历史");
        }
        imePanels.renderClipboardHistory();
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

    /** 功能面板磁贴上的强度：轻、中、强、系统（跟随系统）。 */
    String hapticStrengthTitle() {
        return hapticStrength.shortTitle();
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
            JSONObject candidate = visibleCandidate(visibleCandidateSlot(action));
            JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
            if (id == null || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE)
                    != session) return false;
            if (!apply(NativeClient.removeCandidate(session, strictCandidateLong(id, "generation"),
                                                    strictCandidateLong(id, "index")))) {
                imeDebugOverlay.showDiagnostic("当前候选不支持此操作");
            } else {
                imeDictionarySync.localChange();
            }
            return true;
        } catch (JSONException | LinkageError error) {
            fail(error);
            return true;
        }
    }

    boolean candidateManagementEnabled() {
        if (view == null || view.optBoolean("prediction", false)
                || !InputViewValuePolicy.textOr(view, "local_mode", "none").equals("none")) return false;
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

    boolean candidateVersionIsCurrent(JSONObject id) {
        return session != 0 && id != null
            && CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) == session
            && sameCandidateVersion(id, view);
    }

    void editCandidate(JSONObject id, CandidateManagementAction action) {
        // 菜单开着时新按键或在线结果会更新候选，旧菜单操作不能停止仍有效的输入会话。
        if (!candidateVersionIsCurrent(id)) return;
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
            } else {
                // 删掉的可能是「我的词条」里的词，网盘同步记下它；只记录、不联网。
                if (action == CandidateManagementAction.REMOVE) imeDictionarySync.localChange();
                if (keyboardRoot != null) {
                    // 候选操作结果属于可见状态，由已有的 polite live region 播报。
                    imeDebugOverlay.showMessage(action.announcement());
                    render();
                }
            }
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    private boolean candidateIsCurrent(int slot, JSONObject id, String text) {
        JSONObject current = visibleCandidate(slot);
        JSONObject currentId = current == null ? null : current.optJSONObject("id");
        return current != null && currentId != null && id != null
            && sameCandidateIdentity(currentId, id)
            && text.equals(chineseCandidateOutput(InputViewValuePolicy.textOr(current, "text", ""), view,
                InputViewValuePolicy.integer(current, "source", -1)));
    }

    boolean candidateGlossInsertionEnabled() {
        if (view == null || !"none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"))) return false;
        int scheme = InputViewValuePolicy.scheme(view, 0);
        return scheme != 3 && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && schemeShowsGlosses(scheme);
    }

    /** 「加入生词本」交不出去或存储出错时的提示；其余提示都由共享层给出。 */
    static final String PERSONAL_WORDBOOK_FAILURE = "无法加入生词本";

    /**
     * 长按候选菜单的「加入生词本」：把候选文字、显示着的释义和读音写进背单词的生词本，结果用 {@link #notice} 告诉用户。写文件要取词书目录的锁，放在 `commonPhrasesWorker` 上，和「添加到常用语」一样是键盘里对用户内容的一次写入；不经过引擎，组字不变。
     */
    void addCandidateToPersonalWordbook(String text, String translation, String reading) {
        // 与打字统计同一个目录，也就是设置里背单词读的 `HostStore.sharedStateDirectory`。
        String directory = typingStatisticsDirectory();
        if (directory.isEmpty()) {
            notice(PERSONAL_WORDBOOK_FAILURE);
            return;
        }
        try {
            commonPhrasesWorker.execute(() -> {
                String answer;
                try {
                    answer = NativeClient.personalWordbookAdd(directory, text, translation, reading);
                } catch (RuntimeException | LinkageError error) {
                    answer = null;
                }
                String shown = answer == null ? PERSONAL_WORDBOOK_FAILURE : answer;
                main.post(() -> notice(shown));
            });
        } catch (java.util.concurrent.RejectedExecutionException error) {
            notice(PERSONAL_WORDBOOK_FAILURE);
        }
    }

    /** 候选旁此刻显示的音标，没有显示或还没查到时为空串；规则同 {@link #candidatePronunciationAnnotation}。 */
    String candidateDisplayedReading(JSONObject candidate) {
        if (!candidatePronunciationVisible()) return "";
        String text = CandidatePronunciationPolicy.lookupText(
            InputViewValuePolicy.textOr(candidate, "text", ""),
            candidateEnglishGloss || candidateTranslationsEnabled ? candidateTranslation(candidate) : "",
            candidateTranslationTargets.contains("en"));
        return candidatePronunciations.getOrDefault(text, "");
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

    /** 完整横向候选的译文回调保留取数时的身份，不能插入旧组字的译文。 */
    void insertStripCandidateGloss(JSONObject snapshot, JSONObject candidate, JSONObject id,
                                  String text, String gloss) {
        if (!sameCandidateVersion(snapshot, view) || !candidateVersionIsCurrent(id)
                || candidate == null || !sameCandidateIdentity(candidate.optJSONObject("id"), id)
                || !text.equals(chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), view,
                    InputViewValuePolicy.integer(candidate, "source", -1)))
                || connection == null) return;
        if (!commitText(gloss, TypingSource.LOCAL)) return;
        if (session != 0) command(3);
    }

    /** 高亮候选和它能插入的译文义项（#7079、#6792）。`eligible` 是组字中、候选非空、方案允许插入译文；释义显示关着时义项为空。 */
    private record TranslationTarget(boolean eligible, int slot, JSONObject candidate,
                                     java.util.List<String> senses) {}

    /** 按当前 view 找高亮候选，用 {@link CandidateTranslationPolicy#insertionGlosses} 拆出义项（与长按菜单相同，两种目标语言都拆进来）。 */
    private TranslationTarget translationTarget() {
        boolean eligible = session != 0 && connection != null && hasEngineComposition()
            && view.optJSONArray("candidates") != null && view.optJSONArray("candidates").length() > 0
            && candidateGlossInsertionEnabled();
        int slot = eligible ? highlightedCandidateSlot(view) : -1;
        JSONObject candidate = slot < 0 ? null : visibleCandidate(slot);
        java.util.List<String> senses = java.util.List.of();
        if (eligible && candidate != null && CandidateTranslationPolicy.userTranslationSourceAllowed(
                InputViewValuePolicy.integer(candidate, "source", -1))
                && (candidateEnglishGloss || candidateTranslationsEnabled)) {
            String translation = candidate.isNull("translation")
                ? "" : JsonPolicy.strictStringOrEmpty(candidate.opt("translation"));
            senses = CandidateTranslationPolicy.insertionGlosses(translation);
        }
        return new TranslationTarget(eligible, slot, candidate, senses);
    }

    /** 执行 {@link HardwareTranslationCommitPolicy#COMMIT}（直接上屏唯一的义项）或 {@link HardwareTranslationCommitPolicy#OPEN_PAGE}（打开义项页），其他结果什么也不做。 */
    private void performTranslationCommit(int action, TranslationTarget target) {
        if (action == HardwareTranslationCommitPolicy.COMMIT) {
            JSONObject id = target.candidate().optJSONObject("id");
            String text = chineseCandidateOutput(InputViewValuePolicy.textOr(target.candidate(), "text", ""), view,
                InputViewValuePolicy.integer(target.candidate(), "source", -1));
            closeCandidatePanel();
            insertCandidateGloss(target.slot(), id, text, target.senses().get(0));
        } else if (action == HardwareTranslationCommitPolicy.OPEN_PAGE) {
            closeCandidatePanel();
            imeTranslationSenses.open(target.senses(),
                InputViewValuePolicy.textOr(target.candidate(), "text", ""));
            render();
            if (horizontalCandidateScroll != null) horizontalCandidateScroll.scrollTo(0, 0);
            if (verticalCandidateScroll != null) verticalCandidateScroll.scrollTo(0, 0);
        }
    }

    /**
     * 实体键盘 Ctrl+Enter（#7079）：组字中、候选非空、当前方案允许插入译文时接住这个键。一个义项直接上屏，几个打开义项页，没有（或释义显示关着）就吃掉按键。返回 false 时交给原来的修饰键分支。
     */
    private boolean handleTranslationCommitKey(int keyCode, KeyEvent event) {
        TranslationTarget target = translationTarget();
        int action = HardwareTranslationCommitPolicy.ctrlEnter(keyCode, event.isShiftPressed(),
            event.isCtrlPressed(), event.isAltPressed(), event.isMetaPressed(), event.getRepeatCount(),
            target.eligible(), target.senses().size());
        if (action == HardwareTranslationCommitPolicy.NONE) return false;
        if (event.getRepeatCount() == 0) translationConsumedKeyCode = keyCode;
        performTranslationCommit(action, target);
        return true;
    }

    /** 长按组字中的「确认」能不能上屏译文（{@link HardwareTranslationCommitPolicy#marksReturnKey}）：键角标「译」、读屏的长按动作都看它。 */
    private boolean returnHoldCommitsTranslation() {
        TranslationTarget target = translationTarget();
        return HardwareTranslationCommitPolicy.marksReturnKey(target.eligible(), target.senses().size());
    }

    /** 空闲时提供字面换行；组字中的长按仍归确认或译文上屏，不能覆盖尚未确认的组字。 */
    private boolean returnHoldInsertsNewline() {
        return ReturnKeyHoldPolicy.insertsNewline(
            connection != null, hasEngineComposition(), returnKeyConfirms());
    }

    /** 长按空闲回车插入换行；长按组字中的「确认」保留 #6792 的译文上屏，无译文时当作点按。 */
    void bindReturnTranslationHold(Button button) {
        button.setOnLongClickListener(ignored -> {
            if (returnHoldInsertsNewline()) {
                imeKeyFeedback.playFeedback(button);
                countKey("Enter");
                selectionEcho.invalidate();
                InputConnectionGuard.commitText(connection, "\n", 1);
                return true;
            }
            TranslationTarget target = translationTarget();
            int action = HardwareTranslationCommitPolicy.returnHold(target.eligible(), target.senses().size());
            if (action == HardwareTranslationCommitPolicy.NONE) return false;
            imeKeyFeedback.playFeedback(button);
            countKey("Enter");
            performTranslationCommit(action, target);
            return true;
        });
        // 读屏与键面使用同一条件，组字中没有译文时保留点按确认。
        button.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(
                    View host, android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                if (returnHoldCommitsTranslation())
                    info.addAction(new android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                        android.view.accessibility.AccessibilityNodeInfo.ACTION_LONG_CLICK, "上屏译文"));
                else if (returnHoldInsertsNewline())
                    info.addAction(new android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction(
                        android.view.accessibility.AccessibilityNodeInfo.ACTION_LONG_CLICK, "换行"));
            }
        });
    }

    /** 义项页上屏一个义项：和长按菜单的 {@link #insertCandidateGloss} 一样，按本地输入上屏后用命令 3 结束组字。页面是否还属于当前组字由 {@link ImeTranslationSenses#commit} 先核对。 */
    void commitTranslationSense(String sense) {
        if (connection == null || session == 0 || !commitText(sense, TypingSource.LOCAL)) {
            render();
            return;
        }
        command(3);
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
            && text.equals(chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), view,
                InputViewValuePolicy.integer(candidate, "source", -1)));
    }

    /** 释义（以及韩语汉字的 훈음）总是从候选下面另起一行，和 iOS 候选条一致；放在同一行会把候选撑宽，一屏只剩一两个候选。 */
    CharSequence candidateLabel(String prefix, String text, String annotation,
                                        boolean highlighted) {
        return candidateLabel(prefix, text, annotation, highlighted, false);
    }

    /** `corrected` 为真时这一行是坐上首选的触点纠错：颜色由 {@link CandidateCorrectionMarkPolicy} 决定，候选字盖上标记色，注释画成标记色实色、不再调淡（首选 chip 上候选字本来就是强调色，标记靠注释看得见）。 */
    CharSequence candidateLabel(String prefix, String text, String annotation,
                                        boolean highlighted, boolean corrected) {
        if (CandidateCorrectionMarkPolicy.plainLabel(prefix, annotation, corrected))
            return CandidateSymbolLabel.apply(this, text, 0, text.length());
        String primary = prefix + text;
        SpannableString label = new SpannableString(annotation.isEmpty() ? primary : primary + "\n" + annotation);
        if (!prefix.isEmpty()) {
            label.setSpan(new ForegroundColorSpan(candidateAppearance.numberFor(highlighted)), 0, prefix.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        Integer mark = CandidateCorrectionMarkPolicy.wordMark(candidateAppearance, highlighted, corrected);
        if (mark != null) {
            label.setSpan(new ForegroundColorSpan(mark), prefix.length(), primary.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        if (annotation.isEmpty()) return CandidateSymbolLabel.apply(this, label, prefix.length(), primary.length());
        int annotationStart = primary.length() + 1;
        // 0.62 倍：候选字与一行释义要一起放进 36 dp 的候选行，0.72 倍时释义下半截被裁掉。
        label.setSpan(new RelativeSizeSpan(0.62f), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        int secondary = CandidateCorrectionMarkPolicy.annotationColour(candidateAppearance, highlighted, corrected);
        label.setSpan(new ForegroundColorSpan(secondary), annotationStart, label.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return CandidateSymbolLabel.apply(this, label, prefix.length(), primary.length());
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
        return koreanSchemeActive() && "none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"));
    }

    String candidateAnnotation(JSONObject candidate) {
        String translation = candidateTranslation(candidate);
        if (koreanHanjaRows())
            return CandidateGlossPolicy.hanjaAnnotation(InputViewValuePolicy.textOr(candidate, "annotation", ""),
                translation,
                candidateEnglishGloss || candidateTranslationsEnabled);
        return CandidateGlossPolicy.annotation(InputViewValuePolicy.textOr(candidate, "annotation", ""),
            translation,
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    /** 候选释义禁止来源即使因同词候选合并拿到译文，也不能在 Android 的候选条、读屏或菜单里出现。 */
    private static String candidateTranslation(JSONObject candidate) {
        if (candidate == null || !CandidateTranslationPolicy.userTranslationSourceAllowed(
                InputViewValuePolicy.integer(candidate, "source", -1))) return "";
        return InputViewValuePolicy.textOr(candidate, "translation", "");
    }

    // 五笔 14 键的预编辑只是首选的编码，其余候选的已打部分按组码逐位对照（WubiCodeHintPolicy.typed）。
    private String wubiCodeHint(JSONObject candidate, JSONObject context, String typed) {
        String code = candidate.optString("code", "");
        String candidateTyped = context == null ? typed : WubiCodeHintPolicy.typed(code, typed,
            InputViewValuePolicy.textOr(context, "key_grid", "none"), InputViewValuePolicy.editingText(context));
        return WubiCodeHintPolicy.hint(code, candidateTyped, wubiCodeHint,
            context == null ? -1 : InputViewValuePolicy.scheme(context, -1),
            context == null ? "none" : InputViewValuePolicy.textOr(context, "local_mode", "none"),
            InputViewValuePolicy.booleanValue(context, "answered_by_pinyin_fallback", false));
    }

    String candidateAnnotation(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        if (hint.isEmpty()) return candidateAnnotation(candidate);
        // 五笔剩余编码和双拼辅助码一样占提示位，释义跟在同一行后面，不再被它挤掉（#6997）。
        return CandidateGlossPolicy.annotation(hint, candidateTranslation(candidate),
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    String candidateAccessibilitySuffix(JSONObject candidate) {
        if (koreanHanjaRows())
            return CandidateGlossPolicy.hanjaAccessibilitySuffix(InputViewValuePolicy.textOr(candidate, "annotation", ""),
                candidateTranslation(candidate),
                candidateEnglishGloss || candidateTranslationsEnabled);
        return CandidateGlossPolicy.accessibilitySuffix(InputViewValuePolicy.textOr(candidate, "annotation", ""),
            candidateTranslation(candidate),
            candidateEnglishGloss || candidateTranslationsEnabled);
    }

    String candidateAccessibilitySuffix(JSONObject candidate, String typed) {
        String hint = wubiCodeHint(candidate, view, typed);
        return hint.isEmpty() ? candidateAccessibilitySuffix(candidate)
            : "，还需输入 " + hint + CandidateGlossPolicy.glossAccessibilitySuffix(
                candidateTranslation(candidate),
                candidateEnglishGloss || candidateTranslationsEnabled);
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

    /** `from` 这份 view 的候选条上高亮候选的位置，没有时为 -1。 */
    static int highlightedCandidateSlot(JSONObject from) {
        JSONArray entries = from == null ? null : from.optJSONArray("candidates");
        if (entries == null) return -1;
        for (int index = 0; index < entries.length(); index++) {
            JSONObject candidate = entries.optJSONObject(index);
            if (candidate != null && InputViewValuePolicy.booleanValue(candidate, "highlighted", false)) return index;
        }
        return -1;
    }

    /** The candidate the strip is showing as highlighted, which 以词定字 takes its character from. */
    private JSONObject highlightedCandidate() {
        int slot = highlightedCandidateSlot(view);
        return slot < 0 ? null : visibleCandidate(slot);
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
        finishPendingCompositionMarkerLoss();
        JSONObject candidate = highlightedCandidate();
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (id == null || session == 0
                || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE) != session)
            return false;
        String displayed = chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), view,
            InputViewValuePolicy.integer(candidate, "source", -1));
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
            fail(error);
            return true;
        }
    }

    JSONObject visibleCandidate(int slot) {
        if (view == null || slot < 0) return null;
        JSONArray entries = view.optJSONArray("candidates");
        return entries == null ? null : entries.optJSONObject(slot);
    }

    void selectVisibleCandidate(Button button, int slot) {
        finishPendingCompositionMarkerLoss();
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
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    /** 触屏完整候选使用全局下标，沿用普通选词的编辑器状态收尾。 */
    void selectStripCandidate(Button button, JSONObject candidate) {
        JSONObject id = candidate == null ? null : candidate.optJSONObject("id");
        if (!candidateVersionIsCurrent(id)) return;
        finishPendingCompositionMarkerLoss();
        int source = InputViewValuePolicy.integer(candidate, "source", -1);
        if (!candidateVersionIsCurrent(id) || AiPendingCandidatePolicy.isPlaceholder(source)
                || AiPendingCandidatePolicy.isCloudPending(source)) return;
        imeKeyFeedback.playFeedback(button);
        closeCandidatePanel();
        try {
            apply(NativeClient.selectAnyCandidate(session, strictCandidateLong(id, "generation"),
                strictCandidateLong(id, "index")));
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    private boolean selectHardwareCandidate(int slot) {
        finishPendingCompositionMarkerLoss();
        int actualSlot = visibleCandidateSlot(slot);
        JSONObject candidate = visibleCandidate(actualSlot);
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
            fail(error);
            return true;
        }
    }

    /** 把硬件数字键和候选条显示序号映射到未显示云占位行之前的引擎槽位。 */
    private int visibleCandidateSlot(int visibleSlot) {
        if (visibleSlot < 0 || view == null) return -1;
        JSONArray entries = view.optJSONArray("candidates");
        if (entries == null) return -1;
        int visible = 0;
        for (int slot = 0; slot < entries.length(); slot++) {
            JSONObject candidate = entries.optJSONObject(slot);
            if (candidate == null || !AiPendingCandidatePolicy.isVisible(
                    InputViewValuePolicy.integer(candidate, "source", -1))) continue;
            if (visible == visibleSlot) return slot;
            visible++;
        }
        return -1;
    }

    void updateCandidateButton(Button button, JSONObject candidate, int slot, int visibleSlot) {
        int source = InputViewValuePolicy.integer(candidate, "source", -1);
        String text = chineseCandidateOutput(InputViewValuePolicy.textOr(candidate, "text", ""), view,
            InputViewValuePolicy.integer(candidate, "source", -1));
        ((ContactCandidateButton) button).setContactSource(source);
        boolean highlighted = InputViewValuePolicy.booleanValue(candidate, "highlighted", false);
        CandidateCanvasAssets canvas = candidateAppearance.canvas();
        if (canvas != null && canvas.banner != null && canvas.banner.candidate() != null) {
            // 来源标注独立于原文，不包含固定图钉，也不猜测原文中的括号后缀。
            String badge = source == 2 ? "☁️" : source == 3 ? "🤖" : "";
            candidateDecorationWords.add(new CandidateCanvasAssets.Word(text, badge, highlighted));
        }
        String typed = InputViewValuePolicy.textOr(view, "preedit", "");
        String annotation = candidatePronunciationAnnotation(candidateAnnotation(candidate, typed), candidate);
        // 开着释义时每个候选都占两行：还没有释义（或这个候选没有）的那行用不换行空格占住。否则带释义的 chip 是两行、不带的是一行，居中后候选字一高一低，释义异步到达或候选一换，候选字就上下跳。
        if (annotation.isEmpty() && candidateGlossLineCount() > 0) annotation = "\u00A0";
        // Touch candidates follow Apple's chip surface: the word itself is shown without a
        // numeric prefix. The slot remains available through contentDescription and the shared
        // session/generation/index identity for accessibility and hardware number-row selection.
        // 实体键盘打字（候选条模式）时例外：候选前面标上数字行选词用的 1–9，每页十个时第十个标 0（#5584、#6679）。
        String number = HardwareKeyboardModePolicy.candidatePrefix(hardwareKeyboardMode, numberRowSelection,
            dedicatedEnglish, visibleSlot);
        boolean corrected = CandidateCorrectionMarkPolicy.confident(candidate.opt(CandidateCorrectionMarkPolicy.FIELD));
        button.setText(candidateLabel(number, text, annotation, highlighted, corrected));
        int labelLines = candidateLabelLines(annotation);
        ViewPolicy.setFixedLines(button, labelLines);
        configureCandidateTextLayout(button, labelLines);
        button.setMaxWidth(candidatePronunciationVisible() ? pixels(240) : Integer.MAX_VALUE);
        if (candidatePronunciationVisible()) button.setEllipsize(android.text.TextUtils.TruncateAt.END);
        KeyboardTypographyPolicy.setKeyTextSize(button, candidateFontSize);
        ViewPolicy.setSelected(button, highlighted);
        // AI 占位行画淡一些，和真候选区分开；云占位行在候选条循环中直接隐藏。按钮会复用，其他候选恢复不透明。读屏念它的状态，不提示长按管理（#6888）。
        button.setAlpha(AiPendingCandidatePolicy.alpha(source));
        // render() attaches the button and applies the complete skin tree once below.
        // Avoid creating its candidate drawables before that pass.
        String description = AiPendingCandidatePolicy.description(source, "候选 " + (visibleSlot + 1) + "：",
            ContactCandidatePolicy.description(source, text),
            candidateAccessibilitySuffix(candidate, typed) + candidatePronunciationAccessibilitySuffix(candidate)
                + CandidateCorrectionMarkPolicy.accessibilitySuffix(corrected));
        JSONObject id = candidate.optJSONObject("id");
        button.setContentDescription(id != null && candidateManagementEnabled()
                && AiPendingCandidatePolicy.offersMenu(source)
            ? description + "；长按管理" : description);
        // 按钮会复用：占位行不可长按，读屏也就不提示长按；换回真候选时恢复。
        button.setLongClickable(AiPendingCandidatePolicy.offersMenu(source));
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
            if (expandedCandidateCanvas != null)
                ViewPolicy.hide(expandedCandidateCanvas);
        }
        if (imeNineKeyPanel != null) imeNineKeyPanel.dismiss();
    }

    /**
     * 展开面板里点中一个候选。只选掉前面一段、组字还在时面板留着，换上剩下组字的完整候选，用户接着在面板里往下选（#9193，规则在 {@link CandidatePanelSelectionPolicy}）；整句上屏、组字清空或会话换了照旧收起。九键面板开着笔画筛选时顺带清掉笔画：它比的是刚被选掉的那个字。
     *
     * <p>选词和清笔画的 apply 都不画：面板这时已标成收起，画一次会先把面板收掉（九键面板还会复位左栏、排队清筛选），紧接着又重开。面板状态定下来以后只画一次。手指还按在候选上时这次 render 照旧延后（{@link #candidateRenderDeferred}），松手补的那一帧按定下来的状态画，面板里不会留着旧一代的候选。
     */
    void selectPanelCandidate(JSONObject id) {
        long selectedSession = session;
        boolean nineKeyPanel = imeNineKeyPanel.eligible();
        candidatePanelOpen = false;
        candidatePanelSnapshot = null;
        boolean previousSuppressed = renderingSuppressed;
        renderingSuppressed = true;
        boolean keepsOpen;
        try {
            boolean handled = apply(NativeClient.selectAnyCandidate(session,
                strictCandidateLong(id, "generation"), strictCandidateLong(id, "index")));
            keepsOpen = view != null && CandidatePanelSelectionPolicy.keepsOpen(handled, selectedSession,
                session, InputViewValuePolicy.editingText(view));
            String strokes = keepsOpen && nineKeyPanel && imeNineKeyPanel.eligible()
                ? CandidatePanelSelectionPolicy.strokesAfterPartialSelection(
                    InputViewValuePolicy.textOr(view, "nine_key_strokes", ""))
                : null;
            if (strokes != null)
                apply(NativeClient.setNineKeyFilter(session,
                    InputViewValuePolicy.booleanValue(view, "nine_key_single_character", false), strokes));
        } catch (JSONException | LinkageError error) {
            renderingSuppressed = previousSuppressed;
            fail(error);
            return;
        } finally {
            renderingSuppressed = previousSuppressed;
        }
        if (keepsOpen) openCandidatePanel(true);
        else render();
    }

    void openCandidatePanel() {
        openCandidatePanel(false);
    }

    /** {@code afterSelection} 为真时是展开面板里部分选词之后重开（{@link #selectPanelCandidate}）：剩下的候选只有一页也开，没开成也要画一次。 */
    private void openCandidatePanel(boolean afterSelection) {
        // 全拼九键的三栏面板只要在组字就能打开：左栏选拼音、右栏筛选在只有一页候选时同样有用。
        boolean nineKeyPanel = imeNineKeyPanel.eligible();
        if (session == 0 || view == null || (!nineKeyPanel && !afterSelection
                && strictCandidatePage(view, "page_count") <= 1)) {
            if (afterSelection) render();
            return;
        }
        try {
            JSONObject snapshot = currentCandidateSnapshot();
            JSONArray entries = snapshot.optJSONArray("candidates");
            JSONArray visible = view.optJSONArray("candidates");
            if (!nineKeyPanel && (entries == null || (afterSelection ? entries.length() == 0
                    : visible == null || entries.length() <= visible.length()))) {
                render();
                return;
            }
            candidatePanelSnapshot = snapshot;
            candidatePanelOpen = true;
            render();
        } catch (JSONException | LinkageError error) { fail(error); }
    }

    boolean handwritingActive() {
        String localMode = view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none");
        return session != 0 && !dedicatedEnglish
            && "none".equals(localMode)
            && keyboardLayer == KeyboardLayout.Layer.LETTERS
            && displayedTouchLayout(view) == HANDWRITING_LAYOUT && handwritingCanvas != null;
    }

    void deactivateHandwriting() {
        imeLayoutRows.resetForInput();
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
            boolean downloadVisible = ViewPolicy.isVisible(handwritingDownload);
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

    /** 水平删除的快照仅属于当前按压；敏感框和组字、墨迹继续走原有退格。 */
    BackspaceScrub beginBackspaceScrub() {
        if (connection == null || EditorPolicy.password(editorInputType) || !allowLearning
                || incognitoEnabled || hasEngineComposition()
                || handwritingCanvas != null && handwritingCanvas.hasInk()) return null;
        BackspaceScrub.Frame frame = readBackspaceScrubFrame();
        return frame == null || frame.before().isEmpty() ? null : new BackspaceScrub(connection, frame);
    }

    private BackspaceScrub.Frame readBackspaceScrubFrame() {
        if (connection == null) return null;
        try {
            var request = new android.view.inputmethod.ExtractedTextRequest();
            request.hintMaxChars = BackspaceScrub.MAX_UNITS + 2 * BackspaceScrub.CONTEXT_UNITS;
            request.hintMaxLines = 32;
            var extracted = connection.getExtractedText(request, 0);
            if (extracted == null || extracted.text == null || extracted.partialStartOffset != -1
                    || extracted.startOffset < 0 || extracted.selectionStart < 0
                    || extracted.selectionStart != extracted.selectionEnd
                    || extracted.selectionStart > extracted.text.length()) return null;
            int at = extracted.selectionStart;
            long absolute = (long) extracted.startOffset + at;
            if (absolute > Integer.MAX_VALUE) return null;
            String before = extracted.text.subSequence(Math.max(0,
                at - BackspaceScrub.MAX_UNITS - BackspaceScrub.CONTEXT_UNITS), at).toString();
            String after = extracted.text.subSequence(at,
                Math.min(extracted.text.length(), at + BackspaceScrub.CONTEXT_UNITS)).toString();
            return new BackspaceScrub.Frame((int) absolute, before, after);
        } catch (RuntimeException ignored) { return null; }
    }

    void moveBackspaceScrub(BackspaceScrub scrub, int deleted) {
        if (EditorPolicy.password(editorInputType) || !allowLearning || incognitoEnabled
                || hasEngineComposition() || handwritingCanvas != null && handwritingCanvas.hasInk()) {
            scrub.invalidate();
            return;
        }
        int previous = scrub.deleted();
        scrub.move(new BackspaceScrub.Editor() {
            @Override public Object target() { return connection; }
            @Override public BackspaceScrub.Frame read() { return readBackspaceScrubFrame(); }
            @Override public boolean delete(int units) { return deleteBeforeCursor(units); }
            @Override public boolean restore(String text) {
                // 恢复已删内容不计新输入；绕开统计、学习与剪贴板，只登记选区回声。
                boolean accepted = InputConnectionGuard.commitText(connection, text, 1);
                if (accepted) {
                    selectionEcho.commit(text.length());
                    selectionEcho.expect();
                } else selectionEcho.invalidate();
                return accepted;
            }
        }, deleted);
        if (previous != scrub.deleted()) refreshAfterQuickDelete();
    }

    /** 快速删除只备份编辑器实际接受的删除；不在密码、不学习或无痕输入框中保存。 */
    void deleteAllBeforeCursor() {
        clearQuickDeleteUndo();
        if (handwritingCanvas != null && handwritingCanvas.hasInk()) clearHandwriting();
        if (hasEngineComposition()) discardComposition();
        InputConnection target = connection;
        if (target == null) return;
        boolean captureAllowed = !EditorPolicy.password(editorInputType) && allowLearning && !incognitoEnabled;
        QuickDeleteUndo.Capture capture = captureAllowed ? new QuickDeleteUndo.Capture() : null;
        int initialCursor = capture == null ? -1 : quickDeleteCursor(target);
        CharSequence following = capture == null ? null
            : InputConnectionGuard.textAfterCursor(target, QuickDeleteUndo.CONTEXT_UNITS);
        String after = following == null ? null : following.toString();
        CharSequence selected = InputConnectionGuard.selectedText(target);
        if (selected != null && selected.length() > 0) {
            selected = selected.toString();
            if (!InputConnectionGuard.commitText(target, "", 1)) return;
            selectionEcho.commit(0);
            if (capture != null) capture.deleted(selected);
            initialCursor = capture == null ? -1 : quickDeleteCursor(target);
        }
        long deleted = BackspaceSwipePolicy.clearBeforeCursor(new BackspaceSwipePolicy.Editor() {
            @Override public CharSequence textBeforeCursor(int length) {
                return InputConnectionGuard.textBeforeCursor(target, length);
            }

            @Override public boolean deleteBeforeCursor(int length) {
                boolean accepted = InputConnectionGuard.deleteSurroundingText(target, length, 0);
                if (accepted) selectionEcho.deleteBefore(length);
                else selectionEcho.invalidate();
                return accepted;
            }
        }, capture);
        selectionEcho.expect();
        int cursor = capture == null ? -1 : quickDeleteCursor(target);
        CharSequence before = capture == null ? null
            : InputConnectionGuard.textBeforeCursor(target, QuickDeleteUndo.CONTEXT_UNITS);
        CharSequence currentAfter = capture == null ? null
            : InputConnectionGuard.textAfterCursor(target, QuickDeleteUndo.CONTEXT_UNITS);
        // 输入连接声称成功却没删、读取失败或光标位置不符时，不能生成一份会重复插入的备份。
        if (capture != null && initialCursor >= 0 && cursor == initialCursor - deleted
                && before != null && after != null && currentAfter != null && after.contentEquals(currentAfter)) {
            String text = capture.text();
            if (!text.isEmpty()) {
                quickDeleteUndo = new QuickDeleteUndo(target, cursor, text, before.toString(), after,
                    SystemClock.elapsedRealtime());
                main.postDelayed(expireQuickDeleteUndo, QuickDeleteUndo.LIFETIME_MS);
            }
        }
        refreshAfterQuickDelete();
    }

    /** 快速删除的可选全框模式：显式选中光标前后读到的整段文字后删除，并保留一次撤回。 */
    void deleteAllInput() {
        clearQuickDeleteUndo();
        if (handwritingCanvas != null && handwritingCanvas.hasInk()) clearHandwriting();
        if (hasEngineComposition()) discardComposition();
        InputConnection target = connection;
        if (target == null) return;
        int cursor = quickDeleteCursor(target);
        CharSequence before = InputConnectionGuard.textBeforeCursor(target, QuickDeleteUndo.MAX_UNITS);
        CharSequence after = InputConnectionGuard.textAfterCursor(target, QuickDeleteUndo.MAX_UNITS);
        if (cursor < 0 || before == null || after == null
                || before.length() >= QuickDeleteUndo.MAX_UNITS
                || after.length() >= QuickDeleteUndo.MAX_UNITS) return;
        String deleted = before.toString() + after;
        if (deleted.isEmpty() || deleted.length() > QuickDeleteUndo.MAX_UNITS) return;
        int start = cursor - before.length();
        int end = cursor + after.length();
        if (!InputConnectionGuard.setSelection(target, start, end)
                || !InputConnectionGuard.commitText(target, "", 1)) return;
        selectionEcho.invalidate();
        if (!EditorPolicy.password(editorInputType) && allowLearning && !incognitoEnabled
                && deleted.length() <= QuickDeleteUndo.MAX_UNITS) {
            quickDeleteUndo = new QuickDeleteUndo(target, start, deleted, "", "",
                SystemClock.elapsedRealtime());
            main.postDelayed(expireQuickDeleteUndo, QuickDeleteUndo.LIFETIME_MS);
        }
        refreshAfterQuickDelete();
    }

    /** 只取绝对选区位置；编辑器不支持或仍有选区时不提供撤回。 */
    private int quickDeleteCursor(InputConnection target) {
        if (target == null) return -1;
        // 当前连接上的位置在英文直输的一次编辑器事件里只读一次（这是整段文字的跨进程拷贝，见 EditorReadCache）；范围外照常直接读。
        if (target == connection) return editorReads.cursor(target, () -> readQuickDeleteCursor(target));
        return readQuickDeleteCursor(target);
    }

    private static int readQuickDeleteCursor(InputConnection target) {
        try {
            android.view.inputmethod.ExtractedTextRequest request = new android.view.inputmethod.ExtractedTextRequest();
            request.hintMaxChars = QuickDeleteUndo.CONTEXT_UNITS;
            request.hintMaxLines = 1;
            android.view.inputmethod.ExtractedText text = target.getExtractedText(request, 0);
            if (text == null || text.selectionStart < 0 || text.selectionStart != text.selectionEnd) return -1;
            return text.startOffset + text.selectionStart;
        } catch (RuntimeException ignored) { return -1; }
    }

    boolean canUndoQuickDelete() {
        QuickDeleteUndo snapshot = quickDeleteUndo;
        if (snapshot == null) return false;
        if (EditorPolicy.password(editorInputType) || !allowLearning || incognitoEnabled || hasEngineComposition()
                || !snapshot.matches(connection, quickDeleteCursor(connection),
                    InputConnectionGuard.textBeforeCursor(connection, QuickDeleteUndo.CONTEXT_UNITS),
                    InputConnectionGuard.textAfterCursor(connection, QuickDeleteUndo.CONTEXT_UNITS),
                    SystemClock.elapsedRealtime())) {
            clearQuickDeleteUndo();
            return false;
        }
        return true;
    }

    void undoQuickDelete() {
        if (!canUndoQuickDelete()) return;
        QuickDeleteUndo snapshot = quickDeleteUndo;
        // 撤回不算新输入，不学习、不写统计；明确拒绝且上下文未变时保留一次重试机会。
        boolean batch = InputConnectionGuard.beginBatchEdit(connection);
        boolean accepted = InputConnectionGuard.commitText(connection, snapshot.text(), 1);
        if (batch) InputConnectionGuard.endBatchEdit(connection);
        if (accepted) {
            // 跨进程的 true 可能只表示请求已投递；批量结束后原位置和上下文完全未变则仍可重试。
            if (canUndoQuickDelete()) return;
            selectionEcho.commit(snapshot.text().length());
            selectionEcho.expect();
            refreshAfterQuickDelete();
        } else {
            canUndoQuickDelete();
        }
    }

    private void refreshAfterQuickDelete() {
        if (directEnglishActive()) {
            clearEnglishSuggestions();
            refreshEnglishSuggestions();
        }
        updateAutomaticCapitalization();
        render();
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

    /**
     * 日语九键按下时的五向提示，不拦截触摸、读屏不念。只画在被按的键上、比键大一圈的方框里（{@link JapaneseFlickGuideGeometry}）：中间是此刻轻点会打出的假名，左上右下四格是滑动方向上的假名，当前方向那格用强调色填满。原先五格各有一个键大、拼成的十字盖住周围 3×3 个键（#6716）。
     */
    final class JapaneseFlickPreview extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final String[] labels = new String[5];
        private int selectedDirection;
        private int anchorWidth;
        private int anchorHeight;
        private String paletteKey;
        private int keyColor;
        private int accentColor;
        private int hairlineColor;
        private int onAccentColor;
        private int foregroundColor;
        private Typeface previewTypeface;
        private final int[] overlayLocation = new int[2];
        private final int[] anchorLocation = new int[2];
        /** 提示此刻画的是哪个键；收起时为 null。 */
        private Button owner;

        JapaneseFlickPreview(android.content.Context context) {
            super(context);
            // 收起时是 INVISIBLE 不是 GONE：每个假名键按下、松开都要显隐一次，GONE 和 VISIBLE 之间切换会让整个键区重新布局，INVISIBLE 只重画。浮层铺满键区，大小不随显隐变化。
            ViewPolicy.setInvisible(this);
            ViewPolicy.setNonInteractive(this);
            ViewPolicy.hideFromAccessibility(this);
        }

        /** `direction` 是手指当前的方向（0 为没滑出阈值），`tapDirection` 是此刻轻点会打出的方向，决定中间格写哪个假名。 */
        void show(Button anchor, JapaneseNineKeyLayout.Key key, int direction, int tapDirection) {
            java.util.List<String> kana = key.kana();
            for (int index = 0; index < labels.length; index++)
                labels[index] = index < kana.size() ? kana.get(index) : "";
            if (tapDirection > 0 && tapDirection < kana.size() && !kana.get(tapDirection).isEmpty())
                labels[0] = kana.get(tapDirection);
            selectedDirection = BoundsPolicy.bounded(direction, 0, labels.length - 1);
            anchor.getLocationOnScreen(anchorLocation);
            anchorWidth = anchor.getWidth();
            anchorHeight = anchor.getHeight();
            owner = anchor;
            ViewPolicy.show(this);
            invalidate();
        }

        void hide() {
            owner = null;
            ViewPolicy.setInvisible(this);
        }

        /** 提示画的是 `anchor` 时才收起。 */
        void hideFor(Button anchor) {
            if (owner == anchor) hide();
        }

        boolean showing() { return getVisibility() == VISIBLE; }

        boolean showingFor(Button anchor) { return owner == anchor && showing(); }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            if (anchorWidth <= 0 || anchorHeight <= 0) return;
            float density = DimensionPolicy.density(getContext());
            // 位置在这里按这一帧的布局算，不在 `show` 时算：浮层原先隐藏时是 GONE、没有布局，`show` 那一刻读到的位置是 0，第一次按下时提示画到了左上角；现在隐藏是 INVISIBLE，但键区换布局、浮动键盘拖动之后仍以这一帧为准。坐标取相对浮层自己，它铺满键区，浮动键盘时键区在外框里有偏移。
            getLocationOnScreen(overlayLocation);
            JapaneseFlickGuideGeometry.Guide guide = JapaneseFlickGuideGeometry.layout(
                anchorLocation[0] - overlayLocation[0], anchorLocation[1] - overlayLocation[1],
                anchorWidth, anchorHeight, getWidth(), getHeight(),
                DimensionPolicy.floatPixels(4, density), DimensionPolicy.floatPixels(18, density));
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
            float radius = DimensionPolicy.floatPixels(10, density);
            // 底板带投影，看得出是浮在键上的提示，而不是键面本身变了样。
            paint.setStyle(Paint.Style.FILL);
            paint.setColor(keyColor);
            paint.setShadowLayer(DimensionPolicy.floatPixels(8, density), 0,
                DimensionPolicy.floatPixels(2, density), 0x40000000);
            canvas.drawRoundRect(guide.left(), guide.top(), guide.right(), guide.bottom(), radius, radius, paint);
            paint.clearShadowLayer();
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(BoundsPolicy.bounded(density, 1f, Float.MAX_VALUE));
            paint.setColor(hairlineColor);
            canvas.drawRoundRect(guide.left(), guide.top(), guide.right(), guide.bottom(), radius, radius, paint);
            float cellWidth = guide.cellWidth();
            float cellHeight = guide.cellHeight();
            float inset = DimensionPolicy.floatPixels(1, density);
            float cellRadius = DimensionPolicy.floatPixels(6, density);
            // 方向格的字比键面小，中间格接近键面字号；都不超过格高，免得相邻两格的字挤在一起。
            float directionSize = BoundsPolicy.atMost(KeyboardTypographyPolicy.keySp(getContext(), 15), cellHeight * 0.8f);
            float centerSize = BoundsPolicy.atMost(KeyboardTypographyPolicy.keySp(getContext(), 22), cellHeight * 1.1f);
            ViewPolicy.setTypeface(paint, previewTypeface);
            for (int index = 0; index < labels.length; index++) {
                String label = labels[index];
                if (label == null || label.isEmpty()) continue;
                boolean selected = index == selectedDirection;
                float x = guide.cellLeft(index);
                float y = guide.cellTop(index);
                if (selected) {
                    paint.setStyle(Paint.Style.FILL);
                    paint.setColor(accentColor);
                    canvas.drawRoundRect(x + inset, y + inset, x + cellWidth - inset, y + cellHeight - inset,
                        cellRadius, cellRadius, paint);
                }
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(selected ? onAccentColor : foregroundColor);
                paint.setFakeBoldText(selected);
                ViewPolicy.setTextSize(paint, index == 0 ? centerSize : directionSize);
                Paint.FontMetrics metrics = paint.getFontMetrics();
                // 字身中线对准格子中线。
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
            // 「键盘调整」收窄的宽度和水平位置落成外框的左右内边距：工具栏或候选行、按键区、面板和底栏一起收窄，两侧空出来的是外框自己的皮肤底，不透出应用，点上去也落不到按键。大屏上外框本身是 720 dp 的居中块，在它里面再收窄。
            if (getContext() instanceof MSIMEInputService service) {
                int[] insets = service.keyboardHorizontalInsets(MeasureSpec.getSize(widthMeasureSpec));
                if (getPaddingLeft() != insets[0] || getPaddingRight() != insets[1])
                    setPadding(insets[0], getPaddingTop(), insets[1], getPaddingBottom());
            }
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

        /** 「键盘调整」打开时遮罩以下（底栏、底部抬高、导航栏那一截）同样不接受触摸：遮罩只盖键盘外框，底栏的按钮仍在下面。 */
        @Override public boolean onInterceptTouchEvent(android.view.MotionEvent event) {
            if (getContext() instanceof MSIMEInputService service && service.keyboardAdjustActive
                    && service.keyboardAdjustOverlay != null && event.getY() >= service.keyboardAdjustOverlay.getBottom())
                return true;
            return super.onInterceptTouchEvent(event);
        }

        @android.annotation.SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(android.view.MotionEvent event) {
            if (getContext() instanceof MSIMEInputService service && service.keyboardAdjustActive) return true;
            return super.onTouchEvent(event);
        }
    }

    @Override public View onCreateInputView() {
        long createStartedAt = SystemClock.uptimeMillis();
        imeFunctionPanel.close();
        operationNotice.cancel();
        imePanels.dismissClipboardPreview();
        cancelInputViewRefresh();
        deactivateHandwriting();
        candidateButtons.clear();
        candidateDecorationWords.clear();
        englishSuggestionButtons.clear();
        nineKeySpellingButtons.clear();
        nineKeySpellingIndices = java.util.List.of();
        nineKeySpellingGeneration = -1;
        loadFeedbackPreferences();
        refreshLocalSettings();
        File files = getFilesDir();
        clipboardMode = ClipboardModeStore.load(this);
        // 主模式和访客模式各用自己的历史目录；容量从设置页保存偏好的共享状态目录读取。
        clipboardHistory = ClipboardHistoryStore.forMode(this, clipboardMode);
        voiceResultStore = files == null ? null
            : new VoiceResultStore(files.toPath().resolve("voice-handoff"));
        communityReplyLibrary = files == null ? null : new CommunityReplyLibrary(files.toPath());
        // 这里不再按开关清空历史：进程刚起来时开关还是字段初始值（关），实时偏好还没读到，按它清空会在每次切回本输入法时抹掉整份历史（#5602）。开关关着时由下一次实时读偏好负责清空，见 ClipboardHistoryRetentionPolicy。
        keyboardRoot = new FrameLayout(this);
        PanelSurface surface = new PanelSurface(this);
        keyboardSurface = surface;
        keyboardRoot.addView(keyboardSurface);
        keyboardSurface.addOnLayoutChangeListener((changed, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            if (keyboardAdjustActive && !overlayDrawn()
                    && keyboardAdjustKeyboardTop == KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP) {
                keyboardSurface.getLocationOnScreen(floatingLocation);
                keyboardAdjustKeyboardTop = floatingLocation[1];
            }
            // 停靠背景只画键区所在的底部横带，键区伸缩后按新的上沿重画。
            if (stableDockedWindow() || keyboardAdjustActive) keyboardRoot.invalidate();
            // 面板变高变窄（换布局、调高度、旋转）后按存着的千分比重新摆，保证仍在窗口里；跟随光标的候选栏换了内容、变了高度后同样按光标重新摆。
            if (overlayDrawn()) positionKeyboardSurface();
        });
        keyboardRoot.addOnLayoutChangeListener((changed, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> {
            if (overlayDrawn()) positionKeyboardSurface();
        });
        imeStyler.applyKeyboardSurfaceGeometry();
        LinearLayout keyboard = LayoutPolicy.column(this);
        // 停靠时键盘列按系统栏留出内边距（导航栏那一截画键盘底色），手势导航下再垫一条底栏（ImeBottomBar）；浮动时面板悬在应用上面，不留，系统栏的四边只用来限制面板能拖到哪里。
        keyboard.setOnApplyWindowInsetsListener((target, insets) -> {
            imeBottomBar.readInsets(insets);
            systemBarInsets.set(imeBottomBar.insets());
            imeBottomBar.apply((LinearLayout) target);
            if (overlayDrawn()) positionKeyboardSurface();
            return insets;
        });
        keyboard.addOnLayoutChangeListener((target, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> onKeyboardContentLayout(target));
        keyboardSurface.addView(keyboard, LayoutPolicy.frameMatchParentParams());
        imeBottomBar.build(surface);
        japaneseFlickPreview = new JapaneseFlickPreview(this);
        keyboardSurface.addView(japaneseFlickPreview, LayoutPolicy.frameMatchParentParams());
        quickDeleteOverlay = new QuickDeleteOverlay(this);
        keyboardSurface.addView(quickDeleteOverlay, LayoutPolicy.frameMatchParentParams());
        surface.fullBleed.add(quickDeleteOverlay);
        // 按键气泡的覆盖层：盖在整个键盘上、初始为空，空的 FrameLayout 不拦截触摸，由 ImeLetterRows 持有。
        surface.fullBleed.add(japaneseFlickPreview);
        imeLetterRows.keyPreviewLayer = new FrameLayout(this);
        surface.fullBleed.add(imeLetterRows.keyPreviewLayer);
        keyboardSurface.addView(imeLetterRows.keyPreviewLayer, LayoutPolicy.frameMatchParentParams());
        // 「键盘调整」遮罩：不是 fullBleed，外框把它的底边距设成键盘列的底部内边距，它就正好盖住工具栏或候选行加按键区，不盖底栏、底部抬高和导航栏那一截。
        keyboardAdjustOverlay = new KeyboardAdjustOverlay(this);
        keyboardAdjustOverlay.setListener(keyboardAdjustListener);
        ViewPolicy.hide(keyboardAdjustOverlay);
        keyboardSurface.addView(keyboardAdjustOverlay, LayoutPolicy.frameMatchParentParams());
        candidateCanvasRegion = new CandidateCanvasColumn(this);
        LinearLayout candidateRegion = candidateCanvasRegion;
        candidateHeightSpacer = new View(this);
        candidateRegion.addView(candidateHeightSpacer, LayoutPolicy.matchWidthHeightPx(0));
        imeToolbar.buildCandidateHeader(candidateRegion);
        imeCompositionEditor.install(candidateRegion);
        diagnosticView = ViewPolicy.textLabel(this, "", 12);
        KeyboardTypographyPolicy.setKeyTextSize(diagnosticView, 12);
        diagnosticView.setContentDescription("输入提示");
        // 提示原先是 Toast，读屏会念出来；改到诊断行后由实时区域念出。
        diagnosticView.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        ViewPolicy.hide(diagnosticView);
        candidateRegion.addView(diagnosticView, LayoutPolicy.matchWidthWrapParams());
        candidateRegion.addView(shortcutToolbar, LayoutPolicy.matchWidthHeightPx(
            pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
        imeToolbar.addRecentClipRow(candidateRegion);
        if (inlineAutofill != null) inlineAutofill.buildRow(candidateRegion);
        nineKeySpellings = LayoutPolicy.row(this);
        nineKeySpellingScroll = new HorizontalScrollView(this);
        nineKeySpellingScroll.setHorizontalScrollBarEnabled(false);
        nineKeySpellingScroll.setContentDescription("九键拼音选择");
        nineKeySpellingScroll.addView(nineKeySpellings);
        ViewPolicy.hide(nineKeySpellingScroll);
        candidates = LayoutPolicy.row(this);
        horizontalCandidateScroll = new HorizontalScrollView(this);
        // 和九键拼音行一样不要滚动条：候选条只有一行高，默认滚动条贴在底边，浅色皮肤上是一条细灰线（#5933 录屏里看得到）。
        horizontalCandidateScroll.setHorizontalScrollBarEnabled(false);
        horizontalCandidateScroll.addView(candidates);
        verticalCandidates = LayoutPolicy.column(this);
        verticalCandidateScroll = new ScrollView(this);
        verticalCandidateScroll.addView(verticalCandidates);
        candidateViewport = new FrameLayout(this);
        candidateViewport.addView(horizontalCandidateScroll, LayoutPolicy.frameMatchParentParams());
        candidateViewport.addView(verticalCandidateScroll, LayoutPolicy.frameMatchParentParams());
        ViewPolicy.hide(candidateViewport);
        imeToolbar.addCandidateLine(candidateRegion, candidateViewport,
            pixels(ImeToolbar.CANDIDATE_LINE_DP));
        floatingBar = new FloatingKeyboardBar(this);
        floatingBar.setListener(new FloatingKeyboardBar.Listener() {
            @Override public void dragStarted() { startFloatingDrag(); }

            @Override public void dragged(float dx, float dy) { dragFloatingKeyboard(dx, dy); }

            @Override public void dragEnded() { endFloatingDrag(); }

            @Override public void dock() { setFloatingKeyboard(false); }
        });
        ViewPolicy.hide(floatingBar);
        keyboard.addView(floatingBar, LayoutPolicy.matchWidthHeightPx(
            pixels(FloatingKeyboardPolicy.BAR_HEIGHT_DP)));
        keyboard.addView(candidateRegion);
        // 功能面板和候选展开网格盖住键区但不盖顶部一行，设计里打开它们时工具栏或候选条仍在上面；顶部一行高度会变（读音行、释义行），所以跟着它的实际底边走。
        candidateRegion.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> alignOverlaysBelowTopRow(view));
        imeFrame.keyboard = keyboard;
        // Like Apple, keep the shared candidate/shortcut strip above the reply surface. The
        // ordinary key rows and controls are hidden while this weighted child is visible.
        replyKeyboard = imePanels.createReplyKeyboard();
        ViewPolicy.hide(replyKeyboard);
        keyboard.addView(replyKeyboard, LayoutPolicy.weightedWidthParams(1));
        // 键距是键的外边距；这两个容器把落在空隙里的按下交给拥有那段空隙的键，画面不变（见 KeyboardKeyArea）。
        KeyboardKeyArea letterArea = new KeyboardKeyArea(this, this::followsKeySpacing);
        letterArea.setGlideTracker(imeGlideTyping);
        // 26 键双拼：按键记下每根手指的按下点，键位一变就重新注册几何（ImeTouchPoints）。只记字母键和微软双拼的 `;`（shuangpinLetterKey 造的键），别的键点击时不取按下点。
        letterArea.setTouchPoints(imeTouchPoints.pending, key -> key instanceof ShuangpinHintButton);
        letterArea.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> imeTouchPoints.register());
        keyRows = letterArea;
        keyRows.setOrientation(LinearLayout.VERTICAL);
        imeFrame.wrap(keyRows);
        actionRow = new KeyboardKeyArea(this, this::followsKeySpacing);
        actionRow.setOrientation(LinearLayout.HORIZONTAL);
        actionRow.setContentDescription("键盘功能行");
        actionRowSignature = "";
        imeFrame.wrap(actionRow, LayoutPolicy.matchWidthHeightPx(
            pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP)));
        actionRow.setTag(KeyboardHeightRole.bottomRow());
        // Staging only: every control below is created here and then moved to the row that owns it.
        // The case and delete keys go to the last of the 26 key rows, the shortcut glyphs to the
        // toolbar, and what the action row keeps is whatever KeyboardActionRow lists for the surface.
        LinearLayout controls = LayoutPolicy.row(this);
        shiftButton = button(controls, "⇧", () -> {
            if (usesQuanpinSeparator()) {
                if (hasEngineComposition()) character('\'');
                return;
            }
            // 韩语键面上 Shift 是双辅音那一排，不是切到英文的入口；越南语和藏文里它是字母大小写。
            if (!dedicatedEnglish && session != 0 && !helpcodeCompositionEligible()
                    && !letterCompositionActive()) {
                toggleInputLanguage(false, false);
                if (!dedicatedEnglish || session == 0) return;
                temporaryEnglishShift = true;
                // 这次 Shift 从小写开始，避免编辑器的自动大写把首次点按解释为关闭。
                letterCase.reset();
            }
            letterCase.toggle(SystemClock.uptimeMillis());
            finishTemporaryEnglishShift();
            // 只换键面不重建键行（#6997）：重建后的新键在下一次布局前宽高为 0，紧跟着 Shift 按下的字母会被丢掉。
            imeLetterRows.refreshLetterCase();
            render();
        });
        shiftButton.setContentDescription("切换到英文大写");
        keyId(shiftButton, "ShiftLeft");
        languageButton = keyId(button(controls, "中/英", this::toggleInputLanguage), "SoftLanguage");
        languageButton.setContentDescription("切换中英文");
        // 没有会话、也没有会话在建时（密码框、数字框、启动失败）这个键仍可用（长按要能打开输入法选择框），但点按切不了中英：不给按键反馈、也不记一次按键，免得点上去有声有振动却什么也没发生。会话还在建时点按照常生效（#10086），见 ToolbarReadinessPolicy.languageTap。
        languageButton.setOnClickListener(ignored -> {
            ToolbarReadinessPolicy.LanguageTap tap = ToolbarReadinessPolicy.languageTap(toolbarSession(),
                languageKeyCycles(), dedicatedEnglish, selectedScheme != null && selectedScheme.otherLanguage(),
                pendingLanguageTap);
            if (tap == ToolbarReadinessPolicy.LanguageTap.IGNORE) return;
            imeKeyFeedback.playFeedback(languageButton);
            countKey(languageButton);
            switch (tap) {
                case ENGINE -> {
                    // 会话建好、偏好还没读到时记着的那一下作废：这一下已经照常切了，读到偏好时不能再补按一次。
                    pendingLanguageTap = false;
                    languageKeyTapped();
                }
                case HOST_TOGGLE -> toggleInputLanguageBeforeSession();
                case DEFER -> {
                    pendingLanguageTap = true;
                    render();
                }
                default -> { }
            }
        });
        bindInputMethodPicker(languageButton);
        layerButton = button(controls, "123", () -> {
            // 注音的 #+= 是共享符号行，底行用的就是这个键：那一页上它印 123，回到数字九宫格，不回字母层（KeyboardLayout.drawsSharedSymbolRows）。
            if (imeLetterRows.sharedSymbolRowsDrawn()) {
                imeLetterRows.showNumberPad();
                imeCalculator.refresh();
                return;
            }
            boolean enteringSymbols = keyboardLayer == KeyboardLayout.Layer.LETTERS;
            keyboardLayer = enteringSymbols
                ? KeyboardLayout.Layer.SYMBOLS : KeyboardLayout.Layer.LETTERS;
            if (enteringSymbols) {
                // 从字母层进入数字九宫格时清掉上一轮停留的 #+= 页，123 的入口必须始终是 123。
                imeLetterRows.showNumberPad();
            } else {
                imeLetterRows.rebuildKeyRows();
                render();
            }
            imeCalculator.refresh();
        });
        layerButton.setContentDescription("切换到数字和符号");
        keyId(layerButton, "SoftLayer");
        bindSymbolPanelHold(layerButton);
        symbolPanelButton = keyId(button(controls, "符", imePanels::showSymbolPanel), "SoftSymbol");
        symbolPanelButton.setContentDescription("打开符号面板");
        quickPunctuationButton = keyId(addRowButton(controls, new KeyHintButton(this), ",",
            this::insertQuickPunctuation, true), "Comma");
        quickPunctuationButton.setOnLongClickListener(ignored -> {
            if (commaSecondaryPeriod() || handwritingMarksShown()) {
                imeKeyFeedback.playFeedback(quickPunctuationButton);
                countKey(quickPunctuationButton);
                if (uriPeriodPrimary()) type(',');
                else type(commaSecondaryPeriod() ? '.' : '?');
            } else showQuickPunctuationMenu();
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
        bindInputMethodPicker(globeButton);
        schemeButton = shortcutButton(controls, "输入方式",
            KeyboardShortcutIconPolicy.Icon.SCHEME,
            deferrablePanelToggle(ToolbarOrderPolicy.Item.SCHEME,
                imeToolbar.panelToggle(() -> schemeScroll, imePanels::showSchemePicker)));
        schemeButton.setContentDescription("输入方案");
        skinButton = shortcutButton(controls, "皮肤",
            KeyboardShortcutIconPolicy.Icon.SKIN,
            deferrablePanelToggle(ToolbarOrderPolicy.Item.SKIN,
                imeToolbar.panelToggle(() -> skinScroll, () -> imePanels.showSkinMenu(skinButton))));
        skinButton.setContentDescription("切换键盘皮肤");
        layoutSettingsButton = shortcutButton(controls, "设置",
            KeyboardShortcutIconPolicy.Icon.SETTINGS, this::showLayoutSettings);
        layoutSettingsButton.setContentDescription("键盘设置");
        moreButton = brandButton(controls, imeFunctionPanel::toggleFunctionPanel);
        moreButton.setContentDescription("更多");
        // 收起键：面板开着时先回到键盘（描述随之变为「返回键盘」），没有面板时收起整个键盘。
        Button dismissButton = shortcutButton(controls, "收起",
            KeyboardShortcutIconPolicy.Icon.DISMISS, () -> {
                if (hardwareKeysCollapsed()) {
                    expandFromHardwareKeyboardMode();
                } else if (anyToolbarPanelOpen()) {
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
        expandedCandidates = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(expandedCandidates, 24, 16);
        ViewPolicy.setBackgroundColor(expandedCandidates, 0xfff5f5f5);
        expandedCandidates.setContentDescription("候选面板");
        ViewPolicy.hide(expandedCandidates);
        expandedCandidateScroll = new ScrollView(this);
        expandedCandidateScroll.setFillViewport(true);
        expandedCandidateScroll.addView(expandedCandidates,
            LayoutPolicy.scrollMatchParentParams());
        ViewPolicy.hide(expandedCandidateScroll);
        expandedCandidateCanvas = new CandidateCanvasFrame(this);
        expandedCandidateCanvas.addView(expandedCandidateScroll, LayoutPolicy.frameMatchParentParams());
        ViewPolicy.hide(expandedCandidateCanvas);
        keyboardSurface.addView(expandedCandidateCanvas, LayoutPolicy.frameMatchParentParams());
        keyboardSurface.addView(imeNineKeyPanel.build(), LayoutPolicy.frameMatchParentParams());
        clipboardSurface = new FrameLayout(this);
        ViewPolicy.setBackgroundColor(clipboardSurface, Color.parseColor(skin.background()));
        clipboardSurface.setContentDescription("剪贴板历史");
        clipboardHeader = LayoutPolicy.column(this);
        clipboardPanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(clipboardPanel, 24, 16);
        ViewPolicy.setBackgroundColor(clipboardPanel, Color.parseColor(skin.background()));
        clipboardScroll = new ScrollView(this);
        // 和候选、方案面板一样撑满整个键区：原先内容少时滚动视图本身是透明的，下面的键从空白处露出来。
        clipboardScroll.setFillViewport(true);
        clipboardScroll.addView(clipboardPanel, LayoutPolicy.scrollMatchParentParams());
        FrameLayout.LayoutParams clipboardScrollParams = LayoutPolicy.frameMatchParentParams();
        clipboardScrollParams.topMargin = pixels(32);
        clipboardSurface.addView(clipboardScroll, clipboardScrollParams);
        clipboardSurface.addView(clipboardHeader, LayoutPolicy.frameMatchWidthWrapParams());
        clipboardBackToTop = imePanels.buildClipboardBackToTop();
        FrameLayout.LayoutParams backToTopParams = new FrameLayout.LayoutParams(pixels(44), pixels(44),
            Gravity.BOTTOM | Gravity.END);
        backToTopParams.setMarginEnd(pixels(8));
        backToTopParams.bottomMargin = pixels(8);
        ViewPolicy.hide(clipboardBackToTop);
        clipboardSurface.addView(clipboardBackToTop, backToTopParams);
        // 「撤销」占住右下角，「返回顶部」和它同时出现时由 ImePanels.layoutClipboardFloating 挪到它上方。
        clipboardUndoButton = imePanels.buildClipboardUndo();
        FrameLayout.LayoutParams undoParams = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT,
            pixels(ImePanels.CLIPBOARD_UNDO_HEIGHT_DP), Gravity.BOTTOM | Gravity.END);
        undoParams.setMarginEnd(pixels(8));
        undoParams.bottomMargin = pixels(8);
        ViewPolicy.hide(clipboardUndoButton);
        clipboardSurface.addView(clipboardUndoButton, undoParams);
        clipboardScroll.setOnScrollChangeListener((view, x, y, oldX, oldY) -> imePanels.updateClipboardBackToTop());
        ViewPolicy.hide(clipboardSurface);
        keyboardSurface.addView(clipboardSurface, LayoutPolicy.frameMatchParentParams());
        schemePanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(schemePanel, 24, 16);
        ViewPolicy.setBackgroundColor(schemePanel, Color.parseColor(skin.background()));
        schemePanel.setContentDescription("输入方案选择器");
        schemeScroll = new ScrollView(this);
        schemeScroll.addView(schemePanel, LayoutPolicy.scrollMatchParentParams());
        schemeScroll.setFillViewport(true);
        ViewPolicy.hide(schemeScroll);
        keyboardSurface.addView(schemeScroll, LayoutPolicy.frameMatchParentParams());
        skinPanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(skinPanel, 24, 16);
        ViewPolicy.setBackgroundColor(skinPanel, Color.parseColor(skin.background()));
        skinPanel.setContentDescription("键盘皮肤选择器");
        skinScroll = new ScrollView(this);
        skinScroll.addView(skinPanel, LayoutPolicy.scrollMatchParentParams());
        skinScroll.setFillViewport(true);
        ViewPolicy.hide(skinScroll);
        keyboardSurface.addView(skinScroll, LayoutPolicy.frameMatchParentParams());
        layoutSettingsPanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(layoutSettingsPanel, 24, 16);
        ViewPolicy.setBackgroundColor(layoutSettingsPanel, Color.parseColor(skin.background()));
        layoutSettingsPanel.setContentDescription("键盘设置");
        LinearLayout layoutHeader = LayoutPolicy.row(this);
        TextView layoutTitle = ViewPolicy.textLabel(this, "键盘设置", 18);
        KeyboardTypographyPolicy.setKeyTextSize(layoutTitle, 18);
        layoutHeader.addView(layoutTitle, LayoutPolicy.weightedWrapParams(1));
        Button closeLayout = button(layoutHeader, "返回键盘", this::closeLayoutSettings);
        closeLayout.setContentDescription("返回键盘");
        layoutSettingsPanel.addView(layoutHeader);
        LinearLayout keyboardHeightHeader = LayoutPolicy.row(this);
        TextView keyboardHeightLabel = textView("键盘高度");
        keyboardHeightHeader.addView(keyboardHeightLabel, LayoutPolicy.weightedWrapParams(1));
        keyboardHeightValue = textView("");
        keyboardHeightHeader.addView(keyboardHeightValue);
        layoutSettingsPanel.addView(keyboardHeightHeader);
        keyboardHeightSlider = new SeekBar(this);
        keyboardHeightSlider.setContentDescription("键盘高度");
        configureHeightSlider(keyboardHeightSlider);
        layoutSettingsPanel.addView(keyboardHeightSlider);
        LinearLayout keySpacingHeader = LayoutPolicy.row(this);
        TextView keySpacingLabel = textView("按键间距");
        keySpacingHeader.addView(keySpacingLabel, LayoutPolicy.weightedWrapParams(1));
        keySpacingValue = textView("");
        keySpacingHeader.addView(keySpacingValue);
        layoutSettingsPanel.addView(keySpacingHeader);
        keySpacingSlider = new SeekBar(this);
        keySpacingSlider.setContentDescription("按键间距");
        configureSpacingSlider(keySpacingSlider, true);
        layoutSettingsPanel.addView(keySpacingSlider);
        LinearLayout rowSpacingHeader = LayoutPolicy.row(this);
        TextView rowSpacingLabel = textView("行间距");
        rowSpacingHeader.addView(rowSpacingLabel, LayoutPolicy.weightedWrapParams(1));
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
        keyboardSurface.addView(layoutSettingsScroll, LayoutPolicy.frameMatchParentParams());
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
        keyboardSurface.addView(layoutAdjustView, LayoutPolicy.frameMatchParentParams());
        voiceResultPanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(voiceResultPanel, 24, 16);
        ViewPolicy.setBackgroundColor(voiceResultPanel, Color.parseColor(skin.background()));
        voiceResultPanel.setContentDescription("语音结果面板");
        voiceResultScroll = new ScrollView(this);
        // 底色画在面板上，内容只有几行时也要撑满整块键区，否则下面的键从面板底边露出来半行。
        voiceResultScroll.setFillViewport(true);
        voiceResultScroll.addView(voiceResultPanel);
        ViewPolicy.hide(voiceResultScroll);
        keyboardSurface.addView(voiceResultScroll, LayoutPolicy.frameMatchParentParams());
        aiPolishContainer = LayoutPolicy.column(this);
        ViewPolicy.setBackgroundColor(aiPolishContainer, Color.parseColor(skin.background()));
        aiPolishPanel = LayoutPolicy.column(this);
        ViewPolicy.setSymmetricPadding(aiPolishPanel, 24, 16);
        ViewPolicy.setBackgroundColor(aiPolishPanel, Color.parseColor(skin.background()));
        aiPolishPanel.setContentDescription("AI 润色面板");
        aiPolishScroll = new ScrollView(this);
        aiPolishScroll.setFillViewport(true);
        aiPolishScroll.addView(aiPolishPanel);
        aiPolishContainer.addView(aiPolishScroll, LayoutPolicy.weightedWidthParams(1));
        aiPolishActions = LayoutPolicy.column(this);
        ViewPolicy.setPadding(aiPolishActions, 24, 0, 24, 16);
        aiPolishContainer.addView(aiPolishActions, LayoutPolicy.matchWidthWrapParams());
        ViewPolicy.hide(aiPolishContainer);
        keyboardSurface.addView(aiPolishContainer, LayoutPolicy.frameMatchParentParams());
        moreToolsPanel = LayoutPolicy.column(this);
        ViewPolicy.setPadding(moreToolsPanel, pixels(12), 0, pixels(12), pixels(MORE_TOOLS_BOTTOM_PADDING_DP));
        ViewPolicy.setBackgroundColor(moreToolsPanel, Color.parseColor(skin.background()));
        moreToolsScroll = new ScrollView(this);
        moreToolsScroll.setFillViewport(true);
        moreToolsScroll.setVerticalScrollBarEnabled(false);
        moreToolsScroll.addView(moreToolsPanel, LayoutPolicy.scrollMatchParentParams());
        ViewPolicy.hide(moreToolsScroll);
        keyboardSurface.addView(moreToolsScroll, LayoutPolicy.frameMatchParentParams());
        phrasePanel = LayoutPolicy.column(this);
        phrasePanel.setContentDescription("常用语面板");
        phraseScroll = new ScrollView(this);
        // 与皮肤、剪贴板面板一样铺满键盘区：只按内容高度时，没有常用语的那一行提示只盖住第一排键，空白处的触摸还会穿到下面的键上。
        phraseScroll.addView(phrasePanel, LayoutPolicy.scrollMatchParentParams());
        phraseScroll.setFillViewport(true);
        ViewPolicy.setClickable(phraseScroll, true);
        ViewPolicy.hide(phraseScroll);
        keyboardSurface.addView(phraseScroll, LayoutPolicy.frameMatchParentParams());
        textEditPanel = imeTextEditPanel.build();
        keyboardSurface.addView(textEditPanel, LayoutPolicy.frameMatchParentParams());
        imePanels.buildEmojiPanel();
        imePanels.buildSymbolPanel();
        renderLayoutSettingsState();
        render();
        synchronizeReplyKeyboard();
        // 建整棵键盘视图在主线程上，键盘进程冷启动后第一次弹出（以及换主题、旋转等配置变化后）都要走一遍（#9916）。
        imeDebugOverlay.recordPerf(DiagnosticEventKind.INPUT_VIEW_CREATE, SystemClock.uptimeMillis() - createStartedAt);
        return keyboardRoot;
    }

    /** 把从工具栏打开的面板（功能、候选展开、常用语、表情、符号、剪贴板、皮肤、输入方式、AI）的顶边对齐到顶部一行的底边：设计里这些面板打开时工具栏仍在上面。 */
    private void alignOverlaysBelowTopRow(View region) {
        View parent = (View) region.getParent();
        int top = (parent == null ? 0 : parent.getTop()) + region.getBottom();
        for (View overlay : new View[] {moreToolsScroll, expandedCandidateCanvas, imeNineKeyPanel.root(),
                phraseScroll, emojiPanel, symbolPanel, clipboardSurface, skinScroll, schemeScroll,
                aiPolishContainer, textEditPanel}) {
            if (overlay == null) continue;
            if (!(overlay.getLayoutParams() instanceof FrameLayout.LayoutParams params)
                    || params.topMargin == top) continue;
            params.topMargin = top;
            overlay.post(() -> overlay.setLayoutParams(params));
        }
    }

    /** 是否有从工具栏打开的面板（功能面板、表情、常用语、剪贴板、皮肤、方案、符号、AI、语音结果、旧的键盘设置）开着；开着时品牌键垫 accentSoft、收起键变为「返回键盘」。 */
    boolean anyToolbarPanelOpen() {
        return ViewPolicy.isVisible(moreToolsScroll) || ViewPolicy.isVisible(emojiPanel)
            || ViewPolicy.isVisible(phraseScroll)
            || ViewPolicy.isVisible(clipboardSurface) || ViewPolicy.isVisible(skinScroll)
            || ViewPolicy.isVisible(schemeScroll) || ViewPolicy.isVisible(symbolPanel)
            || ViewPolicy.isVisible(aiPolishContainer) || ViewPolicy.isVisible(voiceResultScroll)
            || ViewPolicy.isVisible(layoutSettingsScroll) || ViewPolicy.isVisible(layoutAdjustView)
            || ViewPolicy.isVisible(textEditPanel)
            || replyOpen;
    }

    void closeToolbarPanels() {
        // 推迟打开的皮肤或输入方式随面板一起作废：用户改点了别的面板或收起了键盘，它不该过一会儿自己弹出来。
        pendingToolbarPanel = null;
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
        imeTextEditPanel.close();
    }

    void closeCommonPhrases() {
        if (phraseScroll != null) ViewPolicy.hide(phraseScroll);
    }

    /** 功能面板「键盘调整」：整块键盘盖上遮罩，拖四条边和「移动」实时预览高度、底部抬高、宽度和水平位置；确认或任何离开方式（收起键盘、返回键、切应用、换输入框）都保存，没有取消。 */
    void showKeyboardAdjust() {
        if (touchGeometrySaving || traditionalOutputSaving || session == 0
                || preferencesSnapshot == null || preferencesDirectory.isEmpty()
                || keyboardAdjustOverlay == null) {
            notice("键盘设置尚未就绪");
            return;
        }
        closeToolbarPanels();
        closeCandidatePanel();
        // 拖动期间不让输入法窗口跟着键盘变高（#10025）：每次改窗口高度，应用都要按新 insets 重排，新尺寸的窗口在键区画好前先露出白底，拖动时整块闪白、应用上下跳。进入时铺满成透明全高窗口，应用的避让边界固定在键盘此刻的上沿，拖动只在窗口里伸缩、移动键盘；结束时才恢复停靠窗口，只改一次高度。
        // 功能面板盖在键区上、与键区同高，此刻的上沿就是应用正在避让的边界；面板收起后的布局还没发生，所以在这里记。浮动键盘不让应用避让，它的屏幕上沿不是边界，不记；调整中途停靠后由外框布局重新记下。
        if (overlayDrawn()) {
            keyboardAdjustKeyboardTop = KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP;
        } else {
            keyboardSurface.getLocationOnScreen(floatingLocation);
            keyboardAdjustKeyboardTop = floatingLocation[1];
        }
        keyboardAdjustConfiguration = new Configuration(getResources().getConfiguration());
        keyboardAdjustLiftDp = KeyboardAdjustPolicy.liftDp(localSettings.bool(AndroidLocalSettings.BOTTOM_PADDING),
            localSettings.integer(AndroidLocalSettings.BOTTOM_PADDING_HEIGHT), imeBottomBar.shown());
        keyboardAdjustOriginalLiftDp = keyboardAdjustLiftDp;
        keyboardAdjustHorizontal = savedKeyboardHorizontal();
        keyboardAdjustOriginalHorizontal = keyboardAdjustHorizontal;
        keyboardAdjustActive = true;
        render();
        imeStyler.applySkin();
        View top = keyboardAdjustOverlay.part(KeyboardAdjustPolicy.Part.TOP);
        if (top != null) top.requestFocus();
    }

    /** 遮罩打开时读屏只走遮罩：键盘列和底栏下面的按键只是预览，按不到，也不该被读屏选中。关上后交还。 */
    private void applyKeyboardAdjustAccessibility() {
        int mode = keyboardAdjustActive ? View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            : View.IMPORTANT_FOR_ACCESSIBILITY_AUTO;
        if (imeFrame.keyboard != null && imeFrame.keyboard.getImportantForAccessibility() != mode)
            imeFrame.keyboard.setImportantForAccessibility(mode);
        imeBottomBar.setImportantForAccessibility(mode);
    }

    /** 本地设置里存着的停靠键盘宽度与水平位置。 */
    private KeyboardAdjustPolicy.Horizontal savedKeyboardHorizontal() {
        return new KeyboardAdjustPolicy.Horizontal(localSettings.integer(AndroidLocalSettings.KEYBOARD_WIDTH_PERCENT),
            localSettings.integer(AndroidLocalSettings.KEYBOARD_POSITION_X));
    }

    /** 此刻哪几项生效（{@link KeyboardAdjustPolicy.Availability}）：调整期间实体键盘的候选条模式会临时展开键区，所以按候选条模式本身判断「收起键区」，与关上遮罩后的状态一致。 */
    private KeyboardAdjustPolicy.Availability keyboardAdjustAvailability() {
        return KeyboardAdjustPolicy.Availability.of(overlayDrawn(), hardwareKeyboardMode,
            imeBottomBar.phoneLandscape(), ImeFrame.oneHanded(ImeFrame.effectiveOneHanded(this)), splitKeyboardDrawn());
    }

    /** 宽度与水平位置此刻是否作用在停靠键盘上：条件同底部抬高（{@link KeyboardBottomBarPolicy#extraPaddingDp}），另外单手模式和分离式键盘时不生效。 */
    private boolean keyboardWidthApplies() {
        return !overlayDrawn() && !hardwareKeysCollapsed() && !imeBottomBar.phoneLandscape()
            && !ImeFrame.oneHanded(ImeFrame.effectiveOneHanded(this)) && !splitKeyboardDrawn();
    }

    /** 键盘外框在可用宽度 {@code availablePx} 里两侧各空出多少像素：调整期间按预览，否则按本地设置；宽度不生效时两侧都是 0。由外框测量时调用。 */
    int[] keyboardHorizontalInsets(int availablePx) {
        if (availablePx <= 0 || !keyboardWidthApplies()) return new int[] {0, 0};
        return KeyboardAdjustPolicy.insetsPx(keyboardAdjustActive ? keyboardAdjustHorizontal : savedKeyboardHorizontal(),
            availablePx, pixels(KeyboardAdjustPolicy.MIN_WIDTH_DP));
    }

    /** 键区下面此刻额外抬起多少 dp（{@link KeyboardBottomBarPolicy#extraPaddingDp}）：调整期间按预览的抬高，否则按本地设置的「键盘底部高度」。 */
    int keyboardLiftDp(boolean barShown) {
        boolean enabled = keyboardAdjustActive ? keyboardAdjustLiftDp > 0
            : localSettings.bool(AndroidLocalSettings.BOTTOM_PADDING);
        int height = keyboardAdjustActive ? KeyboardAdjustPolicy.storedBottomHeightDp(keyboardAdjustLiftDp, barShown)
            : localSettings.integer(AndroidLocalSettings.BOTTOM_PADDING_HEIGHT);
        return KeyboardBottomBarPolicy.extraPaddingDp(enabled, height, barShown, floatingDrawn(), hardwareKeysCollapsed(),
            imeBottomBar.phoneLandscape());
    }

    /** 套用一组预览值：变了的那几项各自重排，高度改行高，抬高改键盘列的底部内边距，宽度和位置改外框的左右内边距。 */
    private void previewKeyboardAdjust(int heightAdjustment, int liftDp, KeyboardAdjustPolicy.Horizontal horizontal) {
        if (!keyboardAdjustActive || touchGeometrySaving || traditionalOutputSaving) return;
        if (heightAdjustment != touchKeyboardHeightAdjustment) {
            touchKeyboardHeightAdjustment = KeyboardHeightPolicy.designHeightAdjustment(heightAdjustment);
            imeStyler.applyKeyboardGeometry();
        }
        if (liftDp != keyboardAdjustLiftDp) {
            keyboardAdjustLiftDp = liftDp;
            imeBottomBar.apply(imeFrame.keyboard);
        }
        if (!horizontal.equals(keyboardAdjustHorizontal)) {
            keyboardAdjustHorizontal = horizontal;
            keyboardSurface.requestLayout();
        }
    }

    /** 关上「键盘调整」并保存：高度经键盘的几何保存写本地设置，抬高、宽度和位置只写生效且变了的项，先套进内存里的本地设置，键盘不必等后台写完才按新值画。 */
    private void finishKeyboardAdjust() {
        if (!keyboardAdjustActive) return;
        java.util.Map<String, Object> edits = AndroidLocalSettings.keyboardAdjustEdits(keyboardAdjustAvailability(),
            imeBottomBar.shown(), keyboardAdjustOriginalLiftDp, keyboardAdjustLiftDp,
            keyboardAdjustOriginalHorizontal, keyboardAdjustHorizontal);
        keyboardAdjustActive = false;
        keyboardAdjustKeyboardTop = KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP;
        keyboardAdjustConfiguration = null;
        if (!edits.isEmpty()) localSettings = localSettings.withEdits(edits);
        saveTouchGeometry(false, edits);
        // render 里的 applyFloatingLayout 按最终键盘高度恢复停靠窗口，整个调整只在这里改一次窗口高度；根视图同时换回整块底色。
        render();
        imeStyler.applySkin();
        if (keyboardSurface != null) keyboardSurface.requestLayout();
    }

    /** 遮罩的拖动和读屏都换算成预览值；数值规则全在 {@link KeyboardAdjustPolicy}。 */
    private final KeyboardAdjustOverlay.Listener keyboardAdjustListener = new KeyboardAdjustOverlay.Listener() {
        @Override public void dragStarted(KeyboardAdjustPolicy.Part part) {
            keyboardAdjustDragHeight = touchKeyboardHeightAdjustment;
            keyboardAdjustDragLiftDp = keyboardAdjustLiftDp;
            keyboardAdjustDragHorizontal = keyboardAdjustHorizontal;
        }

        @Override public void dragged(KeyboardAdjustPolicy.Part part, float dxPx, float dyPx) {
            if (keyboardSurface == null) return;
            KeyboardAdjustPolicy.Availability availability = keyboardAdjustAvailability();
            int dyDp = Math.round(DimensionPolicy.fromPixels(MSIMEInputService.this, dyPx));
            int dx = Math.round(dxPx);
            int window = getResources().getConfiguration().screenHeightDp;
            boolean bar = imeBottomBar.shown();
            int available = keyboardSurface.getWidth();
            int minimum = pixels(KeyboardAdjustPolicy.MIN_WIDTH_DP);
            switch (part) {
                case TOP -> previewKeyboardAdjust(KeyboardAdjustPolicy.dragTop(keyboardAdjustDragHeight, dyDp, window),
                    keyboardAdjustLiftDp, keyboardAdjustHorizontal);
                case BOTTOM -> {
                    if (!availability.lift()) return;
                    KeyboardAdjustPolicy.Vertical vertical = KeyboardAdjustPolicy.dragBottom(keyboardAdjustDragHeight,
                        keyboardAdjustDragLiftDp, dyDp, window, bar);
                    previewKeyboardAdjust(vertical.heightAdjustment(), vertical.liftDp(), keyboardAdjustHorizontal);
                }
                case LEFT, RIGHT -> {
                    if (!availability.width()) return;
                    previewKeyboardAdjust(touchKeyboardHeightAdjustment, keyboardAdjustLiftDp, part == KeyboardAdjustPolicy.Part.LEFT
                        ? KeyboardAdjustPolicy.dragLeft(keyboardAdjustDragHorizontal, dx, available, minimum)
                        : KeyboardAdjustPolicy.dragRight(keyboardAdjustDragHorizontal, dx, available, minimum));
                }
                case MOVE -> {
                    if (!availability.lift()) return;
                    previewKeyboardAdjust(touchKeyboardHeightAdjustment,
                        KeyboardAdjustPolicy.dragLift(keyboardAdjustDragLiftDp, dyDp, bar),
                        availability.width() ? KeyboardAdjustPolicy.dragPosition(keyboardAdjustDragHorizontal, dx,
                            available, minimum) : keyboardAdjustHorizontal);
                }
            }
        }

        @Override public void dragEnded(KeyboardAdjustPolicy.Part part) { }

        @Override public void step(KeyboardAdjustPolicy.Part part, int direction, boolean vertical) {
            if (keyboardSurface == null) return;
            KeyboardAdjustPolicy.Availability availability = keyboardAdjustAvailability();
            int window = getResources().getConfiguration().screenHeightDp;
            boolean bar = imeBottomBar.shown();
            int available = keyboardSurface.getWidth();
            int minimum = pixels(KeyboardAdjustPolicy.MIN_WIDTH_DP);
            switch (part) {
                case TOP -> previewKeyboardAdjust(KeyboardAdjustPolicy.stepHeight(touchKeyboardHeightAdjustment,
                    direction, window), keyboardAdjustLiftDp, keyboardAdjustHorizontal);
                case BOTTOM -> {
                    if (!availability.lift()) return;
                    KeyboardAdjustPolicy.Vertical next = KeyboardAdjustPolicy.stepBottom(touchKeyboardHeightAdjustment,
                        keyboardAdjustLiftDp, direction, window, bar);
                    previewKeyboardAdjust(next.heightAdjustment(), next.liftDp(), keyboardAdjustHorizontal);
                }
                case LEFT, RIGHT -> {
                    if (!availability.width()) return;
                    previewKeyboardAdjust(touchKeyboardHeightAdjustment, keyboardAdjustLiftDp,
                        KeyboardAdjustPolicy.stepWidth(keyboardAdjustHorizontal, part, direction, available, minimum));
                }
                case MOVE -> {
                    if (!availability.lift()) return;
                    if (availability.movesHorizontally() && !vertical) {
                        previewKeyboardAdjust(touchKeyboardHeightAdjustment, keyboardAdjustLiftDp,
                            KeyboardAdjustPolicy.stepPosition(keyboardAdjustHorizontal, direction));
                    } else {
                        previewKeyboardAdjust(touchKeyboardHeightAdjustment, KeyboardAdjustPolicy.dragLift(
                            keyboardAdjustLiftDp, -Integer.signum(direction) * KeyboardAdjustPolicy.LIFT_STEP_DP, bar),
                            keyboardAdjustHorizontal);
                    }
                }
            }
        }

        @Override public KeyboardAdjustPolicy.Range range(KeyboardAdjustPolicy.Part part, boolean vertical) {
            int window = getResources().getConfiguration().screenHeightDp;
            boolean bar = imeBottomBar.shown();
            int available = keyboardSurface == null ? 0 : keyboardSurface.getWidth();
            int minimum = pixels(KeyboardAdjustPolicy.MIN_WIDTH_DP);
            return switch (part) {
                case TOP -> KeyboardAdjustPolicy.heightRange(touchKeyboardHeightAdjustment, window);
                case BOTTOM -> KeyboardAdjustPolicy.bottomRange(touchKeyboardHeightAdjustment, keyboardAdjustLiftDp,
                    window, bar);
                case LEFT, RIGHT -> KeyboardAdjustPolicy.widthRange(keyboardAdjustHorizontal, available, minimum);
                case MOVE -> keyboardAdjustAvailability().movesHorizontally() && !vertical
                    ? KeyboardAdjustPolicy.positionRange(keyboardAdjustHorizontal, available, minimum)
                    : KeyboardAdjustPolicy.liftRange(keyboardAdjustLiftDp, bar);
            };
        }

        @Override public void reset() {
            previewKeyboardAdjust(KeyboardHeightPolicy.DEFAULT_HEIGHT_ADJUSTMENT_DP, 0,
                KeyboardAdjustPolicy.Horizontal.DEFAULT);
        }

        @Override public void confirm() {
            finishKeyboardAdjust();
        }
    };

    /** 触控板模式保存后接回统一快照；密码等直通编辑器没有 Engine 会话，也能保存面板选择。 */
    void acceptTextTrackpadPreferences(JSONObject snapshot) {
        try {
            long revision = PreferencesRevisionPolicy.read(snapshot.opt("revision"), -1);
            long current = preferencesSnapshot == null ? -1
                : PreferencesRevisionPolicy.read(preferencesSnapshot.opt("revision"), -1);
            if (revision < 0 || revision < current) return;
            if (session != 0) applyPreferencesSnapshot(snapshot);
            else {
                preferencesSnapshot = snapshot;
                JSONObject preferences = snapshot.getJSONObject("preferences");
                applyEditorPreferences(preferences);
            }
        } catch (JSONException | LinkageError error) {
            notice("触控板设置已保存，重新打开键盘后生效");
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
            notice(label + "设置尚未就绪");
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
            notice(label + "未能保存");
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

    /** 剪贴板面板「本机」页上的「开启剪贴板历史」：写共享偏好 `clipboard_history`，与设置「隐私」里的开关是同一项。保存成功后 {@link #applyPreferencesSnapshot} 发现它刚打开，会重画开着的剪贴板面板。 */
    void enableClipboardHistory() {
        savePanelPreference("剪贴板历史", preferences -> preferences.put("clipboard_history", true));
    }

    void toggleIncognito() {
        saveLocalPanelSetting("无痕输入", AndroidLocalSettings.INCOGNITO, !incognitoEnabled);
    }

    /** 功能面板「浮动键盘」与工具栏的浮动键盘按钮：切换浮动 / 停靠。 */
    void toggleFloatingKeyboard() {
        setFloatingKeyboard(!floatingKeyboard);
    }

    private void setFloatingKeyboard(boolean enabled) {
        saveLocalPanelSetting("浮动键盘", AndroidLocalSettings.FLOATING_KEYBOARD, enabled);
    }

    /** 浮动键盘此刻是否生效（{@link FloatingKeyboardPolicy#active}）：外接键盘的候选条模式里停在底部。 */
    boolean floatingDrawn() {
        return FloatingKeyboardPolicy.active(floatingKeyboard, hardwareKeyboardMode);
    }

    /** 候选栏此刻是否跟随光标（{@link CaretCandidatePolicy#active}）：外接键盘的候选条模式里、开关打开且这个输入框报告光标位置时。 */
    boolean caretCandidatesDrawn() {
        return CaretCandidatePolicy.active(caretCandidates, hardwareKeysCollapsed(), caretAnchorsAvailable(), caretDocked);
    }

    /** 键盘外框此刻是否悬在应用上面（浮动键盘或跟随光标的候选栏）：输入法窗口铺满、根视图透明、只有外框接收触摸。两者互斥：浮动键盘在候选条模式里不生效。 */
    boolean overlayDrawn() {
        return floatingDrawn() || caretCandidatesDrawn();
    }

    /** Android 9 的可见窗口扩高时会先沿用旧 insets，停靠窗口从首次显示起保持全高。 */
    boolean stableDockedWindow() {
        return android.os.Build.VERSION.SDK_INT == 28;
    }

    /** 两种编辑预览各自持有透明父窗口，只有最后一个结束时才恢复停靠窗口。 */
    boolean editorPreviewExpanded() {
        return clipboardPreviewExpanded || textTrackpadPreviewExpanded;
    }

    void setClipboardPreviewExpanded(boolean expanded) {
        if (clipboardPreviewExpanded == expanded) return;
        if (expanded) {
            clipboardPreviewKeyboardHeight = keyboardSurface.getHeight();
            keyboardSurface.getLocationOnScreen(floatingLocation);
            clipboardPreviewKeyboardTop = floatingLocation[1];
        }
        if (expanded) clipboardPreviewKeyboardHeight = keyboardSurface.getHeight();
        boolean previous = editorPreviewExpanded();
        clipboardPreviewExpanded = expanded;
        updateEditorPreviewWindow(previous);
    }

    void setTextTrackpadPreviewExpanded(boolean expanded) {
        if (textTrackpadPreviewExpanded == expanded) return;
        boolean previous = editorPreviewExpanded();
        textTrackpadPreviewExpanded = expanded;
        updateEditorPreviewWindow(previous);
    }

    private void updateEditorPreviewWindow(boolean previous) {
        boolean expanded = editorPreviewExpanded();
        if (expanded == previous) return;
        if (expanded) editorPreviewKeyboardHeight = keyboardSurface.getHeight();
        applyFloatingLayout(true);
        imeStyler.applySkin();
    }

    /**
     * 把浮动 / 停靠套到输入法窗口上。浮动（以及候选栏跟随光标）时窗口、系统的 inputArea 与键盘根视图都铺满屏幕（根视图透明，见 ImeStyler.applySkin），键盘外框缩窄成一块面板，{@link #onComputeInsets} 只把面板算作可触摸区域、内容区不占高度，所以应用既不被顶起也不被压缩，面板外的触摸落到应用上。停靠时把窗口和 inputArea 恢复成第一次浮动前的高度。
     * 剪贴板全文预览同样使用透明全高窗口，但停靠键区保留原高，应用仍只为键区让出空间。「键盘调整」打开期间也铺满，键盘外框包裹内容贴底，拖动时只在窗口里伸缩、移动，不改窗口高度。API 28 从首次显示起固定全高，避免扩窗先使用旧 insets 使编辑器丢焦点。
     *
     * @param force 系统重建了窗口里的布局（{@link #setInputView}、{@link #updateFullscreenMode}）时为真，状态没变也重新套一遍
     */
    private void applyFloatingLayout(boolean force) {
        boolean caret = caretCandidatesDrawn();
        boolean floating = caret || floatingDrawn();
        boolean fullWindow = stableDockedWindow() || floating || editorPreviewExpanded() || keyboardAdjustActive;
        if (!force && fullWindow == fullWindowLayoutApplied
                && floating == overlayLayoutApplied && caret == caretLayoutApplied) return;
        // 「键盘调整」期间在浮动与停靠之间切换（接上或拔下实体键盘）：记下的上沿属于另一种布局，作废后等外框在新位置布局时重新记下，否则应用会停在浮动面板的上沿被截掉一截。
        if (keyboardAdjustActive && floating != overlayLayoutApplied)
            keyboardAdjustKeyboardTop = KeyboardHeightPolicy.UNRECORDED_KEYBOARD_TOP;
        fullWindowLayoutApplied = fullWindow;
        overlayLayoutApplied = floating;
        caretLayoutApplied = caret;
        android.app.Dialog dialog = getWindow();
        android.view.Window window = dialog == null ? null : dialog.getWindow();
        if (window != null) {
            android.view.WindowManager.LayoutParams attributes = window.getAttributes();
            View inputArea = window.findViewById(android.R.id.inputArea);
            ViewGroup.LayoutParams areaParams = inputArea == null ? null : inputArea.getLayoutParams();
            if (fullWindow && dockedWindowHeight == Integer.MIN_VALUE) dockedWindowHeight = attributes.height;
            if (fullWindow && areaParams != null && dockedInputAreaHeight == Integer.MIN_VALUE)
                dockedInputAreaHeight = areaParams.height;
            int windowHeight = fullWindow ? ViewGroup.LayoutParams.MATCH_PARENT : dockedWindowHeight;
            if (windowHeight != Integer.MIN_VALUE && attributes.height != windowHeight) {
                attributes.height = windowHeight;
                window.setAttributes(attributes);
            }
            int areaHeight = fullWindow ? ViewGroup.LayoutParams.MATCH_PARENT : dockedInputAreaHeight;
            if (areaParams != null && areaHeight != Integer.MIN_VALUE && areaParams.height != areaHeight) {
                areaParams.height = areaHeight;
                // 铺满时 inputArea 仍贴着窗口底部，停靠后的包裹高度同样贴底，与系统默认一致。
                if (areaParams instanceof LinearLayout.LayoutParams linear) linear.gravity = Gravity.BOTTOM;
                else if (areaParams instanceof FrameLayout.LayoutParams frame) frame.gravity = Gravity.BOTTOM;
                inputArea.setLayoutParams(areaParams);
            }
        }
        if (keyboardRoot != null && keyboardRoot.getLayoutParams() != null) {
            ViewGroup.LayoutParams params = keyboardRoot.getLayoutParams();
            int height = fullWindow ? ViewGroup.LayoutParams.MATCH_PARENT : ViewGroup.LayoutParams.WRAP_CONTENT;
            if (params.height != height) {
                params.height = height;
                keyboardRoot.setLayoutParams(params);
            }
        }
        // 拖动条只属于浮动键盘；跟随光标的候选栏由光标位置决定，不能拖。
        if (floatingBar != null) ViewPolicy.setVisible(floatingBar, floating && !caret);
        imeStyler.applyKeyboardSurfaceGeometry();
        if (imeFrame.keyboard != null) imeFrame.keyboard.requestApplyInsets();
        positionKeyboardSurface();
    }

    @Override public void setInputView(View view) {
        super.setInputView(view);
        // 系统把新的输入视图按包裹高度放进 inputArea；需要全高窗口时在首次显示前重新铺满。
        applyFloatingLayout(true);
    }

    @Override public void updateFullscreenMode() {
        super.updateFullscreenMode();
        if (fullWindowLayoutApplied) applyFloatingLayout(true);
    }

    /**
     * 浮动时：内容区与可见区都从窗口底边算起（应用不让出高度），只有面板接收触摸。停靠时用系统默认的计算。跟随光标的候选栏同样如此；它空闲时不显示，可触摸区域为空，所有触摸都落到应用上。
     * 全文预览、「键盘调整」及 API 28 的透明全高窗口只将键盘外框划为根窗口触摸范围；停靠键区的上沿仍是应用让出空间的边界，全文预览和「键盘调整」期间固定为打开时记下的上沿。
     */
    @Override public void onComputeInsets(InputMethodService.Insets outInsets) {
        super.onComputeInsets(outInsets);
        // 浮动时 inputArea 铺满窗口，系统默认的算法会把整个窗口算成键盘、把应用压没，所以这里总要覆盖；面板还没布局时可触摸区域为空。
        // 候选栏、剪贴板预览和文本编辑放大条共用同一块输入区；停靠键盘只在稳定布局后参与 inset 计算。
        if ((!overlayDrawn() && !clipboardPreviewExpanded && !editorPreviewExpanded()
                && !stableDockedWindow() && !keyboardAdjustActive) || keyboardSurface == null) return;
        android.app.Dialog dialog = getWindow();
        android.view.Window window = dialog == null ? null : dialog.getWindow();
        if (window == null) return;
        int height = window.getDecorView().getHeight();
        outInsets.contentTopInsets = height;
        outInsets.visibleTopInsets = height;
        keyboardSurface.getLocationInWindow(floatingLocation);
        if (clipboardPreviewExpanded && !overlayDrawn()) {
            // 窗口扩展与键区布局可能分两拍完成；始终保留原屏幕边界，避免过渡坐标把编辑器压成零高。
            int[] windowOrigin = new int[2];
            window.getDecorView().getLocationOnScreen(windowOrigin);
            int keyboardTop = Math.max(0, clipboardPreviewKeyboardTop - windowOrigin[1]);
            outInsets.contentTopInsets = keyboardTop;
            outInsets.visibleTopInsets = keyboardTop;
        } else if (keyboardAdjustActive && !overlayDrawn()) {
            // 「键盘调整」时应用的边界固定在进入时的键盘上沿，键盘变高、抬高就盖住应用的底部，变矮就露出下面的应用窗口，应用本身不重排。
            int[] windowOrigin = new int[2];
            window.getDecorView().getLocationOnScreen(windowOrigin);
            int keyboardTop = KeyboardHeightPolicy.adjustingInsetTop(
                keyboardAdjustKeyboardTop, windowOrigin[1], floatingLocation[1]);
            outInsets.contentTopInsets = keyboardTop;
            outInsets.visibleTopInsets = keyboardTop;
        } else if (!overlayDrawn()) {
            outInsets.contentTopInsets = floatingLocation[1];
            outInsets.visibleTopInsets = floatingLocation[1];
        }
        outInsets.touchableInsets = InputMethodService.Insets.TOUCHABLE_INSETS_REGION;
        if (keyboardSurface.getVisibility() != View.VISIBLE) {
            outInsets.touchableRegion.setEmpty();
            return;
        }
        outInsets.touchableRegion.set(floatingLocation[0], floatingLocation[1],
            floatingLocation[0] + keyboardSurface.getWidth(), floatingLocation[1] + keyboardSurface.getHeight());
    }

    /** 可移动的水平 / 竖直像素范围：系统栏之间的区域减去面板大小。 */
    private int floatingFreeWidth() {
        return keyboardRoot.getWidth() - systemBarInsets.left - systemBarInsets.right - keyboardSurface.getWidth();
    }

    private int floatingFreeHeight() {
        return keyboardRoot.getHeight() - systemBarInsets.top - systemBarInsets.bottom - keyboardSurface.getHeight();
    }

    /** 按存着的千分比把浮动面板摆进系统栏之间；停靠时清掉平移。面板贴在根视图左上角，位置全靠平移，拖动时不触发重新布局。 */
    void positionFloatingKeyboard() {
        if (keyboardSurface == null || keyboardRoot == null) return;
        if (!floatingDrawn()) {
            keyboardSurface.setTranslationX(0f);
            keyboardSurface.setTranslationY(0f);
            return;
        }
        if (floatingDragging) return;
        keyboardSurface.setTranslationX(systemBarInsets.left
            + FloatingKeyboardPolicy.offset(floatingX, floatingFreeWidth()));
        keyboardSurface.setTranslationY(systemBarInsets.top
            + FloatingKeyboardPolicy.offset(floatingY, floatingFreeHeight()));
    }

    /** 按当前布局摆放键盘外框：跟随光标的候选栏按光标摆，其余情况交给浮动键盘（停靠时清掉平移）。 */
    void positionKeyboardSurface() {
        if (caretCandidatesDrawn()) positionCaretCandidates();
        else positionFloatingKeyboard();
    }

    /**
     * 把候选栏摆到光标旁（{@link CaretCandidatePolicy#x} / {@link CaretCandidatePolicy#y}）：光标的屏幕坐标换算成根视图里的坐标，范围是系统栏之间的区域，所以旋转、分屏或窗口变化后仍在窗口里。还没收到光标位置时候选栏不显示，先按底边居中摆好。
     */
    private void positionCaretCandidates() {
        if (keyboardSurface == null || keyboardRoot == null) return;
        keyboardRoot.getLocationOnScreen(caretRootLocation);
        float left = caretAnchorKnown ? caretAnchorLeft - caretRootLocation[0] : Float.NaN;
        float top = caretAnchorKnown ? caretAnchorTop - caretRootLocation[1] : Float.NaN;
        float bottom = caretAnchorKnown ? caretAnchorBottom - caretRootLocation[1] : Float.NaN;
        int x = CaretCandidatePolicy.x(left, keyboardSurface.getWidth(), systemBarInsets.left,
            keyboardRoot.getWidth() - systemBarInsets.right);
        int y = CaretCandidatePolicy.y(top, bottom, keyboardSurface.getHeight(), systemBarInsets.top,
            keyboardRoot.getHeight() - systemBarInsets.bottom, pixels(CaretCandidatePolicy.GAP_DP));
        keyboardSurface.setTranslationX(x);
        keyboardSurface.setTranslationY(y);
    }

    /** 当前输入框是否答应报告光标位置：要过位置的正是此刻的 InputConnection，且它没有拒绝。 */
    private boolean caretAnchorsAvailable() {
        return caretAnchorsReported && caretAnchorConnection != null && caretAnchorConnection == connection;
    }

    /** 换输入框或离开输入框：上一个输入框的光标位置和「这一次退回底部」都不再适用，下一次 render 重新要位置。 */
    private void resetCaretAnchor() {
        main.removeCallbacks(caretAnchorWaitExpired);
        caretAnchorConnection = null;
        caretAnchorsReported = false;
        caretAnchorKnown = false;
        caretAnchorPending = false;
        caretDocked = false;
    }

    /**
     * 统一维护物理键盘候选栏与触控板放大镜的光标位置订阅。任一消费者需要时持续报告并立即报一次，只有两者都结束才撤回。物理键盘需在第一次按键前订阅，避免第一次组字因位置尚未到达而退回底部；每次 render 开头调用，状态没变时不重复请求。
     */
    void synchronizeCursorUpdates() {
        InputConnection current = connection;
        boolean wanted = current != null && ((caretCandidates && EditorPolicy.useEngine(editorInputType)
            && (hardwareKeyboardAttached || hardwareKeyboardMode)) || imeTextEditPanel.needsCursorUpdates());
        boolean requested = caretAnchorConnection != null && caretAnchorConnection == current;
        if (wanted == requested) return;
        if (requested) InputConnectionGuard.requestCursorUpdates(current, 0);
        resetCaretAnchor();
        if (!wanted) return;
        caretAnchorConnection = current;
        caretAnchorsReported = InputConnectionGuard.requestCursorUpdates(current,
            InputConnection.CURSOR_UPDATE_MONITOR | InputConnection.CURSOR_UPDATE_IMMEDIATE);
        caretAnchorPending = caretAnchorsReported;
        if (caretAnchorPending) main.postDelayed(caretAnchorWaitExpired, CaretCandidatePolicy.ANCHOR_WAIT_MS);
    }

    /**
     * 编辑器报来光标位置（{@link #synchronizeCursorUpdates} 要的）：有组字时左边取组字第一个字，上下取整段组字和插入点所占的行（组字折行时候选栏放在最后一行下方或第一行上方），否则取插入点，经 `getMatrix()` 换成屏幕坐标记下，候选栏跟着挪。这一次因为没有位置退回了底部的，空闲时回到光标旁（{@link CaretCandidatePolicy#dockedAfterAnchor}）。
     */
    @Override public void onUpdateCursorAnchorInfo(android.view.inputmethod.CursorAnchorInfo info) {
        super.onUpdateCursorAnchorInfo(info);
        imeTextEditPanel.cursorAnchor(info);
        if (info == null || !caretAnchorsAvailable()) return;
        int composingStart = info.getComposingTextStart();
        android.graphics.RectF character = composingStart < 0 ? null : info.getCharacterBounds(composingStart);
        android.graphics.RectF bounds;
        if (character != null) {
            bounds = new android.graphics.RectF(character);
            // 左边对齐组字起点，上下边取整段组字和插入点所占的行：组字折到下一行时（笔画字形、超过一行宽的拼音），只按第一个字摆，候选栏会压在后面几行的组字和插入点上。
            CharSequence composing = info.getComposingText();
            int composingEnd = composingStart + (composing == null ? 0 : composing.length());
            for (int index = composingStart + 1; index < composingEnd; index++) {
                android.graphics.RectF next = info.getCharacterBounds(index);
                if (next == null) continue;
                bounds.top = Math.min(bounds.top, next.top);
                bounds.bottom = Math.max(bounds.bottom, next.bottom);
            }
            if (!Float.isNaN(info.getInsertionMarkerHorizontal())) {
                bounds.top = Math.min(bounds.top, info.getInsertionMarkerTop());
                bounds.bottom = Math.max(bounds.bottom, info.getInsertionMarkerBottom());
            }
        } else {
            float x = info.getInsertionMarkerHorizontal();
            // 这一次既没有组字外框也没有插入点（编辑器还没排版好）：保留上一次的位置。
            if (Float.isNaN(x)) return;
            bounds = new android.graphics.RectF(x, info.getInsertionMarkerTop(), x, info.getInsertionMarkerBottom());
        }
        info.getMatrix().mapRect(bounds);
        main.removeCallbacks(caretAnchorWaitExpired);
        caretAnchorPending = false;
        caretAnchorKnown = true;
        caretAnchorLeft = bounds.left;
        caretAnchorTop = bounds.top;
        caretAnchorBottom = bounds.bottom;
        if (keyboardRoot == null) return;
        boolean docked = CaretCandidatePolicy.dockedAfterAnchor(caretDocked, caretStripShown);
        if (docked != caretDocked) {
            // 回到光标旁要换窗口布局和根视图底色，走一遍完整的 render。
            caretDocked = docked;
            render();
            return;
        }
        // 等到第一次报告时候选栏才显示出来；此后只是挪位置。
        applyCaretCandidates(caretStripShown);
    }

    /**
     * 按候选栏该不该显示（组字、候选或提示）更新跟随光标的状态，每次 render 和每次收到光标位置时调用：要显示而既不知道光标在哪儿、也不在等第一次报告时，这一次退回底部并立即换成停靠布局；跟随光标时候选栏只在有内容且已知位置时显示（不显示时仍占着布局，位置可以先算好），显示时摆到光标旁。
     */
    private void applyCaretCandidates(boolean stripShown) {
        caretStripShown = stripShown;
        boolean following = caretCandidates && hardwareKeysCollapsed() && caretAnchorsAvailable();
        boolean docked = CaretCandidatePolicy.docked(caretDocked, following, stripShown, caretAnchorKnown,
            caretAnchorPending);
        if (docked != caretDocked) {
            caretDocked = docked;
            applyFloatingLayout(false);
            imeBottomBar.apply(imeFrame.keyboard);
        }
        if (keyboardSurface == null) return;
        boolean caret = caretCandidatesDrawn();
        int visibility = CaretCandidatePolicy.surfaceShown(caret, stripShown, caretAnchorKnown)
            ? View.VISIBLE : View.INVISIBLE;
        if (keyboardSurface.getVisibility() != visibility) keyboardSurface.setVisibility(visibility);
        if (caret) positionCaretCandidates();
        synchronizeCaretWindow();
    }

    /**
     * 跟随光标的候选栏空闲时收起整个输入法窗口，候选栏又有内容时再叫出来（{@link CaretCandidatePolicy#hidesWindow}）。只叫回自己收起的窗口：用户按返回键收起的，照旧等实体键盘再开始组字时由 {@link #noteHardwareTyping} 叫出来。收起请求还没落地时不重复发；落地前又开始打字的，由 {@link #onFinishInputView} 落地后再叫回来。应用在键盘收起后又立即要求显示时，隔 {@link CaretCandidatePolicy#WINDOW_REHIDE_MS} 才再收，不和它来回打转。
     */
    private void synchronizeCaretWindow() {
        main.removeCallbacks(caretWindowSync);
        if (CaretCandidatePolicy.hidesWindow(caretCandidatesDrawn(), caretStripShown)) {
            if (caretWindowHidden || !isInputViewShown()) return;
            long now = android.os.SystemClock.uptimeMillis();
            long wait = CaretCandidatePolicy.rehideDelay(caretWindowHiddenAt, now);
            if (wait > 0) {
                main.postDelayed(caretWindowSync, wait);
                return;
            }
            caretWindowHidden = true;
            caretHidePending = true;
            caretWindowHiddenAt = now;
            requestHideSelf(0);
        } else if (caretWindowHidden && !isInputViewShown()) {
            requestShowSelf(0);
        }
    }

    private void startFloatingDrag() {
        if (!floatingDrawn() || keyboardSurface == null) return;
        floatingDragging = true;
        floatingDragStartX = keyboardSurface.getTranslationX() - systemBarInsets.left;
        floatingDragStartY = keyboardSurface.getTranslationY() - systemBarInsets.top;
    }

    private void dragFloatingKeyboard(float dx, float dy) {
        if (!floatingDragging || keyboardSurface == null || keyboardRoot == null) return;
        int freeWidth = floatingFreeWidth();
        int freeHeight = floatingFreeHeight();
        float x = FloatingKeyboardPolicy.dragged(floatingDragStartX, dx, freeWidth);
        float y = FloatingKeyboardPolicy.dragged(floatingDragStartY, dy, freeHeight);
        keyboardSurface.setTranslationX(systemBarInsets.left + x);
        keyboardSurface.setTranslationY(systemBarInsets.top + y);
        floatingX = FloatingKeyboardPolicy.fractionOf(x, freeWidth);
        floatingY = FloatingKeyboardPolicy.fractionOf(y, freeHeight);
    }

    /** 松手：重新布局一次让可触摸区域跟到新位置，再把位置写进本地设置。 */
    private void endFloatingDrag() {
        if (!floatingDragging) return;
        floatingDragging = false;
        if (keyboardRoot != null) keyboardRoot.requestLayout();
        final int x = floatingX;
        final int y = floatingY;
        final long operation = ++floatingPositionSaveGeneration;
        Runnable save = () -> {
            java.util.Map<String, Object> edits = new java.util.LinkedHashMap<>(2);
            edits.put(AndroidLocalSettings.FLOATING_KEYBOARD_X, x);
            edits.put(AndroidLocalSettings.FLOATING_KEYBOARD_Y, y);
            AndroidLocalSettings.Snapshot saved;
            try {
                saved = AndroidLocalSettings.update(this, edits);
            } catch (java.io.IOException | RuntimeException error) {
                saved = null;
            }
            final AndroidLocalSettings.Snapshot result = saved;
            // 写不进去时位置只在这次键盘进程里有效，不打扰用户；写成功后让内存里的快照与文件一致。
            main.post(() -> {
                if (result != null && operation == floatingPositionSaveGeneration) localSettings = result;
            });
        };
        try {
            preferencesWorker.execute(save);
        } catch (RuntimeException error) {
            // 键盘正在销毁，工作线程已关：这次位置不保存。
        }
    }

    /** 功能面板上写本地设置的开关（单手、隐私）：在偏好线程上写文件，写好后在主线程生效；隐私模式变了还要把会话的学习开关重新推给引擎。 */
    private void saveLocalPanelSetting(String label, String key, Object value) {
        if (panelPreferenceSaving) {
            notice(label + "设置尚未就绪");
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
            notice(label + "未能保存");
            render();
            return;
        }
        boolean wasIncognito = incognitoEnabled;
        localSettings = saved;
        applyLocalSettings();
        preferencesNotice = "";
        if (wasIncognito != incognitoEnabled && session != 0) {
            // 统计与剪贴板等 Java 隐私门禁立即生效；引擎的 learning 只能在会话边界更新，延到键盘收起时重建，避免整窗闪烁。
            markPrivateSession();
            privacySessionRestartPending = true;
        }
        render();
    }

    /**
     * 隐私模式中途切换时重建引擎会话。同一修订号的快照只改 learning 会被 host-api 当作冲突拒绝（Session::update），所以不能靠 applyPreferencesSnapshot 把学习开关推给现有会话；新会话按当前的学习开关创建，随后 preferencesReloader 的首次应用与它一致，不会冲突。
     */
    private void restartSessionForPrivacy() {
        if (session == 0 || runtimeOptionsBase.isEmpty()) return;
        long startGeneration = ++engineStartGeneration;
        if (connection != null && view != null && !InputViewValuePolicy.editingText(view).isEmpty())
            command(FINISH_COMPOSITION_COMMAND);
        boolean panelOpen = ViewPolicy.isVisible(moreToolsScroll);
        preserveMoreToolsOnStop = panelOpen;
        stop(false);
        try {
            JSONObject options = new JSONObject(runtimeOptionsBase);
            if (learningSuppressed()) options.getJSONObject("preferences").put("learning", false);
            runtimeOptionsForSnapshot = options.toString();
            message = EngineStartupStatusPolicy.restartingMessage();
            scheduleEngineStartup(runtimeOptionsForSnapshot, startGeneration);
        } catch (JSONException error) {
            message = "共享运行时未就绪：仅直接输入";
        }
    }

    long inputLifecycleGeneration() { return engineStartGeneration; }

    /** 功能面板的手写：切到手写，并记下进入前的入口和中英状态；正在手写时回到记下的那个（#9703）。0.5.0 回到的是启用列表里第一个非手写入口，默认全拼 9 键、英文 26 键或小鹤双拼的用户关掉手写都落到全拼 26 键。
     *
     * <p>英文盖在手写上时（选中的是手写、中英键切到了英文）键盘画的是英文，这时点磁贴是要手写，与鸿蒙、iOS 相同。
     */
    void toggleHandwritingScheme() {
        if (selectedScheme == KeyboardScheme.HANDWRITING && !dedicatedEnglish) {
            KeyboardScheme back = KeyboardScheme.handwritingReturn(schemeBeforeHandwriting, enabledSchemes, edition);
            selectKeyboardScheme(back, back == schemeBeforeHandwriting && englishBeforeHandwriting);
            return;
        }
        KeyboardScheme previous = selectedScheme;
        boolean english = dedicatedEnglish;
        if (selectKeyboardScheme(KeyboardScheme.HANDWRITING, false)
                && previous != KeyboardScheme.HANDWRITING) {
            schemeBeforeHandwriting = previous;
            englishBeforeHandwriting = english;
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
            // 原先把异常整个吞掉，用户在定制系统上报来「无法打开设置」时，截图和日志里都看不出是哪一种拦截。提示里带上异常类型，日志里留下完整堆栈。
            android.util.Log.w("MSIMEHost", "Opening host page " + page + " failed", error);
            notice("无法打开设置，请从主屏幕进入（" + error.getClass().getSimpleName() + "）");
        }
    }

    /** 从键盘功能面板直接打开官网；成功后收起输入法，回到系统浏览器。 */
    void openWebsite() {
        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse("https://msime.app/"))
            .addCategory(Intent.CATEGORY_BROWSABLE)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        try {
            closeMoreTools();
            startActivity(intent);
            requestHideSelf(0);
        } catch (ActivityNotFoundException unavailable) {
            notice("没有可以打开官网的应用");
        } catch (RuntimeException unavailable) {
            notice("无法打开官网");
        }
    }

    // A disabled card swallows the press, and the 工具 section draws no state text, so without
    // this 剪贴板历史 and AI 润色 looked exactly like the cards that work and did nothing when
    // pressed. A screen reader was told "不可用"; nobody else was.
    void applyToolCardState(View card, boolean enabled) {
        ViewPolicy.setActiveAlpha(card, enabled, .45f);
    }

    void render() {
        if (renderBatchDepth > 0) {
            renderBatchPending = true;
            return;
        }
        renderedEchoExpectations = selectionEcho.expectations();
        // 完整列表展开可能换代，先同步再绘制，避免候选文字与选词身份来自两代。
        if (candidatePanelOpen && imeNineKeyPanel.eligible()) imeNineKeyPanel.refreshSnapshot();
        scheduleCandidateGlosses();
        scheduleCandidatePronunciations();
        // 候选栏跟随光标要先知道这个输入框报不报告光标位置。
        synchronizeCursorUpdates();
        // 浮动开关、外接键盘的候选条模式或候选栏跟随光标变了：先把窗口换成对应的布局，下面判断分离式键盘、单手模式时用的是新状态。
        applyFloatingLayout(false);
        // 底栏随本地设置、浮动、外接键盘的候选条模式和旋转出现或消失。
        imeBottomBar.apply(imeFrame.keyboard);
        // 旋转、设置变化或布局切换让分离式键盘该画与否变了，而键行还是按旧状态建的：先按新状态重建，下面的底行排布也会跟着换。
        // 设置页改了九键左侧符号栏的符号：同样按新的符号表重建。
        // 中文标点开关在 123 / #+= 层上切换了（工具面板、设置页、Ctrl + .）：这一层的标点按新状态重画。
        // 设置页开关了「数字行」：按新状态重建。
        // 拼音九键和九键双拼之间换了、或九键双拼换了表：同一个触屏布局，按新键面重建。
        // 数字页状态和键行建成时不一致（例如编辑器换了初始键层）：触屏布局没变，也要按当前状态换键面。数字九宫格上切了中/英或换了方案，左列和右列 ！ 的标点写法变了，由 sidebarStale 发现。
        // 九键外框换了是否画右侧大回车（设置页换了九键样式、进出密码框），或大回车外框的地球键变了：外框连同底行整块重建。
        // 设置页换了数字键盘的空格位置：数字层右列最下面一格在空格和 ！ 之间换，按新状态重建。
        if (imeLetterRows.drawnTouchLayout() != displayedTouchLayout(view)
                || imeLetterRows.splitStale() || imeLayoutRows.sidebarStale()
                || imeLetterRows.layerPunctuationStale() || imeLetterRows.numberRowStale()
                || imeLayoutRows.shuangpinStale() || imeLetterRows.languageNumberSwapStale()
                || imeLayoutRows.numberPadSpaceStale()
                || imeLetterRows.numberPadStale() || imeLayoutRows.tallReturnStale())
            imeLetterRows.rebuildKeyRows();
        // 数字页没重建时，临时模式等的进出也会改变随机数字的资格，键面跟着换。
        imeLayoutRows.syncNumberDigitFaces();
        updateSymbolKeyFaces();
        updateLetterCornerHints();
        updateShuangpinKeyHints();
        updateQuickPunctuation();
        imeLayoutRows.updateStrokeWildcardKey();
        imeCalculator.syncVisibility();
        imeLayoutRows.updateNineKeySymbolKey();
        imeLetterRows.updateFourteenKeySeparator();
        String currentEditingText = view == null ? "" : InputViewValuePolicy.editingText(view);
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
            String modeKey = InputViewValuePolicy.textOr(view, "local_mode", "none");
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
        editorReads.open();
        try {
            clearableEnglishWord = clearableEnglishWord();
            clearableEnglishCursor = clearableEnglishWord.isEmpty() ? -1 : quickDeleteCursor(connection);
        } finally {
            editorReads.close();
        }
        clearableEnglishConnection = clearableEnglishWord.isEmpty() ? null : connection;
        boolean hasEnglishWord = !clearableEnglishWord.isEmpty();
        boolean hasEnglishSuggestions = englishSuggestionsActive() && !englishSuggestions.isEmpty();
        boolean handwriting = handwritingActive();
        boolean hasHandwritingResults = handwriting && !handwritingResults.isEmpty()
            && handwritingCandidateToken != null;
        boolean hasEmailSuffixes = imeEmailSuffixes.active();
        boolean hasNineKeyPunctuation = imeNineKeyQuickPunctuation.active();
        // 候选栏是否没有东西可显示：英文直输的当前词只在读音行给「清除」，候选行没有内容，此时照常换回工具栏（#10479）。
        boolean candidateIdle = view == null || (InputViewValuePolicy.editingText(view).isEmpty()
            && "none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"))
            && (visibleCandidates == null || visibleCandidates.length() == 0)
            && !hasEnglishSuggestions
            && !hasEmailSuffixes
            && !hasNineKeyPunctuation
            && !hasHandwritingResults);
        // 整行空闲：另要求没有可清除的英文当前词，读音行和读音标题按它决定，「清除」留在工具栏上方的读音行里。
        boolean idle = candidateIdle && !hasEnglishWord;
        if (preedit != null) {
            // 读音的字号由 ImeStyler.applySkin 在 render 末尾统一设置，这里不再另设一份。
            String editingText = view == null ? "" : InputViewValuePolicy.editingText(view);
            boolean offersLocalModes = idle && supportsLocalTools();
            String localModeKey = view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none");
                    String reading = InputViewValuePolicy.textOr(view, "display_reading",
                InputViewValuePolicy.textOr(view, "reading", ""));
            // 九键的 editing_text 只是按下的数字。读音行显示引擎给的首选读法拼音（94'26 显示 xi'an），首行是英文词时退回 preedit（带拆分分界和选过的拼音，如 ni'426）。14 键同样显示 `nine_key_reading`，引擎在首行是英文词时也给出读音，不让组码字母（bugao）出现在读音行上。
            String nineKeyPreedit = "";
            if (view != null && "none".equals(localModeKey)
                    && drawsKeyGrid(displayedTouchLayout(view))) {
                nineKeyPreedit = InputViewValuePolicy.textOr(view, "nine_key_reading", "");
                if (nineKeyPreedit.isEmpty()) nineKeyPreedit = InputViewValuePolicy.textOr(view, "preedit", "");
            }
            String spelling = !nineKeyPreedit.isEmpty() ? nineKeyPreedit
                : reading.isEmpty() ? editingText : reading;
            String localModeTitle = "none".equals(localModeKey) ? spelling : editingText;
            // 本地模式自己的名字说明正在哪个模式里，不是拼写，所以照常画；模式在触发键之后拼写的内容和其他拼写一样，写进输入框后就不画。
            boolean localModeName = false;
            for (LocalInputMode mode : LocalInputMode.values()) {
                if (mode.preferenceKey().equals(localModeKey)
                        && mode.trigger().equals(editingText)) {
                    localModeTitle = mode.title();
                    localModeName = true;
                    break;
                }
            }
            // 拼写写在输入框里时候选栏不再重复画它（CandidatePreeditStylePolicy）；14 键、应用兼容豁免和编辑器截短时，组字留在键盘上，照常画。
            boolean spellingInEditor = view != null && connection != null && !editorCompositionSuppressed
                && !composingText(view, editingText, true).isEmpty();
            localModeTitle = CandidatePreeditStylePolicy.composedText(CandidatePreeditStylePolicy.style(
                spellingPosition, InputViewValuePolicy.scheme(view, -1), spellingInEditor),
                localModeTitle, localModeName);
            boolean idleTitle = idle && editingText.isEmpty();
            // 新设计去掉了空闲时的品牌药丸：空闲时读音行整行隐藏，品牌标在工具栏最左。
            brandPillVisible = false;
            String phrasePrefix = InputViewValuePolicy.textOr(view, "phrase_prefix", "");
            // 组字光标（#5613）：点读音行把光标移到点中的字母前；光标被移离末尾时画进读音行（「不显示」也画，下一个键就作用在那里）。读音对不上按键时退回画原始按键。
            boolean caretEditable = !idleTitle && view != null && CompositionCaretPolicy.editable(
                InputViewValuePolicy.scheme(view, -1), localModeKey, dedicatedEnglish, editingText);
            int caret = caretEditable
                ? InputViewValuePolicy.integer(view, "caret_position", editingText.length())
                : editingText.length();
            String caretSpelling = spelling;
            // 隔着显示分隔时光标画在哪一侧，看这次组字是不是在组码网格里打的（九键、14 键、18 键的退格先删切分），不看按键是不是数字，也不看 key_grid：网格开着时硬件键盘的字母照样走 26 键，退格删的是分隔前的字母。
            boolean caretKeyGrid = InputViewValuePolicy.booleanValue(view, "grid_composing", false);
            int caretMark = caretEditable
                ? CompositionCaretPolicy.markIndex(spelling, editingText, caret, caretKeyGrid) : -1;
            if (caretEditable && caret < editingText.length() && caretMark < 0) {
                caretSpelling = editingText;
                caretMark = CompositionCaretPolicy.markIndex(editingText, editingText, caret, caretKeyGrid);
            }
            boolean spellingDrawn = caretMark >= 0 || spelling.equals(localModeTitle);
            imeCompositionEditor.render(caretSpelling, editingText, caret, caretKeyGrid);
            String displayText = idleTitle
                ? (dedicatedEnglish ? "英文输入" : productName)
                : PhrasePreeditPolicy.title(phrasePrefix, caretMark >= 0
                    ? CompositionCaretPolicy.withMark(caretSpelling, caretMark) : localModeTitle,
                    !"none".equals(localModeKey));
            // 光标符画成强调色的粗竖条（#6110），文本里仍是那个字符，点读音行的下标换算不变。
            int caretInTitle = idleTitle ? -1
                : CompositionCaretPolicy.markInTitle(phrasePrefix, caretMark, !"none".equals(localModeKey));
            // 拼写已写在输入框里时读音行只画一支笔（SpellingEditIcon），点它打开放大编辑条；不再放「编辑拼写」四个字，描述仍是「编辑拼写」。
            boolean spellingEditIcon = caretEditable && !spellingDrawn && phrasePrefix.isEmpty();
            if (spellingEditIcon) displayText = "";
            preedit.setText(CompositionCaretSpan.mark(displayText, caretInTitle, pixels(2), pixels(2),
                imeStyler.caretColor()));
            showSpellingEditIcon(spellingEditIcon);
            preedit.setContentDescription(caretEditable ? "编辑拼写"
                : offersLocalModes ? "长按打开本地输入模式" : displayText);
            preedit.setLongClickable(offersLocalModes);
            ViewPolicy.setClickable(preedit, caretEditable);
            ViewPolicy.setFocusable(preedit, offersLocalModes);
        }
        if (exitLocalModeButton != null) {
            boolean localModeActive = view != null
                && !"none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none"));
            ViewPolicy.setVisible(exitLocalModeButton, localModeActive || hasEnglishWord);
            ViewPolicy.setEnabled(exitLocalModeButton, hasEnglishWord || (localModeActive && session != 0));
            exitLocalModeButton.setText(hasEnglishWord ? "清除" : "×");
            exitLocalModeButton.setContentDescription(hasEnglishWord ? "清除当前英文单词" : "退出本地模式");
            // render 末尾的 applySkin() 遍历统一设置这个按钮的样式。
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
        // 「键盘调整」的遮罩盖在照常画出的键盘上：工具栏、候选行和按键区都不为它让位，下面画的就是调整后的样子。
        if (keyboardAdjustOverlay != null) {
            ViewPolicy.setVisible(keyboardAdjustOverlay, keyboardAdjustActive);
            if (keyboardAdjustActive) {
                keyboardAdjustOverlay.setAvailability(keyboardAdjustAvailability());
                // 外框里排在遮罩后面加进来的面板（建视图时的各个面板、第一次打开才建的表情和符号面板）会盖在它上面，打开期间让它保持在最上层。
                if (keyboardSurface.getChildAt(keyboardSurface.getChildCount() - 1) != keyboardAdjustOverlay)
                    keyboardAdjustOverlay.bringToFront();
            }
            applyKeyboardAdjustAccessibility();
        }
        if (candidateHeader != null) {
            // 空闲时这一行只给常驻的模式标签（直接输入、准备中）：简繁切换、设置保存、同步重试、Shift 这类一闪而过的提示若也占这一行，每次出现和消失都把整副键盘顶上去又落回来。临时提示只在失败时以 Toast 说出来。
            boolean modeLabel = !message.isEmpty();
            ViewPolicy.setVisible(candidateHeader, !hasDiagnostic && (!idle || modeLabel));
            if (idle && !modeLabel) announceIdleNotice(preferencesNotice);
        }
        boolean keysCollapsed = hardwareKeysCollapsed();
        imeFrame.setKeysCollapsed(keysCollapsed);
        applyCaretCandidates(!candidateIdle || hasDiagnostic);
        // 候选条模式里空闲时工具栏总要显示：展开键在上面，「显示方式：隐藏」时整副键盘会只剩导航栏那一截。
        // 隐藏工具栏时，底行仍能打开表情面板；面板的返回键在工具栏上，打开期间必须临时显示（toolbarExitNeeded）。工具按钮全关时同样不显示这一行（toolbarRowHidden）。
        boolean toolbarRow = candidateIdle && !hasDiagnostic
            && (!toolbarRowHidden() || keysCollapsed || toolbarExitNeeded());
        // 开始打字就收起「最近复制」：用户已经在输入别的内容了。这里看 idle 而不是 candidateIdle：无建议框、网址、邮箱或关掉英文建议时打英文没有候选，只有当前词能说明用户开始打字了。
        if (!idle) recentClip.dismiss();
        long nowMs = System.currentTimeMillis();
        // 候选条模式里不换上「最近复制」：它会盖住工具栏上展开软键盘的那个键。
        // 文本编辑面板保留顶部的复制预览，点「复制」后不必先返回键盘（#8104）。
        boolean recentPanelAllowed = ViewPolicy.isVisible(textEditPanel) || !anyToolbarPanelOpen();
        boolean autofillShown = inlineAutofill != null && inlineAutofill.update(
            idle && !hasDiagnostic && !keysCollapsed && !anyToolbarPanelOpen() && connection != null);
        boolean recentAllowed = !autofillShown && toolbarRow && !keysCollapsed && recentPanelAllowed && recentClipEnabled()
            && connection != null && imePrivacyGate.allows(ImePrivacyGate.Record.CLIPBOARD_HISTORY);
        String recent = recentAllowed ? recentClip.text(nowMs) : null;
        // 图片只在当前输入框收得下时才占这一行（#6781）。
        boolean recentImage = recentAllowed && recentImageSendable(nowMs);
        imeToolbar.updateRecentClipRow(recent, recentImage);
        main.removeCallbacks(recentClipExpiry);
        boolean recentShown = recent != null || recentImage;
        if (recentShown) main.postDelayed(recentClipExpiry, recentClip.remainingMs(nowMs) + 50);
        if (shortcutToolbar != null)
            ViewPolicy.setVisible(shortcutToolbar, toolbarRow && !recentShown && !autofillShown);
        if (candidateLine != null)
            ViewPolicy.setVisible(candidateLine, !candidateIdle && !hasDiagnostic);
        if (!ViewPolicy.isVisible(replyKeyboard))
            imeBottomRow.updateActionRow();
        if (candidateViewport != null)
            ViewPolicy.setVisible(candidateViewport, !candidateIdle && !hasDiagnostic);
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
            // 会话还在建时也可点（#10086）：表情面板只用资源目录和输入连接。
            ViewPolicy.setEnabled(emojiShortcutButton, ToolbarReadinessPolicy.emojiEnabled(toolbarSession(),
                connection != null, !emojiResources.isEmpty()));
            ViewPolicy.setSelected(emojiShortcutButton, ViewPolicy.isVisible(emojiPanel));
        }
        if (phraseShortcutButton != null) {
            ViewPolicy.setVisible(phraseShortcutButton, toolbarPhrase);
            ViewPolicy.setEnabled(phraseShortcutButton, ToolbarReadinessPolicy.phraseEnabled(toolbarSession(),
                !preferencesDirectory.isEmpty()));
            ViewPolicy.setSelected(phraseShortcutButton, ViewPolicy.isVisible(phraseScroll));
        }
        if (clipboardShortcutButton != null) {
            ViewPolicy.setVisible(clipboardShortcutButton, toolbarClipboard);
            ViewPolicy.setEnabled(clipboardShortcutButton, CloudClipboardPanelPolicy.panelAvailable(
                clipboardHistoryEnabled, imePanels.cloudClipboardAllowed()));
            ViewPolicy.setSelected(clipboardShortcutButton, imePanels.clipboardPanelOpen());
        }
        if (dismissShortcutButton != null) {
            dismissShortcutButton.setContentDescription(keysCollapsed ? "显示软键盘"
                : anyToolbarPanelOpen() ? "返回键盘" : "收起键盘");
            // 候选条模式里收起键朝上，点它展开软键盘。
            if (dismissShortcutButton instanceof KeyboardShortcutButton dismiss) dismiss.setFlipped(keysCollapsed);
        }
        if (randomDigitsShortcutButton != null) {
            // 工具栏整条隐藏时「随」够不到，与关闭入口一样清掉运行时的随机状态。
            if (!localSettings.bool(AndroidLocalSettings.TOOLBAR_RANDOM_DIGITS) || toolbarHidden)
                imeLayoutRows.disableRandomDigits();
            ViewPolicy.setVisible(randomDigitsShortcutButton, imeLayoutRows.randomDigitsEntryVisible());
            ViewPolicy.setSelected(randomDigitsShortcutButton, imeLayoutRows.randomDigitsEnabled());
            if (Build.VERSION.SDK_INT >= 30) randomDigitsShortcutButton.setStateDescription(
                imeLayoutRows.randomDigitsEnabled() ? "已开启" : "已关闭");
        }
        if (voiceShortcutButton != null) {
            ViewPolicy.setVisible(voiceShortcutButton, touchVoiceShortcutEnabled);
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
            String currentLocalMode = view == null ? "none" : InputViewValuePolicy.textOr(view, "local_mode", "none");
            boolean visible = MicrosoftShuangpinKeyPolicy.visible(
                dedicatedEnglish, selectedScheme, currentLocalMode);
            ViewPolicy.setVisible(microsoftFinalKey, visible);
            ViewPolicy.setEnabled(microsoftFinalKey, visible && session != 0);
            microsoftFinalKey.setContentDescription("微软双拼 ing");
        }
        if (layerButton != null) {
            boolean symbols = keyboardLayer == KeyboardLayout.Layer.SYMBOLS;
            int layerLayout = displayedTouchLayout(view);
            // 数字九宫格上的返回键印进来之前那块键面的名字；中文键面借来时印「返回」。注音的共享符号行（#+= 页）上它回到数字九宫格，印 123、描述与设计稿 #+= 层的 123 相同。
            boolean sharedSymbolRows = imeLetterRows.sharedSymbolRowsDrawn();
            layerButton.setText(sharedSymbolRows ? KeyboardActionRow.layerTitle(layerLayout, false)
                : KeyboardActionRow.layerTitle(layerLayout, symbols,
                    symbols && imeLetterRows.numberPadDrawn() && KeyboardLayout.chineseFace(
                        view == null ? -1 : InputViewValuePolicy.scheme(view, -1), dedicatedEnglish)));
            layerButton.setContentDescription(KeyboardActionRow.layerDescription(symbols && !sharedSymbolRows));
        }
        if (symbolPanelButton != null) {
            // 这个键出现在 26 键一族、拼音九键和笔画键盘的底行；拼音九键的数字键面也画这一行，在那里同样能打开面板。
            ViewPolicy.setEnabled(symbolPanelButton, session != 0 && connection != null);
            symbolPanelButton.setContentDescription("打开符号面板");
        }
        if (symbolShortcutButton != null) {
            ViewPolicy.setVisible(symbolShortcutButton, longSpaceLayoutActive());
            ViewPolicy.setEnabled(symbolShortcutButton, session != 0 && connection != null);
            ViewPolicy.setSelected(symbolShortcutButton, ViewPolicy.isVisible(symbolPanel));
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
            boolean separator = usesQuanpinSeparator();
            imeLetterRows.updateShiftKeyIdentity(separator);
            shiftButton.setText(separator ? "分词" : letterCase.keyText());
            ViewPolicy.setSelected(shiftButton, !separator && letterCase.usesUppercase());
            shiftButton.setActivated(!separator && letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
            // The final applySkin() traversal styles this attached button once.
            String caseLabel = separator ? "拼音分词" : shiftLayout == KeyboardLayout.KOREAN_LAYOUT
                ? KoreanKeyboardLayout.SHIFT_LABEL
                : letterCase.accessibilityLabel(dedicatedEnglish || session == 0
                    || letterCaseSchemeActive());
            String caseValue = separator ? "分隔拼音音节" : letterCase.accessibilityValue();
            shiftButton.setContentDescription(Build.VERSION.SDK_INT >= 30
                ? caseLabel : caseLabel + "，" + caseValue);
            if (Build.VERSION.SDK_INT >= 30) shiftButton.setStateDescription(caseValue);
        }
        if (languageButton != null) {
            boolean cycles = languageKeyCycles();
            languageButton.setText(LanguageKeyCyclePolicy.label(cycles, selectedScheme, dedicatedEnglish, visibleSchemes));
            // 没有会话也没有会话在建（密码框、数字框）时点按切不了中英，但长按仍要能打开输入法选择框：换到密码管理器的键盘正是在密码框里最常用。禁用的按钮收不到长按，所以这个键始终可用，只把它画淡、读屏念成「暂不可用」，点按在点击监听里直接忽略。会话还在建时点按照常生效，不画淡（#10086）。
            boolean canToggle = ToolbarReadinessPolicy.languageKeyActive(toolbarSession());
            ViewPolicy.setEnabled(languageButton, true);
            ViewPolicy.setActiveAlpha(languageButton, canToggle, .45f);
            languageButton.setContentDescription(!canToggle ? "中英切换暂不可用，长按切换输入法"
                : LanguageKeyCyclePolicy.description(nextLanguageTarget()));
            if (Build.VERSION.SDK_INT >= 30) {
                // 会话在建时记下的那一下键面不变，读屏念出它记下了（#10086）。
                languageButton.setStateDescription(!canToggle ? "输入会话未就绪"
                    : LanguageKeyCyclePolicy.stateDescription(cycles, selectedScheme, dedicatedEnglish,
                        visibleSchemes) + (pendingLanguageTap ? "，准备好后切换" : ""));
            }
        }
        if (floatingShortcutButton != null) {
            ViewPolicy.setVisible(floatingShortcutButton, toolbarFloating);
            ViewPolicy.setSelected(floatingShortcutButton, floatingKeyboard);
            // 候选条模式里浮动不生效，按钮灰掉，免得点了没有反应。
            ViewPolicy.setEnabled(floatingShortcutButton, !hardwareKeyboardMode);
            if (Build.VERSION.SDK_INT >= 30)
                floatingShortcutButton.setStateDescription(floatingKeyboard ? "已开启" : "已关闭");
        }
        if (undoShortcutButton != null) {
            ViewPolicy.setVisible(undoShortcutButton, toolbarUndo);
            ViewPolicy.setEnabled(undoShortcutButton, connection != null && !EditorPolicy.password(editorInputType));
        }
        if (redoShortcutButton != null) {
            ViewPolicy.setVisible(redoShortcutButton, toolbarRedo);
            ViewPolicy.setEnabled(redoShortcutButton, connection != null && !EditorPolicy.password(editorInputType));
        }
        if (textEditShortcutButton != null) {
            ViewPolicy.setVisible(textEditShortcutButton, toolbarTextEdit);
            ViewPolicy.setSelected(textEditShortcutButton, ViewPolicy.isVisible(textEditPanel));
            // 没有可编辑的输入框时面板打不开（ImeTextEditPanel.show 只给一句提示），按钮灰掉。
            ViewPolicy.setEnabled(textEditShortcutButton, connection != null);
        }
        if (schemeButton != null) {
            ViewPolicy.setVisible(schemeButton, toolbarScheme);
            // 会话在建时点下、等着打开的也画成选中，点按有了着落（#10086）。
            ViewPolicy.setSelected(schemeButton, ViewPolicy.isVisible(schemeScroll)
                || pendingToolbarPanel == ToolbarOrderPolicy.Item.SCHEME);
            schemeButton.setText(selectedScheme.glyph() + selectedScheme.badge(wubiProfile));
            schemeButton.setContentDescription("输入方案：" + selectedScheme.title(wubiProfile));
            // 只按「会话与偏好是否就绪」决定可用：简繁、键高、方案的保存都在一瞬间完成，若跟着保存状态禁用，图标每切一次简繁就变灰再变回来。保存进行中的点按由 showSchemePicker 忽略。会话还在建时也可点，点按推迟到就绪后打开（ToolbarReadinessPolicy.preferencePanelTap）。
            ViewPolicy.setEnabled(schemeButton, ToolbarReadinessPolicy.preferencePanelEnabled(toolbarSession(),
                preferencesSnapshot != null));
            if (Build.VERSION.SDK_INT >= 30) {
                String schemeState = toolbarSession() == ToolbarReadinessPolicy.Session.STARTING
                    ? pendingToolbarPanel == ToolbarOrderPolicy.Item.SCHEME ? "准备好后打开" : "正在准备"
                    : session == 0 ? "输入会话未就绪"
                    : preferencesSnapshot == null ? "设置加载中"
                    : schemeSaving ? "正在切换输入方案"
                    : touchGeometrySaving || traditionalOutputSaving ? "正在保存其他设置"
                    : "可用";
                schemeButton.setStateDescription(schemeState);
            }
        }
        if (skinButton != null) {
            ViewPolicy.setVisible(skinButton, toolbarSkin);
            ViewPolicy.setSelected(skinButton, ViewPolicy.isVisible(skinScroll)
                || pendingToolbarPanel == ToolbarOrderPolicy.Item.SKIN);
            // 同上：保存进行中的点按由 ImePanels.showSkinPicker 按 canSaveKeyboardSkin 忽略，图标不跟着变灰；会话还在建时同输入方式一样推迟。
            ViewPolicy.setEnabled(skinButton, ToolbarReadinessPolicy.preferencePanelEnabled(toolbarSession(),
                preferencesSnapshot != null && !preferencesDirectory.isEmpty()));
            skinButton.setContentDescription("切换键盘皮肤；当前" + skin.title());
            // 会话在建时和输入方式按钮一样念出这一下记下了没有（#10086），读屏用户才知道面板为什么还没出来。
            if (Build.VERSION.SDK_INT >= 30) skinButton.setStateDescription(
                toolbarSession() != ToolbarReadinessPolicy.Session.STARTING ? skin.title()
                    : skin.title() + (pendingToolbarPanel == ToolbarOrderPolicy.Item.SKIN ? "，准备好后打开" : "，正在准备"));
        }
        synchronizeReplyKeyboard();
        if (layoutSettingsButton != null) ViewPolicy.hide(layoutSettingsButton);
        if (layoutSettingsButton != null)
            ViewPolicy.setEnabled(layoutSettingsButton, session != 0 && preferencesSnapshot != null
                && !schemeSaving && !touchGeometrySaving && !traditionalOutputSaving);
        imeLayoutRows.renderNineKeySpellings(hasDiagnostic);
        scheduleCandidateTranslations();
        scheduleUserCandidateTranslations();
        scheduleOnlineProviders();
        if (candidates == null) {
            imeStyler.applySkin();
            imeToolbar.styleTopRow();
            return;
        }
        // 注音 9 键的读音选择条叠在候选区上：选择条只在候选列表关着时出现，那时候选行本来就是空的，所以把候选滚动区让成不可见（仍占位，键盘高度不变）。18 键组字时拼音和候选同时出现，它的拼音条在读音行（ImeLayoutRows#attachEighteenSpellingsToReadingRow），不叠在这里，候选照常显示、可点。
        boolean zhuyinSpellingsShown = ViewPolicy.isVisible(nineKeySpellingScroll)
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
        // 手指按在某个候选上时不重画候选条，见 candidateRenderDeferred。这一帧剩下的东西（页码、展开键、
        // 皮肤）也一起跳过；松手后补的那次 render 会把它们一起补上。
        if (candidateTreeHeld(activeCandidates)
                || candidateTreeHeld(expandedCandidates)
                || candidateTreeHeld(imeNineKeyPanel.root())) {
            candidateRenderDeferred = true;
            return;
        }
        candidateRenderDeferred = false;
        boolean continuousStrip = candidateHorizontal && !hardwareKeyboardMode && !handwriting
            && !imeTranslationSenses.active() && !hasNineKeyPunctuation && !imeEmailSuffixes.active()
            && !englishSuggestionsActive() && !hasDiagnostic && !directEnglishActive();
        JSONObject stripSnapshot = imeCandidateStrip.prepare(continuousStrip);
        candidateDecorationWords.clear();
        candidates.removeAllViews();
        if (verticalCandidates != null) verticalCandidates.removeAllViews();
        if (expandCandidates != null) ViewPolicy.hide(expandCandidates);
        if (closePrediction != null) ViewPolicy.hide(closePrediction);
        if (view == null) {
            closeCandidatePanel();
            renderEnglishSuggestions(activeCandidates);
            for (Button button : candidateButtons) ViewPolicy.hide(button);
            imeStyler.applySkin();
            imeToolbar.styleTopRow();
            return;
        }
        JSONArray entries = view.optJSONArray("candidates");
        // 上屏后联想时候选栏右端的按键按用户选的样式（#10078）；只有引擎候选占着这一行时才放，手写、义项、标点、邮箱后缀和英文建议照旧。
        boolean prediction = view.optBoolean("prediction", false);
        boolean expandAllowed = PredictionButtonsPolicy.allowsExpand(predictionButtons, prediction);
        boolean closeShown = closePrediction != null && !hasDiagnostic
            && PredictionButtonsPolicy.showsClose(predictionButtons, prediction);
        if (handwriting) {
            renderSharedHandwritingCandidates(activeCandidates);
        } else if (imeTranslationSenses.active()) {
            // 义项页占用候选行：不画引擎候选，也不显示展开键和页码，义项都在这一页上。
            imeTranslationSenses.render(activeCandidates, candidateHorizontal);
            if (candidatePage != null) candidatePage.setText("");
        } else if (hasNineKeyPunctuation) {
            // 用户刚点了九键的 1 键，先给它要的常用标点，邮箱后缀等下一次转换之后再出现。
            imeNineKeyQuickPunctuation.render(activeCandidates, candidateHorizontal);
        } else if (imeEmailSuffixes.active()) {
            // 邮箱后缀和英文建议占同一个候选行；打到 `@` 之后英文建议本来也没有，先给后缀。
            imeEmailSuffixes.render(activeCandidates);
        } else if (englishSuggestionsActive()) {
            renderEnglishSuggestions(activeCandidates);
        } else if (stripSnapshot != null) {
            imeCandidateStrip.render(stripSnapshot, candidates, horizontalCandidateScroll);
            if (expandCandidates != null && expandAllowed
                    && (strictCandidatePage(view, "page_count") > 1 || imeNineKeyPanel.eligible()))
                ViewPolicy.show(expandCandidates);
            if (closeShown) ViewPolicy.show(closePrediction);
        } else if (entries != null) {
            int visibleSlot = 0;
            for (int slot = 0; slot < entries.length(); slot++) {
                JSONObject candidate = entries.optJSONObject(slot);
                if (candidate == null) continue;
                while (candidateButtons.size() <= slot)
                    candidateButtons.add(imeCandidates.makeCandidateButton(candidateButtons.size()));
                Button candidateView = candidateButtons.get(slot);
                int source = InputViewValuePolicy.integer(candidate, "source", -1);
                if (!AiPendingCandidatePolicy.isVisible(source)) {
                    ViewPolicy.hide(candidateView);
                    continue;
                }
                ViewPolicy.show(candidateView);
                updateCandidateButton(candidateView, candidate, slot, visibleSlot++);
                activeCandidates.addView(candidateView, new LinearLayout.LayoutParams(
                    candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                        : LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT));
            }
            if (!hasDiagnostic && expandCandidates != null && expandAllowed
                    && (strictCandidatePage(view, "page_count") > 1 || imeNineKeyPanel.eligible()))
                ViewPolicy.show(expandCandidates);
            if (closeShown) ViewPolicy.show(closePrediction);
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
        if (ViewPolicy.isVisible(moreToolsScroll))
            imeFunctionPanel.renderMoreTools();
        imeStyler.applySkin();
        imeToolbar.styleTopRow();
    }

    /** 在候选条或展开面板里是否有尚未结束的触摸或排队点击。 */
    private boolean candidateTreeHeld(View view) {
        if (view == null || view.getVisibility() != View.VISIBLE) return false;
        if (candidateTouches.containsKey(view)) return true;
        if (!(view instanceof android.view.ViewGroup group)) return false;
        for (int index = 0; index < group.getChildCount(); index++)
            if (candidateTreeHeld(group.getChildAt(index))) return true;
        return false;
    }

    /** 候选视图结束按压后补上被延后的那一帧；监听返回 false，不改变按钮自身的点击、长按和滚动语义。 */
    void bindCandidateRenderRelease(KeyboardPressButton button) {
        button.setOnTouchListener((view, event) -> {
            int action = event.getActionMasked();
            if (action == android.view.MotionEvent.ACTION_DOWN) {
                candidateTouches.put(view, new Object());
            } else if (action == android.view.MotionEvent.ACTION_UP
                    || action == android.view.MotionEvent.ACTION_CANCEL) {
                Object gesture = candidateTouches.get(view);
                // 触摸监听先于框架的点击入队；再排一次队，既不能提前摘掉按钮，也不能漏掉点击触发的延后绘制。
                main.post(() -> main.post(() -> {
                    if (candidateTouches.get(view) == gesture) candidateTouches.remove(view);
                    if (candidateRenderDeferred) render();
                }));
            }
            return false;
        });
        button.setPressReleaseListener(() -> main.post(() -> {
            // 点击可能在松开通知之后才置标记，所以到执行时再检查；其他手指仍按着时由最后一次松开补帧。
            if (candidateRenderDeferred) render();
        }));
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
            KeyboardPressButton choice = keyboardKey(chineseOutput(candidate, view),
                "手写候选 " + (index + 1), () -> commitHandwritingCandidate(token, candidate));
            bindCandidateRenderRelease(choice);
            KeyboardTypographyPolicy.setKeyTextSize(choice, candidateFontSize);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                candidateHorizontal ? LinearLayout.LayoutParams.WRAP_CONTENT
                    : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
            activeCandidates.addView(choice, params);
        }
    }
}
