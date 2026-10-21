package app.msime.android.test;

import android.os.Bundle;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 候选区设备验收：完整面板选词与横向候选条的部分选词刷新。 */
public final class CandidatePanelDeviceSmoke extends DeviceSmoke {
    private boolean stripPartialSelection;

    @Override public void onCreate(Bundle arguments) {
        stripPartialSelection = arguments != null && "true".equals(arguments.getString("strip-partial-selection"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return stripPartialSelection ? "小鹤双拼横向候选条部分选词后自动刷新，无需再次滑动"
            : "完整候选面板、触摸选择页外候选后上屏并直接继续输入";
    }

    @Override protected void runChecks() throws Exception {
        if (stripPartialSelection) {
            checkStripPartialSelection();
            return;
        }
        restartIme();
        openEditor();
        stage = "candidate panel focus";
        tap(field("msime-test-plain"));
        stage = "candidate panel composition";
        tap(key("n"));
        tap(key("i"));
        await(field("msime-test-plain").and(node -> equalsText("ni", node.getText())));
        stage = "candidate panel expand";
        tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
        stage = "candidate panel complete count";
        await(completeCount());
        stage = "candidate panel out-of-page entry";
        AccessibilityNodeInfo later = await(candidateTen());
        // 展开候选不画序号，全局序号只在描述里；长按仍可打开管理与释义菜单。
        if (later.getText() == null || later.getText().toString().startsWith("10. ")
                || !later.isLongClickable())
            throw new AssertionError("Expanded candidate chip contract mismatch");
        stage = "candidate panel out-of-page selection";
        touch(candidateTen());
        stage = "candidate panel commit";
        await(field("msime-test-plain").and(node -> node.getText() != null
            && node.getText().length() > 0 && !equalsText("ni", node.getText())));
        String committed = await(field("msime-test-plain")).getText().toString();
        stage = "candidate panel returns to keyboard after touch selection";
        touch(key("n"));
        await(field("msime-test-plain").and(node -> equalsText(committed + "n", node.getText())));
    }

    /** 只在选词前滑动；选词后直接观察剩余候选，不用下一次手势触发补绘。 */
    private void checkStripPartialSelection() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "shuangpin").put("shuangpin_profile", "xiaohe")
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false).put("learning", false)
            .put("cloud_candidates", false).put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("xiaohe")).put("selected", "xiaohe"));
        try {
            stage = "configure synthetic Xiaohe strip composition";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", configured).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            Predicate<AccessibilityNodeInfo> language = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            AccessibilityNodeInfo mode = await(key("中").or(key("英")).and(language));
            if (equalsText("英", mode.getText())) tap(key("英").and(language));
            // 小鹤 ni/hao/shi/yao，独立合成输入不使用报告人的原文。
            int typed = 0;
            for (char letter : "nihcuiyc".toCharArray()) {
                stage = "compose synthetic Xiaohe prefix " + (++typed);
                tap(key(String.valueOf(letter)));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && node.getText().length() > 0 && !node.isShowingHintText()));
            }
            stage = "synthetic long phrase and partial candidate are present";
            AccessibilityNodeInfo first = await(stripCandidate(1, "你好是要"));
            AccessibilityNodeInfo scroller = first.getParent();
            while (scroller != null && !equalsText("android.widget.HorizontalScrollView", scroller.getClassName()))
                scroller = scroller.getParent();
            if (scroller == null) throw new AssertionError("Synthetic candidate strip is not horizontal");
            Rect viewport = new Rect();
            scroller.getBoundsInScreen(viewport);
            if (viewport.width() < 100 || viewport.height() < 20)
                throw new AssertionError("Synthetic candidate viewport bounds are unavailable");
            stage = "swipe the horizontal candidate strip before selection";
            int start = viewport.right - 12;
            shellDiscard("input swipe " + start + " " + viewport.centerY() + " "
                + (start - viewport.width() / 8) + " " + viewport.centerY() + " 500");
            stage = "touch the synthetic partial phrase " + viewport.toShortString();
            touch(stripCandidate(0, "你好"));
            stage = "partial selection preserves the remaining composition";
            await(field("msime-test-plain").and(node -> equalsText("你好uiyc", node.getText())));
            stage = "remaining strip candidates refresh without another gesture";
            await(stripCandidate(1, "是要"));
            dropStaleNodes();
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), stripCandidate(0, "你好是要")) != null)
                    throw new AssertionError("Synthetic old phrase remained after partial selection");
            stage = "finish the refreshed synthetic composition";
            touch(stripCandidate(1, "是要"));
            await(field("msime-test-plain").and(node -> equalsText("你好是要", node.getText())));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
    }

    /** 只匹配候选描述；首选必须带新序号，不能把旧列表中仍存在的同字候选当作刷新。 */
    private Predicate<AccessibilityNodeInfo> stripCandidate(int index, String value) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName()) || !node.isClickable()
                    || !node.isVisibleToUser() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            String prefix = index == 0 ? "候选 " : "候选 " + index + "：";
            int delimiter = description.indexOf('：');
            if (!description.startsWith(prefix) || delimiter < 0) return false;
            String text = description.substring(delimiter + 1);
            return text.equals(value) || text.startsWith(value + "，") || text.startsWith(value + "；");
        };
    }

    /** 走真实按下、抬起与框架排队点击，不能用无障碍点击跳过按钮按压状态。 */
    private void touch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = awaitStableBounds(match.and(AccessibilityNodeInfo::isVisibleToUser));
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
    }

    /** 展开网格不画标题；描述包含组字与总数，格式为「完整候选列表；ni；N 个候选」。 */
    private Predicate<AccessibilityNodeInfo> completeCount() {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || node.getContentDescription() == null) return false;
            String value = node.getContentDescription().toString();
            if (!value.startsWith("完整候选列表；") || !value.endsWith(" 个候选")) return false;
            int start = value.lastIndexOf('；') + 1;
            try {
                return Integer.parseInt(value.substring(start, value.length() - 4)) > 9;
            } catch (NumberFormatException ignored) {
                return false;
            }
        };
    }

    private Predicate<AccessibilityNodeInfo> candidateTen() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 10：")
            && node.isClickable();
    }
}
