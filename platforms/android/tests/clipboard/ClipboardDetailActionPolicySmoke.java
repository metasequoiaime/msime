import app.msime.android.AndroidLocalSettings;
import app.msime.android.ClipboardDetailActionPolicy;

/** #11288：详情底部操作行的左/右对齐选项与本机设置契约。 */
public final class ClipboardDetailActionPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        AndroidLocalSettings.Snapshot defaults = AndroidLocalSettings.defaults();
        check(ClipboardDetailActionPolicy.LEFT.equals(
                defaults.choice(AndroidLocalSettings.CLIPBOARD_DETAIL_ACTION_ALIGNMENT)),
            "详情操作默认保持左对齐");
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(
            AndroidLocalSettings.CLIPBOARD_DETAIL_ACTION_ALIGNMENT);
        check(spec.kind == AndroidLocalSettings.Spec.Kind.CHOICE && !spec.synced,
            "详情操作对齐只保存在本机");
        check(ClipboardDetailActionPolicy.RIGHT.equals(spec.accept(ClipboardDetailActionPolicy.RIGHT)),
            "右对齐是可保存选项");
        check(ClipboardDetailActionPolicy.LEFT.equals(spec.accept(ClipboardDetailActionPolicy.LEFT)),
            "左对齐是可保存选项");
        check(spec.accept("center") == null && spec.accept(Boolean.TRUE) == null,
            "未知对齐值被拒绝");
        check(ClipboardDetailActionPolicy.isRight(ClipboardDetailActionPolicy.RIGHT)
                && !ClipboardDetailActionPolicy.isRight(ClipboardDetailActionPolicy.LEFT)
                && !ClipboardDetailActionPolicy.isRight("center"),
            "损坏值不会误判为右对齐");
        check("右对齐".equals(ClipboardDetailActionPolicy.label(ClipboardDetailActionPolicy.RIGHT))
                && "左对齐".equals(ClipboardDetailActionPolicy.label("center")),
            "对齐选项显示文案稳定");
    }
}
