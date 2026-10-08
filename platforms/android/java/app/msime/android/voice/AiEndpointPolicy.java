package app.msime.android;

import java.io.IOException;
import java.net.Proxy;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.URL;
import java.net.URLConnection;
import java.util.Locale;

/**
 * AI 辅助接口地址的传输策略：什么样的地址可以收到用户的 API Token 和输入内容。
 *
 * <p>与 crates/client-core/src/ai/endpoint.rs 逐条一致，两边跑同一组用例 shared/contracts/ai-endpoint/cases.json：https 不限主机；http 只能指向本机或局域网，也就是回环（`localhost`、127.0.0.0/8、::1）、RFC 1918 私有 IPv4、100.64.0.0/10、链路本地（169.254/16、fe80::/10）、IPv6 唯一本地地址 fc00::/7 和 `.local` 主机名。公网主机必须走 https，API Token 不能明文经过互联网。
 *
 * <p>这里从不做 DNS 解析：IP 只认严格写法的字面量（点分十进制不带前导零、方括号里的 IPv6 不带内嵌 IPv4 和区域标识），其余一律按域名处理，域名只认 `localhost` 和 `.local`。写法不规范的 IP 因此会被当成公网域名拒绝，方向是安全的。
 *
 * <p>云候选（Google）不走这里，仍然只接受 https；见 {@link OnlineCandidatePolicy#validURL}。
 */
public final class AiEndpointPolicy {
    /** 接口地址能接受的最大 UTF-8 字节数，与 client-core 和设置页一致。 */
    public static final int MAX_ENDPOINT_BYTES = 2048;
    /** 公网 http 地址被拒绝时给用户看的说明。 */
    public static final String CLEARTEXT_REASON = "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网";

    public enum Result {
        ALLOWED,
        /** http 地址指向的不是本机或局域网主机。 */
        CLEARTEXT_PUBLIC,
        /** 不是带主机的完整 http(s) 地址，或带了用户名、密码、# 片段、控制字符。 */
        INVALID
    }

    private AiEndpointPolicy() {}

    public static Result check(String endpoint) {
        URI uri = parsed(endpoint);
        if (uri == null) return Result.INVALID;
        if ("http".equalsIgnoreCase(uri.getScheme()) && !isLocalNetworkHost(uri.getHost()))
            return Result.CLEARTEXT_PUBLIC;
        return Result.ALLOWED;
    }

    public static boolean allowed(String endpoint) {
        return check(endpoint) == Result.ALLOWED;
    }

    /** 通过检查的地址（去掉首尾空白后解析），不通过时为 null。 */
    public static URI uri(String endpoint) {
        return check(endpoint) == Result.ALLOWED ? parsed(endpoint) : null;
    }

    /** 保存 Token 用的来源键 `scheme://host:port`：主机小写，IPv6 带方括号并写成规范的压缩形式，端口总是写出来（https 默认 443，http 默认 80）；地址不通过时为 null。 */
    public static String credentialOrigin(String endpoint) {
        URI uri = uri(endpoint);
        return uri == null ? null : origin(uri);
    }

    /** 已通过检查的地址的来源键，见 {@link #credentialOrigin(String)}。 */
    public static String origin(URI uri) {
        String scheme = uri.getScheme().toLowerCase(Locale.ROOT);
        int port = uri.getPort() == -1 ? ("https".equals(scheme) ? 443 : 80) : uri.getPort();
        return scheme + "://" + canonicalHost(uri.getHost()) + ":" + port;
    }

    /** 本机、局域网的 http 地址必须直连：系统或 Wi-Fi 设置的代理会收到明文 Token，而代理可能在公网上。https 地址照旧按系统设置走。 */
    public static boolean bypassesProxy(URL target) {
        return "http".equalsIgnoreCase(target.getProtocol());
    }

    /** 打开 AI 请求的连接；http 地址绕过代理，见 {@link #bypassesProxy(URL)}。 */
    public static URLConnection open(URL target) throws IOException {
        return bypassesProxy(target) ? target.openConnection(Proxy.NO_PROXY) : target.openConnection();
    }

    /** 主机是否属于本机或局域网。`host` 取自 {@link URI#getHost()}，IPv6 带方括号。 */
    public static boolean isLocalNetworkHost(String host) {
        if (host == null || host.isEmpty()) return false;
        if (host.startsWith("[") && host.endsWith("]")) {
            int[] segments = ipv6Segments(host.substring(1, host.length() - 1));
            if (segments == null) return false;
            boolean loopback = segments[7] == 1;
            for (int index = 0; index < 7; index++) loopback &= segments[index] == 0;
            // fe80::/10 链路本地；fc00::/7 唯一本地地址。
            return loopback || (segments[0] & 0xffc0) == 0xfe80 || (segments[0] & 0xfe00) == 0xfc00;
        }
        int[] octets = ipv4Octets(host);
        if (octets != null) {
            int a = octets[0];
            int b = octets[1];
            return a == 127 || a == 10 || (a == 172 && b >= 16 && b <= 31) || (a == 192 && b == 168)
                // 100.64.0.0/10：运营商级 NAT 与 Tailscale。
                || (a == 100 && b >= 64 && b <= 127) || (a == 169 && b == 254);
        }
        String domain = host.toLowerCase(Locale.ROOT);
        if (domain.equals("localhost")) return true;
        if (!domain.endsWith(".local")) return false;
        String label = domain.substring(0, domain.length() - ".local".length());
        return !label.isEmpty() && !label.endsWith(".");
    }

