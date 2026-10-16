package app.msime.android.test;

/** Device-only acceptance for Shift helpcode during a Chinese composition. */
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

        stage = "helpcode shift";
        tap(key("⇧"));
        await(key("N"));
        stage = "helpcode letter";
        tap(key("N"));
        // 组字现在写进输入框（DeviceSmoke 里打 nihao 输入框显示 nihao）：辅助码字母以大写进入组字、没有上屏，一次性的 Shift 随之用掉。中文模式的字母键面一直画大写，`key()` 按字母而不按大小写找键，Shift 前后画哪种大小写由 `LetterKeyFacePolicySmoke` 覆盖。这个测试写于组字还不进输入框的时候，原先断言输入框为空。
        await(field("msime-test-plain").and(node -> equalsText("nN", node.getText())));
        await(key("n"));
    }
}
