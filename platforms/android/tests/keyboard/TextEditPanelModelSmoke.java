import android.view.KeyEvent;
import app.msime.android.TextEditPanelModel;
import app.msime.android.TextEditPanelModel.Action;
import app.msime.android.TextEditPanelModel.Key;
import app.msime.android.TextEditLineSelection;
import java.util.EnumSet;
import java.util.List;

public final class TextEditPanelModelSmoke {
    public static void main(String[] args) {
        // 共享默认排列（client-core 的 default_text_edit_button_order），本面板不改它。
        List<Action> order = List.of(Action.COPY, Action.PASTE, Action.SELECT_ALL, Action.CUT,
            Action.SPACE, Action.NUMBER_PAD, Action.DELETE, Action.SELECT_LINE);
        Action lineStart = Action.valueOf("LINE_START"), lineEnd = Action.valueOf("LINE_END");
        check(TextEditPanelModel.upwardSwipeAction(Action.DOCUMENT_START) == lineStart
            && TextEditPanelModel.upwardSwipeAction(Action.DOCUMENT_END) == lineEnd
            && TextEditPanelModel.upwardSwipeAction(Action.SELECT_ALL) == Action.SELECT_LINE,
            "文首、文末、全选上滑保留行级动作");
        check(TextEditPanelModel.upwardSwipeAction(Action.COPY) == null
            && TextEditPanelModel.upwardSwipeDescription(Action.COPY).isEmpty(), "其他侧键没有隐藏上滑动作");
        check(TextEditPanelModel.keys(order).size() == 16, "七个侧键、方向区九格和换行，选择行与行首尾不生成键");
        checkGrid(order);
        java.util.ArrayList<Action> reversed = new java.util.ArrayList<>(order);
        java.util.Collections.reverse(reversed);
        checkGrid(reversed);
        // 选择行排在中间时不占位置，后面的侧键依次补上。
        checkGrid(List.of(Action.DELETE, Action.COPY, Action.SELECT_LINE, Action.CUT, Action.PASTE,
            Action.SELECT_ALL, Action.NUMBER_PAD, Action.SPACE));
        Action[][] grid = grid(order);
        check(TextEditPanelModel.ROWS == 4, "四行网格");
        for (int row = 0; row < 3; row++)
            check(grid[row][2] == Action.LEFT && grid[row][3] == Action.LEFT
                && grid[row][6] == Action.RIGHT && grid[row][7] == Action.RIGHT, "← 和 → 跨方向区上三行: " + row);
        check(grid[0][4] == Action.UP && grid[1][4] == Action.SELECT && grid[2][4] == Action.DOWN,
            "中列从上到下是 ↑、开始选择、↓");
        check(grid[3][2] == Action.DOCUMENT_START && grid[3][4] == Action.TRACKPAD
            && grid[3][6] == Action.DOCUMENT_END && grid[3][8] == Action.RETURN,
            "底排依次是文档开头、触控板开关、文档结尾和换行");
        check(grid[0][0] == Action.COPY && grid[1][0] == Action.PASTE && grid[2][0] == Action.SELECT_ALL
            && grid[3][0] == Action.CUT && grid[0][8] == Action.SPACE
            && grid[1][8] == Action.NUMBER_PAD && grid[2][8] == Action.DELETE,
            "默认左列复制、粘贴、全选、剪切，右列空格、数字、退格；第八个默认动作选择行不显示");

        // 排序页只排七个可见动作，选择行留在原位。
        List<Action> stored = List.of(Action.SELECT_LINE, Action.COPY, Action.CUT, Action.PASTE,
            Action.SELECT_ALL, Action.DELETE, Action.NUMBER_PAD, Action.SPACE);
        List<Action> visible = TextEditPanelModel.visibleOrder(stored);
        check(visible.size() == 7 && !visible.contains(Action.SELECT_LINE), "排序页不列选择行");
        java.util.ArrayList<Action> dragged = new java.util.ArrayList<>(visible);
        java.util.Collections.reverse(dragged);
        List<Action> merged = TextEditPanelModel.mergeVisibleOrder(stored, dragged);
        check(merged.get(0) == Action.SELECT_LINE && merged.subList(1, 8).equals(dragged),
            "保存时选择行留在原位，可见动作按拖好的顺序填回");
        boolean rejected = false;
        try { TextEditPanelModel.mergeVisibleOrder(stored, dragged.subList(0, 6)); }
        catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "少了可见动作的草稿不能保存");

        checkBounds(order, 1080, 640, 9);
        checkBounds(order, 333, 157, 9);
        checkBounds(order, 40, 12, 9);

