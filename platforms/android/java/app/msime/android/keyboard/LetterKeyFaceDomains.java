package app.msime.android;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * 字母键键组的内容域：按当前设置可能出现在字母键上的全部文字（见 {@link KeyFaceDomain}）。只取决于传进来的几项设置，不读服务，所以 JVM 冒烟可以直接检查。
 */
public final class LetterKeyFaceDomains {
    private LetterKeyFaceDomains() { }

    /**
     * 26 键（和同一套字母行的其他语言）的字母键：26 个字母的大小写和微软双拼的 `;`；开着「显示字母键角标」时，加上每个字母在中英文、日语、中文标点开关下的全部角标（{@link LetterHintTable#face(String, boolean, boolean, boolean, ChineseSymbolFaces.Marks)}）；开着双拼提示时，底部是当前方案的全部提示（{@link KeyFaceLayout#shuangpinHints}）。
     */
    public static KeyFaceDomain letters(KeyFaceLayout layout) {
        List<String> faces = new ArrayList<>();
        List<String> hints = new ArrayList<>();
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            faces.add(lower);
            faces.add(lower.toUpperCase(Locale.ROOT));
            if (!layout.corners()) continue;
            for (boolean english : new boolean[] {false, true})
                for (boolean japanese : new boolean[] {false, true})
                    for (boolean chinese : new boolean[] {false, true})
                        hints.add(LetterHintTable.face(lower, english, japanese, chinese, layout.marks()));
        }
        faces.add(";");
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        texts.put(KeyFaceSlot.TOP_RIGHT, hints);
        texts.put(KeyFaceSlot.BOTTOM, layout.shuangpinHints());
        return new KeyFaceDomain(texts);
    }

    /** 14 或 18 键每个字母键在全拼、双拼下的键面，两个字母的键横滑选中其中一个时键面上的那个大写字母（{@link EighteenKeySwipePolicy#letter}）；开着角标时加上中文标点开关下的角标。 */
    public static KeyFaceDomain gridLetters(boolean eighteen, KeyFaceLayout layout) {
        List<String> faces = new ArrayList<>();
        List<String> hints = new ArrayList<>();
        for (List<FourteenKeyLayout.Key> row : eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows()) {
            for (FourteenKeyLayout.Key key : row) {
                faces.add(FourteenKeyLayout.face(key, false));
                faces.add(FourteenKeyLayout.face(key, true));
                if (key.letters().length() == 2)
                    for (char letter : key.letters().toCharArray()) faces.add(String.valueOf(Character.toUpperCase(letter)));
                if (!layout.corners()) continue;
                for (boolean chinese : new boolean[] {false, true})
                    hints.add(GridLetterHintTable.face(key, eighteen, chinese, layout.marks()));
            }
        }
        return domain(faces, hints);
    }

    /** 韩文字母键在不按和按住双辅音时的全部字母（{@link KoreanKeyboardLayout#face}）：按双辅音只重画，不重量。 */
    public static KeyFaceDomain korean() {
        List<String> faces = new ArrayList<>();
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String key = String.valueOf(letter);
            faces.add(KoreanKeyboardLayout.face(key, false));
            faces.add(KoreanKeyboardLayout.face(key, true));
        }
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        return new KeyFaceDomain(texts);
    }

    /** 大千字母层的全部注音符号和声调（{@link ZhuyinKeyboardLayout#rows}）。 */
    public static KeyFaceDomain dachen() {
        List<String> faces = new ArrayList<>();
        for (List<String> row : ZhuyinKeyboardLayout.rows()) for (String key : row) faces.add(ZhuyinKeyboardLayout.face(key));
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        return new KeyFaceDomain(texts);
    }

    /** 字母上方的数字行：1–0（{@link KeyboardLayout#NUMBER_ROW}），没有提示。 */
    public static KeyFaceDomain numberRow() {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, KeyboardLayout.NUMBER_ROW);
        return new KeyFaceDomain(texts);
    }

    private static KeyFaceDomain domain(List<String> faces, List<String> hints) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        texts.put(KeyFaceSlot.TOP_RIGHT, hints);
        return new KeyFaceDomain(texts);
    }
}
