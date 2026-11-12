package app.msime.android;

import android.content.SharedPreferences;

/** 按剪贴板模式分别记住最近一次补读身份，避免主模式的去重状态挡住访客模式。 */
public final class ClipboardCaptureState {
    private static final String MAIN_KEY = "handled_clip_main";
    private static final String VISITOR_KEY = "handled_clip_visitor";
    private static final String LEGACY_KEY = "handled_clip";

    private ClipboardCaptureState() {}

    public static String handledIdentity(SharedPreferences preferences,
            ClipboardModePolicy.Mode mode) {
        if (preferences == null || mode == null) return null;
        String value = preferences.getString(key(mode), null);
        return value != null || mode != ClipboardModePolicy.Mode.MAIN
            ? value : preferences.getString(LEGACY_KEY, null);
    }

    public static void remember(SharedPreferences preferences,
            ClipboardModePolicy.Mode mode, String identity) {
        if (preferences == null || mode == null || identity == null) return;
        preferences.edit().putString(key(mode), identity).apply();
    }

    private static String key(ClipboardModePolicy.Mode mode) {
        return mode == ClipboardModePolicy.Mode.VISITOR ? VISITOR_KEY : MAIN_KEY;
    }
}
