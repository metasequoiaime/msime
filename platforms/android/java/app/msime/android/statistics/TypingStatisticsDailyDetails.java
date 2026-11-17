package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** 共享 Rust 返回的按日明细；这里只解析字段和排版，不重新筛选日期或计算指标。 */
public final class TypingStatisticsDailyDetails {
    private TypingStatisticsDailyDetails() {}

    public record Row(String day, long total, long han, long latin, long number,
            long punctuation, long other, Long activeMs, Double speed) {}

    /** 缺少新字段时保留不可用状态，不能把旧宿主答复当作空历史。 */
    public static List<Row> from(JSONArray array) {
        if (array == null) return null;
        List<Row> result = new ArrayList<>(array.length());
        for (int index = 0; index < array.length(); index++) {
            JSONObject row = array.optJSONObject(index);
            if (row == null) return null;
            Long active = JsonPolicy.strictLong(row.opt("active_ms"));
            double speed = NumberPolicy.strictDouble(row.opt("speed"), Double.NaN);
            result.add(new Row(row.optString("day", ""), count(row, "total"),
                count(row, "han"), count(row, "latin"), count(row, "number"),
                count(row, "punctuation"), count(row, "other"),
                active == null || active < 0 ? null : active,
                Double.isFinite(speed) && speed >= 0 ? speed : null));
        }
        return List.copyOf(result);
    }

    public static List<String> cells(Row row) {
        return List.of(row.day(), NumberPolicy.grouped(row.total()), NumberPolicy.grouped(row.han()),
            NumberPolicy.grouped(row.latin()), NumberPolicy.grouped(row.number()),
            NumberPolicy.grouped(row.punctuation()), NumberPolicy.grouped(row.other()),
            duration(row.activeMs()), TypingStatisticsSummary.whole(row.speed()));
    }

    /** 时长与桌面明细一致：不足一分钟显示秒数，其余向下取整到分钟，未知测量仍显示「—」。 */
    public static String duration(Long milliseconds) {
        if (milliseconds == null) return "—";
        if (milliseconds <= 0) return "0分";
        long minutes = milliseconds / 60_000;
        if (minutes == 0) return Math.max(1L, Math.round(milliseconds / 1_000d)) + "秒";
        long hours = minutes / 60;
        if (hours == 0) return minutes + "分";
        return hours + "小时" + (minutes % 60 == 0 ? "" : minutes % 60 + "分");
    }

    private static long count(JSONObject object, String key) {
        return TypingStatisticsSummary.strictCount(object.opt(key));
    }
}
