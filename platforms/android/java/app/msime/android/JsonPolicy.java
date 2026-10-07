package app.msime.android;

/** Shared strict interpretations for values read from untyped JSON documents. */
public final class JsonPolicy {
    private JsonPolicy() {}

    /** Accept only the JSON boolean {@code true}; strings and numbers are not truthy. */
    public static boolean strictTrue(Object value) {
        return Boolean.TRUE.equals(value);
    }
}
