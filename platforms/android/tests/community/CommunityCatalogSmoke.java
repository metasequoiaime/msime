import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import java.lang.reflect.Method;

public final class CommunityCatalogSmoke {
    public static void main(String[] arguments) throws Exception {
        // The JVM smokes run against android.jar, whose org.json classes are stubs that throw, so the policy is checked separately from parse.
        Method invalid = CommunityCatalog.class.getDeclaredMethod("invalidPage", int.class, boolean.class);
        invalid.setAccessible(true);
        check(!(boolean) invalid.invoke(null, CommunityRequest.PAGE_SIZE, true), "a full page may have more results");
        check((boolean) invalid.invoke(null, CommunityRequest.PAGE_SIZE + 1, false), "a page larger than the shared limit must be rejected");
        check((boolean) invalid.invoke(null, 0, true), "an empty page with more results must be rejected");
        check(!(boolean) invalid.invoke(null, 0, false), "an empty final page must be accepted");
        System.out.println("Android community catalogue bounds passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
