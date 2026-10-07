package app.msime.android;

import android.content.SharedPreferences;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** 冒烟用的内存版 SharedPreferences：android.jar 里只有抛 `Stub!` 的桩，要测读写偏好的逻辑就用它代替。编辑在 apply / commit 时一次生效，与真实实现的内存语义一致。 */
final class MemoryPreferences implements SharedPreferences {
    private final Map<String, Object> values = new HashMap<>();

    @Override public synchronized Map<String, ?> getAll() { return new HashMap<>(values); }

    @Override public synchronized String getString(String key, String fallback) {
        Object value = values.get(key);
        return value instanceof String text ? text : fallback;
    }

    @Override public synchronized Set<String> getStringSet(String key, Set<String> fallback) {
        Object value = values.get(key);
        if (!(value instanceof Set<?> set)) return fallback;
        Set<String> copy = new HashSet<>();
        for (Object item : set) copy.add((String) item);
        return copy;
    }

    @Override public synchronized int getInt(String key, int fallback) {
        Object value = values.get(key);
        return value instanceof Integer number ? number : fallback;
    }

    @Override public synchronized long getLong(String key, long fallback) {
        Object value = values.get(key);
        return value instanceof Long number ? number : fallback;
    }

    @Override public synchronized float getFloat(String key, float fallback) {
        Object value = values.get(key);
        return value instanceof Float number ? number : fallback;
    }

    @Override public synchronized boolean getBoolean(String key, boolean fallback) {
        Object value = values.get(key);
        return value instanceof Boolean flag ? flag : fallback;
    }

    @Override public synchronized boolean contains(String key) { return values.containsKey(key); }

    @Override public Editor edit() { return new MemoryEditor(); }

    @Override public void registerOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {}

    @Override public void unregisterOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {}

    private final class MemoryEditor implements Editor {
        private final Map<String, Object> puts = new HashMap<>();
        private final Set<String> removals = new HashSet<>();
        private boolean clear;

        @Override public Editor putString(String key, String value) { return put(key, value); }

        @Override public Editor putStringSet(String key, Set<String> value) {
            return put(key, value == null ? null : new HashSet<>(value));
        }

        @Override public Editor putInt(String key, int value) { return put(key, value); }

        @Override public Editor putLong(String key, long value) { return put(key, value); }

        @Override public Editor putFloat(String key, float value) { return put(key, value); }

        @Override public Editor putBoolean(String key, boolean value) { return put(key, value); }

        @Override public Editor remove(String key) {
            removals.add(key);
            puts.remove(key);
            return this;
        }

        @Override public Editor clear() {
            clear = true;
            return this;
        }

        @Override public boolean commit() {
            synchronized (MemoryPreferences.this) {
                if (clear) values.clear();
                for (String key : removals) values.remove(key);
                for (Map.Entry<String, Object> entry : puts.entrySet()) {
                    if (entry.getValue() == null) values.remove(entry.getKey());
                    else values.put(entry.getKey(), entry.getValue());
                }
            }
            return true;
        }

        @Override public void apply() { commit(); }

        private Editor put(String key, Object value) {
            removals.remove(key);
            puts.put(key, value);
            return this;
        }
    }
}
