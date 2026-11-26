package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 底行里逗号、句号和空格三个键组的内容域（见 {@link KeyFaceDomain}）：它们的字随标点模式和方案短名换，不是键自己的几种状态，所以按固定的表列出；符、123、中/英和回车的键面由键面规格给出（{@link KeyFaceSpecs}）。只取决于常量，不读服务，JVM 冒烟可以直接检查。列漏了的字画上时会被键组记作漏网（{@link KeyFaceGroup#include}）。
 */
public final class BottomRowKeyFaceDomains {
    private BottomRowKeyFaceDomains() { }

    /** 空格键组的参照文字：空格只取键组的字号和麦克风尺寸，方案短名放不下时自己按字截短（{@link SpaceKeyFace}），所以内容域只放一个常见的短名，字号随键高和字体缩放变，不随最长的短名缩小。 */
    public static final String SPACE_LABEL_REFERENCE = "全拼";
    /** 句号键组的基线参照：键面按汉字的墨迹居中，「。」和写在字里的句号一样落在左下角，不会被当成全组的墨迹居中画成键面正中的圆圈。参照只进内容域，不画在哪个键上。 */
    static final String PERIOD_BASELINE_REFERENCE = "中";

    /** 底行的逗号键：中文「，」、英文「,」、网址框里的「.」、日语的「、」，右上角是长按输入的句号、问号或逗号。 */
    public static KeyFaceDomain comma() {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of("，", ",", ".", "、"));
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of("。", ".", "？", "?", ","));
        return new KeyFaceDomain(texts);
    }

    /** 底行的句号键：中文「。」、英文「.」，加上基线参照；手写底行右上角是长按输入的叹号。 */
    public static KeyFaceDomain period() {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of("。", ".", PERIOD_BASELINE_REFERENCE));
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of("！", "!"));
        return new KeyFaceDomain(texts);
    }

    /** 空格键：只有参照短名（{@link #SPACE_LABEL_REFERENCE}）。 */
    public static KeyFaceDomain space() {
        return mainOnly(List.of(SPACE_LABEL_REFERENCE));
    }

    private static KeyFaceDomain mainOnly(List<String> faces) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, faces);
        return new KeyFaceDomain(texts);
    }
}
