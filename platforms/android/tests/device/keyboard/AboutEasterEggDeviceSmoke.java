package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import java.io.File;
import java.io.FileOutputStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.ExecutorService;

/** 直接操作产品品牌键与关于磁贴，验证隐藏入口、返回和画布重复绘制。 */
public final class AboutEasterEggDeviceSmoke extends Instrumentation {
    private boolean sceneOnly;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        sceneOnly = arguments != null && "true".equals(arguments.getString("scene-only"));
        start();
    }

    @Override public void onStart() {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                if (!sceneOnly) {
                    Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                    service = type.getConstructor().newInstance();
                    Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                    attach.setAccessible(true);
                    attach.invoke(service, getTargetContext());
                    type.getMethod("onCreate").invoke(service);
                    type.getMethod("onCreateInputView").invoke(service);
                    Button brand = (Button) field(type, "moreButton").get(service);
                    check(brand.performClick(), "brand key did not open tools");
                    ViewGroup panel = (ViewGroup) field(type, "moreToolsPanel").get(service);
                    Button about = button(panel, "关于");
                    check(about != null && about.performLongClick(), "keyboard About tile has no easter-egg long press");
                    check(findDescription(panel, "水杉妹坐在树下刷手机") != null, "long press did not show scene inside IME");
                    Object function = field(type, "imeFunctionPanel").get(service);
                    method(function.getClass(), "renderMoreTools").invoke(function);
                    check(findDescription(panel, "水杉妹坐在树下刷手机") != null, "ordinary render removed easter egg");
                    Button back = button(panel, "返回工具");
                    check(back != null && back.performClick(), "easter egg has no return action");
                    check(button(panel, "关于") != null, "return did not restore tools");
                    check(button(panel, "关于").performLongClick(), "second long press failed");
                    method(type, "closeMoreTools").invoke(service);
                    brand.performClick();
                    check(button(panel, "关于") != null && findDescription(panel, "水杉妹坐在树下刷手机") == null,
                        "closing tools retained hidden scene");
                    check(button(panel, "关于").performLongClick(), "long press before input-view recreation failed");
                    type.getMethod("onCreateInputView").invoke(service);
                    brand = (Button) field(type, "moreButton").get(service);
                    panel = (ViewGroup) field(type, "moreToolsPanel").get(service);
                    brand.performClick();
                    check(button(panel, "关于") != null && findDescription(panel, "水杉妹坐在树下刷手机") == null,
                        "input-view recreation retained hidden scene");
                    check(button(panel, "关于").performLongClick(), "long press after input-view recreation failed");
                }
                Class<?> scene;
                try { scene = loader.loadClass("app.msime.android.AboutEasterEgg$Scene"); }
                catch (ClassNotFoundException oldPackage) {
                    scene = loader.loadClass("app.msime.android.home.AboutEasterEgg$Scene");
                }
                Constructor<?> constructor = scene.getDeclaredConstructor(Context.class, String.class, int.class);
                constructor.setAccessible(true);
                String message = "2026年6月6日住进你的手机中，截止到此时此刻你我已经相处 143 天了。";
                int theme = getTargetContext().getResources().getIdentifier("Theme.MSIME", "style", "app.msime.android");
                check(theme != 0, "product theme is missing");
                for (float fontScale : new float[] {1, 2}) for (int width : new int[] {240, 320, 600}) {
                    Configuration configuration = new Configuration(getTargetContext().getResources().getConfiguration());
                    configuration.fontScale = fontScale;
                    Context themed = new android.view.ContextThemeWrapper(
                        getTargetContext().createConfigurationContext(configuration), theme);
                    View view = (View) constructor.newInstance(themed, message, 0xfffafafa);
                    float density = view.getResources().getDisplayMetrics().density;
                    int pixels = Math.round(width * density);
                    view.measure(View.MeasureSpec.makeMeasureSpec(pixels, View.MeasureSpec.EXACTLY),
                        View.MeasureSpec.makeMeasureSpec(Math.round(1000 * density), View.MeasureSpec.AT_MOST));
                    view.layout(0, 0, pixels, view.getMeasuredHeight());
                    Bitmap first = draw(view), second = draw(view);
                    try {
                        try (FileOutputStream output = new FileOutputStream(new File(getTargetContext().getCacheDir(),
                                "about-easter-egg-" + width + "-" + (int) fontScale + ".png"))) {
                            check(first.compress(Bitmap.CompressFormat.PNG, 100, output), "scene screenshot encoding failed");
                        }
                        check(first.sameAs(second), "scene pixels changed merely by redrawing");
                        check(view.getMeasuredHeight() < Math.round(900 * density), "scene consumed dialog's entire height");
                        check(first.getPixel(0, 0) != 0xffff00ff, "scene lacks a valid background colour");
                    } finally { first.recycle(); second.recycle(); }
                }
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 释放隔离服务。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 释放测试工作线程。 */ }
                    }
                }
            }
        });
        Bundle result = new Bundle();
        Throwable error = failure[0];
        result.putString("stream", error == null
            ? "MSIME_DEVICE_SMOKE_PASSED: keyboard About long press, return, close/reopen, input-view recreation, 240/320/600dp and 1x/2x font scene redraw\n"
            : "MSIME_DEVICE_SMOKE_FAILED: about easter egg (" + error.getClass().getSimpleName() + ") "
                + error.getMessage() + "\n");
        finish(error == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static Bitmap draw(View view) {
        Bitmap bitmap = Bitmap.createBitmap(view.getWidth(), view.getHeight(), Bitmap.Config.ARGB_8888);
        bitmap.eraseColor(android.graphics.Color.MAGENTA);
        view.draw(new Canvas(bitmap));
        return bitmap;
    }

    private static Button button(View view, String label) {
        if (view instanceof Button button && label.contentEquals(button.getText())) return button;
        if (view instanceof ViewGroup group) for (int i = 0; i < group.getChildCount(); i++) {
            Button result = button(group.getChildAt(i), label);
            if (result != null) return result;
        }
        return null;
    }

    private static View findDescription(View view, String text) {
        if (view.getContentDescription() != null && view.getContentDescription().toString().contains(text)) return view;
        if (view instanceof ViewGroup group) for (int i = 0; i < group.getChildCount(); i++) {
            View result = findDescription(group.getChildAt(i), text);
            if (result != null) return result;
        }
        return null;
    }

    private static Field field(Class<?> type, String name) throws ReflectiveOperationException {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }

    private static Method method(Class<?> type, String name) throws ReflectiveOperationException {
        Method method = type.getDeclaredMethod(name); method.setAccessible(true); return method;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
