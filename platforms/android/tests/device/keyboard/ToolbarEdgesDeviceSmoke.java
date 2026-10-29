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
                checkLongSpaceGestureAfterRecreation(type, service, loader);
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

    /** 同一服务重建键盘视图后，新逗号键仍要接收下滑；用合成编辑器收集实际提交字符。长空格是中文 26 键的底行样式（共享偏好 touch_twenty_six_key_style），这里直接换上服务持有的样式，英文 26 键不受它影响，所以用中文模式。 */
    private void checkLongSpaceGestureAfterRecreation(Class<?> type, Object service, ClassLoader loader)
            throws Exception {
        Class<?> styles = loader.loadClass("app.msime.android.TouchKeyStyle$Styles");
        Object longSpace = styles.getMethod("parse", String.class, String.class, String.class)
            .invoke(null, "long_space", null, null);
        Field stylesField = field(type, "touchKeyStyles");
        Object previous = stylesField.get(service);
        boolean previousEnglish = field(type, "dedicatedEnglish").getBoolean(service);
        StringBuilder committed = new StringBuilder();
        android.view.inputmethod.InputConnection connection =
            (android.view.inputmethod.InputConnection) java.lang.reflect.Proxy.newProxyInstance(
                android.view.inputmethod.InputConnection.class.getClassLoader(),
                new Class<?>[] {android.view.inputmethod.InputConnection.class}, (proxy, call, args) -> {
                    if (call.getName().equals("commitText")) committed.append((CharSequence) args[0]);
                    if (call.getReturnType() == boolean.class) return true;
                    if (call.getReturnType() == int.class) return 0;
                    return null;
                });
        try {
            for (int generation = 0; generation < 2; generation++) {
                if (generation > 0) type.getMethod("onCreateInputView").invoke(service);
                stylesField.set(service, longSpace);
                field(type, "dedicatedEnglish").setBoolean(service, false);
                field(type, "editorInputType").setInt(service, android.text.InputType.TYPE_CLASS_TEXT);
                field(type, "touchLayoutHint").setInt(service, 0);
                field(type, "connection").set(service, connection);
                method(type, "updateQuickPunctuation").invoke(service);
                Button comma = (Button) field(type, "quickPunctuationButton").get(service);
                comma.setEnabled(true);
                comma.layout(0, 0, 200, 200);
                long down = android.os.SystemClock.uptimeMillis();
                float density = getTargetContext().getResources().getDisplayMetrics().density;
                // 门槛是字母键滑动输入符号的 14 dp；滑过它再留余量，另取一段不到门槛的上移当作点按时的手指漂移。
                int distance = Math.round(14 * density) + 20;
                int drift = Math.round(8 * density);
                int[] actions = {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_MOVE,
                    android.view.MotionEvent.ACTION_UP};
                // 下滑是 #9881 要的方向；上滑与字母键滑动输入符号的默认方向一致，两个方向都要提交句号。
                for (int direction : new int[] {1, -1}) {
                    committed.setLength(0);
                    for (int index = 0; index < actions.length; index++) {
                        android.view.MotionEvent event = android.view.MotionEvent.obtain(down, down + index * 20,
                            actions[index], 40, index == 0 ? 100 : 100 + direction * distance, 0);
                        try { comma.dispatchTouchEvent(event); }
                        finally { event.recycle(); }
                    }
                    // 没有会话时句点原样上屏，有会话时引擎按中文标点给出「。」。
                    check(committed.toString().equals(".") || committed.toString().equals("。"),
                        "long-space comma " + (direction > 0 ? "down" : "up") + " swipe lost after input-view generation "
                            + generation + ": " + committed);
                    down = android.os.SystemClock.uptimeMillis();
                }
                // 点逗号时手指往上带一点（不到门槛）不是滑动，不能出句号。
                committed.setLength(0);
                for (int index = 0; index < actions.length; index++) {
                    android.view.MotionEvent event = android.view.MotionEvent.obtain(down, down + index * 20,
                        actions[index], 40, index == 0 ? 100 : 100 - drift, 0);
                    try { comma.dispatchTouchEvent(event); }
                    finally { event.recycle(); }
                }
                check(!committed.toString().contains(".") && !committed.toString().contains("。"),
                    "long-space comma tap drift became a period after input-view generation " + generation + ": " + committed);
                down = android.os.SystemClock.uptimeMillis();
            }
        } finally {
            stylesField.set(service, previous);
            field(type, "dedicatedEnglish").setBoolean(service, previousEnglish);
        }
    }

    /** 用隔离历史驱动产品的真实渲染，记录卡片滚动时顶栏仍留在原位。 */
    private void checkClipboardHeader(Class<?> type, Object service, ClassLoader loader, float density)
            throws Exception {
        File directory = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(),
            "clipboard-sticky-").toFile();
        Field history = field(type, "clipboardHistory");
        Object original = history.get(service);
        boolean enabled = field(type, "clipboardHistoryEnabled").getBoolean(service);
        Field connection = field(type, "connection");
        Object originalConnection = connection.get(service);
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
            checkClipboardClearConfirm(panels, header, surface, density);
            checkClipboardMoreMenu(type, service, panels, header, cards, surface, density);
            check((Boolean) method(panels.getClass(), "clipboardPanelOpen").invoke(panels),
                "visible clipboard surface is not reported open");
            method(type, "render").invoke(service);
            Object bottomBar = field(type, "imeBottomBar").get(service);
            Button bottomClipboard = (Button) field(bottomBar.getClass(), "clipboardButton").get(bottomBar);
            Button topClipboard = (Button) field(type, "clipboardShortcutButton").get(service);
            check(bottomClipboard.isSelected(), "clipboard bottom bar button is not selected while panel is open");
            check(topClipboard.isSelected(), "clipboard toolbar button is not selected while panel is open");
            // 用真实编辑器接收插入；调用返回后立即检查，不补刷新或模拟下一次按键。
            android.widget.EditText editor = new android.widget.EditText(getTargetContext());
            editor.setInputType(android.text.InputType.TYPE_CLASS_TEXT);
            android.view.inputmethod.InputConnection editorConnection =
                editor.onCreateInputConnection(new android.view.inputmethod.EditorInfo());
            check(editorConnection != null, "synthetic clipboard editor has no input connection");
            connection.set(service, editorConnection);
            String inserted = "合成剪贴板插入";
            method(type, "insertClipboardText", String.class).invoke(service, inserted);
            check(editor.getText().toString().equals(inserted), "clipboard insertion did not reach the editor");
            check(surface.getVisibility() == View.GONE
                    && !(Boolean) method(panels.getClass(), "clipboardPanelOpen").invoke(panels),
                "clipboard insertion did not close the panel");
            check(!topClipboard.isSelected(), "clipboard toolbar button stayed selected after insertion closed panel");
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
            connection.set(service, originalConnection);
            try (var paths = Files.walk(directory.toPath())) {
                for (var path : paths.sorted(Comparator.reverseOrder()).collect(java.util.stream.Collectors.toList())) Files.delete(path);
            }
        }
    }

    /**
     * 确认清空的顶行在窄键盘上也要整行落在键盘宽度以内：问句、「取消」「清空」都有宽高，按钮不出右边界（#10030）。
     *
     * <p>顶行原来套在 HorizontalScrollView 里按 UNSPECIFIED 宽度量，整行约 270dp；240dp 宽时「清空」被挤到右侧屏外，要横滑才点得到。
     */
    private static void checkClipboardClearConfirm(Object panels, ViewGroup header, ViewGroup surface,
            float density) throws Exception {
        Field pending = field(panels.getClass(), "clipboardClearPending");
        Method render = method(panels.getClass(), "renderClipboardHistory");
        pending.setBoolean(panels, true);
        try {
            render.invoke(panels);
            for (int widthDp : new int[] {240, 320, 600}) {
                layout(surface, Math.round(widthDp * density), Math.round(300 * density));
                View question = descendant(header, view -> view instanceof android.widget.TextView
                    && !(view instanceof Button)
                    && "确认清空全部记录（含固定项）？".contentEquals(((android.widget.TextView) view).getText()));
                View cancel = descendant(header, view -> view instanceof Button
                    && "取消".contentEquals(((Button) view).getText()));
                View confirm = descendant(header, view -> view instanceof Button
                    && "确认清空剪贴板历史".contentEquals(view.getContentDescription()));
                check(question != null && cancel != null && confirm != null,
                    "clipboard clear confirm row is missing its question or buttons at " + widthDp + "dp");
                int right = header.getWidth() - header.getPaddingRight();
                for (View part : new View[] {question, cancel, confirm}) {
                    Rect area = bounds(header, part);
                    check(area.width() > 0 && area.height() > 0,
                        "clipboard clear confirm part is not drawn at " + widthDp + "dp: " + area.toShortString());
                    check(area.left >= header.getPaddingLeft() && area.right <= right,
                        "clipboard clear confirm part leaves the keyboard width at " + widthDp + "dp: "
                            + area.toShortString() + " outside " + header.getPaddingLeft() + ".." + right);
                }
                check(bounds(header, cancel).right <= bounds(header, confirm).left,
                    "clipboard clear cancel and confirm overlap at " + widthDp + "dp");
            }
            // 「提示明显」的另一半：确认按钮必须是回车键的强调键帽，「取消」仍是普通文字按钮。只量几何时，去掉 RETURN 也照样通过。
            View cancel = descendant(header, view -> view instanceof Button
                && "取消".contentEquals(((Button) view).getText()));
            View confirm = descendant(header, view -> view instanceof Button
                && "确认清空剪贴板历史".contentEquals(view.getContentDescription()));
            Class<?> press = confirm.getClass().getClassLoader().loadClass("app.msime.android.KeyboardPressButton");
            Method role = method(press, "keyboardRole");
            check(press.isInstance(confirm) && "RETURN".equals(String.valueOf(role.invoke(confirm))),
                "clipboard clear confirm is not drawn as the emphasized return key cap");
            check(press.isInstance(cancel) && !"RETURN".equals(String.valueOf(role.invoke(cancel))),
                "clipboard clear cancel took the emphasized return key cap");
        } finally {
            pending.setBoolean(panels, false);
            render.invoke(panels);
        }
    }
    /**
     * 顶行只留本机、云端、计数、搜索、多选和「更多」，常见宽度下一行放得下、不用横滑；分类、导出、清空在「更多」展开的菜单里（#10037）。
     *
     * <p>菜单项必须是普通键帽，字色就是当前皮肤的按键前景色：深色模式下菜单文字和底色融成一片是报告里特别提醒的问题，只查几何时把菜单项换成别的样式也会通过。
     */
    private static void checkClipboardMoreMenu(Class<?> type, Object service, Object panels, ViewGroup header,
            ViewGroup cards, ViewGroup surface, float density) throws Exception {
        Field open = field(panels.getClass(), "clipboardMoreOpen");
        Field pending = field(panels.getClass(), "clipboardClearPending");
        Method render = method(panels.getClass(), "renderClipboardHistory");
        try {
            open.setBoolean(panels, false);
            render.invoke(panels);
            for (int widthDp : new int[] {360, 600}) {
                layout(surface, Math.round(widthDp * density), Math.round(300 * density));
                for (String moved : new String[] {"管理剪贴板分类", "导出全部剪贴板历史", "清空剪贴板历史"})
                    check(descendant(surface, view -> moved.contentEquals(view.getContentDescription())) == null,
                        "clipboard " + moved + " is still drawn while the more menu is closed");
                View row = descendant(header, view -> view instanceof android.widget.HorizontalScrollView);
                ViewGroup scroll = (ViewGroup) row;
                check(scroll != null && scroll.getChildAt(0).getWidth() <= scroll.getWidth(),
                    "clipboard header still needs horizontal scrolling at " + widthDp + "dp");
                View more = descendant(header, view -> view instanceof Button
                    && "更多剪贴板操作".contentEquals(view.getContentDescription()));
                check(more != null, "clipboard header has no more button at " + widthDp + "dp");
                Rect area = bounds(header, more);
                check(area.width() > 0 && area.height() > 0 && area.right <= header.getWidth() - header.getPaddingRight(),
                    "clipboard more button leaves the keyboard width at " + widthDp + "dp: " + area.toShortString());
            }
            View more = descendant(header, view -> view instanceof Button
                && "更多剪贴板操作".contentEquals(view.getContentDescription()));
            check(more.performClick() && open.getBoolean(panels), "clipboard more button did not open the menu");
            layout(surface, Math.round(360 * density), Math.round(300 * density));
            View menu = descendant(cards, view -> "更多剪贴板操作菜单".contentEquals(view.getContentDescription()));
            check(menu != null, "clipboard more menu is not drawn in the panel");
            Rect menuArea = bounds(surface, menu);
            Rect moreArea = bounds(surface, descendant(header, view -> view instanceof Button
                && "更多剪贴板操作".contentEquals(view.getContentDescription())));
            check(menuArea.top >= moreArea.bottom && menuArea.right <= surface.getWidth(),
                "clipboard more menu is not below the more button: " + menuArea.toShortString());
            Object skin = field(type, "skin").get(service);
            int foreground = android.graphics.Color.parseColor(
                (String) skin.getClass().getMethod("keyForeground").invoke(skin));
            Class<?> press = menu.getClass().getClassLoader().loadClass("app.msime.android.KeyboardPressButton");
            Method role = method(press, "keyboardRole");
            for (String item : new String[] {"管理剪贴板分类", "导出全部剪贴板历史", "清空剪贴板历史"}) {
                View button = descendant((ViewGroup) menu, view -> item.contentEquals(view.getContentDescription()));
                check(button != null, "clipboard more menu has no " + item);
                Rect itemArea = bounds(surface, button);
                check(itemArea.width() > 0 && itemArea.height() > 0 && itemArea.right <= surface.getWidth(),
                    "clipboard more menu item is not drawn inside the keyboard: " + item + " " + itemArea.toShortString());
                check(press.isInstance(button) && "KEY".equals(String.valueOf(role.invoke(button))),
                    "clipboard more menu item is not a plain key cap: " + item);
                check(((Button) button).getCurrentTextColor() == foreground,
                    "clipboard more menu item does not use the skin's key label colour: " + item);
            }
            View clear = descendant((ViewGroup) menu, view -> "清空剪贴板历史".contentEquals(view.getContentDescription()));
            check(clear.performClick() && pending.getBoolean(panels) && !open.getBoolean(panels),
                "clipboard more menu clear did not close the menu and ask for confirmation");
            check(descendant(cards, view -> "更多剪贴板操作菜单".contentEquals(view.getContentDescription())) == null,
                "clipboard more menu stayed open after choosing clear");
        } finally {
            open.setBoolean(panels, false);
            pending.setBoolean(panels, false);
            render.invoke(panels);
        }
    }
    private static View descendant(ViewGroup root, java.util.function.Predicate<View> match) {
        for (int index = 0; index < root.getChildCount(); index++) {
            View child = root.getChildAt(index);
            if (match.test(child)) return child;
            if (child instanceof ViewGroup group) {
                View found = descendant(group, match);
                if (found != null) return found;
            }
        }
        return null;
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
