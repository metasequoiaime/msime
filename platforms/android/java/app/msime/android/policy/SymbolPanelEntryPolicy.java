package app.msime.android;

/**
 * 符号面板能不能打开、点按的符号补不补后半个、记不记进「常用」（#10517）。
 *
 * <p>符号面板经输入连接直接上屏、读随包的符号目录，不需要引擎会话：没有会话的输入框（密码框、数字框、启动失败退回直接输入）和会话还在建的输入框都能打开。终端类应用（Termius、ConnectBot 等）常把输入框声明成可见密码来关掉联想和纠错，这类输入框不建会话，原先点「符」只提示「符号面板尚未就绪」。
 *
 * <p>密码框（{@link EditorPolicy#password}）里按字面输入：轻点「（」不补「）」，否则密码里多出一个字符；用过的符号也不记进「常用」，免得在密码里用过什么被带到别的输入框，与剪贴板、语音、按键统计在密码框里的口径一致。
 */
public final class SymbolPanelEntryPolicy {
    private SymbolPanelEntryPolicy() {}

    /** 只看输入连接和面板是否已建好，不看引擎会话。 */
    public static boolean opens(boolean connection, boolean panelBuilt) {
        return connection && panelBuilt;
    }

    /** 轻点前半个符号时是否在光标右边补上后半个：成对补全开着，并且不是密码框。 */
    public static boolean pairsClosingMark(int inputType, boolean pairedPunctuation) {
        return pairedPunctuation && !EditorPolicy.password(inputType);
    }

    /** 点按的符号是否记进「常用」：不是密码框，也没有因隐私模式或不许个性化学习而停掉学习。 */
    public static boolean recordsRecent(int inputType, boolean learningSuppressed) {
        return !learningSuppressed && !EditorPolicy.password(inputType);
    }
}
