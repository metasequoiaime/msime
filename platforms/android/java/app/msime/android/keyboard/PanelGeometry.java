package app.msime.android;

/**
 * 键盘面板的高度：顶栏（读音行、候选行或工具栏那一行，以及两种状态之间补齐的空白）、键区各排和底部留白，全部由输入值算出，不读视图、不读输入法服务。
 *
 * <p>面板 = 浮动拖动条 + 顶栏 + 键区 + 底部留白。量字形、读设置和系统栏这些事由调用方做完，把结果以像素交进来；这里只做整数运算，所以同一组输入总得到同一组高度，JVM 冒烟可以逐个布局钉住。
 */
public final class PanelGeometry {
    /**
     * 停靠在 API 29+ 时，输入法窗口在面板上方多留的透明高度（dp）。顶排键的气泡连上下阴影边距是 78 dp，浮在面板上沿的读音条在最大字号下约 33 dp，取整到 80。这段不算进应用的内容区，也不在可触摸区域里。
     */
    public static final int HEADROOM_DP = 80;

    private PanelGeometry() { }

    /**
     * 面板上方的透明高度（像素）。只有停靠时按内容包裹高度的窗口（API 29+）才留；API 28 的停靠窗口、浮动键盘、跟随光标的候选栏、编辑预览和「键盘调整」时窗口本来就铺满，不再多留。
     *
     * @param wrapContentWindow 输入法窗口此刻按内容包裹高度
     * @param density 屏幕密度
     */
    public static int headroomPx(boolean wrapContentWindow, float density) {
        return wrapContentWindow ? DimensionPolicy.pixels(HEADROOM_DP, density) : 0;
    }

    /**
     * 顶栏的输入，全部是像素。
     *
     * @param readingRow 读音行这一刻要预留的高度；不预留时为 0
     * @param readingRowShown 读音行此刻画着（组字时，或空闲时显示常驻的模式标签）
     * @param candidateLine 候选行的高度，含预留的释义行和提示行
     * @param designLine 同样行数下「候选栏大小」为 100% 时的候选行高度
     * @param toolbar 单独设置的工具栏高度；没设置（跟随候选栏）时为 0
     * @param idleRowShown 工具栏那一行此刻画着（工具栏、最近复制或自动填充）
     * @param candidateLineShown 候选行此刻画着
     */
    public record TopBandInputs(int readingRow, boolean readingRowShown, int candidateLine, int designLine,
                                int toolbar, boolean idleRowShown, boolean candidateLineShown) { }

    /**
     * 顶栏各部分的高度，全部是像素。
     *
     * @param readingRow 读音行
     * @param candidateLine 候选行
     * @param idleRow 工具栏那一行（工具栏、最近复制、自动填充共用这一高度）
     * @param spacer 补齐空白：单独设置了工具栏高度时，空闲和组字两种状态中较矮的一侧补到较高的一侧
     * @param designRow 「候选栏大小」为 100% 时工具栏那一行的高度，按钮和图标按它缩放
     * @param composing 组字时顶栏的高度（读音行 + 候选行）
     * @param idle 空闲时顶栏的高度（含读音行显示时的那一截）
     */
    public record TopBand(int readingRow, int candidateLine, int idleRow, int spacer, int designRow,
                          int composing, int idle) {
        /** 此刻顶栏实际占的高度：画着的各行加补齐空白。 */
        public int height(TopBandInputs in) {
            return (in.readingRowShown() ? readingRow : 0)
                + (in.idleRowShown() ? idleRow : 0)
                + (in.candidateLineShown() ? candidateLine : 0)
                + spacer;
        }
    }

