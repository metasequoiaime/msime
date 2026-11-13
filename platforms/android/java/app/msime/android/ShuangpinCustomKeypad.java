package app.msime.android;

import java.util.List;

/** 普通 26 键双拼自定义表的设置文字和轻量输入校验；表本身由 host-api 校验。 */
public final class ShuangpinCustomKeypad {
    private ShuangpinCustomKeypad() {}

    public static final List<String> BUILTIN_PROFILES =
        List.of("xiaohe", "ziranma", "shoudao", "microsoft", "smart_abc");
    public static final String DEFAULT_PROFILE = "xiaohe";
    public static final String CUSTOM_PROFILE = "custom";

    public static String normalizedProfile(String value) {
        return value != null && (CUSTOM_PROFILE.equals(value) || BUILTIN_PROFILES.contains(value))
            ? value : DEFAULT_PROFILE;
    }

    public static String profileTitle(String profile) {
        return switch (normalizedProfile(profile)) {
            case "ziranma" -> "自然码双拼";
            case "shoudao" -> "首道双拼";
            case "microsoft" -> "微软双拼";
            case "smart_abc" -> "智能 ABC 双拼";
            case CUSTOM_PROFILE -> "自定义";
            default -> "小鹤双拼";
        };
    }

    public static String partTitle(String part) {
        return switch (part == null ? "" : part) {
            case "initials" -> "声母";
            case "finals" -> "韵母";
            case "zero_initials" -> "零声母音节";
            default -> "";
        };
    }

    public static String unitLabel(String unit) {
        return unit != null && unit.startsWith("v") ? "ü" + unit.substring(1) : unit == null ? "" : unit;
    }

    public static boolean validKey(String part, String key) {
        if (key == null) return false;
        int length = "zero_initials".equals(part) ? 2 : 1;
        if (key.length() != length) return false;
        for (int index = 0; index < key.length(); index++) {
            char value = Character.toLowerCase(key.charAt(index));
            boolean letter = value >= 'a' && value <= 'z';
            boolean semicolon = value == ';' && ("finals".equals(part)
                || ("zero_initials".equals(part) && index == 1));
            if (!letter && !semicolon) return false;
        }
        return true;
    }

    public static String problemMessage(String kind, String part, String unit, String key) {
        String place = partTitle(part) + (unit == null || unit.isEmpty() ? "" : "「" + unitLabel(unit) + "」");
        return switch (kind == null ? "" : kind) {
            case "unknown_unit" -> place + "不是这张表认识的单位";
            case "duplicate_unit" -> place + "写了两次";
            case "missing_unit" -> place + "还没有放到字母键上";
            case "invalid_key" -> place + "的键「" + (key == null ? "" : key) + "」不对："
                + ("zero_initials".equals(part) ? "零声母要两位字母" : "声母和韵母各是一位字母或分号");
            case "initial_key_taken" -> place + "占用了单字母声母的键「" + (key == null ? "" : key) + "」";
            case "ambiguous_code" -> "编码「" + (key == null ? "" : key) + "」对应了多个音节";
            default -> "这张表不能用";
        };
    }
}
