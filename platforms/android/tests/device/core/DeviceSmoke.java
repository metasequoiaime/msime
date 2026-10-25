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
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.InputDevice;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.function.Predicate;

/** Device-only synthetic acceptance; reads both editor and IME accessibility windows. */
public class DeviceSmoke extends Instrumentation {
    protected UiAutomation automation;
    protected String stage = "launch";
    private boolean pinyinStartup;
    private boolean pinyinHideResume;

    /** 执行设备测试命令并排空输出，避免未读取的管道阻塞命令。 */
    protected final void shellDiscard(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    /** 重新注册并启用被测输入法，确认系统的异步回退没有覆盖选择。 */
    protected final void restartIme() throws Exception {
        restartIme("app.msime.android/.MSIMEInputService");
    }

    /** 重新注册指定输入法，确认系统的异步回退没有覆盖选择。 */
    protected final void restartIme(String ime) throws Exception {
        shellDiscard("ime disable " + ime);
        shellDiscard("ime enable " + ime);
        shellDiscard("ime set " + ime);
        // 禁用当前输入法会异步切回系统键盘；命令返回不代表回退已经结束。
        // 连续确认选中水杉再启动编辑器，避免把系统键盘当作产品的空窗口。
        long deadline = SystemClock.uptimeMillis() + 15000;
        int settled = 0;
        do {
            if (selectedIme(ime)) {
                if (++settled >= 4) return;
            } else {
                settled = 0;
                shellDiscard("ime set " + ime);
            }
            SystemClock.sleep(250);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Test input method selection did not settle after restart");
    }

    /**
     * 等系统放弃被强停的输入法连接，再开始本套件。
     *
     * <p>Android 11 以下 smoke.sh 不能附着到已运行的产品进程，以产品为目标的上一个套件结束时，系统连同 `:ime` 一起强停产品。输入法管理服务收到断开时把它当作服务崩溃，记下这一刻、等系统重启服务（`InputMethodManagerService.onServiceDisconnected` 把 `mLastBindTime` 设为现在），而强停已经移除了服务，系统不会再拉起它。断开后 `TIME_TO_RECONNECT`（3 秒）以内，编辑器取得焦点只得到「等待输入法绑定」，弹键盘的请求被丢掉；`ime disable/enable/set` 选回同一个输入法也不会重绑，因为当前绑定的 id 没变。下一个套件若在这 3 秒内打开编辑器，键盘就一直不出现（CI 上 API 28 的「deferred composition length 1 … IME showed []」）。过了这段时间，焦点请求会让系统断开重连，所以这里在看到「有连接、没有输入法接口」时等到它恢复或满 3 秒半。dumpsys 只读输入法绑定状态，不读编辑器内容。
     */
    private void outlastDeadImeConnection() throws Exception {
        if (Build.VERSION.SDK_INT >= 31) return;
        long deadline = SystemClock.uptimeMillis() + 3500;
        while (SystemClock.uptimeMillis() < deadline) {
            String state = shellOutput("dumpsys input_method");
            if (!(state.contains("mHaveConnect=true") && state.contains("mCurMethod=null"))) return;
            SystemClock.sleep(250);
        }
    }

    private String shellOutput(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[4096];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    /** 只读取默认输入法标识，不读取编辑器内容或用户设置日志。 */
    private boolean selectedIme(String ime) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand("settings get secure default_input_method"))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[256];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return ime.equals(
                new String(output.toByteArray(), StandardCharsets.UTF_8).trim());
        }
    }

