package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 完整候选面板的设备验收：真实触摸选词后直接回到键盘继续输入。 */
public final class CandidatePanelDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "完整候选面板、触摸选择页外候选后上屏并直接继续输入";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
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
        // An expanded chip draws no number: the global index lives in the description only. It is
        // long-pressable because candidate management and gloss insertion are reachable from the
        // expanded panel as well as from the strip.
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

    /** 走真实按下、抬起与框架排队点击，不能用无障碍点击跳过按钮按压状态。 */
    private void touch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = await(match.and(AccessibilityNodeInfo::isVisibleToUser));
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        shellDiscard("input tap " + bounds.centerX() + " " + bounds.centerY());
    }

    /** The expanded grid draws no title; its description carries the composition and the complete count, as 完整候选列表；ni；N 个候选. */
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
