package app.msime.android;

import android.text.InputType;
import java.text.BreakIterator;

/**
 * 光标已在文档开头或末尾时，往外的方向键不再发给编辑器（#10141）。
 *
 * <p>编辑器处理不了的 DPAD 会被系统当作焦点导航：原生输入框把焦点交给相邻的可聚焦控件，WebView 和 GeckoView 也可能借此换焦点或重启连接，文本编辑面板随之被关掉。触控板、面板方向键和空格拖动都经这里决定一批键里实际发几个。
 *
 * <p>每批只在发键前读一次（编辑器处理按键是异步的，发完一个键马上回读常常还是旧文字），按读到的那一侧算出最多能走几步：横向按字素数，纵向按换行数（软换行只会让真实行数更多，所以按换行数不会多走），已在边界时一步也不走。以下情况照原样发键：`TYPE_NULL` 编辑器（终端一类要原样收到方向键），读不到文字的编辑器，以及两侧都读出空串、又不能经 `getExtractedText` 确认全文为空的编辑器（自己处理方向键、不维护文字的远程终端和远程桌面是这样）。密码框不读文字，只凭编辑器报来的选区拦住开头一侧。
 */
public final class TextCursorEdgePolicy {
    /** 一批里横向、纵向各发几个键，正负号与请求相同。 */
    public record Plan(int horizontal, int vertical) { }

    /** 发键前对编辑器的同步回读，只在需要时调用；读不到返回 null。 */
    public interface Editor {
        /** 光标（选区起点）前至多 limit 个字符。 */
        CharSequence before(int limit);

        /** 光标（选区终点）后至多 limit 个字符。 */
        CharSequence after(int limit);

        /** 编辑器此刻选中的文字，没有选区时为 null 或空串。只在收起选区那一下要越过边界时读，密码框不读。 */
        CharSequence selected();

        /** 编辑器能否经另一条途径（`getExtractedText`）确认全文就是空的；不提供抽取文字的编辑器为 false。只在两侧都读出空串时问，密码框不问。 */
        boolean confirmsEmpty();
    }

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
     */
    public static Plan plan(int horizontal, int vertical, boolean selecting, int selectionStart, int selectionEnd,
            int inputType, Editor editor) {
        Plan requested = new Plan(horizontal, vertical);
        if ((horizontal == 0 && vertical == 0)
                || (inputType & InputType.TYPE_MASK_CLASS) == InputType.TYPE_NULL) return requested;
        boolean ranged = selectionStart >= 0 && selectionEnd >= 0 && selectionStart != selectionEnd;
        boolean password = EditorPolicy.password(inputType);
        int limit = Math.min(MAX_READ, Math.max(MIN_READ,
            CHARS_PER_STEP * (Math.abs(horizontal) + Math.abs(vertical))));
        Room backward = horizontal < 0 || vertical < 0
            ? room(false, selecting, ranged, selectionStart, selectionEnd, password, editor, limit) : null;
        Room forward = horizontal > 0 || vertical > 0
            ? room(true, selecting, ranged, selectionStart, selectionEnd, password, editor, limit) : null;
        if (!password && !ranged && (Room.empty(backward) || Room.empty(forward))) {
            CharSequence rest = Room.empty(backward)
                ? (forward != null ? forward.text : editor.after(1))
                : (backward != null ? backward.text : editor.before(1));
            // 两侧都是空的：编辑器经 getExtractedText 确认全文为空，才是真的空输入框，照常拦；确认不了的多半不维护文字、自己处理方向键，照原样发。
            if (rest != null && rest.length() == 0 && !editor.confirmsEmpty()) return requested;
        }
        Room horizontalRoom = horizontal < 0 ? backward : forward;
        Room verticalRoom = vertical < 0 ? backward : forward;
        // 不在选择模式时，有选区的第一下只把它收起成光标。选区取的是 onUpdateSelection 的回报，上一批刚把它收起、回报还没到时会过时；收起那一下正好要越过边界时先确认编辑器此刻还有选区。密码框不读，仍按回报算。
        boolean collapse = ranged && !selecting;
        if (collapse && !password && collapseCrossesEdge(horizontal, vertical, horizontalRoom, verticalRoom)) {
            CharSequence selected = editor.selected();
            collapse = selected != null && selected.length() > 0;
        }
        int sentHorizontal = horizontal == 0 ? 0 : allowed(Math.abs(horizontal), false, horizontalRoom, collapse, false);
        boolean sameSide = horizontal != 0 && vertical != 0 && (horizontal < 0) == (vertical < 0);
        // 同一侧先走了横向，纵向只剩横向没用掉的那段文字；收起选区的那一下不消耗文字。
        if (sameSide && verticalRoom != null && sentHorizontal > 0)
            verticalRoom = verticalRoom.drop(collapse ? sentHorizontal - 1 : sentHorizontal);
        boolean verticalCollapse = collapse && sentHorizontal == 0;
        // 原生输入框只有整段选中时上下键收起选区后才把键交还系统（Selection.moveUp / moveDown 返回 false），所以要看另一侧还有没有文字。
        boolean otherSideHasText = verticalCollapse && verticalRoom != null && verticalRoom.clusters() == 0
            && !password && hasText(vertical < 0 ? forward : backward, vertical < 0, editor);
        int sentVertical = vertical == 0 ? 0 : allowed(Math.abs(vertical), true, verticalRoom, verticalCollapse,
            otherSideHasText);
        return new Plan(Integer.signum(horizontal) * sentHorizontal, Integer.signum(vertical) * sentVertical);
    }

