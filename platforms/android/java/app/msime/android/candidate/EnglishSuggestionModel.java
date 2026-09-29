package app.msime.android.candidate;

import java.util.ArrayList;
import java.util.List;

/** Bounded decoder for session-free English completion responses. */
public final class EnglishSuggestionModel {
    private static final int MAX_ITEMS = 32;
    private static final int MAX_WORD_LENGTH = 128;

    private EnglishSuggestionModel() {}

    public record Result(String prefix, List<String> items) {}

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
        ArrayList<String> items = new ArrayList<>();
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
        return new Result(prefixValue.value(), List.copyOf(items));
    }

    private record ParsedString(String value, int nextOffset) {}

    private static ParsedString parseString(String text, int quoteOffset) {
        if (quoteOffset < 0 || quoteOffset >= text.length() || text.charAt(quoteOffset) != '"') return null;
        StringBuilder value = new StringBuilder();
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
