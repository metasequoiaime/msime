import app.msime.android.TextTrackpadHold;

/** 合成时间与连接身份覆盖临时面板入口；所有结束路径都要求恢复恰好一次。 */
public final class TextTrackpadHoldSmoke {
    public static void main(String[] args) {
        Object editor = new Object();
        Object other = new Object();
        TextTrackpadHold hold = new TextTrackpadHold(500, 8);
        check(hold.down(editor, 10, 10, 100) && hold.pending(), "按下等待系统长按时长");
        check(!hold.activate(editor, 599), "未到时长不打开");
        check(hold.up(editor) == TextTrackpadHold.End.TAP, "短按只走原入口");
        check(hold.up(editor) == TextTrackpadHold.End.NONE, "重复松手不再点击");
        hold.down(editor, 10, 10, 100);
        check(hold.activate(editor, 600) && hold.active(), "长按临时打开");
        check(!hold.activate(editor, 700), "迟到长按回调不重复打开");
        check(hold.move(editor, 1, 400, 800), "接手后允许滑到触控板并移出工具按钮");
        check(hold.up(editor) == TextTrackpadHold.End.RESTORE, "松手恢复之前的面板");
        check(hold.cancel() == TextTrackpadHold.End.NONE, "恢复不会重复发生");
        hold.down(editor, 10, 10, 100);
        check(!hold.move(editor, 1, 30, 10), "长按前滑出取消入口");
        check(!hold.activate(editor, 600) && hold.up(editor) == TextTrackpadHold.End.NONE,
            "取消后迟到计时和松手不打开面板");
        hold.down(editor, 10, 10, 100);
        check(!hold.activate(other, 600) && !hold.pending(), "换连接取消等待");
        hold.down(editor, 10, 10, 100);
        hold.activate(editor, 600);
        check(!hold.move(other, 1, 10, 10), "旧连接不能继续操作");
        check(hold.cancel() == TextTrackpadHold.End.RESTORE, "连接失效仍交还临时面板一次");
        hold.down(editor, 10, 10, 100);
        hold.activate(editor, 600);
        check(!hold.move(editor, 2, 10, 10), "多指取消临时触摸");
        check(hold.cancel() == TextTrackpadHold.End.RESTORE, "多指后恢复原面板");
        hold.down(editor, 10, 10, 100);
        check(hold.cancel() == TextTrackpadHold.End.NONE && !hold.pending(), "隐藏时清除计时");
        check(!hold.down(null, 0, 0, 0) && !hold.down(editor, Float.NaN, 0, 0),
            "拒绝空连接和无效坐标");
        System.out.println("Android text trackpad hold: tap, temporary activation, ownership and restoration passed");
    }
    private static void check(boolean ok, String message) {
        if (!ok) throw new AssertionError(message);
    }
}
