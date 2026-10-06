import app.msime.android.CandidateChevronButton;
import app.msime.android.FunctionPanelView;
import app.msime.android.InlineHeightBar;
import app.msime.android.KeyPressAnimator;
import app.msime.android.KeyboardIconKey;
import app.msime.android.KeyboardIconPaths;
import app.msime.android.KeyboardKeyPreview;
import app.msime.android.KeyboardPagerDots;
import app.msime.android.OneHandGutterView;
import app.msime.android.PagedTileGrid;
import app.msime.android.SpaceKeyFace;
import app.msime.android.VoiceListeningView;

public final class ms_w2_kb_ViewLogicSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static boolean near(float a, float b) { return Math.abs(a - b) < 1e-3f; }

    public static void main(String[] args) {
        // 功能面板：4×2 分页，P25 的 20 格分三页。
        int perPage = PagedTileGrid.perPage(4, 2);
        check(perPage == 8, "4x2 grid holds eight tiles");
        check(PagedTileGrid.pageCount(20, perPage) == 3, "twenty tiles span three pages");
        check(PagedTileGrid.pageCount(16, perPage) == 2, "sixteen tiles fill two pages");
        check(PagedTileGrid.pageCount(0, perPage) == 1, "empty grid still has one page");
        check(PagedTileGrid.pageOf(7, perPage) == 0 && PagedTileGrid.pageOf(8, perPage) == 1, "page of index");
        check(PagedTileGrid.settlePage(0, 0f, 300f, -5000f, 1000f, 3) == 1, "fling left goes to next page");
        check(PagedTileGrid.settlePage(1, 300f, 300f, 5000f, 1000f, 3) == 0, "fling right goes to previous page");
        check(PagedTileGrid.settlePage(2, 600f, 300f, -5000f, 1000f, 3) == 2, "fling past the last page clamps");
        check(PagedTileGrid.settlePage(0, 160f, 300f, 0f, 1000f, 3) == 1, "slow release rounds to nearest page");
        check(PagedTileGrid.settlePage(1, 100f, 300f, 0f, 1000f, 3) == 0, "slow release rounds back");
        check(PagedTileGrid.settlePage(1, 100f, 0f, 0f, 1000f, 3) == 1, "unmeasured grid keeps its page");

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

        // 内联高度条：75–130，拖动换算与文案。
        check("键盘布局调整".equals(InlineHeightBar.DESCRIPTION), "layout bar description");
        check(InlineHeightBar.clamp(60) == 75 && InlineHeightBar.clamp(150) == 130 && InlineHeightBar.clamp(110) == 110, "clamp to 75-130");
        check(InlineHeightBar.percentForDrag(100, -50f, 500f) == 110, "dragging up by 10% of the base grows 10%");
        check(InlineHeightBar.percentForDrag(100, 100f, 500f) == 80, "dragging down shrinks");
        check(InlineHeightBar.percentForDrag(100, -1000f, 500f) == 130, "drag clamps at the top");
        check(InlineHeightBar.percentForDrag(120, 30f, 0f) == 120, "unmeasured base keeps the start");
        check("上下拖动调整 · 105%".equals(InlineHeightBar.label(105)), "drag handle label");

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
        check(VoiceListeningView.ORB_DP == 72f, "listening orb is 72dp");
        check(VoiceListeningView.pulseSpread(0.5f, 18f) == 9f && VoiceListeningView.pulseSpread(2f, 18f) == 18f, "pulse spread");
        check(VoiceListeningView.pulseAlpha(0f) == 90 && VoiceListeningView.pulseAlpha(1f) == 0, "pulse fades out");
        check(SpaceKeyFace.LABEL_SP == 13f, "space label is 13sp");
        check(SpaceKeyFace.contentWidth(22f, 4f, 30f) == 56f && SpaceKeyFace.contentWidth(22f, 4f, 0f) == 22f, "space face width");
        check(CandidateChevronButton.WIDTH_DP == 41f && CandidateChevronButton.DIVIDER_HEIGHT_DP == 22f, "chevron is a 1x22 divider plus 40dp button");
        System.out.println("Android keyboard view logic: pager, panel, height bar, animations and geometry passed");
    }
}
