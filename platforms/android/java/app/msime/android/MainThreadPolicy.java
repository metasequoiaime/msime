package app.msime.android;

import android.os.Handler;
import android.os.Looper;

/** Android 宿主共用的主线程消息队列入口。 */
public final class MainThreadPolicy {
    private MainThreadPolicy() {}

    /** 创建绑定到 Android 主线程 Looper 的 Handler。 */
    public static Handler mainHandler() {
        return new Handler(Looper.getMainLooper());
    }
}
