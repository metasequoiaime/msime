package app.msime.android;

/** Shared strict interpretations for values read from untyped JSON documents. */
public final class JsonPolicy {
    private JsonPolicy() {}

    /** Accept only the JSON boolean {@code true}; strings and numbers are not truthy. */
    public static boolean strictTrue(Object value) {
        return Boolean.TRUE.equals(value);
    }

    /** Accept only a JSON boolean, preserving {@code null} for every other value. */
    public static Boolean strictBoolean(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
    }

    /** Accept only a JSON boolean, using the supplied fallback for every other value. */
    public static boolean strictBoolean(Object value, boolean fallback) {
        Boolean parsed = strictBoolean(value);
        return parsed == null ? fallback : parsed;
    }

    /** Accept only a JSON string; numbers and booleans are not coerced to text. */
    public static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }

    /** 只接受 JSON 字符串，其他值返回指定的回退值。 */
    public static String strictString(Object value, String fallback) {
        String text = strictString(value);
        return text == null ? fallback : text;
    }

    /** Accept only a JSON string, using an empty string for every other value. */
    public static String strictStringOrEmpty(Object value) {
        return strictString(value, "");
    }

    /** Accept only a JSON integer that fits in a Java int. */
    public static Integer strictInteger(Object value) {
        if (value instanceof Integer integer) return integer;
        if (value instanceof Long longValue
                && longValue >= Integer.MIN_VALUE && longValue <= Integer.MAX_VALUE)
            return longValue.intValue();
        return null;
    }

    /** Accept only an Integer or Long without coercing strings or decimals. */
    public static Long strictLong(Object value) {
        if (value instanceof Integer integer) return integer.longValue();
        if (value instanceof Long longValue) return longValue;
        return null;
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
