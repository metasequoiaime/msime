package app.msime.android;

import android.app.Activity;
import android.app.Dialog;
import android.content.Context;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.widget.PopupWindow;

/**
 * 把「防止截屏」（{@link AndroidLocalSettings#SCREEN_CAPTURE_PROTECTION}）落到窗口上：开着时给窗口加 {@code FLAG_SECURE}，系统截屏、录屏、投屏和最近任务的缩略图里这块窗口显示为黑色。
 *
 * <p>{@code FLAG_SECURE} 只作用于设了它的那一个窗口，所以设置应用的每个 Activity、Activity 上弹出的对话框和底部面板、输入法窗口以及输入法自己开的 {@link PopupWindow} 都要各自设一次。设置文件由设置应用写、:ime 进程读，两边经 {@link AndroidLocalSettings#load} 读到同一个值。
 */
public final class ScreenCaptureProtection {
    private ScreenCaptureProtection() {}

    /** 此刻是否开着。读的是本地设置的缓存，文件没变时不重新解析。 */
    public static boolean enabled(Context context) {
        return AndroidLocalSettings.load(context).bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION);
    }

    /** 按当前设置给 Activity 的窗口加上或去掉保护；在 {@code onCreate} 里调用时窗口还没显示，第一帧就受保护。 */
    public static void apply(Activity activity) {
        apply(activity.getWindow(), enabled(activity));
    }

    /** 给一个窗口加上或去掉 {@code FLAG_SECURE}；标志已经是想要的值时不动窗口，避免多一次窗口属性更新。 */
    public static void apply(Window window, boolean protect) {
        if (window == null) return;
        int flags = window.getAttributes().flags;
        if (ScreenCaptureProtectionPolicy.windowFlags(flags, protect) == flags) return;
        if (protect) window.addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
    }

    /** 对话框和底部面板是独立窗口，不继承 Activity 的标志；在 {@code show()} 之前调用。 */
    public static <T extends Dialog> T protect(T dialog) {
        apply(dialog.getWindow(), enabled(dialog.getContext()));
        return dialog;
    }

    /** 按当前设置保护对话框后显示它。 */
    public static void show(Dialog dialog) {
        protect(dialog).show();
    }

    /**
     * 输入法里的 {@link PopupWindow} 没有设置窗口标志的公开接口，只能在 {@code showAtLocation} / {@code showAsDropDown} 之后改它根视图的窗口参数。紧接着显示调用时，窗口的第一次布局还没执行，第一帧就带着这个标志。
     */
    public static void apply(PopupWindow popup) {
        View content = popup.getContentView();
        if (content == null) return;
        View root = content.getRootView();
        if (!(root.getLayoutParams() instanceof WindowManager.LayoutParams params)) return;
        int next = ScreenCaptureProtectionPolicy.windowFlags(params.flags, enabled(content.getContext()));
        // 根视图还没挂到 WindowManager 上（显示失败）时没有窗口可改；addView 会同步给它挂上 ViewRootImpl，所以看 parent，不看要等第一次布局才为真的 isAttachedToWindow。
        if (next == params.flags || root.getParent() == null) return;
        params.flags = next;
        WindowManager manager = content.getContext().getSystemService(WindowManager.class);
        if (manager != null) manager.updateViewLayout(root, params);
    }
}
