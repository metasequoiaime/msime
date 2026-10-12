package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.graphics.Rect;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/**
 * 「物理键盘时候选栏跟随光标」（#6792）的设备验收：打开本地设置后，用实体键盘打字时候选栏出现在输入框旁边而不是屏幕底部；空闲时什么也不显示，点输入框落到应用上；不报告光标位置的输入框退回停在底部的候选条；组字折行时候选栏在最后一行下方；空闲时输入法窗口收起，按一下返回键应用就返回。
 *
 * <p>宿主只把来自非虚拟、字母型键盘设备的按下当成实体键盘打字（`HardwareKeyboardModePolicy.physicalTyping`），`adb shell input` 的虚拟设备不算，而专用 AVD 自带的 `qwerty2` 在 `hw.keyboard=no` 时不是字母型。所以安卓 12（API 31）起用系统的 `uinput` 命令接上一块 USB 键盘：它是真的 evdev 设备，系统像对待蓝牙、USB 键盘一样报告「接着实体键盘」，测试结束关掉 `uinput` 就是拔掉。更早的系统只能用 `hw.keyboard=yes` 的 AVD 自带键盘，没有就失败并说明原因。按键以这块键盘的设备号注入。
 */
public final class CaretCandidatesDeviceSmoke extends DeviceSmoke {
    /** {@code AndroidLocalSettings.FILE_NAME} 与 {@code HARDWARE_CARET_CANDIDATES}；设备套件不编译宿主的设置类，这里写同样的字面值。 */
    private static final String LOCAL_SETTINGS_FILE = "android-settings.json";
    private static final String CARET_CANDIDATES_SETTING = "platform.android.hardware_caret_candidates";

    @Override protected String successDescription() {
        return "candidate strip beside the caret for physical typing, nothing drawn while idle, the bottom strip for an editor without cursor anchors, the strip below a wrapped composition, and one back press leaving an idle editor";
    }

