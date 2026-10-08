import android.view.KeyEvent;
import app.msime.android.TextEditPanelModel;
import app.msime.android.TextEditPanelModel.Action;
import app.msime.android.TextEditPanelModel.Key;
import java.util.EnumSet;

public final class TextEditPanelModelSmoke {
    public static void main(String[] args) {
        // 每个动作恰好一个键，键恰好铺满 4 × 4，不重叠。
        EnumSet<Action> seen = EnumSet.noneOf(Action.class);
        Action[][] grid = new Action[TextEditPanelModel.ROWS][TextEditPanelModel.COLUMNS];
        for (Key key : TextEditPanelModel.keys()) {
            check(seen.add(key.action()), "one key per action: " + key.action());
            check(!key.label().isEmpty() && !key.description().isEmpty(), "labelled: " + key.action());
            for (int row = key.row(); row < key.row() + key.rowSpan(); row++) {
                check(row < TextEditPanelModel.ROWS && key.column() < TextEditPanelModel.COLUMNS,
                    "inside the grid: " + key.action());
                check(grid[row][key.column()] == null, "no overlap at " + row + "," + key.column());
                grid[row][key.column()] = key.action();
            }
        }
        check(seen.equals(EnumSet.allOf(Action.class)), "every action has a key");
        for (Action[] row : grid) for (Action cell : row) check(cell != null, "the grid is fully covered");
        check(TextEditPanelModel.key(Action.LEFT).rowSpan() == 3 && TextEditPanelModel.key(Action.RIGHT).rowSpan() == 3,
            "left and right are tall keys");
        check(grid[3][0] == Action.DOCUMENT_START && grid[3][1] == Action.DOCUMENT_END,
            "start and end sit bottom-left as the issue asks");
        check(grid[0][3] == Action.SELECT_ALL && grid[1][3] == Action.COPY && grid[2][3] == Action.CUT
            && grid[3][3] == Action.PASTE, "clipboard column");

        // 方向键和删除连发，其余不连发。
        for (Key key : TextEditPanelModel.keys()) {
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

        check(TextEditPanelModel.endsSelecting(Action.CUT) && TextEditPanelModel.endsSelecting(Action.PASTE)
            && TextEditPanelModel.endsSelecting(Action.DELETE), "cut, paste and delete end selecting");
        check(!TextEditPanelModel.endsSelecting(Action.COPY) && !TextEditPanelModel.endsSelecting(Action.LEFT)
            && !TextEditPanelModel.endsSelecting(Action.SELECT_ALL), "copy and moves keep selecting");
        System.out.println("Android text edit panel: 4x4 layout, key events and clipboard actions passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
