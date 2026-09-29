package app.msime.android;

import java.util.HashMap;
import java.util.Map;

public final class InputModeStoreSmoke {
    private static final class FakePreferences implements InputModeStore.Persistence {
        private final Map<String, Boolean> values = new HashMap<>();
        public boolean contains(String key) { return values.containsKey(key); }
        public boolean getBoolean(String key, boolean fallback) { return values.getOrDefault(key, fallback); }
        public void putBoolean(String key, boolean value) { values.put(key, value); }
    }

    public static void main(String[] args) {
        FakePreferences preferences = new FakePreferences();
        InputModeStore store = new InputModeStore(preferences);
        if (!store.modeFor("app", "com.example.editor", false)) { }
        else throw new AssertionError("default mode");
        store.remember("app", "com.example.editor", true);
        if (!store.modeFor("app", "com.example.editor", false)) throw new AssertionError("app mode");
        if (store.modeFor("app", "com.other.editor", false)) throw new AssertionError("app isolation");
        store.remember("global", "com.example.editor", true);
        if (!store.modeFor("global", "com.other.editor", false)) throw new AssertionError("global mode");
        store.remember("app", "bad/name", true);
        if (store.modeFor("app", "bad/name", false)) throw new AssertionError("invalid package");
    }
}
