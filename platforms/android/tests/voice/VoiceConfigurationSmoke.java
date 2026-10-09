package app.msime.android;

/** 键盘不能把用户要求的本地识别静默切换到系统服务。 */
public final class VoiceConfigurationSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)),
            "voice configuration accepts JSON booleans");
        check(JsonPolicy.strictBoolean("true") == null,
            "voice configuration rejects boolean strings instead of coercing them");
        check("synthetic".equals(JsonPolicy.strictStringOrEmpty("synthetic")),
            "voice configuration accepts JSON strings");
        check("".equals(JsonPolicy.strictStringOrEmpty(7)),
            "voice configuration rejects numeric strings instead of coercing them");
        VoiceConfiguration configuration = VoiceConfiguration.fromProvider(
            "local", "", null);
        check("local".equals(configuration.provider()),
            "an unavailable local model must remain identifiable as local");

        VoiceConfiguration usable = VoiceConfiguration.fromProvider(
            "local", "/data/user/0/app/files/model", null);
        check("local".equals(usable.provider()) && usable.localModel() != null,
            "an absolute local model remains usable");
        System.out.println("Android voice configuration keeps unusable local recognition local");
    }
}
