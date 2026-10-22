package app.msime.android.test;

import android.content.Intent;
import android.os.SystemClock;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import android.view.inputmethod.BaseInputConnection;
import android.widget.EditText;
import java.util.ArrayList;
import java.util.function.Predicate;

/** 独立编辑器验收拼音发布包，不进入设置应用，也不访问产品私有文件。 */
final class PinyinStartupDeviceSmoke {
    private static final String PRODUCT = "app.msime.android.pinyin";

    private PinyinStartupDeviceSmoke() {}

    static void runChecks(DeviceSmoke test, boolean hideResume) throws Exception {
        Intent intent = new Intent(test.getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        EditorActivity activity = (EditorActivity) test.startActivitySync(intent);
        try {
            test.stage = "pinyin edition editor focus without launcher";
            test.tap(test.field("msime-test-plain"));
            if (hideResume) {
                test.stage = "pinyin edition first keyboard view before hide";
                test.await(key("n"));
                test.shellDiscard("input keyevent BACK");
                test.tap(test.field("msime-test-plain"));
            }
            test.stage = "pinyin edition startup in the same editor";
            awaitReady(test);
            for (int round = 0; round < 2; round++) {
                test.stage = "pinyin edition composition " + round;
                for (String letter : new String[] {"n", "i", "h", "a", "o"})
                    test.tap(key(letter));
                test.await(node -> PRODUCT.contentEquals(node.getPackageName() == null ? "" : node.getPackageName())
                    && DeviceSmoke.equalsText("你好", node.getText()));
                assertComposition(test, activity, true, round == 0 ? "nihao" : "你好nihao");
                test.stage = "pinyin edition commit " + round;
                test.tap(key("空格"));
                String committed = round == 0 ? "你好" : "你好你好";
                test.await(test.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(
                    committed, node.getText())));
                assertComposition(test, activity, false, committed);
            }
        } finally {
            test.runOnMainSync(activity::finish);
        }
    }

    /** 首次解包可能比常规会话慢；只等同一编辑器的方案按钮就绪，不靠换焦点恢复。 */
    private static void awaitReady(DeviceSmoke test) {
        long deadline = SystemClock.uptimeMillis() + 60_000;
        do {
            for (AccessibilityWindowInfo window : test.automation.getWindows()) {
                AccessibilityNodeInfo ready = test.findAny(window.getRoot(), node -> isScheme(node) && node.isEnabled());
                if (ready != null) return;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Pinyin session did not become ready in the original editor");
    }

    private static boolean isScheme(AccessibilityNodeInfo node) {
        return DeviceSmoke.equalsText(PRODUCT, node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("输入方案：全拼");
    }

    private static Predicate<AccessibilityNodeInfo> key(String text) {
        return node -> DeviceSmoke.equalsText(PRODUCT, node.getPackageName())
            && node.getText() != null && text.equalsIgnoreCase(node.getText().toString())
            && (text.length() != 1 || DeviceSmoke.equalsText("android.widget.Button", node.getClassName()));
    }

    /** 验收系统编辑器的真实组字标记，避免把相同的已提交字母误认为拼音组字。 */
    private static void assertComposition(DeviceSmoke test, EditorActivity activity,
            boolean composing, String expected) {
        test.runOnMainSync(() -> {
            ArrayList<View> fields = new ArrayList<>();
            activity.getWindow().getDecorView().findViewsWithText(fields, "msime-test-plain",
                View.FIND_VIEWS_WITH_CONTENT_DESCRIPTION);
            if (fields.size() != 1 || !(fields.get(0) instanceof EditText editor))
                throw new AssertionError("Synthetic editor unavailable");
            if (!expected.contentEquals(editor.getText()))
                throw new AssertionError("Synthetic pinyin editor text mismatch");
            int start = BaseInputConnection.getComposingSpanStart(editor.getText());
            int end = BaseInputConnection.getComposingSpanEnd(editor.getText());
            if (composing ? start < 0 || end <= start : start >= 0 || end >= 0)
                throw new AssertionError("Synthetic editor composition marker mismatch");
        });
    }
}
