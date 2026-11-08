package app.msime.android;

import android.view.KeyEvent;
import java.util.List;

/** 文本编辑面板的无缝方向区、七个可排序侧边键和点击边界；只把动作适配为 Android 编辑器事件。
 *
 * <p>四行网格，横向十个半格：左右两列各占两个半格，左列四格和右列前三格放侧键，右列第四格是换行。中央六个半格是一整块方向区：← 和 → 跨前三行，中间一列从上到下是 ↑、开始选择、↓；底排是 ⇤、触控板开关、⇥。网格没有空格。
 *
 * <p>选择行、行首、行尾不生成键，但动作身份和按键适配保留：选择行仍是共享偏好里的一项，已保存的排列和只认识旧布局的版本读得回来。 */
public final class TextEditPanelModel {
    public static final int ROWS = 4;
    /** 横向以半格计数，侧键占两个半格，中央方向区占六个。 */
    public static final int COLUMNS = 10;
    public static final String ORDER_PREFERENCE = "touch_text_edit_button_order";

    /** 键的动作，IME 按它接线。 */
    public enum Action {
        LEFT, UP, SELECT, DOWN, RIGHT, SELECT_LINE, SELECT_ALL, COPY, CUT, PASTE, DOCUMENT_START, DOCUMENT_END, DELETE,
        NUMBER_PAD, SPACE, RETURN, LINE_START, LINE_END, TRACKPAD
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
        new Key(Action.LEFT, "←", "光标左移", 0, 2, 3, 2, true),
        new Key(Action.UP, "↑", "光标上移", 0, 4, 1, 2, true),
        new Key(Action.SELECT, "开始选择", "选择模式", 1, 4, 1, 2, false),
        new Key(Action.DOWN, "↓", "光标下移", 2, 4, 1, 2, true),
        new Key(Action.RIGHT, "→", "光标右移", 0, 6, 3, 2, true),
        new Key(Action.SELECT_ALL, "全选", "全选", 0, 0, 1, 2, false),
        new Key(Action.COPY, "复制", "复制", 0, 0, 1, 2, false),
        new Key(Action.CUT, "剪切", "剪切", 0, 0, 1, 2, false),
        new Key(Action.PASTE, "粘贴", "粘贴", 0, 0, 1, 2, false),
        new Key(Action.DOCUMENT_START, "⇤", "移到行首，双击移到开头", 3, 2, 1, 2, false),
        new Key(Action.TRACKPAD, "触控板", "触控板", 3, 4, 1, 2, false),
        new Key(Action.DOCUMENT_END, "⇥", "移到行尾，双击移到结尾", 3, 6, 1, 2, false),
        new Key(Action.DELETE, "⌫", "退格", 0, 0, 1, 2, true),
        new Key(Action.NUMBER_PAD, "123", "打开数字键盘", 0, 0, 1, 2, false),
        new Key(Action.SPACE, "空格", "插入空格", 0, 0, 1, 2, false),
        new Key(Action.RETURN, "换行", "插入换行", 3, 8, 1, 2, false));

    private TextEditPanelModel() { }

    /** 根据共享偏好给出的完整顺序排键：可排序的动作依次放进左列四格、右列前三格，不显示的动作不占位置；列表按行列排序。 */
    public static List<Key> keys(List<Action> order) {
        List<Action> visible = visibleOrder(order);
        java.util.ArrayList<Key> result = new java.util.ArrayList<>();
        for (Key key : KEYS) {
            int slot = visible.indexOf(key.action());
            if (slot < 0) result.add(key);
            else {
                int row = slot < 4 ? slot : slot - 4;
                int column = slot < 4 ? 0 : 8;
                result.add(new Key(key.action(), key.label(), key.description(),
                    row, column, 1, 2, key.repeats()));
            }
        }
        result.sort(java.util.Comparator.comparingInt(Key::row).thenComparingInt(Key::column));
        return List.copyOf(result);
    }

    /** 面板上显示、排序页里能拖动的侧边动作。选择行留在共享排列里，但不显示也不参与排序。 */
    public static boolean isSortable(Action action) {
        return action == Action.COPY || action == Action.CUT || action == Action.PASTE
            || action == Action.SELECT_ALL || action == Action.DELETE
            || action == Action.NUMBER_PAD || action == Action.SPACE;
    }

    /** 完整排列里可排序的那几项，保持原来的先后。 */
    public static List<Action> visibleOrder(List<Action> order) {
        java.util.ArrayList<Action> result = new java.util.ArrayList<>();
        for (Action action : order) if (isSortable(action)) result.add(action);
        return List.copyOf(result);
    }

