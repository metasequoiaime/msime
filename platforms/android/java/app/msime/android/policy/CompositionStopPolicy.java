package app.msime.android.policy;

/**
 * 停下会话、销毁引擎之前对还在组字区的文字发哪条命令。
 *
 * <p>组字区里是按键本身时（{@link InputSchemeTraits#composesKeyLetters}），原样上屏就是把拼音、双拼码、五笔码、粤拼字母或笔画字形写成正文，用户随后发出去的就是这一段字母（#11831）。这类方案按首选结束组字，留下的汉字才是用户打出来的字；收起键盘时已经这么做过。韩语音节、日文读音这些组字区里是用户写下的文字，仍然是原样上屏。
 */
public final class CompositionStopPolicy {
    /** 不发命令：组字区交给后面的删除或结束处理。 */
    public static final int NO_COMMAND = 0;
    /** 共享宿主命令 2，`Command::CommitRaw`：原样上屏组字区。 */
    public static final int COMMIT_RAW = 2;
    /** 共享宿主命令 9，`Action::Finish`：按首选结束组字。 */
    public static final int FINISH = 9;

    private CompositionStopPolicy() {}

    public static int command(boolean finish, boolean keyLetters) {
        if (!finish) return NO_COMMAND;
        return keyLetters ? FINISH : COMMIT_RAW;
    }
}
