package app.msime.android;

import java.time.DateTimeException;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneId;

/** Formats cloud clipboard timestamps without allowing hostile dates to escape into the UI. */
public final class CloudClipboardTimePolicy {
    private CloudClipboardTimePolicy() {}

    /** 「刚刚、N 分钟前、N 小时前、昨天」；无法表示的时间为空。 */
    public static String relative(String timestamp) {
        if (timestamp == null || timestamp.isEmpty()) return "";
        final Instant then;
        try {
            then = Instant.parse(timestamp);
            Instant now = Instant.now();
            long minutes = Duration.between(then, now).toMinutes();
            if (minutes < 1) return "刚刚";
            if (minutes < 60) return minutes + " 分钟前";
            ZoneId zone = ZoneId.systemDefault();
            LocalDate day = then.atZone(zone).toLocalDate();
            LocalDate today = now.atZone(zone).toLocalDate();
            if (day.equals(today)) return (minutes / 60) + " 小时前";
            if (day.equals(today.minusDays(1))) return "昨天";
            if (day.getYear() == today.getYear()) return day.getMonthValue() + " 月 " + day.getDayOfMonth() + " 日";
            return day.getYear() + " 年 " + day.getMonthValue() + " 月 " + day.getDayOfMonth() + " 日";
        } catch (DateTimeException | ArithmeticException malformed) {
            return "";
        }
    }
}