    /** 把排序页调好的可见顺序填回完整排列：不显示的动作留在原位，只替换可排序动作所在的位置。 */
    public static List<Action> mergeVisibleOrder(List<Action> original, List<Action> visible) {
        java.util.HashSet<Action> expected = new java.util.HashSet<>(visibleOrder(original));
        if (visible.size() != expected.size() || !expected.equals(new java.util.HashSet<>(visible)))
            throw new IllegalArgumentException("Incomplete text edit order");
        java.util.ArrayList<Action> result = new java.util.ArrayList<>();
        int index = 0;
        for (Action action : original)
            result.add(isSortable(action) ? visible.get(index++) : action);
        return List.copyOf(result);
    }

    /**
     * 全选键上滑时的选择行；返回 {@code null} 表示这个键没有上滑动作。⇤ ⇥ 的行首、行尾改成单击（双击到文首文尾，见 {@link TextEditEdgeTapPolicy}），不再有上滑（#11111）。
     */
    public static Action upwardSwipeAction(Action action) {
        return action == Action.SELECT_ALL ? Action.SELECT_LINE : null;
    }

    /** 上滑动作的触摸提示，键面和无障碍描述仍保持原来的动作。 */
    public static String upwardSwipeDescription(Action action) {
        return action == Action.SELECT_ALL ? "上滑选择当前行" : "";
    }

    /** 中央方向区里的键：由网格画公共底面和分隔线，键自己只画按下和选中。 */
    public static boolean isCentral(Key key) { return key.column() > 0 && key.column() < 8; }

    public static Key key(Action action) {
        for (Key key : KEYS) if (key.action() == action) return key;
        throw new IllegalArgumentException("Unknown text edit action " + action);
    }

    /** 视图测量和布局直接使用的像素边界。 */
    public record Bounds(int left, int top, int right, int bottom) {
        public int width() { return Math.max(0, right - left); }
        public int height() { return Math.max(0, bottom - top); }
    }

    /** 侧键四周留 {@code margin} 的键距；中央键贴着方向区外框和分隔线，点击范围延伸到线上。 */
    public static Bounds bounds(Key key, int width, int height, int margin) {
        if (isCentral(key)) {
            Bounds panel = centralBounds(width, height, margin);
            int left = key.column() == 2 ? panel.left() : width * key.column() / COLUMNS;
            int end = key.column() + key.columnSpan();
            int right = end == 8 ? panel.right() : width * end / COLUMNS;
            return new Bounds(left, seam(key.row(), height, margin), right,
                seam(key.row() + key.rowSpan(), height, margin));
        }
        int left = width * key.column() / COLUMNS;
        int right = width * (key.column() + key.columnSpan()) / COLUMNS;
        int top = height * key.row() / ROWS;
        int bottom = height * (key.row() + key.rowSpan()) / ROWS;
        int xInset = Math.min(margin, Math.max(0, (right - left - 1) / 2));
        int yInset = Math.min(margin, Math.max(0, (bottom - top - 1) / 2));
        return new Bounds(left + xInset, top + yInset, right - xInset, bottom - yInset);
    }

    /** 中央方向区外框：上下与侧键首尾齐平，左右各让出一个键距。 */
    public static Bounds centralBounds(int width, int height, int margin) {
        int inset = inset(height, margin);
        return new Bounds(width * 2 / COLUMNS + inset, inset,
            width * 8 / COLUMNS - inset, height - inset);
    }

    /** 方向区里第 {@code row} 行的上沿：每条横缝对齐下一排侧键的上边，所以底排和侧键等高，上面三行只差整数像素舍入。 */
    public static int seam(int row, int height, int margin) {
        int inset = inset(height, margin);
        if (row == ROWS) return height - inset;
        return height * row / ROWS + inset;
    }

    /** 触控板模式的选择开关：贴触控板右上角，四周留 {@code gap}；触控板太小时按可用空间缩小，不越出触控板。 */
    public static Bounds trackpadSelectBounds(int left, int top, int right, int bottom,
                                              int preferredWidth, int preferredHeight, int gap) {
        int width = Math.max(0, Math.min(preferredWidth, right - left - 2 * gap));
        int height = Math.max(0, Math.min(preferredHeight, bottom - top - 2 * gap));
        int chipRight = Math.max(left, right - gap);
        int chipTop = Math.min(bottom, top + gap);
        return new Bounds(chipRight - width, chipTop, chipRight, chipTop + height);
    }

    private static int inset(int height, int margin) {
        return Math.min(Math.max(0, margin), Math.max(0, (height / ROWS - 1) / 2));
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
