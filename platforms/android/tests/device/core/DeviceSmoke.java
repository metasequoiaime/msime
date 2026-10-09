package app.msime.android.test;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.accessibilityservice.AccessibilityService;
import android.app.Activity;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.Intent;
import android.graphics.Rect;
import android.os.Build;
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
        if (Build.VERSION.SDK_INT < 30) return "phrase commit and deletion (API " + Build.VERSION.SDK_INT + ")";
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
            // 安卓 11（API 30）以下到此为止：在 API 28 模拟器上，「更多」面板在屏幕上正常打开、点按也生效，但 UiAutomation 的无障碍树里拿不到面板和它的任何磁贴，后面的繁体输出流程没法在那里驱动。打开应用、准备词库、拼音上屏和删除这条主路径仍在 API 28 上检查。
            if (Build.VERSION.SDK_INT < 30) return;
            // 繁体输出 is a setting rather than a toolbar key, the way Apple has it: it moved out
            // of the shortcut bar and into 更多, so this drives it there.
            stage = "simplified output baseline: more key";
            tap(key("更多"));
            stage = "simplified output baseline: tool panel";
            await(toolPanel());
            stage = "simplified output baseline: traditional tool";
            AccessibilityNodeInfo script = await(tool("繁体输出"));
            stage = "simplified output baseline";
            if (equalsText("已开启", switchState(script))) {
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
        // 组字时读音行可以点（点中的字母前放组字光标，#5613），它的文字可能就是一个字母（打了 n，读音行是 n）；只认按钮，不然会点到读音行上去。
        return node -> equalsText("app.msime.android", node.getPackageName())
            && (letter ? node.getText() != null && text.equalsIgnoreCase(node.getText().toString())
                    && equalsText("android.widget.Button", node.getClassName())
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
        return tool(description).and(node -> equalsText(state, switchState(node)));
    }

    /**
     * 控件的状态文本。安卓 11（API 30）起读应用设的无障碍状态描述；更早的系统没有这个接口，应用在那里只用 selected 表达开关和选中、用 enabled 表达不可用（见 `FunctionPanelView`、`ImeFunctionPanel`），这里折算成同样的文本，让这些检查在 minSdk 28 的模拟器上也能跑。
     */
    protected static CharSequence state(AccessibilityNodeInfo node, String selected, String unselected) {
        if (Build.VERSION.SDK_INT >= 30) return node.getStateDescription();
        if (!node.isEnabled()) return "不可用";
        return node.isSelected() ? selected : unselected;
    }

    protected static CharSequence switchState(AccessibilityNodeInfo node) {
        return state(node, "已开启", "已关闭");
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
        throw new AssertionError("Expected synthetic UI state was not observed; IME showed " + imeTexts());
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
        throw new AssertionError("Expected synthetic control was not observed; IME showed " + imeTexts());
    }

    /** 等控件的位置和大小连续两次读到一样：面板打开、翻页有动画，动画中量到的尺寸是过渡值。 */
    protected AccessibilityNodeInfo awaitStableBounds(Predicate<AccessibilityNodeInfo> match) {
        Rect previous = null;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            AccessibilityNodeInfo node = await(match);
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            if (bounds.equals(previous)) return node;
            previous = bounds;
            SystemClock.sleep(250);
        }
        throw new AssertionError("Bounds never settled; IME showed " + imeTexts());
    }

    /** 超时时输入法窗口里看得见的文字（键面、候选、提示），最多 30 条：只读输入法自己的节点，不读编辑器内容。 */
    protected String imeTexts() {
        java.util.List<String> texts = new java.util.ArrayList<>();
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            collectImeTexts(window.getRoot(), texts);
            if (texts.size() >= 30) break;
        }
        return texts.toString();
    }

    private static void collectImeTexts(AccessibilityNodeInfo node, java.util.List<String> texts) {
        if (node == null || texts.size() >= 30) return;
        if (equalsText("app.msime.android", node.getPackageName())) {
            CharSequence text = node.getText() != null ? node.getText() : node.getContentDescription();
            if (text != null && text.length() > 0) texts.add(text.toString());
        }
        for (int index = 0; index < node.getChildCount(); index++) collectImeTexts(node.getChild(index), texts);
    }
    /**
     * 丢掉 UiAutomation 缓存里的节点，下一次查询从输入法进程重新取。
     *
     * <p>「更多」面板每次切换开关都整页重建磁贴（`ImeFunctionPanel.renderMoreTools` 先 removeAllViews 再挂新磁贴）。重建发出的内容变化事件和查询时预取的子树交错到达时，缓存里可能留下已经拆掉的旧磁贴；之后界面静止、不再有事件来冲掉它，只等空闲再查，拿到的还是同一个过期节点，performAction 连续三次返回 false。2026-10-09 CI 的 API 35 上，「按键音」第二次点击（`more tools sound update`）在 PR 和 develop 上反复失败，失败时整个 MoreTools 套件不到五秒就结束，三次重试几乎是立刻用完的。tap 最后一次失败时报告节点是否还在（`node live` / `node gone`），用来确认这个判断。`clearCache` 从安卓 14（API 34）起才有，更早的系统维持原来的重试。
     */
    protected void dropStaleNodes() {
        if (Build.VERSION.SDK_INT >= 34) automation.clearCache();
    }

    protected void tap(Predicate<AccessibilityNodeInfo> match) throws java.util.concurrent.TimeoutException {
        AccessibilityNodeInfo target = awaitAny(match);
        automation.waitForIdle(500, 5000);
        target = awaitAny(match);
        // Android 15 can reject synthetic coordinates over IME and instrumentation windows.
        // Accessibility click still invokes the real product control and InputConnection path.
        if (equalsText("app.msime.android", target.getPackageName())
                && target.isClickable()) {
            // 查到节点和点下去之间，面板可能已经重新绑定（开关刷新状态、翻页动画收尾），performAction 对过期节点返回 false，点击并没有发生。所以等界面空闲后重新查一次再点，连续三次都不成才算失败：真不接受点击的控件照样失败，不会被重试掩盖，也不会把开关点两次。
            for (int attempt = 1; ; attempt++) {
                if (target.performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                    SystemClock.sleep(150);
                    return;
                }
                if (attempt == 3) {
                    throw new AssertionError("Synthetic control action failed ("
                        + (target.refresh() ? "node live" : "node gone") + ")");
                }
                dropStaleNodes();
                automation.waitForIdle(500, 5000);
                target = awaitAny(match);
            }
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
