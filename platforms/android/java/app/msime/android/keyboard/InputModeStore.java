package app.msime.android;

import android.content.SharedPreferences;

/** Persists the manual Chinese/English mode without storing editor contents. */
public final class InputModeStore {
    public interface Persistence {
        boolean contains(String key);
        boolean getBoolean(String key, boolean fallback);
        void putBoolean(String key, boolean value);
    }
    private static final String GLOBAL_KEY = "global";
    private static final String APP_PREFIX = "app:";
    private static final int MAX_PACKAGE_LENGTH = 256;

    private final Persistence preferences;

    public InputModeStore(Persistence preferences) {
        this.preferences = preferences;
    }

    public static InputModeStore from(SharedPreferences source) {
        return new InputModeStore(new Persistence() {
            public boolean contains(String key) { return source.contains(key); }
            public boolean getBoolean(String key, boolean fallback) {
                return source.getBoolean(key, fallback);
            }
            public void putBoolean(String key, boolean value) {
                source.edit().putBoolean(key, value).apply();
            }
        });
    }

    public boolean modeFor(String scope, String packageName, boolean defaultEnglish) {
        String key = "global".equals(scope) ? GLOBAL_KEY : appKey(packageName);
        if (key == null || !preferences.contains(key)) return defaultEnglish;
        return preferences.getBoolean(key, defaultEnglish);
    }

    public void remember(String scope, String packageName, boolean english) {
        String key = "global".equals(scope) ? GLOBAL_KEY : appKey(packageName);
        if (key != null) preferences.putBoolean(key, english);
    }

    private static String appKey(String packageName) {
        if (packageName == null || packageName.length() == 0 || packageName.length() > MAX_PACKAGE_LENGTH)
            return null;
        for (int index = 0; index < packageName.length(); index++) {
            char value = packageName.charAt(index);
            if (!(value == '.' || value == '_' || value == '-' || value >= 'a' && value <= 'z'
                    || value >= 'A' && value <= 'Z' || value >= '0' && value <= '9')) return null;
        }
        return APP_PREFIX + packageName;
    }
}
