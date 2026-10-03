import app.msime.android.SelectionEchoTracker;

public final class SelectionEchoTrackerSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 九键：上屏「你好」之后用户已经按下了下一个数字键（九键组字不写编辑器），上屏的回声迟到也不算光标移动。
        SelectionEchoTracker nineKey = new SelectionEchoTracker();
        nineKey.reset(5, 5);
        nineKey.commit(2);
        nineKey.expect();
        check(nineKey.acknowledge(7, 7, -1, -1));
        // 回声已经消费掉，之后用户点到别处就是真正的移动。
        check(!nineKey.acknowledge(2, 2, -1, -1));
        // 重新定基之后照常预测：在 2 处再上屏一个字，回声是 3。
        nineKey.commit(1);
        nineKey.expect();
        check(nineKey.acknowledge(3, 3, -1, -1));

        // 26 键：组字区的每次更新和「上屏 + 新组字」同一批写入，各自的回声都认得出来。
        SelectionEchoTracker full = new SelectionEchoTracker();
        full.reset(0, 0);
        full.compose(1);
        full.expect();
        full.compose(2);
        full.expect();
        full.commit(1);
        full.compose(1);
        full.expect();
        check(full.acknowledge(1, 1, 0, 1));
        // 上屏「你」之后组字 h：光标同样停在 2，回声按顺序对上并消掉更早的那一条。
        check(full.acknowledge(2, 2, 0, 2));
        check(full.acknowledge(2, 2, 1, 2));
        check(!full.acknowledge(2, 2, 1, 2));

        // 编辑器把几次写入合成一次回报时，对上较新的预期也会作废更早的。
        SelectionEchoTracker coalesced = new SelectionEchoTracker();
        coalesced.reset(0, 0);
        coalesced.commit(1);
        coalesced.expect();
        coalesced.commit(2);
        coalesced.expect();
        check(coalesced.acknowledge(3, 3, -1, -1));
        check(!coalesced.acknowledge(1, 1, -1, -1));

        // 上屏替换选区：选区的起点加上文字长度；起止顺序反着报也一样。
        SelectionEchoTracker selection = new SelectionEchoTracker();
        selection.reset(6, 2);
        selection.commit(3);
        selection.expect();
        check(selection.acknowledge(5, 5, -1, -1));
        selection.reset(2, 6);
        selection.compose(2);
        selection.expect();
        check(selection.acknowledge(4, 4, 2, 4));

        // 结束组字不动光标，回声仍是自己的。
        SelectionEchoTracker finish = new SelectionEchoTracker();
        finish.reset(0, 0);
        finish.compose(3);
        finish.expect();
        finish.finish();
        finish.expect();
        check(finish.acknowledge(3, 3, 0, 3));
        check(finish.acknowledge(3, 3, -1, -1));

        // 没有写入就不记预期：没有写入的批量编辑不会让之后点回原处的操作被当成回声。
        SelectionEchoTracker idle = new SelectionEchoTracker();
        idle.reset(4, 4);
        idle.expect();
        check(!idle.acknowledge(4, 4, -1, -1));

        // 初始选区未知、或写入结果算不出来时不预测，退回原来「一律当外部变化」的行为，直到下一次回报重新定基。
        SelectionEchoTracker unknown = new SelectionEchoTracker();
        unknown.reset(-1, -1);
        unknown.commit(1);
        unknown.expect();
        check(!unknown.acknowledge(1, 1, -1, -1));
        unknown.commit(1);
        unknown.expect();
        check(unknown.acknowledge(2, 2, -1, -1));
        unknown.invalidate();
        unknown.commit(1);
        unknown.expect();
        check(!unknown.acknowledge(3, 3, -1, -1));
        check(!unknown.acknowledge(-1, -1, -1, -1));
        unknown.commit(1);
        unknown.expect();
        check(!unknown.acknowledge(0, 0, -1, -1));

        // invalidate 后，之前写入的迟到回声已经不能再代表当前编辑器状态；它必须按外部变化重新定基。
        SelectionEchoTracker invalidated = new SelectionEchoTracker();
        invalidated.reset(0, 0);
        invalidated.commit(2);
        invalidated.expect();
        invalidated.invalidate();
        check(!invalidated.acknowledge(2, 2, -1, -1));

        // 回报带着组字区时，下一次上屏替换的是那个组字区。
        SelectionEchoTracker rebased = new SelectionEchoTracker();
        rebased.reset(0, 0);
        check(!rebased.acknowledge(9, 9, 6, 9));
        rebased.commit(1);
        rebased.expect();
        check(rebased.acknowledge(7, 7, -1, -1));

        // 预期有上限，最早的一条被挤掉后就不再认作回声。
        SelectionEchoTracker bounded = new SelectionEchoTracker();
        bounded.reset(0, 0);
        for (int index = 0; index < 20; index++) {
            bounded.commit(1);
            bounded.expect();
        }
        check(!bounded.acknowledge(1, 1, -1, -1));
        bounded.reset(0, 0);
        for (int index = 0; index < 20; index++) {
            bounded.commit(1);
            bounded.expect();
        }
        check(bounded.acknowledge(5, 5, -1, -1));
        check(bounded.acknowledge(20, 20, -1, -1));
        check(!bounded.acknowledge(20, 20, -1, -1));
        // 组字区也要对上：应用去掉了组字区但光标没动，这不是自己的回声，要重新定基、照旧取消组字。
        SelectionEchoTracker dropped = new SelectionEchoTracker();
        dropped.reset(4, 4);
        dropped.compose(1);
        dropped.expect();
        check(!dropped.acknowledge(5, 5, -1, -1));
        dropped.compose(1);
        dropped.expect();
        check(dropped.acknowledge(6, 6, 5, 6));
        dropped.reset(4, 4);
        dropped.compose(2);
        dropped.expect();
        check(!dropped.acknowledge(6, 6, 5, 6));

        // 没有组字时按删除：迟到的回声仍认得出来，九键紧接着按下的数字不会被取消。
        SelectionEchoTracker delete = new SelectionEchoTracker();
        delete.reset(10, 10);
        delete.deleteBefore(2);
        delete.expect();
        check(delete.acknowledge(8, 8, -1, -1));
        delete.deleteCodePointBefore();
        delete.expect();
        check(delete.acknowledge(7, 7, -1, -1));
        delete.deleteCodePointBefore();
        delete.expect();
        check(delete.acknowledge(4, 4, -1, -1) == false);

        // 按码位删除删掉的是代理对：回声是两个单元，对上之后后面的预期按它收窄。
        SelectionEchoTracker surrogate = new SelectionEchoTracker();
        surrogate.reset(10, 10);
        surrogate.deleteCodePointBefore();
        surrogate.expect();
        surrogate.commit(1);
        surrogate.expect();
        check(surrogate.acknowledge(8, 8, -1, -1));
        check(!surrogate.acknowledge(10, 10, -1, -1));
        surrogate.reset(10, 10);
        surrogate.deleteCodePointBefore();
        surrogate.expect();
        surrogate.commit(1);
        surrogate.expect();
        check(surrogate.acknowledge(8, 8, -1, -1));
        check(surrogate.acknowledge(9, 9, -1, -1));
        // 删除之后组字：组字区随同一个偏移确定下来。
        SelectionEchoTracker deleteThenCompose = new SelectionEchoTracker();
        deleteThenCompose.reset(10, 10);
        deleteThenCompose.deleteCodePointBefore();
        deleteThenCompose.compose(1);
        deleteThenCompose.expect();
        check(!deleteThenCompose.acknowledge(10, 10, 8, 10));
        deleteThenCompose.reset(10, 10);
        deleteThenCompose.deleteCodePointBefore();
        deleteThenCompose.compose(1);
        deleteThenCompose.expect();
        check(deleteThenCompose.acknowledge(9, 9, 8, 9));

        // 有选区、有组字区、离文档开头太近、或者连续删除的不确定量超限时不预测删除。
        SelectionEchoTracker unsure = new SelectionEchoTracker();
        unsure.reset(3, 6);
        unsure.deleteCodePointBefore();
        unsure.expect();
        check(!unsure.acknowledge(2, 5, -1, -1));
        unsure.compose(1);
        unsure.deleteBefore(1);
        unsure.expect();
        check(!unsure.acknowledge(2, 2, -1, -1));
        unsure.reset(1, 1);
        unsure.deleteCodePointBefore();
        unsure.expect();
        check(!unsure.acknowledge(0, 0, -1, -1));
        unsure.reset(20, 20);
        for (int index = 0; index < 5; index++) {
            unsure.deleteCodePointBefore();
            unsure.expect();
        }
        // 前四次的回声逐个对上（每次都按回声收窄），超限的第五次没有记预期。
        check(unsure.acknowledge(16, 16, -1, -1));
        check(unsure.acknowledge(15, 15, -1, -1));
        check(unsure.acknowledge(14, 14, -1, -1));
        check(!unsure.acknowledge(13, 13, -1, -1));
        System.out.println("SelectionEchoTracker smoke passed");
    }
}
