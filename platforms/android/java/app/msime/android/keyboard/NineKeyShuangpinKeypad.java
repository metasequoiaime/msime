package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 九键双拼（#6572）的数字键面和设置页的文字。每个数字键上放哪些声母、韵母由 host-api 的 `msime_client_nine_key_shuangpin_layout` 按当前的表给出（{@link NativeClient#nineKeyShuangpinLayout}），这里只决定怎么印、怎么读，不另存一份键位。
 *
 * <p>纯 Java，不碰 `org.json`：check-host 的 JVM 冒烟只有 android.jar 里的 `org.json` 桩，解析在调用方做。
 */
public final class NineKeyShuangpinKeypad {
    private NineKeyShuangpinKeypad() {}

    /** 一个数字键的键面：键（`0`–`9`）、按表的顺序排的声母和韵母（ü 开头的韵母已写成 ü），以及零声母编码是否从这个键开始。 */
    public record Face(char key, List<String> initials, List<String> finals, boolean zeroInitial) {
        public Face {
            initials = List.copyOf(initials);
            finals = List.copyOf(finals);
        }
    }

    /** 内置方案的偏好值，按设置页里的顺序。 */
    public static final List<String> BUILTIN_PROFILES = List.of("migala3", "migala2", "gushi2_low");
    /** 偏好 `nine_key_shuangpin_profile` 的缺省值，与 client-core 一致。 */
    public static final String DEFAULT_PROFILE = "migala3";
    public static final String CUSTOM_PROFILE = "custom";

    /** 只认四个取值，其余（缺省、未知值）按缺省的米旮旯 3，与共享偏好的缺省一致。 */
    public static String normalizedProfile(String value) {
        if (value != null && (CUSTOM_PROFILE.equals(value) || BUILTIN_PROFILES.contains(value))) return value;
        return DEFAULT_PROFILE;
    }

    /** 方案名。 */
    public static String profileTitle(String profile) {
        return switch (normalizedProfile(profile)) {
            case "migala2" -> "米旮旯 2";
            case "gushi2_low" -> "辜氏 2 低重码";
            case CUSTOM_PROFILE -> "自定义";
            default -> "米旮旯 3";
        };
    }

    /** `faces` 里这个键的键面；表没有用到这个键时为 null。 */
    public static Face face(List<Face> faces, char key) {
        if (faces == null) return null;
        for (Face face : faces) {
            if (face.key() == key) return face;
        }
        return null;
    }

    /** 键面文字：第一行是声母（零声母编码从这个键开始时前面加「零」），第二行是韵母；只有一边时只有一行。 */
    public static String label(Face face) {
        String initials = initialsLine(face);
        String finals = String.join(" ", face.finals());
        if (initials.isEmpty()) return finals;
        if (finals.isEmpty()) return initials;
        return initials + "\n" + finals;
    }

    private static String initialsLine(Face face) {
        List<String> parts = new ArrayList<>(face.initials().size() + 1);
        if (face.zeroInitial()) parts.add("零");
        parts.addAll(face.initials());
        return String.join(" ", parts);
    }

    /** 读屏文字：「2，声母 b c 和零声母，韵母 a ai iang」。 */
    public static String description(Face face) {
        StringBuilder text = new StringBuilder().append(face.key());
        if (!face.initials().isEmpty() || face.zeroInitial()) {
            text.append("，声母");
            if (!face.initials().isEmpty()) text.append(' ').append(String.join(" ", face.initials()));
            if (face.zeroInitial()) text.append(face.initials().isEmpty() ? " 零声母" : " 和零声母");
        }
        if (!face.finals().isEmpty()) text.append("，韵母 ").append(String.join(" ", face.finals()));
        return text.toString();
    }

    /** 表的一部分的中文名。 */
    public static String partTitle(String part) {
        return switch (part == null ? "" : part) {
            case "initials" -> "声母";
            case "finals" -> "韵母";
            case "zero_initials" -> "零声母";
            default -> "";
        };
    }

    /** 表里一个单位怎么显示：ü 写作 v 存，显示成 ü。 */
    public static String unitLabel(String unit) {
        if (unit == null) return "";
        return unit.startsWith("v") ? "ü" + unit.substring(1) : unit;
    }

    /** 编辑框里的键合不合规矩：声母和韵母是一个数字，零声母是两个数字。合不合法最终由 Engine 判定，这里只挡住打错的输入。 */
    public static boolean validKey(String part, String key) {
        if (key == null) return false;
        int length = "zero_initials".equals(part) ? 2 : 1;
        if (key.length() != length) return false;
        for (int index = 0; index < key.length(); index++) {
            char digit = key.charAt(index);
            if (digit < '0' || digit > '9') return false;
        }
        return true;
    }

    /**
     * 用户表的问题写成给人看的一句话，带出错的位置；问题的种类与 host-api 的 `ShuangpinCustomProblem` 相同（九键双拼只有这四种）。
     */
    public static String problemMessage(String kind, String part, String unit, String key) {
        String place = partTitle(part) + (unit == null || unit.isEmpty() ? "" : "「" + unitLabel(unit) + "」");
        return switch (kind == null ? "" : kind) {
            case "unknown_unit" -> place + "不是这张表认识的单位";
            case "duplicate_unit" -> place + "写了两次";
            case "missing_unit" -> place + "还没有放到数字键上";
            case "invalid_key" -> place + "的键「" + (key == null ? "" : key) + "」不对："
                + ("zero_initials".equals(part) ? "零声母要两位数字" : "声母和韵母各是一位数字");
            default -> "这张表不能用";
        };
    }
}
