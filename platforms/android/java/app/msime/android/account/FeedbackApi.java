package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 应用内反馈：`POST /v1/feedback`，multipart 的 `payload` 字段是 JSON，另带至多 3 张截图。
 *
 * <p>用户会话鉴权，没有真实账号时用设备的匿名账号，所以没登录也能提交，服务端按用户限流（每小时 5 次、每天 20 次）。诊断信息只在用户打开「附带诊断信息」时才带，而且只保留服务端白名单里的键（{@link #DIAGNOSTIC_KEYS}），每个值不超过 256 字节；这里从不放输入内容。截图由页面先重新编码成不超过 1 MiB 的 PNG 或 JPEG（同时去掉 EXIF），这里再按文件头核对一遍。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class FeedbackApi {
    public static final String PATH = "/v1/feedback";
    public static final int MAX_TEXT = 500;
    public static final int MAX_SCREENSHOTS = 3;
    public static final int MAX_SCREENSHOT_BYTES = 1024 * 1024;
    public static final int MAX_DIAGNOSTIC_VALUE_BYTES = 256;
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

    /** 一张截图：`image/png` 或 `image/jpeg`。 */
    public record Screenshot(String contentType, byte[] bytes) {}

    private final CloudApi api;

    public FeedbackApi(CloudApi api) {
        this.api = api;
    }

    /** 描述去掉首尾空白后非空、不超过 500 个字符、不含换行和制表以外的控制字符时可以提交。 */
    public static boolean validText(String text) {
        if (!TextPolicy.hasText(text)) return false;
        if (TextPolicy.codePointLength(text) > MAX_TEXT) return false;
        return !TextPolicy.hasControlExceptWhitespace(text) && TextPolicy.validUnicode(text);
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

    /** 截图是 PNG 或 JPEG（看文件头，与声明的类型一致）、非空、不超过 1 MiB 时为真。 */
    public static boolean validScreenshot(Screenshot shot) {
        if (shot == null || shot.bytes() == null) return false;
        byte[] data = shot.bytes();
        if (data.length == 0 || data.length > MAX_SCREENSHOT_BYTES) return false;
        if ("image/png".equals(shot.contentType())) {
            return data.length >= 8 && (data[0] & 0xff) == 0x89 && data[1] == 'P' && data[2] == 'N' && data[3] == 'G';
        }
        if ("image/jpeg".equals(shot.contentType())) {
            return data.length >= 3 && (data[0] & 0xff) == 0xff && (data[1] & 0xff) == 0xd8 && (data[2] & 0xff) == 0xff;
        }
        return false;
    }

    /**
     * 提交一条反馈，返回服务端给的编号。
     *
     * @param diagnostics 用户没打开「附带诊断信息」时传 null
     */
    public String submit(Type type, String text, String appVersion, String edition,
            Map<String, String> diagnostics, List<Screenshot> screenshots) throws CloudApi.Failure {
        if (type == null || !validText(text)) throw new IllegalArgumentException("invalid feedback text");
        List<Screenshot> shots = ListPolicy.copyOrEmpty(screenshots);
        if (shots.size() > MAX_SCREENSHOTS) throw new IllegalArgumentException("too many screenshots");
        for (Screenshot shot : shots) {
            if (!validScreenshot(shot)) throw new IllegalArgumentException("invalid screenshot");
        }
        String payload;
        try {
            JSONObject json = new JSONObject()
                .put("type", type.id())
                .put("text", TextPolicy.trimmed(text))
                .put("platform", "android")
                .put("app_version", TextPolicy.clipUtf8(appVersion == null ? "" : appVersion, 64))
                .put("edition", edition == null ? "" : edition);
            if (diagnostics != null) {
                Map<String, String> clean = filterDiagnostics(diagnostics);
                if (!clean.isEmpty()) json.put("diagnostics", new JSONObject(clean));
            }
            payload = json.toString();
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        List<CloudApi.Part> parts = new ArrayList<>(shots.size() + 1);
        parts.add(CloudApi.Part.json("payload", payload));
        for (int index = 0; index < shots.size(); index++) {
            Screenshot shot = shots.get(index);
            String extension = "image/png".equals(shot.contentType()) ? "png" : "jpg";
            parts.add(CloudApi.Part.file("screenshots", "screenshot-" + (index + 1) + "." + extension,
                shot.contentType(), shot.bytes()));
        }
        JSONObject response = api.multipart(PATH, parts, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        return JsonPolicy.strictStringOrEmpty(response.opt("id"));
    }

}
