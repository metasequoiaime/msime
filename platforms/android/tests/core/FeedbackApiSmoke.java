import app.msime.android.CloudClipboardApi;
import app.msime.android.DownloadLinkApi;
import app.msime.android.FeedbackApi;
import app.msime.android.CloudApi;
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

        byte[] png = {(byte) 0x89, 'P', 'N', 'G', 0x0d, 0x0a, 0x1a, 0x0a};
        byte[] jpeg = {(byte) 0xff, (byte) 0xd8, (byte) 0xff, 0x00};
        check(FeedbackApi.validScreenshot(new FeedbackApi.Screenshot("image/png", png)), "png accepted");
        check(FeedbackApi.validScreenshot(new FeedbackApi.Screenshot("image/jpeg", jpeg)), "jpeg accepted");
        check(!FeedbackApi.validScreenshot(new FeedbackApi.Screenshot("image/png", jpeg)), "type must match bytes");
        check(!FeedbackApi.validScreenshot(new FeedbackApi.Screenshot("image/jpeg", new byte[FeedbackApi.MAX_SCREENSHOT_BYTES + 1])), "size limit");

        check(DownloadLinkApi.noVerifiedEmail(new CloudApi.Failure(409, "no_verified_email", "", 0)), "409 no_verified_email");
        check(!DownloadLinkApi.noVerifiedEmail(new CloudApi.Failure(429, "rate_limit_exceeded", "", 0)), "rate limit is not missing email");
        check(DownloadLinkApi.PLATFORMS.contains("harmony-pc"), "platform list");

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
