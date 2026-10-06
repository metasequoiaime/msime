package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.TextView;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowInsetsCompat;

/**
 * 设计里的 toast：底部导航栏上方一枚深色胶囊（浅色模式 `#1C1C1E` 底白字，深色模式 `#F2F2F2` 底 `#111` 字，取 `colorSurfaceInverse` / `colorOnSurfaceInverse`），停 1.6 秒后淡出。
 *
 * <p>画在当前 Activity 的内容层里而不是用系统 Toast：系统 Toast 的样子由系统决定，从 Android 12 起还会给它加上应用图标。同一时间只有一枚，新的直接替换旧的。拿不到 Activity 时（例如从 Application 的 Context 调用）退回系统 Toast，消息不会丢。读屏通过 live region 读出内容。
 */
public final class MsToast {
    private static final Object TAG = new Object();

    private MsToast() {}

    public static void show(Context context, CharSequence text) {
        Activity activity = Ui.activityOf(context);
        ViewGroup content = activity == null ? null : activity.findViewById(android.R.id.content);
        if (!(content instanceof FrameLayout frame)) {
            android.widget.Toast.makeText(context.getApplicationContext(), text,
                android.widget.Toast.LENGTH_SHORT).show();
            return;
        }
        View previous = frame.findViewWithTag(TAG);
        if (previous != null) {
            previous.animate().cancel();
            frame.removeView(previous);
        }

        TextView toast = new TextView(activity);
        toast.setTag(TAG);
        toast.setText(text);
        toast.setGravity(Gravity.CENTER);
        toast.setMaxLines(3);
        Ui.style(toast, Ui.TEXT_TOAST, 400,
            Ui.color(activity, com.google.android.material.R.attr.colorOnSurfaceInverse));
        toast.setBackground(Ui.pill(Ui.color(activity, com.google.android.material.R.attr.colorSurfaceInverse)));
        toast.setPadding(Ui.dp(activity, 20), Ui.dp(activity, 10), Ui.dp(activity, 20), Ui.dp(activity, 10));
        toast.setElevation(Ui.dp(activity, 6));
        toast.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);

        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT,
            Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        int side = Ui.dp(activity, 32);
        params.leftMargin = side;
        params.rightMargin = side;
        params.bottomMargin = Ui.dp(activity, Ui.TOAST_BOTTOM) + navigationInset(frame);
        toast.setAlpha(0f);
        frame.addView(toast, params);
        toast.animate().alpha(1f).setDuration(Ui.FADE_MILLIS).withEndAction(() ->
            toast.animate().alpha(0f).setStartDelay(Ui.TOAST_MILLIS).setDuration(Ui.FADE_MILLIS)
                .withEndAction(() -> frame.removeView(toast)).start()).start();
    }

    /** 手势区或三键导航栏的高度：内容层在边到边模式下伸到它下面，toast 要留在它上方。 */
    private static int navigationInset(View view) {
        WindowInsetsCompat insets = ViewCompat.getRootWindowInsets(view);
        return insets == null ? 0 : insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom;
    }
}
