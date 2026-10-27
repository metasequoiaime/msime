package app.msime.android;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.Locale;

/** Android 导出文本的阅读格式；只读共享历史快照，不改原文或顺序。 */
public final class ClipboardExportFormat {
    private static final DateTimeFormatter DATE_TIME =
        DateTimeFormatter.ofPattern("yyyy年M月d日 EEEE HH:mm", Locale.CHINA);

    private ClipboardExportFormat() { }

    public static String format(List<ClipboardHistory.Item> entries, ZoneId zone) {
        StringBuilder result = new StringBuilder("水杉剪贴板历史，共 " + entries.size() + " 条\n");
        result.append("官网：https://msime.app/\n")
            .append("项目：https://github.com/metasequoiaime/msime\n");
        for (int index = 0; index < entries.size(); index++) {
            ClipboardHistory.Item item = entries.get(index);
            result.append("\n========== ").append(index + 1);
            if (item.pinned()) result.append(" · 已置顶");
            result.append(" ==========\n记录时间：");
            if (item.timestamp() <= 0) result.append("时间未知");
            else result.append(DATE_TIME.format(Instant.ofEpochMilli(item.timestamp()).atZone(zone)));
            result.append("\n来源 App：系统未提供\n包名：系统未提供\n正文：\n")
                .append(item.text()).append('\n');
        }
        return result.toString();
    }
}
