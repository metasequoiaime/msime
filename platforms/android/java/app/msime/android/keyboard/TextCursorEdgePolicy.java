package app.msime.android;

import android.text.InputType;
import java.text.BreakIterator;
import java.util.function.IntFunction;

/**
 * 光标已在文档开头或末尾时，往外的方向键不再发给编辑器（#10141）。
 *
 * <p>编辑器处理不了的 DPAD 会被系统当作焦点导航：原生输入框把焦点交给相邻的可聚焦控件，WebView 和 GeckoView 也可能借此换焦点或重启连接，文本编辑面板随之被关掉。触控板、面板方向键和空格拖动都经这里决定一批键里实际发几个。
 *
 * <p>每批只在发键前读一次（编辑器处理按键是异步的，发完一个键马上回读常常还是旧文字），按读到的那一侧算出最多能走几步：横向按字素数，纵向按换行数（软换行只会让真实行数更多，所以按换行数不会多走），已在边界时一步也不走。以下情况照原样发键：`TYPE_NULL` 编辑器（终端一类要原样收到方向键），读不到文字的编辑器，以及整个文档读出来都是空的编辑器（自己处理方向键、不维护文字的远程终端也是这样）。密码框不读文字，只凭编辑器报来的选区拦住开头一侧。
 */
public final class TextCursorEdgePolicy {
    /** 一批里横向、纵向各发几个键，正负号与请求相同。 */
    public record Plan(int horizontal, int vertical) { }

    static final int MIN_READ = 256;
    static final int MAX_READ = 1024;
    private static final int CHARS_PER_STEP = 16;

    private TextCursorEdgePolicy() { }

    /**
     * @param horizontal 请求的横向步数，负数向左
     * @param vertical 请求的纵向步数，负数向上
     * @param selecting 是否带 Shift 延伸选区
     * @param selectionStart 编辑器最近报来的选区锚点，未知为 -1
     * @param selectionEnd 编辑器最近报来的选区活动端，未知为 -1
     * @param before 读光标（选区起点）前至多 n 个字符，读不到返回 null
     * @param after 读光标（选区终点）后至多 n 个字符，读不到返回 null
     */
    public static Plan plan(int horizontal, int vertical, boolean selecting, int selectionStart, int selectionEnd,
            int inputType, IntFunction<CharSequence> before, IntFunction<CharSequence> after) {
        Plan requested = new Plan(horizontal, vertical);
        if ((horizontal == 0 && vertical == 0)
                || (inputType & InputType.TYPE_MASK_CLASS) == InputType.TYPE_NULL) return requested;
        boolean ranged = selectionStart >= 0 && selectionEnd >= 0 && selectionStart != selectionEnd;
        // 不在选择模式时，有选区的第一下只把它收成光标，不移动。
        boolean collapse = ranged && !selecting;
        boolean password = EditorPolicy.password(inputType);
        int limit = Math.min(MAX_READ, Math.max(MIN_READ,
            CHARS_PER_STEP * (Math.abs(horizontal) + Math.abs(vertical))));
        Room backward = horizontal < 0 || vertical < 0
            ? room(false, selecting, ranged, selectionStart, selectionEnd, password, before, limit) : null;
        Room forward = horizontal > 0 || vertical > 0
            ? room(true, selecting, ranged, selectionStart, selectionEnd, password, after, limit) : null;
        if (!password && !ranged && (Room.empty(backward) || Room.empty(forward))) {
            // 整个文档都是空的：多半是不维护文字、自己处理方向键的编辑器，照原样发。
            CharSequence rest = Room.empty(backward)
                ? (forward != null ? forward.text : after.apply(1))
                : (backward != null ? backward.text : before.apply(1));
            if (rest != null && rest.length() == 0) return requested;
        }
        Room horizontalRoom = horizontal < 0 ? backward : forward;
        int sentHorizontal = horizontal == 0 ? 0 : allowed(Math.abs(horizontal), false, horizontalRoom, collapse);
        Room verticalRoom = vertical < 0 ? backward : forward;
        boolean sameSide = horizontal != 0 && vertical != 0 && (horizontal < 0) == (vertical < 0);
        // 同一侧先走了横向，纵向只剩横向没用掉的那段文字；收起选区的那一下不消耗文字。
        if (sameSide && verticalRoom != null && sentHorizontal > 0)
            verticalRoom = verticalRoom.drop(collapse ? sentHorizontal - 1 : sentHorizontal);
        boolean verticalCollapse = collapse && sentHorizontal == 0;
        int sentVertical = vertical == 0 ? 0 : allowed(Math.abs(vertical), true, verticalRoom, verticalCollapse);
        return new Plan(Integer.signum(horizontal) * sentHorizontal, Integer.signum(vertical) * sentVertical);
    }

