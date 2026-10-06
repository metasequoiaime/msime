package app.msime.android.test;

import android.content.Intent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** Device-only acceptance for the complete, non-paged candidate panel. */
public final class CandidatePanelDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "complete candidate panel and out-of-page InputConnection commit";
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
        tap(candidateTen());
        stage = "candidate panel commit";
        await(field("msime-test-plain").and(node -> node.getText() != null
            && node.getText().length() > 0 && !equalsText("ni", node.getText())));
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
