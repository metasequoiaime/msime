package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.graphics.Rect;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import java.io.File;
import java.nio.file.Files;
import java.util.Comparator;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.concurrent.ExecutorService;

/** 对安装产品实际测量窄屏、宽屏、算式胶囊和重排，保证横向滚动不带走两端按钮。 */
public final class ToolbarEdgesDeviceSmoke extends Instrumentation {
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
                Button undo = (Button) field(type, "undoShortcutButton").get(service);
                Button redo = (Button) field(type, "redoShortcutButton").get(service);
                check(!undo.isEnabled() && !redo.isEnabled(), "history controls enabled without editor");
                Object historyToolbar = field(type, "imeToolbar").get(service);
                method(historyToolbar.getClass(), "performHistory", boolean.class).invoke(historyToolbar, false);
                method(historyToolbar.getClass(), "performHistory", boolean.class).invoke(historyToolbar, true);
                Button more = (Button) field(type, "moreButton").get(service);
                Button dismiss = (Button) field(type, "dismissShortcutButton").get(service);
                HorizontalScrollView scroll = (HorizontalScrollView) field(type, "shortcutScroll").get(service);
                LinearLayout bar = (LinearLayout) field(type, "shortcutBar").get(service);
                check(!inside(more, scroll) && !inside(dismiss, scroll), "toolbar edges still scroll with tools");
                ViewGroup row = (ViewGroup) scroll.getParent();
                row.setVisibility(View.VISIBLE);
                String[] fields = {"emojiShortcutButton", "phraseShortcutButton", "clipboardShortcutButton",
                    "skinButton", "schemeButton", "floatingShortcutButton", "textEditShortcutButton"};
                Button[] buttons = new Button[fields.length];
                for (int index = 0; index < fields.length; index++)
                    buttons[index] = (Button) field(type, fields[index]).get(service);
                Object calculator = field(type, "imeCalculator").get(service);
                Button result = (Button) method(calculator.getClass(), "chip").invoke(calculator);
                float density = getTargetContext().getResources().getDisplayMetrics().density;
                // 先超宽再收窄，覆盖旧滚动偏移在布局变化后被重新钳制的路径。
                for (int widthDp : new int[] {600, 320, 240, 600}) {
                    for (boolean showResult : new boolean[] {false, true}) {
                        for (Button button : buttons) button.setVisibility(View.VISIBLE);
                        result.setText("= 12345678901234");
                        result.setVisibility(showResult ? View.VISIBLE : View.GONE);
                        layout(row, Math.round(widthDp * density), Math.round(64 * density));
                        scroll.scrollTo(0, 0);
                        Rect left = bounds(row, more), right = bounds(row, dismiss);
                        check(Math.abs(left.width() - 44 * density) <= 1, "brand touch width changed");
                        check(left.left >= 0 && right.right <= row.getWidth(), "toolbar edge clipped");
                        check(Math.abs(right.width() - 44 * density) <= 1, "dismiss touch width changed");
                        for (Button button : buttons)
                            check(button.getWidth() >= Math.round(40 * density), "tool shrank below minimum width");
                        scroll.scrollTo(Math.max(0, bar.getWidth() - scroll.getWidth()), 0);
                        if (widthDp <= 320) check(scroll.getScrollX() > 0, "narrow toolbar cannot scroll");
                        check(left.equals(bounds(row, more)) && right.equals(bounds(row, dismiss)),
                            "scroll moved fixed toolbar edges");
                        Rect last = bounds(row, buttons[buttons.length - 1]);
                        Rect viewport = bounds(row, scroll);
                        check(last.left >= viewport.left && last.right <= viewport.right,
                            "last tool is clipped at scroll end: " + widthDp + "/" + showResult
                                + " last=" + last.toShortString() + " viewport=" + viewport.toShortString()
                                + " scroll=" + scroll.getScrollX() + " content=" + bar.getWidth());
                    }
                }
                checkClipboardHeader(type, service, loader, density);
                // 用真实排序入口移动同一组按钮，检查算式仍是第一项、动作对象未被重建。
                Class<?> snapshot = loader.loadClass("app.msime.android.AndroidLocalSettings$Snapshot");
                Constructor<?> constructor = snapshot.getDeclaredConstructor(Map.class);
                constructor.setAccessible(true);
                field(type, "localSettings").set(service, constructor.newInstance(Map.of(
                    "platform.android.toolbar_order", "text_edit,floating,scheme,skin,clipboard,phrase,emoji")));
                Object toolbar = field(type, "imeToolbar").get(service);
                method(toolbar.getClass(), "applyShortcutOrder").invoke(toolbar);
                check(bar.getChildAt(0) == result, "calculator lost first position");
                for (int index = 0; index < buttons.length; index++)
                    check(bar.getChildAt(index + 1) == buttons[buttons.length - index - 1],
                        "custom order changed tool identity");
                for (int index = 1; index < buttons.length; index++) buttons[index].setVisibility(View.GONE);
                result.setVisibility(View.GONE);
                layout(row, Math.round(320 * density), Math.round(64 * density));
                check(scroll.getScrollX() == 0, "hidden tools left stale scroll offset");
                check(buttons[0].getWidth() >= scroll.getWidth() - 2 * Math.round(density),
                    "single visible tool did not fill available width");
            } catch (Exception | AssertionError error) { failure[0] = error; }
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
        Bundle result = new Bundle();
        Throwable error = failure[0];
        result.putString("stream", error == null
            ? "MSIME_DEVICE_SMOKE_PASSED: toolbar fixed edges, 240/320/600dp, calculator, order, hidden tools, resize and sticky clipboard header\n"
            : "MSIME_DEVICE_SMOKE_FAILED: toolbar layout (" + error.getClass().getSimpleName() + ")"
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
        finish(error == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    /** 用隔离历史驱动产品的真实渲染，记录卡片滚动时顶栏仍留在原位。 */
    private void checkClipboardHeader(Class<?> type, Object service, ClassLoader loader, float density)
            throws Exception {
        File directory = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(),
            "clipboard-sticky-").toFile();
        Field history = field(type, "clipboardHistory");
        Object original = history.get(service);
        boolean enabled = field(type, "clipboardHistoryEnabled").getBoolean(service);
        try {
            Class<?> storeType = loader.loadClass("app.msime.android.ClipboardHistoryStore");
            Object store = storeType.getConstructor(File.class).newInstance(directory);
            for (int index = 0; index < 30; index++)
                storeType.getMethod("add", String.class).invoke(store, "合成剪贴板记录 " + index);
            history.set(service, store);
            field(type, "clipboardHistoryEnabled").setBoolean(service, true);
            Object panels = field(type, "imePanels").get(service);
            method(panels.getClass(), "renderClipboardHistory").invoke(panels);
            ViewGroup surface = (ViewGroup) field(type, "clipboardSurface").get(service);
            ViewGroup header = (ViewGroup) field(type, "clipboardHeader").get(service);
            ScrollView scroll = (ScrollView) field(type, "clipboardScroll").get(service);
            ViewGroup cards = (ViewGroup) field(type, "clipboardPanel").get(service);
            surface.setVisibility(View.VISIBLE);
            for (int widthDp : new int[] {320, 600}) {
                layout(surface, Math.round(widthDp * density), Math.round(300 * density));
                scroll.scrollTo(0, 0);
                method(panels.getClass(), "updateClipboardBackToTop").invoke(panels);
                Button backToTop = (Button) field(type, "clipboardBackToTop").get(service);
                check(backToTop.getVisibility() == View.GONE, "back to top is visible before scrolling");
                check(header.getChildCount() > 0 && cards.getChildCount() > 1,
                    "clipboard renderer did not create header and synthetic cards");
                Rect fixed = bounds(surface, header);
                Rect first = bounds(surface, cards.getChildAt(0));
                check(first.top >= fixed.bottom, "clipboard cards overlap fixed header");
                scroll.scrollTo(0, cards.getHeight());
                check(scroll.getScrollY() > 0, "synthetic clipboard history did not overflow");
                method(panels.getClass(), "updateClipboardBackToTop").invoke(panels);
                check(backToTop.getVisibility() == View.VISIBLE, "back to top did not appear after scrolling");
                check(fixed.equals(bounds(surface, header)), "clipboard scrolling moved header");
                check(bounds(surface, cards.getChildAt(0)).top < first.top,
                    "clipboard scrolling did not move history cards");
                check(backToTop.performClick(), "back to top action was not handled");
                check(scroll.getScrollY() == 0, "back to top did not reset history position");
                method(panels.getClass(), "updateClipboardBackToTop").invoke(panels);
                check(backToTop.getVisibility() == View.GONE, "back to top stayed visible at top");
            }
            check((Boolean) method(panels.getClass(), "clipboardPanelOpen").invoke(panels),
                "visible clipboard surface is not reported open");
            method(type, "render").invoke(service);
            Object bottomBar = field(type, "imeBottomBar").get(service);
            Button bottomClipboard = (Button) field(bottomBar.getClass(), "clipboardButton").get(bottomBar);
            check(bottomClipboard.isSelected(), "clipboard bottom bar button is not selected while panel is open");
            method(type, "closeClipboardHistory").invoke(service);
            method(type, "render").invoke(service);
            check(!bottomClipboard.isSelected(), "clipboard bottom bar button stayed selected after insertion closed panel");
            method(panels.getClass(), "showClipboardHistory").invoke(panels);
            method(type, "render").invoke(service);
            check(bottomClipboard.isSelected(), "clipboard bottom bar button did not select after reopening panel");
            check(bottomClipboard.performClick(), "clipboard bottom bar button did not handle the click");
            check(surface.getVisibility() == View.GONE
                    && !(Boolean) method(panels.getClass(), "clipboardPanelOpen").invoke(panels),
                "clipboard bottom bar button did not close the visible panel");
            check(!bottomClipboard.isSelected(), "clipboard bottom bar button stayed selected after closing panel");
            method(type, "closeClipboardHistory").invoke(service);
            check(surface.getVisibility() == View.GONE
                    && !(Boolean) method(panels.getClass(), "clipboardPanelOpen").invoke(panels),
                "clipboard close left fixed header or panel state visible");
        } finally {
            history.set(service, original);
            field(type, "clipboardHistoryEnabled").setBoolean(service, enabled);
            try (var paths = Files.walk(directory.toPath())) {
                for (var path : paths.sorted(Comparator.reverseOrder()).collect(java.util.stream.Collectors.toList())) Files.delete(path);
            }
        }
    }

    private static void layout(View row, int width, int height) {
        row.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
        row.layout(0, 0, width, height);
    }
    private static boolean inside(View view, ViewGroup ancestor) {
        for (android.view.ViewParent parent = view.getParent(); parent != null; parent = parent.getParent())
            if (parent == ancestor) return true;
        return false;
    }
    private static Rect bounds(ViewGroup parent, View view) {
        Rect result = new Rect();
        view.getDrawingRect(result);
        parent.offsetDescendantRectToMyCoords(view, result);
        return result;
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
