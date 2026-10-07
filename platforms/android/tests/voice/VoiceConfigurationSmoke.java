package app.msime.android;

/** 键盘不能把用户要求的本地识别静默切换到系统服务。 */
public final class VoiceConfigurationSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        try {
            java.lang.reflect.Method strictBoolean = VoiceConfiguration.class.getDeclaredMethod(
                "strictBoolean", Object.class);
            strictBoolean.setAccessible(true);
            check(Boolean.TRUE.equals(strictBoolean.invoke(null, Boolean.TRUE)),
                "voice configuration accepts JSON booleans");
            check(strictBoolean.invoke(null, "true") == null,
                "voice configuration rejects boolean strings instead of coercing them");
            java.lang.reflect.Method strictString = VoiceConfiguration.class.getDeclaredMethod(
                "strictString", Object.class);
            strictString.setAccessible(true);
            check("synthetic".equals(strictString.invoke(null, "synthetic")),
                "voice configuration accepts JSON strings");
            check(strictString.invoke(null, 7) == null,
                "voice configuration rejects numeric strings instead of coercing them");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("voice configuration response policy missing", error);
        }
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
