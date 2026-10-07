package app.msime.android;

import java.util.List;

/** Shared immutable list normalization for host models and views. */
public final class ListPolicy {
    private ListPolicy() {}

    /** Return an immutable copy, treating a missing list as the empty list. */
    public static <T> List<T> copyOrEmpty(List<? extends T> values) {
        return values == null ? List.of() : List.copyOf(values);
    }
}
