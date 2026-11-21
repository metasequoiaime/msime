import app.msime.android.ChineseSymbolFaces;
import app.msime.android.EighteenKeyLayout;
import app.msime.android.FourteenKeyLayout;
import app.msime.android.GridLetterHintTable;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceSlot;
import app.msime.android.LetterHintTable;
import app.msime.android.LetterKeyFaceDomains;
import java.util.List;

/** 字母键键组的内容域：按设置列出可能出现的键面和角标，关着角标时不为角标留位置。 */
public final class LetterKeyFaceDomainsSmoke {
    private LetterKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        ChineseSymbolFaces.Marks marks = ChineseSymbolFaces.Marks.DEFAULT;
        KeyFaceDomain letters = LetterKeyFaceDomains.letters(true, marks);
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            check(letters.contains(KeyFaceSlot.MAIN, lower)
                && letters.contains(KeyFaceSlot.MAIN, lower.toUpperCase(java.util.Locale.ROOT)), "缺字母键面 " + lower);
            for (boolean english : new boolean[] {false, true})
                for (boolean chinese : new boolean[] {false, true})
                    check(letters.contains(KeyFaceSlot.TOP_RIGHT, LetterHintTable.face(lower, english, chinese, marks)),
                        "缺角标 " + lower + " english=" + english + " chinese=" + chinese);
        }
        check(letters.contains(KeyFaceSlot.MAIN, ";"), "缺微软双拼的 ;");
        check(!letters.has(KeyFaceSlot.BOTTOM), "双拼提示由键补进键组，内容域里不预先列出");
        check(!LetterKeyFaceDomains.letters(false, marks).has(KeyFaceSlot.TOP_RIGHT), "关着角标时不应为角标留位置");

        // 直角引号样式下 " 的角标是『，内容域跟着样式走。
        ChineseSymbolFaces.Marks corner = ChineseSymbolFaces.Marks.of("corner", "hollow");
        KeyFaceDomain cornerQuotes = LetterKeyFaceDomains.letters(true, corner);
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String face = LetterHintTable.face(String.valueOf(letter), false, true, corner);
            check(cornerQuotes.contains(KeyFaceSlot.TOP_RIGHT, face), "引号样式的角标没进内容域：" + face);
        }

        for (boolean eighteen : new boolean[] {false, true}) {
            KeyFaceDomain grid = LetterKeyFaceDomains.gridLetters(eighteen, true, marks);
            List<List<FourteenKeyLayout.Key>> rows = eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows();
            for (List<FourteenKeyLayout.Key> row : rows) {
                for (FourteenKeyLayout.Key key : row) {
                    for (boolean shuangpin : new boolean[] {false, true})
                        check(grid.contains(KeyFaceSlot.MAIN, FourteenKeyLayout.face(key, shuangpin)),
                            "缺 " + (eighteen ? 18 : 14) + " 键键面 " + FourteenKeyLayout.face(key, shuangpin));
                    for (boolean chinese : new boolean[] {false, true})
                        check(grid.contains(KeyFaceSlot.TOP_RIGHT, GridLetterHintTable.face(key, eighteen, chinese, marks)),
                            "缺 " + (eighteen ? 18 : 14) + " 键角标");
                }
            }
            check(!LetterKeyFaceDomains.gridLetters(eighteen, false, marks).has(KeyFaceSlot.TOP_RIGHT),
                "关着角标时不应为网格角标留位置");
        }
        System.out.println("LetterKeyFaceDomainsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
