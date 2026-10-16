package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.ExecutorService;

/** 安装产品在真实 Android 文字排版下，完整显示所有多字形组合，不被窄格裁切。 */
public final class EmojiCombinationLayoutDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
    @Override public void onStart() {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                service = type.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                attach.setAccessible(true);
                attach.invoke(service, getTargetContext());
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                String document;
                try (var input = getTargetContext().getAssets().open("emoji-combinations.txt")) {
                    document = read(input);
                }
                try (var input = getTargetContext().getAssets().open("emoji-combinations-NOTICE.txt")) {
                    String notice = read(input);
                    check(notice.contains("c99f9b7c451b7b77d379dab7d55fdbfb93b04b12")
                        && notice.contains("Copyright (c) 2026") && notice.contains("All rights reserved."),
                        "upstream attribution was not packaged");
                }
                List<?> notices = (List<?>) method(loader.loadClass("app.msime.android.home.AboutPage"),
                    "listNotices", android.content.res.AssetManager.class).invoke(null, getTargetContext().getAssets());
                check(notices.contains("emoji-combinations-NOTICE.txt"), "about page does not expose BSD notice");
                Class<?> catalog = loader.loadClass("app.msime.android.EmojiCombinationCatalog");
                List<?> items = (List<?>) catalog.getMethod("decode", String.class).invoke(null, document);
                check(items.size() == 165, "installed list is incomplete");
                field(type, "emojiItems").set(service, items);
                View panel = (View) field(type, "emojiPanel").get(service);
                panel.setVisibility(View.VISIBLE);
                ViewGroup grid = (ViewGroup) field(type, "emojiGrid").get(service);
                Object panels = field(type, "imePanels").get(service);
                Method render = method(panels.getClass(), "renderEmojiGrid");
                float density = getTargetContext().getResources().getDisplayMetrics().density;
                for (int widthDp : new int[] {411, 240, 320, 411}) {
                    int width = Math.round(widthDp * density), height = Math.round(280 * density);
                    layout(panel, width, height);
                    render.invoke(panels);
                    layout(panel, width, height);
                    int count = 0;
                    for (int rowIndex = 0; rowIndex < grid.getChildCount(); rowIndex++) {
                        ViewGroup row = (ViewGroup) grid.getChildAt(rowIndex);
                        for (int cellIndex = 0; cellIndex < row.getChildCount(); cellIndex++) {
                            Button cell = (Button) row.getChildAt(cellIndex);
                            check(cell.getWidth() > 0 && cell.getLeft() >= 0 && cell.getRight() <= grid.getWidth(),
                                "combination cell escapes grid at " + widthDp);
                            check(cell.getPaint().measureText(cell.getText().toString())
                                    <= cell.getWidth() - cell.getPaddingLeft() - cell.getPaddingRight() + 1,
                                "combination text clipped at " + widthDp + " index " + count);
                            check(cell.isClickable(), "combination cell lost click action");
                            count++;
                        }
                    }
                    check(count == 165, "multi-glyph combinations were filtered out");
                }
            } catch (Exception | AssertionError error) {
                Throwable cause = error;
                while (cause instanceof java.lang.reflect.InvocationTargetException && cause.getCause() != null)
                    cause = cause.getCause();
                failure[0] = cause;
            }
            finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 清理测试工作线程。 */ }
                    }
                }
            }
        });
        Throwable error = failure[0];
        Bundle result = new Bundle();
        result.putString("stream", error == null
            ? "MSIME_DEVICE_SMOKE_PASSED: all 165 combinations, 240/320/411dp, resize, text fit, clickable cells and packaged BSD notice\n"
            : "MSIME_DEVICE_SMOKE_FAILED: combination layout (" + error.getClass().getSimpleName() + ")"
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
        finish(error == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }
    private static String read(java.io.InputStream input) throws java.io.IOException {
        java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
        byte[] buffer = new byte[4096];
        for (int count; (count = input.read(buffer)) != -1;) output.write(buffer, 0, count);
        return new String(output.toByteArray(), StandardCharsets.UTF_8);
    }
    private static void layout(View view, int width, int height) {
        view.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
        view.layout(0, 0, width, height);
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
