import app.msime.android.CloudClipboardPanelPolicy;
import app.msime.android.CloudClipboardPanelPolicy.Status;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.clipboard.CloudClipboardRelayPolicy.Route;

/**
 * 云剪贴板改走自建中继时 Android 这一侧的纯逻辑：地址和令牌规则与 Rust、共享设置页一致，共享层答的路线只认两个值，中继拒绝令牌不会被当成水杉账号没登录，地址或令牌换了绑定就变。
 */
public final class CloudClipboardRelayPolicySmoke {
    private static final String TOKEN = "synthetic-relay-token-0001";

    public static void main(String[] args) {
        // 与 crates/client-services/src/clipboard_relay_tests.rs 和 apps/desktop/tests/settings/cloud-clipboard-relay.test.tsx 的用例一一对应。
        Object[][] urls = {
            {"https://relay.example.test", true},
            {"https://relay.example.test:8443/msime/", true},
            {"http://127.0.0.1:8787", true},
            {"http://localhost/msime", true},
            {"http://[::1]:8787", true},
            {"HTTPS://relay.example.test", true},
            {"http://LOCALHOST:8787", true},
            {"http://relay.example.test", false},
            {"http://192.168.1.2:8787", false},
            {"https://relay.example.test/?token=x", false},
            {"https://relay.example.test?x", false},
            {"https://relay.example.test/?", false},
            {"https://relay.example.test/#x", false},
            {"https://relay.example.test/#", false},
            {"https://user@relay.example.test", false},
            {"https://user:secret@relay.example.test", false},
            {"https:///msime", false},
            {"wss://relay.example.test", false},
            {"relay.example.test", false},
            {"", false},
            {"https://relay.example.test/\u0007", false},
            {"https://relay.example.test:99999", false},
            {"https://relay.example.test:80a", false},
            {"https://[::1", false},
        };
        for (Object[] row : urls) {
            String url = (String) row[0];
            boolean valid = (Boolean) row[1];
            check((CloudClipboardRelayPolicy.urlProblem(url) == null) == valid, "relay address rule: " + url);
        }
        check("中继地址过长。".equals(CloudClipboardRelayPolicy.urlProblem("https://relay.example.test/" + "a".repeat(2048))),
            "an address over 2048 bytes says it is too long");
        check(CloudClipboardRelayPolicy.urlProblem("http://relay.example.test").contains("HTTPS"),
            "plain http to another host asks for HTTPS");
        check(CloudClipboardRelayPolicy.urlProblem("https://user@relay.example.test").contains("令牌"),
            "userinfo in the address points the user at the token field");

        check(CloudClipboardRelayPolicy.tokenProblem(TOKEN) == null, "a synthetic token is accepted");
        check(CloudClipboardRelayPolicy.tokenProblem("a".repeat(16)) == null, "16 characters is the minimum");
        check(CloudClipboardRelayPolicy.tokenProblem("a".repeat(256)) == null, "256 characters is the maximum");
        for (String token : new String[] {"", "a".repeat(15), "a".repeat(257), "synthetic relay token",
                "synthetic-relay-令牌-0001", "synthetic-relay-token\n"}) {
            check(CloudClipboardRelayPolicy.tokenProblem(token) != null, "relay token rejected: " + token);
        }

        // 与 Rust 的 is_configured 相同：只看地址。
        check(!CloudClipboardRelayPolicy.configured(""), "an empty address means no relay");
        check(!CloudClipboardRelayPolicy.configured(null), "a missing address means no relay");
        check(CloudClipboardRelayPolicy.configured("https://relay.example.test"), "an address means a relay");

        check(CloudClipboardRelayPolicy.route("relay") == Route.RELAY, "the shared layer's relay answer selects the relay");
        check(CloudClipboardRelayPolicy.route("account") == Route.ACCOUNT, "the shared layer's account answer selects the account");
        check(CloudClipboardRelayPolicy.route("Relay") == null && CloudClipboardRelayPolicy.route("") == null
            && CloudClipboardRelayPolicy.route(null) == null, "an unknown answer is a failure, never the account");

        String binding = CloudClipboardRelayPolicy.binding("https://relay.example.test/msime", TOKEN);
        check(binding.startsWith("relay:") && binding.length() == "relay:".length() + 32,
            "a relay binding is relay: and 16 bytes of hex");
        check(!binding.contains(TOKEN), "the token itself never appears in the binding");
        check(binding.equals(CloudClipboardRelayPolicy.binding("https://relay.example.test/msime", TOKEN)),
            "the same relay and token give the same binding");
        check(!binding.equals(CloudClipboardRelayPolicy.binding("https://relay.example.test/other", TOKEN)),
            "another address gives another binding");
        check(!binding.equals(CloudClipboardRelayPolicy.binding("https://relay.example.test/msime",
            "synthetic-relay-token-0002")), "another token gives another binding");
        check(CloudClipboardRelayPolicy.binding("", TOKEN).isEmpty(), "no address means no binding");
        // 账号编号是 UUID，中继的绑定以 relay: 开头，两者不会撞上。
        check(!CloudClipboardPanelPolicy.acceptsBinding(binding, 0, "account-a", 0),
            "a relay binding never accepts an account's rows");
        check(CloudClipboardPanelPolicy.acceptsBinding(binding, 0, binding, 0),
            "rows read from the relay stay usable while the relay is unchanged");
        check(!CloudClipboardPanelPolicy.acceptsBinding(binding, 0,
                CloudClipboardRelayPolicy.binding("https://relay.example.test/msime", "synthetic-relay-token-0002"), 0),
            "rows read from the relay are dropped once the token changes");

        // 中继拒绝令牌不是水杉账号没登录：不提示用户去登录。
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_UNAUTHORIZED) == Status.RELAY_REJECTED,
            "a rejected relay token is not a sign-out");
        check(CloudClipboardPanelPolicy.failed(401, CloudClipboardRelayPolicy.CODE_UNAUTHORIZED) == Status.RELAY_REJECTED,
            "a rejected relay token is not a sign-out whatever the status");
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED) == Status.RELAY_REJECTED,
            "an incomplete relay setting is reported as such");
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_UNAVAILABLE) == Status.FAILED,
            "an unreachable relay is retryable");
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_STORAGE) == Status.FAILED,
            "a relay keystore failure is retryable");
        check(CloudClipboardPanelPolicy.explains(Status.RELAY_REJECTED)
            && !CloudClipboardPanelPolicy.message(Status.RELAY_REJECTED, 0).contains("登录")
            && !CloudClipboardPanelPolicy.message(Status.RELAY_REJECTED, 0).contains("\n"),
            "the keyboard's relay hint is one line and never asks to sign in");
        check(!CloudClipboardPanelPolicy.showsItems(Status.RELAY_REJECTED)
            && !CloudClipboardPanelPolicy.canUpload(true, Status.RELAY_REJECTED, "synthetic note")
            && CloudClipboardPanelPolicy.cloudCount(Status.RELAY_REJECTED, 0) == null,
            "a rejected relay lists nothing, uploads nothing and shows no count");

        for (String code : new String[] {CloudClipboardRelayPolicy.CODE_UNAUTHORIZED, CloudClipboardRelayPolicy.CODE_UNAVAILABLE,
                CloudClipboardRelayPolicy.CODE_STORAGE, CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED,
                CloudClipboardRelayPolicy.CODE_CHANGED, CloudClipboardRelayPolicy.CODE_FULL}) {
            String message = CloudClipboardRelayPolicy.failureMessage(code);
            check(message != null && !message.isEmpty() && !message.contains("登录"), "relay failure has its own message: " + code);
        }
        check(CloudClipboardRelayPolicy.failureMessage("clipboard_device_pending") == null
            && CloudClipboardRelayPolicy.failureMessage(null) == null, "other codes keep the page's own wording");
        // Android 自己跑中继的驱动：驱动报的错与 Rust 的 `RelayError::code` 同样归类。
        check("clipboard_device_pending".equals(CloudClipboardRelayPolicy.driverFailureCode(0, "clipboard_device_pending")),
            "a known driver code is kept");
        check(CloudClipboardRelayPolicy.CODE_FULL.equals(CloudClipboardRelayPolicy.driverFailureCode(409, "clipboard_full")),
            "the relay's own known code is kept");
        check("invalid_cloud_clipboard".equals(CloudClipboardRelayPolicy.driverFailureCode(400, "bad_request")),
            "an unknown 400 is an invalid request");
        check("not_found".equals(CloudClipboardRelayPolicy.driverFailureCode(404, "http_404")), "an unknown 404 is not_found");
        check(CloudClipboardRelayPolicy.CODE_UNAVAILABLE.equals(CloudClipboardRelayPolicy.driverFailureCode(500, "http_500"))
            && CloudClipboardRelayPolicy.CODE_UNAVAILABLE.equals(CloudClipboardRelayPolicy.driverFailureCode(0, null)),
            "anything else is an unavailable relay");
        System.out.println("CloudClipboardRelayPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
