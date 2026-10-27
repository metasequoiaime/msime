import app.msime.android.ClipboardDetailPolicy;
import java.time.ZoneId;

public final class ClipboardDetailPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        ZoneId zone = ZoneId.of("Asia/Shanghai");
        check("2026年10月27日 星期二 03:46".equals(
            ClipboardDetailPolicy.recordedAt(1793043960000L, zone)), "完整本地记录时间");
        check("时间未知".equals(ClipboardDetailPolicy.recordedAt(0L, zone)), "旧记录缺时间");
        check("系统未提供".equals(ClipboardDetailPolicy.sourceLabel(null)), "来源不可用");
        check("Via".equals(ClipboardDetailPolicy.sourceLabel("Via")), "有来源时显示原值");
    }
}
