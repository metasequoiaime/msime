package app.msime.android.home;

import java.util.List;

/** 排序面板的挪动、序号与拖动（#10392、#10396，#10546 合并为一份）：拖动一次跨过几行时保存的顺序要和屏幕上一致，读屏「上移」「下移」只在表内提供，拖动的位次、落点和贴边自动滚动在「文本编辑按键顺序」「符号分类顺序」「按钮顺序」和「九键左侧符号」编辑页同一套规则。 */
public final class ReorderPolicySmoke {
    public static void main(String[] args) {
        List<String> order = List.of("a", "b", "c", "d");

        check(ReorderPolicy.moved(order, 1, 2).equals(List.of("a", "c", "b", "d")), "adjacent move down swaps neighbours");
        check(ReorderPolicy.moved(order, 2, 1).equals(List.of("a", "c", "b", "d")), "adjacent move up swaps neighbours");
        // 跨多行是挪动，不是首尾互换：互换会得到 [c, b, a, d]，和 notifyItemMoved 画出来的 [b, c, a, d] 不一致。
        check(ReorderPolicy.moved(order, 0, 2).equals(List.of("b", "c", "a", "d")), "dragging over two rows moves, not swaps");
        check(ReorderPolicy.moved(order, 3, 0).equals(List.of("d", "a", "b", "c")), "last item moves to the top");
        check(ReorderPolicy.moved(order, 0, 3).equals(List.of("b", "c", "d", "a")), "first item moves to the bottom");
        check(ReorderPolicy.moved(order, 2, 2).equals(order), "moving onto itself keeps the order");

        // 读屏「上移」「下移」逐格挪：一路往下挪三格，每一步都按挪动累加，最终与一次拖到底相同。
        List<String> stepped = order;
        for (int from = 0; from < 3; from++) stepped = ReorderPolicy.moved(stepped, from, from + 1);
        check(stepped.equals(ReorderPolicy.moved(order, 0, 3)), "step-by-step drag ends where a single move does");

        check(ReorderPolicy.moved(order, -1, 0).equals(order), "negative source is ignored");
        check(ReorderPolicy.moved(order, 0, 4).equals(order), "target past the end is ignored");
        check(ReorderPolicy.moved(List.of(), 0, 0).isEmpty(), "empty order stays empty");
        check(order.equals(List.of("a", "b", "c", "d")), "source list is untouched");
        boolean immutable = false;
        try { ReorderPolicy.moved(order, 0, 1).add("e"); } catch (UnsupportedOperationException expected) { immutable = true; }
        check(immutable, "returned order is immutable");

        check(!ReorderPolicy.canMove(0, -1, 4), "first row cannot move up");
        check(ReorderPolicy.canMove(0, 1, 4), "first row can move down");
        check(ReorderPolicy.canMove(3, -1, 4), "last row can move up");
        check(!ReorderPolicy.canMove(3, 1, 4), "last row cannot move down");
        check(!ReorderPolicy.canMove(0, 1, 1), "a single row cannot move");
        check(!ReorderPolicy.canMove(4, -1, 4), "a row outside the list cannot move");

        check(ReorderPolicy.ordinal(0, "复制").equals("1. 复制"), "first row is numbered 1");
        check(ReorderPolicy.ordinal(19, "更多").equals("20. 更多"), "numbering follows the position");
        List<String> renumbered = ReorderPolicy.moved(order, 0, 2);
        check(ReorderPolicy.ordinal(renumbered.indexOf("a"), "a").equals("3. a"), "moved row takes its new number");
        check(ReorderPolicy.ordinal(renumbered.indexOf("b"), "b").equals("1. b"), "rows that made room are renumbered");
        check(ReorderPolicy.ordinalDescription(0, "复制").equals("第 1 项，复制"), "TalkBack reads the position once");
        // 「九键左侧符号」编辑页把名称写成「符号：？」，念法与原来 NineKeySidebarPolicy.ordinalDescription 一字不差。
        check(ReorderPolicy.ordinalDescription(2, "符号：？").equals("第 3 项，符号：？"), "a symbol row reads its position and the symbol");
        check(ReorderPolicy.positionDescription(19).equals("第 20 项"), "the handle's state names its position");
        check(ReorderPolicy.positionDescription(0).equals("第 1 项"), "positions count from one");

        // 拖动途中每一行的位次：被拖行在 to，中间的行让一格，按位次画出的就是挪完的表。
        for (int from = 0; from < order.size(); from++) {
            for (int to = 0; to < order.size(); to++) {
                String[] shown = new String[order.size()];
                for (int index = 0; index < order.size(); index++) shown[ReorderPolicy.dragSlot(index, from, to)] = order.get(index);
                check(List.of(shown).equals(ReorderPolicy.moved(order, from, to)),
                    "slots during a drag from " + from + " to " + to + " match the dropped order");
            }
        }
        check(ReorderPolicy.dragSlot(0, 0, 2) == 2 && ReorderPolicy.dragSlot(1, 0, 2) == 0
            && ReorderPolicy.dragSlot(2, 0, 2) == 1 && ReorderPolicy.dragSlot(3, 0, 2) == 3,
            "rows between the two ends make way by one");
        // 每行 40 高：中线 20、60、100、140。
        float[] centers = {20, 60, 100, 140};
        check(ReorderPolicy.dragTarget(20, 0, centers) == 0, "a row that has not moved stays put");
        check(ReorderPolicy.dragTarget(59, 0, centers) == 0, "short of the next row's midline it stays");
        check(ReorderPolicy.dragTarget(61, 0, centers) == 1, "past the next row's midline it takes that place");
        check(ReorderPolicy.dragTarget(101, 0, centers) == 2, "past two midlines it moves two places");
        check(ReorderPolicy.dragTarget(500, 0, centers) == 3, "far below the list it goes last");
        check(ReorderPolicy.dragTarget(-50, 3, centers) == 0, "far above the list it goes first");
        check(ReorderPolicy.dragTarget(99, 3, centers) == 2, "dragging up past one midline");
        check(ReorderPolicy.dragTarget(141, 1, centers) == 3, "the dragged row's own midline is never counted");
        check(ReorderPolicy.dragTarget(140, 0, centers) == 3, "pinned at the bottom it reaches the last place");
        check(ReorderPolicy.dragTarget(20, 3, centers) == 0, "pinned at the top it reaches the first place");
        check(ReorderPolicy.dragTarget(60, 0, centers) == 1 && ReorderPolicy.dragTarget(60, 3, centers) == 1,
            "touching a midline counts from either side");

        // 滚动区 100..700，边缘带 50，每帧最多 20：「符号分类顺序」20 行比一屏高，拖到边缘时面板要自己滚。
        check(ReorderPolicy.edgeScrollStep(400, 100, 700, 50, 20) == 0, "the middle does not scroll");
        check(ReorderPolicy.edgeScrollStep(150, 100, 700, 50, 20) == 0
            && ReorderPolicy.edgeScrollStep(650, 100, 700, 50, 20) == 0, "the band starts inside the edge");
        check(ReorderPolicy.edgeScrollStep(125, 100, 700, 50, 20) == -10, "half into the top band scrolls up at half speed");
        check(ReorderPolicy.edgeScrollStep(690, 100, 700, 50, 20) == 16, "deep in the bottom band scrolls down faster");
        check(ReorderPolicy.edgeScrollStep(40, 100, 700, 50, 20) == -20
            && ReorderPolicy.edgeScrollStep(900, 100, 700, 50, 20) == 20, "past the edge is the top speed");
        check(ReorderPolicy.edgeScrollStep(149.9f, 100, 700, 50, 20) == -1, "just inside the band still moves");
        check(ReorderPolicy.edgeScrollStep(120, 100, 180, 50, 20) == 0, "a viewport too short for two bands never auto-scrolls");
        check(ReorderPolicy.scrollTowardRoom(16, 120, -40, 120) == 0, "pinned at the bottom of the list it stops scrolling down");
        check(ReorderPolicy.scrollTowardRoom(-16, -40, -40, 120) == 0, "pinned at the top of the list it stops scrolling up");
        check(ReorderPolicy.scrollTowardRoom(-16, 120, -40, 120) == -16
            && ReorderPolicy.scrollTowardRoom(16, -40, -40, 120) == 16, "the other direction still scrolls");
        check(ReorderPolicy.scrollTowardRoom(16, 80, -40, 120) == 16, "with room left it keeps scrolling");
        check(ReorderPolicy.scrollTowardRoom(0, 120, -40, 120) == 0, "no step stays no step");
        System.out.println("ReorderPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
