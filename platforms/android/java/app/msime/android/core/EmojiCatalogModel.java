package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

/** Host-only paging and recent-selection policy for the Engine-owned emoji catalog. */
public final class EmojiCatalogModel {
    public static final int COLUMNS = 8;
    public static final int PAGE_SIZE = 64;
    public static final int RECENTS_LIMIT = 24;
    public static final int MAX_TEXT_CODE_POINTS = 32;
    public static final int MAX_ANNOTATION_CODE_POINTS = 1_024;

    public record Category(String group, String title) {}
    public record Item(String text, String annotation, String group) {
        public Item {
            if (text == null || text.isEmpty()
                    || text.codePointCount(0, text.length()) > MAX_TEXT_CODE_POINTS)
                throw new IllegalArgumentException("Invalid emoji catalog text");
            if (annotation == null || annotation.codePointCount(0, annotation.length())
                    > MAX_ANNOTATION_CODE_POINTS)
                throw new IllegalArgumentException("Invalid emoji annotation");
            if (group == null || group.isEmpty() || group.length() > 128)
                throw new IllegalArgumentException("Invalid emoji group");
        }
    }
    public record Page(List<Item> items, int nextOffset, boolean complete) {
        public Page {
            items = List.copyOf(items);
        }
    }

    // Unicode group order; database row sort order interleaves Symbols and Flags.
    private static final List<Category> CATEGORIES = List.of(
        new Category("Smileys and emotion", "笑脸"),
        new Category("People and body", "人物"),
        new Category("Animals and nature", "动物"),
        new Category("Food and drink", "食物"),
        new Category("Travel and places", "旅行"),
        new Category("Activities", "活动"),
        new Category("Objects", "物品"),
        new Category("Symbols", "符号"),
        new Category("Flags", "旗帜")
    );

    private EmojiCatalogModel() {}

    public static List<Category> categories() { return CATEGORIES; }

    public static Page validatePage(
            List<Item> items, int requestedOffset, int limit, long nextOffset, boolean complete) {
        if (requestedOffset < 0 || limit < 1 || limit > 255 || items == null
                || items.size() > limit || nextOffset < requestedOffset
                || nextOffset > (long) requestedOffset + limit
                || nextOffset > Integer.MAX_VALUE || (!complete && nextOffset == requestedOffset))
            throw new IllegalArgumentException("Invalid emoji catalog page");
        return new Page(items, (int) nextOffset, complete);
    }

    /** Most-recent first, deduplicated, and bounded without retaining invalid persisted values. */
    public static List<String> normalizeRecents(List<String> stored) {
        LinkedHashSet<String> unique = new LinkedHashSet<>();
        if (stored != null) {
            for (String text : stored) {
                if (text == null || text.isEmpty()
                        || text.codePointCount(0, text.length()) > MAX_TEXT_CODE_POINTS) continue;
                unique.add(text);
                if (unique.size() == RECENTS_LIMIT) break;
            }
        }
        return List.copyOf(unique);
    }

    public static List<String> recordRecent(List<String> stored, String selected) {
        if (selected == null || selected.isEmpty()
                || selected.codePointCount(0, selected.length()) > MAX_TEXT_CODE_POINTS)
            throw new IllegalArgumentException("Invalid recent emoji");
        ArrayList<String> reordered = new ArrayList<>();
        reordered.add(selected);
        for (String text : normalizeRecents(stored)) {
            if (!selected.equals(text)) reordered.add(text);
            if (reordered.size() == RECENTS_LIMIT) break;
        }
        return List.copyOf(reordered);
    }
}
