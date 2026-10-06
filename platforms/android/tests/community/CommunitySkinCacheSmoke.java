import app.msime.android.CommunitySkinCache;
import java.lang.reflect.Method;

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
        System.out.println("CommunitySkinCache smoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
