package app.msime.android;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;

/** Android Context 包装链的共享读取策略。 */
public final class ContextPolicy {
    private ContextPolicy() {}

    /** 沿 ContextWrapper 链查找 Activity；链上没有 Activity 时返回 null。 */
    public static Activity activity(Context context) {
        Context current = context;
        while (current instanceof ContextWrapper wrapper) {
            if (current instanceof Activity activity) return activity;
            current = wrapper.getBaseContext();
        }
        return null;
    }
}
