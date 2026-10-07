package app.msime.android;

/** 语音贡献的本地校验：WAV 头、大小、时长与字段。 */
public final class VoiceContributionApiSmoke {
    public static void main(String[] arguments) {
        byte[] pcm = new byte[32_000];
        byte[] wav = WavAudio.wrap(pcm, pcm.length, WavAudio.SAMPLE_RATE);
        check(wav != null, "wav wraps");
        check(VoiceContributionApi.durationMillis(pcm.length) == 1000, "one second of 16 kHz mono");
        VoiceContributionApi.Contribution ok = new VoiceContributionApi.Contribution("zh-CN", "local", 1000,
            "今天下午三点开会。", "1.2.3", wav);
        check(VoiceContributionApi.valid(ok), "a well-formed contribution passes");
        check(!VoiceContributionApi.valid(null), "nothing");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN", "local", 1000, " ", "1.2.3", wav)),
            "an empty transcript is not uploaded");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN", "local", 61_000, "好", "1.2.3", wav)),
            "longer than a minute is refused");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN", "local", 1000, "好", "1.2.3",
            new byte[VoiceContributionApi.MAX_AUDIO_BYTES + 1])), "more than 2 MiB is refused");
        byte[] notWav = wav.clone();
        notWav[0] = 'X';
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN", "local", 1000, "好", "1.2.3", notWav)),
            "only WAV audio is sent");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("", "local", 1000, "好", "1.2.3", wav)),
            "a language is required");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN", "local", 1000,
            "好\u0000", "1.2.3", wav)), "control characters are refused in transcripts");
        check(!VoiceContributionApi.valid(new VoiceContributionApi.Contribution("zh-CN\uD800", "local", 1000,
            "好", "1.2.3", wav)), "unpaired surrogates are refused in metadata");
        check(VoiceContributionApi.PATH.equals("/v1/voice/contributions"), "the contribution endpoint");
        try {
            java.lang.reflect.Method strictString = VoiceContributionApi.class.getDeclaredMethod(
                "strictString", Object.class);
            strictString.setAccessible(true);
            check("synthetic".equals(strictString.invoke(null, "synthetic")),
                "voice contribution response ids accept strings");
            check(strictString.invoke(null, 7) == null,
                "voice contribution response ids reject numbers instead of coercing them");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("voice contribution response string policy missing", error);
        }
        System.out.println("Android voice contribution passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
