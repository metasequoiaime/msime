package app.msime.android;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
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
 * <p>键名与账号设置文档的同步键相同（`general.app_theme`、`platform.android.*`）。{@link Spec#synced} 为真的十七项随云同步交给 client-core 的 `android_local`（crates/client-core/src/account/settings_sync.rs 的 `ANDROID_LOCAL_SETTINGS`，两边的键与取值范围由 AndroidLocalSettingsSmoke 锁住）；隐私模式、语音数据贡献、开发者选项、键盘高度、浮动键盘的开关与位置和工具栏上的浮动键盘按钮只留在本机。
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
    public static final String HANDWRITING_MODE = "platform.android.handwriting_mode";
    public static final String HANDWRITING_DELAY_MS = "platform.android.handwriting_delay_ms";
    public static final String HANDWRITING_SHOW_PINYIN = "platform.android.handwriting_show_pinyin";
    public static final String HANDWRITING_STROKE_COLOR = "platform.android.handwriting_stroke_color";
    public static final String HANDWRITING_STROKE_WIDTH = "platform.android.handwriting_stroke_width";
    public static final String VOICE_OFFLINE_FALLBACK = "platform.android.voice_offline_fallback";

    // ---- 只在本机的键 ----
    public static final String INCOGNITO = "platform.android.incognito";
    public static final String VOICE_CONTRIBUTE_AUDIO = "platform.android.voice_contribute_audio";
    /** 「滑行输入」（{@link GlideTypingPolicy}），默认关。服务端的同步字段表还没有这个键，所以先只在本机。 */
    public static final String GLIDE_TYPING = "platform.android.glide_typing";
    /** 设计范围的键盘高度调整（dp，-46..110，即 75%..160%）。缺省时宿主沿用共享偏好里的 `touch_keyboard_height_adjustment`（-12..48）。 */
    public static final String KEYBOARD_HEIGHT_ADJUSTMENT = "platform.android.keyboard_height_adjustment";
    /** 浮动键盘（{@link FloatingKeyboardPolicy}），默认关。位置与屏幕尺寸相关，开关与位置都只在本机，不随账号同步。 */
    public static final String FLOATING_KEYBOARD = "platform.android.floating_keyboard";
    /** 浮动键盘在可移动范围里的水平 / 竖直位置，千分比（0 最左 / 最上，1000 最右 / 最下）。 */
    public static final String FLOATING_KEYBOARD_X = "platform.android.floating_keyboard_x";
    public static final String FLOATING_KEYBOARD_Y = "platform.android.floating_keyboard_y";
    /** 工具栏上的「浮动键盘」按钮，默认不显示；同步的工具栏开关表在 Rust 的 `ANDROID_LOCAL_SETTINGS` 里，这一项先只在本机。 */
    public static final String TOOLBAR_FLOATING = "platform.android.toolbar_floating";
    public static final String DEVELOPER_DEBUG_OVERLAY = "platform.android.developer.debug_overlay";
    public static final String DEVELOPER_LOG_LEVEL = "platform.android.developer.log_level";
    /** 「记录输入日志」：只记时间和事件种类，不记按键内容、文本和候选。 */
    public static final String DEVELOPER_INPUT_LOG = "platform.android.developer.input_log";
    public static final String MCP_RETENTION = "platform.android.developer.mcp_retention";
    public static final String MCP_CRASH_LOGS = "platform.android.developer.mcp_crash_logs";
    public static final String MCP_PERFORMANCE_LOGS = "platform.android.developer.mcp_performance_logs";
    public static final String MCP_INPUT_EVENTS = "platform.android.developer.mcp_input_events";
    public static final String MCP_CONFIG_SNAPSHOT = "platform.android.developer.mcp_config_snapshot";

    /** 与 {@link KeyboardGeometry#MIN_DESIGN_HEIGHT_ADJUSTMENT_DP} / {@link KeyboardGeometry#MAX_DESIGN_HEIGHT_ADJUSTMENT_DP} 相同。 */
    public static final int HEIGHT_ADJUSTMENT_MIN = KeyboardGeometry.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP;
    public static final int HEIGHT_ADJUSTMENT_MAX = KeyboardGeometry.MAX_DESIGN_HEIGHT_ADJUSTMENT_DP;

    /** 一项设置的类型、默认值与取值范围。 */
    public static final class Spec {
        public enum Kind { BOOLEAN, CHOICE, INTEGER }

        public final String key;
        public final Kind kind;
        public final Object defaultValue;
        public final boolean synced;
        private final String[] choices;
        public final int min;
        public final int max;
        public final int step;

        private Spec(String key, Kind kind, Object defaultValue, boolean synced,
                     String[] choices, int min, int max, int step) {
            this.key = key;
            this.kind = kind;
            this.defaultValue = defaultValue;
            this.synced = synced;
            this.choices = choices;
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
        bool(KEY_POPUP, true, true);
        bool(SWIPE_DOWN_SYMBOLS, true, true);
        choice(SWIPE_SYMBOLS_DIRECTION, SwipeHintPolicy.DOWN, true, SwipeHintPolicy.DOWN, SwipeHintPolicy.UP);
        bool(SPACE_CURSOR, true, true);
        bool(SPACE_VOICE, true, true);
        choice(KEY_ANIMATION, "none", true, "none", "bounce", "ripple", "glow", "lift");
        bool(TOOLBAR_PHRASE, true, true);
        bool(TOOLBAR_SCHEME, true, true);
        bool(TOOLBAR_HIDDEN, false, true);
        choice(HANDWRITING_MODE, "overlap", true, "single", "overlap", "line");
        integer(HANDWRITING_DELAY_MS, 500, true, 200, 1500, 100);
        bool(HANDWRITING_SHOW_PINYIN, true, true);
        choice(HANDWRITING_STROKE_COLOR, "follow_skin", true, "follow_skin", "black", "white", "blue");
        integer(HANDWRITING_STROKE_WIDTH, 3, true, 1, 8, 1);
        bool(VOICE_OFFLINE_FALLBACK, false, true);

        bool(INCOGNITO, false, false);
        bool(VOICE_CONTRIBUTE_AUDIO, false, false);
        bool(GLIDE_TYPING, false, false);
        integer(KEYBOARD_HEIGHT_ADJUSTMENT, 0, false, HEIGHT_ADJUSTMENT_MIN, HEIGHT_ADJUSTMENT_MAX, 1);
        bool(FLOATING_KEYBOARD, false, false);
        integer(FLOATING_KEYBOARD_X, FloatingKeyboardPolicy.DEFAULT_X_FRACTION, false, 0,
            FloatingKeyboardPolicy.MAX_FRACTION, 1);
        integer(FLOATING_KEYBOARD_Y, FloatingKeyboardPolicy.DEFAULT_Y_FRACTION, false, 0,
            FloatingKeyboardPolicy.MAX_FRACTION, 1);
        bool(TOOLBAR_FLOATING, false, false);
        bool(DEVELOPER_DEBUG_OVERLAY, false, false);
        choice(DEVELOPER_LOG_LEVEL, "warn", false, "error", "warn", "info", "debug");
        bool(DEVELOPER_INPUT_LOG, false, false);
        choice(MCP_RETENTION, "one_day", false, "one_hour", "one_day", "seven_days");
        bool(MCP_CRASH_LOGS, true, false);
        bool(MCP_PERFORMANCE_LOGS, true, false);
        bool(MCP_INPUT_EVENTS, false, false);
        bool(MCP_CONFIG_SNAPSHOT, true, false);
    }

    private static void bool(String key, boolean fallback, boolean synced) {
        SPECS.put(key, new Spec(key, Spec.Kind.BOOLEAN, fallback, synced, null, 0, 0, 1));
    }

    private static void choice(String key, String fallback, boolean synced, String... choices) {
        SPECS.put(key, new Spec(key, Spec.Kind.CHOICE, fallback, synced, choices, 0, 0, 1));
    }

    private static void integer(String key, int fallback, boolean synced, int min, int max, int step) {
        SPECS.put(key, new Spec(key, Spec.Kind.INTEGER, fallback, synced, null, min, max, step));
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
            if (Files.isSymbolicLink(file) || !Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS))
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
        Path temporary = Files.createTempFile(file.getParent(), FILE_NAME + ".", ".tmp");
        try {
            Files.write(temporary, content, StandardOpenOption.TRUNCATE_EXISTING);
            try {
                Files.move(temporary, file, StandardCopyOption.ATOMIC_MOVE,
                    StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException ignored) {
                Files.move(temporary, file, StandardCopyOption.REPLACE_EXISTING);
            }
        } finally {
            Files.deleteIfExists(temporary);
        }
    }
}
