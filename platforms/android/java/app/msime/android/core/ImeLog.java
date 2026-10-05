package app.msime.android;

import android.util.Log;

/**
 * 键盘进程的日志：按 `developer_options.log_level` 过滤后交给 android.util.Log。默认门槛是警告，`error` 只留错误，`info` 再放出信息，`debug` 全部输出；未知取值按默认处理。
 *
 * <p>这里只记键盘自身的状态与失败原因，从不记输入内容、候选或按键字符。
 */
final class ImeLog {
    static final String TAG = "MSIME";
    private static volatile int threshold = Log.WARN;

    private ImeLog() {}

    /** 偏好值到 android.util.Log 级别。 */
    static int levelFor(String preference) {
        if (preference == null) return Log.WARN;
        return switch (preference) {
            case "error" -> Log.ERROR;
            case "info" -> Log.INFO;
            case "debug" -> Log.DEBUG;
            default -> Log.WARN;
        };
    }

    /** 按偏好里的 `developer_options.log_level` 更新门槛。 */
    static void applyLevel(String preference) {
        threshold = levelFor(preference);
    }

    static int threshold() {
        return threshold;
    }

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
