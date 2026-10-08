import app.msime.android.ArithmeticResultPolicy;

/** 数字键面的四则运算（#5688）：从光标前文字末尾取算式、求值、格式化，以及上屏的文字。 */
public final class ArithmeticResultPolicySmoke {
    public static void main(String[] args) {
        results();
        noResult();
        commitText();
        System.out.println("Android arithmetic result: trailing expressions, precedence, decimals, full-width input and commit text passed");
    }

    private static void results() {
        check("144", "12*12");
        check("144", "买苹果 12*12");
        check("14", "2+3*4");
        check("20", "(2+3)*4");
        check("0.3", "0.1+0.2");
        check("0.3333333333", "1/3");
        check("2.5", "5/2");
        check("-1", "-3+2");
        check("-15", "5*-3");
        check("7.5", "3.5+2×(4-1)/1.5");
        check("12", "24÷2");
        check("144", "１２＊１２");
        check("144", "１２×１２");
        check("0", "3-3");
        check("15", "12 + 3");
        check("15", ")12+3");
        check("1.5", ".5+1");
        check("100000000000000", "99999999999999+1");
        check("5", "版本 2+3");
        // 除法之后再乘：中间值的舍入误差不能放大到结果里。
        check("10000000", "50/3*600000");
        check("1000000", "1/7*7000000");
        check("1", "1/3*3");
        check("33.3333333333", "100/3");
        // 只排除日期和号码的写法，真正的减法、连除和小数照常算。
        check("7", "10-3");
        check("5", "20 - 10 - 5");
        check("40", "100/5*2");
        check("1.55", "1.05+0.5");
        check("0.5", "0.25*2");
        check("1", "0-1+2");
        check("98", "100 - 1 - 1");
    }

    private static void noResult() {
        for (String text : new String[] {"", "12", "-5", "(5)", "12*", "12*12=144", "1/0", "2*(3", "1..2+3",
                "1000000000000000*1", "abc", "12+3a",
                // 日期、电话号码不是算式。
                "2026-10-08", "2026/10/08", "今天 2026-10-08", "138-1234-5678", "+86 138-1234-5678", "0571-88886666",
                "10-08", "电话 010-12345678",
                // 代价：不带空格的同号连减、连除也当成号码；加上空格或换个运算符就照常算。
                "20-10-5", "100/5/2"}) {
            check(ArithmeticResultPolicy.evaluateTrailing(text) == null, "no result for " + text);
        }
        check(ArithmeticResultPolicy.evaluateTrailing(null) == null, "no text, no result");
    }

    private static void commitText() {
        ArithmeticResultPolicy.Result result = ArithmeticResultPolicy.evaluateTrailing("12*12");
        check(result != null && "=144".equals(result.commitText()) && "= 144".equals(result.label()),
            "the result commits as =144 after a bare expression");
        ArithmeticResultPolicy.Result afterEquals = ArithmeticResultPolicy.evaluateTrailing("12*12=");
        check(afterEquals != null && afterEquals.afterEquals() && "144".equals(afterEquals.commitText()),
            "after a typed = only the value is committed");
        ArithmeticResultPolicy.Result fullWidthEquals = ArithmeticResultPolicy.evaluateTrailing("１２＊１２＝ ");
        check(fullWidthEquals != null && "144".equals(fullWidthEquals.commitText()), "a full-width = counts too");
        StringBuilder longText = new StringBuilder();
        for (int index = 0; index < 100; index++) longText.append('字');
        longText.append("6*7");
        ArithmeticResultPolicy.Result tail = ArithmeticResultPolicy.evaluateTrailing(longText);
        check(tail != null && "42".equals(tail.value()), "only the tail of a long context is read");
    }

    private static void check(String expected, String text) {
        ArithmeticResultPolicy.Result result = ArithmeticResultPolicy.evaluateTrailing(text);
        check(result != null && expected.equals(result.value()),
            text + " should be " + expected + ", got " + (result == null ? null : result.value()));
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
