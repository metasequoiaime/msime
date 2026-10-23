package app.msime.android.test;

import android.graphics.Rect;
import android.graphics.Bitmap;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真实键盘上全文浮层、卡片滚动和跨行拖选，只使用合成文本。 */
public final class ClipboardSegmentDragDeviceSmoke extends DeviceSmoke {
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private long downTime;
    private boolean previewActionsOnly;

    @Override public void onCreate(android.os.Bundle arguments) {
        previewActionsOnly = arguments != null && "true".equals(arguments.getString("preview-actions-only"));
        super.onCreate(arguments);
    }
    private static String word(int i) { return "Synthetic" + (char) ('A' + i); }

    @Override protected String successDescription() {
        if (previewActionsOnly) return "full-text preview inserts original text, edits, saves and returns without leaking its window";
        return "floating full-text preview above IME, scrolling, close, action passthrough and lifecycle cleanup; card scroll, swipe, segment drag and preview action regression";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        for (File f : new File[] {preferences, history, local})
            backups.put(f, f.isFile() ? Files.readAllBytes(f.toPath()) : null);
        try {
            JSONObject prefs = read(preferences);
            prefs.put("revision", prefs.getLong("revision") + 1);
            prefs.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            write(preferences, prefs.toString());
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)).toString());
            if (previewActionsOnly) { previewActions(history); return; }
            StringBuilder source = new StringBuilder();
            for (int i = 0; i < 26; i++) { if (i > 0) source.append(' '); source.append(word(i)); }
            StringBuilder preview = new StringBuilder();
            for (int i = 0; i < 80; i++) preview.append("Synthetic preview line ").append(i).append('\n');
            write(history, new JSONArray().put(new JSONObject().put("text", preview.toString())
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
            openClipboardEditor();
            floatingPreview(history, preview.toString());
            write(history, new JSONArray().put(new JSONObject().put("text", source.toString())
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
            openClipboardEditor();
            AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
                && node.getText().toString().startsWith(word(0)));
            stage = "long clipboard card scrolls without insertion";
            byte[] seededHistory = Files.readAllBytes(history.toPath());
            previewCard(card);
            stage = "preview, swipe and dismiss leave editor and history untouched";
            if (!java.util.Arrays.equals(seededHistory, Files.readAllBytes(history.toPath())))
                throw new AssertionError("Preview rewrote history");
            // 空编辑器的节点文字是提示语 Plain editor，不是空串；和其他套件一样按 isShowingHintText 判空。
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            card = await(node -> node.isLongClickable() && node.getText() != null
                && node.getText().toString().startsWith(word(0)));
            stage = "long press remains available after preview";
            if (!card.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK)) throw new AssertionError("Long press refused");
            tap(node -> equalsText("分词", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
            stage = "single tap keeps original behavior";
            tap(described("词片 " + word(0)));
            selected(0, true);
            tap(described("词片 " + word(0)));
            selected(0, false);
            stage = "drag spans multiple rows";
            Rect first = rect(0), fourth = rect(5);
            if (fourth.centerY() <= first.centerY()) throw new AssertionError("Fixture did not wrap");
            drag(first, fourth, false);
            for (int i = 0; i <= 5; i++) selected(i, true);
            selected(6, false);
            stage = "reverse drag deselects selected range";
            drag(rect(5), rect(0), false);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "cancel restores original selection";
            drag(rect(0), rect(5), true);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "range shrink restores words outside final range";
            Rect start = rect(0), far = rect(5), near = rect(1);
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            move(start, far);
            move(far, near);
            event(MotionEvent.ACTION_UP, near.centerX(), near.centerY());
            selected(0, true); selected(1, true); selected(2, false); selected(3, false);
            stage = "edge hold scrolls to hidden final word";
            tap(key("全选")); tap(key("全不选"));
            start = rect(0);
            Rect bottom = new Rect();
            await(described("分词滚动区域")).getBoundsInScreen(bottom);
            int edgeY = bottom.bottom - 8;
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            // 最后一行可能有多个词片，停在右下角才能覆盖末词。
            Rect edge = new Rect(bottom.right - 8, edgeY, bottom.right - 7, edgeY + 1);
            move(start, edge);
            SystemClock.sleep(8000);
            event(MotionEvent.ACTION_UP, edge.centerX(), edge.centerY());
            selected(25, true);
            tap(key("插入"));
            await(field("msime-test-plain").and(node -> equalsText(source.toString(), node.getText())));
            previewActions(history);
        } finally {
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else publish(entry.getKey(), entry.getValue());
            }
            restartIme();
        }
    }

    private void previewActions(File history) throws Exception {
        StringBuilder source = new StringBuilder();
        for (int i = 0; i < 80; i++) source.append("Synthetic preview line ").append(i).append('\n');
        String original = source.toString();
        String replacement = "Synthetic modified preview text🙂";
        write(history, new JSONArray().put(new JSONObject().put("text", original)
            .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
        byte[] seeded = Files.readAllBytes(history.toPath());
        openClipboardEditor(true);
        stage = "preview insertion commits full original text once and restores keyboard";
        openFloatingPreview();
        touch(described("插入剪贴板全文"));
        previewClosed();
        stage = "preview insertion preserves every line in a multiline editor";
        await(field("msime-test-plain").and(node -> equalsText(original, node.getText())));
        await(key("n"));
        if (!java.util.Arrays.equals(seeded, Files.readAllBytes(history.toPath())))
            throw new AssertionError("Preview insertion changed history");
        // 再开合成编辑器，修改不应先把旧正文插入其中。
        openClipboardEditor(true);
        stage = "preview modification opens existing native editor with full text";
        openFloatingPreview();
        touch(described("修改剪贴板记录"));
        previewClosed();
        await(described("剪贴板记录的文字").and(node -> equalsText(original, node.getText())));
        android.os.Bundle changed = new android.os.Bundle();
        changed.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, replacement);
        if (!await(described("剪贴板记录的文字")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, changed))
            throw new AssertionError("Synthetic clipboard replacement refused");
        tap(node -> equalsText("保存", node.getText()) && node.isEnabled());
        stage = "preview edit saves and returns to unchanged external editor";
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        previewClosed();
        tap(field("msime-test-plain"));
        await(key("n"));
        tap(described("剪贴板"));
        AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(replacement));
        if (!card.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Modified synthetic record long press refused");
        stage = "modified preview still inserts and preserves record identity";
        touch(described("插入剪贴板全文"));
        previewClosed();
        await(field("msime-test-plain").and(node -> equalsText(replacement, node.getText())));
        JSONArray saved = new JSONArray(new String(Files.readAllBytes(history.toPath()), StandardCharsets.UTF_8));
        if (saved.length() != 1 || !saved.getJSONObject(0).getBoolean("pinned")
                || saved.getJSONObject(0).getLong("timestampMs") != 1700000000000L
                || !replacement.equals(saved.getJSONObject(0).getString("text")))
            throw new AssertionError("Preview edit lost record identity");
    }

    private void openClipboardEditor() throws Exception {
        openClipboardEditor(false);
    }

    private void openClipboardEditor(boolean multiline) throws Exception {
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + (multiline ? " --ez msime_test_multiline true" : ""));
        tap(field("msime-test-plain"));
        await(key("n"));
        tap(described("剪贴板"));
    }

    private void floatingPreview(File history, String text) throws Exception {
        stage = "physical long press opens full-text preview above keyboard";
        byte[] before = Files.readAllBytes(history.toPath());
        Bitmap beforePreview = automation.takeScreenshot();
        Rect editorBounds = new Rect();
        await(field("msime-test-plain")).getBoundsInScreen(editorBounds);
        Rect card = openFloatingPreview();
        AccessibilityNodeInfo scroll = awaitStableBounds(described("剪贴板全文滚动区域"));
        Rect bounds = new Rect();
        scroll.getBoundsInScreen(bounds);
        if (bounds.bottom > card.top || bounds.height() < card.height() * 2)
            throw new AssertionError("Preview did not extend above the keyboard");
        // Android 8 的无障碍树会把长文本拆成多个 TextView，不能要求一个节点承载全文。
        // 预览滚动区域和首行分别在上面、这里核对，足以证明全文已经显示且没有复用可长按的卡片。
        await(node -> node.getText() != null
            && node.getText().toString().contains("Synthetic preview line 0")
            && !node.isLongClickable());
        Rect editorAfter = new Rect();
        await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused)).getBoundsInScreen(editorAfter);
        Rect cardAfter = new Rect();
        await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith("Synthetic preview line 0")).getBoundsInScreen(cardAfter);
        if (!editorBounds.equals(editorAfter) || !card.equals(cardAfter))
            throw new AssertionError("Floating preview moved the editor or keyboard card");
        await(described("剪贴板记录操作"));
        // 仅在专用合成编辑器中留存画面，供核对浮层边界、配色和滚动条。
        automation.waitForIdle(300, 3000);
        SystemClock.sleep(250);
        Bitmap screenshot = automation.takeScreenshot();
        if (screenshot == null) throw new AssertionError("Synthetic preview screenshot unavailable");
        try (var output = new java.io.FileOutputStream(
                new File(getTargetContext().getCacheDir(), "synthetic-clipboard-preview.png"))) {
            if (!screenshot.compress(Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("Synthetic preview screenshot encoding failed");
            if (beforePreview == null) throw new AssertionError("Synthetic editor screenshot unavailable");
            // 仅有无障碍边界和触摸还不足以证明窗口真的绘制在键区外（API 28 会静默裁剪）。
            int changed = 0;
            for (int row = 1; row < 64; row++) for (int column = 1; column < 64; column++) {
                int x = bounds.left + bounds.width() * column / 64;
                int y = bounds.top + bounds.height() * row / 64;
                if (beforePreview.getPixel(x, y) != screenshot.getPixel(x, y)) changed++;
            }
            if (changed < 40) throw new AssertionError("Floating preview was cropped outside the keyboard");
        } finally {
            screenshot.recycle();
            if (beforePreview != null) beforePreview.recycle();
        }
        stage = "floating preview supports touch and accessible scrolling to end";
        drag(new Rect(bounds.centerX(), bounds.bottom - 16, bounds.centerX() + 1, bounds.bottom - 15),
            new Rect(bounds.centerX(), bounds.top + 16, bounds.centerX() + 1, bounds.top + 17), false);
        int pages = 0;
        while (scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)) {
            SystemClock.sleep(100);
            scroll.refresh();
            if (++pages > 30) throw new AssertionError("Floating preview never reached its end");
        }
        if (pages == 0 || !scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD))
            throw new AssertionError("Floating preview did not scroll through its full text");
        stage = "close preserves editor, history and management actions";
        touch(described("关闭剪贴板全文预览"));
        previewClosed();
        await(described("剪贴板记录操作"));
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        if (!java.util.Arrays.equals(before, Files.readAllBytes(history.toPath())))
            throw new AssertionError("Floating preview rewrote history");
        stage = "outside touch reaches management action on first tap";
        openFloatingPreview();
        touch(node -> equalsText("收起", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
        previewClosed();
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), described("剪贴板记录操作")) != null)
                throw new AssertionError("Outside touch did not reach the action row");
        }
        stage = "hiding keyboard removes floating preview";
        openFloatingPreview();
        shellDiscard("input keyevent 4");
        previewClosed();
        tap(field("msime-test-plain"));
        await(key("n"));
        previewClosed();
        tap(described("剪贴板"));
        stage = "changing input field removes floating preview";
        openFloatingPreview();
        if (!await(field("msime-test-password")).performAction(AccessibilityNodeInfo.ACTION_FOCUS))
            throw new AssertionError("Synthetic editor focus switch refused");
        await(field("msime-test-password").and(AccessibilityNodeInfo::isFocused));
        previewClosed();
    }

    private Rect openFloatingPreview() {
        AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith("Synthetic preview line 0"));
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 200);
        event(MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        await(described("关闭剪贴板全文预览"));
        return bounds;
    }

    private void touch(java.util.function.Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        await(match).getBoundsInScreen(bounds);
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        event(MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(200);
    }

    private void previewClosed() {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            boolean found = false;
            for (var window : automation.getWindows())
                found |= find(window.getRoot(), described("关闭剪贴板全文预览")) != null;
            if (!found) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Floating preview survived its close boundary");
    }

    private void previewCard(AccessibilityNodeInfo card) throws Exception {
        if (!card.isScrollable()) throw new AssertionError("Long card was not scrollable");
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        Rect from = new Rect(bounds.centerX(), bounds.bottom - 8, bounds.centerX() + 1, bounds.bottom - 7);
        Rect to = new Rect(bounds.centerX(), bounds.top + 8, bounds.centerX() + 1, bounds.top + 9);
        drag(from, to, false);
        SystemClock.sleep(200);
        card.refresh();
        Rect after = new Rect();
        card.getBoundsInScreen(after);
        if (!bounds.equals(after)) throw new AssertionError("Preview scrolled the outer list");
        // 读屏也能翻页到全文末尾，再回到开头，滚动动作不能触发插入。
        int pages = 0;
        while (card.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)) {
            if (++pages > 40) throw new AssertionError("Preview did not reach the end");
            card.refresh();
        }
        if (pages == 0) throw new AssertionError("Preview fixture did not span multiple pages");
        while (card.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)) {
            if (--pages < -2) throw new AssertionError("Preview did not return to the start");
            card.refresh();
        }
        stage = "horizontal delete still works after vertical preview";
        Rect swipeFrom = new Rect(bounds.right - 8, bounds.centerY(), bounds.right - 7, bounds.centerY() + 1);
        Rect swipeTo = new Rect(bounds.left + 8, bounds.centerY(), bounds.left + 9, bounds.centerY() + 1);
        drag(swipeFrom, swipeTo, false);
        await(described("删除这条剪贴板记录").and(AccessibilityNodeInfo::isVisibleToUser));
        // 已打开的卡片第一次点按只收回删除区，下一步再检查长按管理。
        tap(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(word(0)));
    }

    private Rect rect(int i) {
        Rect r = new Rect();
        await(described("词片 " + word(i)).and(AccessibilityNodeInfo::isVisibleToUser)).getBoundsInScreen(r);
        return r;
    }
    private void selected(int i, boolean value) {
        await(described("词片 " + word(i)).and(node -> node.isSelected() == value));
    }
    private void drag(Rect from, Rect to, boolean cancel) {
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, from.centerX(), from.centerY());
        move(from, to);
        event(cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, to.centerX(), to.centerY());
    }
    private void move(Rect from, Rect to) {
        for (int i = 1; i <= 12; i++) {
            event(MotionEvent.ACTION_MOVE, from.centerX() + (to.centerX() - from.centerX()) * i / 12f,
                from.centerY() + (to.centerY() - from.centerY()) * i / 12f);
            SystemClock.sleep(20);
        }
    }
    private void event(int action, float x, float y) {
        MotionEvent e = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0);
        e.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try { if (!automation.injectInputEvent(e, true)) throw new AssertionError("Touch injection refused"); }
        finally { e.recycle(); }
    }
    private static JSONObject read(File f) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8));
    }
    private void write(File f, String text) throws Exception { publish(f, text.getBytes(StandardCharsets.UTF_8)); }
}
