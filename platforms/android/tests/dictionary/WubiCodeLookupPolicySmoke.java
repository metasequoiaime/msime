import app.msime.android.WubiCodeLookupPolicy;
import java.util.List;

/** 五笔查码页的排法与文字：整段词在前、两列编码、当前码表标「（当前）」、没有编码写「—」，与共享设置页一致（#6789）。 */
public final class WubiCodeLookupPolicySmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        WubiCodeLookupPolicy.Codes phrase = new WubiCodeLookupPolicy.Codes("你好", List.of("wqvb"), List.of());
        WubiCodeLookupPolicy.Codes ni = new WubiCodeLookupPolicy.Codes("你", List.of("wq", "wqiy"), List.of("wq", "wqiy"));
        WubiCodeLookupPolicy.Codes duan = new WubiCodeLookupPolicy.Codes("段", List.of("wdm", "wdmc"), List.of("th", "thdc"));

        List<WubiCodeLookupPolicy.Codes> rows = WubiCodeLookupPolicy.rows(phrase, List.of(ni, duan));
        check(rows.equals(List.of(phrase, ni, duan)), "the phrase comes first, then each character");
        check(WubiCodeLookupPolicy.rows(null, List.of(ni)).equals(List.of(ni)), "no phrase row without a phrase");

        check(WubiCodeLookupPolicy.columnTitle("wubi86", "wubi86").equals("86 五笔（当前）"), "86 marked current");
        check(WubiCodeLookupPolicy.columnTitle("wubi98", "wubi86").equals("98 五笔"), "98 not current");
        check(WubiCodeLookupPolicy.columnTitle("wubi98", "wubi98").equals("98 五笔（当前）"), "98 marked current");
        check(WubiCodeLookupPolicy.columnTitle("wubi86", "wubi06").equals("86 五笔（当前）"),
            "an unknown profile reads as 86, as the keyboard does");

        check(WubiCodeLookupPolicy.codes(List.of()).equals("—"), "a missing code is a dash");
        check(WubiCodeLookupPolicy.columns(duan, "wubi98").equals("86 五笔  wdm wdmc\n98 五笔（当前）  th thdc"),
            "two columns, short codes first");
        check(WubiCodeLookupPolicy.columns(phrase, "wubi86").equals("86 五笔（当前）  wqvb\n98 五笔  —"),
            "a phrase only one table has");
        check(WubiCodeLookupPolicy.description(ni, "wubi86").equals("你，86 五笔（当前） wq wqiy，98 五笔 wq wqiy"),
            "the row is read whole");

        check(!WubiCodeLookupPolicy.searchable("  \n"), "blank text is not looked up");
        check(WubiCodeLookupPolicy.searchable(" 段 "), "text with a character is looked up");
        check(WubiCodeLookupPolicy.truncated(64).equals("字太多，只列出了前 64 个不同的字。"), "truncation note");
        System.out.println("Android Wubi code lookup presentation passed");
    }
}
