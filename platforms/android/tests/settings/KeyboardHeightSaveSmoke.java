import app.msime.android.KeyboardHeightSave;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/** 键盘里调整布局后的落盘顺序：高度先写、与共享偏好文档的保存互不依赖。 */
public final class KeyboardHeightSaveSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 共享文档冲突或写入失败（响应为 null）时，高度照样已经写下，而且是先写的。
        List<String> order = new ArrayList<>();
        KeyboardHeightSave.Result conflicted = KeyboardHeightSave.run(
            () -> order.add("height"), () -> {
                order.add("shared");
                return null;
            });
        check(order.equals(List.of("height", "shared")), "height is written before the shared document");
        check(conflicted.heightSaved(), "a failed shared save must not drop the height");
        check(conflicted.sharedAttempted() && conflicted.sharedResponse() == null, "shared failure is reported");

        // 高度写不成也不妨碍共享文档。
        order.clear();
        KeyboardHeightSave.Result heightFailed = KeyboardHeightSave.run(
            () -> { throw new IOException("synthetic"); }, () -> {
                order.add("shared");
                return "{\"ok\":true}";
            });
        check(!heightFailed.heightSaved(), "height failure is reported");
        check(order.equals(List.of("shared")), "shared save still runs");
        check("{\"ok\":true}".equals(heightFailed.sharedResponse()), "shared response is kept");

        // 只调了高度：不碰共享文档，免得一次无谓的 revision 冲突。
        order.clear();
        KeyboardHeightSave.Result heightOnly = KeyboardHeightSave.run(() -> order.add("height"), null);
        check(order.equals(List.of("height")), "only the height is written");
        check(heightOnly.heightSaved() && !heightOnly.sharedAttempted(), "no shared save attempted");

        // 高度没变：只写共享文档，结果仍算高度已保存。
        KeyboardHeightSave.Result sharedOnly = KeyboardHeightSave.run(null, () -> "{\"ok\":true}");
        check(sharedOnly.heightSaved() && sharedOnly.sharedAttempted(), "shared only");

        // 不合规的高度（AndroidLocalSettings 拒绝时抛 IllegalArgumentException）同样只算高度没写成。
        KeyboardHeightSave.Result rejected = KeyboardHeightSave.run(
            () -> { throw new IllegalArgumentException("synthetic"); }, null);
        check(!rejected.heightSaved(), "rejected height is reported");
        System.out.println("Android keyboard height save: height first and independent of the shared document passed");
    }
}
