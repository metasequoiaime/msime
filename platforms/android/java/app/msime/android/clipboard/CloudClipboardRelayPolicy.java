package app.msime.android.clipboard;

import app.msime.android.DigestPolicy;
import app.msime.android.TextPolicy;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

/**
 * 云剪贴板改走自建中继（`docs/cloud-clipboard-relay.md`）时 Android 宿主这一侧的纯逻辑：设置页的地址和令牌规则、共享层答的路线、键盘用来认出「中继换过了」的绑定，以及给人看的错误说法。不依赖 Android 和 org.json，JVM 冒烟直接测它。
 *
 * <p>地址和令牌的规则与 `crates/client-core/src/preferences.rs` 的 `valid_relay_url`、`valid_relay_token` 以及共享设置页的 `packages/ui/src/settings/cloud-clipboard-relay.ts` 相同。保存偏好时 Rust 还会再校验一遍，不合规的值整份偏好都存不进去；这里只为了在输入框里当场说明哪里不行。
 */
public final class CloudClipboardRelayPolicy {
    /** 偏好里的键：`{url, token}`，没配置时不写进文档。 */
    public static final String PREFERENCE = "cloud_clipboard_relay";
    public static final int MAX_URL_BYTES = 2048;
    public static final int MIN_TOKEN_CHARS = 16;
    public static final int MAX_TOKEN_CHARS = 256;

    /** 中继以 401 拒绝了令牌。不是水杉账号未登录，不能提示用户去登录。 */
    public static final String CODE_UNAUTHORIZED = "relay_unauthorized";
    /** 没有发出去或没有拿到可用的响应：断网、超时、证书不受信任等。 */
    public static final String CODE_UNAVAILABLE = "relay_unavailable";
    /** 本机的中继密钥串或锁文件读写失败。 */
    public static final String CODE_STORAGE = "relay_storage";
    /** 偏好里没有中继，或者地址、令牌不合规。 */
    public static final String CODE_NOT_CONFIGURED = "relay_not_configured";
    /** 宿主自己的码：这一页或这次读取开始之后，偏好里的中继地址或令牌换过了。 */
    public static final String CODE_CHANGED = "relay_changed";
    /** 中继上 50 条都已置顶，新的一条放不下。 */
    public static final String CODE_FULL = "clipboard_full";

    private static final String FULL_URL = "请填写以 https:// 开头的完整中继地址。";
    private static final String[] LOOPBACK_HOSTS = {"localhost", "127.0.0.1", "[::1]"};
    /** 绑定取 SHA-256 的前 16 字节，与 Rust 给令牌空间编号的长度相同；它在本机内存里比较，落盘只在 AndroidKeyStore 加密的中继密钥串里（标明那份属于哪个中继），不发出去。 */
    private static final int BINDING_BYTES = 16;

    /** 云剪贴板走的路。 */
    public enum Route { ACCOUNT, RELAY }

    private CloudClipboardRelayPolicy() {}

    /**
     * 驱动和中继给出的已知错误码，与 `crates/client-services/src/account/clipboard.rs` 的 `CLIPBOARD_ERROR_CODES` 相同。Android 自己跑中继的驱动（{@link app.msime.android.CloudClipboardRelay}），驱动报的错要按 Rust 那条路（`RelayError::code`）同样归类，面板和设置页才给出相同的说法。
     */
    // 不用 Set.of：它要 API 30，minSdk 是 28。
    private static final java.util.List<String> KNOWN_CODES = Arrays.asList(
        "clipboard_device_pending", "clipboard_sync_disabled", "clipboard_key_conflict", "clipboard_device_limit",
        "clipboard_e2e_required", "invalid_clipboard_device", "invalid_clipboard_key", "invalid_clipboard_ciphertext",
        "invalid_clipboard_text", "invalid_device_id", "invalid_search", "invalid_response", "invalid_state",
        "not_found", "account_invalid", "account_unauthorized", "account_rate_limited", "account_storage",
        "account_cancelled", "account_conflict", "account_unavailable", "account_banned", CODE_FULL,
        CODE_UNAUTHORIZED, CODE_NOT_CONFIGURED, CODE_UNAVAILABLE, CODE_STORAGE);

    /** 经中继跑驱动时，驱动报的错（状态码和码）交给界面的错误码：已知的码原样保留，其余按状态码归类，400 是请求不合规，404 是不存在，别的都算中继不可用。 */
    public static String driverFailureCode(int status, String code) {
        if (code != null && KNOWN_CODES.contains(code)) return code;
        return switch (status) {
            case 400 -> "invalid_cloud_clipboard";
            case 404 -> "not_found";
            default -> CODE_UNAVAILABLE;
        };
    }

