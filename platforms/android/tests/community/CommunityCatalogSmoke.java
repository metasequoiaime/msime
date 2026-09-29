import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import java.lang.reflect.Method;
import org.json.JSONArray;
import org.json.JSONObject;

public final class CommunityCatalogSmoke {
    public static void main(String[] arguments) throws Exception {
        JSONArray values = new JSONArray();
        for (int index = 0; index < CommunityRequest.PAGE_SIZE + 1; index++) {
            values.put(new JSONObject().put("id", "fixture-" + index).put("name", "Fixture"));
        }
        JSONObject root = new JSONObject().put("skins", values).put("has_more", true);
        Method parse = CommunityCatalog.class.getDeclaredMethod(
            "parse", CommunityRequest.Kind.class, JSONObject.class);
        parse.setAccessible(true);
        CommunityCatalog.Page page = (CommunityCatalog.Page) parse.invoke(
            null, CommunityRequest.Kind.SKIN, root);
        check(page.failed(), "a page larger than the shared limit must be rejected");
        System.out.println("Android community catalogue bounds passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
