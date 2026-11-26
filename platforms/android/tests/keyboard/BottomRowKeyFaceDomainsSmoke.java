import app.msime.android.BottomRowKeyFaceDomains;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceSlot;

/** 底行逗号、句号和空格的内容域：逗号的四种写法和角标，句号和它的基线参照，空格只有参照短名。符、123、中/英和回车由键面规格给出，见 KeyFaceSpecsSmoke。 */
public final class BottomRowKeyFaceDomainsSmoke {
    private BottomRowKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        KeyFaceDomain comma = BottomRowKeyFaceDomains.comma();
        for (String face : new String[] {"，", ",", ".", "、"}) check(comma.contains(KeyFaceSlot.MAIN, face), "逗号键缺 " + face);
        for (String hint : new String[] {"。", "？", ","}) check(comma.contains(KeyFaceSlot.TOP_RIGHT, hint), "逗号键角标缺 " + hint);

        KeyFaceDomain period = BottomRowKeyFaceDomains.period();
        check(period.contains(KeyFaceSlot.MAIN, "。") && period.contains(KeyFaceSlot.MAIN, ".")
            && period.contains(KeyFaceSlot.MAIN, "中"), "句号键缺键面或基线参照");
        check(period.contains(KeyFaceSlot.TOP_RIGHT, "！"), "句号键缺手写底行的叹号角标");

        KeyFaceDomain space = BottomRowKeyFaceDomains.space();
        check(space.contains(KeyFaceSlot.MAIN, BottomRowKeyFaceDomains.SPACE_LABEL_REFERENCE)
            && !space.contains(KeyFaceSlot.MAIN, "双拼 · 智能 ABC"), "空格只放参照短名，长短名自己截短");

        System.out.println("BottomRowKeyFaceDomainsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
