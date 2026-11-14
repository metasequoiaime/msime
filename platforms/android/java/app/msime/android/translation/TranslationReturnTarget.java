package app.msime.android;

import java.util.Objects;

/** 不生成包含编辑器文字的 toString；编辑器身份、选区和前后文必须同时一致。 */
public final class TranslationReturnTarget {
    private final String owner;
    private final int field;
    private final int type;
    private final int start;
    private final int end;
    private final String before;
    private final String selected;
    private final String after;
    public TranslationReturnTarget(String owner, int field, int type, int start, int end, String before, String selected, String after) {
        this.owner = owner; this.field = field; this.type = type; this.start = start; this.end = end;
        this.before = before; this.selected = selected; this.after = after;
    }
    public boolean same(TranslationReturnTarget other) {
        return other != null && owner.equals(other.owner) && field == other.field && type == other.type
            && start == other.start && end == other.end && before.equals(other.before)
            && Objects.equals(selected, other.selected) && after.equals(other.after);
    }
}
