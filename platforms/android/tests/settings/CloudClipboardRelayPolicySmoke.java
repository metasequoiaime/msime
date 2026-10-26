import app.msime.android.CloudClipboardPanelPolicy;
import app.msime.android.CloudClipboardPanelPolicy.Status;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.clipboard.CloudClipboardRelayPolicy.Route;

/**
 * 云剪贴板改走自建中继或 WebDAV 网盘时 Android 这一侧的纯逻辑：地址、令牌、网盘账号和应用密码的规则与 Rust、共享设置页一致，共享层答的路线只认两个值，中继拒绝令牌不会被当成水杉账号没登录，地址或令牌换了绑定就变，走网盘时的说法不提中继和令牌。
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

        // WebDAV 网盘：与 crates/client-core/src/preferences/tests.rs 的 cloud_clipboard_webdav_is_all_or_nothing_and_exclusive_with_the_relay 一一对应，三项都合规才能保存。
        String user = "synthetic@example.test";
        String password = "synthetic-app-password";
        String longUser = "u".repeat(257);
        Object[][] drives = {
            {"https://dav.example.test/dav/msime", user, password, true},
            {"https://dav.example.test/dav/", user, password, true},
            {"http://127.0.0.1:8080/dav", user, password, true},
            {"http://dav.example.test/dav", user, password, false},
            {"https://user:secret@dav.example.test/dav", user, password, false},
            {"https://dav.example.test/dav?x=1", user, password, false},
            {"https://dav.example.test/dav", "", password, false},
            {"https://dav.example.test/dav", "has:colon", password, false},
            {"https://dav.example.test/dav", "bad\u0007", password, false},
            {"https://dav.example.test/dav", user, "", false},
            {"https://dav.example.test/dav", user, "bad\n", false},
            {"https://dav.example.test/dav", longUser, password, false},
            {"", user, password, false},
        };
        for (Object[] row : drives) {
            String url = (String) row[0];
            String username = (String) row[1];
            String secret = (String) row[2];
            boolean valid = (Boolean) row[3];
            boolean accepted = CloudClipboardRelayPolicy.webdavUrlProblem(url) == null
                && CloudClipboardRelayPolicy.usernameProblem(username) == null
                && CloudClipboardRelayPolicy.passwordProblem(secret) == null;
            check(accepted == valid, "webdav rule: " + url + " " + username);
        }
        // 网盘地址与中继地址是同一条规则，只是说法不同。
        for (Object[] row : urls) {
            String url = (String) row[0];
            check((CloudClipboardRelayPolicy.webdavUrlProblem(url) == null) == (Boolean) row[1],
                "webdav address follows the relay address rule: " + url);
        }
        check("请填写服务器地址。".equals(CloudClipboardRelayPolicy.webdavUrlProblem("")),
            "an empty webdav address asks for the server address");
        for (String url : new String[] {"", "http://dav.example.test/dav", "https://user@dav.example.test/dav",
                "https://dav.example.test/dav?x=1", "dav.example.test"}) {
            String problem = CloudClipboardRelayPolicy.webdavUrlProblem(url);
            check(problem != null && !problem.contains("中继") && !problem.contains("令牌"),
                "a webdav address problem never talks about a relay or a token: " + url);
        }
        check(CloudClipboardRelayPolicy.webdavUrlProblem("https://user@dav.example.test/dav").contains("应用密码"),
            "userinfo in a webdav address points the user at the account and password fields");
        // 账号和应用密码按 UTF-8 字节数算 256 的上限，控制字符按 Rust 的 char::is_control（Cc，含 U+007F 到 U+009F）。
        check(CloudClipboardRelayPolicy.usernameProblem("u".repeat(256)) == null, "a 256-byte account is accepted");
        check(CloudClipboardRelayPolicy.usernameProblem("账".repeat(85)) == null, "85 three-byte characters fit in 256 bytes");
        check(CloudClipboardRelayPolicy.usernameProblem("账".repeat(86)) != null, "86 three-byte characters are 258 bytes");
        check(CloudClipboardRelayPolicy.passwordProblem("p".repeat(256)) == null, "a 256-byte app password is accepted");
        check(CloudClipboardRelayPolicy.passwordProblem("p".repeat(257)) != null, "a 257-byte app password is rejected");
        check(CloudClipboardRelayPolicy.passwordProblem("has:colon and spaces") == null,
            "an app password may contain colons and spaces");
        for (String control : new String[] {"\u0000", "\u001f", "\u007f", "\u0085", "\u009f"}) {
            check(CloudClipboardRelayPolicy.usernameProblem("synthetic" + control) != null, "account control character rejected");
            check(CloudClipboardRelayPolicy.passwordProblem("synthetic" + control) != null, "app password control character rejected");
        }
        check(CloudClipboardRelayPolicy.usernameProblem("synthetic\u00a0user") == null
            && CloudClipboardRelayPolicy.passwordProblem("synthetic\u00a0password") == null,
            "U+00A0 is not a control character");
        check(CloudClipboardRelayPolicy.usernameProblem(null) != null && CloudClipboardRelayPolicy.passwordProblem(null) != null,
            "a missing account or app password is a problem");

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
        // WebDAV 网盘的绑定只看地址和账号，不含应用密码：换应用密码不换设备身份。
        String dav = CloudClipboardRelayPolicy.webdavBinding("https://dav.example.test/dav", "synthetic@example.test");
        check(dav.startsWith("webdav:") && dav.length() == "webdav:".length() + 32,
            "a webdav binding is webdav: and 16 bytes of hex");
        check(!dav.contains("synthetic@example.test"), "the account itself never appears in the binding");
        check(!dav.equals(CloudClipboardRelayPolicy.webdavBinding("https://dav.example.test/dav", "other@example.test")),
            "another account gives another binding");
        check(!dav.equals(CloudClipboardRelayPolicy.webdavBinding("https://dav.example.test/other", "synthetic@example.test")),
            "another folder gives another binding");
        check(!dav.equals(CloudClipboardRelayPolicy.binding("https://dav.example.test/dav", "synthetic@example.test")),
            "a drive never shares a binding with a relay at the same address");
        check(CloudClipboardRelayPolicy.webdavBinding("", "synthetic@example.test").isEmpty(), "no folder means no binding");
        for (String code : new String[] {CloudClipboardRelayPolicy.CODE_RATE_LIMITED, CloudClipboardRelayPolicy.CODE_ROLLBACK,
                CloudClipboardRelayPolicy.CODE_UNSUPPORTED}) {
            check(CloudClipboardRelayPolicy.failureMessage(code) != null, "the drive's own failures are explained: " + code);
        }
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_RATE_LIMITED) == Status.FAILED,
            "a rate-limited drive is retryable, not a sign-out");
        check(CloudClipboardPanelPolicy.failed(0, CloudClipboardRelayPolicy.CODE_ROLLBACK) == Status.RELAY_REJECTED,
            "a rolled-back drive log sends the user to the settings, retrying does not help");

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
        // 走 WebDAV 网盘（含设置页的测试连接）时同样的码说成网盘、账号和应用密码，不提中继和令牌，也不提登录。
        check("网盘拒绝了这个账号或应用密码".equals(CloudClipboardRelayPolicy.failureMessage(CloudClipboardRelayPolicy.CODE_UNAUTHORIZED, true)),
            "a drive that rejects the account says so");
        check("连不上网盘，请检查地址和网络；证书须由公网信任的机构签发".equals(
            CloudClipboardRelayPolicy.failureMessage(CloudClipboardRelayPolicy.CODE_UNAVAILABLE, true)), "an unreachable drive says so");
        check("地址、账号或应用密码不完整".equals(CloudClipboardRelayPolicy.failureMessage(CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED, true)),
            "an incomplete drive setting says so");
        for (String code : new String[] {CloudClipboardRelayPolicy.CODE_UNAUTHORIZED, CloudClipboardRelayPolicy.CODE_UNAVAILABLE,
                CloudClipboardRelayPolicy.CODE_STORAGE, CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED,
                CloudClipboardRelayPolicy.CODE_CHANGED, CloudClipboardRelayPolicy.CODE_FULL,
                CloudClipboardRelayPolicy.CODE_RATE_LIMITED, CloudClipboardRelayPolicy.CODE_ROLLBACK,
                CloudClipboardRelayPolicy.CODE_UNSUPPORTED}) {
            String message = CloudClipboardRelayPolicy.failureMessage(code, true);
            check(message != null && !message.contains("中继") && !message.contains("令牌") && !message.contains("登录"),
                "drive failure never talks about a relay, a token or signing in: " + code);
        }
        check(CloudClipboardRelayPolicy.failureMessage(CloudClipboardRelayPolicy.CODE_UNAUTHORIZED, false)
                .equals(CloudClipboardRelayPolicy.failureMessage(CloudClipboardRelayPolicy.CODE_UNAUTHORIZED)),
            "the relay keeps its own wording");
        check(CloudClipboardRelayPolicy.failureMessage("clipboard_device_pending", true) == null,
            "other codes keep the page's own wording for a drive too");
        // 键盘面板按绑定认出网盘，被拒时说网盘。
        check(CloudClipboardRelayPolicy.webdav(dav) && !CloudClipboardRelayPolicy.webdav(binding)
            && !CloudClipboardRelayPolicy.webdav("") && !CloudClipboardRelayPolicy.webdav(null)
            && !CloudClipboardRelayPolicy.webdav("account-a"), "only a webdav binding is a drive");
        String driveRejected = CloudClipboardPanelPolicy.message(Status.RELAY_REJECTED, 0, true);
        check(driveRejected.equals(CloudClipboardPanelPolicy.WEBDAV_REJECTED_MESSAGE) && driveRejected.contains("网盘")
            && !driveRejected.contains("中继") && !driveRejected.contains("令牌") && !driveRejected.contains("登录")
            && !driveRejected.contains("\n"), "the keyboard's drive hint is one line about the drive");
        check(CloudClipboardPanelPolicy.message(Status.RELAY_REJECTED, 0, false).equals(CloudClipboardPanelPolicy.RELAY_REJECTED_MESSAGE)
            && CloudClipboardPanelPolicy.message(Status.FAILED, 0, true).equals(CloudClipboardPanelPolicy.message(Status.FAILED, 0)),
            "only the rejected state changes wording for a drive");
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
