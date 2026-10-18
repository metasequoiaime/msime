package app.msime.android;

import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;

/**
 * 语音贡献：一段不超过 2 MiB、60 秒的 WAV 和它的识别文本，由 Rust 的 voice_contribution 上传（`POST /v1/voice/contributions`，时长由 Rust 从音频算出）。服务端每用户每小时 30 次，保存 180 天。
 *
 * <p>只在用户打开 `voice_input.contribute_audio`（开启前已确认上传内容与保存期限）、隐私判断允许（不在隐私模式、不是不做个性化学习的输入框）且不是密码框时由键盘调用。只用设备的匿名账号；会话在主进程里由 Rust 持有（#8201 迁移计划 5b），所以音频先写进 cache 下的私有目录，再经 {@link AccountSessionProvider} 交给主进程上传，`:ime` 自己不拿令牌。本类不 import `androidx`、`R` 或 `home/`；调用阻塞在网络上，不要在主线程调用。
 */
public final class VoiceContributionApi {
    public static final int MAX_AUDIO_BYTES = 2 * 1024 * 1024;
    public static final long MAX_DURATION_MILLIS = 60_000;
    public static final int MAX_TRANSCRIPT = 2000;
    /** Maximum UTF-8 byte length of metadata fields carried in a contribution. */
    public static final int MAX_METADATA_FIELD_LENGTH = 64;

    /** 一次贡献：识别语言、识别器、时长、识别文本、应用版本与 WAV 音频。 */
    public record Contribution(String language, String provider, long durationMillis, String transcript,
                               String appVersion, byte[] wav) {}

    private final Context application;

    public VoiceContributionApi(Context context) {
        this.application = context.getApplicationContext();
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
        if (!TextPolicy.validBoundedText(transcript, MAX_TRANSCRIPT)) return false;
        return nonEmpty(contribution.language()) && nonEmpty(contribution.provider())
            && nonEmpty(contribution.appVersion());
    }

    /** PCM 的时长（毫秒）：16 kHz 单声道 16 位。 */
    public static long durationMillis(int pcmBytes) {
        return BoundsPolicy.nonNegative(pcmBytes) / 2L * 1000L / WavAudio.SAMPLE_RATE;
    }

    private static boolean nonEmpty(String value) {
        return TextPolicy.boundedNonEmpty(value, MAX_METADATA_FIELD_LENGTH);
    }

    /** 上传一次贡献，返回服务端给的 id。不合规的贡献直接拒绝，不发请求；上传失败时抛出，消息是 Rust 的错误码。音频文件调用结束就删掉。 */
    public String upload(Contribution contribution) throws IOException {
        if (!valid(contribution)) throw new IllegalArgumentException("invalid voice contribution");
        File directory = new File(application.getCacheDir(), AccountSessionRoutingPolicy.VOICE_CONTRIBUTION_DIRECTORY);
        if (!directory.isDirectory() && !directory.mkdirs() && !directory.isDirectory())
            throw new IOException("voice contribution directory");
        File audio = File.createTempFile("voice-", ".wav", directory);
        try {
            try (OutputStream output = new FileOutputStream(audio)) {
                output.write(contribution.wav());
            }
            Bundle extras = new Bundle();
            extras.putString(AccountSessionRoutingPolicy.KEY_VOICE_AUDIO_PATH, audio.getAbsolutePath());
            extras.putString(AccountSessionRoutingPolicy.KEY_VOICE_LANGUAGE, contribution.language());
            extras.putString(AccountSessionRoutingPolicy.KEY_VOICE_PROVIDER, contribution.provider());
            extras.putString(AccountSessionRoutingPolicy.KEY_VOICE_TRANSCRIPT, contribution.transcript());
            extras.putString(AccountSessionRoutingPolicy.KEY_VOICE_APP_VERSION, contribution.appVersion());
            Uri uri = Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName()));
            Bundle reply = application.getContentResolver().call(
                uri, AccountSessionRoutingPolicy.METHOD_VOICE_CONTRIBUTION, null, extras);
            if (reply == null) throw new IllegalStateException("account_unavailable");
            String failure = reply.getString(AccountSessionRoutingPolicy.KEY_ERROR);
            if (failure != null && !failure.isEmpty()) throw new IllegalStateException(failure);
            String id = reply.getString(AccountSessionRoutingPolicy.KEY_VOICE_ID);
            return id == null ? "" : id;
        } finally {
            // 主进程读完就删了；它没读到（provider 连不上、路径被拒）时在这里删。
            if (audio.exists()) audio.delete();
        }
    }

}
