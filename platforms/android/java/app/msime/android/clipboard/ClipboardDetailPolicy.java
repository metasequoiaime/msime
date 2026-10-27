package app.msime.android;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.Locale;

/** 剪贴板详情的时间和来源文案；未知来源不能用当前编辑器冒充复制来源。 */
public final class ClipboardDetailPolicy {
    private static final DateTimeFormatter DATE_TIME =
        DateTimeFormatter.ofPattern("yyyy年M月d日 EEEE HH:mm", Locale.CHINA);

    private ClipboardDetailPolicy() { }

    public static String recordedAt(long timestampMs, ZoneId zone) {
        if (timestampMs <= 0) return "时间未知";
        return DATE_TIME.format(Instant.ofEpochMilli(timestampMs).atZone(zone));
    }

    public static String sourceLabel(String source) {
        return source == null || source.isBlank() ? "系统未提供" : source;
    }
}
