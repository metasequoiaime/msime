package app.msime.android.test;

import android.content.Intent;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 邮箱后缀（#6147）的设备验收：邮箱输入框和普通输入框里打到 `abc@` 后候选栏都给出后缀、点一下补全；密码框里的 `@` 不给。普通输入框进框是中文，`abc` 先作为拼音组字，点 123 层的 `@` 时按原文上屏再接半角 `@`，覆盖了中文模式下的这条路。覆盖宿主接线（选区变化后重新匹配、候选栏分支、batch edit 里的上屏），这些在 JVM 层没有测试。 */
public final class EmailSuffixDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "email suffix chips and completion in email and plain fields, and no chips in a password field";
    }

    @Override protected void runChecks() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);

        stage = "email field typing";
        tap(field("msime-test-email"));
        for (String key : new String[] {"a", "b", "c"}) tap(key(key));
        stage = "email field at sign";
        tapSymbol("@");
        await(field("msime-test-email").and(node -> equalsText("abc@", node.getText())));
        stage = "email suffix chips";
        await(suffixChip("@qq.com"));
        stage = "email suffix completion";
        tap(suffixChip("@163.com"));
        await(field("msime-test-email").and(node -> equalsText("abc@163.com", node.getText())));
        stage = "email suffix chips gone after completion";
        assertNoSuffixChips();

        // 普通输入框（TYPE_CLASS_TEXT）里同样给：聊天框、备注里写邮箱地址也能补全。
        stage = "plain field typing";
        tap(field("msime-test-plain"));
        for (String key : new String[] {"a", "b", "c"}) tap(key(key));
        stage = "plain field at sign";
        tapSymbol("@");
        await(field("msime-test-plain").and(node -> node.getText() != null
            && node.getText().toString().endsWith("abc@")));
        stage = "plain field suffix chips";
        await(suffixChip("@qq.com"));
        stage = "plain field suffix completion";
        tap(suffixChip("@gmail.com"));
        await(field("msime-test-plain").and(node -> node.getText() != null
            && node.getText().toString().endsWith("abc@gmail.com")));
        stage = "plain field chips gone after completion";
        assertNoSuffixChips();

        // 密码框里不读上文，打完 abc@ 也不出现后缀。密码框的无障碍文字是掩码，只核对长度。
        stage = "password field typing";
        tap(field("msime-test-password"));
        await(field("msime-test-password").and(AccessibilityNodeInfo::isFocused));
        for (String key : new String[] {"a", "b", "c"}) tap(key(key));
        stage = "password field at sign";
        tapSymbol("@");
        await(field("msime-test-password").and(node -> node.getText() != null && node.getText().length() == 4));
        stage = "password field shows no suffix";
        assertNoSuffixChips();
    }

    /** 候选栏里的后缀按钮，无障碍描述是「邮箱后缀 n：@163.com」。 */
    private Predicate<AccessibilityNodeInfo> suffixChip(String suffix) {
        return describedPrefix("邮箱后缀 ").and(node -> node.getContentDescription().toString().endsWith("：" + suffix));
    }

    /** 等一会儿（选区变化后的匹配和渲染都在主线程上，几百毫秒内完成），再确认看得见的输入法窗口里没有任何后缀按钮。 */
    private void assertNoSuffixChips() throws java.util.concurrent.TimeoutException {
        automation.waitForIdle(500, 5000);
        SystemClock.sleep(800);
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (find(window.getRoot(), describedPrefix("邮箱后缀 ")) != null)
                throw new AssertionError("Email suffix chips appeared where they should not; IME showed " + imeTexts());
        }
    }
}
