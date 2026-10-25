package app.msime.android;

/** 把本次实际读取的 16 位 PCM 幅度映射为聆听界面的 0–1 音量。 */
public final class VoiceAudioLevel {
    private VoiceAudioLevel() { }

    /** RMS 低于 −60 dBFS 时视作静音，满幅为 1；不保存采样。 */
    public static float fromPcm(short[] samples, int count) {
        if (samples == null || count <= 0) return 0;
        int length = Math.min(count, samples.length);
        if (length == 0) return 0;
        double squares = 0;
        for (int index = 0; index < length; index++) {
            double sample = samples[index];
            squares += sample * sample;
        }
        double rms = Math.sqrt(squares / length) / 32768.0;
        if (rms <= .001) return 0;
        return (float) Math.max(0, Math.min(1, (20 * Math.log10(rms) + 60) / 60));
    }
}
