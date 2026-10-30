import app.msime.android.TextTrackpadSelectionVersion;

/** 按主线程上的实际先后重放连点回读与选区回报，不靠定时制造竞态。 */
public final class TextTrackpadSelectionVersionSmoke {
    public static void main(String[] args) {
        // 快速三击：第二下的选词结果在第三下按下前已写进编辑器，它的回报却排在第三下松手之后才到。
        TextTrackpadSelectionVersion version = new TextTrackpadSelectionVersion();
        version.invalidate();
        long word = version.current();
        check(word == version.current(), "选词回读发布时代次未变");
        version.applied(6, 10);
        version.invalidate();
        long line = version.current();
        check(word != line, "新的一下作废上一下的回读");
        version.reported(6, 10);
        check(line == version.current(), "触控板自己设下的选区回报不作废选行回读");

        // 回报只认一次：同一段选区再报一次就是编辑器自己的变化。
        version.reported(6, 10);
        check(line != version.current(), "重复回报按外部变化处理");

        // 外部改动照旧作废在途回读。
        long pending = version.current();
        version.reported(8, 8);
        check(pending != version.current(), "外部选区变化作废在途回读");

        // 更早的外部回报先到时，不吞掉随后到达的自身回报。
        version.applied(0, 10);
        version.invalidate();
        long afterStale = version.current();
        version.reported(8, 8);
        check(afterStale != version.current(), "先到的旧回报照旧作废");
        long afterEcho = version.current();
        version.reported(0, 10);
        check(afterEcho == version.current(), "随后的自身回报仍被识别");

        // 换编辑器或关面板后，尚未回报的自身选区不再算数。
        version.applied(23, 35);
        version.reset();
        long reset = version.current();
        version.reported(23, 35);
        check(reset != version.current(), "重置后的同值回报按外部变化处理");

        // 选区本来就是这一段时 setSelection 不改变什么，编辑器不发回报；记录只陪紧随其后的一下触摸，再下一下触摸就作废，之后的同值回报照旧作废在途回读。
        version.applied(0, 10);
        version.invalidate();
        version.invalidate();
        long doubleTap = version.current();
        version.reported(0, 10);
        check(doubleTap != version.current(), "没等到回报的自身选区过了一下触摸就不再算数");
        System.out.println("Android text trackpad selection version: own echoes, outside changes, unanswered selections and reset passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
