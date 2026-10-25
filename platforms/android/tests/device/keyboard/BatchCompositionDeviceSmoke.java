package app.msime.android.test;

import android.content.Intent;
import android.os.Build;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 延迟到批量结束才写入的编辑器仍能连续组字，并在中英切换后提交汉字。 */
public final class BatchCompositionDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "deferred batch editor composes Chinese before and after language switching";
    }

    @Override protected void runChecks() throws Exception {
        restartIme();
        Intent intent = new Intent(getTargetContext(), BatchEditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        tap(field("msime-test-batch-editor"));
        composeAndCommit("", "你好");
        stage = "switch to English";
        tap(key("中"));
        await(key("英"));
        stage = "return to Chinese";
        tap(key("英"));
        await(key("中"));
        composeAndCommit("你好", "你好你好");
    }

    private void composeAndCommit(String prefix, String committed) throws Exception {
        String[][] steps = {{"n", "n"}, {"i", "ni"}, {"h", "ni'h"},
            {"a", "ni'ha"}, {"o", "ni'hao"}};
        for (int index = 0; index < steps.length; index++) {
            stage = "deferred composition length " + (index + 1);
            tap(key(steps[index][0]));
            String expected = prefix + steps[index][1];
            await(field("msime-test-batch-editor").and(node -> equalsText(expected, node.getText())));
        }
        stage = "deferred Chinese commit";
        tap(key("空格"));
        await(field("msime-test-batch-editor").and(node -> equalsText(committed, node.getText())));
    }

    // 旧系统的无障碍节点缓存需要主动刷新，才能看到同一输入框的新文字。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }
}
