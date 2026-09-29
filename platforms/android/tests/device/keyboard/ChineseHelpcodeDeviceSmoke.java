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
        await(imeTextContains("n"));

        stage = "helpcode shift";
        tap(key("⇧"));
        await(key("N"));
        stage = "helpcode letter";
        tap(key("N"));
        await(field("msime-test-plain").and(node -> equalsText("", node.getText())));
        await(key("n"));
    }
}
