package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 离线英文释义的设备验收：候选条和展开列表上的释义、长按菜单、选中身份和关闭。英文释义的默认值由 client-core 决定（新装是关），这里先显式打开再测功能本身。 */
public final class CandidateGlossDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "offline candidate gloss strip, expanded long press, selection, held return committing the gloss and opt-out";
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

            stage = "expanded candidate offline gloss";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            AccessibilityNodeInfo expanded = await(expandedGlossCandidate());
            stage = "expanded candidate long press menu";
            if (!expanded.isLongClickable()
                    || !expanded.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Expanded candidate did not expose long press");
            await(glossMenuItem());
            if (!automation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK))
                throw new AssertionError("Candidate gloss menu did not close");
            tap(candidatePanelClose());

            stage = "glossed candidate selection identity";
            tap(glossCandidate());
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            // 屏幕键盘长按组字中的「确认」（#6792）：「你好」只有 hello 一个义项，直接上屏译文，中文和拼音都不上屏。
            stage = "held return commits the gloss";
            typeGreeting();
            await(glossCandidate());
            AccessibilityNodeInfo confirm = await(returnKeyOfferingGloss());
            if (!confirm.isLongClickable() || !confirm.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("The composing return key did not expose its long press");
            await(field("msime-test-plain").and(node -> equalsText("你好hello", node.getText())));

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

    private Predicate<AccessibilityNodeInfo> glossMenuItem() {
        // 长按菜单是系统 PopupMenu：可点的是整行，显示「hello」的文字节点本身不可点。
        return node -> preview(node) && equalsText("hello", node.getText()) && clickableWithin(node, 3);
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

    private Predicate<AccessibilityNodeInfo> candidatePanelClose() {
        return node -> preview(node) && node.getContentDescription() != null
            && equalsText("收起候选面板", node.getContentDescription());
    }

    /** 自己或往上 `levels` 层内的祖先可点击。 */
    private static boolean clickableWithin(AccessibilityNodeInfo node, int levels) {
        for (AccessibilityNodeInfo current = node; current != null && levels >= 0;
                current = current.getParent(), levels--) {
            if (current.isClickable()) return true;
        }
        return false;
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
