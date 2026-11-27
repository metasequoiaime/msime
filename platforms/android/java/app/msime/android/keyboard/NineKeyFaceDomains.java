package app.msime.android;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 九键键组的内容域（见 {@link KeyFaceDomain}）：拼音九键字母层、数字九宫格和九键双拼的数字键上可能画的全部文字。只取决于传进来的表，不读服务，所以 JVM 冒烟可以直接检查。
 */
public final class NineKeyFaceDomains {
    private NineKeyFaceDomains() { }

    /**
     * 拼音九键字母层：九格的键面（1 键空闲时是「@#」，组字时按设置是「分词」或「重输」）、右列的 ！，右上角是长按输入的数字 1–9 和 ！ 上滑滑出的 0。
     */
    public static KeyFaceDomain letters() {
        List<String> faces = new ArrayList<>();
        List<String> digits = new ArrayList<>();
        for (List<NineKeyLayout.Key> row : NineKeyLayout.rows()) {
            for (NineKeyLayout.Key key : row) {
                faces.add(NineKeyLayout.face(key, false));
                for (String action : List.of(NineKeyCompositionActionPolicy.SEPARATE, NineKeyCompositionActionPolicy.REWRITE))
                    faces.add(NineKeyLayout.face(key, false, true, action));
                digits.add(NineKeyLayout.digitInput(key));
            }
        }
        faces.add(exclamation());
        digits.add(SuperscriptShortcutPolicy.ZERO);
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        texts.put(KeyFaceSlot.TOP_RIGHT, digits);
        return new KeyFaceDomain(texts);
    }

    /** 数字九宫格：1–9、底行的 0、右列的小数点和 ！ 的各种写法（{@link NumberPadPunctuation}）。 */
    public static KeyFaceDomain digits() {
        List<String> faces = new ArrayList<>();
        for (List<NineKeyLayout.Key> row : NineKeyLayout.rows())
            for (NineKeyLayout.Key key : row) faces.add(NineKeyLayout.face(key, true));
        faces.add("0");
        faces.add(".");
        for (NumberPadPunctuation.Style style : NumberPadPunctuation.Style.values())
            faces.add(NumberPadPunctuation.face(exclamation(), style));
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        return new KeyFaceDomain(texts);
    }

    /** 九键双拼：数字 0–9 作主标签，底部是当前表里每个数字的简短声韵提示（{@link NineKeyShuangpinKeypad#compactHint}）。 */
    public static KeyFaceDomain shuangpin(List<NineKeyShuangpinKeypad.Face> faces) {
        List<String> digits = new ArrayList<>();
        for (char digit = '0'; digit <= '9'; digit++) digits.add(String.valueOf(digit));
        List<String> hints = new ArrayList<>();
        for (NineKeyShuangpinKeypad.Face face : faces) hints.add(NineKeyShuangpinKeypad.compactHint(face));
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, digits);
        texts.put(KeyFaceSlot.BOTTOM, hints);
        return new KeyFaceDomain(texts);
    }

    /** 右列的 ！：九键标点表的最后一个。 */
    private static String exclamation() {
        List<String> punctuation = NineKeyLayout.punctuation();
        return punctuation.get(punctuation.size() - 1);
    }
}
