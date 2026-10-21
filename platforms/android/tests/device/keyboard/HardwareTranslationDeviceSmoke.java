package app.msime.android.test;

import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/**
 * 实体键盘 Ctrl+Enter 上屏候选译文（#7079）的设备验收：单义项直接上屏；多义项打开义项页，数字选第几项、空格上屏高亮项、Esc 只关页而组字还在；释义关着时按键被吃掉、拼音不上屏；没有组字时输入法不接这个键。
 *
 * <p>键从 `uinput` 接上的 USB 键盘打进去（API 31 起；更早的系统要 `hw.keyboard=yes` 的 AVD），理由见 {@link DeviceSmoke#physicalKeyboard}。用的是随包离线英文释义：「你好」是一个义项 `hello`，「我的」是两个义项 `my; mine`。数据换了、这两个前提不成立时在对应阶段失败并说明，而不是测成别的东西。
 */
public final class HardwareTranslationDeviceSmoke extends DeviceSmoke {
    // 清单已达 100 个 instrumentation：数字回归复用这个硬件按键入口，可单独运行合成事件部分。
    private boolean digitsOnly;

    @Override public void onCreate(Bundle arguments) {
        digitsOnly = arguments != null && "true".equals(arguments.getString("digits-only"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return digitsOnly ? "native service hardware digit routing with synthetic key events"
            : "native service digit routing; Ctrl+Enter translation commits, sense pages and editor pass-through";
    }

    @Override protected void runChecks() throws Exception {
        HardwareDigitChecks.run(this);
        if (digitsOnly) return;
        ParcelFileDescriptor[] uinput = null;
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
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", true);
        try {
            stage = "physical keyboard device";
            if (Build.VERSION.SDK_INT >= 31) {
                uinput = plugKeyboard(KeyEvent.KEYCODE_1, KeyEvent.KEYCODE_2, KeyEvent.KEYCODE_3,
                    KeyEvent.KEYCODE_4, KeyEvent.KEYCODE_5, KeyEvent.KEYCODE_6, KeyEvent.KEYCODE_7,
                    KeyEvent.KEYCODE_8, KeyEvent.KEYCODE_9, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_ESCAPE,
                    KeyEvent.KEYCODE_CTRL_LEFT);
            }
            int keyboard = physicalKeyboard();

            stage = "offline gloss preference on";
            publish(preferences, snapshot(revision + 1, new JSONObject(base.toString())));
            restartIme();
            openEditor();

            // 单义项：「你好」只有 hello，Ctrl+Enter 直接上屏它，拼音和中文都不上屏。
            stage = "single sense candidate";
            type(keyboard, KeyEvent.KEYCODE_N, KeyEvent.KEYCODE_I, KeyEvent.KEYCODE_H, KeyEvent.KEYCODE_A,
                KeyEvent.KEYCODE_O);
            AccessibilityNodeInfo greeting = await(glossedCandidate("你好"));
            if (!glossOf(greeting).equals("hello"))
                throw new AssertionError("The bundled gloss of 你好 is no longer the single sense hello");
            stage = "single sense commit";
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            await(field("msime-test-plain").and(node -> equalsText("hello", node.getText())));

            // 多义项：「我的」是 my; mine，Ctrl+Enter 打开义项页，候选行换成「译文 1」「译文 2」，按 2 上屏第二项。
            stage = "multi sense candidate";
            typeWode(keyboard);
            AccessibilityNodeInfo mine = await(glossedCandidate("我的"));
            if (!glossOf(mine).contains(";"))
                throw new AssertionError("The bundled gloss of 我的 no longer has several senses");
            stage = "multi sense page";
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            String second = senseOf(await(describedPrefix("译文 2：")));
            await(describedPrefix("译文 1："));
            if (findVisible(describedPrefix("候选 1：我的")) != null)
                throw new AssertionError("The sense page did not replace the candidate strip; IME showed " + imeTexts());
            stage = "multi sense digit commit";
            type(keyboard, KeyEvent.KEYCODE_2);
            String afterDigit = "hello" + second;
            await(field("msime-test-plain").and(node -> equalsText(afterDigit, node.getText())));

            // Esc 只关页：候选条回来，拼音还在组字。
            stage = "sense page escape";
            typeWode(keyboard);
            await(glossedCandidate("我的"));
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            await(describedPrefix("译文 1："));
            type(keyboard, KeyEvent.KEYCODE_ESCAPE);
            await(describedPrefix("候选 1：我的"));
            if (findVisible(describedPrefix("译文 1：")) != null)
                throw new AssertionError("Escape left the sense page open; IME showed " + imeTexts());
            await(field("msime-test-plain").and(node -> equalsText(afterDigit + "wode", node.getText())));

            // 再开一次页，空格上屏高亮的第一项。
            stage = "sense page space commit";
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            String first = senseOf(await(describedPrefix("译文 1：")));
            type(keyboard, KeyEvent.KEYCODE_SPACE);
            String afterSpace = afterDigit + first;
            await(field("msime-test-plain").and(node -> equalsText(afterSpace, node.getText())));

            // 释义关着：Ctrl+Enter 被吃掉，拼音不上屏，组字和候选都还在。
            stage = "offline gloss preference off";
            publish(preferences, snapshot(revision + 2,
                new JSONObject(base.toString()).put("candidate_english_gloss", false)));
            restartIme();
            openEditor();
            stage = "glosses off swallows the key";
            type(keyboard, KeyEvent.KEYCODE_N, KeyEvent.KEYCODE_I, KeyEvent.KEYCODE_H, KeyEvent.KEYCODE_A,
                KeyEvent.KEYCODE_O);
            await(describedPrefix("候选 1：你好"));
            await(field("msime-test-plain").and(node -> equalsText("nihao", node.getText())));
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            SystemClock.sleep(800);
            AccessibilityNodeInfo composing = await(field("msime-test-plain"));
            if (!equalsText("nihao", composing.getText()) || findVisible(describedPrefix("候选 1：你好")) == null)
                throw new AssertionError("Ctrl+Enter with glosses off did not leave the composition alone; IME showed "
                    + imeTexts());

            // 没有组字：输入法不接 Ctrl+Enter，交给应用；至少没有任何文字被输入法上屏。
            stage = "idle editor keeps its key";
            type(keyboard, KeyEvent.KEYCODE_SPACE);
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
            ctrlChord(keyboard, KeyEvent.KEYCODE_ENTER);
            SystemClock.sleep(800);
            AccessibilityNodeInfo idle = await(field("msime-test-plain"));
            if (!equalsText("你好", idle.getText()))
                throw new AssertionError("Ctrl+Enter in an idle editor changed its text");
        } finally {
            if (uinput != null) unplugKeyboard(uinput);
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }

    private void typeWode(int keyboard) throws Exception {
        type(keyboard, KeyEvent.KEYCODE_W, KeyEvent.KEYCODE_O, KeyEvent.KEYCODE_D, KeyEvent.KEYCODE_E);
    }

    /** 候选条上首选是 `text`、而且已经带上英文释义的那个候选；释义是异步到的，要等它出现再按 Ctrl+Enter。 */
    private Predicate<AccessibilityNodeInfo> glossedCandidate(String text) {
        return describedPrefix("候选 1：" + text).and(node ->
            node.getContentDescription().toString().contains("英文释义："));
    }

    /** 候选读屏描述里的英文释义：`，英文释义：` 之后，到下一个全角分号（如「；长按管理」）为止。 */
    private static String glossOf(AccessibilityNodeInfo candidate) {
        String description = candidate.getContentDescription().toString();
        String gloss = description.substring(description.indexOf("英文释义：") + "英文释义：".length());
        int end = gloss.indexOf('；');
        return end < 0 ? gloss : gloss.substring(0, end);
    }

    /** 义项按钮读屏描述「译文 n：xxx」里的义项。 */
    private static String senseOf(AccessibilityNodeInfo sense) {
        String description = sense.getContentDescription().toString();
        return description.substring(description.indexOf('：') + 1);
    }

    private byte[] snapshot(long revision, JSONObject preferences) throws Exception {
        return new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8);
    }

    /** 新开一个编辑页（清掉上一轮的文字）并让纯文本输入框拿到焦点。 */
    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        if (Build.VERSION.SDK_INT >= 30) {
            await(field("msime-test-plain").and(node ->
                equalsText("ime-requests-settled", node.getStateDescription())));
        }
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }
}