        // 触控板模式的选择开关贴触控板右上角，触控板太小时缩小而不越界。
        TextEditPanelModel.Bounds chip = TextEditPanelModel.trackpadSelectBounds(100, 10, 700, 400, 64, 32, 4);
        check(chip.right() == 696 && chip.top() == 14 && chip.width() == 64 && chip.height() == 32,
            "选择开关贴右上角: " + chip);
        for (int[] tiny : new int[][] {{0, 0, 20, 10}, {5, 5, 5, 5}, {0, 0, 70, 30}}) {
            chip = TextEditPanelModel.trackpadSelectBounds(tiny[0], tiny[1], tiny[2], tiny[3], 64, 32, 4);
            check(chip.width() >= 0 && chip.height() >= 0 && chip.left() >= tiny[0] && chip.right() <= tiny[2]
                && chip.top() >= tiny[1] && chip.bottom() <= tiny[3], "选择开关不越出触控板: " + chip);
        }

        // 方向键和删除连发，其余不连发。
        for (Key key : TextEditPanelModel.keys(order)) {
            boolean repeats = EnumSet.of(Action.LEFT, Action.RIGHT, Action.UP, Action.DOWN, Action.DELETE)
                .contains(key.action());
            check(key.repeats() == repeats, "repeat flag of " + key.action());
        }

        check(TextEditPanelModel.keyCode(Action.LEFT) == KeyEvent.KEYCODE_DPAD_LEFT
            && TextEditPanelModel.keyCode(Action.RIGHT) == KeyEvent.KEYCODE_DPAD_RIGHT
            && TextEditPanelModel.keyCode(Action.UP) == KeyEvent.KEYCODE_DPAD_UP
            && TextEditPanelModel.keyCode(Action.DOWN) == KeyEvent.KEYCODE_DPAD_DOWN, "arrow key codes");
        check(TextEditPanelModel.keyCode(Action.DOCUMENT_START) == KeyEvent.KEYCODE_MOVE_HOME
            && TextEditPanelModel.keyCode(Action.DOCUMENT_END) == KeyEvent.KEYCODE_MOVE_END, "home and end");
        check(TextEditPanelModel.keyCode(Action.DELETE) == KeyEvent.KEYCODE_DEL, "delete is backspace");
        check(TextEditPanelModel.keyCode(Action.COPY) == KeyEvent.KEYCODE_UNKNOWN
            && TextEditPanelModel.keyCode(Action.SELECT) == KeyEvent.KEYCODE_UNKNOWN, "menu actions send no key");

        int shift = KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON;
        int ctrl = KeyEvent.META_CTRL_ON | KeyEvent.META_CTRL_LEFT_ON;
        check(TextEditPanelModel.keyCode(lineStart) == KeyEvent.KEYCODE_MOVE_HOME
            && TextEditPanelModel.keyCode(lineEnd) == KeyEvent.KEYCODE_MOVE_END, "行边界使用 Home 和 End");
        check(TextEditPanelModel.metaState(lineStart, false) == 0
            && TextEditPanelModel.metaState(lineEnd, false) == 0, "行边界不能携带整篇文档的 Ctrl");
        check(TextEditPanelModel.metaState(lineStart, true) == shift
            && TextEditPanelModel.metaState(lineEnd, true) == shift, "选择模式延伸到当前行边界");
        check(TextEditPanelModel.metaState(Action.LEFT, false) == 0, "plain arrow moves the caret");
        check(TextEditPanelModel.metaState(Action.LEFT, true) == shift
            && TextEditPanelModel.metaState(Action.DOWN, true) == shift, "selecting arrows carry shift");
        check(TextEditPanelModel.metaState(Action.DOCUMENT_START, false) == ctrl
            && TextEditPanelModel.metaState(Action.DOCUMENT_END, false) == ctrl, "start and end use the whole document");
        check(TextEditPanelModel.metaState(Action.DOCUMENT_END, true) == (ctrl | shift),
            "selecting to the end extends the selection");
        check(TextEditPanelModel.metaState(Action.DELETE, true) == 0, "delete carries no modifier");

        check(TextEditPanelModel.contextMenuAction(Action.SELECT_ALL) == android.R.id.selectAll
            && TextEditPanelModel.contextMenuAction(Action.COPY) == android.R.id.copy
            && TextEditPanelModel.contextMenuAction(Action.CUT) == android.R.id.cut
            && TextEditPanelModel.contextMenuAction(Action.PASTE) == android.R.id.paste, "clipboard menu ids");
        check(TextEditPanelModel.contextMenuAction(Action.LEFT) == 0
            && TextEditPanelModel.contextMenuAction(Action.SELECT) == 0
            && TextEditPanelModel.contextMenuAction(Action.SELECT_LINE) == 0
            && TextEditPanelModel.contextMenuAction(Action.TRACKPAD) == 0, "other keys are not menu actions");
        check(TextEditPanelModel.keyCode(Action.TRACKPAD) == KeyEvent.KEYCODE_UNKNOWN
            && !TextEditPanelModel.endsSelecting(Action.TRACKPAD), "触控板开关不发按键");

