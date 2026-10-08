import app.msime.android.CommunitySkinCache;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;

public final class CommunitySkinCacheSmoke {
    public static void main(String[] arguments) throws Exception {
        Method strictString = CommunitySkinCache.class.getDeclaredMethod("strictString", Object.class);
        strictString.setAccessible(true);
        check("synthetic".equals(strictString.invoke(null, "synthetic")),
            "community cache text accepts JSON strings");
        check(strictString.invoke(null, 7) == null,
            "community cache text rejects numbers instead of coercing them");
        check(strictString.invoke(null, Boolean.TRUE) == null,
            "community cache text rejects booleans instead of coercing them");
        Path root = Files.createTempDirectory("community-skin-cache-");
        Path outside = Files.createTempFile("community-skin-cache-hardlink-", ".json");
        try {
            Files.writeString(outside,
                "[{\"id\":\"outside\",\"name\":\"outside\",\"author\":\"\",\"design\":{}}]");
            Files.createLink(root.resolve("community-skins.json"), outside);
            check(CommunitySkinCache.read(root).isEmpty(),
                "community cache rejects hard-linked files");
        } finally {
            Files.deleteIfExists(root.resolve("community-skins.json"));
            Files.deleteIfExists(root);
            Files.deleteIfExists(outside);
        }
        System.out.println("CommunitySkinCache smoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
