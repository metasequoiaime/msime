import app.msime.android.TypingStatisticsDailyDetails;
import app.msime.android.TypingStatisticsDailyDetails.Row;
import app.msime.android.TypingStatisticsSummary;
import java.util.List;

/** 明细排版保留未知与零、千分位及各列的共享顺序。 */
public final class TypingStatisticsDailyDetailsSmoke {
    public static void main(String[] args) {
        Row legacy = new Row("2026-11-16", 12_345, 10_000, 1_000, 300, 40, 1_005, null, null);
        check(TypingStatisticsDailyDetails.cells(legacy).equals(List.of(
            "2026-11-16", "12,345", "10,000", "1,000", "300", "40", "1,005", "—", "—")),
            "历史记录不能伪装成零时长和零速度");
        Row measured = new Row("2026-11-17", 0, 0, 0, 0, 0, 0, 0L, 0d);
        check(TypingStatisticsDailyDetails.cells(measured).equals(List.of(
            "2026-11-17", "0", "0", "0", "0", "0", "0", "0分", "0")),
            "已测量的零不能变成未知");
        check("59分".equals(TypingStatisticsDailyDetails.duration(3_599_999L)), "时长按分钟向下取整");
        check("1秒".equals(TypingStatisticsDailyDetails.duration(400L)), "不足一秒仍显示已测量的时长");
        check("12秒".equals(TypingStatisticsDailyDetails.duration(12_400L)), "不足一分钟显示秒数");
        check("60秒".equals(TypingStatisticsDailyDetails.duration(59_999L)), "分钟之前按秒四舍五入");
        check("1小时".equals(TypingStatisticsDailyDetails.duration(3_600_000L)), "整小时");
        check("1小时1分".equals(TypingStatisticsDailyDetails.duration(3_660_000L)), "小时加分钟");
        var summary = new TypingStatisticsSummary(null, null, null, List.of(), List.of(legacy, measured));
        check(summary.dailyDetails().equals(List.of(legacy, measured)), "不在 Java 重新排序或过滤记录日");
        check(new TypingStatisticsSummary(null, null, null, List.of()).dailyDetails() == null,
            "旧摘要保留不可用状态");
        System.out.println("TypingStatisticsDailyDetailsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
