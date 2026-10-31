package app.msime.android;

/** 按键音效缓存同时绑定包 ID 与来源目录，切换输入会话后不能复用旧目录的样本。 */
public final class KeySoundLifecyclePolicySmoke {
    public static void main(String[] args) {
        check(KeySoundLifecyclePolicy.needsReload("default", "/prefs/a", "default", "/prefs/a") == false,
            "the same pack from the same source stays loaded");
        check(KeySoundLifecyclePolicy.needsReload("default", "/prefs/a", "default", "/prefs/b"),
            "the same pack from a different preferences directory reloads");
        check(KeySoundLifecyclePolicy.needsReload("default", "/prefs/a", "typewriter", "/prefs/a"),
            "a different pack reloads even when the source is unchanged");
        check(KeySoundLifecyclePolicy.needsReload(null, null, "default", "/prefs/a"),
            "missing loaded sound state reloads");
        check(!KeySoundLifecyclePolicy.acceptsLoaded("default", "/prefs/a", "default", ""),
            "a loaded pack is not accepted when the requested source is missing");
        System.out.println("Android key sound lifecycle policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
