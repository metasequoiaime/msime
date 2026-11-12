import android.content.SharedPreferences;
import app.msime.android.ClipboardCaptureState;
import app.msime.android.ClipboardModePolicy;

/** 访客模式的剪贴板补读去重状态必须与主模式分开。 */
public final class ClipboardCaptureStateSmoke {
    private static final class MemoryPreferences implements SharedPreferences {
        private final java.util.Map<String, String> values = new java.util.HashMap<>();

        @Override public String getString(String key, String fallback) {
            return values.getOrDefault(key, fallback);
        }
        @Override public Editor edit() { return new Editor() {
            private final java.util.Map<String, String> pending = new java.util.HashMap<>();
            @Override public Editor putString(String key, String value) {
                pending.put(key, value); return this;
            }
            @Override public boolean commit() { apply(); return true; }
            @Override public void apply() { values.putAll(pending); }
            @Override public Editor clear() { values.clear(); return this; }
            @Override public Editor remove(String key) { values.remove(key); return this; }
            @Override public Editor putStringSet(String key, java.util.Set<String> value) { return this; }
            @Override public Editor putInt(String key, int value) { return this; }
            @Override public Editor putLong(String key, long value) { return this; }
            @Override public Editor putFloat(String key, float value) { return this; }
            @Override public Editor putBoolean(String key, boolean value) { return this; }
        }; }
        @Override public java.util.Map<String, ?> getAll() { return new java.util.HashMap<>(values); }
        @Override public java.util.Set<String> getStringSet(String key, java.util.Set<String> fallback) { return fallback; }
        @Override public int getInt(String key, int fallback) { return fallback; }
        @Override public long getLong(String key, long fallback) { return fallback; }
        @Override public float getFloat(String key, float fallback) { return fallback; }
        @Override public boolean getBoolean(String key, boolean fallback) { return fallback; }
        @Override public boolean contains(String key) { return values.containsKey(key); }
        @Override public void registerOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {}
        @Override public void unregisterOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {}
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        MemoryPreferences preferences = new MemoryPreferences();
        ClipboardCaptureState.remember(preferences, ClipboardModePolicy.Mode.MAIN, "same-clip");
        check("same-clip".equals(ClipboardCaptureState.handledIdentity(
            preferences, ClipboardModePolicy.Mode.MAIN)), "main identity is retained");
        check(ClipboardCaptureState.handledIdentity(
            preferences, ClipboardModePolicy.Mode.VISITOR) == null,
            "visitor mode starts with an independent identity");
        ClipboardCaptureState.remember(preferences, ClipboardModePolicy.Mode.VISITOR, "same-clip");
        check("same-clip".equals(ClipboardCaptureState.handledIdentity(
            preferences, ClipboardModePolicy.Mode.VISITOR)), "visitor identity is retained");
        check("same-clip".equals(ClipboardCaptureState.handledIdentity(
            preferences, ClipboardModePolicy.Mode.MAIN)), "main identity remains independent");
        MemoryPreferences legacy = new MemoryPreferences();
        legacy.edit().putString("handled_clip", "legacy-clip").apply();
        check("legacy-clip".equals(ClipboardCaptureState.handledIdentity(
            legacy, ClipboardModePolicy.Mode.MAIN)), "main mode reads the legacy identity");
        System.out.println("ClipboardCaptureStateSmoke ok");
    }
}
