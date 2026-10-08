import app.msime.android.CommonPhrasesPanelPolicy;

/** 键盘常用语面板下方直达常用语页的按钮（#5673：列表不空时键盘里找不到增删改的入口）。 */
public final class CommonPhrasesPanelPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check("添加常用语".equals(CommonPhrasesPanelPolicy.entryLabel(0)), "an empty panel invites adding");
        check("管理常用语".equals(CommonPhrasesPanelPolicy.entryLabel(1)), "a panel with phrases still offers managing them");
        check("管理常用语".equals(CommonPhrasesPanelPolicy.entryLabel(200)), "a full panel offers managing them");
        System.out.println("Android common phrases panel entry passed");
    }
}
