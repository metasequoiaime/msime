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
        check(ClipboardDetailPolicy.reopensPreview(ClipboardDetailPolicy.Origin.PREVIEW, true),
            "从全文预览进入的详情返回预览");
        check(!ClipboardDetailPolicy.reopensPreview(ClipboardDetailPolicy.Origin.PREVIEW, false),
            "记录已不在列表时不重开预览");
        check(!ClipboardDetailPolicy.reopensPreview(ClipboardDetailPolicy.Origin.LIST, true),
            "从列表进入的详情只回列表");
        check("返回剪贴板全文预览".equals(ClipboardDetailPolicy.backDescription(ClipboardDetailPolicy.Origin.PREVIEW))
            && "返回剪贴板历史列表".equals(ClipboardDetailPolicy.backDescription(ClipboardDetailPolicy.Origin.LIST)),
            "返回键说明指向实际返回的那一级");
    }
}