    private static URI parsed(String endpoint) {
        if (endpoint == null || endpoint.isEmpty() || TextPolicy.utf8Length(endpoint) > MAX_ENDPOINT_BYTES
                || TextPolicy.hasControl(endpoint) || !TextPolicy.validUnicode(endpoint)
                // `#` 之后都是片段，哪怕是空的。
                || endpoint.contains("#")) return null;
        String value = TextPolicy.trimmed(endpoint);
        // `https:///v1` 要求 `://` 后面紧跟主机，免得路径被当成主机。
        int separator = value.indexOf("://");
        if (separator < 0 || value.length() <= separator + 3 || value.charAt(separator + 3) == '/') return null;
        URI uri;
        try {
            uri = new URI(value);
        } catch (URISyntaxException error) {
            return null;
        }
        String scheme = uri.getScheme();
        if (scheme == null || (!"https".equalsIgnoreCase(scheme) && !"http".equalsIgnoreCase(scheme))
                || uri.getHost() == null || uri.getHost().isEmpty() || uri.getRawUserInfo() != null
                || uri.getRawFragment() != null || uri.getPort() < -1 || uri.getPort() > 65535) return null;
        return uri;
    }

    /** 严格的点分十进制：四段、每段 0 到 255、不带前导零。不是这种写法时返回 null。 */
    private static int[] ipv4Octets(String host) {
        String[] parts = host.split("\\.", -1);
        if (parts.length != 4) return null;
        int[] octets = new int[4];
        for (int index = 0; index < 4; index++) {
            String part = parts[index];
            if (part.isEmpty() || part.length() > 3 || (part.length() > 1 && part.charAt(0) == '0')) return null;
            int value = 0;
            for (int position = 0; position < part.length(); position++) {
                char digit = part.charAt(position);
                if (digit < '0' || digit > '9') return null;
                value = value * 10 + (digit - '0');
            }
            if (value > 255) return null;
            octets[index] = value;
        }
        return octets;
    }

    /** 方括号里的 IPv6 文本展开成 8 个 16 位段；内嵌 IPv4、区域标识等其他写法返回 null。 */
    private static int[] ipv6Segments(String text) {
        int gap = text.indexOf("::");
        if (gap >= 0 && text.indexOf("::", gap + 1) >= 0) return null;
        String[] head = groups(gap >= 0 ? text.substring(0, gap) : text);
        String[] tail = gap >= 0 ? groups(text.substring(gap + 2)) : new String[0];
        if (head == null || tail == null) return null;
        int fill = 8 - head.length - tail.length;
        if (gap >= 0 ? fill < 1 : fill != 0) return null;
        int[] segments = new int[8];
        for (int index = 0; index < head.length; index++) segments[index] = hextet(head[index]);
        for (int index = 0; index < tail.length; index++) segments[8 - tail.length + index] = hextet(tail[index]);
        for (int segment : segments) if (segment < 0) return null;
        return segments;
    }

    private static String[] groups(String part) {
        if (part.isEmpty()) return new String[0];
        String[] groups = part.split(":", -1);
        for (String group : groups) if (group.isEmpty()) return null;
        return groups;
    }

    private static int hextet(String group) {
        if (group.length() > 4) return -1;
        int value = 0;
        for (int position = 0; position < group.length(); position++) {
            int digit = Character.digit(group.charAt(position), 16);
            if (digit < 0) return -1;
            value = value * 16 + digit;
        }
        return value;
    }

    /** 主机小写；IPv6 改写成 RFC 5952 的压缩形式，和设置页（WHATWG URL）与 client-core 算出的来源键一致。 */
    private static String canonicalHost(String host) {
        String lower = host.toLowerCase(Locale.ROOT);
        if (!lower.startsWith("[") || !lower.endsWith("]")) return lower;
        int[] segments = ipv6Segments(lower.substring(1, lower.length() - 1));
        if (segments == null) return lower;
        int bestStart = -1;
        int bestLength = 1;
        for (int start = 0; start < 8;) {
            if (segments[start] != 0) {
                start++;
                continue;
            }
            int end = start;
            while (end < 8 && segments[end] == 0) end++;
            if (end - start > bestLength) {
                bestStart = start;
                bestLength = end - start;
            }
            start = end;
        }
        StringBuilder result = new StringBuilder("[");
        for (int index = 0; index < 8; index++) {
            if (index == bestStart) {
                result.append("::");
                index += bestLength - 1;
                continue;
            }
            if (result.length() > 1 && result.charAt(result.length() - 1) != ':') result.append(':');
            result.append(Integer.toHexString(segments[index]));
        }
        return result.append(']').toString();
    }
}
