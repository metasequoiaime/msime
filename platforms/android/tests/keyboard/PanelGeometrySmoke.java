import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardHeightPolicy;
import app.msime.android.KeyboardHeightRole;
import app.msime.android.PanelGeometry;

public final class PanelGeometrySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    private static void equal(int actual, int expected, String message) {
        if (actual != expected) throw new AssertionError(message + ": expected " + expected + ", got " + actual);
    }

    private static PanelGeometry.TopBandInputs band(int readingRow, boolean readingShown, int line, int designLine,
                                                    int toolbar, boolean idleRowShown, boolean lineShown) {
        return new PanelGeometry.TopBandInputs(readingRow, readingShown, line, designLine, toolbar, idleRowShown,
            lineShown);
    }

    public static void main(String[] args) {
        topBand();
        keyRows();
        bottomInset();
    }

    /** 顶栏，按 1 倍密度写成 dp 数，便于和设计稿对照。 */
    private static void topBand() {
        // 26 键、拼音显示在输入框：不预留读音行，空闲时工具栏跟随候选行，组字时只有候选行。
        PanelGeometry.TopBandInputs idle = band(0, false, 32, 32, 0, true, false);
        PanelGeometry.TopBand b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 32, "toolbar follows the candidate line");
        equal(b.spacer(), 0, "no spacer without an explicit toolbar size");
        equal(b.height(idle), 32, "26-key idle band");
        PanelGeometry.TopBandInputs composing = band(0, false, 32, 32, 0, false, true);
        equal(PanelGeometry.topBand(composing).height(composing), 32, "26-key composing band");

        // 14/18 键：读音行常驻（14 dp），工具栏那一行跟随读音行加候选行，两种状态等高。
        idle = band(14, false, 32, 32, 0, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 46, "idle toolbar row covers the reading row too");
        equal(b.height(idle), 46, "14/18 idle band");
        equal(b.designRow(), 46, "design row includes the reading row when it is hidden");
        composing = band(14, true, 32, 32, 0, false, true);
        b = PanelGeometry.topBand(composing);
        equal(b.height(composing), 46, "14/18 composing band equals idle");
        equal(b.designRow(), 32, "design row is only the candidate line while the reading row shows");

        // 空闲时读音行显示常驻的模式标签：工具栏那一行让出读音行的高度，总高不变。
        idle = band(14, true, 32, 32, 0, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 32, "mode label takes the reading row's share");
        equal(b.height(idle), 46, "band unchanged by the mode label");

        // 释义或提示行加高候选行时，工具栏那一行跟着加高。
        idle = band(14, false, 46, 46, 0, true, false);
        equal(PanelGeometry.topBand(idle).height(idle), 60, "one gloss row grows the band");

        // 单独设置工具栏 100%（56 dp）高于组字的 46 dp：空闲时工具栏 56，组字时补 10 dp 空白。
        idle = band(14, false, 32, 32, 56, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 56, "explicit toolbar height");
        equal(b.spacer(), 0, "idle side is the taller one");
        equal(b.height(idle), 56, "idle band with explicit toolbar");
        composing = band(14, true, 32, 32, 56, false, true);
        b = PanelGeometry.topBand(composing);
        equal(b.spacer(), 10, "composing side is padded to the toolbar");
        equal(b.height(composing), 56, "stable band: composing equals idle");

        // 单独设置的工具栏矮于组字（50%，44 dp < 46 dp）：工具栏垫到组字的高度，不允许比候选栏矮。
        idle = band(14, false, 32, 32, 44, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 46, "toolbar never shorter than the composing band");
        equal(b.height(idle), 46, "short explicit toolbar");
        // 组字一侧较高、又开了释义（60 dp）：空闲时由补齐空白垫到 60。
        idle = band(14, false, 46, 46, 56, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.idleRow(), 60, "explicit toolbar grows to the composing band");
        equal(b.spacer(), 0, "toolbar row itself already took the height");
        equal(b.height(idle), 60, "idle band follows the taller composing band");

        // 顶栏整个不画（诊断信息占着、候选条模式）时只剩补齐空白。
        PanelGeometry.TopBandInputs none = band(0, false, 32, 32, 0, false, false);
        equal(PanelGeometry.topBand(none).height(none), 0, "nothing drawn");

        // 实际密度下按像素原样相加，不再取整。
        idle = band(37, false, 84, 84, 147, true, false);
        b = PanelGeometry.topBand(idle);
        equal(b.composing(), 121, "pixel composing band");
        equal(b.idle(), 147, "pixel idle band");
        composing = band(37, true, 84, 84, 147, false, true);
        equal(PanelGeometry.topBand(composing).height(composing), 147, "pixel composing band padded");
    }

    private static void keyRows() {
        int half = 4;
        int row = KeyboardGeometry.KEY_ROW_HEIGHT_DP;
        KeyboardHeightRole bottom = KeyboardHeightRole.bottomRow();
        KeyboardHeightRole grid = KeyboardHeightRole.nineKeyGrid();
        KeyboardHeightRole block = KeyboardHeightRole.threeRowsWithBottomRow();
        KeyboardHeightRole letters = new KeyboardHeightRole(row, 3, 0, true);

        equal(PanelGeometry.keyRowPx(bottom, 0, false, 1f, half), 60, "bottom row: 52 dp + one row gap");
        equal(PanelGeometry.keyRowPx(letters, 0, false, 1f, half), 60, "letter row: 52 dp + one row gap");
        equal(PanelGeometry.keyRowPx(grid, 0, true, 1f, half), 3 * row + 3 * 2 * half, "nine-key grid: three rows");
        equal(PanelGeometry.keyRowPx(block, 0, false, 1f, half), 4 * row + 4 * 2 * half, "block with bottom row");

        // +20 dp：26 键四行均分，底行 5、三行字母共 15。
        equal(PanelGeometry.keyRowPx(bottom, 20, false, 1f, 0), 57, "bottom row takes a quarter");
        equal(PanelGeometry.keyRowPx(letters, 20, false, 1f, 0), 57, "letter rows share the rest");
        // 九宫格加高只扩大上方键区，底行不变；缩小时仍均分。
        equal(PanelGeometry.keyRowPx(bottom, 20, true, 1f, 0), 52, "nine-key bottom row keeps its height");
        equal(PanelGeometry.keyRowPx(grid, 20, true, 1f, 0), 176, "nine-key grid takes the whole increase");
        equal(PanelGeometry.keyRowPx(bottom, -12, true, 1f, 0), 49, "nine-key shrink is shared");
        equal(PanelGeometry.keyRowPx(grid, -12, true, 1f, 0), 147, "nine-key grid shrinks by three quarters");
        // 九宫格标记只在画九宫格时生效：同一个底行在 26 键里照常分摊。
        equal(PanelGeometry.keyRowPx(grid, 20, false, 1f, 0), 171, "grid role outside nine-key shares like rows");

        // 不论哪种布局，键区总高（不含行距）都是四行默认键高加整份调整。
        for (int adjustment = KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP;
                adjustment <= KeyboardHeightPolicy.MAX_DESIGN_HEIGHT_ADJUSTMENT_DP; adjustment++) {
            int expected = 4 * row + adjustment;
            int twentySix = 0;
            for (int index = 0; index < 3; index++)
                twentySix += PanelGeometry.keyRowPx(new KeyboardHeightRole(row, 3, index, false), adjustment,
                    false, 1f, 0);
            twentySix += PanelGeometry.keyRowPx(bottom, adjustment, false, 1f, 0);
            equal(twentySix, expected, "26-key total at " + adjustment);
            int nineKey = PanelGeometry.keyRowPx(grid, adjustment, true, 1f, 0)
                + PanelGeometry.keyRowPx(bottom, adjustment, true, 1f, 0);
            equal(nineKey, expected, "nine-key total at " + adjustment);
            equal(PanelGeometry.keyRowPx(block, adjustment, false, 1f, 0), expected, "block total at " + adjustment);
        }

        // 像素：dp 先按 Android 的规则取整，再加行距像素。
        equal(PanelGeometry.keyRowPx(bottom, 0, false, 2.625f, 9), 137 + 18, "bottom row at 420 dpi");
        check(PanelGeometry.keyRowPx(letters, 0, false, 2.625f, 9) == PanelGeometry.keyRowPx(bottom, 0, false, 2.625f, 9),
            "letter and bottom rows match at 420 dpi");
    }

    private static void bottomInset() {
        PanelGeometry.BottomInset none = PanelGeometry.bottomInset(true, 48, true, 20, 40, 10);
        equal(none.padding(), 0, "floating keyboard leaves no bottom inset");
        PanelGeometry.BottomInset system = PanelGeometry.bottomInset(false, 48, false, 0, 40, 10);
        equal(system.padding(), 58, "system bar plus lift");
        PanelGeometry.BottomInset bar = PanelGeometry.bottomInset(false, 0, true, 20, 40, 0);
        equal(bar.padding(), 60, "bar and the gesture area below it replace the system inset");
        equal(bar.barMargin(), 20, "bar sits above the gesture area");
        equal(PanelGeometry.bottomInset(false, 0, true, 20, 40, 12).padding(), 72, "lift goes on top of the bar");
    }
}
