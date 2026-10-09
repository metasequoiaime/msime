package app.msime.android;

/**
 * 键盘「常用语」面板列表下方那个直达应用常用语页的按钮。
 *
 * <p>键盘里没有可以输入的文本框，常用语的添加、修改和删除都在应用的常用语页（{@code PhrasesPage}）里做。以前只在列表为空时给「添加常用语」，列表不空时键盘里找不到去处（#5673），现在总有一个按钮，按列表是否为空换文字。
 */
public final class CommonPhrasesPanelPolicy {
    private CommonPhrasesPanelPolicy() {}

    /** 按钮文字：还没有常用语时引导添加，有了以后是管理（添加、修改、删除）。 */
    public static String entryLabel(int phraseCount) {
        return phraseCount <= 0 ? "添加常用语" : "管理常用语";
    }

    /** 剪贴板面板长按操作行里把这一条存成无编码常用语的按钮（#5909）。 */
    public static final String ADD_FROM_CLIPBOARD = "添加到常用语";
    /** 存好以后的提示。只说存进了常用语：常用语开着同步时会上传，不能写成「只保存在本机」。 */
    public static final String ADDED_FROM_CLIPBOARD = "已添加到常用语";

    /**
     * 「添加到常用语」之后给用户的那句话：成功时是 {@link #ADDED_FROM_CLIPBOARD}；失败时用常用语存储给出的原因（已经有这条、已达上限、过长），没有原因时用常用语的通用失败说法。
     */
    public static String clipboardAddNotice(boolean added, String failure) {
        if (added) return ADDED_FROM_CLIPBOARD;
        return failure == null || failure.isEmpty() ? CommonPhrasesStore.failureMessage("") : failure;
    }
}
