package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Android 本地设置：只有 Android 宿主用、不进共享偏好（crates/client-core 的 Preferences）的设置，存在 `filesDir/bootstrap/state/android-settings.json`。
 *
 * <p>设置应用和 :ime 进程是同一个 UID，读写同一个文件。写入在同目录的锁文件上加进程间文件锁，读出、修改、写到临时文件再原子改名；读取按文件的 inode、修改时间和大小缓存，文件被另一进程换掉后下一次 {@link #load} 就读到新值。文件缺失、过大、损坏或某一项取值不合规时，那一项（或整份）回到默认值；读取从不改写文件。
 *
 * <p>键名与账号设置文档的同步键相同（`general.app_theme`、`platform.android.*`）。{@link Spec#synced} 为真的十八项随云同步交给 client-services 的 `android_local`（crates/client-services/src/account/settings_sync.rs 的 `ANDROID_LOCAL_SETTINGS`，两边的键与取值范围由 AndroidLocalSettingsSmoke 锁住）；隐私模式、防止截屏、语音数据贡献、开发者选项、键盘高度、浮动键盘的开关与位置、物理键盘时候选栏跟随光标、工具栏尺寸及工具栏上的浮动键盘和文本编辑按钮、键盘底栏和底部留白只留在本机。
 */
public final class AndroidLocalSettings {
    public static final String FILE_NAME = "android-settings.json";
    static final int MAX_BYTES = 16 * 1024;
    static final int VERSION = 1;

    // ---- 随账号同步的键 ----
    public static final String APP_THEME = "general.app_theme";
    public static final String ONE_HANDED = "platform.android.one_handed";
    /** 平板横屏时把 26 键一族和韩文键盘分成左右两半（{@link SplitKeyboardPolicy}），默认关。 */
    public static final String SPLIT_KEYBOARD = "platform.android.split_keyboard";
    /** 分离式键盘开始启用的最小屏幕宽度（dp），默认 600。 */
    public static final String SPLIT_KEYBOARD_THRESHOLD = "platform.android.split_keyboard_threshold";
    /** 分离式键盘左右两半之间的空隙比例（百分比），默认 25。 */
    public static final String SPLIT_KEYBOARD_GAP = "platform.android.split_keyboard_gap";
    public static final String KEY_POPUP = "platform.android.key_popup";
    public static final String SWIPE_DOWN_SYMBOLS = "platform.android.swipe_down_symbols";
    /** 「滑动输入符号」的方向，取值见 {@link SwipeHintPolicy}。开关仍是 {@link #SWIPE_DOWN_SYMBOLS}，键名保留旧名以免已同步的值失效。 */
    public static final String SWIPE_SYMBOLS_DIRECTION = "platform.android.swipe_symbols_direction";
    public static final String SPACE_CURSOR = "platform.android.space_cursor";
    public static final String SPACE_VOICE = "platform.android.space_voice";
    public static final String KEY_ANIMATION = "platform.android.key_animation";
    public static final String TOOLBAR_PHRASE = "platform.android.toolbar_phrase";
    public static final String TOOLBAR_SCHEME = "platform.android.toolbar_scheme";
    public static final String TOOLBAR_HIDDEN = "platform.android.toolbar_hidden";
    /** 旧版的「长空格布局」，默认关闭，仅在本机保存。现在由共享偏好 `touch_twenty_six_key_style` 的 `long_space` 接替，这个键只由 {@link KeyLayoutStyleMigration} 读一次、迁完即删；留着声明是为了读出升级前写下的值（不认识的键在读取时会被丢掉）。 */
    public static final String LONG_SPACE_LAYOUT = "platform.android.long_space_layout";
    public static final String HANDWRITING_MODE = "platform.android.handwriting_mode";
    public static final String HANDWRITING_DELAY_MS = "platform.android.handwriting_delay_ms";
    public static final String HANDWRITING_SHOW_PINYIN = "platform.android.handwriting_show_pinyin";
    public static final String HANDWRITING_STROKE_COLOR = "platform.android.handwriting_stroke_color";
    public static final String HANDWRITING_STROKE_WIDTH = "platform.android.handwriting_stroke_width";
    public static final String VOICE_OFFLINE_FALLBACK = "platform.android.voice_offline_fallback";

    // ---- 只在本机的键 ----
    public static final String INCOGNITO = "platform.android.incognito";
    public static final String VOICE_CONTRIBUTE_AUDIO = "platform.android.voice_contribute_audio";
    /** 本地键盘语音的停顿计时与 VAD 灵敏度，只留在本机，默认保持原行为。 */
    public static final String VOICE_PAUSE_MILLIS = "platform.android.voice_pause_millis";
    public static final String VOICE_VAD_SENSITIVITY = "platform.android.voice_vad_sensitivity";
    public static final int VOICE_PAUSE_DEFAULT_MILLIS = 1500;
    public static final int VOICE_PAUSE_MIN_MILLIS = 500;
    public static final int VOICE_PAUSE_MAX_MILLIS = 5000;
    /** 「防止截屏」（#10098，{@link ScreenCaptureProtection}），默认关。只在本机、不进备份：换一台设备或从备份恢复都不能替用户打开或关掉它，切换规则见 {@link ScreenCaptureProtectionPolicy}。 */
    public static final String SCREEN_CAPTURE_PROTECTION = "platform.android.screen_capture_protection";
    /** 「防止截屏」是否打开过：第一次打开时写入，之后关闭也不清除，此后再开或再关都要系统认证。清除应用数据（或经过认证的「重置所有设置」）才回到未打开过。 */
    public static final String SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE = "platform.android.screen_capture_protection_enabled_before";
    /** 「滑行输入」（{@link GlideTypingPolicy}），默认关。服务端的同步字段表还没有这个键，所以先只在本机。 */
    public static final String GLIDE_TYPING = "platform.android.glide_typing";
    /** 中英键按「中 → 英 → 已启用的其他语言键盘 → 中」轮换（{@link LanguageKeyCyclePolicy}，#6648），默认关，关着时只切中英。服务端的同步字段表还没有这个键，所以先只在本机。 */
    public static final String LANGUAGE_KEY_CYCLE = "platform.android.language_key_cycle";
    /** 交换语言键与数字层入口，英语和外语键盘一并使用，默认关闭，只在本机保存。 */
    public static final String SWAP_LANGUAGE_NUMBER_KEYS = "platform.android.swap_language_number_keys";
    /** 英文是否按编辑器标记自动大写，默认关闭，只在本机保存；手动 Shift 不受影响。 */
    public static final String AUTO_CAPITALIZATION = "platform.android.auto_capitalization";
    /** 剪贴板面板一行排几条（{@link ClipboardLayoutPolicy}）：`one` 单列、`two` 双列，默认单列。同步字段表里没有这个键，只在本机。 */
    public static final String CLIPBOARD_COLUMNS = "platform.android.clipboard_columns";
    /** 工具栏显示最近复制的文字或图片（{@link RecentClipboardSuggestion}，图片见 #6781），默认开；剪贴板历史关着时不生效。同步字段表里没有这个键，只在本机。 */
    public static final String CLIPBOARD_SUGGESTION = "platform.android.clipboard_suggestion";
    /** 剪贴板删除记录（单条或多选）后可撤销的秒数（{@link ClipboardUndoPolicy}，#10073、#10080），5 到 9 秒，默认 5 秒。同步字段表里没有这个键，只在本机。 */
    public static final String CLIPBOARD_UNDO_SECONDS = "platform.android.clipboard_undo_seconds";
    /** 单条或多选删除剪贴板记录前是否要求系统认证（#10408）；访客模式下始终要求认证。只在本机，默认关。 */
    public static final String CLIPBOARD_DELETE_AUTHENTICATION = "platform.android.clipboard_delete_authentication";
    /** 「复制后自动显示快捷键」（#10079）：最近复制那一行在预览右边多一个分词或分类快捷键，默认开，好让用户知道有这个功能；最近复制不显示时它也不显示。只在本机。 */
    public static final String CLIPBOARD_SUGGESTION_ACTION = "platform.android.clipboard_suggestion_action";
    /** 复制后快捷键的功能（{@link RecentClipboardSuggestion#quickAction}）：`segment` 分词、`category` 移动分类，默认分类。只在本机。 */
    public static final String CLIPBOARD_SUGGESTION_ACTION_KIND = "platform.android.clipboard_suggestion_action_kind";
    /** 旧版的「数字行」（#6022），默认关，只在本机。现在由共享偏好 `touch_number_row` 接替，这个键只由 {@link KeyLayoutStyleMigration} 读一次、迁完即删；同步的 `platform.android.number_row` 与它同名，但走共享偏好，不经这个文件。 */
    public static final String NUMBER_ROW = "platform.android.number_row";
    /** 旧版的「九键滑动输入数字」（{@link NineKeySwipePolicy}）：关闭、上滑输入数字或下滑输入数字，默认关闭，只在本机。现在由共享偏好 `touch_nine_key_swipe_digits` 接替（#10229），这个键只由 {@link KeyLayoutStyleMigration} 读一次、迁完即删；规格声明留着，否则读文件时这个键会被丢掉、迁移读不到。同步的 `platform.android.nine_key_swipe` 与它同名，但走共享偏好，不经这个文件。 */
    public static final String NINE_KEY_SWIPE = "platform.android.nine_key_swipe";
    /** 拼音九键滑动的判定距离（dp，{@link NineKeySwipePolicy#MIN_THRESHOLD_DP}..{@link NineKeySwipePolicy#MAX_THRESHOLD_DP}，默认 {@link NineKeySwipePolicy#DEFAULT_THRESHOLD_DP}），只影响九键滑动（共享偏好 `touch_nine_key_swipe_digits`），26 键的滑动阈值不变。只在本机、不随账号同步，iOS 和鸿蒙没有这一项（固定 24）。 */
    public static final String NINE_KEY_SWIPE_DISTANCE = "platform.android.nine_key_swipe_distance";
    /** 旧版的九键左侧符号（字母键面那张表，用空格分开），只在本机。现在由共享偏好 `touch_nine_key_sidebar_symbols`（{@link NineKeySidebarPolicy#LETTER_PREFERENCE}）接替，这个键只由 {@link KeyLayoutStyleMigration} 读一次、迁完即删；留着声明是为了读出升级前写下的值（不认识的键在读取时会被丢掉）。同步的 `platform.android.nine_key_symbols` 与它同名，但走共享偏好，不经这个文件。 */
    public static final String NINE_KEY_SYMBOLS = "platform.android.nine_key_symbols";
    /** 旧版的九键数字键盘左侧符号，格式同 {@link #NINE_KEY_SYMBOLS}，只在本机。现在由共享偏好 `touch_nine_key_digit_sidebar_symbols`（{@link NineKeySidebarPolicy#DIGIT_PREFERENCE}）接替，同样只由 {@link KeyLayoutStyleMigration} 读一次、迁完即删。 */
    public static final String NINE_KEY_DIGIT_SYMBOLS = "platform.android.nine_key_digit_symbols";
    /** 退格键上滑快速删除的范围：默认只删光标前，可选清空整个输入框。 */
    public static final String QUICK_DELETE_SCOPE = "platform.android.quick_delete_scope";
    /** 设计范围的键盘高度调整（dp，-46..129，即 75%..170%）。缺省时宿主沿用共享偏好里的 `touch_keyboard_height_adjustment`（-12..48）。 */
    public static final String KEYBOARD_HEIGHT_ADJUSTMENT = "platform.android.keyboard_height_adjustment";
    /** 浮动键盘（{@link FloatingKeyboardPolicy}），默认关。位置与屏幕尺寸相关，开关与位置都只在本机，不随账号同步。 */
    public static final String FLOATING_KEYBOARD = "platform.android.floating_keyboard";
    /** 浮动键盘在可移动范围里的水平 / 竖直位置，千分比（0 最左 / 最上，1000 最右 / 最下）。 */
    public static final String FLOATING_KEYBOARD_X = "platform.android.floating_keyboard_x";
    public static final String FLOATING_KEYBOARD_Y = "platform.android.floating_keyboard_y";
    /** 「物理键盘时候选栏跟随光标」（{@link CaretCandidatePolicy}，#6792），默认关：外接键盘的候选条模式里候选栏放在输入光标旁，编辑器不报告光标位置时仍停在底部。与设备有没有实体键盘有关，只在本机，不随账号同步。 */
    public static final String HARDWARE_CARET_CANDIDATES = "platform.android.hardware_caret_candidates";
    /** 工具栏上的「浮动键盘」按钮，默认不显示；同步的工具栏开关表在 Rust 的 `ANDROID_LOCAL_SETTINGS` 里，这一项先只在本机。 */
    public static final String TOOLBAR_FLOATING = "platform.android.toolbar_floating";
    /** 可选工具按钮的稳定 ID 排列，只在本机保存。 */
    public static final String TOOLBAR_ORDER = "platform.android.toolbar_order";
    /** 空闲工具栏独立尺寸；未写入时仍跟随候选栏高度。 */
    public static final String TOOLBAR_SIZE_PERCENT = "platform.android.toolbar_size_percent";
    /** 符号面板分类的稳定 ID 排列，只在本机保存。 */
    public static final String SYMBOL_CATEGORY_ORDER = "platform.android.symbol_category_order";
    /** 中文工具栏上的「随」入口，默认不显示；这里只保存入口可见性，运行时随机开关不持久化。 */
    public static final String TOOLBAR_RANDOM_DIGITS = "platform.android.toolbar_random_digits";
    /** 工具栏上的「文本编辑」按钮（#6351），点按开关文本编辑面板，默认不显示；同步字段表里没有这个键，先只在本机。 */
    public static final String TOOLBAR_TEXT_EDIT = "platform.android.toolbar_text_edit";
    /** 手势导航下垫在键区下面的底栏（{@link KeyboardBottomBarPolicy}），默认开。是否出现还取决于导航栏的高度，换一台设备时意义不同，同步字段表里也没有这个键，只在本机。 */
    public static final String BOTTOM_BAR = "platform.android.bottom_bar";
    /** 键区下面垫一段不可点的空白（{@link KeyboardBottomBarPolicy#paddingShown}），把回车等底行键抬离屏幕底边（#6392），默认关。和键盘底栏一样与设备的导航方式有关，同步字段表里也没有这个键，只在本机。 */
    public static final String BOTTOM_PADDING = "platform.android.bottom_padding";
    /** 底部留白目标高度（dp），独立于按键高度，只在本机保存。 */
    public static final String BOTTOM_PADDING_HEIGHT = "platform.android.bottom_padding_height";
    public static final String DEVELOPER_DEBUG_OVERLAY = "platform.android.developer.debug_overlay";
    /** 调试日志总开关；关闭后不显示调试信息，也不记录输入日志。 */
    public static final String DEVELOPER_LOGGING_ENABLED = "platform.android.developer.logging_enabled";
    public static final String DEVELOPER_LOG_LEVEL = "platform.android.developer.log_level";
    /** 「记录输入日志」：只记时间和事件种类，不记按键内容、文本和候选。 */
    public static final String DEVELOPER_INPUT_LOG = "platform.android.developer.input_log";
    /** 剪贴板导出目录；空值代表 Download，仅保存在本机。 */
    public static final String CLIPBOARD_EXPORT_TREE = "platform.android.clipboard_export_tree";
    /** 本地备份导出目录；空值表示每次用文件选择器，不随备份和账号同步。 */
    public static final String BACKUP_EXPORT_TREE = "platform.android.backup_export_tree";
    /** 系统文件选择器授权的诊断包目录，仅保存在本机。 */
    public static final String DIAGNOSTICS_EXPORT_TREE = "platform.android.developer.diagnostics_export_tree";
    public static final String MCP_RETENTION = "platform.android.developer.mcp_retention";
    public static final String MCP_CRASH_LOGS = "platform.android.developer.mcp_crash_logs";
    public static final String MCP_PERFORMANCE_LOGS = "platform.android.developer.mcp_performance_logs";
    public static final String MCP_INPUT_EVENTS = "platform.android.developer.mcp_input_events";
    public static final String MCP_CONFIG_SNAPSHOT = "platform.android.developer.mcp_config_snapshot";

    /** 与 {@link KeyboardGeometry#MIN_DESIGN_HEIGHT_ADJUSTMENT_DP} / {@link KeyboardGeometry#MAX_DESIGN_HEIGHT_ADJUSTMENT_DP} 相同。 */
    public static final int HEIGHT_ADJUSTMENT_MIN = KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP;
    public static final int HEIGHT_ADJUSTMENT_MAX = KeyboardHeightPolicy.MAX_DESIGN_HEIGHT_ADJUSTMENT_DP;

    /** 一项设置的类型、默认值与取值范围。 */
    public static final class Spec {
        public enum Kind { BOOLEAN, CHOICE, INTEGER, TEXT }

        public final String key;
        public final Kind kind;
        public final Object defaultValue;
        public final boolean synced;
        private final String[] choices;
        /** TEXT 项的规范化：合规时返回存储用的文本，否则 null。 */
        private final java.util.function.UnaryOperator<String> normalizer;
        public final int min;
        public final int max;
        public final int step;

        private Spec(String key, Kind kind, Object defaultValue, boolean synced,
                     String[] choices, java.util.function.UnaryOperator<String> normalizer, int min, int max, int step) {
            this.key = key;
            this.kind = kind;
            this.defaultValue = defaultValue;
            this.synced = synced;
            this.choices = choices;
            this.normalizer = normalizer;
            this.min = min;
            this.max = max;
            this.step = step;
        }

        public String[] choices() { return choices == null ? new String[0] : choices.clone(); }

        /** 合规时返回规范化的值（整数统一为 Integer），否则 null。 */
        public Object accept(Object raw) {
            switch (kind) {
                case BOOLEAN:
                    return raw instanceof Boolean ? raw : null;
                case CHOICE:
                    if (!(raw instanceof String text)) return null;
                    for (String choice : choices) if (choice.equals(text)) return choice;
                    return null;
                case TEXT:
                    return raw instanceof String text ? normalizer.apply(text) : null;
                default:
                    if (!(raw instanceof Number number)) return null;
                    double value = number.doubleValue();
                    if (Double.isNaN(value) || Double.isInfinite(value) || value != Math.rint(value)) return null;
                    if (value < min || value > max) return null;
                    int whole = (int) value;
                    return (whole - min) % step == 0 ? Integer.valueOf(whole) : null;
            }
        }
    }

    private static final Map<String, Spec> SPECS = new LinkedHashMap<>(33);

    static {
        choice(APP_THEME, "siji", true, "siji", "chunya", "xiayin", "qiushan", "dongxue");
        choice(ONE_HANDED, "off", true, "off", "left", "right");
        bool(SPLIT_KEYBOARD, false, true);
        integer(SPLIT_KEYBOARD_THRESHOLD, SplitKeyboardPolicy.DEFAULT_THRESHOLD_DP, false,
            SplitKeyboardPolicy.MIN_THRESHOLD_DP, SplitKeyboardPolicy.MAX_THRESHOLD_DP, SplitKeyboardPolicy.THRESHOLD_STEP_DP);
        integer(SPLIT_KEYBOARD_GAP, SplitKeyboardPolicy.DEFAULT_GAP_PERCENT, false,
            SplitKeyboardPolicy.MIN_GAP_PERCENT, SplitKeyboardPolicy.MAX_GAP_PERCENT, SplitKeyboardPolicy.GAP_PERCENT_STEP);
        bool(KEY_POPUP, true, true);
        bool(SWIPE_DOWN_SYMBOLS, true, true);
        choice(SWIPE_SYMBOLS_DIRECTION, SwipeHintPolicy.UP, true, SwipeHintPolicy.DOWN, SwipeHintPolicy.UP);
        bool(SPACE_CURSOR, true, true);
        bool(SPACE_VOICE, true, true);
        choice(KEY_ANIMATION, "none", true, "none", "bounce", "ripple", "glow", "lift");
        bool(TOOLBAR_PHRASE, true, true);
        bool(TOOLBAR_SCHEME, true, true);
        bool(TOOLBAR_HIDDEN, false, true);
        bool(LONG_SPACE_LAYOUT, false, false);
        choice(HANDWRITING_MODE, "overlap", true, "single", "overlap", "line");
        integer(HANDWRITING_DELAY_MS, 500, true, 200, 1500, 100);
        bool(HANDWRITING_SHOW_PINYIN, true, true);
        choice(HANDWRITING_STROKE_COLOR, "follow_skin", true, "follow_skin", "black", "white", "blue");
        integer(HANDWRITING_STROKE_WIDTH, 3, true, 1, 8, 1);
        bool(VOICE_OFFLINE_FALLBACK, true, true);

        bool(INCOGNITO, false, false);
        bool(VOICE_CONTRIBUTE_AUDIO, false, false);
        integer(VOICE_PAUSE_MILLIS, VOICE_PAUSE_DEFAULT_MILLIS, false,
            VOICE_PAUSE_MIN_MILLIS, VOICE_PAUSE_MAX_MILLIS, 100);
        integer(VOICE_VAD_SENSITIVITY, 50, false, 10, 90, 1);
        bool(SCREEN_CAPTURE_PROTECTION, false, false);
        bool(SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE, false, false);
        bool(GLIDE_TYPING, false, false);
        bool(LANGUAGE_KEY_CYCLE, false, false);
        bool(SWAP_LANGUAGE_NUMBER_KEYS, false, false);
        bool(AUTO_CAPITALIZATION, false, false);
        choice(CLIPBOARD_COLUMNS, ClipboardLayoutPolicy.ONE_COLUMN, false,
            ClipboardLayoutPolicy.ONE_COLUMN, ClipboardLayoutPolicy.TWO_COLUMNS);
        bool(CLIPBOARD_SUGGESTION, true, false);
        integer(CLIPBOARD_UNDO_SECONDS, ClipboardUndoPolicy.DEFAULT_SECONDS, false,
            ClipboardUndoPolicy.MIN_SECONDS, ClipboardUndoPolicy.MAX_SECONDS, 1);
        bool(CLIPBOARD_DELETE_AUTHENTICATION, false, false);
        bool(CLIPBOARD_SUGGESTION_ACTION, true, false);
        choice(CLIPBOARD_SUGGESTION_ACTION_KIND, RecentClipboardSuggestion.ACTION_CATEGORY, false,
            RecentClipboardSuggestion.ACTION_SEGMENT, RecentClipboardSuggestion.ACTION_CATEGORY);
        bool(NUMBER_ROW, false, false);
        choice(NINE_KEY_SWIPE, NineKeySwipePolicy.OFF, false, NineKeySwipePolicy.OFF, SwipeHintPolicy.UP,
            SwipeHintPolicy.DOWN);
        integer(NINE_KEY_SWIPE_DISTANCE, NineKeySwipePolicy.DEFAULT_THRESHOLD_DP, false,
            NineKeySwipePolicy.MIN_THRESHOLD_DP, NineKeySwipePolicy.MAX_THRESHOLD_DP, NineKeySwipePolicy.THRESHOLD_STEP_DP);
        text(NINE_KEY_SYMBOLS, NineKeySidebarPolicy.format(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS), false,
            NineKeySidebarPolicy::normalize);
        text(NINE_KEY_DIGIT_SYMBOLS, NineKeySidebarPolicy.format(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS), false,
            NineKeySidebarPolicy::normalize);
        choice(QUICK_DELETE_SCOPE, "before", false, "before", "all");
        integer(KEYBOARD_HEIGHT_ADJUSTMENT, 0, false, HEIGHT_ADJUSTMENT_MIN, HEIGHT_ADJUSTMENT_MAX, 1);
        bool(FLOATING_KEYBOARD, false, false);
        integer(FLOATING_KEYBOARD_X, FloatingKeyboardPolicy.DEFAULT_X_FRACTION, false, 0,
            FloatingKeyboardPolicy.MAX_FRACTION, 1);
        integer(FLOATING_KEYBOARD_Y, FloatingKeyboardPolicy.DEFAULT_Y_FRACTION, false, 0,
            FloatingKeyboardPolicy.MAX_FRACTION, 1);
        bool(HARDWARE_CARET_CANDIDATES, false, false);
        bool(TOOLBAR_FLOATING, false, false);
        bool(TOOLBAR_TEXT_EDIT, false, false);
        bool(TOOLBAR_RANDOM_DIGITS, false, false);
        text(TOOLBAR_ORDER, ToolbarOrderPolicy.format(ToolbarOrderPolicy.DEFAULT_ORDER), false,
            ToolbarOrderPolicy::normalize);
        integer(TOOLBAR_SIZE_PERCENT, ToolbarSizePolicy.DEFAULT_PERCENT, false,
            ToolbarSizePolicy.MIN_PERCENT, ToolbarSizePolicy.MAX_PERCENT, 10);
        text(SYMBOL_CATEGORY_ORDER, SymbolCategoryOrderPolicy.format(SymbolCategoryOrderPolicy.DEFAULT_ORDER), false,
            SymbolCategoryOrderPolicy::normalize);
        bool(BOTTOM_BAR, true, false);
        bool(BOTTOM_PADDING, false, false);
        integer(BOTTOM_PADDING_HEIGHT, KeyboardBottomBarPolicy.DEFAULT_PADDING_DP, false,
            KeyboardBottomBarPolicy.MIN_PADDING_DP, KeyboardBottomBarPolicy.MAX_PADDING_DP, 1);
        bool(DEVELOPER_DEBUG_OVERLAY, false, false);
        bool(DEVELOPER_LOGGING_ENABLED, true, false);
        choice(DEVELOPER_LOG_LEVEL, "debug", false, "error", "warn", "info", "debug");
        bool(DEVELOPER_INPUT_LOG, false, false);
        text(CLIPBOARD_EXPORT_TREE, "", false,
            value -> value.length() <= 8192 && (value.isEmpty() || value.startsWith("content://")) ? value : null);
        text(BACKUP_EXPORT_TREE, "", false,
            value -> value.length() <= 8192 && (value.isEmpty() || value.startsWith("content://")) ? value : null);
        text(DIAGNOSTICS_EXPORT_TREE, "", false,
            value -> value.length() <= 8192 && (value.isEmpty() || value.startsWith("content://")) ? value : null);
        choice(MCP_RETENTION, "one_day", false, "one_hour", "one_day", "seven_days");
        bool(MCP_CRASH_LOGS, true, false);
        bool(MCP_PERFORMANCE_LOGS, true, false);
        bool(MCP_INPUT_EVENTS, false, false);
        bool(MCP_CONFIG_SNAPSHOT, true, false);
    }

    private static void bool(String key, boolean fallback, boolean synced) {
        SPECS.put(key, new Spec(key, Spec.Kind.BOOLEAN, fallback, synced, null, null, 0, 0, 1));
    }

    private static void choice(String key, String fallback, boolean synced, String... choices) {
        SPECS.put(key, new Spec(key, Spec.Kind.CHOICE, fallback, synced, choices, null, 0, 0, 1));
    }

    private static void text(String key, String fallback, boolean synced,
                             java.util.function.UnaryOperator<String> normalizer) {
        SPECS.put(key, new Spec(key, Spec.Kind.TEXT, fallback, synced, null, normalizer, 0, 0, 1));
    }

    private static void integer(String key, int fallback, boolean synced, int min, int max, int step) {
        SPECS.put(key, new Spec(key, Spec.Kind.INTEGER, fallback, synced, null, null, min, max, step));
    }

    /** 全部设置项，按声明顺序。 */
    public static Map<String, Spec> specs() { return Collections.unmodifiableMap(SPECS); }

    public static Spec spec(String key) {
        Spec spec = SPECS.get(key);
        if (spec == null) throw new IllegalArgumentException("unknown Android local setting: " + key);
        return spec;
    }

    /** 某一时刻的设置：只保存显式写过且合规的值，其余读默认值。 */
    public static final class Snapshot {
        private final Map<String, Object> values;

        Snapshot(Map<String, Object> values) {
            this.values = Collections.unmodifiableMap(new TreeMap<>(values));
        }

        public boolean has(String key) { return values.containsKey(spec(key).key); }

        public Object value(String key) {
            Spec spec = spec(key);
            Object value = values.get(key);
            return value == null ? spec.defaultValue : value;
        }

        public boolean bool(String key) {
            requireKind(key, Spec.Kind.BOOLEAN);
            return (Boolean) value(key);
        }

        public String choice(String key) {
            requireKind(key, Spec.Kind.CHOICE);
            return (String) value(key);
        }

        public int integer(String key) {
            requireKind(key, Spec.Kind.INTEGER);
            return (Integer) value(key);
        }

        public String text(String key) {
            requireKind(key, Spec.Kind.TEXT);
            return (String) value(key);
        }

        /** 显式写过的值，键名排序。 */
        public Map<String, Object> explicit() { return values; }

        /** 参与同步的全部键及其当前值（未写过的取默认值），交给 `msime_client_account_settings_export` 的 `android_local`。 */
        public Map<String, Object> synced() {
            Map<String, Object> synced = new TreeMap<>();
            for (Spec spec : SPECS.values()) if (spec.synced) synced.put(spec.key, value(spec.key));
            return synced;
        }

        @Override public boolean equals(Object other) {
            return other instanceof Snapshot snapshot && values.equals(snapshot.values);
        }

        @Override public int hashCode() { return values.hashCode(); }

        private static void requireKind(String key, Spec.Kind kind) {
            if (spec(key).kind != kind) throw new IllegalArgumentException(key + " is not a " + kind);
        }
    }

    private static final Snapshot DEFAULTS = new Snapshot(Collections.emptyMap());
    private static final Object CACHE_LOCK = new Object();
    /** 把同一进程内的 {@link #update(Path, Map)} 串行化，原因见那里的注释。 */
    private static final Object UPDATE_LOCK = new Object();
    private static Path cachedFile;
    private static Object cachedStamp;
    private static Snapshot cachedSnapshot = DEFAULTS;

    private AndroidLocalSettings() {}

    public static Snapshot defaults() { return DEFAULTS; }

    /**
     * 设置页「键盘底部高度」滑块显示的值（#10447）：{@link #BOTTOM_PADDING} 开着时是 {@link #BOTTOM_PADDING_HEIGHT}，关着时是 0。
     *
     * <p>这个数是键区底边到系统导航区的目标总距离，键盘底栏画着时它的 40 dp 计在里面（{@link KeyboardBottomBarPolicy#extraPaddingDp}）。没动过的用户显示 0；以前打开过「底部留白」的老用户直接显示原来的高度，不用迁移。
     */
    public static int bottomHeightDp(Snapshot settings) {
        return settings.bool(BOTTOM_PADDING) ? settings.integer(BOTTOM_PADDING_HEIGHT) : 0;
    }

    /** 滑块提交 `valueDp` 时要写的项：大于 0 时打开 {@link #BOTTOM_PADDING} 并记下高度；拖到 0 只关开关，原来的高度留着不清。 */
    public static Map<String, Object> bottomHeightEdits(int valueDp) {
        Map<String, Object> edits = new LinkedHashMap<>(2);
        edits.put(BOTTOM_PADDING, valueDp > 0);
        if (valueDp > 0) edits.put(BOTTOM_PADDING_HEIGHT, valueDp);
        return edits;
    }

    public static Path file(Context context) {
        return context.getFilesDir().toPath().resolve("bootstrap").resolve("state").resolve(FILE_NAME);
    }

    public static Snapshot load(Context context) { return load(file(context)); }

    /** 读出当前设置。文件与上次读时相同（inode、修改时间、大小都没变）就返回缓存，否则重新读。读不出时一律是默认值。 */
    public static Snapshot load(Path file) {
        Object stamp = stamp(file);
        synchronized (CACHE_LOCK) {
            if (file.equals(cachedFile) && Objects.equals(stamp, cachedStamp)) return cachedSnapshot;
        }
        Snapshot snapshot = stamp == null ? DEFAULTS : read(file);
        synchronized (CACHE_LOCK) {
            cachedFile = file;
            cachedStamp = stamp;
            cachedSnapshot = snapshot;
        }
        return snapshot;
    }

    public static Snapshot update(Context context, Map<String, Object> edits) throws IOException {
        return update(file(context), edits);
    }

    public static Snapshot put(Context context, String key, Object value) throws IOException {
        Map<String, Object> edits = new LinkedHashMap<>(1);
        edits.put(key, value);
        return update(file(context), edits);
    }

    /**
     * 在进程间文件锁里读出当前文件、套用 `edits`、原子写回。值为 null 表示删掉这一项（回到默认值）。键不认识或取值不合规时抛 IllegalArgumentException，什么也不写。
     */
    public static Snapshot update(Path file, Map<String, Object> edits) throws IOException {
        if (file == null) throw new IllegalArgumentException("settings file");
        Map<String, Object> accepted = new LinkedHashMap<>(edits.size());
        for (Map.Entry<String, Object> edit : edits.entrySet()) {
            Spec spec = spec(edit.getKey());
            if (edit.getValue() == null) {
                accepted.put(spec.key, null);
                continue;
            }
            Object value = spec.accept(edit.getValue());
            if (value == null) {
                throw new IllegalArgumentException("invalid value for " + spec.key + ": " + edit.getValue());
            }
            accepted.put(spec.key, value);
        }
        // 文件锁属于整个 JVM 而不是线程：同一进程里另一个线程已经持有时，channel.lock() 不会等待，而是抛 OverlappingFileLockException。所以先在这里把本进程的写入排成一队，文件锁只负责协调主进程与 :ime。CACHE_LOCK 只在这把锁里面取，load() 从不取这把锁，加锁顺序不会颠倒。
        synchronized (UPDATE_LOCK) {
            Path parent = file.getParent();
            if (parent == null) throw new IOException("settings directory unavailable");
            SafePaths.ensureDirectory(parent);
            Path lockFile = parent.resolve(FILE_NAME + ".lock");
            try (FileChannel channel = FileChannel.open(lockFile, StandardOpenOption.CREATE,
                    StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS)) {
                FileLock lock = channel.lock();
                try {
                    Map<String, Object> next = new TreeMap<>(stamp(file) == null
                        ? Collections.<String, Object>emptyMap() : read(file).explicit());
                    for (Map.Entry<String, Object> edit : accepted.entrySet()) {
                        if (edit.getValue() == null) next.remove(edit.getKey());
                        else next.put(edit.getKey(), edit.getValue());
                    }
                    String encoded;
                    try {
                        encoded = encode(next);
                    } catch (JSONException error) {
                        throw new IOException("settings encoding failed", error);
                    }
                    writeAtomically(file, encoded.getBytes(StandardCharsets.UTF_8));
                    Snapshot snapshot = new Snapshot(next);
                    Object stamp = stamp(file);
                    synchronized (CACHE_LOCK) {
                        cachedFile = file;
                        cachedStamp = stamp;
                        cachedSnapshot = snapshot;
                    }
                    return snapshot;
                } finally {
                    lock.release();
                }
            }
        }
    }

    public static Snapshot restoreDefaults(Context context) throws IOException {
        return restoreDefaults(file(context));
    }

    /** 「恢复出厂设置」：删掉全部显式写过的值，每一项回到默认值。 */
    public static Snapshot restoreDefaults(Path file) throws IOException {
        Map<String, Object> edits = new LinkedHashMap<>(SPECS.size());
        for (String key : SPECS.keySet()) edits.put(key, null);
        return update(file, edits);
    }

    public static Snapshot applySynced(Context context, Map<String, ?> cloud) throws IOException {
        return applySynced(file(context), cloud);
    }

    /** 写回云端文档里属于本地设置的值（`msime_client_account_settings_apply` 返回的 `android_local`）：只收参与同步、取值合规的键，其余忽略；没有可写的就不碰文件。 */
    public static Snapshot applySynced(Path file, Map<String, ?> cloud) throws IOException {
        Map<String, Object> edits = new LinkedHashMap<>(cloud.size());
        for (Map.Entry<String, ?> entry : cloud.entrySet()) {
            Spec spec = SPECS.get(entry.getKey());
            if (spec == null || !spec.synced) continue;
            Object value = spec.accept(entry.getValue());
            if (value != null) edits.put(spec.key, value);
        }
        return edits.isEmpty() ? load(file) : update(file, edits);
    }

    /** 只保留认识且合规的键；其余（包括别的版本写下、本版本不认识的键）丢掉。 */
    static Map<String, Object> accepted(Map<String, ?> raw) {
        Map<String, Object> accepted = new TreeMap<>();
        for (Map.Entry<String, ?> entry : raw.entrySet()) {
            Spec spec = SPECS.get(entry.getKey());
            if (spec == null) continue;
            Object value = spec.accept(entry.getValue());
            if (value != null) accepted.put(spec.key, value);
        }
        return accepted;
    }

    static Snapshot decode(String text) throws JSONException {
        if (text == null || text.length() > MAX_BYTES) throw new JSONException("settings size");
        JSONObject document = new JSONObject(text);
        JSONObject settings = document.optJSONObject("settings");
        if (settings == null) return DEFAULTS;
        Map<String, Object> raw = new LinkedHashMap<>(settings.length());
        for (Iterator<String> keys = settings.keys(); keys.hasNext(); ) {
            String key = keys.next();
            raw.put(key, settings.opt(key));
        }
        return new Snapshot(accepted(raw));
    }

    static String encode(Map<String, Object> values) throws JSONException {
        JSONObject settings = new JSONObject();
        for (Map.Entry<String, Object> entry : accepted(values).entrySet()) {
            settings.put(entry.getKey(), entry.getValue());
        }
        return new JSONObject().put("version", VERSION).put("settings", settings).toString();
    }

    private static Snapshot read(Path file) {
        try {
            if (!privateFile(file))
                throw new IOException("settings path is not a regular file");
            byte[] bytes;
            try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
                bytes = HttpBodyPolicy.readRequired(input, MAX_BYTES);
            }
            return decode(new String(bytes, StandardCharsets.UTF_8));
        } catch (IOException | JSONException | RuntimeException ignored) {
            return DEFAULTS;
        }
    }

    static boolean privateFile(Path file) {
        return file != null
            && Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)
            && SafePaths.isSingleLink(file);
    }

    /** 文件身份：inode、修改时间和大小；文件不存在或不是普通文件时为 null。 */
    private static Object stamp(Path file) {
        try {
            BasicFileAttributes attributes = Files.readAttributes(file, BasicFileAttributes.class,
                LinkOption.NOFOLLOW_LINKS);
            if (!attributes.isRegularFile()) return null;
            return java.util.Arrays.asList(attributes.fileKey(), attributes.lastModifiedTime(),
                attributes.size());
        } catch (IOException | RuntimeException error) {
            return null;
        }
    }

    static void writeAtomically(Path file, byte[] content) throws IOException {
        if (content.length > MAX_BYTES) throw new IOException("settings size");
        FilePolicy.writeAtomically(file, content);
    }
}
