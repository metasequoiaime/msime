package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;

/** 表情面板的分类布局、默认的水杉娘表情包、真实滑动、分页和上屏设备验收。 */
public final class EmojiPickerDeviceSmoke extends DeviceSmoke {
    /** 表情包分类签的无障碍描述（StickerPackModel.TAB_DESCRIPTION）；APK 总是带着表情包，所以它一定在。 */
    private static final String STICKER_TAB = "水杉娘表情包";

    @Override protected String successDescription() {
        return "category width, sticker pack first and default, horizontal gestures, vertical pages, insertion, deletion, recents and restart";
    }

    @Override protected void runChecks() throws Exception {
        restartIme();
        openEditor();
        stage = "emoji composition";
        tap(key("n"));
        tap(key("i"));
        await(field("msime-test-plain").and(node -> equalsText("ni", node.getText())));

        // 组词时候选条占着顶部一行，工具栏的「表情」要等组词结束才出现；先上屏，后面的前缀从输入框里读。
        stage = "composition committed before emoji";
        tap(key("空格"));
        await(description("表情").and(AccessibilityNodeInfo::isClickable));

        stage = "emoji toolbar entry";
        tap(description("表情"));
        await(description("表情面板"));
        stage = "emoji panel stays inside the keyboard";
        assertPanelBelowEditor();
        stage = "emoji categories use the former ABC space";
        AccessibilityNodeInfo panel = await(description("表情面板"));
        if (find(panel, node -> equalsText("ABC", node.getText())) != null)
            throw new AssertionError("Emoji panel still contains the duplicate ABC return key");
        Rect panelBounds = new Rect();
        Rect categoryBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
        await(description("表情分类")).getBoundsInScreen(categoryBounds);
        int chromeWidth = Math.round(90 * getTargetContext().getResources().getDisplayMetrics().density);
        if (categoryBounds.width() < panelBounds.width() - chromeWidth)
            throw new AssertionError("Emoji categories did not receive the freed return-key width");

        stage = "sticker pack is the first and default category";
        AccessibilityNodeInfo firstTab = await(description("表情分类")).getChild(0);
        if (firstTab == null || !equalsText(STICKER_TAB, firstTab.getContentDescription()))
            throw new AssertionError("The sticker pack was not the first emoji category");
        await(description(STICKER_TAB).and(AccessibilityNodeInfo::isSelected));
        // Pixel 6 竖屏：一行五格。
        await(description("表情网格；" + STICKER_TAB + "，每行 5 个"));

        stage = "emoji category navigation";
        tap(description("表情分类 笑脸"));
        await(description("表情分类 笑脸").and(AccessibilityNodeInfo::isSelected));
        stage = "bounded first emoji page";
        AccessibilityNodeInfo grid = await(description("表情网格；每行八个"));
        // 首页按 PAGE_SIZE（64）向目录要一批，再去掉这台设备字体画不出来的（`hasGlyph`），所以只能断言它是一批、不是整个分类：模拟器的字体缺几个新表情，笑脸首页是 55 个。
        try {
            await(emojiCount(value -> value > 0 && value <= 64));
        } catch (AssertionError error) {
            grid.refresh();
            throw new AssertionError("Emoji grid state was " + grid.getStateDescription());
        }
        grid.refresh();
        int firstPage = emojiCountOf(grid);
        if (!grid.isScrollable()) throw new AssertionError("Emoji grid was not scrollable");

        stage = "horizontal category gestures do not insert emoji";
        String beforeSwipe = await(field("msime-test-plain")).getText().toString();
        swipe(-1, false);
        await(description("表情分类 人物").and(AccessibilityNodeInfo::isSelected));
        await(emojiCount(value -> value > 0));
        assertEditorText(beforeSwipe);
        swipe(1, false);
        await(description("表情分类 笑脸").and(AccessibilityNodeInfo::isSelected));
        await(emojiCount(value -> value > 0));
        assertEditorText(beforeSwipe);
        stage = "first category and recents boundary";
        // 往右划依次经过「最近」（有记录时）到第一个分类表情包，再划不回绕，也不发出表情包。
        boolean hasRecents = find(await(description("表情面板")), description("表情分类 最近")) != null;
        if (hasRecents) {
            swipe(1, false);
            await(description("表情分类 最近").and(AccessibilityNodeInfo::isSelected));
        }
        swipe(1, false);
        await(description(STICKER_TAB).and(AccessibilityNodeInfo::isSelected));
        swipe(1, false);
        await(description(STICKER_TAB).and(AccessibilityNodeInfo::isSelected));
        assertEditorText(beforeSwipe);
        tap(description("表情分类 笑脸"));
        stage = "cancelled horizontal gesture keeps category";
        swipe(-1, false, true);
        await(description("表情分类 笑脸").and(AccessibilityNodeInfo::isSelected));
        assertEditorText(beforeSwipe);
        stage = "flags swipe reaches combinations";
        tap(description("表情分类 旗帜"));
        await(description("表情分类 旗帜").and(AccessibilityNodeInfo::isSelected));
        swipe(-1, false);
        await(description("表情分类 组合表情").and(AccessibilityNodeInfo::isSelected));
        await(description("表情网格；组合表情自动适配宽度"));
        assertEditorText(beforeSwipe);
        stage = "select last category";
        AccessibilityNodeInfo tabs = await(description("表情分类"));
        AccessibilityNodeInfo lastTab = tabs.getChild(tabs.getChildCount() - 1);
        if (lastTab == null || lastTab.getContentDescription() == null)
            throw new AssertionError("Last emoji category was not exposed");
        // 分类可能继续追加，边界以当前栏的末项为准，不把某个 Unicode 分类写死为末项。
        String lastCategory = lastTab.getContentDescription().toString();
        tap(description(lastCategory));
        await(description(lastCategory).and(AccessibilityNodeInfo::isSelected));
        stage = "last category does not wrap";
        swipe(-1, false);
        await(description(lastCategory).and(AccessibilityNodeInfo::isSelected));
        assertEditorText(beforeSwipe);
        tap(description("表情分类 笑脸"));
        await(emojiCount(value -> value > 0));
        stage = "vertical gesture keeps the selected category";
        swipe(-1, true);
        await(description("表情分类 笑脸").and(AccessibilityNodeInfo::isSelected));
        assertEditorText(beforeSwipe);

        stage = "emoji page advance";
        for (int attempt = 0; attempt < 8; attempt++) {
            grid = await(description("表情网格；每行八个"));
            grid.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(150);
        }
        await(emojiCountAbove(firstPage));

        stage = "emoji insertion";
        tap(description("按键 表情 😀"));
        stage = "emoji panel return";
        tap(description("表情"));
        await(description("表情").and(AccessibilityNodeInfo::isClickable));
        AccessibilityNodeInfo editor = await(field("msime-test-plain").and(node ->
            node.getText() != null && node.getText().toString().endsWith("😀")
                && node.getText().length() > "😀".length()));
        String committedWithEmoji = editor.getText().toString();
        String finishedPrefix = committedWithEmoji.substring(
            0, committedWithEmoji.length() - "😀".length());

        stage = "emoji recent entry";
        tap(description("表情"));
        openRecents();
        stage = "emoji deletion";
        tap(description("删除"));
        tap(description("返回键盘"));
        await(field("msime-test-plain").and(node -> equalsText(finishedPrefix, node.getText())));
        stage = "emoji recent insertion";
        tap(description("表情"));
        openRecents();
        tap(description("按键 表情 😀"));
        tap(description("返回键盘"));
        await(field("msime-test-plain").and(node -> equalsText(committedWithEmoji, node.getText())));

        stage = "emoji recents survive restart";
        restartIme();
        openEditor();
        tap(description("表情"));
        openRecents();
        await(description("按键 表情 😀").and(AccessibilityNodeInfo::isClickable));
        tap(description("返回键盘"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    /** 有最近记录时面板仍默认打开表情包，「最近」排在它后面，要点一下才切过去。 */
    private void openRecents() throws Exception {
        await(description(STICKER_TAB).and(AccessibilityNodeInfo::isSelected));
        tap(description("表情分类 最近"));
        await(description("表情分类 最近").and(AccessibilityNodeInfo::isSelected));
    }

    private void assertEditorText(String expected) throws Exception {
        await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
    }

    /** 在网格内注入完整触摸轨迹，横划经过表情按钮，检查父容器确实取消点击。 */
    private void swipe(int direction, boolean vertical) throws Exception {
        swipe(direction, vertical, false);
    }

    private void swipe(int direction, boolean vertical, boolean cancel) throws Exception {
        Rect grid = new Rect();
        await(node -> equalsText("app.msime.android", node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("表情网格；")).getBoundsInScreen(grid);
        float startX = vertical ? grid.centerX() : grid.left + grid.width() * (direction < 0 ? .8f : .2f);
        float startY = vertical ? grid.top + grid.height() * .8f : grid.centerY();
        float endX = vertical ? startX : grid.left + grid.width() * (direction < 0 ? .2f : .8f);
        float endY = vertical ? grid.top + grid.height() * .2f : startY;
        long downTime = SystemClock.uptimeMillis();
        for (int step = 0; step <= 12; step++) {
            int action = step == 0 ? MotionEvent.ACTION_DOWN : step == 12
                ? (cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP) : MotionEvent.ACTION_MOVE;
            float fraction = step / 12f;
            MotionEvent event = MotionEvent.obtain(downTime, downTime + step * 20, action,
                startX + (endX - startX) * fraction, startY + (endY - startY) * fraction, 0);
            event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            try {
                if (!automation.injectInputEvent(event, true)) throw new AssertionError("Emoji gesture injection failed");
            } finally { event.recycle(); }
        }
        SystemClock.sleep(300);
    }

    /** 表情面板覆盖整屏的回归：面板顶边必须在输入框底边之下，即仍在键盘窗口里。 */
    private void assertPanelBelowEditor() throws Exception {
        android.graphics.Rect panel = new android.graphics.Rect();
        android.graphics.Rect editor = new android.graphics.Rect();
        await(description("表情面板")).getBoundsInScreen(panel);
        await(field("msime-test-plain")).getBoundsInScreen(editor);
        if (panel.top < editor.bottom)
            throw new AssertionError("Emoji panel covered the editor: panel " + panel + ", editor " + editor);
    }

    private void openEditor() throws Exception {
        Intent intent = new Intent(getTargetContext(), EditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        startActivitySync(intent);
        tap(field("msime-test-plain"));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }


    private Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    private Predicate<AccessibilityNodeInfo> emojiCountAbove(int minimum) {
        return emojiCount(value -> value > minimum);
    }

    private Predicate<AccessibilityNodeInfo> emojiCount(
            java.util.function.IntPredicate accepted) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && emojiCountOf(node) >= 0 && accepted.test(emojiCountOf(node));
    }

    /** 网格的状态描述形如「笑脸 · 116 个表情」，数量是「 个表情」前的最后一个词；读不出来时是 -1。 */
    private static int emojiCountOf(AccessibilityNodeInfo node) {
        if (node.getStateDescription() == null) return -1;
        String text = node.getStateDescription().toString();
        int separator = text.indexOf(" 个表情");
        if (separator <= 0) return -1;
        String count = text.substring(text.lastIndexOf(' ', separator - 1) + 1, separator);
        try { return Integer.parseInt(count); }
        catch (NumberFormatException ignored) { return -1; }
    }

}
