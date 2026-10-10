import app.msime.android.FourteenKeyLayout;
import app.msime.android.KeyPressIds;
import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.KeyboardScheme;
import app.msime.android.NineKeyLayout;
import app.msime.android.NineKeyPanelPolicy;
import app.msime.android.SplitKeyboardPolicy;
import app.msime.android.TypingSource;
import java.util.ArrayList;
import java.util.List;

/** 全拼 14 键的键表、组码、键面文字与各项门控：与 iOS、HarmonyOS 的 `FourteenKeyLayout` 和引擎 `KeyGrid::FourteenKey` 是同一张表。 */
public final class FourteenKeyLayoutSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        table();
        faces();
        gates();
        System.out.println("Android fourteen-key layout: key table, group codes, faces, labels, hold letters, key ids and gates passed");
    }

    private static void table() {
        List<String> rows = new ArrayList<>();
        for (List<FourteenKeyLayout.Key> row : FourteenKeyLayout.rows()) {
            StringBuilder line = new StringBuilder();
            for (FourteenKeyLayout.Key key : row) {
                if (line.length() > 0) line.append(' ');
                line.append(FourteenKeyLayout.face(key));
            }
            rows.add(line.toString());
        }
        check(rows.equals(List.of("QW ER TY UI OP", "AS DF GH JK L", "ZX CV BN M")),
            "the rows are QW ER TY UI OP / AS DF GH JK L / ZX CV BN M");
        // 由键表逐字母生成的编码表必须等于引擎的 `abcdedggujjlmbooqeatucqztz`：每个字母落在它那一组的首字母上。
        char[] generated = new char[26];
        StringBuilder codes = new StringBuilder();
        int keys = 0;
        for (List<FourteenKeyLayout.Key> row : FourteenKeyLayout.rows()) {
            for (FourteenKeyLayout.Key key : row) {
                keys++;
                codes.append(key.input());
                for (char letter : key.letters().toCharArray()) {
                    check(generated[letter - 'a'] == 0, "each letter is on one key: " + letter);
                    generated[letter - 'a'] = key.input();
                }
            }
        }
        check(keys == 14, "fourteen keys");
        check(new String(generated).equals(FourteenKeyLayout.GROUP_CODES)
            && FourteenKeyLayout.GROUP_CODES.equals("abcdedggujjlmbooqeatucqztz"), "the generated table is the engine's");
        check(codes.toString().equals("qetuoadgjlzcbm"), "the group codes are each group's first letter");
        for (char letter = 'a'; letter <= 'z'; letter++) {
            check(FourteenKeyLayout.groupCode(letter) == generated[letter - 'a'], "group code of " + letter);
            check(FourteenKeyLayout.groupCode(Character.toUpperCase(letter)) == generated[letter - 'a'],
                "group code of upper " + letter);
        }
        check(FourteenKeyLayout.groupCode('1') == 0 && FourteenKeyLayout.groupCode('\'') == 0,
            "no group for a non-letter");
    }

    private static void faces() {
        FourteenKeyLayout.Key qw = FourteenKeyLayout.rows().get(0).get(0);
        FourteenKeyLayout.Key l = FourteenKeyLayout.rows().get(1).get(4);
        FourteenKeyLayout.Key m = FourteenKeyLayout.rows().get(2).get(3);
        check(qw.input() == 'q' && FourteenKeyLayout.face(qw).equals("QW"), "QW sends q and shows QW");
        check(FourteenKeyLayout.accessibilityLabel(qw).equals("按键 Q W")
            && FourteenKeyLayout.description(qw).equals("Q W"), "a pair reads 按键 Q W");
        check(FourteenKeyLayout.accessibilityLabel(l).equals("字母 L")
            && FourteenKeyLayout.accessibilityLabel(m).equals("字母 M"), "a single key reads 字母 L");
        // 长按：两字母键弹出这两个小写字母（没有数字，没有原样项），L、M 不弹。
        check(FourteenKeyLayout.holdLetters(qw).equals("qw"), "QW holds q and w");
        check(FourteenKeyLayout.holdLetters(l).isEmpty() && FourteenKeyLayout.holdLetters(m).isEmpty(),
            "L and M do not hold");
        // 键位 id 与 client-core KEY_IDS 逐字一致，不记到 26 键的 KeyQ 上。
        List<String> ids = new ArrayList<>();
        for (List<FourteenKeyLayout.Key> row : FourteenKeyLayout.rows())
            for (FourteenKeyLayout.Key key : row) ids.add(FourteenKeyLayout.keyId(key));
        check(ids.equals(List.of("FourteenQW", "FourteenER", "FourteenTY", "FourteenUI", "FourteenOP",
            "FourteenAS", "FourteenDF", "FourteenGH", "FourteenJK", "FourteenL", "FourteenZX", "FourteenCV",
            "FourteenBN", "FourteenM")), "key ids follow the faces");
        for (String id : ids) check(KeyPressIds.isKnown(id), "the store knows " + id);
        // 左端键照搬九键 1 键：空闲时「符」打开符号面板，组字时「分词」。
        check(FourteenKeyLayout.separatorFace(false).equals("符")
            && FourteenKeyLayout.separatorFace(true).equals("分词"), "the separator key faces");
        check(FourteenKeyLayout.separatorDescription(true).equals(NineKeyLayout.description(
            NineKeyLayout.rows().get(0).get(0), false, true)), "the separator reads as nine-key's 1 key");
        check(FourteenKeyLayout.LETTER_WEIGHT * 5 == 10f, "five pairs are as wide as ten letters");
    }

    private static void gates() {
        int fourteen = KeyboardLayout.FOURTEEN_KEY_LAYOUT;
        check(fourteen == 8, "the surface constant is appended");
        // 14 键看引擎的网格，排在九键之前；本地模式和其他方案下由调用方传 false，回落 26 键。
        check(KeyboardLayout.resolveTouchLayout(false, false, true, 0, "fourteen_key") == fourteen,
            "the fourteen-key grid draws the fourteen-key surface");
        check(KeyboardLayout.resolveTouchLayout(false, false, false, 0, "fourteen_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT, "without the grid (local mode) it is 26 keys");
        check(KeyboardLayout.resolveTouchLayout(true, false, false, 0, "handwriting")
            == KeyboardLayout.HANDWRITING_LAYOUT, "handwriting is untouched");
        check(KeyboardLayout.resolveTouchLayout(false, true, false, 0, "nine_key")
            == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, "nine-key is untouched");
        // 关闭：Shift、数字行、分体（滑行只在标准 26 键上，见 ImeGlideTyping）。
        check(!KeyboardLayout.carriesLetterCase(fourteen), "no Shift");
        check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, fourteen, 760), "no number row");
        check(!SplitKeyboardPolicy.drawn(true, 800, true, fourteen), "never split");
        // 底行与 26 键相同：123 ， 空格 。 中 回车。
        check(KeyboardActionRow.designEntries(fourteen, false).equals(
            KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false))
            && KeyboardActionRow.designEntries(fourteen, true).equals(
                KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the bottom row is the 26-key one");
        check(KeyboardActionRow.layerTitle(fourteen, false).equals("123")
            && KeyboardActionRow.layerTitle(fourteen, true).equals("14键"), "the layer key titles");
        // 123 跟随 `touch_twenty_six_key_number_layout`，分体时不借九宫格（14 键不分体，这里只是同一条规则）。
        check(NineKeyLayout.twentySixKeyDigits(fourteen, true, true, false), "the 9-grid digit face is borrowed");
        check(!NineKeyLayout.twentySixKeyDigits(fourteen, true, false, false), "the row 123 layer by default");
        check(!NineKeyLayout.twentySixKeyDigits(fourteen, false, true, false), "letters stay letters");
        // 三栏面板与九键共用：按 14 键的布局判断，字母层组字时画三栏，123 层不画。
        check(KeyboardLayout.drawsKeyGrid(fourteen), "the 14 keys are a key-grid face");
        check(NineKeyPanelPolicy.threeColumn(fourteen, true, false, true), "the three-column panel opens while composing");
        check(!NineKeyPanelPolicy.threeColumn(fourteen, false, false, true), "the 123 layer keeps the candidate grid");
        // 方案与统计。
        check(KeyboardScheme.QUANPIN_FOURTEEN_KEY.touchKeyboardLayout().equals("fourteen_key")
            && KeyboardScheme.QUANPIN_FOURTEEN_KEY.preferenceId().equals("fourteen_key")
            && KeyboardScheme.QUANPIN_FOURTEEN_KEY.optIn(), "the scheme is fourteen_key and opt-in");
        check(TypingSource.resolve(KeyboardScheme.QUANPIN_FOURTEEN_KEY, false, "none") == TypingSource.FOURTEEN_KEY
            && TypingSource.FOURTEEN_KEY.id().equals("fourteenKey"), "the source is fourteenKey");
        check(TypingSource.resolve(KeyboardScheme.QUANPIN_FOURTEEN_KEY, true, "none") == TypingSource.ENGLISH,
            "English on the fourteen-key scheme is English");
    }
}