    /** 收起选区的那一下是否正是越过边界的那一下：横向请求比剩下的字素多，或纵向那一侧已经没有文字。 */
    private static boolean collapseCrossesEdge(int horizontal, int vertical, Room horizontalRoom, Room verticalRoom) {
        return (horizontal != 0 && horizontalRoom != null && Math.abs(horizontal) > horizontalRoom.clusters())
            || (vertical != 0 && verticalRoom != null && verticalRoom.clusters() == 0);
    }

    /** 选区另一侧（纵向移动的反方向）还有没有文字；已读过就用读过的，读不到按没有算。 */
    private static boolean hasText(Room known, boolean forward, Editor editor) {
        if (known != null && known.text != null) return known.clusters() > 0;
        CharSequence text = forward ? editor.after(1) : editor.before(1);
        return text != null && text.length() > 0;
    }

    /** 移动这一侧还剩多少；null 表示不知道，照原样发。 */
    private static Room room(boolean forward, boolean selecting, boolean ranged, int selectionStart,
            int selectionEnd, boolean password, Editor editor, int limit) {
        // 选择模式下活动端朝锚点收缩：先走完选区里的字，再走锚点外侧的文字。选区里的字按 UTF-16 长度算，选区里有代理对或组合附加符时比真实字素数多，一批很长、正好走到文档边界时仍可能多发几下。
        boolean shrinking = selecting && ranged
            && (forward ? selectionEnd < selectionStart : selectionEnd > selectionStart);
        if (password) {
            // 密码框零读取：开头一侧看报来的活动端（不在选择模式时是选区起点），末尾一侧不知道文档长度。
            if (forward || selectionStart < 0 || selectionEnd < 0) return null;
            return Room.offset(selecting ? selectionEnd : Math.min(selectionStart, selectionEnd));
        }
        CharSequence text = forward ? editor.after(limit) : editor.before(limit);
        if (text == null) return null;
        return new Room(text, forward, shrinking ? Math.abs(selectionEnd - selectionStart) : 0);
    }

    private static int allowed(int count, boolean vertical, Room room, boolean collapse, boolean otherSideHasText) {
        if (room == null) return count;
        int clusters = room.clusters();
        if (vertical) {
            if (clusters == 0) return collapse && otherSideHasText ? 1 : 0;
            return collapse ? 1 : Math.min(count, Math.max(1, room.lineBreaks()));
        }
        return Math.min(count, clusters + (collapse ? 1 : 0));
    }

    /** 移动方向一侧的文字（选择模式收缩时加上选区里的字），或密码框里只知道的字符数。 */
    private static final class Room {
        final CharSequence text;
        final boolean forward;
        final int offset;
        /** 活动端朝锚点收缩时选区里还剩的字符数，离光标最近，没有换行可数。 */
        final int inside;

        Room(CharSequence text, boolean forward, int inside) {
            this.text = text;
            this.forward = forward;
            this.offset = -1;
            this.inside = Math.max(0, inside);
        }

        private Room(int offset) {
            this.text = null;
            this.forward = false;
            this.offset = Math.max(0, offset);
            this.inside = 0;
        }

        static Room offset(int offset) { return new Room(offset); }

        static boolean empty(Room room) {
            return room != null && room.text != null && room.text.length() == 0 && room.inside == 0;
        }

        int clusters() {
            if (text == null) return offset;
            BreakIterator iterator = BreakIterator.getCharacterInstance();
            iterator.setText(text.toString());
            int count = 0;
            iterator.first();
            while (iterator.next() != BreakIterator.DONE) count++;
            return inside + count;
        }

        /** 只数锚点外侧文字里的换行；选区里的行数不读不知道，按 0 算，宁可少走。 */
        int lineBreaks() {
            if (text == null) return 0;
            int count = 0;
            for (int at = 0; at < text.length(); at++) if (text.charAt(at) == '\n') count++;
            return count;
        }

        /** 去掉离光标最近的 n 个字素：先扣选区里的字，再扣外侧文字。 */
        Room drop(int clusters) {
            if (clusters <= 0) return this;
            if (text == null) return new Room(offset - clusters);
            if (clusters <= inside) return new Room(text, forward, inside - clusters);
            clusters -= inside;
            String value = text.toString();
            BreakIterator iterator = BreakIterator.getCharacterInstance();
            iterator.setText(value);
            int cut;
            if (forward) {
                iterator.first();
                cut = iterator.next(clusters);
                return new Room(cut == BreakIterator.DONE ? "" : value.substring(cut), true, 0);
            }
            iterator.last();
            cut = iterator.next(-clusters);
            return new Room(cut == BreakIterator.DONE ? "" : value.substring(0, cut), false, 0);
        }
    }
}
