import android.view.KeyEvent;
import app.msime.android.TextEditPanelModel;
import app.msime.android.TextEditPanelModel.Action;
import app.msime.android.TextEditPanelModel.Key;
import java.util.EnumSet;
import java.util.List;

public final class TextEditPanelModelSmoke {
    public static void main(String[] args) {
        List<Action> order = List.of(Action.COPY, Action.PASTE, Action.SELECT_ALL, Action.CUT,
            Action.SPACE, Action.NUMBER_PAD, Action.DELETE);
        check(TextEditPanelModel.keys(order).size() == 17, "行首和行尾必须加入固定网格");
        Action lineStart = Action.valueOf("LINE_START"), lineEnd = Action.valueOf("LINE_END");
        checkGrid(order);
        java.util.ArrayList<Action> reversed = new java.util.ArrayList<>(order);
        java.util.Collections.reverse(reversed);
        checkGrid(reversed);
        Action[][] grid = grid(order);
        check(grid[0][2] == lineStart && grid[0][6] == lineEnd, "行边界位于上排两个空位");
        check(grid[0][4] == Action.UP && grid[1][2] == Action.LEFT
            && grid[1][4] == Action.SELECT && grid[1][6] == Action.RIGHT && grid[2][4] == Action.DOWN,
            "arrows form a cross around selection");
        check(grid[3][2] == Action.DOCUMENT_START && grid[3][5] == Action.DOCUMENT_END
            && grid[3][8] == Action.RETURN, "document edges and return keep fixed bottom positions");

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
            && TextEditPanelModel.contextMenuAction(Action.SELECT) == 0, "other keys are not menu actions");

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
            && !TextEditPanelModel.endsSelecting(Action.SELECT_ALL), "copy and moves keep selecting");
        System.out.println("Android text edit panel: fixed cross layout, reordered side actions, key events and clipboard actions passed");
    }

    private static Action[][] grid(List<Action> order) {
        EnumSet<Action> seen = EnumSet.noneOf(Action.class);
        Action[][] grid = new Action[TextEditPanelModel.ROWS][TextEditPanelModel.COLUMNS];
        for (Key key : TextEditPanelModel.keys(order)) {
            check(seen.add(key.action()), "one key per action: " + key.action());
            check(!key.label().isEmpty() && !key.description().isEmpty(), "labelled: " + key.action());
            for (int row = key.row(); row < key.row() + key.rowSpan(); row++) {
                for (int column = key.column(); column < key.column() + key.columnSpan(); column++) {
                    check(row < grid.length && column < grid[row].length, "inside grid");
                    check(grid[row][column] == null, "no overlapping keys");
                    grid[row][column] = key.action();
                }
            }
        }
        check(seen.equals(EnumSet.allOf(Action.class)), "every action has a visible key");
        return grid;
    }

    private static void checkGrid(List<Action> order) {
        Action[][] grid = grid(order);
        for (int index = 0; index < order.size(); index++)
            check(grid[index < 4 ? index : index - 4][index < 4 ? 0 : 8] == order.get(index),
                "side action follows saved order");
        for (int row = 0; row < grid.length; row++) for (int column = 0; column < grid[row].length; column++) {
            boolean blank = row == 2 && (column == 2 || column == 3 || column == 6 || column == 7);
            check((grid[row][column] == null) == blank, "下排两个角落仍留白");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
