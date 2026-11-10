import app.msime.android.InputSchemeTraits;
import app.msime.android.SpellingEditorPolicy;

/** 编辑开关只移除空入口行，读音、状态及非拼写方案不受影响。 */
public final class SpellingEditorPolicySmoke {
    public static void main(String[] args) {
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN,
                InputSchemeTraits.WUBI, InputSchemeTraits.CANTONESE}) {
            check(SpellingEditorPolicy.reserveHeader(true, scheme, false, false, "", "", false),
                "默认编辑入口应保留");
            check(!SpellingEditorPolicy.reserveHeader(false, scheme, false, false, "", "", false),
                "关闭后组字中的空笔行应移除");
            check(!SpellingEditorPolicy.reserveHeader(false, scheme, false, true, "合成标题", "", false),
                "空闲品牌不应重新撑高键盘");
            check(SpellingEditorPolicy.reserveHeader(false, scheme, true, true, "", "", false),
                "候选栏拼写布局从空闲时就预留读音行");
            check(SpellingEditorPolicy.reserveHeader(false, scheme, false, false, "ni", "", false),
                "必要的拼写不能丢失");
            check(SpellingEditorPolicy.reserveHeader(false, scheme, false, true, "", "准备中", false),
                "准备和失败状态不能被隐藏");
            check(SpellingEditorPolicy.reserveHeader(false, scheme, false, false, "", "", true),
                "退出模式等动作必须可达");
        }
        check(SpellingEditorPolicy.reserveHeader(false, InputSchemeTraits.JAPANESE,
            false, true, "", "", false), "日语候选行保持原布局");
    }

    private static void check(boolean actual, String description) {
        if (!actual) throw new AssertionError(description);
    }
}
