import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import java.lang.reflect.Method;

public final class CommunityCatalogSmoke {
    public static void main(String[] arguments) throws Exception {
        // The JVM smokes run against android.jar, whose org.json classes are stubs that throw, so the bound is checked on the page length rather than through parse.
        Method exceeds = CommunityCatalog.class.getDeclaredMethod("exceedsPageLimit", int.class);
        exceeds.setAccessible(true);
        check(!(boolean) exceeds.invoke(null, CommunityRequest.PAGE_SIZE), "a full page must be accepted");
        check((boolean) exceeds.invoke(null, CommunityRequest.PAGE_SIZE + 1), "a page larger than the shared limit must be rejected");
        System.out.println("Android community catalogue bounds passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
