// AI 辅助接口地址的传输策略，与 crates/client-core/src/ai/endpoint.rs 逐条一致：https 不限主机；http 只能指向本机或局域网（回环、RFC 1918、100.64.0.0/10、链路本地、IPv6 唯一本地地址、`.local`），因为 API Token 不能明文经过互联网。检查时不做 DNS 解析，除 `localhost` 和 `.local` 以外的域名一律按公网处理。两边共用 shared/contracts/ai-endpoint/cases.json 里的用例。

const MAX_ENDPOINT_BYTES = 2048;

export type AiEndpointProblem = "invalid" | "cleartext_public";

/** 接口地址的问题；没有问题时返回 null。 */
export function aiEndpointProblem(endpoint: string): AiEndpointProblem | null {
  return checkAiEndpoint(endpoint).problem;
}

/** 保存 Token 用的来源键 `scheme://host:port`，端口总是写出来；地址不合规时为 null。 */
export function aiEndpointOrigin(endpoint: string): string | null {
  const { url } = checkAiEndpoint(endpoint);
  if (!url) return null;
  const scheme = url.protocol.slice(0, -1);
  return `${scheme}://${url.hostname.toLowerCase()}:${url.port || (scheme === "https" ? "443" : "80")}`;
}

/** 接口地址不能用时给用户看的说明；地址可用时为空字符串。 */
export function aiEndpointHint(endpoint: string): string {
  switch (aiEndpointProblem(endpoint)) {
    case "cleartext_public":
      return "http:// 只能用于本机或局域网地址（如 localhost、127.0.0.1、192.168.x.x、10.x.x.x、*.local）；公网服务请使用 https://，以免 API Token 明文经过互联网。";
    case "invalid":
      return "请填写完整的接口地址：https://，或本机、局域网的 http://，且不能包含用户名、密码或 # 片段。";
    default:
      return "";
  }
}

function checkAiEndpoint(endpoint: string): { url: URL | null; problem: AiEndpointProblem | null } {
  const invalid = { url: null, problem: "invalid" as const };
  if (
    !endpoint ||
    new TextEncoder().encode(endpoint).length > MAX_ENDPOINT_BYTES ||
    // 控制字符一律不收；`new URL` 会悄悄删掉换行和制表符，不能交给它判断。
    /[\u0000-\u001f\u007f-\u009f]/.test(endpoint) ||
    // `#` 之后都是片段，哪怕是空的；`URL.hash` 对空片段返回空字符串，只能看原文。
    endpoint.includes("#") ||
    // 反斜杠在 WHATWG 里等同 `/`，curl 等宿主却把它当成主机或用户名的一部分，同一个地址会连到不同主机。
    endpoint.includes("\\")
  ) {
    return invalid;
  }
  // `https:///v1` 会被解析成主机 `v1`；要求 `://` 后面紧跟主机。
  const separator = endpoint.indexOf("://");
  if (separator < 0 || endpoint.length <= separator + 3 || endpoint[separator + 3] === "/") {
    return invalid;
  }
  let url: URL;
  try {
    url = new URL(endpoint);
  } catch {
    return invalid;
  }
  if (
    (url.protocol !== "https:" && url.protocol !== "http:") ||
    !url.hostname ||
    url.username ||
    url.password
  ) {
    return invalid;
  }
  if (url.protocol === "http:") {
    // 明文地址的主机必须按解析后的样子书写（大小写除外）；`10.1`、`0x7f000001`、`127.0.0.1.`、百分号编码这类写法手写解析的宿主不认。
    if (writtenHost(endpoint)?.toLowerCase() !== url.hostname.toLowerCase()) return invalid;
    if (!isLocalNetworkHost(url.hostname)) return { url: null, problem: "cleartext_public" };
  }
  return { url, problem: null };
}

/** `://` 之后、端口之前原样书写的主机；IPv6 带方括号。首尾空格与 `new URL` 一样先去掉。 */
function writtenHost(endpoint: string): string | null {
  const trimmed = endpoint.replace(/^ +| +$/g, "");
  const separator = trimmed.indexOf("://");
  if (separator < 0) return null;
  const authority = trimmed.slice(separator + 3).split(/[/?]/)[0];
  if (authority.startsWith("[")) {
    const close = authority.indexOf("]");
    return close < 0 ? null : authority.slice(0, close + 1);
  }
  return authority.split(":")[0];
}

/** `hostname` 取自 `URL.hostname`：IPv4 已规范成点分十进制，IPv6 带方括号且已规范成小写压缩形式。 */
export function isLocalNetworkHost(hostname: string): boolean {
  if (hostname.startsWith("[") && hostname.endsWith("]")) {
    const segments = ipv6Segments(hostname.slice(1, -1));
    if (!segments) return false;
    const loopback = segments.slice(0, 7).every((segment) => segment === 0) && segments[7] === 1;
    return (
      loopback ||
      // fe80::/10 链路本地；fc00::/7 唯一本地地址。
      (segments[0] & 0xffc0) === 0xfe80 ||
      (segments[0] & 0xfe00) === 0xfc00
    );
  }
  const ipv4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(hostname);
  if (ipv4) {
    const [a, b] = [Number(ipv4[1]), Number(ipv4[2])];
    return (
      a === 127 ||
      a === 10 ||
      (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && b === 168) ||
      // 100.64.0.0/10：运营商级 NAT 与 Tailscale。
      (a === 100 && b >= 64 && b <= 127) ||
      (a === 169 && b === 254)
    );
  }
  const domain = hostname.toLowerCase();
  if (domain === "localhost") return true;
  if (!domain.endsWith(".local")) return false;
  const label = domain.slice(0, -".local".length);
  return label.length > 0 && !label.endsWith(".");
}

/** 把 `URL.hostname` 给出的 IPv6 文本展开成 8 个 16 位段；它不会带内嵌 IPv4 或区域标识。 */
function ipv6Segments(text: string): number[] | null {
  const halves = text.split("::");
  if (halves.length > 2) return null;
  const parse = (part: string) => (part ? part.split(":") : []);
  const head = parse(halves[0]);
  const tail = halves.length === 2 ? parse(halves[1]) : [];
  const fill = halves.length === 2 ? 8 - head.length - tail.length : 0;
  if (fill < 0 || (halves.length === 1 && head.length !== 8)) return null;
  const groups = [...head, ...Array<string>(fill).fill("0"), ...tail];
  if (groups.length !== 8 || groups.some((group) => !/^[0-9a-f]{1,4}$/i.test(group))) return null;
  return groups.map((group) => parseInt(group, 16));
}
