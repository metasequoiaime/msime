package app.msime.android;

import java.util.LinkedHashMap;
import java.util.Map;

/** Visibility and bounded decoding for Engine-owned double-pinyin key hints. */
public final class ShuangpinKeyHintPolicy {
    private static final int MAX_KEYS = 64;
    private static final int MAX_HINT_LENGTH = 128;
    private static final String OK_TRUE = "\"ok\"\\s*:\\s*true";

    /** 共享偏好里控制提示的开关，缺省为关；打开后双拼和粤语双拼才画提示，其余条件与它无关。 */
    public static final String PREFERENCE_KEY = "touch_shuangpin_key_hints";

    /** 粤语双拼键面在引擎里的保留名（`host::CANTONESE_SHUANGPIN_HINTS`），经同一个 `NativeClient.shuangpinKeyHints` 取。它不是普通话双拼方案名，会话的 `shuangpin_profile` 里不会出现。 */
    public static final String CANTONESE_PROFILE = "cantonese";

    /** 粤语双拼的触屏方案 id（`touch_keyboard_schemes.selected`）。普通粤拼与粤语双拼是同一个引擎方案（scheme 5），只有键盘知道选的是哪一个。 */
    public static final String CANTONESE_SHUANGPIN_SCHEME = "cantonese_shuangpin";

    private ShuangpinKeyHintPolicy() { }

    /** 有 view 时按哪个键面画提示，同时是要记下的那一个：双拼会话见 {@link #rememberedProfile(int, String)}；粤拼会话、且选中的触屏方案是粤语双拼时是 {@link #CANTONESE_PROFILE}；普通粤拼完整拼写，不画。会话跑的不是粤拼时（比如缺了 `msime-cantonese.db`，运行时留在上一个中文方案）不给粤语双拼键面，免得把键标成没在跑的方案，和 iOS 的 `cantoneseShuangpinKeyHints` 一样。 */
    public static String rememberedProfile(int scheme, String profile, String touchScheme) {
        if (scheme == InputSchemeTraits.CANTONESE)
            return CANTONESE_SHUANGPIN_SCHEME.equals(touchScheme) ? CANTONESE_PROFILE : "";
        return rememberedProfile(scheme, profile);
    }

    /** 没有 view 时按 {@link #profileWithoutView(boolean, String)} 取记下的键面，但粤语双拼的键面只在此刻选中的仍是粤语双拼时用：view 到达前刚换到别的触屏方案时不沿用它。 */
    public static String profileWithoutView(boolean sessionExpected, String remembered, String touchScheme) {
        String profile = profileWithoutView(sessionExpected, remembered);
        return CANTONESE_PROFILE.equals(profile) && !CANTONESE_SHUANGPIN_SCHEME.equals(touchScheme) ? "" : profile;
    }

    /** 传给 {@link #hint} 的方案号：粤语双拼的键面和双拼一样画，其余照会话的方案。 */
    public static int hintScheme(int scheme, String profile) {
        return CANTONESE_PROFILE.equals(profile) ? InputSchemeTraits.SHUANGPIN : scheme;
    }

    /** 有 view 时记下的双拼方案：方案是双拼（scheme 1）时是它的 `shuangpin_profile`，其他方案为空。本地模式和英文只决定此刻画不画，不改记下的方案，免得临时进一次本地模式就把它冲掉。 */
    public static String rememberedProfile(int scheme, String profile) {
        return scheme == 1 && profile != null ? profile : "";
    }

    /**
     * 没有 view 时按哪个双拼方案画提示。换应用、换页面时 onStartInput 先停掉会话、清空 view，会话要在后台建一两秒；光标移动取消组字后 view 也会暂时清空。会话正在建或仍然在（{@code sessionExpected}）时用上次记下的方案，第一帧就带着提示和下边距，会话到达时不再整排跳一下。不打算建会话的输入框（密码、数字）和建会话失败后不画。
     */
    public static String profileWithoutView(boolean sessionExpected, String remembered) {
        return sessionExpected && remembered != null ? remembered : "";
    }

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
        while (cursor < limit && TextPolicy.isWhitespace(value.charAt(cursor))) cursor++;
        return cursor;
    }

    private static int skipSpaceAndCommas(String value, int cursor, int limit) {
            while (cursor < limit && (TextPolicy.isWhitespace(value.charAt(cursor))
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
