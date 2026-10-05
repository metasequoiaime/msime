package app.msime.android;

import android.view.inputmethod.EditorInfo;

/**
 * 「是否记录」的统一判断（扩展点）：打字统计、按键计数、剪贴板捕获都先问这里。
 *
 * <p>现在的口径与拆分前完全相同：按键计数排除密码框和要求不做个性化学习的输入框（EditorPolicy.excludesKeyStatistics），并且要等共享存储里的统计开关读回来是开；打字统计只要有目录和非空文本就交给共享存储，开关由存储自己判断；剪贴板只在剪贴板历史开启且存储已建好时捕获。输入事件、语音贡献与 diagnostic_log.mobile 的判断以后也放在这里。
 */
final class ImePrivacyGate {
    private final MSIMEInputService s;

    ImePrivacyGate(MSIMEInputService s) {
        this.s = s;
    }

    /** 这个输入框的按键是否不计数；没有输入框信息时一律不计。 */
    static boolean excludesKeyStatistics(EditorInfo info) {
        return info == null || excludesKeyStatistics(info.inputType, info.imeOptions);
    }

    static boolean excludesKeyStatistics(int inputType, int imeOptions) {
        return EditorPolicy.excludesKeyStatistics(inputType, imeOptions);
    }

    /** 一次上屏是否交给打字统计；统计开关由共享存储判断，这里只挡掉没有目录或没有文本的情况。 */
    static boolean recordsTyping(String directory, String text) {
        return !directory.isEmpty() && text != null && !text.isEmpty();
    }

    static boolean countsKeys(boolean statisticsEnabled, boolean editorExcluded) {
        return statisticsEnabled && !editorExcluded;
    }

    static boolean capturesClipboard(boolean historyEnabled, boolean storeReady) {
        return historyEnabled && storeReady;
    }

    /** 当前输入框的按键是否计入热力图。 */
    boolean countsKeys() {
        return countsKeys(s.keyStatisticsEnabled, s.keyStatisticsExcluded);
    }

    /** 当前是否把系统剪贴板存进剪贴板历史。 */
    boolean capturesClipboard() {
        return capturesClipboard(s.clipboardHistoryEnabled, s.clipboardHistory != null);
    }
}
