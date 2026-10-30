package app.msime.android;

/** 按键音效的缓存身份同时包含包 ID 和来源目录。 */
public final class KeySoundLifecyclePolicy {
    private KeySoundLifecyclePolicy() {}

    public static boolean acceptsLoaded(String loadedPack, String loadedSource,
                                        String requestedPack, String requestedSource) {
        return nonEmpty(loadedPack) && nonEmpty(loadedSource)
            && loadedPack.equals(requestedPack) && loadedSource.equals(requestedSource);
    }

    public static boolean needsReload(String loadedPack, String loadedSource,
                                      String requestedPack, String requestedSource) {
        return !acceptsLoaded(loadedPack, loadedSource, requestedPack, requestedSource);
    }

    private static boolean nonEmpty(String value) {
        return value != null && !value.isEmpty();
    }
}
