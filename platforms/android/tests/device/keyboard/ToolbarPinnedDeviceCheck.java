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
            settings.set(service, snapshot.newInstance(Map.of("platform.android.toolbar_pinned", "text_edit")));
            recent.invoke(toolbar, null, false);
            checkForcedDismiss(type, service, normal, middle, render);
        } finally {
            settings.set(service, original);
            recent.invoke(toolbar, null, false);
            order.invoke(toolbar);
            resize.invoke(toolbar, 100, false, 100, 100);
        }
    }

    /** 常驻位换成文本编辑、普通收起入口关着时，收起键仍是候选条模式唯一的展开入口，也是「显示方式：隐藏」时面板临时露出那一行的返回键：两种状态都要在常驻按钮右边露出来，状态结束后回到中间滚动区并隐藏。 */
    private static void checkForcedDismiss(Class<?> type, Object service, ViewGroup normal, ViewGroup middle,
            Method render) throws Exception {
        Button dismiss = (Button) field(type, "dismissShortcutButton").get(service);
        Button pinned = (Button) field(type, "textEditShortcutButton").get(service);
        android.view.View emoji = (android.view.View) field(type, "emojiPanel").get(service);
        Field hardware = field(type, "hardwareKeyboardMode");
        Field expanded = field(type, "hardwareKeyboardUserExpanded");
        Field hidden = field(type, "toolbarHidden");
        Field enabled = field(type, "toolbarDismiss");
        boolean hardwareBefore = hardware.getBoolean(service), expandedBefore = expanded.getBoolean(service);
        boolean hiddenBefore = hidden.getBoolean(service), enabledBefore = enabled.getBoolean(service);
        int emojiBefore = emoji.getVisibility();
        try {
            enabled.setBoolean(service, false);
            hidden.setBoolean(service, false);
            emoji.setVisibility(android.view.View.GONE);
            render.invoke(service);
            check(dismiss.getVisibility() != android.view.View.VISIBLE, "unpinned dismiss shown while its switch is off");
            hardware.setBoolean(service, true);
            render.invoke(service);
            checkDismissOnEdge(dismiss, pinned, normal, middle, "显示软键盘", "candidate bar mode");
            check(pinned.getVisibility() == android.view.View.VISIBLE, "pinned tool hidden in candidate bar mode");
            check(dismiss.performClick(), "expand key has no action in candidate bar mode");
            check(!hardware.getBoolean(service), "expand key did not leave candidate bar mode");
            check(dismiss.getParent() == middle && dismiss.getVisibility() != android.view.View.VISIBLE,
                "dismiss stayed on the edge after the soft keyboard expanded");
            hidden.setBoolean(service, true);
            emoji.setVisibility(android.view.View.VISIBLE);
            render.invoke(service);
            checkDismissOnEdge(dismiss, pinned, normal, middle, "返回键盘", "hidden toolbar with a panel open");
            emoji.setVisibility(android.view.View.GONE);
            hidden.setBoolean(service, false);
            render.invoke(service);
            check(dismiss.getParent() == middle && dismiss.getVisibility() != android.view.View.VISIBLE,
                "dismiss stayed on the edge after the panel closed");
            check(normal.getChildAt(normal.getChildCount() - 1) == pinned, "pinned tool lost rightmost position");
        } finally {
            hardware.setBoolean(service, hardwareBefore);
            expanded.setBoolean(service, expandedBefore);
            hidden.setBoolean(service, hiddenBefore);
            enabled.setBoolean(service, enabledBefore);
            emoji.setVisibility(emojiBefore);
            render.invoke(service);
        }
    }

    private static void checkDismissOnEdge(Button dismiss, Button pinned, ViewGroup normal, ViewGroup middle,
            String description, String state) {
        check(dismiss.getVisibility() == android.view.View.VISIBLE, "dismiss hidden in " + state);
        check(description.contentEquals(dismiss.getContentDescription()), "dismiss reads wrong in " + state);
        check(dismiss.getParent() == normal && middle.indexOfChild(dismiss) == -1, "dismiss not on the edge in " + state);
        check(normal.getChildAt(normal.getChildCount() - 1) == dismiss
            && normal.getChildAt(normal.getChildCount() - 2) == pinned, "dismiss not right of the pinned tool in " + state);
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
