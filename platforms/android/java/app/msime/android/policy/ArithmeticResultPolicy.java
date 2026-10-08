package app.msime.android;

import java.math.BigDecimal;
import java.math.MathContext;
import java.math.RoundingMode;

/**
 * 数字键面上的简单四则运算（#5688）：从光标前的文字末尾找出一个算式（如 `12*12`、`3.5+2×(4-1)`），算出结果给工具栏显示，点一下上屏。
 *
 * <p>只认加减乘除、括号、小数点和一元负号；`×` `÷` 与全角的数字、运算符（全角输入打出来的）一并认得。算式要以数字或右括号结尾、至少有一次二元运算，所以打到 `12*` 时没有结果，单独一个数字也没有。像日期、电话号码的写法（`2026-10-08`、`138-1234-5678`、`0571-88886666`）不算（{@link #looksLikeNumberSequence}）。末尾已经有 `=` 时只给结果本身，否则上屏的是 `=结果`，凑成 `12*12=144`。用 BigDecimal 计算，`0.1+0.2` 是 `0.3`；除不尽时保留 10 位小数，除以零或结果的绝对值达到 10^15 时不给结果。
 */
public final class ArithmeticResultPolicy {
    /** 最多看光标前多少个字符；也是调用方向编辑器要上文时的长度。 */
    public static final int MAX_CONTEXT = 64;
    /** 结果最多保留的小数位数。 */
    static final int MAX_FRACTION_DIGITS = 10;
    private static final BigDecimal LIMIT = BigDecimal.TEN.pow(15);
    /** 计算中途的上限：再大的中间值不可能落回 {@link #LIMIT} 以内的有用结果，提前放弃。 */
    private static final BigDecimal INTERMEDIATE_LIMIT = BigDecimal.TEN.pow(30);
    /** 除法的中间精度（34 位有效数字）：只在最后格式化时舍到 {@link #MAX_FRACTION_DIGITS} 位。用 16 位（DECIMAL64）时 `50/3*600000` 会把除法的舍入误差放大到第 9 位小数，显示成 `10000000.000000002`。 */
    private static final MathContext DIVISION = MathContext.DECIMAL128;

    /** 一个算式的结果。 */
    public record Result(String value, boolean afterEquals) {
        /** 点按时上屏的文字：算式后面已经有 `=` 时只是结果，否则是 `=结果`。 */
        public String commitText() {
            return afterEquals ? value : "=" + value;
        }

        /** 工具栏上显示的文字。 */
        public String label() {
            return "= " + value;
        }
    }

    private ArithmeticResultPolicy() {}

    /**
     * 光标前文字末尾那个算式的结果。
     *
     * @param before 光标前的文字（只看最后 {@link #MAX_CONTEXT} 个字符）
     * @return 结果；末尾不是一个有二元运算的完整算式、除以零或结果过大时为 null
     */
    public static Result evaluateTrailing(CharSequence before) {
        if (before == null || before.length() == 0) return null;
        int from = Math.max(0, before.length() - MAX_CONTEXT);
        StringBuilder text = new StringBuilder(before.length() - from);
        for (int index = from; index < before.length(); index++) text.append(normalize(before.charAt(index)));
        int end = text.length();
        while (end > 0 && text.charAt(end - 1) == ' ') end--;
        boolean afterEquals = end > 0 && text.charAt(end - 1) == '=';
        if (afterEquals) {
            end--;
            while (end > 0 && text.charAt(end - 1) == ' ') end--;
        }
        int start = end;
        while (start > 0 && expressionChar(text.charAt(start - 1))) start--;
        // 从最长的后缀开始试：前面混进来的半个算式（例如多余的右括号或运算符）去掉之后，剩下的仍可能是完整的算式。
        for (int candidate = start; candidate < end; candidate++) {
            char first = text.charAt(candidate);
            if (!isDigit(first) && first != '(' && first != '-' && first != '.') continue;
            // 不从一个数的中间开始：`123+4` 里的 `23+4` 不是用户写的算式。
            if (candidate > 0 && (isDigit(text.charAt(candidate - 1)) || text.charAt(candidate - 1) == '.')) continue;
            String expression = text.substring(candidate, end);
            BigDecimal value = new Parser(expression).parse();
            if (value == null) continue;
            // 最长的完整算式像日期、电话号码时整体不算，也不退到它更短的后缀（`1234-5678` 同样不是算式）。
            if (looksLikeNumberSequence(expression)) return null;
            String formatted = format(value);
            return formatted == null ? null : new Result(formatted, afterEquals);
        }
        return null;
    }

    /** 全角数字和运算符、`×` `÷` 换成半角；其余原样。 */
    static char normalize(char c) {
        if (c >= '０' && c <= '９') return (char) ('0' + (c - '０'));
        return switch (c) {
            case '＋' -> '+';
            case '－', '−' -> '-';
            case '＊', '×' -> '*';
            case '／', '÷' -> '/';
            case '（' -> '(';
            case '）' -> ')';
            case '．' -> '.';
            case '＝' -> '=';
            case '　' -> ' ';
            default -> c;
        };
    }

