import app.msime.android.AppleWebSignIn;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public final class AppleWebSignInSmoke {
    public static void main(String[] arguments) throws Exception {
        // RFC 7636 附录 B 的对照向量。
        check("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".equals(
            AppleWebSignIn.challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")), "S256 matches RFC 7636 appendix B");
        String verifier = AppleWebSignIn.newVerifier(new SecureRandom());
        check(verifier.length() == 43 && verifier.matches("[A-Za-z0-9_-]{43}"), "verifier is 43 base64url characters");
        check(!verifier.equals(AppleWebSignIn.newVerifier(new SecureRandom())), "verifiers are random");
        check(AppleWebSignIn.challenge(verifier).length() == 43, "challenge has no padding");

        Method fresh = AppleWebSignIn.class.getDeclaredMethod("fresh", long.class, long.class);
        fresh.setAccessible(true);
        long now = 1_700_000_000_000L;
        check((boolean) fresh.invoke(null, now - 60_000L, now), "a one-minute-old flow is fresh");
        check((boolean) fresh.invoke(null, now - 600_000L, now), "exactly ten minutes is still fresh");
        check(!(boolean) fresh.invoke(null, now - 600_001L, now), "older than ten minutes is expired");
        check(!(boolean) fresh.invoke(null, now + 1L, now), "a flow from the future is refused");
        check(!(boolean) fresh.invoke(null, 0L, now), "a missing timestamp is refused");

        Method url = AppleWebSignIn.class.getDeclaredMethod("validAuthorizationUrl", String.class);
        url.setAccessible(true);
        check((boolean) url.invoke(null, "https://appleid.apple.com/auth/authorize?client_id=x&state=y"), "Apple's page opens");
        check(!(boolean) url.invoke(null, "https://appleid.apple.com.evil.example/auth"), "a look-alike host is refused");
        check(!(boolean) url.invoke(null, "http://appleid.apple.com/auth"), "plain HTTP is refused");
        check(AppleWebSignIn.validGrant(AppleWebSignIn.newVerifier(new SecureRandom())), "a base64url grant is accepted");
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
