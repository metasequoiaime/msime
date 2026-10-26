package app.msime.android;

import android.content.Context;
import java.io.File;

/** 访客剪贴板模式的跨进程持久状态；输入法和设置页共用同一个 UID 偏好文件。 */
public final class ClipboardModeStore {
    private ClipboardModeStore() {}

    public static ClipboardModePolicy.Mode load(Context context) {
        if (context == null) return ClipboardModePolicy.Mode.MAIN;
        boolean visitor = context.getSharedPreferences(
            ClipboardModePolicy.PREFERENCES, Context.MODE_PRIVATE)
            .getBoolean(ClipboardModePolicy.VISITOR_KEY, false);
        return ClipboardModePolicy.fromVisitorFlag(visitor);
    }

    public static void save(Context context, ClipboardModePolicy.Mode mode) {
        if (context == null || mode == null) return;
        context.getSharedPreferences(ClipboardModePolicy.PREFERENCES, Context.MODE_PRIVATE)
            .edit().putBoolean(ClipboardModePolicy.VISITOR_KEY,
                mode == ClipboardModePolicy.Mode.VISITOR).apply();
    }

    public static File directory(Context context) {
        if (context == null) return null;
        return ClipboardModePolicy.directory(context.getFilesDir(), load(context));
    }
}
