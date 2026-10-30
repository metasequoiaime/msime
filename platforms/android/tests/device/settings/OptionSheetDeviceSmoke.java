package app.msime.android.test;

import android.app.Activity;
import android.app.Dialog;
import android.content.Context;
import android.content.Intent;
import android.graphics.Point;
import android.os.Build;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.widget.TextView;
import java.lang.reflect.Field;

/** 原生设置宿主中的长选项面板，取消行无需拖动即可点击。 */
public final class OptionSheetDeviceSmoke extends DeviceSmoke {
    private boolean animationOnly;
    private String keyFeedback;

    @Override public void onCreate(android.os.Bundle arguments) {
        animationOnly = arguments != null && "true".equals(arguments.getString("animation-selection-only"));
        keyFeedback = arguments == null ? null : arguments.getString("key-feedback");
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return keyFeedback == null ? "animation selection follows saved value, option sheet footer stays above system navigation and settings sheets open at full height"
            : "sound-only".equals(keyFeedback) ? "default key sound pack plays from the app even with system sound effects unloaded"
            : "nine-key preview lifecycle and immediate feedback preferences";
    }

    @SuppressWarnings("deprecation")
    @Override protected void runChecks() throws Exception {
        if (keyFeedback != null) {
            new KeyboardFeedbackDeviceChecks(this).run(keyFeedback);
            return;
        }
        new KeyboardAnimationDeviceChecks(this).run();
        if (animationOnly) return;
        Activity activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.home.OptionSheet");
        Dialog[] dialog = new Dialog[1];
        stage = "show synthetic nine-option sheet";
        runCheckedOnMain(() -> {
            try {
                Object sheet = type.getConstructor(Context.class, CharSequence.class, CharSequence.class)
                    .newInstance(activity, "合成选择列表", null);
                for (int index = 0; index < 9; index++)
                    type.getMethod("option", CharSequence.class, boolean.class, Runnable.class)
                        .invoke(sheet, "合成选项 " + index, index == 0, (Runnable) () -> {});
                Field field = type.getDeclaredField("dialog");
                field.setAccessible(true);
                dialog[0] = (Dialog) field.get(sheet);
                type.getMethod("show").invoke(sheet);
            } catch (ReflectiveOperationException error) { throw new AssertionError("Synthetic sheet setup failed"); }
        });
        try {
            SystemClock.sleep(800);
            stage = "footer fully above navigation without dragging";
            runCheckedOnMain(() -> {
                View decor = dialog[0].getWindow().getDecorView();
                TextView cancel = findCancel(decor);
                if (cancel == null) throw new AssertionError("Cancel row missing");
                View row = cancel;
                while (!row.isClickable() && row.getParent() instanceof View parent) row = parent;
                Point display = new Point();
                activity.getWindowManager().getDefaultDisplay().getRealSize(display);
                WindowInsets insets = decor.getRootWindowInsets();
                int bottom = Build.VERSION.SDK_INT >= 30
                    ? insets.getInsets(WindowInsets.Type.navigationBars()).bottom : insets.getStableInsetBottom();
                int[] position = new int[2];
                row.getLocationOnScreen(position);
                if (position[1] < 0 || position[1] + row.getHeight() > display.y - bottom)
                    throw new AssertionError("Cancel row is covered by system navigation");
                row.performClick();
                if (dialog[0].isShowing()) throw new AssertionError("Cancel did not close sheet");
            });
        } finally {
            runOnMainSync(() -> { dialog[0].dismiss(); activity.finish(); });
        }
        checkSettingsSheetOpensFullHeight();
    }

    /** 比折叠高度高、但放得下一屏的设置面板打开就是整个内容的高度，末尾的操作行不用拖就在导航栏上方（按钮顺序面板的「保存」曾被折叠高度藏到屏幕下面）。 */
    @SuppressWarnings("deprecation")
    private void checkSettingsSheetOpensFullHeight() throws Exception {
        Activity activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.home.SettingsSheet");
        Dialog[] dialog = new Dialog[1];
        TextView[] last = new TextView[1];
        stage = "show synthetic settings sheet taller than the collapsed height";
        runCheckedOnMain(() -> {
            try {
                Object sheet = type.getConstructor(Context.class, String.class, String.class)
                    .newInstance(activity, "合成设置面板", null);
                Point display = new Point();
                activity.getWindowManager().getDefaultDisplay().getRealSize(display);
                // 13 行各占屏高的 1/20：内容高过 Material 的折叠高度（屏高减去屏宽的 9/16），又和标题一起放得下一屏。
                for (int index = 0; index < 13; index++) {
                    TextView row = new TextView(activity);
                    row.setText(index == 12 ? "合成末行" : "合成行 " + index);
                    row.setMinHeight(display.y / 20);
                    type.getMethod("add", View.class).invoke(sheet, row);
                    last[0] = row;
                }
                Field field = type.getDeclaredField("dialog");
                field.setAccessible(true);
                dialog[0] = (Dialog) field.get(sheet);
                type.getMethod("show").invoke(sheet);
            } catch (ReflectiveOperationException error) { throw new AssertionError("Synthetic settings sheet setup failed"); }
        });
        try {
            SystemClock.sleep(800);
            stage = "settings sheet last row fully above navigation without dragging";
            runCheckedOnMain(() -> {
                View decor = dialog[0].getWindow().getDecorView();
                Point display = new Point();
                activity.getWindowManager().getDefaultDisplay().getRealSize(display);
                WindowInsets insets = decor.getRootWindowInsets();
                int bottom = Build.VERSION.SDK_INT >= 30
                    ? insets.getInsets(WindowInsets.Type.navigationBars()).bottom : insets.getStableInsetBottom();
                int[] position = new int[2];
                last[0].getLocationOnScreen(position);
                if (!last[0].isShown() || position[1] < 0 || position[1] + last[0].getHeight() > display.y - bottom)
                    throw new AssertionError("Settings sheet opened at the collapsed height; its last row is below the navigation bar");
            });
        } finally {
            runOnMainSync(() -> { dialog[0].dismiss(); activity.finish(); });
        }
    }

    // 主线程断言回传测试线程，避免 fixture 失败被误报成产品崩溃。
    private void runCheckedOnMain(Runnable action) {
        AssertionError[] failure = new AssertionError[1];
        runOnMainSync(() -> {
            try { action.run(); }
            catch (AssertionError error) { failure[0] = error; }
            catch (RuntimeException error) { failure[0] = new AssertionError("Synthetic sheet UI failed"); }
        });
        if (failure[0] != null) throw failure[0];
    }

    private static TextView findCancel(View view) {
        if (view instanceof TextView text && "取消".contentEquals(text.getText())) return text;
        if (view instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++) {
                TextView result = findCancel(group.getChildAt(index));
                if (result != null) return result;
            }
        }
        return null;
    }
}
