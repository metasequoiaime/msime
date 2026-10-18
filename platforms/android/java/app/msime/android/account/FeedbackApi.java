package app.msime.android;

import android.content.Context;
import java.io.File;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 应用内反馈：由 Rust 的 {@code msime_client_account} 的 feedback 以 multipart 发到 `POST /v1/feedback`（#8201 迁移计划 5c），另带至多 3 张截图。
 *
 * <p>登录账号优先，没有登录时用设备的匿名账号，所以没登录也能提交，服务端按用户限流（每小时 5 次、每天 20 次）。诊断信息只在用户打开「附带诊断信息」时才带，而且只保留服务端白名单里的键（{@link #DIAGNOSTIC_KEYS}），每个值不超过 256 字节；这里从不放输入内容。
 *
 * <p>截图：Rust 不重新编码，只读文件头和尺寸，一张超过 1 MiB、单边超过 8192 或总像素超过 8388608 是 account_too_large（迁移差异 25）。页面照旧把每张图缩到长边 1600 像素、1 MiB 以内重新编码成 JPEG（顺带去掉 EXIF 里的位置），所以交给 Rust 的总在限制以内；{@link #fitsRust} 再核对一遍，写进应用私有目录的文件交给 Rust 读。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class FeedbackApi {
    public static final int MAX_TEXT = 500;
    public static final int MAX_SCREENSHOTS = 3;
    public static final int MAX_SCREENSHOT_BYTES = 1024 * 1024;
    public static final int MAX_DIAGNOSTIC_VALUE_BYTES = 256;
    /** Rust 收的截图单边与总像素上限（头文件 feedback 那一段）。 */
    public static final int MAX_SCREENSHOT_EDGE = 8192;
    public static final long MAX_SCREENSHOT_PIXELS = 8_388_608L;
    /** 服务端接受的诊断字段，其他键一律丢掉。 */
    public static final List<String> DIAGNOSTIC_KEYS = List.of(
        "device", "os", "app_version", "edition", "scheme", "keyboard_layout", "skin", "ime_enabled", "ime_default");

    /** 反馈类型：服务端的取值与界面上的名字。 */
    public enum Type {
        BUG("bug", "问题"),
        SUGGESTION("suggestion", "建议"),
        DICTIONARY("dictionary", "词库纠错");

        private final String id;
        private final String title;

        Type(String id, String title) {
            this.id = id;
            this.title = title;
        }

        public String id() { return id; }

        public String title() { return title; }
    }

    private FeedbackApi() {}

    /** 描述去掉首尾空白后非空、不超过 500 个字符、不含换行和制表以外的控制字符时可以提交。 */
    public static boolean validText(String text) {
        return TextPolicy.validBoundedText(text, MAX_TEXT);
    }

    /** 只保留白名单键；值去掉控制字符、截到 256 字节以内（不截断在一个字符中间），空值丢掉。 */
    public static Map<String, String> filterDiagnostics(Map<String, String> raw) {
        Map<String, String> clean = new LinkedHashMap<>(DIAGNOSTIC_KEYS.size());
        if (raw == null) return clean;
        for (String key : DIAGNOSTIC_KEYS) {
            String value = raw.get(key);
            if (value == null) continue;
            String trimmed = TextPolicy.clipUtf8(
                TextPolicy.trimmed(TextPolicy.replaceControls(value, ' ')), MAX_DIAGNOSTIC_VALUE_BYTES);
            if (!trimmed.isEmpty()) clean.put(key, trimmed);
        }
        return clean;
    }

    /** 一张截图的大小和尺寸在 Rust 的限制以内：不超过 1 MiB、单边不超过 8192、总像素不超过 8388608。尺寸读不出（0 或负数）时不算。 */
    public static boolean fitsRust(long bytes, int width, int height) {
        return bytes > 0 && bytes <= MAX_SCREENSHOT_BYTES && width > 0 && height > 0
            && width <= MAX_SCREENSHOT_EDGE && height <= MAX_SCREENSHOT_EDGE
            && (long) width * height <= MAX_SCREENSHOT_PIXELS;
    }

    /**
     * 提交一条反馈，返回服务端给的编号。
     *
     * @param diagnostics 用户没打开「附带诊断信息」时传 null
     * @param screenshots 页面已经重新编码、核对过的截图文件（应用私有目录里的普通文件，绝对路径）
     */
    public static String submit(Context context, Type type, String text, String appVersion, String edition,
            Map<String, String> diagnostics, List<File> screenshots) throws AccountStore.Failure {
        if (type == null || !validText(text)) throw new AccountStore.Failure("account_invalid", 0);
        List<File> shots = ListPolicy.copyOrEmpty(screenshots);
        if (shots.size() > MAX_SCREENSHOTS) throw new AccountStore.Failure("account_invalid", 0);
        final JSONObject action;
        try {
            action = new JSONObject()
                .put("operation", "feedback")
                .put("type", type.id())
                .put("text", TextPolicy.trimmed(text))
                .put("platform", "android")
                .put("app_version", TextPolicy.clipUtf8(appVersion == null ? "" : appVersion, 64))
                .put("edition", edition == null ? "" : edition);
            if (diagnostics != null) {
                Map<String, String> clean = filterDiagnostics(diagnostics);
                if (!clean.isEmpty()) action.put("diagnostics", new JSONObject(clean));
            }
            if (!shots.isEmpty()) {
                JSONArray paths = new JSONArray();
                for (File shot : shots) paths.put(shot.getAbsolutePath());
                action.put("screenshots", paths);
            }
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        Object value = RustAccount.preferredAccount(context, action);
        return value instanceof JSONObject response ? JsonPolicy.strictStringOrEmpty(response.opt("id")) : "";
    }

}
