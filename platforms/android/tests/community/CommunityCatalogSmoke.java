import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.JsonPolicy;
import app.msime.android.TextPolicy;
import java.lang.reflect.Method;
import java.lang.reflect.InvocationTargetException;
import java.util.List;
import java.util.UUID;

public final class CommunityCatalogSmoke {
    public static void main(String[] arguments) throws Exception {
        // The JVM smokes run against android.jar, whose org.json classes are stubs that throw, so the policy is checked separately from parse.
        Method invalid = CommunityCatalog.class.getDeclaredMethod("invalidPage", int.class, int.class, boolean.class);
        invalid.setAccessible(true);
        check(!(boolean) invalid.invoke(null, CommunityRequest.PAGE_SIZE, CommunityRequest.PAGE_SIZE, true), "a full page may have more results");
        check((boolean) invalid.invoke(null, CommunityRequest.PAGE_SIZE + 1, CommunityRequest.PAGE_SIZE + 1, false), "a page larger than the shared limit must be rejected");
        check((boolean) invalid.invoke(null, 0, 0, true), "an empty page with more results must be rejected");
        check((boolean) invalid.invoke(null, 1, 0, true), "a page with only malformed rows must not retry the same offset");
        check((boolean) invalid.invoke(null, 2, 1, true), "dropping any row must not shift the next offset");
        check(!(boolean) invalid.invoke(null, 0, 0, false), "an empty final page must be accepted");
        check(TextPolicy.lowercase("a1234567-1234-1234-1234-123456789abc").equals(
            TextPolicy.lowercase("A1234567-1234-1234-1234-123456789ABC")),
            "UUID duplicate detection must ignore hexadecimal case");
        check(!TextPolicy.lowercase("a1234567-1234-1234-1234-123456789abc").equals(
            TextPolicy.lowercase("b1234567-1234-1234-1234-123456789abc")),
            "different UUIDs must remain distinct");
        Method responseLimit = CommunityCatalog.class.getDeclaredMethod(
            "maximumResponseBytes", CommunityRequest.Kind.class);
        responseLimit.setAccessible(true);
        check((int) responseLimit.invoke(null, CommunityRequest.Kind.SKIN) == 4 * 1024 * 1024,
            "skin pages keep the ordinary response bound");
        check((int) responseLimit.invoke(null, CommunityRequest.Kind.DICTIONARY) == 48 * 1024 * 1024,
            "dictionary pages allow the shared resource response bound");
        check((int) responseLimit.invoke(null, CommunityRequest.Kind.REPLY) == 48 * 1024 * 1024,
            "reply pages allow the shared resource response bound");
        check((int) responseLimit.invoke(null, CommunityRequest.Kind.PHRASE) == 48 * 1024 * 1024,
            "phrase pages allow the shared resource response bound");
        String token = "e".repeat(64);
        Method retryListing = CommunityCatalog.class.getDeclaredMethod(
            "shouldRetryListing", int.class, String.class, int.class);
        retryListing.setAccessible(true);
        check((boolean) retryListing.invoke(null, 401, token, 0),
            "an account 401 retries the listing once");
        check(!(boolean) retryListing.invoke(null, 401, token, 1),
            "a listing cannot retry an account 401 twice");
        check(!(boolean) retryListing.invoke(null, 500, token, 0),
            "a server failure is not an account refresh signal");
        check(!(boolean) retryListing.invoke(null, 401, "", 0),
            "an anonymous or missing token does not trigger account refresh");
        Method validItem = CommunityCatalog.class.getDeclaredMethod(
            "validItem", CommunityCatalog.Item.class, CommunityRequest.Kind.class);
        validItem.setAccessible(true);
        CommunityCatalog.Item malformed = new CommunityCatalog.Item(
            "not-a-uuid", CommunityRequest.Kind.SKIN, "名称", "说明", "作者", 0, 0, 0, null,
            CommunityRequest.Category.OTHER, false, 0, null);
        check(!(boolean) validItem.invoke(null, malformed, CommunityRequest.Kind.SKIN),
            "malformed community items must be rejected");
        CommunityCatalog.Discovery unavailable = CommunityCatalog.discovery(null, 8);
        check(unavailable.failed() && unavailable.items() == null && !unavailable.failure().isEmpty(),
            "a missing discovery page becomes a displayable failure");
        CommunityCatalog.Discovery failed = CommunityCatalog.discovery(
            new CommunityCatalog.Page(List.of(), false, "合成失败"), 8);
        check(failed.failed() && failed.items() == null && "合成失败".equals(failed.failure()),
            "a discovery page preserves the catalogue failure");
        CommunityCatalog.Page source = new CommunityCatalog.Page(List.of(malformed, malformed), true, "");
        CommunityCatalog.Discovery bounded = CommunityCatalog.discovery(source, 1);
        check(!bounded.failed() && bounded.failure() == null && bounded.items().size() == 1
                && bounded.items().get(0) == malformed && source.items().size() == 2,
            "a successful discovery page is copied and bounded without changing its source");
        CommunityCatalog.Discovery empty = CommunityCatalog.discovery(source, 0);
        check(!empty.failed() && empty.items().isEmpty(),
            "a non-positive discovery limit returns a successful empty list");
        CommunityCatalog.Item nilId = new CommunityCatalog.Item(
            "00000000-0000-0000-0000-000000000000", CommunityRequest.Kind.SKIN, "名称", "说明", "作者",
            0, 0, 0, null, CommunityRequest.Category.OTHER, false, 0, null);
        check(!(boolean) validItem.invoke(null, nilId, CommunityRequest.Kind.SKIN),
            "nil community item IDs must be rejected");
        Method validUuid = CommunityCatalog.class.getDeclaredMethod("validUuid", String.class);
        validUuid.setAccessible(true);
        check(!(boolean) validUuid.invoke(null, "00000000-0000-0000-0000-000000000000"),
            "nil UUIDs must not be accepted as community item IDs");
        Method validReportItem = CommunityCatalog.class.getDeclaredMethod(
            "validReportItem", CommunityCatalog.Item.class);
        validReportItem.setAccessible(true);
        check(!(boolean) validReportItem.invoke(null, nilId),
            "reporting must reject an item with a nil ID even when bypassing catalogue parsing");
        Method validDownloadItem = CommunityCatalog.class.getDeclaredMethod(
            "validDownloadItem", CommunityCatalog.Item.class);
        validDownloadItem.setAccessible(true);
        check(!(boolean) validDownloadItem.invoke(null, malformed),
            "download counting must reject an item with an unsafe ID");
        check(!(boolean) validDownloadItem.invoke(null, nilId),
            "download counting must reject an item with a nil ID");
        CommunityCatalog.Item invalidRating = new CommunityCatalog.Item(
            UUID.randomUUID().toString(), CommunityRequest.Kind.SKIN, "名称", "说明", "作者",
            0, 0, 1, null, CommunityRequest.Category.OTHER, false, 0, null);
        check(!(boolean) validItem.invoke(null, invalidRating, CommunityRequest.Kind.SKIN),
            "a rating average without ratings must be rejected");
        Method validName = CommunityCatalog.class.getDeclaredMethod("validName", String.class, int.class);
        validName.setAccessible(true);
        check(!(boolean) validName.invoke(null, "坏\uD800名", 128),
            "community names reject malformed Unicode");
        Method validDescription = CommunityCatalog.class.getDeclaredMethod("validDescription", String.class);
        validDescription.setAccessible(true);
        check(!(boolean) validDescription.invoke(null, "说明\uD800"),
            "community descriptions reject malformed Unicode");
        // 分类只属于皮肤：皮肤条目必须有分类（缺失时已解析成 other），词库和回复条目不能有。
        Method validCategory = CommunityCatalog.class.getDeclaredMethod(
            "validCategory", CommunityRequest.Kind.class, CommunityRequest.Category.class);
        validCategory.setAccessible(true);
        check((boolean) validCategory.invoke(null, CommunityRequest.Kind.SKIN,
            CommunityRequest.Category.GUOFENG), "a skin with a category is valid");
        check(!(boolean) validCategory.invoke(null, CommunityRequest.Kind.SKIN, null),
            "a skin whose category did not parse must be rejected");
        check(!(boolean) validCategory.invoke(null, CommunityRequest.Kind.DICTIONARY,
            CommunityRequest.Category.OTHER), "a dictionary carries no category");
        check((boolean) validCategory.invoke(null, CommunityRequest.Kind.REPLY, null),
            "a reply set without a category is valid");
        check(!(boolean) validCategory.invoke(null, CommunityRequest.Kind.PHRASE,
            CommunityRequest.Category.OTHER), "a phrase pack carries no category");
        check("synthetic name".equals(JsonPolicy.strictString("synthetic name")),
            "community string fields accept strings");
        check(JsonPolicy.strictString(42) == null,
            "community string fields reject numbers instead of coercing them");
        check(JsonPolicy.strictString(Boolean.TRUE) == null,
            "community string fields reject booleans instead of coercing them");
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)),
            "community boolean fields accept booleans");
        check(JsonPolicy.strictBoolean("true") == null,
            "community boolean fields reject strings instead of coercing them");
        check(JsonPolicy.strictTrue(Boolean.TRUE),
            "community pagination accepts JSON booleans");
        check(!JsonPolicy.strictTrue("true"),
            "community pagination rejects strings instead of coercing them");
        check(JsonPolicy.strictTrue(Boolean.TRUE),
            "a report is successful only when the backend confirms it");
        check(!JsonPolicy.strictTrue(Boolean.FALSE),
            "a backend refusal must not be reported as a successful report");
        check(!JsonPolicy.strictTrue("true"),
            "a string reported value must not be coerced into success");
        Method countNumber = CommunityCatalog.class.getDeclaredMethod("countNumber", Object.class);
        countNumber.setAccessible(true);
        check(Long.valueOf(9_007_199_254_740_991L).equals(
                countNumber.invoke(null, Long.valueOf(9_007_199_254_740_991L))),
            "community counts retain the largest JavaScript integer");
        check(Long.valueOf(42L).equals(countNumber.invoke(null, Integer.valueOf(42))),
            "community counts accept ordinary JSON integers");
        check(countNumber.invoke(null, Double.valueOf(42.0)) == null,
            "a JSON decimal is not an integer count");
        check(countNumber.invoke(null, Double.valueOf("9007199254740991.1")) == null,
            "a large fractional count cannot pass after Double rounding");
        Method setCategory = CommunityCatalog.class.getDeclaredMethod(
            "setCategory", CommunityCatalog.Item.class, CommunityRequest.Category.class);
        java.lang.reflect.Field unsafeField = Class.forName("sun.misc.Unsafe")
            .getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        Object catalog = unsafeField.get(null);
        Method allocate = catalog.getClass().getMethod("allocateInstance", Class.class);
        CommunityCatalog uninitialized = (CommunityCatalog) allocate.invoke(catalog, CommunityCatalog.class);
        try {
            CommunityCatalog.Update update = (CommunityCatalog.Update) setCategory.invoke(
                uninitialized, null, CommunityRequest.Category.OTHER);
            check(update.failed() && !update.failure().isEmpty(),
                "a missing item must return a category update failure");
        } catch (InvocationTargetException error) {
            throw new AssertionError("a missing item must not throw", error.getCause());
        }
        Method install = CommunityCatalog.class.getDeclaredMethod(
            "install", java.nio.file.Path.class, CommunityCatalog.Item.class);
        try {
            String failure = (String) install.invoke(uninitialized, null, null);
            check(failure != null && !failure.isEmpty(),
                "a missing item must return an install failure");
        } catch (InvocationTargetException error) {
            throw new AssertionError("a missing item must not throw during install", error.getCause());
        }
        System.out.println("Android community catalogue bounds passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
