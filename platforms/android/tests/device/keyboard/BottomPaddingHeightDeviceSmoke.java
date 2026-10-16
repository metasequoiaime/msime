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

/** 安装产品的留白设置读写与真实键盘内边距，覆盖最小系统和底栏计入目标的行为。 */
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
                        int expected = 4 + Math.round((barShown ? Math.max(40, height) : height) * density);
                        check(Math.abs(keyboard.getPaddingBottom() - expected) <= 1,
                            "padding target differs: " + height + "/" + barShown);
                        View bar = (View) field(bottom.getClass(), "bar").get(bottom);
                        check((bar.getVisibility() == View.VISIBLE) == barShown, "bottom bar visibility changed");
                    }
                }
                // 调高留白后关闭开关，仍保留保存的高度，但实际不抬高键区。
                Object disabled = update.invoke(null, file, Map.of("platform.android.bottom_bar", false,
                    "platform.android.bottom_padding", false));
                field(type, "localSettings").set(service, disabled);
                apply.invoke(bottom, keyboard);
                check(keyboard.getPaddingBottom() == 4, "disabled padding still lifts keys");
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
            ? "MSIME_DEVICE_SMOKE_PASSED: persisted 0/40/80/160dp padding, real IME insets, bottom bar accounting and disable\n"
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
