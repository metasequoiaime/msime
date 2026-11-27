package app.msime.android;

/**
 * 键区里一个定高视图的高度角色：行构建时打在视图的 tag 上，高度由 {@link PanelGeometry#keyRowPx} 按当前的高度调整和行距算出。
 */
public final class KeyboardHeightRole {
    final int baseHeight;
    final int rowCount;
    final int rowIndex;
    final boolean includesRowSpacing;
    /** 要加上的行距份数：一行键加一份；一个占三行的九键块加三份。 */
    final int rowSpacings;
    /** 底行（功能行、123 / #+= 层自带的底行）：和一行键同高、带一份行距，分到四行均分后底行的那一份调整（{@link KeyboardHeightPolicy#bottomRowAdjustment}）。 */
    final boolean bottomRow;
    /** 整块里还包着底行（日语九键、注音九键没有单独的底行，四行都在块里）：`baseHeight` 已经是四行，块拿整份调整。其余的键行和键块分摊底行以外的那部分（{@link KeyboardHeightPolicy#keyRowsAdjustment}）。 */
    final boolean withBottomRow;
    /** 九宫格主体；上方补充数字行仍按原字母行分摊，避免切层改变总高。 */
    final boolean nineKeyGrid;

    public KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex,
                              boolean includesRowSpacing) {
        this(baseHeight, rowCount, rowIndex, includesRowSpacing ? 1 : 0);
    }

    public KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex, int rowSpacings) {
        this(baseHeight, rowCount, rowIndex, rowSpacings, false, false, false);
    }

    private KeyboardHeightRole(int baseHeight, int rowCount, int rowIndex, int rowSpacings,
                               boolean bottomRow, boolean withBottomRow, boolean nineKeyGrid) {
        this.baseHeight = baseHeight;
        this.rowCount = rowCount;
        this.rowIndex = rowIndex;
        this.includesRowSpacing = rowSpacings > 0;
        this.rowSpacings = rowSpacings;
        this.bottomRow = bottomRow;
        this.withBottomRow = withBottomRow;
        this.nineKeyGrid = nineKeyGrid;
    }

    /** 底行这一行。 */
    public static KeyboardHeightRole bottomRow() {
        return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP, 1, 0, 1, true, false, false);
    }

    /** 九宫格三行外框，单独标记以免上方的补充符号行跟着加高。 */
    public static KeyboardHeightRole nineKeyGrid() {
        return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3,
            1, 0, 3, false, false, true);
    }

    /** 三行键加底行的整块：四行键高、四份行距，拿整份高度调整，总高和三行键加一条底行的布局相同。 */
    public static KeyboardHeightRole threeRowsWithBottomRow() {
        return new KeyboardHeightRole(KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT,
            1, 0, KeyboardHeightPolicy.KEYBOARD_ROW_COUNT, false, true, false);
    }
}
