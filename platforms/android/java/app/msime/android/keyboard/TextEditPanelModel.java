package app.msime.android;

import android.view.KeyEvent;
import java.util.List;

/**
 * 文本编辑面板（#5625，从功能面板「文本编辑」打开）：方向键、选择、全选、复制、剪切、粘贴、跳到文档开头和结尾、删除。按 Gboard 的编辑面板排成 4 × 4：左边三列是方向键（←、→ 各占三行，中间一列 ↑ 选择 ↓）和底下一行的 开头 / 结尾 / 删除，右边一列是 全选 / 复制 / 剪切 / 粘贴。
 *
 * <p>只描述键、位置和每个键交给编辑器的是什么：方向和开头结尾是按键事件（「选择」开着时带 Shift，延伸选区；开头结尾带 Ctrl，到整篇文档的两端），全选、复制、剪切、粘贴是 InputConnection 的上下文菜单动作，删除是退格键事件（有选区时删选区）。无 Android 依赖（只用按键码和资源 id 常量），由 JVM 回归验证。
 */
public final class TextEditPanelModel {
    public static final int ROWS = 4;
    public static final int COLUMNS = 4;

    /** 键的动作，IME 按它接线。 */
    public enum Action {
        LEFT, UP, SELECT, DOWN, RIGHT, SELECT_ALL, COPY, CUT, PASTE, DOCUMENT_START, DOCUMENT_END, DELETE
    }

    /**
     * 一个键。
     *
     * @param label 键面文字
     * @param description 无障碍描述
     * @param row 起始行（从 0 起）
     * @param column 起始列（从 0 起）
     * @param rowSpan 占几行
     * @param repeats 按住是否连发（方向键和删除）
     */
    public record Key(Action action, String label, String description, int row, int column,
                      int rowSpan, boolean repeats) {}

    private static final List<Key> KEYS = List.of(
        new Key(Action.LEFT, "←", "光标左移", 0, 0, 3, true),
        new Key(Action.UP, "↑", "光标上移", 0, 1, 1, true),
        new Key(Action.SELECT, "选择", "选择模式", 1, 1, 1, false),
        new Key(Action.DOWN, "↓", "光标下移", 2, 1, 1, true),
        new Key(Action.RIGHT, "→", "光标右移", 0, 2, 3, true),
        new Key(Action.SELECT_ALL, "全选", "全选", 0, 3, 1, false),
        new Key(Action.COPY, "复制", "复制", 1, 3, 1, false),
        new Key(Action.CUT, "剪切", "剪切", 2, 3, 1, false),
        new Key(Action.PASTE, "粘贴", "粘贴", 3, 3, 1, false),
        new Key(Action.DOCUMENT_START, "⇤", "移到开头", 3, 0, 1, false),
        new Key(Action.DOCUMENT_END, "⇥", "移到结尾", 3, 1, 1, false),
        new Key(Action.DELETE, "⌫", "退格", 3, 2, 1, true));

    private TextEditPanelModel() { }

    /** 全部键，按行再按列排。 */
    public static List<Key> keys() { return KEYS; }

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
            case DOCUMENT_START -> KeyEvent.KEYCODE_MOVE_HOME;
            case DOCUMENT_END -> KeyEvent.KEYCODE_MOVE_END;
            case DELETE -> KeyEvent.KEYCODE_DEL;
            default -> KeyEvent.KEYCODE_UNKNOWN;
        };
    }

    /** 按键事件的修饰键：移动类的键在选择模式下带 Shift；开头结尾始终带 Ctrl（整篇文档而不是当前行）；删除不带。 */
    public static int metaState(Action action, boolean selecting) {
        int meta = 0;
        boolean moves = action == Action.LEFT || action == Action.RIGHT || action == Action.UP
            || action == Action.DOWN || action == Action.DOCUMENT_START || action == Action.DOCUMENT_END;
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
