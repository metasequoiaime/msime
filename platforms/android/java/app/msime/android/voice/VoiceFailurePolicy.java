package app.msime.android;

/**
 * 云端识别（整句上传与豆包流式）失败时给用户的提示和记进诊断的类别，键盘和识别窗口共用这一份。
 *
 * <p>两条入口以前各写一遍同样的 switch，豆包流式更是不分原因一律报「语音识别服务未响应」，用户分不清是断网、服务限时还是没说话（#9886）。失败原因由识别器归到 {@link HttpAsrRecognizer.Failure}，这里只负责把原因翻成文字和诊断类别。提示不超过 {@link PlatformSpeechPolicy#MAX_LENGTH} 个字符，Android 12 起 Toast 只显示两行。
 */
public final class VoiceFailurePolicy {
    private VoiceFailurePolicy() {}

    /** 没有识别出任何文字时的提示；取消不提示，返回 null。 */
    public static String message(HttpAsrRecognizer.Failure failure) {
        return switch (failure) {
            case PERMISSION -> "语音识别需要麦克风权限";
            case UNAVAILABLE -> "麦克风被其他应用占用";
            case NETWORK -> "语音服务未响应，请检查网络与密钥";
            case TIMEOUT, SERVICE_TIMEOUT -> "语音服务请求超时，请稍后重试";
            case LIMIT -> "音频超过服务限制，请缩短录音";
            case SERVICE -> "语音服务拒绝请求，请检查服务配置";
            case BUSY -> "语音服务暂时不可用，请稍后重试";
            case ENDED -> "语音服务提前结束了识别，可能已到服务时长限制";
            case EMPTY -> "没有听到内容";
            case CANCELLED -> null;
        };
    }

    /**
     * 诊断类别。本机这一侧等服务超时（{@link HttpAsrRecognizer.Failure#TIMEOUT}）记超时，以前记成网络错误，和提示的「请求超时」对不上；服务端自己回 408/504（{@link HttpAsrRecognizer.Failure#SERVICE_TIMEOUT}）是服务的回应，仍记回应失败。
     */
    public static DiagnosticEventKind diagnostic(HttpAsrRecognizer.Failure failure) {
        return switch (failure) {
            case PERMISSION -> DiagnosticEventKind.VOICE_FAILURE_PERMISSION;
            case UNAVAILABLE -> DiagnosticEventKind.VOICE_FAILURE_AUDIO;
            case NETWORK -> DiagnosticEventKind.VOICE_FAILURE_NETWORK;
            case TIMEOUT -> DiagnosticEventKind.VOICE_FAILURE_TIMEOUT;
            case SERVICE_TIMEOUT, LIMIT, SERVICE, BUSY, ENDED -> DiagnosticEventKind.VOICE_FAILURE_RESPONSE;
            case EMPTY -> DiagnosticEventKind.VOICE_EMPTY;
            case CANCELLED -> DiagnosticEventKind.VOICE_CANCELLED;
        };
    }

    /** 服务回了非 2xx 状态码时的原因：413 是超出限制，408/504 是服务端超时，其余按拒绝请求。整句上传的 HTTP 回应和豆包被拒的握手共用这张表。 */
    public static HttpAsrRecognizer.Failure status(int status) {
        return switch (HttpAsrPolicy.statusFailure(status)) {
            case TOO_LARGE -> HttpAsrRecognizer.Failure.LIMIT;
            case TIMED_OUT -> HttpAsrRecognizer.Failure.SERVICE_TIMEOUT;
            case REJECTED -> HttpAsrRecognizer.Failure.SERVICE;
        };
    }

    /**
     * 豆包 WebSocket 握手没被接受时的原因。读不出状态码（连接在握手中途断了，或者回来的不是 HTTP）算网络问题；回了 101 却对不上校验值，是服务或中间代理答非所问，按拒绝请求；其余按 {@link #status} 归类。
     */
    public static HttpAsrRecognizer.Failure handshake(String response) {
        int status = WebSocketFrames.handshakeStatus(response);
        if (status < 0) return HttpAsrRecognizer.Failure.NETWORK;
        if (status == 101) return HttpAsrRecognizer.Failure.SERVICE;
        return status(status);
    }

    /**
     * 豆包错误帧的错误码对应的原因，码表见《大模型流式语音识别 API》：45000002 是空音频，45000081 是服务端等音频包超时，550xxxxx 是服务内部错误（其中 55000031 是服务过载），改配置都没有用；其余 450xxxxx（参数无效、音频格式不对）和读不出错误码的按拒绝请求。以前错误帧一律按拒绝请求，服务过载或超时也提示用户去检查配置（#9886）。
     */
    public static HttpAsrRecognizer.Failure doubaoError(Integer code) {
        if (code == null) return HttpAsrRecognizer.Failure.SERVICE;
        if (code == 45_000_002) return HttpAsrRecognizer.Failure.EMPTY;
        if (code == 45_000_081) return HttpAsrRecognizer.Failure.SERVICE_TIMEOUT;
        if (code >= 55_000_000 && code <= 55_099_999) return HttpAsrRecognizer.Failure.BUSY;
        return HttpAsrRecognizer.Failure.SERVICE;
    }

    /**
     * 豆包在最终结果之前发来关闭帧时的原因，{@code status} 是 {@link WebSocketFrames#closeStatus} 读出的状态码。没带状态码、1000 正常关闭和 1001 离开，都是服务端没报错就结束了会话，多半是它那一侧的会话时长或空闲限制；1009 是消息超出限制；1011–1014 是服务端故障或重启，稍后重试；其余（协议错误、策略拒绝等）按拒绝请求。
     */
    public static HttpAsrRecognizer.Failure closed(int status) {
        return switch (status) {
            case -1, 1000, 1001 -> HttpAsrRecognizer.Failure.ENDED;
            case 1009 -> HttpAsrRecognizer.Failure.LIMIT;
            case 1011, 1012, 1013, 1014 -> HttpAsrRecognizer.Failure.BUSY;
            default -> HttpAsrRecognizer.Failure.SERVICE;
        };
    }

    /**
     * 豆包流式已经识别出一部分文字之后连接才断：文字照常交出去，再用这句说明后面没识别完的原因。取消、空结果和权限这类不会发生在识别途中的原因返回 null。
     */
    public static String interrupted(HttpAsrRecognizer.Failure failure) {
        return switch (failure) {
            case NETWORK -> "网络中断，已保留识别到的文字";
            case TIMEOUT, SERVICE_TIMEOUT -> "语音服务响应超时，已保留识别到的文字";
            case SERVICE, LIMIT, BUSY -> "语音服务中止了识别，已保留识别到的文字";
            case ENDED -> "语音服务提前结束，已保留识别到的文字";
            case UNAVAILABLE -> "麦克风中断，已保留识别到的文字";
            case PERMISSION, EMPTY, CANCELLED -> null;
        };
    }

    /**
     * 录音到了单次上限、由识别器自己停下时的提示，例如「单次语音最长 60 秒，已识别前 60 秒」。以前到点悄悄截断，用户不知道后半段没录进去。整分钟且超过一分钟时按分钟说。
     */
    public static String capped(long maxMillis) {
        long seconds = Math.max(1, maxMillis / 1000);
        String span = seconds > 60 && seconds % 60 == 0 ? seconds / 60 + " 分钟" : seconds + " 秒";
        return "单次语音最长 " + span + "，已识别前 " + span;
    }
}
