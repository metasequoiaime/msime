package app.msime.android;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.Locale;

/** 剪贴板详情的时间和来源文案；未知来源不能用当前编辑器冒充复制来源。 */
public final class ClipboardDetailPolicy {
    private static final DateTimeFormatter DATE_TIME =
        DateTimeFormatter.ofPattern("yyyy年M月d日 EEEE HH:mm", Locale.CHINA);

    /** 详情从哪一级打开：卡片左侧的详情键在历史列表里，「详情」按钮在长按出来的全文预览里；返回时回到同一级。 */
    public enum Origin { LIST, PREVIEW }

    private ClipboardDetailPolicy() { }

    /** 从全文预览进入的详情，返回时只在记录仍在当前列表里时重开预览；记录已被删掉或移出当前分类时只回到列表，不对已不存在的记录提供插入和修改。 */
    public static boolean reopensPreview(Origin origin, boolean recordListed) {
        return origin == Origin.PREVIEW && recordListed;
    }

    public static String backDescription(Origin origin) {
        return origin == Origin.PREVIEW ? "返回剪贴板全文预览" : "返回剪贴板历史列表";
    }

    public static String recordedAt(long timestampMs, ZoneId zone) {
        if (timestampMs <= 0) return "时间未知";
        return DATE_TIME.format(Instant.ofEpochMilli(timestampMs).atZone(zone));
    }

    public static String sourceLabel(String source) {
        return source == null || source.isBlank() ? "系统未提供" : source;
    }
}
