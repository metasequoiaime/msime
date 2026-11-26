import app.msime.android.ChineseSymbolFaces;
import app.msime.android.EighteenKeyLayout;
import app.msime.android.EighteenKeySwipePolicy;
import app.msime.android.FourteenKeyLayout;
import app.msime.android.GridLetterHintTable;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceSlot;
import app.msime.android.KeyboardLayout;
import app.msime.android.LetterHintTable;
import app.msime.android.KeyFaceLayout;
import app.msime.android.LetterKeyFaceDomains;
import app.msime.android.NineKeyCompositionActionPolicy;
import java.util.List;

/** 字母键键组的内容域：按设置列出可能出现的键面和角标，关着角标时不为角标留位置。 */
public final class LetterKeyFaceDomainsSmoke {
    private LetterKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        ChineseSymbolFaces.Marks marks = ChineseSymbolFaces.Marks.DEFAULT;
        KeyFaceDomain letters = LetterKeyFaceDomains.letters(layout(true, marks, List.of()));
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            check(letters.contains(KeyFaceSlot.MAIN, lower)
                && letters.contains(KeyFaceSlot.MAIN, lower.toUpperCase(java.util.Locale.ROOT)), "缺字母键面 " + lower);
            for (boolean english : new boolean[] {false, true})
                for (boolean japanese : new boolean[] {false, true})
                    for (boolean chinese : new boolean[] {false, true})
                        check(letters.contains(KeyFaceSlot.TOP_RIGHT, LetterHintTable.face(lower, english, japanese, chinese, marks)),
                            "缺角标 " + lower + " english=" + english + " japanese=" + japanese + " chinese=" + chinese);
        }
        check(letters.contains(KeyFaceSlot.MAIN, ";"), "缺微软双拼的 ;");
        check(!letters.has(KeyFaceSlot.BOTTOM), "没有双拼提示表时不应为底部提示留位置");
        check(!LetterKeyFaceDomains.letters(layout(false, marks, List.of())).has(KeyFaceSlot.TOP_RIGHT), "关着角标时不应为角标留位置");
        // 双拼提示按布局里的提示表列出，不等键画上时再补。
        KeyFaceDomain hinted = LetterKeyFaceDomains.letters(layout(false, marks, List.of("iu", "uang")));
        check(hinted.contains(KeyFaceSlot.BOTTOM, "iu") && hinted.contains(KeyFaceSlot.BOTTOM, "uang"), "双拼提示表应进底部的内容域");

        // 直角引号样式下 " 的角标是『，内容域跟着样式走。
        ChineseSymbolFaces.Marks corner = ChineseSymbolFaces.Marks.of("corner", "hollow");
        KeyFaceDomain cornerQuotes = LetterKeyFaceDomains.letters(layout(true, corner, List.of()));
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String face = LetterHintTable.face(String.valueOf(letter), false, true, corner);
            check(cornerQuotes.contains(KeyFaceSlot.TOP_RIGHT, face), "引号样式的角标没进内容域：" + face);
        }

        for (boolean eighteen : new boolean[] {false, true}) {
            KeyFaceDomain grid = LetterKeyFaceDomains.gridLetters(eighteen, layout(true, marks, List.of()));
            List<List<FourteenKeyLayout.Key>> rows = eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows();
            for (List<FourteenKeyLayout.Key> row : rows) {
                for (FourteenKeyLayout.Key key : row) {
                    for (boolean shuangpin : new boolean[] {false, true})
                        check(grid.contains(KeyFaceSlot.MAIN, FourteenKeyLayout.face(key, shuangpin)),
                            "缺 " + (eighteen ? 18 : 14) + " 键键面 " + FourteenKeyLayout.face(key, shuangpin));
                    // 横滑选中一个字母时键面换成那个字母。
                    for (float dx : new float[] {-100f, 100f}) {
                        char selected = EighteenKeySwipePolicy.letter(key.letters(), dx, 0);
                        check(selected == 0 || grid.contains(KeyFaceSlot.MAIN, String.valueOf(selected)),
                            "缺横滑选中的字母 " + selected);
                    }
                    for (boolean chinese : new boolean[] {false, true})
                        check(grid.contains(KeyFaceSlot.TOP_RIGHT, GridLetterHintTable.face(key, eighteen, chinese, marks)),
                            "缺 " + (eighteen ? 18 : 14) + " 键角标");
                }
            }
            check(!LetterKeyFaceDomains.gridLetters(eighteen, layout(false, marks, List.of())).has(KeyFaceSlot.TOP_RIGHT),
                "关着角标时不应为网格角标留位置");
        }
        KeyFaceDomain numbers = LetterKeyFaceDomains.numberRow();
        for (String digit : KeyboardLayout.NUMBER_ROW)
            check(numbers.contains(KeyFaceSlot.MAIN, digit), "缺数字行的 " + digit);
        check(!numbers.has(KeyFaceSlot.TOP_RIGHT) && !numbers.has(KeyFaceSlot.BOTTOM), "数字行没有提示");
        // 韩文：两套辅音元音都在内容域里（按双辅音只重画）；大千：每个注音符号和声调都在。
        java.util.List<String> korean = LetterKeyFaceDomains.korean().texts().get(KeyFaceSlot.MAIN);
        for (String jamo : java.util.List.of("ㅂ", "ㅃ", "ㅈ", "ㅉ", "ㅏ", "ㅡ")) check(korean.contains(jamo), "韩文内容域缺 " + jamo);
        java.util.List<String> dachen = LetterKeyFaceDomains.dachen().texts().get(KeyFaceSlot.MAIN);
        for (String face : java.util.List.of("ㄅ", "ㄤ", "ˊ", "ˇ", "ˋ", "˙")) check(dachen.contains(face), "大千内容域缺 " + face);
        System.out.println("LetterKeyFaceDomainsSmoke passed");
    }

    private static KeyFaceLayout layout(boolean corners, ChineseSymbolFaces.Marks marks, List<String> hints) {
        return new KeyFaceLayout(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true, NineKeyCompositionActionPolicy.SEPARATE, corners,
            marks, false, List.of(), hints);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
