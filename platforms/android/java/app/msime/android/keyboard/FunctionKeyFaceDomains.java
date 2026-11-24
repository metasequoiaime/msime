package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 键区功能键键组的内容域（见 {@link KeyFaceDomain}）：⇧、⌫ 平时画图标，不占文字；会画成字的是 26 键组字时 ⇧ 换成的「分词」、14/18 键第三排左端的「符」和「分词」、九键右列的「重输」，以及 14/18 键分词键能用网格辅助码时的角标「辅」。图标的大小另由模板定（{@link KeyFaceTemplates#FUNCTIONS}）。
 *
 * <p>键面文字由建键的地方设置；这里列的若与之不同，键画上时会自己补进键组（{@link KeyFaceGroup#include}），只是那一次要重量一遍。
 */
public final class FunctionKeyFaceDomains {
    private FunctionKeyFaceDomains() { }

    /** 26 键全拼组字时 ⇧ 上的字。 */
    static final String SHIFT_SEPARATOR_FACE = "分词";
    /** 九键右列 ⌫ 下面那一格。 */
    static final String NINE_KEY_REWRITE_FACE = "重输";

    public static KeyFaceDomain functions() {
        return domain(List.of());
    }

    /**
     * 日语九键：除上面这些，还有两侧列的文字键（123，符号层上是「あいう」；符；切换；语言键的「英」「中」；空白，组字时是「変換」）和网格左下角的「小゛゜」（符号层上是「（）」）。只给日语九键用：「小゛゜」三个字比别处的功能键文字宽，放进别的键盘的组会让那里的功能键都按它缩小。语言键轮换到其他语言时写那种语言的字，画上时自己补进键组。
     */
    public static KeyFaceDomain japaneseNineKey() {
        return domain(List.of("123", JapaneseNineKeyLayout.LAYER_TITLE, "符", "切换", "英", "中",
            JapaneseNineKeyActions.spaceTitle(false), JapaneseNineKeyActions.spaceTitle(true), "小゛゜", "（）"));
    }

    private static KeyFaceDomain domain(List<String> extra) {
        List<String> main = new java.util.ArrayList<>(List.of(SHIFT_SEPARATOR_FACE, FourteenKeyLayout.separatorFace(false),
            FourteenKeyLayout.separatorFace(true), NINE_KEY_REWRITE_FACE));
        main.addAll(extra);
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, main);
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of(GridHelpcodePolicy.cornerHint(true)));
        return new KeyFaceDomain(texts);
    }
}
