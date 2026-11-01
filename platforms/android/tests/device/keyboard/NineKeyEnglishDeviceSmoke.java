package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 经真实九键和 InputConnection 验证引擎英文候选与完整词上屏。 */
public final class NineKeyEnglishDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key English candidate discovery and exact InputConnection commit";
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
            .put("candidate_english_gloss", false)
            .put("english_suggestions", true)
            .put("mixed_input", new JSONObject().put("english", true)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable shared nine-key English preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            // 全局模式可能继承前一套测试；先通过真实语言键恢复中文九键。
            if (await(key("中").or(key("英"))).getText().toString().equals("英")) tap(key("英"));
            for (String[] example : new String[][] {
                {"448482", "GitHub"}, {"7738439", "preview"},
                {"266648", "commit"}, {"25274", "clash"}}) {
                stage = "完整九键英文候选 " + example[1];
                for (char digit : example[0].toCharArray()) {
                    String label = new String[] {"", "", "ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ"}[digit - '0'];
                    tap(key(label));
                }
                tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
                tap(candidate(example[1]));
                await(field("msime-test-plain").and(node -> equalsText(example[1], node.getText())));
                Bundle clear = new Bundle();
                clear.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
                if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, clear))
                    throw new AssertionError("合成编辑器清空失败");
                dropStaleNodes();
            }

            stage = "real nine-key digit entry";
            tap(key("MNO"));
            tap(key("JKL"));
            // 九键按下的数字不写进输入框，写的是引擎给的拼音读音 `nine_key_reading`（MSIMEInputService.composingText）：引擎收到两个数字后有了候选，「展开」可用，输入框里有组字。首行是英文词时引擎不给九键读音，宿主退回 `preedit`，所以这里只断言组字写进了输入框，不断言其中没有数字。
            await(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().length() > 0 && !node.isShowingHintText()));

            // 固定词库的英文权重为零，验收完整候选集，不假定首页排序。
            stage = "complete nine-key English candidates";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            tap(candidate("ok"));

            stage = "nine-key English commit identity";
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok")));

            // 第一格长按的 1 经真实 InputConnection 上屏；短按的常用标点见下面的 quick punctuation 一段。
            stage = "nine-key long-press one";
            AccessibilityNodeInfo one = await(key("@#"));
            if (!one.isLongClickable() || !one.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Key 1 must offer its literal digit");
            tap(described("输入 1"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok1")));

            stage = "nine-key numeric one and layer restoration";
            tap(key("123"));
            tap(key("1"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok11")));
            tap(key("九键"));
            await(key("@#").and(AccessibilityNodeInfo::isLongClickable));

            // 空闲时第一格短按不打开符号面板，而是把常用标点放进候选条；点一个经 Engine 上屏（中文标点下 \ 写成 、），候选条随后回到空闲。
            stage = "nine-key 1 key quick punctuation";
            tap(key("@#"));
            tap(described("常用标点 5：、"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok11、")));
            awaitGone(describedPrefix("常用标点 "));

            // 符号面板改从九键底行最左边的 符 打开。
            stage = "nine-key bottom row symbol panel key";
            tap(described("打开符号面板"));
            await(described("符号面板"));
            tap(described("关闭符号面板"));
            await(key("@#"));

            // 九键切到独立英文后画 26 键，逗号的可见性必须跟随实际键面。
            stage = "nine-key switches to English comma";
            tap(described("切换到英文输入"));
            await(key("q"));
            tap(describedPrefix("常用标点："));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().equalsIgnoreCase("ok11、,")));

            stage = "英文清除只删除当前词并保留前后文本";
            setEditorText("alpha  tail", 6, 6);
            awaitGone(described("清除当前英文单词"));
            typeWord("languge");
            await(described("清除当前英文单词"));
            // 在纠错尚可能回包时清除，不先点原文或采用建议。
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");
            awaitGone(described("清除当前英文单词"));
            SystemClock.sleep(750);
            awaitGone(describedPrefix("英文建议 "));
            expectEditorText("alpha  tail");

            stage = "清除后继续英文输入和选区保护";
            typeWord("sample");
            expectEditorText("alpha sample tail");
            await(described("清除当前英文单词"));
            setEditorSelection(8, 8);
            awaitGone(described("清除当前英文单词"));
            setEditorSelection(6, 12);
            awaitGone(described("清除当前英文单词"));
            setEditorSelection(12, 12);
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");

            // #10479：没有补全的词点原文只收起建议，词仍在光标前；候选行没有内容时换回工具栏，「清除」留在读音行里照常可用。
            stage = "无补全的词点原文后恢复工具栏";
            setEditorText("alpha  tail", 6, 6);
            typeWord("qzxjvkqzxj");
            SystemClock.sleep(750);
            tap(englishSuggestion("qzxjvkqzxj"));
            expectEditorText("alpha qzxjvkqzxj tail");
            awaitGone(describedPrefix("英文建议 "));
            await(described("键盘快捷栏"));
            awaitGone(described("展开候选"));
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");
            awaitGone(described("清除当前英文单词"));
            await(described("键盘快捷栏"));

            stage = "全角字母与撇号仍按当前词清除";
            setEditorText("alpha ｓａｍｐｌｅ tail", 12, 12);
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");
            setEditorText("alpha can't tail", 11, 11);
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");

            stage = "截断的长词不提供清除";
            char[] longWord = new char[129];
            java.util.Arrays.fill(longWord, 'x');
            setEditorText(new String(longWord), 129, 129);
            awaitGone(described("清除当前英文单词"));

            stage = "单词边界和中文模式不提供英文清除";
            setEditorText("alpha ", 6, 6);
            awaitGone(described("清除当前英文单词"));
            stage = "切回中文时隐藏英文清除";
            tap(key("英"));
            await(key("中"));
            await(key("ABC"));
            tap(key("MNO"));
            awaitGone(described("清除当前英文单词"));

            stage = "关闭英文建议后仍可清除当前词";
            base.put("english_suggestions", false);
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 2).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            if (await(key("中").or(key("英"))).getText().toString().equals("中")) tap(key("中"));
            setEditorText("alpha  tail", 6, 6);
            typeWord("sample");
            tap(described("清除当前英文单词"));
            expectEditorText("alpha  tail");
            awaitGone(describedPrefix("英文建议 "));

            stage = "密码框不提供当前英文词清除";
            tap(field("msime-test-password"));
            awaitGone(described("清除当前英文单词"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private void typeWord(String word) throws Exception {
        for (char letter : word.toCharArray()) tap(key(String.valueOf(letter)));
    }

    private void setEditorText(String text, int start, int end) throws Exception {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("合成编辑器重置失败");
        dropStaleNodes();
        expectEditorText(text);
        setEditorSelection(start, end);
    }

    private void setEditorSelection(int start, int end) throws Exception {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end);
        AccessibilityNodeInfo editor = await(field("msime-test-plain"));
        if (editor.getTextSelectionStart() == start && editor.getTextSelectionEnd() == end) return;
        if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("合成编辑器选区设置失败");
        dropStaleNodes();
        await(field("msime-test-plain").and(node -> node.getTextSelectionStart() == start
            && node.getTextSelectionEnd() == end));
    }

    private void expectEditorText(String text) {
        await(field("msime-test-plain").and(node -> equalsText(text, node.getText())));
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            // 英文候选按英文词典的写法显示（ok 显示为 OK），不区分大小写比较。
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equalsIgnoreCase(text);
        };
    }

    /** 内容是原文本身的英文建议，序号随纠错和补全变化，不假定在哪一格。 */
    private Predicate<AccessibilityNodeInfo> englishSuggestion(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("英文建议 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            int pronunciation = value.indexOf('，');
            return (pronunciation < 0 ? value : value.substring(0, pronunciation)).equals(text);
        };
    }

    /** 等匹配的节点从所有窗口里消失。 */
    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            boolean shown = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                shown |= find(window.getRoot(), match) != null;
            if (!shown) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("等待目标控件消失超时；输入法显示 " + imeTexts());
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
