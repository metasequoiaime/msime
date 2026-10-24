package app.msime.android;

import android.view.KeyEvent;
import java.util.List;

/** 文本编辑面板的固定十字、行与文档边界、七个可排序侧边键；只把动作适配为 Android 编辑器事件。 */
public final class TextEditPanelModel {
    public static final int ROWS = 4;
    /** 横向以半格计数，底部两个文档边界键各占一点五格。 */
    public static final int COLUMNS = 10;
    public static final String ORDER_PREFERENCE = "touch_text_edit_button_order";

    /** 键的动作，IME 按它接线。 */
    public enum Action {
        LEFT, UP, SELECT, DOWN, RIGHT, SELECT_ALL, COPY, CUT, PASTE, DOCUMENT_START, DOCUMENT_END, DELETE,
        NUMBER_PAD, SPACE, RETURN, LINE_START, LINE_END
    }

    /**
     * 一个键。
     *
     * @param label 键面文字
     * @param description 无障碍描述
     * @param row 起始行（从 0 起）
     * @param column 起始列（从 0 起）
     * @param rowSpan 占几行
     * @param columnSpan 占几个半格
     * @param repeats 按住是否连发（方向键和删除）
     */
    public record Key(Action action, String label, String description, int row, int column,
                      int rowSpan, int columnSpan, boolean repeats) {}

    private static final List<Key> KEYS = List.of(
        new Key(Action.LEFT, "←", "光标左移", 1, 2, 1, 2, true),
        new Key(Action.UP, "↑", "光标上移", 0, 4, 1, 2, true),
        new Key(Action.SELECT, "选择", "选择模式", 1, 4, 1, 2, false),
        new Key(Action.DOWN, "↓", "光标下移", 2, 4, 1, 2, true),
        new Key(Action.RIGHT, "→", "光标右移", 1, 6, 1, 2, true),
        new Key(Action.LINE_START, "行首", "移到行首", 0, 2, 1, 2, false),
        new Key(Action.LINE_END, "行尾", "移到行尾", 0, 6, 1, 2, false),
        new Key(Action.SELECT_ALL, "全选", "全选", 0, 0, 1, 2, false),
        new Key(Action.COPY, "复制", "复制", 0, 0, 1, 2, false),
        new Key(Action.CUT, "剪切", "剪切", 0, 0, 1, 2, false),
        new Key(Action.PASTE, "粘贴", "粘贴", 0, 0, 1, 2, false),
        new Key(Action.DOCUMENT_START, "⇤", "移到开头", 3, 2, 1, 3, false),
        new Key(Action.DOCUMENT_END, "⇥", "移到结尾", 3, 5, 1, 3, false),
        new Key(Action.DELETE, "⌫", "退格", 0, 0, 1, 2, true),
        new Key(Action.NUMBER_PAD, "123", "打开数字键盘", 0, 0, 1, 2, false),
        new Key(Action.SPACE, "空格", "插入空格", 0, 0, 1, 2, false),
        new Key(Action.RETURN, "换行", "插入换行，长按打开符号", 3, 8, 1, 2, false));

    private TextEditPanelModel() { }

    /** 根据共享偏好给出的完整顺序排键，列表按行列排序，下排两个空白角不生成键。 */
    public static List<Key> keys(List<Action> order) {
        java.util.ArrayList<Key> result = new java.util.ArrayList<>();
        for (Key key : KEYS) {
            int slot = order.indexOf(key.action());
            if (slot < 0) result.add(key);
            else result.add(new Key(key.action(), key.label(), key.description(),
                slot < 4 ? slot : slot - 4, slot < 4 ? 0 : 8, 1, 2, key.repeats()));
        }
        result.sort(java.util.Comparator.comparingInt(Key::row).thenComparingInt(Key::column));
        return List.copyOf(result);
    }

    public static Key key(Action action) {
        for (Key key : KEYS) if (key.action() == action) return key;
        throw new IllegalArgumentException("Unknown text edit action " + action);
    }

    /** 交给编辑器的按键码；不走按键事件的动作返回 {@link KeyEvent#KEYCODE_UNKNOWN}。 */
    public static int keyCode(Action action) {
        return switch (action) {
            case LEFT -> KeyEvent.KEYCODE_DPAD_LEFT;
            case RIGHT -> KeyEvent.KEYCODE_DPAD_RIGHT;
            case UP -> KeyEvent.KEYCODE_DPAD_UP;
            case DOWN -> KeyEvent.KEYCODE_DPAD_DOWN;
            case DOCUMENT_START, LINE_START -> KeyEvent.KEYCODE_MOVE_HOME;
            case DOCUMENT_END, LINE_END -> KeyEvent.KEYCODE_MOVE_END;
            case DELETE -> KeyEvent.KEYCODE_DEL;
            case SPACE -> KeyEvent.KEYCODE_SPACE;
            case RETURN -> KeyEvent.KEYCODE_ENTER;
            default -> KeyEvent.KEYCODE_UNKNOWN;
        };
    }

    /** 移动键在选择模式下带 Shift；只有文档首尾始终带 Ctrl，行边界与删除不带 Ctrl。 */
    public static int metaState(Action action, boolean selecting) {
        int meta = 0;
        boolean moves = action == Action.LEFT || action == Action.RIGHT || action == Action.UP
            || action == Action.DOWN || action == Action.DOCUMENT_START || action == Action.DOCUMENT_END
            || action == Action.LINE_START || action == Action.LINE_END;
        if (moves && selecting) meta |= KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON;
        if (action == Action.DOCUMENT_START || action == Action.DOCUMENT_END)
            meta |= KeyEvent.META_CTRL_ON | KeyEvent.META_CTRL_LEFT_ON;
        return meta;
    }

    /** 交给 InputConnection.performContextMenuAction 的资源 id；不是菜单动作时为 0。 */
    public static int contextMenuAction(Action action) {
        return switch (action) {
            case SELECT_ALL -> android.R.id.selectAll;
            case COPY -> android.R.id.copy;
            case CUT -> android.R.id.cut;
            case PASTE -> android.R.id.paste;
            default -> 0;
        };
    }

    /** 这个动作之后选择模式是否关掉：剪切、粘贴和删除之后已经没有要延伸的选区了。 */
    public static boolean endsSelecting(Action action) {
        return action == Action.CUT || action == Action.PASTE || action == Action.DELETE;
    }
}
