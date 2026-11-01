package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.Build;
import android.os.SystemClock;
import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 经真实软键和 InputConnection 验证共享纠错、边界提交与拒绝纠错。 */
public final class EnglishCorrectionDeviceSmoke extends DeviceSmoke {
    private boolean completionsOnly;

    @Override public void onCreate(Bundle arguments) {
        completionsOnly = arguments != null && "true".equals(arguments.getString("prefix-completion"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() { return "mobile English correction and editor gates"; }

    @Override protected void runChecks() throws Exception {
        if (!completionsOnly) EnglishSuggestionPendingChecks.run(this);
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Invalid test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
        ClipData previousClip = clipboard.getPrimaryClip();
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("default_ime_mode", "english").put("ime_mode_scope", "global")
            .put("english_suggestions", true).put("chinese_punctuation", false)
            .put("punctuation_lock", "follow").put("character_width", "halfwidth")
            .put("privacy_mode", false).put("clipboard_history", true)
            .put("touch_toolbar", new JSONObject().put("clipboard", true))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        try {
            stage = "configure English correction";
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
            // 「最近复制」只在工具栏行上出现：工具栏要显示，剪贴板建议要开着（#10479 的复核用例）。
            JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
                .put("settings", new JSONObject())
                : new JSONObject(new String(originalLocal, StandardCharsets.UTF_8));
            settings.getJSONObject("settings").put("platform.android.toolbar_hidden", false)
                .put("platform.android.clipboard_suggestion", true);
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EnglishCorrectionEditorActivity");
            tap(field("correction-plain"));
            // 全局输入模式可能继承其他套件；只通过用户可见的中英键恢复英文。
            if (await(key("中").or(key("英"))).getText().toString().equals("中")) tap(key("中"));
            checkCompletions();
            if (completionsOnly) return;
            for (String[] example : new String[][] {
                {"whatisthat", "what is that"}, {"languaage", "language"},
                {"languge", "language"}, {"langiage", "language"},
                {"imchinese", "I'm Chinese"}, {"speskebglish", "speak English"},
                {"i", "I"}, {"youre", "you're"}}) {
                stage = "correction example " + example[0];
                clear("correction-plain");
                typeWord(example[0]);
                await(described("英文建议 1：" + example[0]));
                awaitAny(described("英文建议 2：" + example[1]));
                tap(key("空格"));
                expect("correction-plain", example[1] + " ");
            }
            for (String raw : new String[] {"language", "hello", "omg", "thx"}) {
                stage = "unchanged word " + raw;
                clear("correction-plain");
                typeWord(raw);
                tap(key("空格"));
                expect("correction-plain", raw + " ");
            }
            stage = "explicit original candidate rejects correction";
            clear("correction-plain");
            typeWord("languge");
            tap(described("英文建议 1：languge"));
            tap(key("空格"));
            expect("correction-plain", "languge ");
            stage = "punctuation follows correction with ASCII space";
            clear("correction-plain");
            typeWord("languge");
            tapSymbol(".");
            expect("correction-plain", "language. ");
            for (String field : new String[] {"correction-no-suggestions", "correction-url", "correction-email"}) {
                stage = "editor gate " + field;
                clear(field);
                // 无建议框和网址框（#9725）遵循全局模式记忆，邮箱临时进英文，三者都不能沿用旧副本切回中文。
                await(key("英"));
                typeWord("languge");
                expect(field, "languge");
                tap(key("空格"));
                expect(field, "languge ");
            }
            // #10479 复核：无建议框里打英文没有任何候选，候选栏照常是工具栏，但开始打字仍要收起「最近复制」，否则它一直盖住工具栏，点它会把剪贴板接在半截词后面。
            stage = "typing in a no-suggestions field dismisses the recent clip";
            clear("correction-no-suggestions");
            await(key("英"));
            clipboard.setPrimaryClip(ClipData.newPlainText("msime synthetic", "synthetic-recent-clip"));
            await(described("粘贴最近复制的内容：synthetic-recent-clip"));
            typeWord("hello");
            expect("correction-no-suggestions", "hello");
            awaitGone(describedPrefix("粘贴最近复制的内容："));
            awaitGone(describedPrefix("英文建议 "));
            await(described("清除当前英文单词"));
            stage = "password does not rewrite the typed word";
            clear("correction-password");
            typeWord("languge");
            tap(key("空格"));
            await(field("correction-password").and(node -> node.getText() != null
                && node.getText().length() == 8));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else publish(local, originalLocal);
            if (previousClip == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(previousClip);
        }
    }

    /** 展示补全需要真实词库和编辑器，不把手动候选当作空格默认扩写。 */
    private void checkCompletions() throws Exception {
        stage = "three-letter English completion";
        clear("correction-plain");
        typeWord("eng");
        await(described("英文建议 1：eng"));
        long started = android.os.SystemClock.elapsedRealtime();
        Predicate<AccessibilityNodeInfo> english = node -> {
            CharSequence description = node.getContentDescription();
            return description != null && description.toString().startsWith("英文建议 ")
                && description.toString().endsWith("：English");
        };
        awaitAny(english);
        android.util.Log.i("MSIMEDeviceSmoke", "English completion observation ms="
            + (android.os.SystemClock.elapsedRealtime() - started));
        tap(key("空格"));
        expect("correction-plain", "eng ");

        stage = "explicit completion preserves preceding and following text";
        clear("correction-plain");
        Bundle text = new Bundle();
        text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "left  right");
        if (!await(field("correction-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
            throw new AssertionError("Synthetic editor content failed");
        Bundle selection = new Bundle();
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 5);
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 5);
        if (!await(field("correction-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection))
            throw new AssertionError("Synthetic editor selection failed");
        dropStaleNodes();
        typeWord("eng");
        awaitAny(english);
        tap(english);
        expect("correction-plain", "left English right");
    }

    private void typeWord(String word) throws Exception {
        String originalStage = stage;
        for (char letter : word.toCharArray()) {
            stage = originalStage + ": " + letter;
            tap(key(String.valueOf(letter)));
        }
        stage = originalStage;
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

    private void clear(String description) throws Exception {
        tap(field(description));
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
        if (!await(field(description)).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Synthetic editor reset failed");
        dropStaleNodes();
    }

    private void expect(String description, String value) {
        await(field(description).and(node -> equalsText(value, node.getText())));
    }

    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node,
            Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34) node.refresh();
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node,
            Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34) node.refresh();
        return super.findAny(node, match);
    }
}
