import app.msime.android.ClipboardSelectionPolicy;

public final class ClipboardSelectionPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ClipboardSelectionPolicy.MAX_SELECTIONS == 20, "selection cap is 20");
        check(ClipboardSelectionPolicy.canSelect(0), "empty selection can add");
        check(ClipboardSelectionPolicy.canSelect(19), "nineteen selected can add");
        check(!ClipboardSelectionPolicy.canSelect(20), "twenty selected cannot add");

        // 插入上限 4 条，和选择上限 20 条各管各的（#10411）。
        check(ClipboardSelectionPolicy.MAX_INSERTIONS == 4, "insertion cap is 4");
        check(!ClipboardSelectionPolicy.canInsert(0), "empty selection cannot insert");
        check(ClipboardSelectionPolicy.canInsert(1), "one selected can insert");
        check(ClipboardSelectionPolicy.canInsert(4), "four selected can insert");
        check(!ClipboardSelectionPolicy.canInsert(5), "five selected cannot insert");
        check(ClipboardSelectionPolicy.canSelect(4), "insertion cap does not limit selection");

        check("①".equals(ClipboardSelectionPolicy.ordinalLabel(0)), "first ordinal is circled one");
        check("④".equals(ClipboardSelectionPolicy.ordinalLabel(3)), "fourth ordinal is circled four");
        check("⑳".equals(ClipboardSelectionPolicy.ordinalLabel(19)), "twentieth ordinal is circled twenty");
        checkThrows(() -> ClipboardSelectionPolicy.ordinalLabel(-1), "negative ordinal is rejected");
        checkThrows(() -> ClipboardSelectionPolicy.ordinalLabel(20), "ordinal past the selection cap is rejected");

        java.util.Set<String> selected = new java.util.LinkedHashSet<>(
            java.util.List.of("合成甲", "合成乙", "合成丙", "合成丁"));
        check(ClipboardSelectionPolicy.ordinal(selected, "合成甲") == 0, "first tap is ordinal one");
        check(ClipboardSelectionPolicy.ordinal(selected, "合成丁") == 3, "fourth tap is ordinal four");
        check(ClipboardSelectionPolicy.ordinal(selected, "未选") == -1, "unselected text has no ordinal");
        selected.remove("合成甲");
        check(ClipboardSelectionPolicy.ordinal(selected, "合成乙") == 0, "deselecting one moves two to one");
        check(ClipboardSelectionPolicy.ordinal(selected, "合成丙") == 1, "deselecting one moves three to two");
        check(ClipboardSelectionPolicy.ordinal(selected, "合成丁") == 2, "deselecting one moves four to three");
        selected.add("合成甲");
        check(ClipboardSelectionPolicy.ordinal(selected, "合成甲") == 3, "reselecting appends at the end");

        check("合成乙合成丙合成丁合成甲".equals(ClipboardSelectionPolicy.insertionText(selected)),
            "insertion joins in ordinal order without separators");
        check("".equals(ClipboardSelectionPolicy.insertionText(java.util.List.of())), "empty insertion is empty");

        check(!ClipboardSelectionPolicy.exceedsInsertionCap(4), "fourth selection keeps insert enabled");
        check(ClipboardSelectionPolicy.exceedsInsertionCap(5), "fifth selection disables insert once");
        check(!ClipboardSelectionPolicy.exceedsInsertionCap(6), "later selections do not repeat the hint");
        check("一次最多插入 4 项，导出和删除不受影响".equals(ClipboardSelectionPolicy.INSERTION_LIMIT_HINT),
            "insertion hint names the cap");
        check("已选择第 2 项；".equals(ClipboardSelectionPolicy.selectedDescription(1)), "selected description names the ordinal");
        check("".equals(ClipboardSelectionPolicy.selectedDescription(-1)), "unselected description is empty");
        System.out.println("Android clipboard multi-selection cap passed");
    }

    private static void checkThrows(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError(message);
    }
}
