package app.msime.android;

/** Shared strict interpretations for values read from untyped JSON documents. */
public final class JsonPolicy {
    private JsonPolicy() {}

    /** Accept only the JSON boolean {@code true}; strings and numbers are not truthy. */
    public static boolean strictTrue(Object value) {
        return Boolean.TRUE.equals(value);
    }

    /** Accept only a JSON string; numbers and booleans are not coerced to text. */
    public static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }

    /** Quote a JSON string, escaping controls and the two JSON-hostile line separators. */
    public static String quote(String value) {
        String source = TextPolicy.emptyIfNull(value);
        StringBuilder out = new StringBuilder(source.length() + 2).append('"');
        for (int index = 0; index < source.length(); index++) {
            char character = source.charAt(index);
            switch (character) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                case '\b' -> out.append("\\b");
                case '\f' -> out.append("\\f");
                default -> {
                    if (character < 0x20 || character == ' ' || character == ' ') {
                        out.append(String.format(java.util.Locale.ROOT, "\\u%04x", (int) character));
                    } else {
                        out.append(character);
                    }
                }
            }
        }
        return out.append('"').toString();
    }
}
