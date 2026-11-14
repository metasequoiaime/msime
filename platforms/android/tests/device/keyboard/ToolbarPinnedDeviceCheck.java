package app.msime.android.test;

import android.view.ViewGroup;
import android.widget.Button;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;

/** #11427：对真实按钮检查常驻位、复制预览往返和缩放后的尺寸，复用现有设备入口。 */
final class ToolbarPinnedDeviceCheck {
    static void run(Class<?> type, Object service, ClassLoader loader) throws Exception {
        Field settings = field(type, "localSettings");
        Object original = settings.get(service);
        Constructor<?> snapshot = loader.loadClass("app.msime.android.AndroidLocalSettings$Snapshot")
            .getDeclaredConstructor(Map.class);
        snapshot.setAccessible(true);
        Object toolbar = field(type, "imeToolbar").get(service);
        Method order = method(toolbar.getClass(), "applyShortcutOrder");
        Method recent = method(toolbar.getClass(), "updateRecentClipRow", String.class, boolean.class);
        Method resize = method(toolbar.getClass(), "applyShortcutSize", int.class, boolean.class, int.class, int.class);
        ViewGroup normal = (ViewGroup) field(type, "shortcutToolbar").get(service);
        ViewGroup middle = (ViewGroup) field(type, "shortcutBar").get(service);
        ViewGroup clip = (ViewGroup) field(type, "recentClipRow").get(service);
        Method render = method(type, "render");
        try {
            for (String id : new String[] {"dismiss", "text_edit", "voice"}) {
                settings.set(service, snapshot.newInstance(Map.of("platform.android.toolbar_pinned", id)));
                recent.invoke(toolbar, null, false);
                order.invoke(toolbar);
                render.invoke(service);
                String name = id.equals("dismiss") ? "dismissShortcutButton"
                    : id.equals("voice") ? "voiceShortcutButton" : "textEditShortcutButton";
                Button pinned = (Button) field(type, name).get(service);
                check(pinned.getParent() == normal, id + " did not move to fixed edge");
                check(normal.getChildAt(normal.getChildCount() - 1) == pinned, id + " is not rightmost");
                check(middle.indexOfChild(pinned) == -1, id + " is duplicated in scrolling tools");
                check(pinned.hasOnClickListeners(), id + " lost its action");
                check(pinned.getVisibility() == android.view.View.VISIBLE, id + " pin obeyed the ordinary hidden switch");
                for (int percent : new int[] {90, 150}) {
                    resize.invoke(toolbar, percent, true, 100, 100);
                    int width = pinned.getLayoutParams().width;
                    int height = pinned.getLayoutParams().height;
                    for (int repeat = 0; repeat < 2; repeat++) {
                        recent.invoke(toolbar, "synthetic clipboard sample", false);
                        ViewGroup slot = (ViewGroup) pinned.getParent();
                        check(slot != normal && slot.getParent() == clip, id + " was hidden by copy preview");
                        check(clip.getChildAt(clip.getChildCount() - 1) == slot, id + " lost rightmost position");
                        check(slot.getLayoutParams().width == width, id + " slot did not follow toolbar size");
                        check(pinned.getLayoutParams().width == width && pinned.getLayoutParams().height == height,
                            id + " size changed while opening copy preview");
                        check(pinned.hasOnClickListeners(), id + " lost its action in copy preview");
                        recent.invoke(toolbar, null, false);
                        check(pinned.getParent() == normal, id + " did not return after copy preview closed");
                        check(pinned.getLayoutParams().width == width && pinned.getLayoutParams().height == height,
                            id + " size changed after closing copy preview");
                    }
                }
            }
        } finally {
            settings.set(service, original);
            recent.invoke(toolbar, null, false);
            order.invoke(toolbar);
            resize.invoke(toolbar, 100, false, 100, 100);
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters);
        method.setAccessible(true);
        return method;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
