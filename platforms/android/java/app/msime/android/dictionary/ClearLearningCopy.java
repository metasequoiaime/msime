package app.msime.android;

/**
 * 「词库 › 学习」里「清除学习记录」的文案和结果提示，做法见 {@link DictionaryCollectionsStore#resetLearning}。
 *
 * <p>iOS 的 `ClearLearningCopy` 与这里的标题、说明、按钮、确认框第一句、网盘同步那一句和失败提示逐字相同；确认框其余部分各说各的：iOS 的账号同步不含个人词库，也没有命名词库，导入和从社区添加的词都是自造词。
 */
public final class ClearLearningCopy {
    private ClearLearningCopy() {}

    public static final String TITLE = "清除学习记录";
    public static final String SUBTITLE = "删除学到的调频、联想和自造词，设置保留";
    public static final String ACTION = "清除";
    public static final String CONFIRM_TITLE = "清除学习记录？";
    /** 删什么，两端相同的那一句。用户自己加的快捷短语也是自造词，跟着删；常用语不是。 */
    public static final String REMOVED =
        "将永久删除本机学到的候选调频、置顶和固定位置、选词记录、整句联想、拼写纠错记录，以及自己添加和修改的词条（自造词，含快捷短语），无法撤销。";
    /** 「我的词条」网盘同步把删除同步到别的设备，两端相同；一次删得多时它先挡下、等用户确认（`MASS_DELETE_MIN`）。 */
    public static final String WEBDAV = "开着词库网盘同步时，删掉的自造词也会从其他设备上删除（一次删得多时会先请你确认）。";
    public static final String CONFIRM_MESSAGE = REMOVED
        + "设置、常用语、剪贴板和打字统计不受影响；已添加的词库保留，它们的词会重新写入。"
        + "开着云同步时，账号里的个人词库也会换成清除后的这一份，登录同一账号的其他设备同步时会跟着换掉，或者提示你选择。"
        + WEBDAV;
    public static final String CLEARED = "已清除学习记录";
    /** 键盘会话开着时 Rust 报 `dictionary maintenance busy`。键盘在输入框失焦后才销毁会话，之后还会短暂写一次词库，所以只说收起键盘不够。 */
    public static final String BUSY = "键盘正在使用词库，请收起键盘、稍等几秒再试。";
    public static final String FAILED = "清除学习记录失败，请稍后重试。";

    /**
     * 清除成功的提示。
     *
     * @param pending 已添加词库这一次没写完、等键盘空闲时继续写的条数
     * @param collectionsFailed 已添加词库的词没能重新排队（Rust 回答里带 `collections_error`）
     * @param unsynced 云同步开着，却没能记下这次清除（下一轮不会上传清除后的这一份）
     */
    public static String cleared(int pending, boolean collectionsFailed, boolean unsynced) {
        String text = CLEARED;
        if (collectionsFailed) text += "，但已添加词库的词没能重新写入";
        else if (pending > 0) text += "，已添加词库的 " + NumberPolicy.groupedCount(pending) + "会在键盘空闲时写回";
        if (unsynced) text += "；云同步没能记下这次清除，账号里的个人词库仍是清除前的那一份";
        return text;
    }
}