        check(TextEditPanelModel.key(Action.NUMBER_PAD).label().equals("123")
            && TextEditPanelModel.key(Action.SPACE).label().equals("空格")
            && TextEditPanelModel.key(Action.RETURN).label().equals("换行"),
            "all editor shortcuts are visible");
        check(TextEditPanelModel.keyCode(Action.SPACE) == KeyEvent.KEYCODE_SPACE
            && TextEditPanelModel.keyCode(Action.RETURN) == KeyEvent.KEYCODE_ENTER
            && TextEditPanelModel.keyCode(Action.NUMBER_PAD) == KeyEvent.KEYCODE_UNKNOWN,
            "editor shortcut key codes");

        check(TextEditPanelModel.endsSelecting(Action.CUT) && TextEditPanelModel.endsSelecting(Action.PASTE)
            && TextEditPanelModel.endsSelecting(Action.DELETE), "cut, paste and delete end selecting");
        check(!TextEditPanelModel.endsSelecting(Action.COPY) && !TextEditPanelModel.endsSelecting(Action.LEFT)
            && !TextEditPanelModel.endsSelecting(Action.SELECT_ALL)
            && !TextEditPanelModel.endsSelecting(Action.SELECT_LINE), "copy and moves keep selecting");

        TextEditLineSelection.Range line = TextEditLineSelection.range("甲乙\n丙丁\n戊", 3, 3);
        check(line.start() == 3 && line.end() == 5, "caret selects the current line");
        line = TextEditLineSelection.range("甲乙\n丙丁\n戊", 2, 2);
        check(line.start() == 0 && line.end() == 2, "newline selects the preceding line");
        line = TextEditLineSelection.range("甲乙\n丙丁\n戊", 6, 6);
        check(line.start() == 6 && line.end() == 7, "last line is bounded");
        System.out.println("Android text edit panel: seamless direction block, seven reordered side actions, key events and clipboard actions passed");
    }

    private static final java.util.Set<Action> HIDDEN = EnumSet.of(Action.SELECT_LINE, Action.LINE_START, Action.LINE_END);

    private static Action[][] grid(List<Action> order) {
        EnumSet<Action> seen = EnumSet.noneOf(Action.class);
        Action[][] grid = new Action[TextEditPanelModel.ROWS][TextEditPanelModel.COLUMNS];
        for (Key key : TextEditPanelModel.keys(order)) {
            check(seen.add(key.action()), "one key per action: " + key.action());
            check(!key.label().isEmpty() && !key.description().isEmpty(), "labelled: " + key.action());
            for (int row = key.row(); row < key.row() + key.rowSpan(); row++) {
                for (int column = key.column(); column < key.column() + key.columnSpan(); column++) {
                    check(row < grid.length && column < grid[row].length, "inside grid");
                    check(grid[row][column] == null, "no overlapping keys: " + key.action()
                        + " at " + row + "," + column + " with " + grid[row][column]);
                    grid[row][column] = key.action();
                }
            }
        }
        EnumSet<Action> expected = EnumSet.allOf(Action.class);
        expected.removeAll(HIDDEN);
        check(seen.equals(expected), "除了选择行和行首尾，每个动作都有键");
        return grid;
    }

    private static void checkGrid(List<Action> order) {
        Action[][] grid = grid(order);
        List<Action> visible = TextEditPanelModel.visibleOrder(order);
        check(visible.size() == 7, "七个可见侧键");
        for (int index = 0; index < visible.size(); index++)
            check(grid[index < 4 ? index : index - 4][index < 4 ? 0 : 8] == visible.get(index),
                "side action follows saved order");
        for (int row = 0; row < grid.length; row++) for (int column = 0; column < grid[row].length; column++)
            check(grid[row][column] != null, "网格没有空格: " + row + "," + column);
    }

    /** 中央键正好铺满方向区外框、互不重叠；底排中央键与同排侧键上下齐平。 */
    private static void checkBounds(List<Action> order, int width, int height, int margin) {
        TextEditPanelModel.Bounds area = TextEditPanelModel.centralBounds(width, height, margin);
        long covered = 0;
        Key returnKey = null, trackpad = null;
        for (Key key : TextEditPanelModel.keys(order)) {
            TextEditPanelModel.Bounds bounds = TextEditPanelModel.bounds(key, width, height, margin);
            check(bounds.width() >= 0 && bounds.height() >= 0, "no negative size at " + width + "x" + height);
            if (key.action() == Action.RETURN) returnKey = key;
            if (key.action() == Action.TRACKPAD) trackpad = key;
            if (!TextEditPanelModel.isCentral(key)) continue;
            check(bounds.left() >= area.left() && bounds.right() <= area.right()
                && bounds.top() >= area.top() && bounds.bottom() <= area.bottom(), "central key inside the block: " + key.action());
            covered += (long) bounds.width() * bounds.height();
        }
        check(covered == (long) area.width() * area.height(), "中央键铺满方向区: " + width + "x" + height);
        TextEditPanelModel.Bounds side = TextEditPanelModel.bounds(returnKey, width, height, margin);
        TextEditPanelModel.Bounds middle = TextEditPanelModel.bounds(trackpad, width, height, margin);
        check(side.top() == middle.top() && side.bottom() == middle.bottom(), "底排与侧键齐平: " + width + "x" + height);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
