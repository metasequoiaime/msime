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
}
