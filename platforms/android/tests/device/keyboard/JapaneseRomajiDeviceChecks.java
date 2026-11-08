package app.msime.android.test;

/** 经产品键盘与真实编辑器验证长音留在日语读音里，不直接上屏减号。 */
final class JapaneseRomajiDeviceChecks {
    private JapaneseRomajiDeviceChecks() { }

    static void run(DeviceSmoke test) throws Exception {
        test.stage = "日语罗马字长音键就绪";
        test.await(test.key("ー").and(node -> "android.widget.Button".contentEquals(node.getClassName())));
        test.stage = "长音接入连续读音";
        for (String key : new String[] {"k", "o", "ー", "h", "i", "ー"}) test.tap(test.key(key));
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null && "こーひー".contentEquals(node.getText())));
        test.stage = "删除长音后继续组词";
        test.tap(test.key("⌫"));
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null && "こーひ".contentEquals(node.getText())));
        test.tap(test.key("ー").and(node -> "android.widget.Button".contentEquals(node.getClassName())));
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null && "こーひー".contentEquals(node.getText())));
        // 罗马字预编辑共六个键；全部撤回后单独输入长音，避免依赖词典排序。
        for (int index = 0; index < 6; index++) test.tap(test.key("⌫"));
        test.await(test.field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        test.stage = "单独输入并提交长音";
        test.tap(test.key("ー").and(node -> "android.widget.Button".contentEquals(node.getClassName())));
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null && "ー".contentEquals(node.getText())));
        test.tap(test.key("確定"));
        test.await(test.field("msime-test-plain").and(node -> node.getText() != null && "ー".contentEquals(node.getText())));
    }
}
