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
        ClipboardHistory.Item omitted = new ClipboardHistory.Item("未选择的合成记录", 1L, false);
        List<ClipboardHistory.Item> history = List.of(older, omitted, newer);
        List<ClipboardHistory.Item> selected = ClipboardExportFormat.select(history,
            List.of(newer.text(), older.text(), older.text(), "已不存在的合成记录"));
        check(selected.equals(List.of(older, newer)), "选择按历史顺序筛选，重复和失效选择不增加条目");
        check(ClipboardExportFormat.select(history, null).equals(history), "未指定选择时导出全部");
        check(ClipboardExportFormat.select(history, List.of()).isEmpty(), "空选择不导出历史");
        check(history.size() == 3, "筛选不修改共享历史快照");
        check(ClipboardExportFormat.format(selected, ZoneId.of("Asia/Shanghai")).equals(export),
            "选中项仍使用完整时间、链接、原文与置顶标记格式");
    }
}
