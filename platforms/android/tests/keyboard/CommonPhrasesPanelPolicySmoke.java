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
        // 剪贴板历史允许回车和制表符：回车换成换行后照样能存，制表符原样保留并说清是它的问题，而不是说空白或过长。
        check("line1\nline2\nline3".equals(CommonPhrasesPanelPolicy.clipboardPhraseText("line1\r\nline2\rline3")),
            "Windows and old Mac line endings become newlines");
        check("".equals(CommonPhrasesPanelPolicy.clipboardPhraseText(null)), "a missing entry becomes empty text");
        check(CommonPhrasesPanelPolicy.clipboardRefusal(CommonPhrasesPanelPolicy.clipboardPhraseText("line1\r\nline2")) == null,
            "an entry with CRLF line endings can become a phrase");
        check(CommonPhrasesPanelPolicy.clipboardRefusal("synthetic entry") == null, "an ordinary entry is not refused");
        String tab = CommonPhrasesPanelPolicy.clipboardRefusal(CommonPhrasesPanelPolicy.clipboardPhraseText("a\tb"));
        check(tab != null && tab.contains("制表符") && !tab.equals(CommonPhrasesStore.failureMessage("common_phrases_invalid")),
            "a tab is named as the reason, not emptiness or length");
        check(CommonPhrasesPanelPolicy.clipboardPhraseText("a\tb").equals("a\tb"), "tabs are not silently rewritten");
        String bell = CommonPhrasesPanelPolicy.clipboardRefusal("synthetic\u0007bell");
        check(bell != null && bell.contains("制表符等"), "other control characters get the same reason");
        String tooLong = CommonPhrasesPanelPolicy.clipboardRefusal("x".repeat(CommonPhrasesStore.MAX_PHRASE_UNITS + 1));
        check(tooLong != null && tooLong.contains("超过 " + CommonPhrasesStore.MAX_PHRASE_UNITS + " 字"), "an over-long entry names the limit");
        // 回车换成换行以后长度变短：正好卡在上限的 CRLF 文字换完能存。
        String crlfAtLimit = "x".repeat(CommonPhrasesStore.MAX_PHRASE_UNITS - 2) + "\r\n";
        check(CommonPhrasesPanelPolicy.clipboardRefusal(CommonPhrasesPanelPolicy.clipboardPhraseText(crlfAtLimit + "y")) == null,
            "length is checked after line endings are normalized");
        String blank = CommonPhrasesPanelPolicy.clipboardRefusal(" \r\n ");
        check(blank != null && blank.contains("没有文字"), "a blank entry says it has no text");
        System.out.println("Android common phrases panel entry passed");
    }
}
