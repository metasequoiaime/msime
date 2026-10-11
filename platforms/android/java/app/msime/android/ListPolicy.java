package app.msime.android;

import java.util.List;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.function.Predicate;

/** Shared immutable list normalization for host models and views. */
public final class ListPolicy {
    private ListPolicy() {}

    /** Return an immutable copy, treating a missing list as the empty list. */
    public static <T> List<T> copyOrEmpty(List<? extends T> values) {
        return values == null ? List.of() : List.copyOf(values);
    }

    /** Preserve recent-first order while discarding invalid and duplicate stored entries. */
    public static List<String> normalizeRecents(List<String> stored, int limit,
            Predicate<String> valid) {
        LinkedHashSet<String> unique = new LinkedHashSet<>(limit);
        if (stored != null) {
            for (String value : stored) {
                if (!valid.test(value)) continue;
                unique.add(value);
                if (unique.size() == limit) break;
            }
        }
        return List.copyOf(unique);
    }

    /** Put a validated selection first and retain the newest distinct stored entries. */
    public static List<String> recordRecent(List<String> stored, String selected, int limit,
            Predicate<String> valid) {
        ArrayList<String> reordered = new ArrayList<>(limit);
        reordered.add(selected);
        for (String value : normalizeRecents(stored, limit, valid)) {
            if (reordered.size() == limit) break;
            if (!selected.equals(value)) reordered.add(value);
        }
        return List.copyOf(reordered);
    }
}
