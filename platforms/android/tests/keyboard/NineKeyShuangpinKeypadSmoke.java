import app.msime.android.NineKeyShuangpinKeypad;
import app.msime.android.NineKeyShuangpinKeypad.Face;
import java.util.List;

/** 九键双拼的键面文字、读屏文字、方案名和表的问题提示（#6572）。键位本身来自 host-api，这里只核对怎么印、怎么说。 */
public final class NineKeyShuangpinKeypadSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 米旮旯 3 的 2 键：b c 和零声母，韵母 a ai iang。
        Face two = new Face('2', List.of("b", "c"), List.of("a", "ai", "iang"), true);
        check(NineKeyShuangpinKeypad.label(two).equals("零 b c\na ai iang"), NineKeyShuangpinKeypad.label(two));
        check(NineKeyShuangpinKeypad.description(two).equals("2，声母 b c 和零声母，韵母 a ai iang"),
            NineKeyShuangpinKeypad.description(two));
        // 0 键只有韵母，1 键也是：只印一行。
        Face zero = new Face('0', List.of(), List.of("u", "ü", "ei"), false);
        check(NineKeyShuangpinKeypad.label(zero).equals("u ü ei"), NineKeyShuangpinKeypad.label(zero));
        check(NineKeyShuangpinKeypad.description(zero).equals("0，韵母 u ü ei"), NineKeyShuangpinKeypad.description(zero));
        Face eight = new Face('8', List.of("t", "zh", "sh"), List.of(), false);
        check(NineKeyShuangpinKeypad.label(eight).equals("t zh sh"), "initials only");
        check(NineKeyShuangpinKeypad.face(List.of(two, zero), '0') == zero, "lookup by key");
        check(NineKeyShuangpinKeypad.face(List.of(two, zero), '1') == null, "a key the table does not use");

        check(NineKeyShuangpinKeypad.normalizedProfile(null).equals("migala3"), "default profile");
        check(NineKeyShuangpinKeypad.normalizedProfile("gushi2").equals("migala3"), "unknown profile");
        check(NineKeyShuangpinKeypad.normalizedProfile("custom").equals("custom"), "custom profile");
        check(NineKeyShuangpinKeypad.profileTitle("gushi2_low").equals("辜氏 2 低重码"), "title");
        check(NineKeyShuangpinKeypad.BUILTIN_PROFILES.equals(List.of("migala3", "migala2", "gushi2_low")), "builtins");

        check(NineKeyShuangpinKeypad.validKey("finals", "0"), "0 is a key");
        check(!NineKeyShuangpinKeypad.validKey("finals", "22"), "a final takes one digit");
        check(NineKeyShuangpinKeypad.validKey("zero_initials", "21"), "zero initial pair");
        check(!NineKeyShuangpinKeypad.validKey("zero_initials", "2"), "zero initial takes two digits");
        check(!NineKeyShuangpinKeypad.validKey("initials", "a"), "letters are not keys");

        check(NineKeyShuangpinKeypad.unitLabel("ve").equals("üe"), "umlaut");
        check(NineKeyShuangpinKeypad.problemMessage("missing_unit", "finals", "iong", null)
            .equals("韵母「iong」还没有放到数字键上"), "missing");
        check(NineKeyShuangpinKeypad.problemMessage("invalid_key", "zero_initials", "a", "2")
            .equals("零声母「a」的键「2」不对：零声母要两位数字"), "invalid key");
        check(NineKeyShuangpinKeypad.problemMessage("duplicate_unit", "initials", "zh", null)
            .equals("声母「zh」写了两次"), "duplicate");
        System.out.println("NineKeyShuangpinKeypadSmoke ok");
    }
}
