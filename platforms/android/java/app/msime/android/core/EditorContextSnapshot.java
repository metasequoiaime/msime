package app.msime.android;

import java.util.Objects;

/** In-memory insertion guard. Context text is never serialized or exposed by this object. */
public final class EditorContextSnapshot {
    private final Object target;
    private final long revision;
    private final String before;
    private final String selected;
    private final String after;

    public EditorContextSnapshot(Object target, long revision, String before,
                                 String selected, String after) {
        this.target = Objects.requireNonNull(target);
        this.revision = revision;
        this.before = before;
        this.selected = selected;
        this.after = after;
    }

    public boolean matches(Object currentTarget, long currentRevision, String currentBefore,
                           String currentSelected, String currentAfter) {
        return target == currentTarget && revision == currentRevision
            && Objects.equals(before, currentBefore)
            && Objects.equals(selected, currentSelected) && Objects.equals(after, currentAfter);
    }
}
