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
     * 26 键（和同一套字母行的其他语言）的字母键：26 个字母的大小写和微软双拼的 `;`；开着「显示字母键角标」时，加上每个字母在中英文、中文标点开关下的全部角标。双拼提示不在这里，随方案画上时由键自己补进键组。
     */
    public static KeyFaceDomain letters(boolean corners, ChineseSymbolFaces.Marks marks) {
        List<String> faces = new ArrayList<>();
        List<String> hints = new ArrayList<>();
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            faces.add(lower);
            faces.add(lower.toUpperCase(Locale.ROOT));
            if (!corners) continue;
            for (boolean english : new boolean[] {false, true})
                for (boolean chinese : new boolean[] {false, true})
                    hints.add(LetterHintTable.face(lower, english, chinese, marks));
        }
        faces.add(";");
        return domain(faces, hints);
    }

    /** 14 或 18 键每个字母键在全拼、双拼下的键面；开着角标时加上中文标点开关下的角标。 */
    public static KeyFaceDomain gridLetters(boolean eighteen, boolean corners, ChineseSymbolFaces.Marks marks) {
        List<String> faces = new ArrayList<>();
        List<String> hints = new ArrayList<>();
        for (List<FourteenKeyLayout.Key> row : eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows()) {
            for (FourteenKeyLayout.Key key : row) {
                faces.add(FourteenKeyLayout.face(key, false));
                faces.add(FourteenKeyLayout.face(key, true));
                if (!corners) continue;
                for (boolean chinese : new boolean[] {false, true})
                    hints.add(GridLetterHintTable.face(key, eighteen, chinese, marks));
            }
        }
        return domain(faces, hints);
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
