import app.msime.android.CloudClipboardApi;
import app.msime.android.DownloadLinkApi;
import app.msime.android.FeedbackApi;
import app.msime.android.JsonPolicy;
import app.msime.android.TextPolicy;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class FeedbackApiSmoke {
    public static void main(String[] arguments) {
        check(FeedbackApi.Type.BUG.id().equals("bug") && FeedbackApi.Type.DICTIONARY.id().equals("dictionary"), "type ids");
        check(!FeedbackApi.validText("   "), "blank text refused");
        check(FeedbackApi.validText("候选词不对\n第二行"), "newlines allowed");
        check(FeedbackApi.validText("字".repeat(500)), "500 characters allowed");
        check(!FeedbackApi.validText("字".repeat(501)), "501 characters refused");
        check(TextPolicy.codePointLength("😀a") == 2, "code points are counted");
        check(!FeedbackApi.validText("a\u0000b"), "control characters refused");
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "feedback response ids accept strings");
        check(JsonPolicy.strictString(7) == null,
            "feedback response ids reject numbers instead of coercing them");

        // 诊断字段只保留白名单键，值截到 256 字节。
        Map<String, String> raw = new LinkedHashMap<>();
        raw.put("device", "Pixel 8");
        raw.put("os", "Android 15\n");
        raw.put("typed_text", "secret");
        raw.put("skin", "水".repeat(200));
        Map<String, String> clean = FeedbackApi.filterDiagnostics(raw);
        check(!clean.containsKey("typed_text"), "unknown keys dropped");
        check("Android 15".equals(clean.get("os")), "controls stripped");
        check(clean.get("skin").getBytes(java.nio.charset.StandardCharsets.UTF_8).length <= 256, "values clipped to 256 bytes");
        check(FeedbackApi.DIAGNOSTIC_KEYS.containsAll(clean.keySet()), "only allow-listed keys");

        // Rust 不重新编码截图：页面交过去的每一张都要在它的限制以内（迁移差异 25）。
        check(FeedbackApi.fitsRust(FeedbackApi.MAX_SCREENSHOT_BYTES, 1600, 1200), "a re-encoded screenshot fits");
        check(!FeedbackApi.fitsRust(FeedbackApi.MAX_SCREENSHOT_BYTES + 1, 1600, 1200), "over 1 MiB");
        check(FeedbackApi.fitsRust(1024, 8192, 1024) && !FeedbackApi.fitsRust(1024, 8193, 10)
            && !FeedbackApi.fitsRust(1024, 10, 8193), "each edge is at most 8192");
        check(FeedbackApi.fitsRust(1024, 4096, 2048) && !FeedbackApi.fitsRust(1024, 4096, 2049),
            "at most 8388608 pixels");
        check(!FeedbackApi.fitsRust(0, 10, 10) && !FeedbackApi.fitsRust(1024, 0, 10), "unreadable sizes are refused");

        check(DownloadLinkApi.PLATFORMS.contains("harmony-pc"), "platform list");

        // 云剪贴板页按状态和错误码分辨失败；Rust 的码换成同一套。
        check(CloudClipboardApi.rustFailure("account_unauthorized").signedOut(), "no session reads as signed out");
        check("session_changed".equals(CloudClipboardApi.rustFailure("account_cancelled").code), "a replaced login");
        check(CloudClipboardApi.rustFailure("account_network").network(), "a request that never got a reply");
        check("clipboard_sync_disabled".equals(CloudClipboardApi.rustFailure("clipboard_sync_disabled").code),
            "a driver code is kept as is");

        check(CloudClipboardApi.validRetention(0) && CloudClipboardApi.validRetention(30) && !CloudClipboardApi.validRetention(3), "retention values");
        List<CloudClipboardApi.Item> ordered = CloudClipboardApi.ordered(List.of(
            new CloudClipboardApi.Item("a", "1", "t", false, ""),
            new CloudClipboardApi.Item("b", "2", "t", true, ""),
            new CloudClipboardApi.Item("c", "3", "t", false, "")));
        check(ordered.get(0).id().equals("b") && ordered.get(1).id().equals("a"), "pinned first, otherwise stable");
        System.out.println("FeedbackApiSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
