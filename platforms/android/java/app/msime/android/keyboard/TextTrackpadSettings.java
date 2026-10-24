package app.msime.android;

import org.json.JSONObject;

/** 共享层已规范化的设置快照；缺失或读取失败时不启用功能，不在 Java 另存默认值。 */
public record TextTrackpadSettings(boolean enabled, boolean holdToOpen, int horizontalDp, int verticalDp,
        boolean tapSelection, boolean haptics, boolean trail, boolean magnifier) {
    public static TextTrackpadSettings from(JSONObject preferences) {
        if (preferences == null) return null;
        JSONObject value = preferences.optJSONObject("android_text_trackpad");
        if (value == null) return null;
        for (String key : new String[] {"enabled", "hold_to_open", "tap_selection", "haptics", "trail", "magnifier"})
            if (!(value.opt(key) instanceof Boolean)) return null;
        int horizontal = NumberPolicy.strictInt(value, "horizontal_dp", -1);
        int vertical = NumberPolicy.strictInt(value, "vertical_dp", -1);
        if (horizontal <= 0 || vertical <= 0) return null;
        return new TextTrackpadSettings(Boolean.TRUE.equals(value.opt("enabled")), Boolean.TRUE.equals(value.opt("hold_to_open")),
            horizontal, vertical, Boolean.TRUE.equals(value.opt("tap_selection")), Boolean.TRUE.equals(value.opt("haptics")),
            Boolean.TRUE.equals(value.opt("trail")), Boolean.TRUE.equals(value.opt("magnifier")));
    }
}
