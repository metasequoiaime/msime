package app.msime.android.test;

import android.os.SystemClock;
import android.graphics.Rect;
import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 全拼九键设备验收：展开面板、逐字选词后的剩余候选，以及选定读法后的最近候选。 */
public final class NineKeyPanelDeviceSmoke extends DeviceSmoke {
    private static final String PANEL = "九键候选面板";
    private boolean partialSelection;
    private boolean recentOnly;

    @Override public void onCreate(Bundle arguments) {
        partialSelection = arguments != null && "true".equals(arguments.getString("partial-selection"));
        recentOnly = arguments != null && "true".equals(arguments.getString("recent-learning"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return partialSelection ? "九键展开后连续逐字选词，面板留着并换上剩余组字的候选，选完整句上屏后收起"
            : recentOnly ? "nine-key spelling picks lead the next unlocked composition across input restarts"
            : "nine-key panel locks, backspace, filters and recent spelling selections";
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
            .put("touch_keyboard_layout", "nine_key")
            .put("traditional_chinese_output", false)
            .put("learning", true)
            .put("frequency", new JSONObject().put("mode", "disabled")
                .put("trigger_count", 1).put("linear_step", 1))
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable shared nine-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            // 前一套英文纠错会保留全局英文状态；通过真实语言键建立中文九键前置条件。
            stage = "restore Chinese nine-key input mode";
            Predicate<AccessibilityNodeInfo> language = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            AccessibilityNodeInfo mode = await(key("中").or(key("英")).and(language));
            if (equalsText("英", mode.getText())) tap(key("英").and(language));
            await(key("ABC"));
            if (partialSelection) {
                checkPartialSelection();
                return;
            }

            if (recentOnly) {
                verifyRecentSpellingSelection();
                return;
            }

            // 6464224：6464 可以是 ming 或 ning，224 可以是 bai 或 cai。
            stage = "nine-key composition";
            for (String face : new String[] {"MNO", "GHI", "MNO", "GHI", "ABC", "ABC", "GHI"})
                tap(key(face));
            stage = "collapsed spelling column";
            await(described("选择拼音 ning"));

            stage = "open the three-column panel";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            await(inPanel(described("候选面板 重输")));
            await(inPanel(described("只显示单字")));

            stage = "lock ning inside the panel";
            tap(inPanel(described("选择拼音 ning")));
            stage = "lock bai inside the panel";
            tap(inPanel(described("选择拼音 bai")));
            stage = "panel stays open after locking every digit";
            await(described(PANEL));
            await(inPanel(described("选择拼音 cai")));

            // 全部锁定时 ⌫ 撤销最后一次锁定（回到 ning'224，拼音栏仍是 224 的选项），面板不收起；再按一次才删数字（ning'22，没有 bai 了）。
            stage = "backspace undoes the last lock and keeps the panel";
            tap(described("候选面板 删除"));
            await(described(PANEL));
            await(inPanel(described("选择拼音 bai")));
            stage = "second backspace deletes a digit and keeps the panel";
            tap(described("候选面板 删除"));
            await(described(PANEL));
            await(inPanel(described("选择拼音 ba")));
            // 删掉的是数字：读音从 ning'224 变成 ning'22（显示为 ning'ba）。拼音栏现在也列出以这两个数字开头的更长音节（ba、bai、ban…），所以不能再用「bai 消失」来判断。读音写在输入框里，候选栏不再画它。
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().contains("ning'ba")));

            stage = "single-character filter";
            tap(inPanel(described("只显示单字")));
            await(inPanel(described("显示全部候选")).and(AccessibilityNodeInfo::isSelected));
            AccessibilityNodeInfo first = await(inPanel(describedPrefix("候选 1：")));
            String text = first.getText() == null ? "" : first.getText().toString().split("\n")[0];
            if (text.codePointCount(0, text.length()) != 1)
                throw new AssertionError("Single-character filter left a longer candidate");
            tap(inPanel(described("显示全部候选")));
            await(inPanel(described("只显示单字")));

            stage = "stroke column toggle";
            tap(inPanel(described("切换到笔画筛选")));
            await(inPanel(described("笔画筛选 横")));
            await(inPanel(described("未选笔画")));
            tap(inPanel(described("切换到拼音")));
            await(inPanel(described("选择拼音 ba")));

            stage = "return collapses the panel";
            tap(inPanel(described("收起候选面板")));
            awaitGone(described(PANEL));
            await(described("选择拼音 ba"));

            stage = "cancel inside the panel ends the composition and closes it";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            tap(inPanel(described("候选面板 重输")));
            awaitGone(described(PANEL));
            awaitGone(describedPrefix("选择拼音 "));
            // 九键组字不在输入框里标记，重输后输入框应当仍是空的。
            AccessibilityNodeInfo editor = await(field("msime-test-plain"));
            if (editor.getText() != null && editor.getText().length() > 0 && !editor.isShowingHintText())
                throw new AssertionError("Cancelled nine-key composition reached the editor");


            stage = "touch selection closes the nine-key panel";
            tap(key("MNO"));
            tap(key("GHI"));
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            AccessibilityNodeInfo choice = await(inPanel(describedPrefix("候选 1：")));
            Rect bounds = new Rect();
            choice.getBoundsInScreen(bounds);
            shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
            awaitGone(described(PANEL));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().length() > 0 && !node.isShowingHintText()));
            String committed = await(field("msime-test-plain")).getText().toString();
            stage = "nine-key keyboard accepts input without hiding the IME";
            await(key("MNO")).getBoundsInScreen(bounds);
            shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().length() > committed.length()));
            // 触摸回归留下了已提交文字与新组字，先清空再验证最近选词，避免两项用例互相影响。
            tap(key("重输"));
            android.os.Bundle clear = new android.os.Bundle();
            clear.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
            if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, clear))
                throw new AssertionError("Cannot clear synthetic editor before recent selections");
            verifyRecentSpellingSelection();

        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    /** 每次只选一个字（#9193）：只展开一次，部分选词后面板留着、直接换上剩下音节的候选，单字筛选也留着；选完最后一个字整句上屏、面板收起。整个过程不发返回操作。 */
    private void checkPartialSelection() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity"
            + " --ez msime_test_composition_spans true");
        tap(field("msime-test-plain"));
        stage = "compose four synthetic nine-key syllables";
        for (String face : new String[] {"MNO", "GHI", "GHI", "ABC", "MNO",
                "MNO", "GHI", "GHI", "ABC", "MNO"}) tap(key(face));
        await(field("msime-test-plain").and(node -> node.getText() != null
            && node.getText().length() > 0 && !node.isShowingHintText()));
        assertComposition("", true);
        stage = "expand the composition once";
        tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
        await(described(PANEL));
        stage = "filter the first syllable to single-character candidates";
        tap(inPanel(described("只显示单字")));
        String prefix = "";
        for (String character : new String[] {"你", "好", "你", "好"}) {
            stage = "single-character filter survives " + prefix.length() + " partial selections";
            await(inPanel(described("显示全部候选")));
            stage = "touch a single character from the remaining syllables " + prefix.length();
            // 旧一代的面板里没有下一个字（你、好交替），面板没换上剩下音节的候选时这一步找不到目标。
            touch(inPanel(candidate(character)));
            prefix += character;
            final String selected = prefix;
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().startsWith(selected)));
            boolean pending = prefix.length() < 4;
            assertComposition(prefix, pending);
            stage = pending ? "partial selection keeps the panel open " + prefix.length()
                : "the last selection commits and closes the panel";
            if (pending) await(described(PANEL));
            else awaitGone(described(PANEL));
        }
        await(field("msime-test-plain").and(node -> equalsText("你好你好", node.getText())));
    }

    /** 读取独立编辑器提供的真实组字范围，不跨进程取得 Activity 对象。 */
    private void assertComposition(String prefix, boolean pending) {
        dropStaleNodes();
        AccessibilityNodeInfo editor = await(field("msime-test-plain"));
        String text = editor.getText() == null ? "" : editor.getText().toString();
        if (!text.startsWith(prefix) || (pending ? text.length() <= prefix.length() : !text.equals(prefix)))
            throw new AssertionError("Synthetic partial selection text mismatch");
        Bundle state = editor.getExtras();
        if (!state.containsKey("msime-test-composing-start") || !state.containsKey("msime-test-composing-end"))
            throw new AssertionError("Synthetic editor did not report composition markers");
        int start = state.getInt("msime-test-composing-start");
        int end = state.getInt("msime-test-composing-end");
        if (pending ? start < 0 || end <= start : start >= 0 || end >= 0)
            throw new AssertionError("Synthetic partial selection composition marker mismatch");
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            return text.equals(value.contains("；") ? value.substring(0, value.indexOf('；')) : value);
        };
    }

    /** 真实触摸覆盖按下、抬起与排队点击之间的延后绘制。 */
    private void touch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        Rect bounds = new Rect();
        awaitStableBounds(match.and(AccessibilityNodeInfo::isVisibleToUser)).getBoundsInScreen(bounds);
        shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
    }

    /** 用合成输入交替选择竞争读法，确认锁定首候选会被新会话的未锁定输入读到。 */
    private void verifyRecentSpellingSelection() throws Exception {
        for (String spelling : new String[] {"mi", "ni", "mi"}) {
            stage = "lock synthetic spelling " + spelling;
            tap(key("MNO"));
            tap(key("GHI"));
            tap(described("选择拼音 " + spelling));
            AccessibilityNodeInfo chosen = await(describedPrefix("候选 1："));
            String word = chosen.getText() == null ? "" : chosen.getText().toString().split("\n")[0];
            if (word.isEmpty()) throw new AssertionError("Synthetic spelling has no first candidate");
            tap(describedPrefix("候选 1："));
            await(field("msime-test-plain").and(node -> equalsText(word, node.getText())));

            stage = "new input connection remembers unlocked pick " + spelling;
            restartIme();
            openEditor();
            AccessibilityNodeInfo editor = await(field("msime-test-plain"));
            android.os.Bundle clear = new android.os.Bundle();
            clear.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
            if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, clear))
                throw new AssertionError("Cannot clear synthetic editor");
            tap(field("msime-test-plain"));
            tap(key("MNO"));
            tap(key("GHI"));
            await(describedPrefix("候选 1：").and(node -> equalsText(word, node.getText())));
            tap(key("重输"));
            awaitGone(describedPrefix("选择拼音 "));
        }
    }

    /** 只认三栏面板里的控件：收起时的侧栏拼音列在面板下面仍在树里，描述相同。 */
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


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
