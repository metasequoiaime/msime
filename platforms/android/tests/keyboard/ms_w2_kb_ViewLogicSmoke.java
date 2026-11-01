import app.msime.android.CandidateChevronButton;
import app.msime.android.FunctionPanelView;
import app.msime.android.KeyboardAdjustOverlay;
import app.msime.android.KeyboardAdjustPolicy;
import app.msime.android.KeyPressAnimator;
import app.msime.android.KeyboardIconKey;
import app.msime.android.KeyboardIconPaths;
import app.msime.android.KeyboardKeyPreview;
import app.msime.android.KeyboardHeightPolicy;
import app.msime.android.KeyboardPagerDots;
import app.msime.android.KeyboardSchemeCard;
import app.msime.android.KeyboardSpacingPolicy;
import app.msime.android.OneHandGutterView;
import app.msime.android.PagedTileGrid;
import app.msime.android.SpaceKeyFace;
import app.msime.android.VoiceListeningView;

public final class ms_w2_kb_ViewLogicSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static boolean near(float a, float b) { return Math.abs(a - b) < 1e-3f; }

    private static void checkPicker(PagedTileGrid.PickerGrid grid, int rows, float scale, String message) {
        check(grid.rows() == rows && near(grid.scale(), scale), message + ": " + grid);
    }

    public static void main(String[] args) {
        // 皮肤等面板仍使用四列两行网格，20 格分三页。
        int perPage = PagedTileGrid.perPage(4, 2);
        check(perPage == 8, "4x2 grid holds eight tiles");
        check(PagedTileGrid.pageCount(20, perPage) == 3, "twenty tiles span three pages");
        check(PagedTileGrid.pageCount(16, perPage) == 2, "sixteen tiles fill two pages");
        check(PagedTileGrid.pageCount(0, perPage) == 1, "empty grid still has one page");
        check(PagedTileGrid.pageCount(1, 0) == 1, "zero page capacity is clamped");
        check(PagedTileGrid.pageCount(Integer.MAX_VALUE, 2) == 1_073_741_824,
            "page count does not overflow its numerator");
        check(PagedTileGrid.perPage(Integer.MAX_VALUE, 2) == Integer.MAX_VALUE,
            "page capacity does not wrap on multiplication");
        check(PagedTileGrid.pageOf(7, perPage) == 0 && PagedTileGrid.pageOf(8, perPage) == 1, "page of index");
        check(PagedTileGrid.settlePage(0, 0f, 300f, -5000f, 1000f, 3) == 1, "fling left goes to next page");
        check(PagedTileGrid.settlePage(1, 300f, 300f, 5000f, 1000f, 3) == 0, "fling right goes to previous page");
        check(PagedTileGrid.settlePage(2, 600f, 300f, -5000f, 1000f, 3) == 2, "fling past the last page clamps");
        check(PagedTileGrid.settlePage(0, 160f, 300f, 0f, 1000f, 3) == 1, "slow release rounds to nearest page");
        check(PagedTileGrid.settlePage(1, 100f, 300f, 0f, 1000f, 3) == 0, "slow release rounds back");
        check(PagedTileGrid.settlePage(1, 100f, 0f, 0f, 1000f, 3) == 1, "unmeasured grid keeps its page");

        // 输入方式和功能面板按网格实际能占的高度排行，与 iOS 的 pickerLayout、鸿蒙的 pickerGrid 同一条规则、同一组用例：放得下原尺寸（三行 3 × 52 + 2 × 12 = 180 dp）就是原尺寸，放不下按比例缩小，三行的磁贴矮于 44 dp 才改成两行。
        float row = FunctionPanelView.ITEM_HEIGHT_DP;
        checkPicker(PagedTileGrid.pickerGrid(false, 200f, row), 3, 1f, "a tall panel keeps three full-size rows");
        checkPicker(PagedTileGrid.pickerGrid(false, 180f, row), 3, 1f, "exactly full size");
        checkPicker(PagedTileGrid.pickerGrid(false, 162f, row), 3, .9f, "a shorter panel scales three rows down");
        checkPicker(PagedTileGrid.pickerGrid(false, 152.4f, row), 3, 152.4f / 180f, "three rows down to the 44 dp tile");
        checkPicker(PagedTileGrid.pickerGrid(false, 152f, row), 2, 1f, "below the 44 dp tile two full-size rows");
        checkPicker(PagedTileGrid.pickerGrid(false, 100f, row), 2, 44f / 52f, "two rows never shrink below 44 dp");
        checkPicker(PagedTileGrid.pickerGrid(true, 200f, row), 2, 1f, "phone landscape stays two rows");
        checkPicker(PagedTileGrid.pickerGrid(true, 110f, row), 2, 110f / 120f, "phone landscape scales two rows");
        check(near(PagedTileGrid.pickerGrid(false, 162f, row).rowHeight(row), 46.8f)
            && near(PagedTileGrid.pickerGrid(false, 162f, row).rowGap(), 10.8f), "row height and gap scale together");
        check(near(PagedTileGrid.pickerGrid(true, 200f, row).rowGap(), 16f), "two rows keep the 16 dp gap");
        check(near(PagedTileGrid.pickerMinimumHeightDp(row), 2 * 44f + 16f * 44f / 52f), "smallest arrangement height");
        // 在每一个高度上：排出来的网格不比给它的高度高（只在比最矮的排法还矮时例外），磁贴不矮于 44 dp，高度变矮时行数不会变多。
        int previousRows = 3;
        for (float grid = 260f; grid >= 60f; grid -= .25f) {
            for (float full : new float[] {row, KeyboardSchemeCard.HEIGHT_DP}) {
                PagedTileGrid.PickerGrid picked = PagedTileGrid.pickerGrid(false, grid, full);
                float used = picked.rows() * picked.rowHeight(full) + (picked.rows() - 1) * picked.rowGap();
                check(used <= grid + 1e-3f || grid < PagedTileGrid.pickerMinimumHeightDp(full),
                    "the grid fits " + grid + " dp");
                check(picked.rowHeight(full) >= PagedTileGrid.PICKER_MIN_TILE_DP - 1e-3f, "tiles stay at least 44 dp");
                if (full == row) {
                    check(picked.rows() <= previousRows, "a shorter panel never gains a row at " + grid + " dp");
                    previousRows = picked.rows();
                }
            }
        }
        // 用预测的键区高度看各档大致的结果（真实面板因取整再矮两三个像素，网格按量到的高度算，所以照样放得下）：功能面板网格以外 = 自身内边距 10 + 4、页点 8 + 6，外面容器底部 10。
        check(FunctionPanelView.CHROME_DP == 28f, "function panel chrome");
        float chrome = FunctionPanelView.CHROME_DP + 10f;
        float defaultKeyArea = KeyboardHeightPolicy.keyAreaHeightDp(KeyboardHeightPolicy.DEFAULT_HEIGHT_ADJUSTMENT_DP,
            800, KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS);
        float minimumKeyArea = KeyboardHeightPolicy.keyAreaHeightDp(KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP,
            800, KeyboardSpacingPolicy.MIN_ROW_SPACING_TENTHS);
        check(near(minimumKeyArea, 178f) && near(defaultKeyArea, 236f), "key area spans 178–236 dp from 75% / 4 dp to default");
        check(near(KeyboardHeightPolicy.keyAreaHeightDp(0, 360, -1), defaultKeyArea), "landscape key area matches portrait");
        checkPicker(PagedTileGrid.pickerGrid(false, defaultKeyArea - chrome, row), 3, 1f, "100% is full size");
        float eighty = KeyboardHeightPolicy.keyAreaHeightDp(KeyboardHeightPolicy.heightPercentToAdjustment(80), 800,
            KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS);
        checkPicker(PagedTileGrid.pickerGrid(false, eighty - chrome, row), 3, (eighty - chrome) / 180f, "80% scales three rows");
        float seventyFive = KeyboardHeightPolicy.keyAreaHeightDp(KeyboardHeightPolicy.heightPercentToAdjustment(75), 800,
            KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS);
        checkPicker(PagedTileGrid.pickerGrid(false, seventyFive - chrome, row), 2, 1f, "75% falls back to two rows");
        checkPicker(PagedTileGrid.pickerGrid(false, minimumKeyArea - chrome, row), 2, 1f, "the minimum height fits two rows");
        // 输入方式面板：卡片 56 dp，网格以外 = 内边距 14 + 6、页点格 10 和上下各 6（ImePanels）。
        float schemeChrome = 42f;
        checkPicker(PagedTileGrid.pickerGrid(false, defaultKeyArea - schemeChrome, KeyboardSchemeCard.HEIGHT_DP), 3, 1f,
            "the scheme picker is full size at 100%");
        checkPicker(PagedTileGrid.pickerGrid(false, minimumKeyArea - schemeChrome, KeyboardSchemeCard.HEIGHT_DP), 2, 1f,
            "the scheme picker fits two rows at the minimum height");
        check(PagedTileGrid.pageCount(23, PagedTileGrid.perPage(4, 3)) == 2
            && PagedTileGrid.pageCount(23, PagedTileGrid.perPage(4, 2)) == 3, "function tools paginate by rows");

        // 页点：活动点宽 16 dp。
        check(KeyboardPagerDots.ACTIVE_DP == 16f, "active dot is 16dp wide");
        check(near(KeyboardPagerDots.totalWidthDp(3), 16f + 2 * 6f + 2 * 6f), "three dots width");
        check(KeyboardPagerDots.totalWidthDp(0) == 0f, "no dots no width");

        // 面板条目状态与 §2.8 文案，条目高 52 dp。
        check("已开启".equals(FunctionPanelView.stateText(FunctionPanelView.State.ON)), "on state text");
        check("已关闭".equals(FunctionPanelView.stateText(FunctionPanelView.State.OFF)), "off state text");
        check("不可用".equals(FunctionPanelView.stateText(FunctionPanelView.State.UNAVAILABLE)), "unavailable state text");
        check(FunctionPanelView.stateText(FunctionPanelView.State.NONE) == null, "plain entries have no state");
        check(FunctionPanelView.ITEM_HEIGHT_DP == 52f, "panel tiles are 52dp");

        // 「键盘调整」遮罩：约 45% 不透明的黑色，三个按钮和每个柄都有读屏说明；拖动换算在 KeyboardAdjustPolicySmoke。
        check("键盘调整".equals(KeyboardAdjustOverlay.DESCRIPTION), "adjust overlay description");
        check((KeyboardAdjustOverlay.DIM_COLOR >>> 24) == 115
            && (KeyboardAdjustOverlay.DIM_COLOR & 0xFFFFFF) == 0, "overlay is black at about 45%");
        check("重置".equals(KeyboardAdjustOverlay.RESET_LABEL) && "移动".equals(KeyboardAdjustOverlay.MOVE_LABEL)
            && "确认".equals(KeyboardAdjustOverlay.CONFIRM_LABEL), "the three buttons read 重置 / 移动 / 确认");
        java.util.Set<String> handleDescriptions = new java.util.HashSet<>();
        for (KeyboardAdjustPolicy.Part part : KeyboardAdjustPolicy.Part.values())
            handleDescriptions.add(KeyboardAdjustOverlay.description(part));
        check(handleDescriptions.size() == KeyboardAdjustPolicy.Part.values().length,
            "every handle has its own accessibility description");
        check("移动".equals(KeyboardAdjustOverlay.description(KeyboardAdjustPolicy.Part.MOVE)),
            "the move button is described by its label");

        // 按压动画：偏好值往返，未知值回落 none。
        for (KeyPressAnimator.Style style : KeyPressAnimator.Style.values()) {
            check(KeyPressAnimator.Style.fromPreference(style.preference()) == style, "round trip " + style);
        }
        check(KeyPressAnimator.Style.fromPreference("sparkle") == KeyPressAnimator.Style.NONE, "unknown animation is none");
        check(KeyPressAnimator.Style.fromPreference(null) == KeyPressAnimator.Style.NONE, "missing animation is none");
        float[] bounce = KeyPressAnimator.bounceScales();
        check(bounce[0] == 1f && bounce[bounce.length - 1] == 1f, "bounce returns to rest");

        // 按键图标、气泡、单手侧栏、聆听脉冲、空格键面、chevron 的几何。
        check(KeyboardIconKey.iconFor(KeyboardIconKey.Kind.SHIFT) == KeyboardIconPaths.Icon.SHIFT, "shift icon");
        check(KeyboardIconKey.iconFor(KeyboardIconKey.Kind.CAPS_LOCK) == KeyboardIconPaths.Icon.CAPS_LOCK, "caps lock icon");
        check(KeyboardIconKey.iconFor(KeyboardIconKey.Kind.BACKSPACE) == KeyboardIconPaths.Icon.BACKSPACE, "backspace icon");
        check(KeyboardIconKey.iconFor(KeyboardIconKey.Kind.RETURN) == KeyboardIconPaths.Icon.RETURN, "return icon");
        check(KeyboardIconKey.ICON_DP == 22f, "key icons are 22dp");
        check(KeyboardKeyPreview.HEIGHT_DP == 54f, "preview bubble is 54dp tall");
        check(near(KeyboardKeyPreview.bubbleWidth(100f, 1f), 138f), "letter bubble width");
        check(near(KeyboardKeyPreview.bubbleWidth(100f, 1.9f), 150f), "wide key bubble width");
        check(near(KeyboardKeyPreview.bubbleLeft(0f, 40f, 60f, 400f), 0f), "bubble clamps at the left edge");
        check(near(KeyboardKeyPreview.bubbleLeft(380f, 20f, 60f, 400f), 340f), "bubble clamps at the right edge");
        check(near(KeyboardKeyPreview.bubbleLeft(100f, 40f, 60f, 400f), 90f), "bubble centres on the key");
        check(near(KeyboardKeyPreview.bubbleTop(200f, 54f, 6f), 152f), "bubble overlaps the key top");
        check(OneHandGutterView.gutterWidth(1000) == 150 && OneHandGutterView.keysWidth(1000) == 850, "one-hand gutter takes 15%");
        check(VoiceListeningView.ORB_DP == 64f, "listening orb is at most 64dp");
        check(near(VoiceListeningView.MIC_RATIO * 72f, 32f), "the mic is 32dp on a 72dp orb");
        check(VoiceListeningView.TITLE_SP == 16f && VoiceListeningView.HINT_SP == 13f, "listening title and hint sizes");
        check(SpaceKeyFace.LABEL_SP == 13f, "space label is 13sp");
        check(SpaceKeyFace.contentWidth(22f, 4f, 30f) == 56f && SpaceKeyFace.contentWidth(22f, 4f, 0f) == 22f, "space face width");
        check(CandidateChevronButton.WIDTH_DP == 41f && CandidateChevronButton.DIVIDER_HEIGHT_DP == 22f, "chevron is a 1x22 divider plus 40dp button");
        System.out.println("Android keyboard view logic: pager, panel, height bar, animations and geometry passed");
    }
}
