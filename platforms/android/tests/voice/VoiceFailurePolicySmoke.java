package app.msime.android;

/** 云端识别失败的提示、诊断类别和到上限的提示（#9886），键盘与识别窗口共用这一份。 */
public final class VoiceFailurePolicySmoke {
    public static void main(String[] args) {
        // 沿用 #9939 的提示文字，豆包流式与整句上传一致。
        check("语音服务未响应，请检查网络与密钥".equals(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.NETWORK)),
            "网络失败提示");
        check("语音服务请求超时，请稍后重试".equals(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.TIMEOUT)),
            "本机超时提示");
        check("语音服务请求超时，请稍后重试".equals(
            VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.SERVICE_TIMEOUT)), "服务超时提示");
        check("语音服务拒绝请求，请检查服务配置".equals(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.SERVICE)),
            "服务拒绝提示");
        check("没有听到内容".equals(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.EMPTY)), "空结果提示");
        check(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.CANCELLED) == null, "取消不提示");

        // 本机超时以前记成网络错误，和提示的「请求超时」对不上。
        check(VoiceFailurePolicy.diagnostic(HttpAsrRecognizer.Failure.TIMEOUT)
            == DiagnosticEventKind.VOICE_FAILURE_TIMEOUT, "本机超时记超时");
        check(VoiceFailurePolicy.diagnostic(HttpAsrRecognizer.Failure.NETWORK)
            == DiagnosticEventKind.VOICE_FAILURE_NETWORK, "网络失败记网络");
        check(VoiceFailurePolicy.diagnostic(HttpAsrRecognizer.Failure.SERVICE_TIMEOUT)
            == DiagnosticEventKind.VOICE_FAILURE_RESPONSE, "服务端回的 408/504 记回应失败");
        check(VoiceFailurePolicy.diagnostic(HttpAsrRecognizer.Failure.EMPTY)
            == DiagnosticEventKind.VOICE_EMPTY, "空结果记空");
        for (HttpAsrRecognizer.Failure failure : HttpAsrRecognizer.Failure.values()) {
            check(VoiceFailurePolicy.diagnostic(failure).wire().startsWith("voice_"), "只记语音类别");
            String message = VoiceFailurePolicy.message(failure);
            check(message == null || message.length() <= PlatformSpeechPolicy.MAX_LENGTH, "提示放得进两行 Toast");
            String interrupted = VoiceFailurePolicy.interrupted(failure);
            check(interrupted == null || interrupted.length() <= PlatformSpeechPolicy.MAX_LENGTH,
                "中断提示放得进两行 Toast");
        }

        // 识别到一半断开：文字保留，提示里分得清断网、超时和服务中止。
        String network = VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.NETWORK);
        String timeout = VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.TIMEOUT);
        String service = VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.SERVICE);
        check(network != null && timeout != null && service != null, "途中断开都有提示");
        check(!network.equals(timeout) && !timeout.equals(service) && !network.equals(service),
            "断网、超时和服务中止的提示各不相同");
        check(network.contains("已保留"), "说明文字已保留");
        check(VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.CANCELLED) == null, "取消不提示");
        check(VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.EMPTY) == null, "空结果不是途中断开");

        // 到上限的提示。
        check("单次语音最长 60 秒，已识别前 60 秒".equals(VoiceFailurePolicy.capped(HttpAsrRecognizer.MAX_MILLIS)),
            "整句上传的上限按秒说");
        check("单次语音最长 5 分钟，已识别前 5 分钟".equals(VoiceFailurePolicy.capped(DoubaoRecognizer.MAX_MILLIS)),
            "豆包流式的上限按分钟说");
        check("单次语音最长 90 秒，已识别前 90 秒".equals(VoiceFailurePolicy.capped(90_000)), "不是整分钟时按秒说");
        check(HttpAsrRecognizer.MAX_MILLIS == 60_000, "整句上传仍是 60 秒");
        check(DoubaoRecognizer.MAX_MILLIS == 300_000, "豆包流式放宽到 5 分钟");

        // 被拒的握手按状态码归类，读不出状态码是连接在握手中途断了。
        check(VoiceFailurePolicy.handshake("HTTP/1.1 401 Unauthorized\r\n\r\n") == HttpAsrRecognizer.Failure.SERVICE,
            "鉴权被拒");
        check(VoiceFailurePolicy.handshake("HTTP/1.1 504 Gateway Timeout\r\n\r\n")
            == HttpAsrRecognizer.Failure.SERVICE_TIMEOUT, "网关超时");
        check(VoiceFailurePolicy.handshake("HTTP/1.1 413 Payload Too Large\r\n\r\n") == HttpAsrRecognizer.Failure.LIMIT,
            "超出限制");
        check(VoiceFailurePolicy.handshake("HTTP/1.1 101 Switching Protocols\r\n\r\n")
            == HttpAsrRecognizer.Failure.SERVICE, "101 却对不上校验值");
        check(VoiceFailurePolicy.handshake("") == HttpAsrRecognizer.Failure.NETWORK, "握手中途断开");
        check(VoiceFailurePolicy.status(408) == HttpAsrRecognizer.Failure.SERVICE_TIMEOUT, "408 是服务超时");
        check(VoiceFailurePolicy.status(500) == HttpAsrRecognizer.Failure.SERVICE, "其余按拒绝");

        // 豆包错误帧按错误码归类：服务过载、等包超时和空音频不能提示用户去检查配置。
        check(VoiceFailurePolicy.doubaoError(55_000_031) == HttpAsrRecognizer.Failure.BUSY, "服务过载");
        check(VoiceFailurePolicy.doubaoError(55_000_000) == HttpAsrRecognizer.Failure.BUSY, "服务内部错误的下界");
        check(VoiceFailurePolicy.doubaoError(55_099_999) == HttpAsrRecognizer.Failure.BUSY, "服务内部错误的上界");
        check(VoiceFailurePolicy.doubaoError(45_000_081) == HttpAsrRecognizer.Failure.SERVICE_TIMEOUT, "等包超时");
        check(VoiceFailurePolicy.doubaoError(45_000_002) == HttpAsrRecognizer.Failure.EMPTY, "空音频");
        check(VoiceFailurePolicy.doubaoError(45_000_001) == HttpAsrRecognizer.Failure.SERVICE, "请求参数无效");
        check(VoiceFailurePolicy.doubaoError(45_000_151) == HttpAsrRecognizer.Failure.SERVICE, "音频格式不对");
        check(VoiceFailurePolicy.doubaoError(null) == HttpAsrRecognizer.Failure.SERVICE, "读不出错误码");

        // 最终结果之前的关闭帧按状态码归类：没报错的正常关闭是服务结束了会话（时长或空闲限制）。
        check(VoiceFailurePolicy.closed(-1) == HttpAsrRecognizer.Failure.ENDED, "没带状态码");
        check(VoiceFailurePolicy.closed(1000) == HttpAsrRecognizer.Failure.ENDED, "正常关闭");
        check(VoiceFailurePolicy.closed(1001) == HttpAsrRecognizer.Failure.ENDED, "服务端离开");
        check(VoiceFailurePolicy.closed(1009) == HttpAsrRecognizer.Failure.LIMIT, "消息超出限制");
        for (int status : new int[] {1011, 1012, 1013, 1014}) {
            check(VoiceFailurePolicy.closed(status) == HttpAsrRecognizer.Failure.BUSY, "服务端故障 " + status);
        }
        check(VoiceFailurePolicy.closed(1008) == HttpAsrRecognizer.Failure.SERVICE, "策略拒绝");
        check(VoiceFailurePolicy.closed(1002) == HttpAsrRecognizer.Failure.SERVICE, "协议错误");

        // 没有文字时，断网、超时、服务结束会话、服务故障、拒绝请求和空回复的提示各不相同（#9886 的验收）。
        java.util.Set<String> distinct = new java.util.HashSet<>();
        for (HttpAsrRecognizer.Failure failure : new HttpAsrRecognizer.Failure[] {
                HttpAsrRecognizer.Failure.NETWORK, HttpAsrRecognizer.Failure.TIMEOUT,
                HttpAsrRecognizer.Failure.ENDED, HttpAsrRecognizer.Failure.BUSY,
                HttpAsrRecognizer.Failure.SERVICE, HttpAsrRecognizer.Failure.EMPTY}) {
            check(distinct.add(VoiceFailurePolicy.message(failure)), "提示重复：" + failure);
        }
        check(VoiceFailurePolicy.message(HttpAsrRecognizer.Failure.ENDED).contains("时长"), "服务结束会话时说出时长限制");
        check(VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.ENDED) != null
            && VoiceFailurePolicy.interrupted(HttpAsrRecognizer.Failure.BUSY) != null, "途中结束也有提示");
        System.out.println("VoiceFailurePolicySmoke passed");
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
