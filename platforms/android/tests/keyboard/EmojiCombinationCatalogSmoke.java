import app.msime.android.EmojiCatalogModel;
import app.msime.android.EmojiCombinationCatalog;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** 组合表情的原始 Unicode、全量数据与分页边界。 */
public final class EmojiCombinationCatalogSmoke {
    public static void main(String[] args) throws Exception {
        String document = Files.readString(Path.of("shared/emoji-combinations/emoji-combinations.txt"));
        String digest = java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256")
            .digest(document.getBytes(java.nio.charset.StandardCharsets.UTF_8)));
        check(digest.equals("b864b6d4ec51bc8e690638042dad8d642b8ab8ea4b7cf1424d483a4f1cc0a104"), "fixed upstream Unicode sequence and order");
        var all = EmojiCombinationCatalog.decode(document);
        check(all.size() == 165, "complete upstream list");
        check(all.get(0).text().equals("🌝🌝") && all.get(164).text().equals("🌑🌒🌓🌔🌕🌖🌗🌘🌑"),
            "source order and tail");
        for (String text : List.of("8️⃣🅱️Q了", "ᥬ🌝᭄", "🧡ྀི💛ྀི❤️ྀི", "(   ˊ ᵕ ˋ 💦)"))
            check(all.stream().anyMatch(item -> text.equals(item.text())), "Unicode sequence preserved");
        List<EmojiCatalogModel.Item> loaded = new ArrayList<>();
        int cursor = 0;
        for (int size : new int[] {64, 64, 37}) {
            var page = EmojiCombinationCatalog.page(all, cursor, 64);
            check(page.items().size() == size, "bounded pages");
            check(page.complete() == (size == 37), "completion follows final page");
            cursor = page.nextOffset();
            loaded.addAll(page.items());
        }
        check(loaded.equals(all), "pagination neither loses nor repeats items");
        check(EmojiCombinationCatalog.page(all, 165, 64).complete(), "empty tail completes");
        rejects(() -> EmojiCombinationCatalog.page(all, -1, 64));
        rejects(() -> EmojiCombinationCatalog.page(all, 166, 64));
        rejects(() -> EmojiCombinationCatalog.page(all, 0, 0));
        rejects(() -> EmojiCombinationCatalog.decode(""));
        rejects(() -> EmojiCombinationCatalog.decode("🌝🌝\n🌝🌝\n"));
        rejects(() -> EmojiCombinationCatalog.decode("🌝🌝\n\n"));
        System.out.println("Android combination emoji: complete Unicode data and bounded pagination passed");
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    private static void rejects(Runnable action) {
        try { action.run(); } catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("Malformed combination catalog accepted");
    }
}
