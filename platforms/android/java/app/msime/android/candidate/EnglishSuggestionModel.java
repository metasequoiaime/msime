package app.msime.android.candidate;

import java.util.ArrayList;
import java.util.List;

/** 不创建输入会话的英文候选响应解码；展示列表的合并由共享引擎决定。 */
public final class EnglishSuggestionModel {
    private static final int MAX_ITEMS = 32;
    private static final int MAX_WORD_LENGTH = 128;

    private EnglishSuggestionModel() {}

    /** corrected 只来自合并展示查询：为真时第二行是纠错，否则其后都是前缀补全。 */
    public record Result(String prefix, List<String> items, boolean corrected) {}

    public static Result decode(String response) {
        if (response == null || response.length() > 262_144)
            throw new IllegalArgumentException("English completion response is too large");
        if (!response.contains("\"ok\":true"))
            throw new IllegalArgumentException("English completion failed");
        int prefixStart = response.indexOf("\"prefix\":\"");
        int itemsStart = response.indexOf("\"items\":[");
        if (prefixStart < 0 || itemsStart < 0)
            throw new IllegalArgumentException("Invalid English completion response");
        ParsedString prefixValue = parseString(response, prefixStart + 9);
        if (prefixValue == null || prefixValue.value().isEmpty()
                || prefixValue.value().length() > MAX_WORD_LENGTH)
            throw new IllegalArgumentException("Invalid English completion response");
        ArrayList<String> items = new ArrayList<>(MAX_ITEMS);
        int offset = itemsStart + 9;
        while (offset < response.length() && response.charAt(offset) != ']') {
            if (response.charAt(offset) != '"' || items.size() >= MAX_ITEMS)
                throw new IllegalArgumentException("Invalid English completion items");
            ParsedString itemValue = parseString(response, offset);
            if (itemValue == null) throw new IllegalArgumentException("Invalid English completion item");
            String item = itemValue.value();
            if (item.isEmpty() || item.length() > MAX_WORD_LENGTH)
                throw new IllegalArgumentException("Invalid English completion item");
            items.add(item);
            offset = itemValue.nextOffset();
            while (offset < response.length() && response.charAt(offset) == ',') offset++;
        }
        if (offset >= response.length() || response.charAt(offset) != ']')
            throw new IllegalArgumentException("Invalid English completion items");
        int correctedStart = response.indexOf("\"corrected\":");
        boolean corrected = correctedStart >= 0 && response.startsWith("true", correctedStart + 12);
        if (corrected && items.size() != 2)
            throw new IllegalArgumentException("Invalid English correction display");
        return new Result(prefixValue.value(), List.copyOf(items), corrected);
    }

    private record ParsedString(String value, int nextOffset) {}

    private static ParsedString parseString(String text, int quoteOffset) {
        if (quoteOffset < 0 || quoteOffset >= text.length() || text.charAt(quoteOffset) != '"') return null;
        StringBuilder value = new StringBuilder(text.length() - quoteOffset - 1);
        for (int offset = quoteOffset + 1; offset < text.length(); offset++) {
            char current = text.charAt(offset);
            if (current == '"') return new ParsedString(value.toString(), offset + 1);
            if (current != '\\') {
                value.append(current);
                continue;
            }
            if (++offset >= text.length()) return null;
            char escaped = text.charAt(offset);
            switch (escaped) {
                case '"', '\\', '/' -> value.append(escaped);
                case 'b' -> value.append('\b');
                case 'f' -> value.append('\f');
                case 'n' -> value.append('\n');
                case 'r' -> value.append('\r');
                case 't' -> value.append('\t');
                case 'u' -> {
                    if (offset + 4 >= text.length()) return null;
                    try { value.append((char) Integer.parseInt(text.substring(offset + 1, offset + 5), 16)); }
                    catch (NumberFormatException error) { return null; }
                    offset += 4;
                }
                default -> { return null; }
            }
        }
        return null;
    }
}