    /** 移动这一侧还剩多少；null 表示不知道或不会碰到边界，照原样发。 */
    private static Room room(boolean forward, boolean selecting, boolean ranged, int selectionStart,
            int selectionEnd, boolean password, IntFunction<CharSequence> read, int limit) {
        // 选择模式下活动端朝锚点收缩：越过锚点之前碰不到文档边界。
        if (selecting && ranged && (forward ? selectionEnd < selectionStart : selectionEnd > selectionStart))
            return null;
        if (password) {
            // 密码框零读取：开头一侧看报来的选区起点，末尾一侧不知道文档长度。
            if (forward || selectionStart < 0 || selectionEnd < 0) return null;
            return Room.offset(Math.min(selectionStart, selectionEnd));
        }
        CharSequence text = read.apply(limit);
        return text == null ? null : new Room(text, forward);
    }

    private static int allowed(int count, boolean vertical, Room room, boolean collapse) {
        if (room == null) return count;
        int clusters = room.clusters();
        if (vertical) {
            // 原生输入框整段选中时按上下键只收起选区、仍把键交还系统（Selection.moveUp / moveDown 返回 false），所以边界一侧没有文字时连收起也不发。
            if (clusters == 0) return 0;
            return collapse ? 1 : Math.min(count, Math.max(1, room.lineBreaks()));
        }
        return Math.min(count, clusters + (collapse ? 1 : 0));
    }

    /** 移动方向一侧的文字，或密码框里只知道的字符数。 */
    private static final class Room {
        final CharSequence text;
        final boolean forward;
        final int offset;

        Room(CharSequence text, boolean forward) {
            this.text = text;
            this.forward = forward;
            this.offset = -1;
        }

        private Room(int offset) {
            this.text = null;
            this.forward = false;
            this.offset = Math.max(0, offset);
        }

        static Room offset(int offset) { return new Room(offset); }

        static boolean empty(Room room) { return room != null && room.text != null && room.text.length() == 0; }

        int clusters() {
            if (text == null) return offset;
            BreakIterator iterator = BreakIterator.getCharacterInstance();
            iterator.setText(text.toString());
            int count = 0;
            iterator.first();
            while (iterator.next() != BreakIterator.DONE) count++;
            return count;
        }

        int lineBreaks() {
            if (text == null) return 0;
            int count = 0;
            for (int at = 0; at < text.length(); at++) if (text.charAt(at) == '\n') count++;
            return count;
        }

        /** 去掉离光标最近的 n 个字素。 */
        Room drop(int clusters) {
            if (clusters <= 0) return this;
            if (text == null) return new Room(offset - clusters);
            String value = text.toString();
            BreakIterator iterator = BreakIterator.getCharacterInstance();
            iterator.setText(value);
            int cut;
            if (forward) {
                iterator.first();
                cut = iterator.next(clusters);
                return new Room(cut == BreakIterator.DONE ? "" : value.substring(cut), true);
            }
            iterator.last();
            cut = iterator.next(-clusters);
            return new Room(cut == BreakIterator.DONE ? "" : value.substring(0, cut), false);
        }
    }
}
