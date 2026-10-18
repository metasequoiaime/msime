import app.msime.android.AppleWebSignIn;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public final class AppleWebSignInSmoke {
    /** 服务端 grant 的形状：32 字节的无填充 base64url，43 个字符。 */
    private static final String GRANT = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

    public static void main(String[] arguments) throws Exception {
        // verifier、S256 摘要和授权地址的校验都在 Rust（apple_web_start），由 client-services 的测试覆盖；这里只剩回调和等待中的流程。
        Method fresh = AppleWebSignIn.class.getDeclaredMethod("fresh", long.class, long.class);
        fresh.setAccessible(true);
        long now = 1_700_000_000_000L;
        check((boolean) fresh.invoke(null, now - 60_000L, now), "a one-minute-old flow is fresh");
        check((boolean) fresh.invoke(null, now - 600_000L, now), "exactly ten minutes is still fresh");
        check(!(boolean) fresh.invoke(null, now - 600_001L, now), "older than ten minutes is expired");
        check(!(boolean) fresh.invoke(null, now + 1L, now), "a flow from the future is refused");
        check(!(boolean) fresh.invoke(null, 0L, now), "a missing timestamp is refused");

        check(AppleWebSignIn.validGrant(GRANT), "a base64url grant is accepted");
        check(!AppleWebSignIn.validGrant("short"), "a short grant is refused");
        check(!AppleWebSignIn.validGrant("a".repeat(40) + "/+="), "standard base64 characters are refused");

        // 白名单与 editions.json 的版本 id 对应：full 是不带后缀的包名。
        Path editions = locate("shared/contracts/editions.json");
        if (editions != null) {
            List<String> ids = new ArrayList<>();
            Matcher matcher = Pattern.compile("\"id\"\\s*:\\s*\"([a-z]+)\"").matcher(Files.readString(editions));
            while (matcher.find()) ids.add(matcher.group(1));
            List<String> apps = new ArrayList<>();
            for (String id : ids) apps.add("full".equals(id) ? "app.msime.android" : "app.msime.android." + id);
            check(apps.size() == AppleWebSignIn.APPS.size() && AppleWebSignIn.APPS.containsAll(apps),
                "the app allowlist must name every edition in editions.json");
        }
        System.out.println("Android Apple web sign-in passed");
    }

    private static Path locate(String relative) {
        for (Path directory = Path.of("").toAbsolutePath(); directory != null; directory = directory.getParent()) {
            Path candidate = directory.resolve(relative);
            if (Files.isRegularFile(candidate)) return candidate;
        }
        return null;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
