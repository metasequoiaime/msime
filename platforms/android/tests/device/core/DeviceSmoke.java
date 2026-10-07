package app.msime.android.test;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.accessibilityservice.AccessibilityService;
import android.app.Activity;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.Intent;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.InputDevice;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** Device-only synthetic acceptance; reads both editor and IME accessibility windows. */
public class DeviceSmoke extends Instrumentation {
    protected UiAutomation automation;
    protected String stage = "launch";
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            automation = getUiAutomation();
            AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
            automation.setServiceInfo(info);
            runChecks();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: " + successDescription() + "\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            // Only fixed test-stage messages, never serialize editor contents.
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + error.getClass().getSimpleName() + ")" + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
    protected String successDescription() {
        return "phrase commit, Traditional Chinese display/commit, deletion and password direct input";
    }
    protected void runChecks() throws Exception {
            Intent intent = new Intent(getTargetContext(), EditorActivity.class);
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
            startActivitySync(intent);
            stage = "plain focus";
            tap(field("msime-test-plain"));
            stage = "typing";
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            stage = "phrase composition";
            await(field("msime-test-plain").and(node -> equalsText("nihao", node.getText())));
            stage = "phrase commit key";
            tap(key("空格"));
            stage = "phrase commit result";
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
            stage = "deletion";
            tap(key("⌫"));
            await(field("msime-test-plain").and(node -> equalsText("你", node.getText())));
            // 繁体输出 is a setting rather than a toolbar key, the way Apple has it: it moved out
            // of the shortcut bar and into 更多, so this drives it there.
            stage = "simplified output baseline";
            tap(key("更多"));
            await(toolPanel());
            AccessibilityNodeInfo script = await(tool("繁体输出"));
            if (equalsText("已开启", script.getStateDescription())) {
                tap(tool("繁体输出"));
                await(toolWithState("繁体输出", "已关闭"));
            }
            stage = "traditional output switch";
            tap(tool("繁体输出"));
            await(toolWithState("繁体输出", "已开启"));
            tap(tool("返回键盘"));
            await(key("n").and(AccessibilityNodeInfo::isClickable));
            stage = "traditional candidate display";
            for (String key : new String[] {"s", "h", "u", "r", "u", "f", "a"}) tap(key(key));
            await(imeTextContains("輸入法"));
            stage = "traditional commit key";
            tap(key("空格"));
            stage = "traditional commit result";
            await(field("msime-test-plain").and(node -> equalsText("你輸入法", node.getText())));
            stage = "restore simplified output";
            tap(key("更多"));
            await(toolPanel());
            tap(tool("繁体输出"));
            await(toolWithState("繁体输出", "已关闭"));
            tap(tool("返回键盘"));
            await(key("n").and(AccessibilityNodeInfo::isClickable));
            stage = "password focus action";
            tap(field("msime-test-password"));
            stage = "password field focus";
            await(field("msime-test-password").and(AccessibilityNodeInfo::isFocused));
            stage = "password direct mode";
            await(imeTextContains("直接输入"));
            stage = "password direct input";
            tap(key("n"));
            await(field("msime-test-password").and(node -> node.getText() != null && node.getText().length() == 1));

            stage = "view-hide composition";
            tap(field("msime-test-plain"));
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            // The field still holds 你輸入法 from the traditional stage, and getText() includes the
            // composing region, so this is that text with the pinyin still being composed on it.
            await(field("msime-test-plain").and(node -> equalsText("你輸入法nihao", node.getText())));
            if (!automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK))
                throw new AssertionError("Keyboard hide action failed");
            SystemClock.sleep(500);
            tap(field("msime-test-plain"));
            await(field("msime-test-plain").and(node -> equalsText("你輸入法你好", node.getText())));
            if (findAnyVisibleImeNode(imeText("nihao")) != null)
                throw new AssertionError("Hidden keyboard retained composition");
    }
    protected Predicate<AccessibilityNodeInfo> field(String description) {
        return node -> equalsText("app.msime.android.test", node.getPackageName()) && equalsText(description, node.getContentDescription());
    }
    /**
     * The key that types `text`, whatever case it is drawn in.
     *
     * <p>A Chinese keyboard draws its 26 letter keys in caps while still sending the lowercase
     * letter, so a letter key is identified by the letter and not by its face. Which case is drawn
     * is `LetterKeyFacePolicy`'s contract and has its own host coverage; asserting it again from
     * here only made these cases fail the moment the faces became correct. Every other key -- 空格,
     * 简, 繁, ⌫ -- still matches exactly.
     */
    protected Predicate<AccessibilityNodeInfo> key(String text) {
        boolean letter = text.length() == 1 && Character.isLetter(text.charAt(0))
            && text.charAt(0) < 128;
        return node -> equalsText("app.msime.android", node.getPackageName())
            && (letter ? node.getText() != null && text.equalsIgnoreCase(node.getText().toString())
                : equalsText(text, node.getText()));
    }
    /**
     * A control identified by the start of its accessibility description.
     *
     * <p>Several shortcut entries put their current value in both the face and the description --
     * the scheme entry reads `拼26` / `输入方案：全拼 26 键`, the skin entry names the skin in use --
     * so neither is an identity. The stable part is the prefix.
     */
    protected Predicate<AccessibilityNodeInfo> described(String description) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(description, node.getContentDescription());
    }

    protected Predicate<AccessibilityNodeInfo> describedPrefix(String prefix) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith(prefix);
    }
    protected Predicate<AccessibilityNodeInfo> toolPanel() {
        return described("更多工具");
    }

    protected Predicate<AccessibilityNodeInfo> tool(String description) {
        return described(description);
    }

    protected Predicate<AccessibilityNodeInfo> toolWithState(String description, String state) {
        return tool(description).and(node -> equalsText(state, node.getStateDescription()));
    }

    protected Predicate<AccessibilityNodeInfo> scriptState() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.isEnabled() && (equalsText("简", node.getText()) || equalsText("繁", node.getText()));
    }
    protected Predicate<AccessibilityNodeInfo> imeTextContains(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getText() != null && node.getText().toString().contains(text);
    }
    private Predicate<AccessibilityNodeInfo> imeText(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(text, node.getText());
    }
    private AccessibilityNodeInfo findAnyVisibleImeNode(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }
    protected static boolean equalsText(String expected, CharSequence actual) { return actual != null && expected.contentEquals(actual); }
    protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node == null) return null;
        if (node.isVisibleToUser() && match.test(node)) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = find(node.getChild(index), match);
            if (found != null) return found;
        }
        return null;
    }
    /** Matches anywhere in the tree, including nodes scrolled outside the visible area. */
    protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node,
                                           Predicate<AccessibilityNodeInfo> match) {
        if (node == null) return null;
        if (match.test(node)) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = findAny(node.getChild(index), match);
            if (found != null) return found;
        }
        return null;
    }
    protected AccessibilityNodeInfo await(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo found = find(window.getRoot(), match);
                if (found != null) return found;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected synthetic UI state was not observed");
    }
    protected AccessibilityNodeInfo awaitAny(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo found = findAny(window.getRoot(), match);
                if (found != null) return found;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected synthetic control was not observed");
    }
    protected void tap(Predicate<AccessibilityNodeInfo> match) throws java.util.concurrent.TimeoutException {
        AccessibilityNodeInfo target = awaitAny(match);
        automation.waitForIdle(500, 5000);
        target = awaitAny(match);
        // Android 15 can reject synthetic coordinates over IME and instrumentation windows.
        // Accessibility click still invokes the real product control and InputConnection path.
        if (equalsText("app.msime.android", target.getPackageName())
                && target.isClickable()) {
            if (!target.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("Synthetic control action failed");
            SystemClock.sleep(150);
            return;
        }
        if (!target.isVisibleToUser()) target = await(match);
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        long now = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(now, now, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY(), 0);
        MotionEvent up = MotionEvent.obtain(now, now + 50, MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY(), 0);
        down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            boolean pressed = automation.injectInputEvent(down, true);
            boolean released = automation.injectInputEvent(up, true);
            if (!pressed || !released) throw new AssertionError("Touch injection failed");
        } finally { down.recycle(); up.recycle(); }
        SystemClock.sleep(150);
    }
    /**
     * Page the function panel until the tile described `description` is on screen.
     *
     * <p>The panel is three pages of 4 × 2 tiles; the grid (the panel's only scrollable child) exposes the forward and backward scroll actions, so this walks forward to the last page and back again rather than assuming which page holds a tile.
     */
    protected AccessibilityNodeInfo showTool(String description) throws java.util.concurrent.TimeoutException {
        for (int attempt = 0; attempt < 6; attempt++) {
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo found = find(window.getRoot(), tool(description));
                if (found != null) return found;
            }
            AccessibilityNodeInfo grid = awaitAny(node -> equalsText("app.msime.android", node.getPackageName())
                && node.isScrollable() && node.getParent() != null
                && equalsText("更多工具", node.getParent().getContentDescription()));
            int action = attempt < 3 ? AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                : AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD;
            grid.performAction(action);
            SystemClock.sleep(400);
        }
        return await(tool(description));
    }
    protected void tapSymbol(String symbol) throws java.util.concurrent.TimeoutException {
        // The punctuation sits on the 123 layer (the third row holds the five Chinese or English marks), and that layer's bottom-left key goes back to the letters: it reads 拼音 or ABC, so it is found by its description.
        tap(key("123"));
        // A symbol key in Chinese mode wears its Chinese face whatever the engine is configured to
        // insert -- the face follows the mode, the inserted mark follows the punctuation setting,
        // exactly as on Apple. Accept either face and let the caller assert what was inserted.
        String chinese = app.msime.android.ChineseSymbolFaces.face(symbol, true);
        tap(key(symbol).or(key(chinese)));
        tap(described("切换到字母键盘"));
    }
}
