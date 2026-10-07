import app.msime.android.VoiceCapturePolicy;

/** Bounds one microphone read to the capture units still allowed by the recording cap. */
public final class VoiceCapturePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(VoiceCapturePolicy.readLength(960, 0, 320) == 320,
            "a full buffer is used while the cap has room");
        check(VoiceCapturePolicy.readLength(960, 640, 320) == 320,
            "an exact final buffer stays unchanged");
        check(VoiceCapturePolicy.readLength(960, 800, 320) == 160,
            "the final read is bounded to the remaining bytes");
        check(VoiceCapturePolicy.readLength(960, 960, 320) == 0,
            "no read is requested after the cap");
        check(VoiceCapturePolicy.readLength(960, 1000, 320) == 0,
            "an already exceeded cap cannot read more");
        check(VoiceCapturePolicy.readLength(960, 0, 0) == 0,
            "a zero-sized buffer cannot be read");
        check(VoiceCapturePolicy.readLength(0, 0, 320) == 0,
            "a zero cap cannot be read");
        check(VoiceCapturePolicy.readLength(5_000_000_000L, 4_999_999_900L, 320) == 100,
            "long sample counters keep the final read bounded");
        System.out.println("VoiceCapturePolicySmoke passed");
    }
}
