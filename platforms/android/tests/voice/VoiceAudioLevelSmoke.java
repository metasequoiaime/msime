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
        // 豆包与整句上传读的是 16 位小端字节，换算要和 short 采样完全一致。
        check(VoiceAudioLevel.fromPcm16le(new byte[] {(byte) 0xcb, 0x0c, 0x35, (byte) 0xf3}, 4)
            == VoiceAudioLevel.fromPcm(new short[] {3275, -3275}, 2), "字节采样与 short 采样同一换算");
        check(VoiceAudioLevel.fromPcm16le(new byte[] {0x00, (byte) 0x80}, 2) == 1, "字节里的最大负采样不溢出");
        check(VoiceAudioLevel.fromPcm16le(new byte[] {0x00, 0x40, 0x00}, 3)
            == VoiceAudioLevel.fromPcm(new short[] {16384}, 1), "末尾不成对的字节不计");
        check(VoiceAudioLevel.fromPcm16le(new byte[3200], 3200) == 0, "字节静音不显示信号");
        check(VoiceAudioLevel.fromPcm16le(null, 2) == 0 && VoiceAudioLevel.fromPcm16le(new byte[] {1, 1}, 1) == 0
            && VoiceAudioLevel.fromPcm16le(new byte[] {1, 1}, -1) == 0, "空、半个或失败的字节读取不显示信号");
        System.out.println("VoiceAudioLevelSmoke passed");
    }
}