    /** 用原子写入发布测试偏好；失败时保留原文件。 */
    protected final void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try {
            output.write(contents);
            target.finishWrite(output);
        } catch (Exception error) {
            target.failWrite(output);
            throw error;
        }
    }

    /** `uinput` 接上的键盘的标准输入，往里写注入命令就是在那块键盘上按键；没有接时为空。 */
    protected FileOutputStream uinputInput;

    @Override public void onCreate(Bundle arguments) {
        pinyinStartup = arguments != null && "true".equals(arguments.getString("pinyin-startup"));
        pinyinHideResume = arguments != null && "true".equals(arguments.getString("pinyin-hide-resume"));
        super.onCreate(arguments);
        start();
    }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            automation = getUiAutomation();
            AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
            automation.setServiceInfo(info);
            outlastDeadImeConnection();
            if (pinyinStartup) PinyinStartupDeviceSmoke.runChecks(this, pinyinHideResume);
            else runChecks();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: " + successDescription() + "\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            // Only fixed test-stage messages, never serialize editor contents.
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + error.getClass().getSimpleName() + ")" + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
    protected String successDescription() {
        if (pinyinStartup) return "pinyin edition prepares without launcher, composes and commits in the same editor";
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
            await(field("msime-test-plain").and(node -> equalsText("ni'hao", node.getText())));
            stage = "phrase commit key";
            tap(key("空格"));
            stage = "phrase commit result";
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
            stage = "deletion";
            tap(key("⌫"));
            await(field("msime-test-plain").and(node -> equalsText("你", node.getText())));
            stage = "limited composition focus";
            final String limitedPrefix = "甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉戌亥天地玄黄";
            tap(field("msime-test-limited"));
            AccessibilityNodeInfo limited = await(field("msime-test-limited").and(AccessibilityNodeInfo::isFocused));
            Bundle selection = new Bundle();
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, limitedPrefix.length());
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, limitedPrefix.length());
            if (!limited.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection))
                throw new AssertionError("Synthetic limited editor selection failed");
            await(field("msime-test-limited").and(node -> node.getTextSelectionStart() == limitedPrefix.length()
                && node.getTextSelectionEnd() == limitedPrefix.length()));
            await(field("msime-test-limited").and(node -> equalsText(limitedPrefix, node.getText())));
            stage = "limited composition typing";
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            await(field("msime-test-limited").and(node -> equalsText(limitedPrefix, node.getText())));
            await(imeText("nihao"));
            // 截短后继续输入；新建的 Sink 不能重新把已隐藏的组字写入编辑器。
            stage = "limited composition continues while hidden";
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            await(field("msime-test-limited").and(node -> equalsText(limitedPrefix, node.getText())));
            await(imeText("nihaonihao"));
            stage = "limited composition commit";
            tap(key("空格"));
            await(field("msime-test-limited").and(node -> equalsText(limitedPrefix + "你好你好", node.getText())));
            // 后续繁体输出仍验证原来的普通输入框，也验证隐藏状态不跨编辑器泄漏。
            tap(field("msime-test-plain"));
            await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
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
            await(field("msime-test-plain").and(node -> equalsText("你輸入法ni'hao", node.getText())));
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
            // 旧系统会缓存第一拍的读音节点；刷新后再断言当前组字，避免把 n 当成 nihao 的实际状态。
            && (Build.VERSION.SDK_INT >= 34 || node.refresh())
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
        // 旧系统会保留已经变化的文本、可见性与子树；断言前从进程重新读取。
        if (node == null || Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
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
        if (node == null || Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
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
            dropStaleNodes();
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
            dropStaleNodes();
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
        if (target.getExtras().containsKey("msime-test-ime-requests-settled")) {
            String description = target.getContentDescription().toString();
            String packageName = target.getPackageName().toString();
            await(node -> equalsText(packageName, node.getPackageName())
                && equalsText(description, node.getContentDescription()) && node.isFocused()
                && node.getExtras().getBoolean("msime-test-ime-requests-settled"));
        }
    }
    // ---- 实体键盘 ----
    // 宿主只把来自非虚拟、字母型键盘设备的按下当成实体键盘打字（`HardwareKeyboardModePolicy.physicalTyping`），`adb shell input` 的虚拟设备不算，而专用 AVD 自带的 `qwerty2` 在 `hw.keyboard=no` 时不是字母型。所以安卓 12（API 31）起用系统的 `uinput` 命令接上一块 USB 键盘：它是真的 evdev 设备，系统像对待蓝牙、USB 键盘一样报告「接着实体键盘」，测试结束关掉 `uinput` 就是拔掉。更早的系统只能用 `hw.keyboard=yes` 的 AVD 自带键盘，没有就失败并说明原因。

    /** 一块非虚拟、字母型的键盘：`uinput` 接上的那块，或 `hw.keyboard=yes` 的 AVD 自带的那块。设备接上后要过一会儿才出现在列表里。 */
    protected static int physicalKeyboard() {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            for (int id : InputDevice.getDeviceIds()) {
                InputDevice device = InputDevice.getDevice(id);
                if (device != null && !device.isVirtual()
                        && device.getKeyboardType() == InputDevice.KEYBOARD_TYPE_ALPHABETIC) return id;
            }
            SystemClock.sleep(200);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("No physical alphabetic keyboard: API " + Build.VERSION.SDK_INT
            + " before 31 needs an AVD with hw.keyboard=yes");
    }

    /**
     * 用 `uinput` 接上一块 USB 键盘，返回命令的标准输出、标准输入和标准错误；标准输入一直开着，键盘就一直接着。键盘总有 26 个字母键和空格，`extraKeyCodes` 是套件另外要按的键（Android 键码），一并登记。
     */
    protected ParcelFileDescriptor[] plugKeyboard(int... extraKeyCodes) throws Exception {
        ParcelFileDescriptor[] streams = automation.executeShellCommandRwe("uinput -");
        StringBuilder keys = new StringBuilder();
        // Linux 键码：KEY_Q..KEY_P 是 16..25，KEY_A..KEY_L 是 30..38，KEY_Z..KEY_M 是 44..50，空格 57。
        for (int[] range : new int[][] {{16, 25}, {30, 38}, {44, 50}, {57, 57}}) {
            for (int code = range[0]; code <= range[1]; code++) keys.append(keys.length() == 0 ? "" : ",").append(code);
        }
        for (int keyCode : extraKeyCodes) keys.append(',').append(linuxKey(keyCode));
        String register = "{\"id\":1,\"command\":\"register\",\"name\":\"MSIME test keyboard\",\"vid\":6353,"
            + "\"pid\":26514,\"bus\":\"usb\",\"configuration\":[{\"type\":\"UI_SET_EVBIT\",\"data\":[1]},"
            + "{\"type\":\"UI_SET_KEYBIT\",\"data\":[" + keys + "]}]}\n";
        uinputInput = new FileOutputStream(streams[1].getFileDescriptor());
        uinputInput.write(register.getBytes(StandardCharsets.UTF_8));
        uinputInput.flush();
        return streams;
    }

    /** 关掉 `uinput` 的标准输入：命令退出，键盘拔掉。 */
    protected void unplugKeyboard(ParcelFileDescriptor[] streams) throws Exception {
        uinputInput = null;
        for (ParcelFileDescriptor stream : streams) stream.close();
    }

    /** Android 键码对应的 Linux 键码：字母、数字行、空格、回车、Esc 和左 Ctrl。 */
    protected static int linuxKey(int keyCode) {
        if (keyCode >= KeyEvent.KEYCODE_1 && keyCode <= KeyEvent.KEYCODE_9) return keyCode - KeyEvent.KEYCODE_1 + 2;
        return switch (keyCode) {
            case KeyEvent.KEYCODE_0 -> 11;
            case KeyEvent.KEYCODE_Q -> 16;
            case KeyEvent.KEYCODE_W -> 17;
            case KeyEvent.KEYCODE_E -> 18;
            case KeyEvent.KEYCODE_R -> 19;
            case KeyEvent.KEYCODE_T -> 20;
            case KeyEvent.KEYCODE_Y -> 21;
            case KeyEvent.KEYCODE_U -> 22;
            case KeyEvent.KEYCODE_I -> 23;
            case KeyEvent.KEYCODE_O -> 24;
            case KeyEvent.KEYCODE_P -> 25;
            case KeyEvent.KEYCODE_A -> 30;
            case KeyEvent.KEYCODE_S -> 31;
            case KeyEvent.KEYCODE_D -> 32;
            case KeyEvent.KEYCODE_F -> 33;
            case KeyEvent.KEYCODE_G -> 34;
            case KeyEvent.KEYCODE_H -> 35;
            case KeyEvent.KEYCODE_J -> 36;
            case KeyEvent.KEYCODE_K -> 37;
            case KeyEvent.KEYCODE_L -> 38;
            case KeyEvent.KEYCODE_Z -> 44;
            case KeyEvent.KEYCODE_X -> 45;
            case KeyEvent.KEYCODE_C -> 46;
            case KeyEvent.KEYCODE_V -> 47;
            case KeyEvent.KEYCODE_B -> 48;
            case KeyEvent.KEYCODE_N -> 49;
            case KeyEvent.KEYCODE_M -> 50;
            case KeyEvent.KEYCODE_SPACE -> 57;
            case KeyEvent.KEYCODE_ENTER -> 28;
            case KeyEvent.KEYCODE_ESCAPE -> 1;
            case KeyEvent.KEYCODE_CTRL_LEFT -> 29;
            default -> throw new AssertionError("No Linux key code for " + KeyEvent.keyCodeToString(keyCode));
        };
    }

    /** 逐个按下、松开这些键：接了 `uinput` 键盘时从那块键盘的 evdev 打进去，走真实的输入读取链路；否则以 AVD 自带键盘的设备号注入。 */
    protected void type(int device, int... keyCodes) throws Exception {
        for (int keyCode : keyCodes) {
            if (uinputInput != null) {
                int code = linuxKey(keyCode);
                inject("1," + code + ",1,0,0,0,1," + code + ",0,0,0,0");
                continue;
            }
            long now = SystemClock.uptimeMillis();
            injectKey(device, now, KeyEvent.ACTION_DOWN, keyCode, 0);
            injectKey(device, now + 30, KeyEvent.ACTION_UP, keyCode, 0);
            SystemClock.sleep(80);
        }
    }

    /** 按着左 Ctrl 按一下 `keyCode`：Ctrl 按下、键按下、键松开、Ctrl 松开，和真人按组合键的顺序一样。 */
    protected void ctrlChord(int device, int keyCode) throws Exception {
        if (uinputInput != null) {
            int ctrl = linuxKey(KeyEvent.KEYCODE_CTRL_LEFT);
            int code = linuxKey(keyCode);
            inject("1," + ctrl + ",1,0,0,0,1," + code + ",1,0,0,0,1," + code + ",0,0,0,0,1," + ctrl + ",0,0,0,0");
            return;
        }
        int meta = KeyEvent.META_CTRL_ON | KeyEvent.META_CTRL_LEFT_ON;
        long now = SystemClock.uptimeMillis();
        injectKey(device, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_CTRL_LEFT, meta);
        injectKey(device, now + 20, KeyEvent.ACTION_DOWN, keyCode, meta);
        injectKey(device, now + 50, KeyEvent.ACTION_UP, keyCode, meta);
        injectKey(device, now + 70, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_CTRL_LEFT, 0);
        SystemClock.sleep(80);
    }

    /** 往 `uinput` 键盘写一串 evdev 事件（类型、键码、值三个一组，EV_KEY 后面跟 EV_SYN），写完等系统读走。 */
    private void inject(String events) throws Exception {
        String command = "{\"id\":1,\"command\":\"inject\",\"events\":[" + events + "]}\n";
        uinputInput.write(command.getBytes(StandardCharsets.UTF_8));
        uinputInput.flush();
        SystemClock.sleep(120);
    }

    private void injectKey(int device, long time, int action, int keyCode, int metaState) {
        KeyEvent event = new KeyEvent(time, time, action, keyCode, 0, metaState, device, 0, 0,
            InputDevice.SOURCE_KEYBOARD);
        if (!automation.injectInputEvent(event, true)) throw new AssertionError("Key injection failed");
    }

    /**
     * 翻功能菜单，直到描述为 `description` 的条目出现在屏幕上。
     *
     * <p>功能菜单最多两页，每页四列三行；唯一可滚动的网格提供前后翻页动作，因此遍历到末页再返回，不假设条目的固定页码。
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
        // 中文 123 默认显示字面符号侧栏；扩展符号页的标点键才跟随中文标点设置。
        tap(key("123"));
        if (java.util.Set.of(",", ".", "?", "!").contains(symbol)) tap(key("#+="));
        // 同时接受全角与半角键面，由调用方断言实际插入内容。
        String chinese = app.msime.android.ChineseSymbolFaces.face(symbol, true);
        tap(key(symbol).or(key(chinese)));
        tap(described("切换到字母键盘"));
    }
}
