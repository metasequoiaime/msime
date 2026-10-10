package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 邮箱后缀（#6147）：符号面板「网络」分类里的后缀表，以及邮箱输入框里打到 `xxx@` 时候选栏给出的后缀。
 *
 * <p>只看光标前的文字：末尾要是 `<本地部分>@<已打的域名>`，本地部分至少一个 ASCII 字母、数字或 `._%+-`，紧挨着 `@`；已打的域名可以为空，只含 ASCII 字母、数字、`.` 和 `-`。以最后一个 `@` 为准。给出以已打域名开头（不分大小写）的后缀；已打的域名已经是一个完整后缀时不再给。全角的 `＠` 不算。是否只在邮箱输入框里出现由宿主决定（{@link EditorPolicy#emailAddress}）。
 */
public final class EmailSuffixPolicy {
    /** 最多看光标前多少个字符；也是调用方向编辑器要上文时的长度。 */
    public static final int MAX_CONTEXT = 64;

    /** 常用邮箱后缀：国内常用的在前，然后是海外的。符号面板「网络」分类和候选栏用同一份、同一个顺序。 */
    public static final List<String> SUFFIXES = List.of(
        "@qq.com", "@163.com", "@126.com", "@foxmail.com", "@139.com", "@189.cn", "@aliyun.com",
        "@gmail.com", "@outlook.com", "@hotmail.com", "@live.com", "@icloud.com", "@yahoo.com",
        "@proton.me", "@protonmail.com", "@aol.com", "@mail.com", "@gmx.com", "@naver.com", "@yahoo.co.jp");

    /**
     * 光标前已经打到 `@` 之后的一段。
     *
     * @param typedDomain `@` 之后已经打的域名（原样，可能为空）
     * @param suffixes 以它开头的后缀，按 {@link #SUFFIXES} 的顺序，至少一个
     */
    public record Match(String typedDomain, List<String> suffixes) {
        public Match {
            suffixes = List.copyOf(suffixes);
        }
    }

    /**
     * 选中一个后缀时对编辑器做的事：先删掉光标前 {@code deleteCount} 个字符（已经打的域名），再上屏 {@code insert}（后缀去掉 `@`，统一小写）。
     */
    public record Replacement(int deleteCount, String insert) {}

    private EmailSuffixPolicy() {}

    /** 光标前的文字末尾是不是一个打到 `@` 的邮箱；不是或没有可给的后缀时返回 null。 */
    public static Match match(CharSequence beforeCursor) {
        if (beforeCursor == null) return null;
        int end = beforeCursor.length();
        int at = end;
        while (at > 0 && domainChar(beforeCursor.charAt(at - 1))) at--;
        if (at == 0 || beforeCursor.charAt(at - 1) != '@') return null;
        int atIndex = at - 1;
        if (atIndex == 0 || !localChar(beforeCursor.charAt(atIndex - 1))) return null;
        String typed = beforeCursor.subSequence(at, end).toString();
        String needle = "@" + typed.toLowerCase(Locale.ROOT);
        List<String> suffixes = new ArrayList<>(SUFFIXES.size());
        for (String suffix : SUFFIXES) {
            if (suffix.equals(needle)) return null;
            if (suffix.startsWith(needle)) suffixes.add(suffix);
        }
        return suffixes.isEmpty() ? null : new Match(typed, suffixes);
    }

    /** 在 {@code match} 处补全成 {@code suffix}；后缀不在这次的匹配里时返回 null。 */
    public static Replacement replacement(Match match, String suffix) {
        if (match == null || suffix == null || !match.suffixes().contains(suffix)) return null;
        return new Replacement(match.typedDomain().length(), suffix.substring(1));
    }

    private static boolean domainChar(char value) {
        return asciiLetterOrDigit(value) || value == '.' || value == '-';
    }

    private static boolean localChar(char value) {
        return asciiLetterOrDigit(value) || value == '.' || value == '_' || value == '%' || value == '+' || value == '-';
    }

    private static boolean asciiLetterOrDigit(char value) {
        return (value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z') || (value >= '0' && value <= '9');
    }
}
