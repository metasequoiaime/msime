package app.msime.android;

import java.util.List;
import java.util.regex.Pattern;

/** 候选选中文字色的设置展示与解析结果适配；主题切换和触屏生效规则由共享 Rust 持有。 */
public final class CandidateSelectedTextPolicy {
    /** `custom_theme.candidate_colors` 里的槽位名。 */
    public static final String SLOT = "selected_text";
    private static final Pattern HEX = Pattern.compile("#[0-9A-Fa-f]{6}");

    /** 设置页给的几种常用颜色，最后还有「自定义…」输入 `#RRGGBB`。 */
    public record Preset(String label, String value) {}
    public static final List<Preset> PRESETS = List.of(
        new Preset("黑色", "#000000"), new Preset("白色", "#FFFFFF"), new Preset("水杉绿", "#2C7A4B"),
        new Preset("蓝色", "#1A73E8"), new Preset("紫色", "#7B3FBF"), new Preset("红色", "#D93025"),
        new Preset("橙色", "#E8710A"));

    private CandidateSelectedTextPolicy() {}

    /** 合法的 `#RRGGBB` 规范成大写，其余为 null。 */
    public static String normalized(String value) {
        return value != null && HEX.matcher(value).matches() ? value.toUpperCase(java.util.Locale.ROOT) : null;
    }

    /** 把共享解析器给出的可选 RGB 转成 Android 颜色，不读取主题或偏好。 */
    public static Integer resolvedColour(String colour) {
        String value = normalized(colour);
        return value == null ? null : 0xff000000 | Integer.parseInt(value.substring(1), 16);
    }

    /** 设置页行尾显示的当前值。 */
    public static String label(String picked) {
        String value = normalized(picked);
        if (value == null) return "跟随主题";
        for (Preset preset : PRESETS) if (preset.value().equals(value)) return preset.label();
        return value;
    }

}
