package app.msime.android;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 九键键组的内容域（见 {@link KeyFaceDomain}）：拼音九键字母层、数字九宫格、九键双拼的数字键、笔画键盘、注音九键和日语九键（同一个九键外框）上可能画的全部文字。只取决于传进来的表，不读服务，所以 JVM 冒烟可以直接检查。
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

    /** 笔画键盘的笔画键：2×3 网格上的五种笔画和通配键。 */
    public static KeyFaceDomain strokes() {
        List<String> faces = new ArrayList<>();
        for (List<StrokeKeyboardLayout.Key> row : StrokeKeyboardLayout.rows())
            for (StrokeKeyboardLayout.Key key : row) faces.add(StrokeKeyboardLayout.face(key));
        return mainOnly(faces);
    }

    /** 笔画键盘的其他键：网格最后一行的 @#、0、句点（中文标点时是「。」）和右列的 ！。 */
    public static KeyFaceDomain strokeKeys() {
        return mainOnly(List.of("@#", "0", ChineseSymbolFaces.face(".", true), ChineseSymbolFaces.face(".", false),
            exclamation()));
    }

    /** 注音九键左列的五个声调符号。 */
    public static KeyFaceDomain zhuyinTones() {
        List<String> faces = new ArrayList<>();
        for (ZhuyinNineKeyLayout.Tone tone : ZhuyinNineKeyLayout.tones()) faces.add(tone.face());
        return mainOnly(faces);
    }

    /** 注音九键的音键：1–9 和 0 上的注音符号，底部是各自发送的数字。 */
    public static KeyFaceDomain zhuyinSounds() {
        List<ZhuyinNineKeyLayout.Key> keys = new ArrayList<>();
        for (List<ZhuyinNineKeyLayout.Key> row : ZhuyinNineKeyLayout.rows()) keys.addAll(row);
        keys.add(ZhuyinNineKeyLayout.zero());
        List<String> symbols = new ArrayList<>(), digits = new ArrayList<>();
        for (ZhuyinNineKeyLayout.Key key : keys) {
            symbols.add(key.symbols());
            digits.add(String.valueOf(key.digit()));
        }
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, symbols);
        texts.put(KeyFaceSlot.BOTTOM, digits);
        return new KeyFaceDomain(texts);
    }

    /**
     * 注音九键最后一行的逗号和句号（中文标点时是「，」「。」）。另含一个注音符号作基线参照：「，」「。」按自己的墨迹居中，内容域里只剩半角的 , . 时它们会被当成全组的墨迹居中，句点就画成了键面正中的一个点；有注音符号在，它们和旁边的音键一样落在文字的基线上。参照只进内容域，不画在哪个键上。
     */
    public static KeyFaceDomain zhuyinMarks() {
        String reference = ZhuyinNineKeyLayout.rows().get(0).get(0).symbols().substring(0, 1);
        return mainOnly(List.of(ChineseSymbolFaces.face(",", true), ChineseSymbolFaces.face(".", true),
            ChineseSymbolFaces.face(",", false), ChineseSymbolFaces.face(".", false), reference));
    }

    /** 日语九键网格上的键面：假名层是每个键的第一个假名，符号层（{@code symbols}）是数字；两层各一组，切换层时整副键盘重建。 */
    public static KeyFaceDomain japaneseKana(boolean symbols) {
        List<String> faces = new ArrayList<>();
        for (JapaneseNineKeyLayout.Key key : symbols ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys())
            faces.add(key.kana().get(0));
        return mainOnly(faces);
    }

    private static KeyFaceDomain mainOnly(List<String> faces) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        return new KeyFaceDomain(texts);
    }

    /** 右列的 ！：九键标点表的最后一个。 */
    private static String exclamation() {
        List<String> punctuation = NineKeyLayout.punctuation();
        return punctuation.get(punctuation.size() - 1);
    }
}
