import app.msime.android.ChineseOutputPolicy;

public final class ChineseOutputPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(ChineseOutputPolicy.applies(false, 0, "none"));
        check(ChineseOutputPolicy.applies(false, 2, "unicode"));
        check(!ChineseOutputPolicy.applies(true, 0, "none"));
        check(!ChineseOutputPolicy.applies(false, 3, "none"));
        check(!ChineseOutputPolicy.applies(false, 4, "none"));
        check(!ChineseOutputPolicy.applies(false, 0, "temporary_japanese"));
        // 粤拼和注音本来就写繁体字，越南语和藏文不是中文；本宿主不认识的方案序号沿用原来的答案。
        check(ChineseOutputPolicy.applies(false, 1, "none"));
        check(!ChineseOutputPolicy.applies(false, 5, "none"));
        check(!ChineseOutputPolicy.applies(false, 6, "none"));
        check(!ChineseOutputPolicy.applies(false, 7, "none"));
        check(!ChineseOutputPolicy.applies(false, 8, "none"));
        check(ChineseOutputPolicy.applies(false, 9, "none"));
        check(ChineseOutputPolicy.applies(false, -1, "none"));

        // The shared tables are phrase-level, which is the reason this host stopped converting one
        // character at a time: whether 发 is 發 or 髮 is a property of the word. A per-character
        // converter turns 头发 into 頭發, and nothing about the result says it went wrong.
        ChineseOutputPolicy.Converter phrase = text ->
            text.replace("头发", "頭髮").replace("发展", "發展");
        check(ChineseOutputPolicy.output("头发", true, true, phrase).equals("頭髮"));
        check(ChineseOutputPolicy.output("发展", true, true, phrase).equals("發展"));

        ChineseOutputPolicy.Converter sample = text -> text.replace("输入", "輸入");
        check(ChineseOutputPolicy.output("水杉输入法", false, true, sample).equals("水杉输入法"));
        check(ChineseOutputPolicy.output("水杉输入法", true, false, sample).equals("水杉输入法"));
        check(ChineseOutputPolicy.output("水杉输入法", true, true, sample).equals("水杉輸入法"));
        check(ChineseOutputPolicy.output("", true, true, sample).isEmpty());
        check(ChineseOutputPolicy.output("输入法", true, true, text -> null).equals("输入法"));
        check(ChineseOutputPolicy.output("输入法", true, true, text -> {
            throw new IllegalArgumentException();
        }).equals("输入法"));
        System.out.println("Android Chinese output: boundaries and conversion failure fallback passed");
    }
}
