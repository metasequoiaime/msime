package app.msime.android;

import java.util.List;
import java.util.regex.Pattern;

/**
 * 候选选中文字色（`custom_theme.candidate_colors.selected_text`，#10740）在 Android 上的规则。
 *
 * <p>触屏候选条照旧按键盘配色画，自定义主题的其余候选取色器不作用于它；只有全局主题是自定义、且设了这个取色器时，首选候选的字换成它。用了候选皮肤包时，共享解析器已经把它放在选中文字色的最上面，由 {@link CandidateAppearance#fromResolved} 取用。与鸿蒙手机候选条的 `GlobalTheme.touchSelectedText` 同一规则。
 *
 * <p>设这个颜色时文档还要跟着改什么，与共享设置页的 `updateCandidateColor`、Windows 的 `candidate_color_change` 相同：选了颜色就选上自定义主题；原来不是自定义主题时，把当时的主题留作底，并清掉两个槽位的皮肤包；交还给主题时别的都不动。值的合法性由共享 Rust 的偏好校验把关，这里只决定怎么画。
 */
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

    /** 触屏候选条首选候选的字色：全局主题是自定义、取色器合法时是不透明的 ARGB，否则为 null（照旧用键盘配色）。 */
    public static Integer stripColour(String globalTheme, String picked) {
        String value = "custom".equals(globalTheme) ? normalized(picked) : null;
        return value == null ? null : 0xff000000 | Integer.parseInt(value.substring(1), 16);
    }

    /** 设置页行尾显示的当前值。 */
    public static String label(String picked) {
        String value = normalized(picked);
        if (value == null) return "跟随主题";
        for (Preset preset : PRESETS) if (preset.value().equals(value)) return preset.label();
        return value;
    }

    /** 设一个候选颜色或交还给主题时，除了那一项还要改什么。 */
    public record Change(boolean selectCustom, boolean resetSkins, String base) {}

    /** `current` 是当前全局主题，`clearing` 为真表示交还给主题。原来不是自定义主题时，当时的主题成为底（`native` 不能作底，按 `system`）。 */
    public static Change change(String current, boolean clearing) {
        String theme = current == null || current.isEmpty() ? "system" : current;
        boolean reset = !clearing && !"custom".equals(theme);
        return new Change(!clearing, reset, reset ? ("native".equals(theme) ? "system" : theme) : null);
    }
}
