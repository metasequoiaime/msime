package app.msime.android;

import android.util.Log;

/**
 * 键盘进程的日志（扩展点）：按级别过滤后交给 android.util.Log。现在的门槛等同 Log.w，低于警告的日志不输出；以后由 developer_options.log_level 决定门槛。
 */
final class ImeLog {
    static final String TAG = "MSIME";
    private static volatile int threshold = Log.WARN;

    private ImeLog() {}

    static boolean enabled(int level) {
        return level >= threshold;
    }

    static void d(String message) {
        if (enabled(Log.DEBUG)) Log.d(TAG, message);
    }

    static void i(String message) {
        if (enabled(Log.INFO)) Log.i(TAG, message);
    }

    static void w(String message) {
        if (enabled(Log.WARN)) Log.w(TAG, message);
    }

    static void w(String message, Throwable error) {
        if (enabled(Log.WARN)) Log.w(TAG, message, error);
    }

    static void e(String message, Throwable error) {
        if (enabled(Log.ERROR)) Log.e(TAG, message, error);
    }
}
