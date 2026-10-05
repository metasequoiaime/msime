package app.msime.android;

import android.view.inputmethod.EditorInfo;

/**
 * 「是否记录」的统一判断（P17 / P19）：打字统计、按键计数、语音时长、剪贴板历史、云剪贴板、输入事件、语音贡献与 diagnostic_log.mobile 都先问这里。
 *
 * <p>三种情况下一律不记：隐私模式（`touch_incognito`）开着、输入框带 `IME_FLAG_NO_PERSONALIZED_LEARNING`、或是密码类输入框。除此之外各项仍按原来的口径：按键计数要等共享存储里的统计开关读回来是开；打字统计只挡掉没有目录或没有文本的情况，开关由存储自己判断；剪贴板只在剪贴板历史开启且存储已建好时捕获。云候选、翻译、AI 这类功能性请求各有自己的开关，不在这里判断。
 */
final class ImePrivacyGate {
    /** 受隐私规则约束的各类记录。 */
    enum Record {
        /** 上屏字数的打字统计（`record`）。 */
        TYPING,
        /** 按键热力图（`record_keys`）。 */
        KEYS,
        /** 语音输入时长（`record_voice`）。 */
        VOICE_DURATION,
        /** 把系统剪贴板存进剪贴板历史。 */
        CLIPBOARD_HISTORY,
        /** 推送到云剪贴板。 */
        CLOUD_CLIPBOARD,
        /** 本地「记录输入日志」的 input-events.jsonl 与 perf.jsonl。 */
        INPUT_EVENTS,
        /** 上传语音与识别文本以改进识别。 */
        VOICE_CONTRIBUTION,
        /** diagnostic_log.mobile 的诊断日志。 */
        DIAGNOSTIC_LOG
    }

    /** 键盘进程里唯一的服务实例的判断入口；没有服务时（冒烟、宿主进程）为 null，静态判断按没有隐私限制处理。 */
    private static volatile ImePrivacyGate current;

    private final MSIMEInputService s;

    ImePrivacyGate(MSIMEInputService s) {
        this.s = s;
        current = this;
    }

    /** 这个输入框的按键是否不计数；没有输入框信息时一律不计。 */
    static boolean excludesKeyStatistics(EditorInfo info) {
        return info == null || excludesKeyStatistics(info.inputType, info.imeOptions);
    }

    static boolean excludesKeyStatistics(int inputType, int imeOptions) {
        return EditorPolicy.excludesKeyStatistics(inputType, imeOptions);
    }

    /** 隐私模式、不做个性化学习的输入框、密码框三种情况之一时，所有记录与贡献都不做。 */
    static boolean suppressed(boolean incognito, int inputType, int imeOptions) {
        return incognito || EditorPolicy.excludesKeyStatistics(inputType, imeOptions);
    }

    /** 某一类记录在给定状态下是否允许；三种隐私情况下全部为假，其他情况下全部为真（各项自己的开关另行判断）。 */
    static boolean allows(Record record, boolean incognito, int inputType, int imeOptions) {
        if (record == null) return false;
        return !suppressed(incognito, inputType, imeOptions);
    }

    /** 一次上屏是否交给打字统计；统计开关由共享存储判断，这里挡掉没有目录、没有文本以及三种隐私情况。 */
    static boolean recordsTyping(String directory, String text) {
        if (directory.isEmpty() || text == null || text.isEmpty()) return false;
        ImePrivacyGate gate = current;
        return gate == null || gate.allows(Record.TYPING);
    }

    static boolean countsKeys(boolean statisticsEnabled, boolean editorExcluded) {
        return statisticsEnabled && !editorExcluded;
    }

    static boolean capturesClipboard(boolean historyEnabled, boolean storeReady) {
        return historyEnabled && storeReady;
    }

    /** 输入框是否允许个性化学习，换算回 imeOptions 位供判断使用。 */
    private int imeOptions() {
        return s.allowLearning ? 0 : EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
    }

    /** 当前输入框与隐私模式下，这一类记录是否允许。 */
    boolean allows(Record record) {
        return allows(record, s.incognitoEnabled, s.editorInputType, imeOptions());
    }

    /** 当前输入框的按键是否计入热力图。 */
    boolean countsKeys() {
        return countsKeys(s.keyStatisticsEnabled, s.keyStatisticsExcluded) && allows(Record.KEYS);
    }

    /** 当前是否把系统剪贴板存进剪贴板历史。 */
    boolean capturesClipboard() {
        return capturesClipboard(s.clipboardHistoryEnabled, s.clipboardHistory != null)
            && allows(Record.CLIPBOARD_HISTORY);
    }

    /** 是否允许把条目推到云剪贴板（云剪贴板自身的可用性由调用方另行判断）。 */
    boolean pushesCloudClipboard() {
        return allows(Record.CLOUD_CLIPBOARD);
    }

    /** 一次语音识别结束后是否记语音时长。 */
    boolean recordsVoice() {
        return allows(Record.VOICE_DURATION);
    }

    /** 是否写本地输入事件与耗时日志。 */
    boolean recordsInputEvents() {
        return allows(Record.INPUT_EVENTS);
    }

    /** 是否允许上传这次语音以改进识别（`contribute_audio` 本身由调用方判断）。 */
    boolean contributesVoice() {
        return allows(Record.VOICE_CONTRIBUTION);
    }

    /** 是否写 diagnostic_log.mobile 的诊断日志。 */
    boolean writesDiagnosticLog() {
        return allows(Record.DIAGNOSTIC_LOG);
    }
}
