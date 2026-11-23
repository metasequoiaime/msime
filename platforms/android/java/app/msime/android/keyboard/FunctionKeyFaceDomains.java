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
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of(SHIFT_SEPARATOR_FACE, FourteenKeyLayout.separatorFace(false),
            FourteenKeyLayout.separatorFace(true), NINE_KEY_REWRITE_FACE));
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of(GridHelpcodePolicy.cornerHint(true)));
        return new KeyFaceDomain(texts);
    }
}