    /**
     * 顶栏：组字时是读音行加候选行；没单独设置工具栏高度时，工具栏那一行跟随组字时的高度，两种状态等高。单独设置时按设置的高度，两种状态中较矮的一侧由补齐空白垫到较高的一侧，所以空闲和组字之间面板不跳（{@link ToolbarSizePolicy#stableHeight}）。读音行在空闲时也显示（常驻的模式标签）时，工具栏那一行让出读音行的高度。
     */
    public static TopBand topBand(TopBandInputs in) {
        int readingRow = Math.max(0, in.readingRow());
        int line = Math.max(0, in.candidateLine());
        boolean custom = in.toolbar() > 0;
        int composing = readingRow + line;
        int idle = custom ? ToolbarSizePolicy.stableHeight(in.toolbar(), composing) : composing;
        int idleRow = idle - (in.readingRowShown() ? readingRow : 0);
        int spacer = custom && in.idleRowShown() ? ToolbarSizePolicy.idleSpacerPx(idle, composing)
            : custom && in.candidateLineShown() ? ToolbarSizePolicy.composingSpacerPx(idle, composing) : 0;
        int designRow = in.readingRowShown() ? in.designLine() : readingRow + in.designLine();
        return new TopBand(readingRow, line, idleRow, spacer, designRow, composing, idle);
    }

    /**
     * 键区里一个定高视图（一行键、一块键或底行）的像素高度。九宫格加高时只扩大上方键区，底行保持默认高度；其余布局和缩小量四行均分。键区总高始终是四行默认键高加整份调整，再加各行的行距。
     *
     * @param role 视图的高度角色
     * @param adjustmentDp 当前窗口里实际要画的高度调整（{@link KeyboardHeightPolicy#windowHeightAdjustment}），整棵树用同一个值
     * @param nineKeyGridDrawn 这一刻画的是九宫格
     * @param density 屏幕密度
     * @param halfSpacingPx 半个行距的像素（{@link KeyboardSpacingPolicy#halfGapPixels}）
     */
    public static int keyRowPx(KeyboardHeightRole role, int adjustmentDp, boolean nineKeyGridDrawn,
                               float density, int halfSpacingPx) {
        boolean nineKeyGrid = nineKeyGridDrawn && (role.bottomRow || role.nineKeyGrid);
        int heightDp;
        if (role.bottomRow) {
            heightDp = role.baseHeight + KeyboardHeightPolicy.bottomRowAdjustment(adjustmentDp, nineKeyGrid);
        } else {
            int share = role.withBottomRow ? adjustmentDp
                : KeyboardHeightPolicy.keyRowsAdjustment(adjustmentDp, nineKeyGrid);
            heightDp = KeyboardHeightPolicy.adjustedRowHeight(role.baseHeight, share, role.rowCount, role.rowIndex);
        }
        return DimensionPolicy.pixels(heightDp, density) + halfSpacingPx * 2 * role.rowSpacings;
    }

    /**
     * 键盘列的底部留白和键盘底栏的位置，全部是像素。
     *
     * @param padding 键盘列的底部内边距：系统栏，底栏画着时换成底栏和它下面让出的手势区，再加用户选的抬高
     * @param barMargin 底栏的底边距；底栏不画时没有意义
     */
    public record BottomInset(int padding, int barMargin) {
        public static final BottomInset NONE = new BottomInset(0, 0);
    }

    /**
     * 底部留白。浮动（以及候选栏跟随光标）时四边都不留；停靠时留出系统栏，底栏画着时底部换成底栏加它下面让出的手势区，最后加上抬高。
     *
     * @param overlay 浮动键盘或跟随光标的候选栏画着
     * @param systemBottom 键盘视图收到的底部系统栏 inset
     * @param barShown 键盘底栏画着
     * @param barMargin 底栏的底边距（{@link KeyboardBottomBarPolicy#barBottomPx}）
     * @param bar 底栏的高度
     * @param lift 用户选的底部抬高（「键盘调整」期间是预览的值）
     */
    public static BottomInset bottomInset(boolean overlay, int systemBottom, boolean barShown, int barMargin,
                                          int bar, int lift) {
        if (overlay) return BottomInset.NONE;
        int bottom = barShown ? barMargin + bar : systemBottom;
        return new BottomInset(bottom + lift, barMargin);
    }
}
