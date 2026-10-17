import app.msime.android.ClipboardEditKey;
import java.nio.charset.StandardCharsets;
import javax.crypto.spec.SecretKeySpec;

/** 剪贴板编辑令牌使用密钥计算，输入变化会改变标签且不泄露文字。 */
public final class ClipboardEditKeySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        SecretKeySpec first = new SecretKeySpec("synthetic clipboard edit key one".getBytes(StandardCharsets.UTF_8), "HmacSHA256");
        SecretKeySpec second = new SecretKeySpec("synthetic clipboard edit key two".getBytes(StandardCharsets.UTF_8), "HmacSHA256");
        String text = "合成剪贴板记录🙂";
        String key = ClipboardEditKey.editKey(first, 1_760_000_000_000L, text);
        check(key.equals(ClipboardEditKey.editKey(first, 1_760_000_000_000L, text)), "the edit token is stable");
        check(!key.contains(text), "the edit token carries no clipboard text");
        check(!key.equals(ClipboardEditKey.editKey(first, 1_760_000_000_001L, text)), "the timestamp is authenticated");
        check(!key.equals(ClipboardEditKey.editKey(first, 1_760_000_000_000L, text + ".")), "the text is authenticated");
        check(!key.equals(ClipboardEditKey.editKey(first, 1_760_000_000_000L, "合成剪贴板记录")), "the text length is authenticated");
        check(!key.equals(ClipboardEditKey.editKey(second, 1_760_000_000_000L, text)), "the installation key is required");
        check(key.matches("[0-9]+:[0-9a-f]{64}:[0-9]+"), "the edit token is bounded and opaque");
        System.out.println("ClipboardEditKeySmoke ok");
    }
}
