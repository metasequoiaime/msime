package app.msime.android.test;

import android.graphics.Rect;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 全拼 14 键的设备验收，跑的是三端同一份手工脚本：BN UI GH AS OP 打出「你好」；AS GH UI 时读音行右侧的拼音条里有 shi 和 shu，选了锁定、退格撤销，拼音条不盖住候选行；展开候选是九键的三栏面板；分词键得到「西安」；长按 QW 选 w；英文画 26 键、切回仍是 14 键；123 是 26 键的设计层；密码框画 26 键；「26 键数字键盘」选了九宫格时，组字中的 123 借九键数字层，拼音条留在读音行，侧栏照常是符号栏。
 */
public final class FourteenKeyDeviceSmoke extends DeviceSmoke {
    private static final String STRIP = "14 键拼音选择";
    private static final String PANEL = "九键候选面板";

    @Override protected String successDescription() {
        return "fourteen-key group keys, reading-row pinyin strip locks, three-column panel, separator, long-press letters, English, 123 layers and the borrowed digit pad";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "fourteen_key")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin").put("fourteen_key"))
                .put("selected", "fourteen_key"));
        try {
            stage = "enable shared fourteen-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            stage = "fourteen-key face";
            await(described("按键 Q W"));
            await(described("字母 L"));
            await(described("字母 M"));
            await(described("按键 符号"));
            // 中文 14 键没有 Shift，也没有 26 键的单字母键。
            awaitGone(key("q"));

            // 「你好」：BN UI GH AS OP，组码 b u g a o。
            stage = "nihao through the group keys";
            for (String pair : new String[] {"B N", "U I", "G H", "A S", "O P"}) tap(described("按键 " + pair));
            // 组字是组码字母，不写进输入框；读音行显示引擎首选的读音（BN UI 也是 bu，首选可能是「不好」），不是组码 bugao。
            await(described("ni'hao").or(described("bu'hao")));
            awaitGone(described("bugao"));
            await(field("msime-test-plain").and(FourteenKeyDeviceSmoke::emptyField));
            stage = "nihao commit";
            tap(candidate("你好"));
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            // AS GH UI：拼音条在读音行右侧，列出 shi 和 shu；选 shu 锁定（读音变成 shu），退格先撤销锁定。
            stage = "reading-row pinyin strip";
            for (String pair : new String[] {"A S", "G H", "U I"}) tap(described("按键 " + pair));
            await(described(STRIP));
            AccessibilityNodeInfo shi = await(described("选择拼音 shi"));
            await(described("选择拼音 shu"));
            Rect strip = new Rect();
            shi.getBoundsInScreen(strip);
            Rect candidate = new Rect();
            await(describedPrefix("候选 1：")).getBoundsInScreen(candidate);
            if (strip.bottom > candidate.top + 1)
                throw new AssertionError("Pinyin strip overlaps the candidate row: strip " + strip
                    + ", candidate " + candidate);
            Rect reading = new Rect();
            await(described("shi")).getBoundsInScreen(reading);
            if (strip.left < reading.right || Math.abs(strip.centerY() - reading.centerY()) > reading.height())
                throw new AssertionError("Pinyin strip is not beside the reading: strip " + strip
                    + ", reading " + reading);
            stage = "lock shu";
            tap(described("选择拼音 shu"));
            await(described("shu"));
            stage = "backspace undoes the lock";
            tap(key("⌫"));
            await(described("shi"));
            await(described("选择拼音 shu"));

            // 展开候选用九键的三栏面板：左栏是拼音，单字筛选照常可用；返回收起面板，组字还在。
            stage = "open the three-column panel";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            await(inPanel(described("选择拼音 shu")));
            await(inPanel(described("只显示单字")));
            stage = "close the three-column panel";
            tap(inPanel(described("收起候选面板")));
            awaitGone(described(PANEL));
            await(described("选择拼音 shu"));
            stage = "cancel the composition";
            tap(key("⌫"));
            tap(key("⌫"));
            tap(key("⌫"));
            awaitGone(described(STRIP));
            await(described("按键 符号"));

            // 分词键：ZX UI 分词 AS BN 是 z u ' a b，得到「西安」而不是「先」。
            stage = "separator";
            tap(described("按键 Z X"));
            tap(described("按键 U I"));
            tap(described("按键 分词，在这里断开音节"));
            tap(described("按键 A S"));
            tap(described("按键 B N"));
            await(described("xi'an"));
            tap(candidate("西安"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("你好西安")));

            // 长按 QW 弹出 q 和 w（没有数字），选 w 上屏字面字母。
            stage = "long-press letters";
            AccessibilityNodeInfo qw = await(described("按键 Q W"));
            if (!qw.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Long press on QW failed");
            await(described("输入 q"));
            tap(described("输入 w"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("西安w")));
            AccessibilityNodeInfo l = await(described("字母 L"));
            if (l.isLongClickable()) throw new AssertionError("L must not offer a long press");

            // 英文画 26 键 QWERTY，切回中文仍是 14 键，`touch_keyboard_layout` 不变。
            stage = "English draws 26 keys";
            tap(key("中"));
            await(key("q").and(AccessibilityNodeInfo::isClickable));
            awaitGone(described("按键 Q W"));
            stage = "Chinese returns to 14 keys";
            tap(key("英"));
            await(described("按键 Q W"));
            awaitGone(key("q"));
            JSONObject stored = new JSONObject(new String(Files.readAllBytes(preferences.toPath()),
                StandardCharsets.UTF_8)).getJSONObject("preferences");
            if (!"fourteen_key".equals(stored.optString("touch_keyboard_layout")))
                throw new AssertionError("English rewrote the stored layout");

            // 123 是 26 键的设计层，返回键回到 14 键。
            stage = "123 layer";
            tap(key("123"));
            await(key("1").and(AccessibilityNodeInfo::isClickable));
            await(described("更多符号"));
            tap(described("切换到字母键盘"));
            await(described("按键 Q W"));

            // 密码框画 26 键。
            stage = "password field falls back to 26 keys";
            tap(field("msime-test-password"));
            await(field("msime-test-password").and(AccessibilityNodeInfo::isFocused));
            await(key("q").and(AccessibilityNodeInfo::isClickable));
            awaitGone(described("按键 Q W"));
            stage = "plain field returns to 14 keys";
            tap(field("msime-test-plain"));
            await(described("按键 Q W"));

            // 「26 键数字键盘」选了九宫格：组字中点 123 借九键的数字层，拼音条留在读音行右侧，侧栏照常是符号栏，与 iOS、HarmonyOS 相同。
            stage = "number layout set to the 9-grid";
            long current = new JSONObject(new String(Files.readAllBytes(preferences.toPath()),
                StandardCharsets.UTF_8)).getLong("revision");
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", current + 1)
                .put("preferences", new JSONObject(base.toString())
                    .put("touch_twenty_six_key_number_layout", "nine_key"))
                .toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            await(described("按键 Q W"));
            stage = "borrowed digit pad while composing";
            for (String pair : new String[] {"A S", "G H", "U I"}) tap(described("按键 " + pair));
            await(described("选择拼音 shi"));
            tap(key("123"));
            Rect rail = new Rect();
            await(described("符号栏，可上下滑动")).getBoundsInScreen(rail);
            Rect beside = new Rect();
            await(described("选择拼音 shi")).getBoundsInScreen(beside);
            if (beside.bottom > rail.top + 1)
                throw new AssertionError("Pinyin strip moved into the digit pad: strip " + beside + ", rail " + rail);
            await(described("shi"));
            stage = "borrowed digit pad returns to 14 keys";
            tap(described("切换到字母键盘"));
            await(described("按键 Q W"));
            await(described("选择拼音 shi"));
            tap(key("⌫"));
            tap(key("⌫"));
            tap(key("⌫"));
            awaitGone(described(STRIP));
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private static boolean emptyField(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }

    /** 候选条上写着 `text` 的候选（描述是「候选 N：text；…」）。 */
    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    /** 三栏面板里的节点。 */
    private Predicate<AccessibilityNodeInfo> inPanel(Predicate<AccessibilityNodeInfo> match) {
        return match.and(node -> {
            for (AccessibilityNodeInfo parent = node.getParent(); parent != null;
                    parent = parent.getParent()) {
                if (equalsText(PANEL, parent.getContentDescription())) return true;
            }
            return false;
        });
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (find(window.getRoot(), match) != null) {
                    present = true;
                    break;
                }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic UI state did not go away; IME showed " + imeTexts());
    }

    private void restartIme() throws Exception {
        shell("ime disable app.msime.android/.MSIMEInputService");
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    private void publish(File file, byte[] contents) throws Exception {
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
}
