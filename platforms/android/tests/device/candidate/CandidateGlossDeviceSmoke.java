package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 离线英文释义的设备验收：候选条和展开列表上的释义、长按候选打开的释义卡片（#10915：打开、上屏译文、复制译文、几种关闭方式，原长按菜单的义项上屏和词库操作）、选中身份和关闭。英文释义的默认值由 client-core 决定（新装是关），这里先显式打开再测功能本身。 */
public final class CandidateGlossDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "offline candidate gloss strip, gloss card open, commit, copy, close and old menu actions, expanded long press, selection, held return committing the gloss and opt-out";
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
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", true);
        try {
            stage = "offline gloss preference on";
            publish(preferences, snapshot(revision + 1, new JSONObject(base.toString())));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            typeGreeting();

            stage = "candidate strip offline gloss";
            AccessibilityNodeInfo strip = await(glossCandidate());
            if (strip.getText() == null || !strip.getText().toString().contains("hello"))
                throw new AssertionError("Candidate strip did not expose the offline annotation");

            glossCard(strip);

            stage = "expanded candidate offline gloss";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            AccessibilityNodeInfo expanded = await(expandedGlossCandidate());
            stage = "expanded candidate long press card";
            longPress(expanded);
            await(described("英语 · 离线释义：hello；点按上屏"));
            // 卡片盖在展开面板上；返回键只关卡片，面板还在。
            stage = "expanded candidate card closes on back";
            back();
            awaitGone(described("上屏译文"));
            stage = "expanded candidate panel stays after the card";
            await(describedPrefix("完整候选列表；"));
            await(expandedGlossCandidate());
            // 面板底部的「返回」在候选多时排在很长的网格后面，读屏树里不一定有；用候选栏上的展开键收起。
            tap(described("展开候选"));
            awaitGone(describedPrefix("完整候选列表；"));

            stage = "glossed candidate selection identity";
            tap(glossCandidate());
            await(field("msime-test-plain").and(node -> equalsText("hellohello你好", node.getText())));

            // 屏幕键盘长按组字中的「确认」（#6792）：「你好」只有 hello 一个义项，直接上屏译文，中文和拼音都不上屏。
            stage = "held return commits the gloss";
            typeGreeting();
            await(glossCandidate());
            AccessibilityNodeInfo confirm = await(returnKeyOfferingGloss());
            if (!confirm.isLongClickable() || !confirm.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("The composing return key did not expose its long press");
            await(field("msime-test-plain").and(node -> equalsText("hellohello你好hello", node.getText())));

            stage = "disable offline gloss preference";
            publish(preferences, snapshot(revision + 2,
                new JSONObject(base.toString()).put("candidate_english_gloss", false)));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            typeGreeting();
            await(plainCandidate());
            SystemClock.sleep(1200);
            if (findVisible(glossCandidate()) != null)
                throw new AssertionError("Disabled candidate gloss became visible");
            if (findVisible(returnKeyOfferingGloss()) != null)
                throw new AssertionError("The return key offered a held gloss with glosses off");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    /**
     * 长按候选条上的「你好」打开释义卡片，逐项走一遍：卡片内容、删除词条的就地确认、三种关闭方式（返回键、点候选栏、卡片的「收起」）、复制译文、上屏译文，再走原长按菜单的「优先显示」和义项上屏。输入框里的文字只来自卡片的两次上屏（hellohello），结束时留着一段新的 nihao 组字。
     */
    private void glossCard(AccessibilityNodeInfo strip) throws Exception {
        stage = "gloss card opens";
        longPress(strip);
        if (!await(cardTitle()).isHeading()) throw new AssertionError("The card title is not a heading");
        await(described("候选：你好"));
        await(node -> preview(node) && equalsText("英语 · 离线释义", node.getText()));
        await(described("英语 · 离线释义：hello；点按上屏"));
        await(described("复制译文").and(AccessibilityNodeInfo::isEnabled));
        await(described("上屏译文").and(AccessibilityNodeInfo::isEnabled));
        await(described("优先显示"));
        await(described("固定到首位"));
        await(described("取消固定"));
        // 「加入生词本」只在显示着可上屏的释义时出现，「你好」有 hello。
        await(described("加入生词本"));

        // 有英文释义时，朗读入口是否出现取决于设备实际安装的 TTS 服务；不把模拟器缺引擎当成免验条件。
        stage = "gloss card read aloud availability";
        boolean speechEngineInstalled = !getTargetContext().getPackageManager().queryIntentServices(
            new android.content.Intent(android.speech.tts.TextToSpeech.Engine.INTENT_ACTION_TTS_SERVICE), 0).isEmpty();
        if (speechEngineInstalled) {
            await(described("朗读").and(AccessibilityNodeInfo::isEnabled));
        } else {
            awaitGone(described("朗读"));
        }

        stage = "gloss card removal asks in place";
        tap(described("删除词条…"));
        await(described("确认删除此词条"));

        stage = "gloss card closes on back";
        back();
        awaitGone(cardTitle());
        await(glossCandidate());

        stage = "gloss card closes from the candidate bar";
        CharSequence composing = await(field("msime-test-plain")).getText();
        String before = composing == null ? "" : composing.toString();
        longPress(await(glossCandidate()));
        AccessibilityNodeInfo shield = await(described("关闭释义"));
        android.graphics.Rect cover = new android.graphics.Rect();
        android.graphics.Rect candidate = new android.graphics.Rect();
        shield.getBoundsInScreen(cover);
        await(glossCandidate()).getBoundsInScreen(candidate);
        if (!cover.contains(candidate))
            throw new AssertionError("The card's candidate-bar cover does not cover the candidate");
        tap(described("关闭释义"));
        awaitGone(cardTitle());
        await(glossCandidate());
        // 点候选栏只关卡片，不选词：输入框里还是原来的组字。
        await(field("msime-test-plain").and(node -> equalsText(before, node.getText())));

        stage = "gloss card closes with its own button";
        longPress(await(glossCandidate()));
        tap(described("收起释义"));
        awaitGone(cardTitle());
        await(glossCandidate());

        stage = "gloss card copies the translation";
        android.content.ClipboardManager clipboard =
            getTargetContext().getSystemService(android.content.ClipboardManager.class);
        clipboard.setPrimaryClip(android.content.ClipData.newPlainText("synthetic", "msime-gloss-card-sentinel"));
        longPress(await(glossCandidate()));
        tap(described("复制译文"));
        expectClipboard(clipboard, "hello");
        await(cardTitle());

        stage = "gloss card commits the translation";
        tap(described("上屏译文"));
        await(field("msime-test-plain").and(node -> equalsText("hello", node.getText())));
        awaitGone(cardTitle());

        stage = "gloss card keeps the dictionary actions";
        typeGreeting();
        longPress(await(glossCandidate()));
        tap(described("优先显示"));
        await(node -> preview(node) && node.getText() != null && node.getText().toString().contains("已优先显示"));
        awaitGone(cardTitle());

        stage = "gloss card inserts a listed sense";
        longPress(await(glossCandidate()));
        tap(described("英语 · 离线释义：hello；点按上屏"));
        await(field("msime-test-plain").and(node -> equalsText("hellohello", node.getText())));
        awaitGone(cardTitle());
        typeGreeting();
    }

    private Predicate<AccessibilityNodeInfo> cardTitle() {
        return node -> preview(node) && equalsText("释义", node.getText());
    }

    private void longPress(AccessibilityNodeInfo node) {
        if (!node.isLongClickable() || !node.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Candidate did not expose long press");
    }

    private void back() {
        if (!automation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK))
            throw new AssertionError("Back was not delivered");
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> predicate) {
        long until = SystemClock.uptimeMillis() + 5000;
        while (findVisible(predicate) != null) {
            if (SystemClock.uptimeMillis() > until) throw new AssertionError("Expected UI did not go away");
            SystemClock.sleep(100);
        }
    }

    private void expectClipboard(android.content.ClipboardManager clipboard, String expected) {
        long until = SystemClock.uptimeMillis() + 4000;
        do {
            android.content.ClipData clip = clipboard.getPrimaryClip();
            if (clip != null && clip.getItemCount() == 1 && expected.contentEquals(clip.getItemAt(0).getText())) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("The copied translation did not reach the clipboard");
    }

    private byte[] snapshot(long revision, JSONObject preferences) throws Exception {
        return new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8);
    }

    private void typeGreeting() throws Exception {
        for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
    }

    private Predicate<AccessibilityNodeInfo> glossCandidate() {
        return node -> preview(node) && node.getContentDescription() != null
            && node.getText() != null
            && !node.getText().toString().startsWith("1 ")
            && node.getContentDescription().toString().startsWith("候选 1：你好")
            && node.getContentDescription().toString().contains("英文释义：hello");
    }

    private Predicate<AccessibilityNodeInfo> expandedGlossCandidate() {
        return node -> preview(node) && node.getText() != null
            // 释义现在在候选字下面另起一行（「你好」换行「hello」），不再用两个空格接在后面。
            && node.getText().toString().startsWith("你好")
            && node.getText().toString().contains("hello")
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 1：你好");
    }

    private Predicate<AccessibilityNodeInfo> plainCandidate() {
        return node -> preview(node) && node.getContentDescription() != null
            && node.getText() != null
            && !node.getText().toString().startsWith("1 ")
            && node.getContentDescription().toString().startsWith("候选 1：你好")
            && !node.getContentDescription().toString().contains("英文释义：");
    }

    /** 组字中、长按能上屏译文的回车键：读屏描述是「确认；长按上屏译文」。 */
    private Predicate<AccessibilityNodeInfo> returnKeyOfferingGloss() {
        return node -> preview(node) && node.getContentDescription() != null
            && equalsText("确认；长按上屏译文", node.getContentDescription());
    }

    private boolean preview(AccessibilityNodeInfo node) {
        return equalsText("app.msime.android", node.getPackageName());
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> predicate) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), predicate);
            if (found != null) return found;
        }
        return null;
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
