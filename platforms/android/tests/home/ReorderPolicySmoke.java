package app.msime.android.home;

import java.util.List;

/** 排序面板的挪动与序号（#10392）：拖动一次跨过几行时保存的顺序要和屏幕上一致，读屏「上移」「下移」只在表内提供。 */
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

        // ItemTouchHelper 逐格回调 onMove：一路往下拖三格，每一步都按挪动累加，最终与一次挪到底相同。
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
        System.out.println("ReorderPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
