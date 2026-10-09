import app.msime.android.CommonPhrasesPanelPolicy;
import app.msime.android.CommonPhrasesStore;

/** 键盘常用语面板下方直达常用语页的按钮（#5673：列表不空时键盘里找不到增删改的入口）。 */
public final class CommonPhrasesPanelPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check("添加常用语".equals(CommonPhrasesPanelPolicy.entryLabel(0)), "an empty panel invites adding");
        check("管理常用语".equals(CommonPhrasesPanelPolicy.entryLabel(1)), "a panel with phrases still offers managing them");
        check("管理常用语".equals(CommonPhrasesPanelPolicy.entryLabel(200)), "a full panel offers managing them");

        // #5909：剪贴板长按操作行里的「添加到常用语」。
        check("添加到常用语".equals(CommonPhrasesPanelPolicy.ADD_FROM_CLIPBOARD), "the clipboard action uses the issue's label");
        check("已添加到常用语".equals(CommonPhrasesPanelPolicy.clipboardAddNotice(true, "")), "a stored phrase is confirmed");
        String duplicate = CommonPhrasesStore.failureMessage("common_phrases_duplicate");
        check(duplicate.equals(CommonPhrasesPanelPolicy.clipboardAddNotice(false, duplicate)), "an existing phrase says so");
        String full = CommonPhrasesStore.failureMessage("common_phrases_limit");
        check(full.equals(CommonPhrasesPanelPolicy.clipboardAddNotice(false, full)), "a full list names the limit");
        check(CommonPhrasesStore.failureMessage("").equals(CommonPhrasesPanelPolicy.clipboardAddNotice(false, "")),
            "a failure without a reason uses the generic wording");
        check(CommonPhrasesStore.failureMessage("").equals(CommonPhrasesPanelPolicy.clipboardAddNotice(false, null)),
            "a missing reason uses the generic wording");
        check(!CommonPhrasesPanelPolicy.clipboardAddNotice(true, "").contains("本机"),
            "the notice does not promise the phrase stays on this device: phrases sync");
        // 剪贴板历史不限 1000 字，常用语限：过长、空白和含控制字符的记录在键盘里就被拦下。
        check(CommonPhrasesStore.validText("synthetic clipboard entry"), "an ordinary entry can become a phrase");
        check(CommonPhrasesStore.validText("synthetic\nmultiline"), "a multi-line entry can become a phrase");
        check(!CommonPhrasesStore.validText("x".repeat(CommonPhrasesStore.MAX_PHRASE_UNITS + 1)), "an entry over 1000 units cannot");
        check(!CommonPhrasesStore.validText(" \n\t"), "a blank entry cannot");
        check(!CommonPhrasesStore.validText("synthetic\u0007bell"), "an entry with a control character cannot");
        System.out.println("Android common phrases panel entry passed");
    }
}
