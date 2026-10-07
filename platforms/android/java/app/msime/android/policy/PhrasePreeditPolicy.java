package app.msime.android;

/**
 * 正在拼的词里已经选中的那一段，画在哪里。
 *
 * <p>候选只吃掉部分输入时，引擎会继续组字并把选中的那一段交回宿主。运行时按 {@code phrase_preedit}
 * 把它留在组字里（{@code view.phrase_prefix}）而不是立刻上屏——用户还在打后半截，前半截已经进了
 * 文档的话，搜索框会拿半个词去搜，编辑器为它记一次撤销。
 *
 * <p>留住它的宿主必须把它画出来，两件事是一个决定：请求了却不画，用户已经选中的字既不在文档里也
 * 不在屏幕上，而组字在他按取消之前不会结束。这个宿主有两处要画——编辑框里的组字，和候选条上那行
 * 读音——规则都是「已选的那一段领在前面」，与来源把 {@code word_for_creating_word} 拼在读音前面
 * 是同一件事。
 */
public final class PhrasePreeditPolicy {
    private PhrasePreeditPolicy() {}

    /** 交给编辑框的组字：已选的那一段 + 还在打的读音。 */
    public static String composing(String phrasePrefix, String editingText) {
        String prefix = TextPolicy.emptyIfNull(phrasePrefix);
        String editing = TextPolicy.emptyIfNull(editingText);
        return prefix + editing;
    }

    /**
     * 候选条上那行标题。
     *
     * <p>本地模式（Emoji、颜文字、日期时间等）有自己的标题，那时不加前缀：那些模式下不存在「正在
     * 拼的词」，把一段汉字接在模式名前面只会让人以为模式名变了。
     */
    public static String title(String phrasePrefix, String title, boolean localMode) {
        String prefix = TextPolicy.emptyIfNull(phrasePrefix);
        String shown = TextPolicy.emptyIfNull(title);
        return prefix.isEmpty() || localMode ? shown : prefix + shown;
    }
}
