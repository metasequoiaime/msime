package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.graphics.Rect;
import android.os.Build;
import android.os.Bundle;
import android.view.View;
import android.widget.LinearLayout;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.concurrent.ExecutorService;

/** 安装产品的留白设置读写与真实键盘内边距，覆盖最小系统和底栏计入目标的行为，以及「键盘调整」的抬高预览和收窄后外框两侧空出的宽度。 */
public final class BottomPaddingHeightDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
    @Override public void onStart() {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            Path directory = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                service = type.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                attach.setAccessible(true);
                attach.invoke(service, getTargetContext());
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                Object frame = field(type, "imeFrame").get(service);
                LinearLayout keyboard = (LinearLayout) field(frame.getClass(), "keyboard").get(frame);
                Object bottom = field(type, "imeBottomBar").get(service);
                Method apply = method(bottom.getClass(), "apply", LinearLayout.class);
                ((Rect) field(bottom.getClass(), "insets").get(bottom)).set(1, 2, 3, 4);
                field(bottom.getClass(), "navigationKnown").setBoolean(bottom, Build.VERSION.SDK_INT >= 31);
                field(bottom.getClass(), "navigationBottom").setInt(bottom, 0);
                field(bottom.getClass(), "gestureBottom").setInt(bottom, 0);
                Class<?> settings = loader.loadClass("app.msime.android.AndroidLocalSettings");
                Class<?> snapshot = loader.loadClass("app.msime.android.AndroidLocalSettings$Snapshot");
                Method update = settings.getMethod("update", Path.class, Map.class);
                Method load = settings.getMethod("load", Path.class);
                directory = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "padding-smoke-");
                Path file = directory.resolve("android-settings.json");
                float density = getTargetContext().getResources().getDisplayMetrics().density;
                // 同时覆盖小 inset 和厂商异常的大 inset；仅底栏显示时按已知导航栏收窄。
                for (int insetBottom : new int[] {4, 144}) {
                    ((Rect) field(bottom.getClass(), "insets").get(bottom)).bottom = insetBottom;
                    for (boolean barEnabled : new boolean[] {false, true}) {
                        boolean barShown = barEnabled && Build.VERSION.SDK_INT >= 31;
                        for (int height : new int[] {0, 40, 80, 160}) {
                            update.invoke(null, file, Map.of("platform.android.bottom_bar", barEnabled,
                                "platform.android.bottom_padding", true, "platform.android.bottom_padding_height", height));
                            Object saved = load.invoke(null, file);
                            check(((Integer) snapshot.getMethod("integer", String.class).invoke(saved,
                                "platform.android.bottom_padding_height")) == height, "custom padding did not persist");
                            field(type, "localSettings").set(service, saved);
                            apply.invoke(bottom, keyboard);
                            int expected = (barShown ? 0 : insetBottom) + Math.round((barShown ? Math.max(40, height) : height) * density);
                            check(Math.abs(keyboard.getPaddingBottom() - expected) <= 1,
                                "padding target differs: " + height + "/" + barShown + "/" + insetBottom);
                            View bar = (View) field(bottom.getClass(), "bar").get(bottom);
                            check((bar.getVisibility() == View.VISIBLE) == barShown, "bottom bar visibility changed");
                            if (barShown) check(((android.widget.FrameLayout.LayoutParams) bar.getLayoutParams())
                                .bottomMargin == 0, "abnormal inset still lifts bottom bar");
                            check(keyboard.getPaddingLeft() == 1 && keyboard.getPaddingTop() == 2
                                && keyboard.getPaddingRight() == 3, "bottom clamp changed another edge");
                        }
                    }
                }
                // 调高留白后关闭开关，仍保留保存的高度，但实际不抬高键区。
                ((Rect) field(bottom.getClass(), "insets").get(bottom)).bottom = 4;
                Object disabled = update.invoke(null, file, Map.of("platform.android.bottom_bar", false,
                    "platform.android.bottom_padding", false));
                field(type, "localSettings").set(service, disabled);
                apply.invoke(bottom, keyboard);
                check(keyboard.getPaddingBottom() == 4, "disabled padding still lifts keys");
                // 「键盘调整」打开时按预览的抬高垫，不读存着的值；关上后回到存着的值。
                field(type, "keyboardAdjustActive").setBoolean(service, true);
                field(type, "keyboardAdjustLiftDp").setInt(service, 32);
                apply.invoke(bottom, keyboard);
                check(Math.abs(keyboard.getPaddingBottom() - (4 + Math.round(32 * density))) <= 1,
                    "keyboard adjust did not preview its lift");
                field(type, "keyboardAdjustActive").setBoolean(service, false);
                apply.invoke(bottom, keyboard);
                check(keyboard.getPaddingBottom() == 4, "closing keyboard adjust kept the previewed lift");
                // 存着 80% 贴左时，外框右边空出可用宽度的 20%（键盘不窄于 240 dp）；竖屏手机上才生效。
                android.content.res.Configuration configuration = getTargetContext().getResources().getConfiguration();
                if (configuration.orientation != android.content.res.Configuration.ORIENTATION_LANDSCAPE
                        || configuration.smallestScreenWidthDp >= 600) {
                    Object narrow = update.invoke(null, file, Map.of("platform.android.keyboard_width_percent", 80,
                        "platform.android.keyboard_position_x", 0));
                    field(type, "localSettings").set(service, narrow);
                    int available = 2000;
                    int[] insets = (int[]) method(type, "keyboardHorizontalInsets", int.class).invoke(service, available);
                    int width = Math.max(1600, Math.round(240 * density));
                    check(insets[0] == 0 && insets[1] == available - width, "narrowed keyboard insets differ: "
                        + insets[0] + "/" + insets[1]);
                }
            } catch (Exception | AssertionError error) {
                Throwable cause = error;
                while (cause instanceof java.lang.reflect.InvocationTargetException && cause.getCause() != null)
                    cause = cause.getCause();
                failure[0] = cause;
            } finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 清理测试工作线程。 */ }
                    }
                }
                if (directory != null) {
                    try (var paths = Files.walk(directory)) {
                        for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toArray(Path[]::new))
                            Files.deleteIfExists(path);
                    } catch (java.io.IOException error) { if (failure[0] == null) failure[0] = error; }
                }
            }
        });
        Throwable error = failure[0];
        Bundle result = new Bundle();
        result.putString("stream", error == null
            ? "MSIME_DEVICE_SMOKE_PASSED: persisted 0/40/80/160dp padding, 4/144px inset clamp, bottom bar margin and disable, keyboard adjust lift preview and width insets\n"
            : "MSIME_DEVICE_SMOKE_FAILED: bottom padding (" + error.getClass().getSimpleName() + ")"
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
        finish(error == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters); method.setAccessible(true); return method;
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
