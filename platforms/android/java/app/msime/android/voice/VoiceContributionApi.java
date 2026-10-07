package app.msime.android;

import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 语音贡献：`POST /v1/voice/contributions`，multipart 的 `payload` 是 `{language,provider,duration_ms,transcript,app_version}`，`audio` 是不超过 2 MiB、60 秒的 WAV。服务端每用户每小时 30 次，保存 180 天。
 *
 * <p>只在用户打开 `voice_input.contribute_audio`（开启前已确认上传内容与保存期限）、隐私判断允许（不在隐私模式、不是不做个性化学习的输入框）且不是密码框时由键盘调用。用设备的匿名账号会话鉴权，`:ime` 进程里令牌经 AccountSessionProvider 从主进程取。本类不 import `androidx`、`R` 或 `home/`；请求阻塞在网络上，不要在主线程调用。
 */
public final class VoiceContributionApi {
    public static final String PATH = "/v1/voice/contributions";
    public static final int MAX_AUDIO_BYTES = 2 * 1024 * 1024;
    public static final long MAX_DURATION_MILLIS = 60_000;
    public static final int MAX_TRANSCRIPT = 2000;
    /** Maximum UTF-8 byte length of metadata fields carried in a contribution. */
    public static final int MAX_METADATA_FIELD_LENGTH = 64;

    /** 一次贡献：识别语言、识别器、时长、识别文本、应用版本与 WAV 音频。 */
    public record Contribution(String language, String provider, long durationMillis, String transcript,
                               String appVersion, byte[] wav) {}

    private final CloudApi api;

    public VoiceContributionApi(CloudApi api) {
        this.api = api;
    }

    /** WAV 的文件头（RIFF / WAVE）与大小都合规、时长在 (0, 60 s]、识别文本非空且不超长时才上传。 */
    public static boolean valid(Contribution contribution) {
        if (contribution == null) return false;
        byte[] wav = contribution.wav();
        if (wav == null || wav.length <= WavAudio.HEADER_BYTES || wav.length > MAX_AUDIO_BYTES) return false;
        if (wav[0] != 'R' || wav[1] != 'I' || wav[2] != 'F' || wav[3] != 'F'
                || wav[8] != 'W' || wav[9] != 'A' || wav[10] != 'V' || wav[11] != 'E') return false;
        if (contribution.durationMillis() <= 0 || contribution.durationMillis() > MAX_DURATION_MILLIS) return false;
        String transcript = contribution.transcript();
        if (transcript == null || TextPolicy.trimmed(transcript).isEmpty()) return false;
        if (!TextPolicy.withinCodePoints(transcript, MAX_TRANSCRIPT)
                || TextPolicy.hasControlExceptWhitespace(transcript)
                || !TextPolicy.validUnicode(transcript)) return false;
        return nonEmpty(contribution.language()) && nonEmpty(contribution.provider())
            && nonEmpty(contribution.appVersion());
    }

    /** PCM 的时长（毫秒）：16 kHz 单声道 16 位。 */
    public static long durationMillis(int pcmBytes) {
        return BoundsPolicy.nonNegative(pcmBytes) / 2L * 1000L / WavAudio.SAMPLE_RATE;
    }

    private static boolean nonEmpty(String value) {
        return value != null && !value.isEmpty()
            && TextPolicy.utf8Length(value) <= MAX_METADATA_FIELD_LENGTH
            && !TextPolicy.hasControl(value) && TextPolicy.validUnicode(value);
    }

    /** 上传一次贡献，返回服务端给的 id。不合规的贡献直接拒绝，不发请求。 */
    public String upload(Contribution contribution) throws CloudApi.Failure {
        if (!valid(contribution)) throw new IllegalArgumentException("invalid voice contribution");
        final String payload;
        try {
            payload = new JSONObject()
                .put("language", contribution.language())
                .put("provider", contribution.provider())
                .put("duration_ms", contribution.durationMillis())
                .put("transcript", contribution.transcript())
                .put("app_version", contribution.appVersion())
                .toString();
        } catch (JSONException error) {
            throw new IllegalArgumentException("invalid voice contribution", error);
        }
        JSONObject response = api.multipart(PATH, List.of(
            CloudApi.Part.json("payload", payload),
            CloudApi.Part.file("audio", "voice.wav", "audio/wav", contribution.wav())),
            CloudApi.Auth.ANONYMOUS);
        String id = strictString(response.opt("id"));
        return id == null ? "" : id;
    }

    /** org.json's optString coerces numbers; contribution identifiers must stay JSON strings. */
    static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }
}