    private static boolean isDigit(char c) {
        return c >= '0' && c <= '9';
    }

    private static boolean expressionChar(char c) {
        return (c >= '0' && c <= '9') || c == '.' || c == '+' || c == '-' || c == '*' || c == '/'
            || c == '(' || c == ')' || c == ' ';
    }

    /**
     * 形式上是算式、其实是日期或号码的写法，不给结果：两个以上同一个 `-` 或 `/` 连着的纯数字段（`2026-10-08`、`2026/10/08`、`138-1234-5678`），或者有以 0 开头的多位整数（`0571-88886666`、`10-08`），算术里不会这样写数。代价是不带空格的同号连减、连除（`20-10-5`、`100/5/2`）也不给结果，写成 `20 - 10 - 5` 或混用别的运算符就照常算；数字键盘上打日期、电话号码远比这种写法常见。
     */
    static boolean looksLikeNumberSequence(String expression) {
        for (int index = 0; index + 1 < expression.length(); index++) {
            boolean numberStart = index == 0 || (!isDigit(expression.charAt(index - 1)) && expression.charAt(index - 1) != '.');
            if (numberStart && expression.charAt(index) == '0' && isDigit(expression.charAt(index + 1))) return true;
        }
        char separator = 0;
        int separators = 0;
        boolean digitBefore = false;
        for (int index = 0; index < expression.length(); index++) {
            char c = expression.charAt(index);
            if (isDigit(c)) {
                digitBefore = true;
                continue;
            }
            if ((c != '-' && c != '/') || !digitBefore || (separator != 0 && c != separator)) return false;
            separator = c;
            separators++;
            digitBefore = false;
        }
        return digitBefore && separators >= 2;
    }

    private static String format(BigDecimal value) {
        if (value.abs().compareTo(LIMIT) >= 0) return null;
        BigDecimal rounded = value.scale() > MAX_FRACTION_DIGITS
            ? value.setScale(MAX_FRACTION_DIGITS, RoundingMode.HALF_UP) : value;
        if (rounded.signum() == 0) return "0";
        return rounded.stripTrailingZeros().toPlainString();
    }

    /** 递归下降：expr := term (('+'|'-') term)*，term := factor (('*'|'/') factor)*，factor := ('-'|'+') factor | number | '(' expr ')'。 */
    private static final class Parser {
        private final String text;
        private int position;
        private int operations;
        private boolean failed;

        Parser(String text) {
            this.text = text;
        }

        /** 整串恰好是一个至少带一次二元运算的算式时返回它的值，否则 null。 */
        BigDecimal parse() {
            BigDecimal value = expression();
            skipSpaces();
            if (failed || value == null || position != text.length() || operations == 0) return null;
            return value;
        }

        private BigDecimal expression() {
            BigDecimal value = term();
            while (!failed && value != null) {
                skipSpaces();
                if (!peek('+') && !peek('-')) return value;
                char operator = text.charAt(position++);
                BigDecimal right = term();
                if (right == null) return null;
                value = operator == '+' ? value.add(right) : value.subtract(right);
                operations++;
            }
            return value;
        }

        private BigDecimal term() {
            BigDecimal value = factor();
            while (!failed && value != null) {
                skipSpaces();
                if (!peek('*') && !peek('/')) return value;
                char operator = text.charAt(position++);
                BigDecimal right = factor();
                if (right == null) return null;
                if (operator == '*') {
                    value = value.multiply(right);
                } else {
                    if (right.signum() == 0) {
                        failed = true;
                        return null;
                    }
                    value = value.divide(right, DIVISION);
                }
                if (value.abs().compareTo(INTERMEDIATE_LIMIT) > 0) {
                    failed = true;
                    return null;
                }
                operations++;
            }
            return value;
        }

        private BigDecimal factor() {
            skipSpaces();
            if (position >= text.length()) return null;
            char c = text.charAt(position);
            if (c == '-' || c == '+') {
                position++;
                BigDecimal value = factor();
                return value == null || c == '+' ? value : value.negate();
            }
            if (c == '(') {
                position++;
                BigDecimal value = expression();
                skipSpaces();
                if (value == null || !peek(')')) return null;
                position++;
                return value;
            }
            return number();
        }

        private BigDecimal number() {
            int start = position;
            int dots = 0;
            while (position < text.length()) {
                char c = text.charAt(position);
                if (c == '.') dots++;
                else if (c < '0' || c > '9') break;
                position++;
            }
            String literal = text.substring(start, position);
            if (literal.isEmpty() || dots > 1 || literal.equals(".")) return null;
            return new BigDecimal(literal.startsWith(".") ? "0" + literal : literal.endsWith(".") ? literal + "0" : literal);
        }

        private boolean peek(char expected) {
            return position < text.length() && text.charAt(position) == expected;
        }

        private void skipSpaces() {
            while (position < text.length() && text.charAt(position) == ' ') position++;
        }
    }
}
