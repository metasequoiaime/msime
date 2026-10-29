package app.msime.android;

/**
 * 「词库 › 学习」里「清除学习记录」的文案和结果提示，做法见 {@link DictionaryCollectionsStore#resetLearning}。
 *
 * <p>iOS 的 `ClearLearningCopy` 与这里逐字相同，只是确认框里没有云同步那一句、多一句导入的词也会删除：iOS 的账号同步不含个人词库，也没有命名词库，导入和从社区添加的词都是自造词。
 */
public final class ClearLearningCopy {
    private ClearLearningCopy() {}

    public static final String TITLE = "清除学习记录";
    public static final String SUBTITLE = "删除学到的调频、联想和自造词，设置保留";
    public static final String ACTION = "清除";
    public static final String CONFIRM_TITLE = "清除学习记录？";
    /** 删什么、留什么，两端相同的那一句。 */
    public static final String REMOVED =
        "将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词），无法撤销。";
    public static final String CONFIRM_MESSAGE = REMOVED
        + "设置、常用语、剪贴板和打字统计不受影响；已添加的词库保留，它们的词会重新写入。"
        + "开着云同步时，账号里的个人词库也会换成清除后的这一份。";
    public static final String CLEARED = "已清除学习记录";

    /** 清除成功的提示；已添加词库的词这一次没写完时说一声，免得用户以为它们也被清掉了。 */
    public static String cleared(int pending) {
        if (pending <= 0) return CLEARED;
        return CLEARED + "，已添加词库的 " + NumberPolicy.groupedCount(pending) + "会在键盘空闲时写回";
    }
}
