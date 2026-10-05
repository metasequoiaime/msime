package app.msime.android.test;

import android.content.Intent;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** Device-only acceptance for the dedicated, paged Apple-style emoji browser. */
public final class EmojiPickerDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "composition order, category pages, insertion, deletion, recents and restart";
    }

    @Override protected void runChecks() throws Exception {
        openEditor();
        stage = "emoji composition";
        tap(key("n"));
        tap(key("i"));
        await(field("msime-test-plain").and(node -> equalsText("ni", node.getText())));

        // The candidate strip takes the shortcut row while a composition is open, the way Apple
        // has it, so 更多 is only reachable once the composition is done. Commit it first and let
        // the rest of this case derive the committed prefix from the field.
        stage = "composition committed before emoji";
        tap(key("空格"));
        await(key("更多").and(AccessibilityNodeInfo::isClickable));

        stage = "emoji more entry";
        tap(key("更多"));
        tap(description("表情"));
        await(description("表情面板"));

        stage = "emoji category navigation";
        tap(description("表情分类 笑脸"));
        await(description("表情分类 笑脸").and(AccessibilityNodeInfo::isSelected));
        stage = "bounded first emoji page";
        await(emojiCountAtLeast(64));
        AccessibilityNodeInfo grid = await(description("表情网格；每行八个"));
        if (!grid.isScrollable()) throw new AssertionError("Emoji grid was not scrollable");

        stage = "emoji page advance";
        for (int attempt = 0; attempt < 8; attempt++) {
            grid = await(description("表情网格；每行八个"));
            grid.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(150);
        }
        await(emojiCountAbove(64));

        stage = "emoji insertion";
        tap(description("按键 表情 😀"));
        stage = "emoji panel return";
        tap(description("返回键盘"));
        await(description("表情").and(AccessibilityNodeInfo::isClickable));
        AccessibilityNodeInfo editor = await(field("msime-test-plain").and(node ->
            node.getText() != null && node.getText().toString().endsWith("😀")
                && node.getText().length() > "😀".length()));
        String committedWithEmoji = editor.getText().toString();
        String finishedPrefix = committedWithEmoji.substring(
            0, committedWithEmoji.length() - "😀".length());

        stage = "emoji recent entry";
        tap(description("表情"));
        await(description("表情分类 最近").and(AccessibilityNodeInfo::isSelected));
        stage = "emoji deletion";
        tap(description("删除"));
        tap(description("返回键盘"));
        await(field("msime-test-plain").and(node -> equalsText(finishedPrefix, node.getText())));
        stage = "emoji recent insertion";
        tap(description("表情"));
        await(description("表情分类 最近").and(AccessibilityNodeInfo::isSelected));
        tap(description("按键 表情 😀"));
        tap(description("返回键盘"));
        await(field("msime-test-plain").and(node -> equalsText(committedWithEmoji, node.getText())));

        stage = "emoji recents survive restart";
        rebindInputMethod();
        openEditor();
        tap(description("表情"));
        await(description("表情分类 最近").and(AccessibilityNodeInfo::isSelected));
        await(description("按键 表情 😀").and(AccessibilityNodeInfo::isClickable));
        tap(description("返回键盘"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    private void openEditor() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        tap(field("msime-test-plain"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    private void rebindInputMethod() throws Exception {
        shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    private Predicate<AccessibilityNodeInfo> emojiCountAtLeast(int minimum) {
        return emojiCount(value -> value >= minimum);
    }

    private Predicate<AccessibilityNodeInfo> emojiCountAbove(int minimum) {
        return emojiCount(value -> value > minimum);
    }

    private Predicate<AccessibilityNodeInfo> emojiCount(
            java.util.function.IntPredicate accepted) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || node.getText() == null) return false;
            String text = node.getText().toString();
            // 状态行形如「笑脸 · 116 个表情」，数量是「 个表情」前的最后一个词。
            int separator = text.indexOf(" 个表情");
            if (separator <= 0) return false;
            String count = text.substring(text.lastIndexOf(' ', separator - 1) + 1, separator);
            try { return accepted.test(Integer.parseInt(count)); }
            catch (NumberFormatException ignored) { return false; }
        };
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }
}
