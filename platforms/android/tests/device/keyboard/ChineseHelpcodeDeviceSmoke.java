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
        injectKeyboardEvent(letter);
        injectKeyboardEvent(KeyEvent.changeAction(letter, KeyEvent.ACTION_UP));
        // 物理 Shift 把辅助码字母写成大写组字，不提交；中文键面仍按当前方案显示，不以键面大小写判断事件结果。
        await(field("msime-test-plain").and(node -> equalsText("nN", node.getText())));
        await(key("n"));
    }
}
