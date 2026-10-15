package app.msime.android.test;

import android.os.SystemClock;
import android.view.KeyEvent;

/** 中文组合中的键盘 Shift 辅助码验收。 */
public final class ChineseHelpcodeDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Chinese composition helpcode, Engine narrowing and one-shot Shift reset";
    }

    @Override protected void runChecks() throws Exception {
        android.content.Intent intent = new android.content.Intent(
            getTargetContext(), EditorActivity.class);
        intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK
            | android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        stage = "helpcode composition start";
        tap(field("msime-test-plain"));
        tap(key("n"));
        // 拼写写在输入框里，候选栏不再画它。
        await(field("msime-test-plain").and(node -> equalsText("n", node.getText())));

        stage = "keyboard Shift letter";
        // 中文全拼触屏已改为分词键；带 Shift 的键盘事件仍保留辅助码语义。
        long time = SystemClock.uptimeMillis();
        KeyEvent letter = new KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_N,
            0, KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON);
        sendKeySync(letter);
        sendKeySync(KeyEvent.changeAction(letter, KeyEvent.ACTION_UP));
        // 组字现在写进输入框（DeviceSmoke 里打 nihao 输入框显示 nihao）：辅助码字母以大写进入组字、没有上屏，一次性的 Shift 用掉后键面回到小写。这个测试写于组字还不进输入框的时候，原先断言输入框为空。
        await(field("msime-test-plain").and(node -> equalsText("nN", node.getText())));
        await(key("n"));
    }
}
