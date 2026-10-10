import app.msime.android.EmailSuffixPolicy;

import java.util.List;

/** #6147：邮箱输入框里打到 `xxx@` 时候选栏给出的邮箱后缀。 */
public final class EmailSuffixPolicySmoke {
    public static void main(String[] args) {
        List<String> all = EmailSuffixPolicy.SUFFIXES;
        check(all.size() == 20 && all.size() == new java.util.HashSet<>(all).size(), "twenty distinct suffixes");
        check(all.subList(0, 7).equals(List.of("@qq.com", "@163.com", "@126.com", "@foxmail.com", "@139.com",
            "@189.cn", "@aliyun.com")), "domestic suffixes come first");
        for (String suffix : all)
            check(suffix.startsWith("@") && suffix.indexOf('@', 1) < 0 && suffix.equals(suffix.toLowerCase()),
                suffix + " is one lowercase @domain");

        EmailSuffixPolicy.Match bare = EmailSuffixPolicy.match("abc@");
        check(bare != null && bare.typedDomain().isEmpty() && bare.suffixes().equals(all), "abc@ offers every suffix");
        EmailSuffixPolicy.Match digit = EmailSuffixPolicy.match("abc@1");
        check(digit != null && digit.suffixes().equals(List.of("@163.com", "@126.com", "@139.com", "@189.cn")),
            "abc@1 narrows to the numeric domains");
        EmailSuffixPolicy.Match yahoo = EmailSuffixPolicy.match("abc@yahoo.c");
        check(yahoo != null && yahoo.suffixes().equals(List.of("@yahoo.com", "@yahoo.co.jp")), "abc@yahoo.c offers both");
        EmailSuffixPolicy.Match gm = EmailSuffixPolicy.match("abc@gm");
        check(gm != null && gm.suffixes().equals(List.of("@gmail.com", "@gmx.com")), "abc@gm offers gmail and gmx");
        EmailSuffixPolicy.Match upper = EmailSuffixPolicy.match("Abc@GMa");
        check(upper != null && upper.typedDomain().equals("GMa") && upper.suffixes().equals(List.of("@gmail.com")),
            "typed domains match without case");
        check(EmailSuffixPolicy.match("abc@163.com") == null, "a finished suffix offers nothing");
        check(EmailSuffixPolicy.match("abc@example.org") == null, "an unknown domain offers nothing");
        check(EmailSuffixPolicy.match("a.b_c+d-e%f@") != null, "the local part may carry ._%+-");
        check(EmailSuffixPolicy.match("x@y@").typedDomain().isEmpty(), "the last @ counts");
        check(EmailSuffixPolicy.match("我的邮箱 hi@") != null, "only the characters next to @ need to be ASCII");

        check(EmailSuffixPolicy.match("@") == null, "a lone @ is not an address");
        check(EmailSuffixPolicy.match("微信@") == null, "a Chinese mention is not an address");
        check(EmailSuffixPolicy.match("a b @") == null, "a space before @ is not an address");
        check(EmailSuffixPolicy.match("a@@") == null, "@@ is not an address");
        check(EmailSuffixPolicy.match("abc＠") == null, "a full-width ＠ does not count");
        check(EmailSuffixPolicy.match("abc@ ") == null, "the caret must sit right after the domain");
        check(EmailSuffixPolicy.match("") == null && EmailSuffixPolicy.match(null) == null, "no text, no suffixes");

        EmailSuffixPolicy.Replacement fresh = EmailSuffixPolicy.replacement(bare, "@163.com");
        check(fresh != null && fresh.deleteCount() == 0 && fresh.insert().equals("163.com"), "abc@ completes to abc@163.com");
        EmailSuffixPolicy.Replacement partial = EmailSuffixPolicy.replacement(upper, "@gmail.com");
        check(partial != null && partial.deleteCount() == 3 && partial.insert().equals("gmail.com"),
            "a typed GMa is replaced by the lowercase domain");
        check(EmailSuffixPolicy.replacement(digit, "@gmail.com") == null, "a suffix outside the match is refused");
        check(EmailSuffixPolicy.replacement(null, "@163.com") == null, "no match, no replacement");
        System.out.println("Android email suffixes: ordering, matching after @, and completion passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