    /** 中继地址的问题，没有问题时为 null：https（http 只放行本机回环地址）、有主机名、可以带路径前缀，不带查询串、片段和用户信息，不超过 2048 字节。 */
    public static String urlProblem(String url) {
        if (url == null || url.isEmpty()) return "请填写中继地址。";
        if (TextPolicy.utf8Length(url) > MAX_URL_BYTES) return "中继地址过长。";
        for (int index = 0; index < url.length(); index++) {
            char c = url.charAt(index);
            if (c < 0x20 || c == 0x7f) return "中继地址不能包含控制字符。";
        }
        int separator = url.indexOf("://");
        if (separator <= 0) return FULL_URL;
        String scheme = TextPolicy.lowercase(url.substring(0, separator));
        String rest = url.substring(separator + 3);
        int end = rest.length();
        for (char stop : new char[] {'/', '\\', '?', '#'}) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end) end = at;
        }
        String authority = rest.substring(0, end);
        if (authority.isEmpty()) return FULL_URL;
        if (authority.indexOf('@') >= 0) return "中继地址不能包含用户名或密码，令牌请填在下面。";
        if (url.indexOf('?') >= 0 || url.indexOf('#') >= 0) return "中继地址不能带查询串或片段。";
        String host = host(authority);
        if (host == null) return FULL_URL;
        if ("https".equals(scheme)) return null;
        if ("http".equals(scheme)) {
            return Arrays.asList(LOOPBACK_HOSTS).contains(TextPolicy.lowercase(host))
                ? null : "中继必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。";
        }
        return FULL_URL;
    }

    /** 中继令牌的问题，没有问题时为 null：16 到 256 个可打印 ASCII 字符，不含空格。 */
    public static String tokenProblem(String token) {
        if (token == null || token.isEmpty()) return "请填写中继令牌。";
        for (int index = 0; index < token.length(); index++) {
            char c = token.charAt(index);
            if (c < 0x21 || c > 0x7e) return "中继令牌只能包含可打印的 ASCII 字符，不能有空格。";
        }
        if (token.length() < MIN_TOKEN_CHARS || token.length() > MAX_TOKEN_CHARS)
            return "中继令牌应为 " + MIN_TOKEN_CHARS + " 到 " + MAX_TOKEN_CHARS + " 个字符。";
        return null;
    }

    /** 偏好里是否配置了中继，与 Rust 的 `CloudClipboardRelayPreferences::is_configured` 相同：只看地址。 */
    public static boolean configured(String url) {
        return url != null && !url.isEmpty();
    }

    /** 共享层 `msime_client_cloud_clipboard_source` 答的 `source`；不认识的值为 null，调用方按失败处理，不当作水杉账号。 */
    public static Route route(String source) {
        if ("relay".equals(source)) return Route.RELAY;
        if ("account".equals(source)) return Route.ACCOUNT;
        return null;
    }

    /**
     * 偏好里这一份中继配置的绑定：没配置时为空串，否则是 `relay:` 加上地址和令牌摘要的十六进制。
     *
     * <p>键盘面板拿它代替账号编号认出这一页条目属于谁：地址或令牌换过，已经画出来的条目和还没回来的读取就都作废，不会把旧中继的条目插进输入框，也不会把一条记录发到用户刚换掉的中继。令牌本身不留在绑定里。
     */
    public static String binding(String url, String token) {
        if (!configured(url)) return "";
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException(impossible);
        }
        digest.update(url.getBytes(StandardCharsets.UTF_8));
        digest.update((byte) 0);
        digest.update(TextPolicy.emptyIfNull(token).getBytes(StandardCharsets.UTF_8));
        return "relay:" + DigestPolicy.hex(Arrays.copyOf(digest.digest(), BINDING_BYTES));
    }

    /** 经中继失败时给人看的一句话；不是中继特有的错误码时为 null，由调用方用自己的说法。 */
    public static String failureMessage(String code) {
        if (code == null) return null;
        return switch (code) {
            case CODE_UNAUTHORIZED -> "中继拒绝了这个令牌，请检查云剪贴板设置里的中继令牌";
            case CODE_UNAVAILABLE -> "连不上中继，请检查中继地址和网络；证书须由公网信任的机构签发";
            case CODE_STORAGE -> "这台设备上的中继密钥读写失败，请稍后再试";
            case CODE_NOT_CONFIGURED -> "中继设置不完整，请重新填写中继地址和令牌";
            case CODE_CHANGED -> "中继设置已改变，请重新打开页面";
            case CODE_FULL -> "云剪贴板已满 50 条且都已置顶，请先取消置顶或删除一些";
            default -> null;
        };
    }

    /** 主机名（IPv6 连同方括号），端口部分不对或主机名为空时为 null。 */
    private static String host(String authority) {
        String host;
        String port;
        if (authority.startsWith("[")) {
            int close = authority.indexOf(']');
            if (close < 0) return null;
            host = authority.substring(0, close + 1);
            String tail = authority.substring(close + 1);
            if (!tail.isEmpty() && !tail.startsWith(":")) return null;
            port = tail.isEmpty() ? "" : tail.substring(1);
        } else {
            int colon = authority.lastIndexOf(':');
            host = colon < 0 ? authority : authority.substring(0, colon);
            port = colon < 0 ? "" : authority.substring(colon + 1);
        }
        if (host.isEmpty() || host.equals("[]")) return null;
        for (int index = 0; index < host.length(); index++) {
            if (TextPolicy.isWhitespace(host.charAt(index))) return null;
        }
        if (port.length() > 5) return null;
        for (int index = 0; index < port.length(); index++) {
            if (!TextPolicy.isAsciiDigit(port.charAt(index))) return null;
        }
        return port.isEmpty() || Integer.parseInt(port) <= 65535 ? host : null;
    }
}
