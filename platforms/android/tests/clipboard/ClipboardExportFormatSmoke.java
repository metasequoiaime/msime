import app.msime.android.ClipboardExportFormat;
import app.msime.android.ClipboardHistory;
import java.time.ZoneId;
import java.util.List;

public final class ClipboardExportFormatSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        ClipboardHistory.Item older = new ClipboardHistory.Item("  合成甲\r\n尾行  ",
            1793043960000L, true);
        ClipboardHistory.Item newer = new ClipboardHistory.Item("合成乙", 0L, false);
        String export = ClipboardExportFormat.format(List.of(older, newer), ZoneId.of("Asia/Shanghai"));
        check(export.contains("水杉剪贴板历史，共 2 条"), "条数");
        check(export.contains("https://msime.app/"), "官网");
        check(export.contains("https://github.com/metasequoiaime/msime"), "项目");
        check(export.contains("2026年10月27日 星期二 03:46"), "完整记录时间");
        check(export.contains("来源 App：系统未提供\n包名：系统未提供"), "未知来源如实标注");
        check(export.contains("========== 1 · 已置顶 =========="), "固定标记");
        check(export.indexOf("  合成甲\r\n尾行  \n") < export.indexOf("合成乙\n"), "原文与顺序");
        check(export.contains("记录时间：时间未知"), "旧记录缺时间");
        check(ClipboardExportFormat.format(List.of(), ZoneId.of("UTC"))
            .contains("共 0 条"), "空历史");
    }
}
