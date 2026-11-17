package app.msime.android;

/** 符号网格行高：多行均分可见高度，矮面板保留最小格高。 */
public final class SymbolGridRowPolicySmoke {
    public static void main(String[] args) {
        check(SymbolGridRowPolicy.cellHeight(188, 40, 3) == 41, "四行均分剩余高度");
        check(SymbolGridRowPolicy.cellHeight(230, 40, 3) == 40, "五行刚好保持最小格高");
        check(SymbolGridRowPolicy.cellHeight(100, 40, 3) == 44, "两行均分可见高度");
        check(SymbolGridRowPolicy.cellHeight(44, 40, 3) == 40, "不足一行时保留最小格高");
        check(SymbolGridRowPolicy.cellHeight(0, 40, 3) == 40, "未测量时保留最小格高");
        check(SymbolGridRowPolicy.cellHeight(188, 0, 3) == 0, "无最小格高时返回零");
        System.out.println("Android symbol grid row policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
