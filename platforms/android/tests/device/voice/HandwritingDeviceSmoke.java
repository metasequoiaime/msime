package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.os.ParcelFileDescriptor;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.function.Predicate;

/** Device-only acceptance for the packaged ML Kit recognizer and real IME controls. */
public final class HandwritingDeviceSmoke extends DeviceSmoke {
    private static final String PREVIEW_PACKAGE = "app.msime.android";
    private static final long MODEL_TIMEOUT_MILLIS = 180_000;
    private static final String TOUCH_REQUEST = "msime-handwriting-touch.request";
    private static final String TOUCH_ACK = "msime-handwriting-touch.ack";
    private int touchRequestSequence;

    @Override protected String successDescription() {
        return "ML Kit readiness, real ink recognition, first-candidate keys, undo and scheme restore";
    }

    @Override protected void runChecks() throws Exception {
        stage = "handwriting preview wake";
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        stage = "handwriting IME rebind";
        shell("ime disable app.msime.android/.MSIMEInputService");
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        SystemClock.sleep(1000);
        stage = "handwriting editor launch";
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "handwriting editor focus";
        tap(field("msime-test-plain"));
        await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));

        stage = "handwriting keyboard ready";
        AccessibilityNodeInfo schemeControl = awaitAnyFor(schemeControl(true), 30_000);
        String originalScheme = schemeControl.getContentDescription().toString()
            .substring("输入方案：".length());
        boolean restoreScheme = !"手写".equals(originalScheme);
        boolean completed = false;
        String failureStage = null;
        try {
            if (restoreScheme) {
                stage = "handwriting scheme panel";
                accessibleClick(schemeControl(true));
                stage = "handwriting scheme selection";
                accessibleClick(description("输入方案卡片 手写"));
            }

            stage = "handwriting model availability";
            AccessibilityNodeInfo availability = awaitFor(handwritingReadyOrDownload(), 30_000);
            if (equalsText("此构建不含手写识别", availability.getText()))
                throw new AssertionError("Packaged handwriting recognizer was unavailable");
            AccessibilityNodeInfo download = findVisible(description("下载中文手写模型；完成后可离线识别"));
            if (download != null) {
                stage = "handwriting model download";
                tap(description("下载中文手写模型；完成后可离线识别"));
            }
            awaitFor(imeTextContains("在此手写，停笔后选字"), MODEL_TIMEOUT_MILLIS);

            stage = "handwriting ink recognition for space";
            drawMiddleCharacter(canvasBounds());
            stage = "handwriting ink touch path for space";
            awaitFor(handwritingInkObserved(), 15_000);
            stage = "handwriting ink recognition for space";
            AccessibilityNodeInfo first = awaitFor(firstCandidate(), 60_000);
            CharSequence firstText = first.getText();
            if (firstText == null || firstText.length() == 0)
                throw new AssertionError("Recognized candidate was empty");
            tap(key("空格"));
            AccessibilityNodeInfo afterSpace = awaitFor(field("msime-test-plain").and(node ->
                node.getText() != null && !node.getText().toString().isEmpty()
                    && node.getText().chars().noneMatch(Character::isWhitespace)), 15_000);
            String firstCommitted = afterSpace.getText().toString();

            stage = "handwriting ink recognition for return";
            drawMiddleCharacter(canvasBounds());
            AccessibilityNodeInfo second = awaitFor(firstCandidate(), 60_000);
            CharSequence secondText = second.getText();
            if (secondText == null || secondText.length() == 0)
                throw new AssertionError("Recognized candidate was empty");
            stage = "handwriting return key commit";
            tap(returnKey());
            AccessibilityNodeInfo afterReturn = awaitFor(field("msime-test-plain").and(node ->
                node.getText() != null && node.getText().length() > firstCommitted.length()
                    && node.getText().chars().noneMatch(Character::isWhitespace)), 15_000);
            String committed = afterReturn.getText().toString();

            stage = "handwriting hardware space commit";
            drawMiddleCharacter(canvasBounds());
            awaitFor(firstCandidate(), 60_000);
            injectKey(KeyEvent.KEYCODE_SPACE);
            String beforeHardwareSpace = committed;
            AccessibilityNodeInfo afterHardwareSpace = awaitFor(field("msime-test-plain").and(node ->
                node.getText() != null && node.getText().length() > beforeHardwareSpace.length()
                    && node.getText().chars().noneMatch(Character::isWhitespace)), 15_000);
            committed = afterHardwareSpace.getText().toString();

            stage = "handwriting hardware delete undo";
            drawStroke(canvasBounds(), new float[][] {{0.50f, 0.10f}, {0.50f, 0.90f}});
            injectKey(KeyEvent.KEYCODE_DEL);
            awaitFor(imeTextContains("在此手写，停笔后选字"), 15_000);
            AccessibilityNodeInfo editor = awaitFor(field("msime-test-plain"), 15_000);
            if (!equalsText(committed, editor.getText()))
                throw new AssertionError("Hardware delete changed committed editor text");

            stage = "handwriting symbol layer open";
            accessibleClick(description("切换符号键盘"));
            stage = "handwriting symbol layer ready";
            awaitAnyFor(description("关闭符号面板"), 15_000);
            stage = "handwriting letter layer restore";
            accessibleClick(description("关闭符号面板"));
            stage = "handwriting UI restored after symbols";
            awaitFor(imeTextContains("在此手写，停笔后选字"), 30_000);
            completed = true;
        } catch (Exception | AssertionError error) {
            failureStage = stage;
            throw error;
        } finally {
            if (restoreScheme) {
                try {
                    stage = "handwriting scheme restore";
                    accessibleClick(schemeControl(true));
                    accessibleClick(description("输入方案卡片 " + originalScheme));
                    awaitAnyFor(description("输入方案：" + originalScheme), 15_000);
                    if (!completed) stage = failureStage;
                } catch (Exception | AssertionError restoreError) {
                    if (completed) throw restoreError;
                    stage = failureStage;
                }
            }
        }
    }

    private Predicate<AccessibilityNodeInfo> schemeControl(boolean requireEnabled) {
        return node -> equalsText(PREVIEW_PACKAGE, node.getPackageName())
            && (!requireEnabled || node.isEnabled())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("输入方案：");
    }

    private Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText(PREVIEW_PACKAGE, node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    private Predicate<AccessibilityNodeInfo> handwritingReadyOrDownload() {
        return node -> description("下载中文手写模型；完成后可离线识别").test(node)
            || imeTextContains("此构建不含手写识别").test(node)
            || imeTextContains("在此手写，停笔后选字").test(node);
    }

    private Predicate<AccessibilityNodeInfo> firstCandidate() {
        return node -> equalsText(PREVIEW_PACKAGE, node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().equals("按键 手写候选 1");
    }

    private Predicate<AccessibilityNodeInfo> returnKey() {
        return node -> {
            if (!equalsText(PREVIEW_PACKAGE, node.getPackageName()) || node.getText() == null)
                return false;
            String text = node.getText().toString();
            // 确认 is the return key while a composition is open.
            return "换行".equals(text) || "确认".equals(text) || "前往".equals(text) || "搜索".equals(text)
                || "发送".equals(text) || "下一项".equals(text) || "完成".equals(text)
                || "上一项".equals(text);
        };
    }

    private Predicate<AccessibilityNodeInfo> handwritingInkObserved() {
        return node -> firstCandidate().test(node)
            || imeTextContains("停笔后识别").test(node)
            || imeTextContains("正在识别").test(node)
            || imeTextContains("未识别").test(node)
            || imeTextContains("识别失败").test(node);
    }

    private Predicate<AccessibilityNodeInfo> canvas() {
        return node -> equalsText(PREVIEW_PACKAGE, node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("手写区域；");
    }

    private Rect canvasBounds() {
        AccessibilityNodeInfo canvas = awaitFor(canvas(), 15_000);
        Rect bounds = new Rect();
        canvas.getBoundsInScreen(bounds);
        if (bounds.width() < 100 || bounds.height() < 100)
            throw new AssertionError("Handwriting canvas was too small");
        return bounds;
    }

    private void drawMiddleCharacter(Rect bounds) throws Exception {
        drawStroke(bounds, new float[][] {{0.22f, 0.25f}, {0.22f, 0.68f}});
        drawStroke(bounds, new float[][] {{0.22f, 0.25f}, {0.78f, 0.25f}});
        drawStroke(bounds, new float[][] {{0.78f, 0.25f}, {0.78f, 0.68f}});
        drawStroke(bounds, new float[][] {{0.22f, 0.68f}, {0.78f, 0.68f}});
        drawStroke(bounds, new float[][] {{0.50f, 0.10f}, {0.50f, 0.90f}});
    }

    private void drawStroke(Rect bounds, float[][] points) throws Exception {
        if (points.length != 2) throw new AssertionError("Synthetic stroke must be linear");
        int startX = Math.round(bounds.left + bounds.width() * points[0][0]);
        int startY = Math.round(bounds.top + bounds.height() * points[0][1]);
        int endX = Math.round(bounds.left + bounds.width() * points[1][0]);
        int endY = Math.round(bounds.top + bounds.height() * points[1][1]);
        int request = ++touchRequestSequence;
        File requestFile = new File(getContext().getCacheDir(), TOUCH_REQUEST);
        File ackFile = new File(getContext().getCacheDir(), TOUCH_ACK);
        if (!ackFile.exists()) {
            try (var output = new FileOutputStream(ackFile)) { output.flush(); }
        }
        byte[] payload = (request + " " + startX + " " + startY + " "
            + endX + " " + endY + "\n").getBytes(StandardCharsets.UTF_8);
        try (var output = new FileOutputStream(requestFile)) {
            output.write(payload);
        }
        long deadline = SystemClock.uptimeMillis() + 15_000;
        do {
            if (ackFile.exists() && Integer.toString(request).equals(read(ackFile).trim())) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Physical touch bridge timed out");
    }

    private String read(File file) throws Exception {
        byte[] buffer = new byte[32];
        try (var input = new FileInputStream(file)) {
            int count = input.read(buffer);
            return count <= 0 ? "" : new String(buffer, 0, count, StandardCharsets.UTF_8);
        }
    }

    private void injectKey(int keyCode) {
        long now = SystemClock.uptimeMillis();
        KeyEvent down = new KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0);
        KeyEvent up = new KeyEvent(now, now + 30, KeyEvent.ACTION_UP, keyCode, 0);
        if (!automation.injectInputEvent(down, true) || !automation.injectInputEvent(up, true))
            throw new AssertionError("Hardware key injection failed");
        SystemClock.sleep(150);
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    private AccessibilityNodeInfo findAny(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = findAny(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    private AccessibilityNodeInfo awaitFor(Predicate<AccessibilityNodeInfo> match, long timeoutMillis) {
        long deadline = SystemClock.uptimeMillis() + timeoutMillis;
        do {
            AccessibilityNodeInfo found = findVisible(match);
            if (found != null) return found;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected handwriting UI state was not observed");
    }

    private AccessibilityNodeInfo awaitAnyFor(Predicate<AccessibilityNodeInfo> match,
                                               long timeoutMillis) {
        long deadline = SystemClock.uptimeMillis() + timeoutMillis;
        do {
            AccessibilityNodeInfo found = findAny(match);
            if (found != null) return found;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected handwriting control was not observed");
    }

    private void accessibleClick(Predicate<AccessibilityNodeInfo> match) {
        AccessibilityNodeInfo target = awaitAnyFor(match, 15_000);
        if (!target.isVisibleToUser()) {
            target.performAction(
                AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
            SystemClock.sleep(250);
            target = awaitAnyFor(match, 15_000);
        }
        if (!target.performAction(AccessibilityNodeInfo.ACTION_CLICK))
            throw new AssertionError("Synthetic control action failed");
        SystemClock.sleep(200);
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { /* Discard synthetic command output. */ }
        }
    }
}
