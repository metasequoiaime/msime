package app.msime.android;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

/**
 * 候选旁边那些按词查的显示信息（释义、读音）查过的回答：键是词，值是回答，没有回答也记（空串）。最多记 {@code capacity} 条，最久没用的先丢。
 * 无痕时（{@link #setRemembering} 传 false）不记，已记的丢掉。只在一条线程上用。
 */
final class CandidateLookupMemo<K> {
    /** 查过、没有回答。 */
    static final String NONE = "";

    private final LinkedHashMap<K, String> remembered;
    private boolean remembering = true;

    CandidateLookupMemo(int capacity) {
        remembered = new LinkedHashMap<>(16, 0.75f, true) {
            private static final long serialVersionUID = 1L;

            @Override protected boolean removeEldestEntry(Map.Entry<K, String> eldest) {
                return size() > capacity;
            }
        };
    }

    void setRemembering(boolean remembering) {
        if (!remembering) remembered.clear();
        this.remembering = remembering;
    }

    void clear() {
        remembered.clear();
    }

    /** 还没记住的键，按出现的先后、去重。 */
    List<K> unseen(Collection<K> keys) {
        LinkedHashSet<K> unseen = new LinkedHashSet<>();
        for (K key : keys) {
            if (!remembered.containsKey(key)) unseen.add(key);
        }
        return new ArrayList<>(unseen);
    }

    /** 记住的回答（可能是 {@link #NONE}）；没记住时为 null。 */
    String get(K key) {
        return remembered.get(key);
    }

    void put(K key, String value) {
        if (remembering) remembered.put(key, value == null ? NONE : value);
    }
}
