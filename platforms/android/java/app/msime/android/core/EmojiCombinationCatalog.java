package app.msime.android;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;

/** 多字形组合表情的随包文本目录，和普通 Emoji 使用相同的有界分页。来源与许可见 shared/emoji-combinations。 */
public final class EmojiCombinationCatalog {
    public static final String GROUP = "Symbol emoticons";
    public static final String ASSET = "emoji-combinations.txt";
    public static final int MAX_BYTES = 32_768;
    private EmojiCombinationCatalog() {}

    public static List<EmojiCatalogModel.Item> decode(String document) {
        List<EmojiCatalogModel.Item> items = new ArrayList<>();
        HashSet<String> seen = new HashSet<>();
        String[] lines = document.split("\\n", -1);
        int count = lines.length;
        if (count > 0 && lines[count - 1].isEmpty()) count--;
        for (int index = 0; index < count; index++) {
            String text = lines[index];
            if (text.trim().isEmpty() || text.indexOf('\0') >= 0 || !seen.add(text) || items.size() >= 512)
                throw new IllegalArgumentException("Invalid combination catalog");
            items.add(new EmojiCatalogModel.Item(text, "组合表情", GROUP));
        }
        if (items.isEmpty()) throw new IllegalArgumentException("Empty combination catalog");
        return List.copyOf(items);
    }

    public static EmojiCatalogModel.Page page(List<EmojiCatalogModel.Item> items, int offset, int limit) {
        if (offset < 0 || offset > items.size() || limit < 1 || limit > EmojiCatalogModel.PAGE_SIZE)
            throw new IllegalArgumentException("Invalid combination catalog cursor");
        int end = Math.min(items.size(), offset + limit);
        return EmojiCatalogModel.validatePage(items.subList(offset, end), offset, limit, end, end == items.size());
    }
}
