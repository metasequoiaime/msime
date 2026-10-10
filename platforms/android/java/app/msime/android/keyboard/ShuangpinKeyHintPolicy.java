package app.msime.android;

import java.util.LinkedHashMap;
import java.util.Map;

/** Visibility and bounded decoding for Engine-owned double-pinyin key hints. */
public final class ShuangpinKeyHintPolicy {
    private static final int MAX_KEYS = 64;
    private static final int MAX_HINT_LENGTH = 128;
    private static final String OK_TRUE = "\"ok\"\\s*:\\s*true";

    /** 共享偏好里控制提示的开关，缺省为开；关掉后双拼也不画提示，其余条件与它无关。 */
    public static final String PREFERENCE_KEY = "touch_shuangpin_key_hints";

    private ShuangpinKeyHintPolicy() { }

    public static boolean visible(boolean dedicatedEnglish, int scheme, String localMode, boolean enabled) {
        return enabled && !dedicatedEnglish && scheme == 1 && "none".equals(localMode);
    }

    /** Decode the shared host envelope; malformed or failed responses intentionally yield no map. */
    public static Map<String, String> decode(String response) {
        if (response == null || response.length() > 65_536) return Map.of();
        if (!java.util.regex.Pattern.compile(OK_TRUE).matcher(response).find()) return Map.of();
        int valueKey = response.indexOf("\"value\"");
        int open = valueKey < 0 ? -1 : response.indexOf('{', valueKey);
        int close = matchingObjectEnd(response, open);
        if (open < 0 || close < 0) return Map.of();
        Map<String, String> hints = new LinkedHashMap<>(MAX_KEYS);
        int cursor = open + 1;
        while (cursor < close) {
            cursor = skipSpaceAndCommas(response, cursor, close);
            if (cursor == close) break;
            if (response.charAt(cursor) != '"') return Map.of();
            int keyEnd = response.indexOf('"', cursor + 1);
            if (keyEnd < 0 || keyEnd > close) return Map.of();
            String key = response.substring(cursor + 1, keyEnd);
            cursor = skipSpaces(response, keyEnd + 1, close);
            if (cursor >= close || response.charAt(cursor) != ':') return Map.of();
            cursor = skipSpaces(response, cursor + 1, close);
            if (cursor >= close || response.charAt(cursor) != '"') return Map.of();
            int hintEnd = response.indexOf('"', cursor + 1);
            if (hintEnd < 0 || hintEnd > close) return Map.of();
            String hint = response.substring(cursor + 1, hintEnd);
            if (!validKey(key) || hint.length() > MAX_HINT_LENGTH || hint.indexOf('\\') >= 0)
                return Map.of();
            hints.put(key, hint);
            cursor = hintEnd + 1;
            cursor = skipSpaces(response, cursor, close);
            if (cursor < close && response.charAt(cursor) != ',') return Map.of();
        }
        return hints.size() > MAX_KEYS ? Map.of() : Map.copyOf(hints);
    }

    private static int skipSpaces(String value, int cursor, int limit) {
        while (cursor < limit && Character.isWhitespace(value.charAt(cursor))) cursor++;
        return cursor;
    }

    private static int skipSpaceAndCommas(String value, int cursor, int limit) {
        while (cursor < limit && (Character.isWhitespace(value.charAt(cursor))
                || value.charAt(cursor) == ',')) cursor++;
        return cursor;
    }

    private static int matchingObjectEnd(String value, int open) {
        if (open < 0) return -1;
        boolean quoted = false;
        for (int index = open; index < value.length(); index++) {
            char current = value.charAt(index);
            if (current == '"' && (index == 0 || value.charAt(index - 1) != '\\')) quoted = !quoted;
            if (!quoted && current == '}') return index;
        }
        return -1;
    }

    public static String hint(Map<String, String> hints, String key,
            boolean dedicatedEnglish, int scheme, String localMode, boolean enabled) {
        if (!visible(dedicatedEnglish, scheme, localMode, enabled) || key == null || hints == null)
            return "";
        return hints.getOrDefault(TextPolicy.uppercase(key), "");
    }

    private static boolean validKey(String key) {
        if (key == null || key.length() != 1) return false;
        char value = key.charAt(0);
        return value >= 'A' && value <= 'Z' || value == ';';
    }
}
