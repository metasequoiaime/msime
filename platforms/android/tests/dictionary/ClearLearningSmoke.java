import app.msime.android.ClearLearningCopy;
import app.msime.android.DictionaryCollectionsStore;

/** 「词库 › 学习」的「清除学习记录」：确认框说清删什么、留什么、会同步到账号和网盘，失败和结果提示按 Rust 的回答说。 */
public final class ClearLearningSmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        check(ClearLearningCopy.TITLE.equals("清除学习记录"), "row title");
        check(ClearLearningCopy.CONFIRM_TITLE.equals("清除学习记录？"), "dialog title");
        // 引擎清除的每一类都写出来，保留的也写出来。
        for (String removed : new String[] {"候选调频", "置顶和固定位置", "选词记录", "整句联想", "拼写纠错记录", "自造词", "含快捷短语", "无法撤销"}) {
            check(ClearLearningCopy.CONFIRM_MESSAGE.contains(removed), "names " + removed);
        }
        for (String kept : new String[] {"设置", "常用语", "剪贴板", "打字统计", "已添加的词库保留"}) {
            check(ClearLearningCopy.CONFIRM_MESSAGE.contains(kept), "keeps " + kept);
        }
        check(ClearLearningCopy.CONFIRM_MESSAGE.contains("账号里的个人词库也会换成清除后的这一份"), "says it syncs");
        check(ClearLearningCopy.CONFIRM_MESSAGE.contains("登录同一账号的其他设备"), "says other devices follow");
        check(ClearLearningCopy.CONFIRM_MESSAGE.endsWith(ClearLearningCopy.WEBDAV), "says the WebDAV sync deletes too");
        check(ClearLearningCopy.CONFIRM_MESSAGE.startsWith(ClearLearningCopy.REMOVED), "shared sentence first");

        check(ClearLearningCopy.cleared(0, false, false).equals("已清除学习记录"), "plain success");
        check(ClearLearningCopy.cleared(1203, false, false).equals("已清除学习记录，已添加词库的 1,203 条会在键盘空闲时写回"), "pending success");
        check(ClearLearningCopy.cleared(0, true, false).equals("已清除学习记录，但已添加词库的词没能重新写入"), "requeue failed");
        check(ClearLearningCopy.cleared(0, false, true).equals("已清除学习记录；云同步没能记下这次清除，账号里的个人词库仍是清除前的那一份"),
            "sync not marked");

        check(DictionaryCollectionsStore.resetFailureMessage("dictionary maintenance busy").equals("键盘正在使用词库，请收起键盘、稍等几秒再试。"),
            "busy");
        check(DictionaryCollectionsStore.resetFailureMessage("learned-data reset rejected").equals("清除学习记录失败，请稍后重试。"),
            "rejected");
        check(DictionaryCollectionsStore.resetFailureMessage("").equals("清除学习记录失败，请稍后重试。"), "unknown");
    }
}