    @Override protected void runChecks() throws Exception {
        ParcelFileDescriptor[] uinput = null;
        File root = getTargetContext().getFilesDir();
        File localSettings = new File(new File(new File(root, "bootstrap"), "state"), LOCAL_SETTINGS_FILE);
        byte[] originalLocal = localSettings.exists() ? Files.readAllBytes(localSettings.toPath()) : null;
        try {
            stage = "physical keyboard device";
            if (Build.VERSION.SDK_INT >= 31) uinput = plugKeyboard();
            int keyboard = physicalKeyboard();

            stage = "enable caret candidates";
            if (!localSettings.getParentFile().isDirectory() && !localSettings.getParentFile().mkdirs())
                throw new AssertionError("Local settings directory unavailable");
            publish(localSettings, new JSONObject().put("version", 1)
                .put("settings", new JSONObject().put(CARET_CANDIDATES_SETTING, true))
                .toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
            if (Build.VERSION.SDK_INT >= 30) {
                stage = "editor IME requests settled";
                await(field("msime-test-plain").and(node ->
                    equalsText("ime-requests-settled", node.getStateDescription())));
            }

            // 接着实体键盘时，输入框一获得焦点宿主就要了光标位置，第一次按键的组字直接出现在光标旁。
            stage = "caret strip while composing";
            type(keyboard, KeyEvent.KEYCODE_N, KeyEvent.KEYCODE_I, KeyEvent.KEYCODE_H, KeyEvent.KEYCODE_A,
                KeyEvent.KEYCODE_O);
            Rect plain = bounds(field("msime-test-plain"));
            Rect candidate = bounds(imeCandidate("你好"));
            int screenHeight = getTargetContext().getResources().getDisplayMetrics().heightPixels;
            float density = getTargetContext().getResources().getDisplayMetrics().density;
            // 候选栏放在光标所在行下方：顶边落在输入框里或紧挨着它下面，不在屏幕下半。
            if (candidate.top < plain.top || candidate.top > plain.bottom + Math.round(160 * density)
                    || candidate.bottom > screenHeight / 2)
                throw new AssertionError("Candidate strip is not beside the caret: candidate " + candidate
                    + ", editor " + plain + ", screen height " + screenHeight);

            stage = "caret strip commit";
            type(keyboard, KeyEvent.KEYCODE_SPACE);
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            // 空闲时候选栏和工具栏都不画：没有展开键，也没有候选。
            stage = "idle draws nothing";
            SystemClock.sleep(500);
            if (findVisible(described("显示软键盘")) != null || findVisible(imeCandidate("你好")) != null)
                throw new AssertionError("Idle caret mode still draws the strip; IME showed " + imeTexts());

            // 点另一个输入框：空闲时输入法窗口已收起，窗口还没收起的那一刻它也是透明、不接收触摸的，点击落到应用上。这个输入框不报告光标位置，候选栏退回底部。
            stage = "no cursor anchor focus";
            tap(field("msime-test-no-cursor-anchor"));
            await(field("msime-test-no-cursor-anchor").and(AccessibilityNodeInfo::isFocused));
            stage = "bottom strip without cursor anchors";
            type(keyboard, KeyEvent.KEYCODE_N, KeyEvent.KEYCODE_I, KeyEvent.KEYCODE_H, KeyEvent.KEYCODE_A,
                KeyEvent.KEYCODE_O);
            candidate = bounds(imeCandidate("你好"));
            if (candidate.top < screenHeight / 2)
                throw new AssertionError("Editor without cursor anchors did not get the bottom strip: candidate "
                    + candidate + ", screen height " + screenHeight);
            type(keyboard, KeyEvent.KEYCODE_SPACE);
            await(field("msime-test-no-cursor-anchor").and(node -> equalsText("你好", node.getText())));

            // 组字折到下一行：候选栏放在组字最后一行下方，不压在折下去的组字和插入点上。这个输入框 120 dp 宽，20 个字母的拼音一行放不下。
            stage = "wrapped composition focus";
            tap(field("msime-test-wrapping"));
            await(field("msime-test-wrapping").and(AccessibilityNodeInfo::isFocused));
            stage = "strip below a wrapped composition";
            for (int run = 0; run < 4; run++)
                type(keyboard, KeyEvent.KEYCODE_N, KeyEvent.KEYCODE_I, KeyEvent.KEYCODE_H, KeyEvent.KEYCODE_A,
                    KeyEvent.KEYCODE_O);
            candidate = bounds(imeCandidate("你好"));
            Rect wrapping = bounds(field("msime-test-wrapping"));
            // 输入框确实折成了不止一行（单行输入框的高度作参照），候选栏顶边在最后一行的下半之下；只按第一个字摆时它落在第一行下方，也就是第二行上。
            // 多出来的一行只加行高、不加内边距，所以两行的输入框比单行高出不到一半。
            if (wrapping.height() < plain.height() * 5 / 4)
                throw new AssertionError("Composition did not wrap: editor " + wrapping + ", one line " + plain);
            if (candidate.top < wrapping.bottom - plain.height() / 2)
                throw new AssertionError("Candidate strip covers the wrapped composition: candidate " + candidate
                    + ", editor " + wrapping + ", one line " + plain);
            stage = "wrapped composition commit";
            // 首选可能只取了前面一段，剩下的拼音还在组字（输入框里还有字母）；空格选到组字全部上屏为止。
            for (int attempt = 0; attempt < 4; attempt++) {
                type(keyboard, KeyEvent.KEYCODE_SPACE);
                SystemClock.sleep(300);
                AccessibilityNodeInfo editor = findVisible(field("msime-test-wrapping"));
                if (editor != null && editor.getText() != null && !hasLetter(editor.getText())) break;
            }
            await(field("msime-test-wrapping").and(node -> node.getText() != null
                && node.getText().toString().startsWith("你好") && !hasLetter(node.getText())));

            // 回到报告位置的输入框、空闲：整个输入法窗口收起，没有看不见的窗口去接第一下返回键，按一下应用就返回。
            stage = "idle caret mode hides the window";
            tap(field("msime-test-plain"));
            await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
            // EditorActivity 在输入框获得焦点后的 3 秒内还会几次要求显示软键盘（requestIme），每次窗口出现都会被重新收起（间隔 WINDOW_REHIDE_MS）；等这些请求过去再看。
            SystemClock.sleep(4000);
            awaitNoImeWindow();
            stage = "one back press leaves the editor";
            if (!automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK))
                throw new AssertionError("Back action failed");
            awaitGone(field("msime-test-plain"));
        } finally {
            if (uinput != null) unplugKeyboard(uinput);
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (originalLocal == null) Files.deleteIfExists(localSettings.toPath());
            else publish(localSettings, originalLocal);
            restartIme();
        }
    }

    /** 输入框里还有没上屏的拼音字母。 */
    private static boolean hasLetter(CharSequence text) {
        for (int index = 0; index < text.length(); index++) {
            char character = text.charAt(index);
            if (character >= 'a' && character <= 'z') return true;
        }
        return false;
    }

    /** 等输入法窗口从窗口列表里消失：收起后系统不再把它当成显示中的键盘。 */
    private void awaitNoImeWindow() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean shown = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                shown |= window.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD;
            if (!shown) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Idle caret mode kept the input method window shown; IME showed " + imeTexts());
    }

    /** 等匹配的节点从所有窗口里消失。 */
    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            if (findVisible(match) == null) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("One back press did not leave the editor; IME showed " + imeTexts());
    }

    /** 候选条上的候选：只认输入法的按钮。 */
    private Predicate<AccessibilityNodeInfo> imeCandidate(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getText() != null && node.getText().toString().contains(text)
            && equalsText("android.widget.Button", node.getClassName());
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    /** 候选栏挪到光标旁、键盘收起都有一次重新布局，量到的可能是过渡位置；等位置连续两次读到一样再量。 */
    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
    }



}
