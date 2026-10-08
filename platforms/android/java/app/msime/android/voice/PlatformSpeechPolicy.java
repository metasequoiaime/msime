package app.msime.android;

import android.speech.SpeechRecognizer;

/**
 * 系统语音识别服务（SpeechRecognizer）的纯逻辑：错误码翻译成用户看得懂、反馈时也能对上号的提示，键盘里没起来时要不要改走识别窗口，以及 onRmsChanged 的分贝值换成聆听面板的音量。
 *
 * <p>只用 SpeechRecognizer 的 int 常量（编译期内联），不调用任何 Android API，JVM 冒烟可以直接跑。以前所有错误都报同一句「语音识别未返回结果」，用户和诊断包里都看不出是网络、权限、没听到声音还是服务本身不可用（#5553），所以提示里一律带上错误码。
 */
public final class PlatformSpeechPolicy {
    /** onRmsChanged 在常见实现里大致落在 -2–10 dB：低于下限当作安静，高于上限当作满格。 */
    static final float QUIET_RMS_DB = -2f;
    static final float LOUD_RMS_DB = 10f;
    /** 音量回落时每次保留上一帧的比例：声音一大立刻跟上，变小时缓缓落下，光圈不会一闪一闪。 */
    static final float LEVEL_RELEASE = 0.75f;

    /** 设备自带的识别服务自己出了问题时给的出路：换一个不依赖它的识别方式。设置页里服务商下拉框的选项就是「本地模型（离线）」和豆包。 */
    static final String ALTERNATIVES = "也可在设置的「语音输入」页改用本地模型或填写豆包密钥";

    private PlatformSpeechPolicy() {}

    /** 一个错误码对应的提示，原因后紧跟「（错误码 N）」（Toast 放不下时截掉的是结尾，错误码要留在前面）；识别服务自身的故障再接上 {@link #ALTERNATIVES}。 */
    public static String message(int error) {
        String reason = switch (error) {
            case SpeechRecognizer.ERROR_NETWORK_TIMEOUT, SpeechRecognizer.ERROR_NETWORK ->
                "系统语音识别服务连不上网络，请检查网络";
            case SpeechRecognizer.ERROR_SERVER, SpeechRecognizer.ERROR_SERVER_DISCONNECTED ->
                "系统语音识别服务出错，请稍后重试";
            case SpeechRecognizer.ERROR_AUDIO -> "系统语音识别服务无法录音，麦克风可能被其他应用占用";
            case SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "没有听到声音，请靠近麦克风再试";
            case SpeechRecognizer.ERROR_NO_MATCH -> "没有识别出文字，请说得清楚些再试";
            case SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "系统语音识别服务正忙，请稍后重试";
            case SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS ->
                "系统语音识别服务没有麦克风权限，请在系统设置里给默认语音识别应用开启麦克风";
            case SpeechRecognizer.ERROR_TOO_MANY_REQUESTS -> "系统语音识别服务请求过于频繁，请稍后重试";
            case SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED, SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE ->
                "系统语音识别服务不支持所选语言，请在语音设置里换一种识别语言";
            case SpeechRecognizer.ERROR_CLIENT -> "系统语音识别服务无法启动";
            default -> "系统语音识别失败";
        };
        String coded = reason + "（错误码 " + error + "）";
        return serviceFault(error) ? coded + "；" + ALTERNATIVES : coded;
    }

    /**
     * 识别服务报了成功（onResults）却没有给出任何文字。它没有走 onError，没有错误码可报，这里也不借用 ERROR_NO_MATCH 的码：反馈回来的「错误码 7」应当只表示服务自己报的没听懂，不然分不清是哪一种失败（#5553）。
     */
    public static String emptyResult() {
        return "系统语音识别服务没有返回文字；" + ALTERNATIVES;
    }

    /**
     * 这个错误码是不是设备识别服务自身的故障（网络、服务端、服务起不来、不支持所选语言、限流或未知错误），换成本地模型或豆包就能绕开。没听到声音、没听懂、麦克风被占用、识别服务缺权限、服务正忙是用户这边重试或调整就能解决的，不给这个出路。
     */
    static boolean serviceFault(int error) {
        return switch (error) {
            case SpeechRecognizer.ERROR_SPEECH_TIMEOUT, SpeechRecognizer.ERROR_NO_MATCH,
                 SpeechRecognizer.ERROR_AUDIO, SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS,
                 SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> false;
            default -> true;
        };
    }

    /**
     * 键盘里直接调起的识别在开始聆听（onReadyForSpeech）之前就失败、而且是调用方一侧的原因（服务拒绝了这个进程、服务连接断开、权限被判为不足）时，改用识别窗口再试一次：窗口是个前台 Activity，原先的流程就是它，有的识别服务只认前台界面发起的请求。已经开始聆听之后的失败（网络、没听到声音等）换个窗口也一样，直接报给用户。
     */
    public static boolean retryInActivity(int error, boolean listening) {
        if (listening) return false;
        return error == SpeechRecognizer.ERROR_CLIENT
            || error == SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS
            || error == SpeechRecognizer.ERROR_SERVER_DISCONNECTED;
    }

    /** onRmsChanged 的分贝值换成 0–1 的音量；NaN 当作安静。 */
    public static float level(float rmsDb) {
        if (Float.isNaN(rmsDb)) return 0f;
        float scaled = (rmsDb - QUIET_RMS_DB) / (LOUD_RMS_DB - QUIET_RMS_DB);
        return Math.max(0f, Math.min(1f, scaled));
    }

    /** 下一帧显示的音量：变大直接跟上，变小按 {@link #LEVEL_RELEASE} 回落。 */
    public static float smoothed(float shown, float next) {
        float target = Math.max(0f, Math.min(1f, Float.isNaN(next) ? 0f : next));
        if (target >= shown) return target;
        return shown * LEVEL_RELEASE + target * (1f - LEVEL_RELEASE);
    }
}
