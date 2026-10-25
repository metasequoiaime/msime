import app.msime.android.VoiceAudioLevel;

/** 用合成采样验证聆听动画的实际音量，不依赖麦克风或设备音频。 */
public final class VoiceAudioLevelSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(VoiceAudioLevel.fromPcm(new short[1600], 1600) == 0, "静音不显示信号");
        float quiet = VoiceAudioLevel.fromPcm(new short[] {328, -328}, 2);
        float loud = VoiceAudioLevel.fromPcm(new short[] {3277, -3277}, 2);
        check(quiet > 0 && loud > quiet && loud < 1, "信号随实际幅度增长");
        check(VoiceAudioLevel.fromPcm(new short[] {16384}, 1)
            == VoiceAudioLevel.fromPcm(new short[] {-16384}, 1), "音量不随采样符号改变");
        check(VoiceAudioLevel.fromPcm(new short[] {Short.MIN_VALUE}, 1) == 1,
            "最大负采样不溢出");
        check(VoiceAudioLevel.fromPcm(new short[] {Short.MIN_VALUE, Short.MAX_VALUE}, 2) > .999f,
            "满幅交替采样保持有界");
        check(VoiceAudioLevel.fromPcm(new short[] {0, 0, Short.MIN_VALUE}, 2) == 0,
            "忽略本次读取之外的缓冲尾部");
        check(VoiceAudioLevel.fromPcm(null, 2) == 0, "空采样不显示信号");
        check(VoiceAudioLevel.fromPcm(new short[] {Short.MIN_VALUE}, 0) == 0,
            "零长度读取不显示信号");
        check(VoiceAudioLevel.fromPcm(new short[] {Short.MIN_VALUE}, -1) == 0,
            "失败读取不显示信号");
        check(VoiceAudioLevel.fromPcm(new short[] {Short.MIN_VALUE}, 2000) == 1,
            "长度越界仍只读取可用采样");
        System.out.println("VoiceAudioLevelSmoke passed");
    }
}
