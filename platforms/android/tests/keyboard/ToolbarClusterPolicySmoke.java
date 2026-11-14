package app.msime.android;

/** 空闲工具栏聚拢排法的纯规则：格宽和间距与 iOS 相同、放得下时靠行尾、放不下时从最前面的工具起滚动、间距不随数量和尺寸变化。真实布局由设备套件的 ToolbarEdgesDeviceSmoke 逐个对这里的位置。 */
public final class ToolbarClusterPolicySmoke {
    public static void main(String[] args) {
        // 与 iOS 工具栏相同的数值：40 的格子（KeyboardToolbarButton.padSide）、8 的间距（shortcutBar.spacing）。
        check(ToolbarClusterPolicy.CELL_DP == 40, "cell matches the iOS toolbar cell");
        check(ToolbarClusterPolicy.GAP_DP == 8, "gap matches the iOS toolbar spacing");
        // 工具的最小点击宽度没有变：100% 和 90% 都是一格，放大时按比例加宽。
        check(ToolbarSizePolicy.buttonWidthDp(ToolbarClusterPolicy.CELL_DP, 90) == 40, "90% keeps the minimum cell");
        check(ToolbarSizePolicy.buttonWidthDp(ToolbarClusterPolicy.CELL_DP, 150) == 60, "150% widens the cell");

        int cell = 40, gap = 8, viewport = 300;
        // 放得下：工具贴着视口的行尾，相邻两个工具的距离固定是格宽加间距，和可见工具的数量无关。
        for (int visible = 1; visible <= 6; visible++) {
            check(ToolbarClusterPolicy.scrollRange(viewport, 0, visible, cell, gap) == 0, visible + " tools fit");
            int last = ToolbarClusterPolicy.toolOffset(visible - 1, viewport, 0, visible, cell, gap);
            check(last + cell == viewport, visible + " tools end at the viewport end");
            for (int index = 1; index < visible; index++)
                check(ToolbarClusterPolicy.toolOffset(index, viewport, 0, visible, cell, gap)
                    - ToolbarClusterPolicy.toolOffset(index - 1, viewport, 0, visible, cell, gap) == cell + gap,
                    "pitch does not depend on how many tools are visible");
        }
        // 原来的排法把 300 宽等分给可见工具：2 个工具时中心距是 150，6 个时是 50。现在都是 48。
        check(ToolbarClusterPolicy.toolOffset(0, viewport, 0, 2, cell, gap) == 300 - 2 * 48 + 8, "two tools sit at the end, not spread");
        check(ToolbarClusterPolicy.leadingSpace(viewport, 0, 2, cell, gap) == 204, "the slack goes in front of the tools");
        // 正好放下和差 1 放不下的分界。
        check(ToolbarClusterPolicy.scrollRange(288, 0, 6, cell, gap) == 0 && ToolbarClusterPolicy.leadingSpace(288, 0, 6, cell, gap) == 0,
            "an exact fit neither scrolls nor leaves slack");
        check(ToolbarClusterPolicy.scrollRange(287, 0, 6, cell, gap) == 1, "one unit short starts scrolling");
        // 放不下：没有空白，第一个工具在滚动位置 0 时整个露在视口里，格宽不压缩，能滚到最后一个工具。
        int many = 11;
        check(ToolbarClusterPolicy.leadingSpace(viewport, 0, many, cell, gap) == 0, "overflow leaves no slack");
        check(ToolbarClusterPolicy.toolOffset(0, viewport, 0, many, cell, gap) == gap, "first tool starts the scroll content");
        check(ToolbarClusterPolicy.toolOffset(0, viewport, 0, many, cell, gap) + cell <= viewport, "first tool is visible at the scroll start");
        int range = ToolbarClusterPolicy.scrollRange(viewport, 0, many, cell, gap);
        check(range == many * 48 - viewport, "overflow scrolls by the hidden width");
        check(ToolbarClusterPolicy.toolOffset(many - 1, viewport, 0, many, cell, gap) + cell - range == viewport,
            "last tool ends at the viewport end after scrolling to the end");
        // 结果胶囊排在最前面：放得下时它留在行首，工具仍贴行尾；放不下时它占滚动内容的开头。
        check(ToolbarClusterPolicy.toolOffset(2, viewport, 90, 3, cell, gap) + cell == viewport, "tools stay at the end beside the chip");
        check(ToolbarClusterPolicy.leadingSpace(viewport, 90, 3, cell, gap) == 300 - 90 - 144, "slack sits between chip and tools");
        check(ToolbarClusterPolicy.toolOffset(0, viewport, 90, 6, cell, gap) == 98, "overflowing tools follow the chip");
        check(ToolbarClusterPolicy.scrollRange(viewport, 90, 6, cell, gap) == 78, "the chip counts toward the scroll range");
        // 没有可见工具时不滚动。
        check(ToolbarClusterPolicy.scrollRange(viewport, 0, 0, cell, gap) == 0 && ToolbarClusterPolicy.clusterWidth(-1, cell, gap) == 0,
            "no tools, no content");

        // 收起键是 44 的点击区，比格子宽 4：它前面留 6，最后一个工具到收起键的图标中心距 20 + 6 + 22 = 48，和工具之间相同。
        check(ToolbarClusterPolicy.edgeGap(8, 44, 40) == 6, "dismiss gap keeps the icon pitch");
        // 150% 时格子 60、收起键 66，间距仍是 8：30 + 5 + 33 = 68 = 60 + 8。
        check(ToolbarClusterPolicy.edgeGap(8, 66, 60) == 5, "dismiss gap keeps the pitch when enlarged");
        check(ToolbarClusterPolicy.edgeGap(8, 40, 40) == 8 && ToolbarClusterPolicy.edgeGap(8, 30, 40) == 8, "no narrower edge, no change");
        check(ToolbarClusterPolicy.edgeGap(8, 80, 40) == 0, "never negative");
        // 排序时工具排在结果胶囊和可伸缩空白之后。
        check(ToolbarClusterPolicy.LEADING_CHILDREN == 2, "chip and stretch space precede the tools");
        System.out.println("ToolbarClusterPolicySmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
