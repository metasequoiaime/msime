package app.msime.client;

/** Shared character-level checks for text accepted by Android host policies. */
public final class TextPolicy {
    private TextPolicy() {}

    public static boolean hasControl(String value) {
        if (value == null) return false;
        return value.codePoints().anyMatch(Character::isISOControl);
    }
}
