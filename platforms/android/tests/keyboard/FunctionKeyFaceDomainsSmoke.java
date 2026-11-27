import app.msime.android.FourteenKeyLayout;
import app.msime.android.FunctionKeyFaceDomains;
import app.msime.android.GridHelpcodePolicy;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceSlot;

/** 键区功能键的内容域：会画成字的分词、符、重输和角标「辅」都在里面，⇧、⌫ 的节点文字不在（它们画图标）。 */
public final class FunctionKeyFaceDomainsSmoke {
    private FunctionKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        KeyFaceDomain domain = FunctionKeyFaceDomains.functions();
        for (boolean composing : new boolean[] {false, true})
            check(domain.contains(KeyFaceSlot.MAIN, FourteenKeyLayout.separatorFace(composing)),
                "缺 14/18 键分词键的键面 " + FourteenKeyLayout.separatorFace(composing));
        check(domain.contains(KeyFaceSlot.MAIN, "分词"), "缺 26 键组字时 ⇧ 上的「分词」");
        check(domain.contains(KeyFaceSlot.MAIN, "重输"), "缺九键右列的「重输」");
        check(domain.contains(KeyFaceSlot.TOP_RIGHT, GridHelpcodePolicy.CORNER_HINT), "缺分词键的角标「辅」");
        check(!domain.contains(KeyFaceSlot.MAIN, "⇧") && !domain.contains(KeyFaceSlot.MAIN, "⌫"),
            "画图标的键的节点文字不应进内容域");
        check(!domain.has(KeyFaceSlot.BOTTOM), "功能键没有底部提示");
        System.out.println("FunctionKeyFaceDomainsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
