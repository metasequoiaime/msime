import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";

/**
 * AI 辅助接口地址的传输策略，与 crates/client-core/src/ai/endpoint.rs 一致：https 不限主机；http 只能指向本机或局域网，也就是回环（`localhost`、`127.0.0.0/8`、`::1`）、RFC 1918 私有 IPv4、`100.64.0.0/10`、链路本地（`169.254/16`、`fe80::/10`）、IPv6 唯一本地地址 `fc00::/7` 和 `.local` 主机名。公网主机必须走 https，API Token 不能明文经过互联网。检查时不做 DNS 解析，除 `localhost` 和 `.local` 以外的域名一律按公网处理。
 *
 * 这里没有 WHATWG URL 解析器可用（node 测试和设备上都要能跑），所以只认规范写法：IPv4 必须是四段十进制且不带前导零，其他看起来像数字的主机（`10.1`、`0x7f000001`、`010.0.0.1`）一律按无效拒绝，而不是猜它指向哪里——系统 http 模块交给 curl 时，这类写法可能被解析成别的地址。拒绝的方向总是安全的。用例与 Rust 共用 shared/contracts/ai-endpoint/cases.json。
 */

/** `allowed`、`invalid` 或 `cleartext_public`，与共用用例的 `result` 相同。 */
export interface AiEndpointCheck {
  result: string;
  /** 保存 Token 用的来源键 `scheme://host:port`；地址不合规时为 null。 */
  origin: string | null;
}

const MAX_ENDPOINT_BYTES: number = 2048;

export class AiEndpointPolicy {
  /** 公网 http 地址被拒绝时给用户看的说明。 */
  static readonly CLEARTEXT_HINT: string =
    "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网";

  static check(endpoint: string): AiEndpointCheck {
    const invalid: AiEndpointCheck = { result: "invalid", origin: null };
    if (
      endpoint.length === 0 ||
      utf8Length(endpoint) > MAX_ENDPOINT_BYTES ||
      TextPolicy.hasControl(endpoint) ||
      endpoint.includes("#") ||
      // curl 把反斜杠当成主机或用户名的一部分：`http://127.0.0.1\@example.com` 会连到 example.com，不能按反斜杠之前的回环地址放行。
      endpoint.includes("\\")
    ) {
      return invalid;
    }
    const lower: string = endpoint.toLowerCase();
    let scheme: string;
    if (lower.startsWith("https://")) scheme = "https";
    else if (lower.startsWith("http://")) scheme = "http";
    else return invalid;
    const rest: string = endpoint.substring(scheme.length + 3);
    const end: number = rest.search(/[\/?]/);
    const authority: string = end < 0 ? rest : rest.substring(0, end);
    if (authority.length === 0 || authority.includes("@")) return invalid;

    let host: string;
    let portText: string;
    if (authority.startsWith("[")) {
      const close: number = authority.indexOf("]");
      if (close < 0) return invalid;
      const segments: number[] | null = AiEndpointPolicy.ipv6Segments(
        authority.substring(1, close),
      );
      if (segments === null) return invalid;
      host = `[${AiEndpointPolicy.ipv6Text(segments)}]`;
      const after: string = authority.substring(close + 1);
      if (after.length > 0 && !after.startsWith(":")) return invalid;
      portText = after.substring(1);
    } else {
      const colon: number = authority.indexOf(":");
      host = (colon < 0 ? authority : authority.substring(0, colon)).toLowerCase();
      portText = colon < 0 ? "" : authority.substring(colon + 1);
      if (host.length === 0 || !/^[a-z0-9._-]+$/.test(host)) return invalid;
      if (AiEndpointPolicy.looksNumeric(host) && AiEndpointPolicy.ipv4Octets(host) === null) {
        return invalid;
      }
    }
    let port: number = scheme === "https" ? 443 : 80;
    if (portText.length > 0) {
      if (!/^\d{1,5}$/.test(portText)) return invalid;
      port = Number(portText);
      if (port > 65535) return invalid;
    }
    if (scheme === "http" && !AiEndpointPolicy.isLocalNetworkHost(host)) {
      return { result: "cleartext_public", origin: null };
    }
    return { result: "allowed", origin: `${scheme}://${host}:${port}` };
  }

  static allowed(endpoint: string): boolean {
    return AiEndpointPolicy.check(endpoint).result === "allowed";
  }

  /** `host` 是规范化后的主机：小写，IPv6 带方括号。 */
  static isLocalNetworkHost(host: string): boolean {
    if (host.startsWith("[") && host.endsWith("]")) {
      const segments: number[] | null = AiEndpointPolicy.ipv6Segments(
        host.substring(1, host.length - 1),
      );
      if (segments === null) return false;
      let loopback: boolean = segments[7] === 1;
      for (let index: number = 0; index < 7; index++) {
        if (segments[index] !== 0) loopback = false;
      }
      // fe80::/10 链路本地；fc00::/7 唯一本地地址。
      return loopback || (segments[0] & 0xffc0) === 0xfe80 || (segments[0] & 0xfe00) === 0xfc00;
    }
    const octets: number[] | null = AiEndpointPolicy.ipv4Octets(host);
    if (octets !== null) {
      const first: number = octets[0];
      const second: number = octets[1];
      return (
        first === 127 ||
        first === 10 ||
        (first === 172 && second >= 16 && second <= 31) ||
        (first === 192 && second === 168) ||
        // 100.64.0.0/10：运营商级 NAT 与 Tailscale。
        (first === 100 && second >= 64 && second <= 127) ||
        (first === 169 && second === 254)
      );
    }
    const domain: string = host.toLowerCase();
    if (domain === "localhost") return true;
    if (!domain.endsWith(".local")) return false;
    const label: string = domain.substring(0, domain.length - ".local".length);
    return label.length > 0 && !label.endsWith(".");
  }

  /** 最后一段以数字开头的主机按 IPv4 处理（与 WHATWG 一致），这时必须是规范写法。 */
  private static looksNumeric(host: string): boolean {
    const labels: string[] = host.split(".");
    let last: string = labels[labels.length - 1];
    if (last.length === 0 && labels.length > 1) last = labels[labels.length - 2];
    return /^\d/.test(last);
  }

  /** 四段十进制、每段 0 到 255、不带前导零；其他写法返回 null。 */
  private static ipv4Octets(host: string): number[] | null {
    const parts: string[] = host.split(".");
    if (parts.length !== 4) return null;
    const octets: number[] = [];
    for (const part of parts) {
      if (!/^(0|[1-9]\d{0,2})$/.test(part)) return null;
      const value: number = Number(part);
      if (value > 255) return null;
      octets.push(value);
    }
    return octets;
  }

  /** 展开 IPv6 文本为 8 个 16 位段，允许末尾内嵌 IPv4；带区域标识或格式不对时返回 null。 */
  private static ipv6Segments(text: string): number[] | null {
    if (text.length === 0 || !/^[0-9a-fA-F:.]+$/.test(text)) return null;
    const halves: string[] = text.split("::");
    if (halves.length > 2) return null;
    const head: string[] = halves[0].length > 0 ? halves[0].split(":") : [];
    const tail: string[] = halves.length === 2 && halves[1].length > 0 ? halves[1].split(":") : [];
    const groups: string[] = head.concat(tail);
    const values: number[] = [];
    for (let index: number = 0; index < groups.length; index++) {
      const group: string = groups[index];
      if (group.includes(".")) {
        // 内嵌的 IPv4 只能出现在最后。
        if (index !== groups.length - 1) return null;
        const octets: number[] | null = AiEndpointPolicy.ipv4Octets(group);
        if (octets === null) return null;
        values.push(octets[0] * 256 + octets[1], octets[2] * 256 + octets[3]);
      } else {
        if (!/^[0-9a-fA-F]{1,4}$/.test(group)) return null;
        values.push(parseInt(group, 16));
      }
    }
    const headCount: number = AiEndpointPolicy.segmentCount(head);
    if (halves.length === 1) return values.length === 8 ? values : null;
    const fill: number = 8 - values.length;
    if (fill < 1) return null;
    const result: number[] = values.slice(0, headCount);
    for (let index: number = 0; index < fill; index++) result.push(0);
    return result.concat(values.slice(headCount));
  }

  private static segmentCount(groups: string[]): number {
    let count: number = 0;
    for (const group of groups) count += group.includes(".") ? 2 : 1;
    return count;
  }

  /** RFC 5952 的规范文本：小写、去前导零、最长的一段（至少两段）连续零压缩成 `::`，与 Rust 的 `Url::host_str` 相同。 */
  private static ipv6Text(segments: number[]): string {
    let bestStart: number = -1;
    let bestLength: number = 0;
    let index: number = 0;
    while (index < 8) {
      if (segments[index] !== 0) {
        index++;
        continue;
      }
      let end: number = index;
      while (end < 8 && segments[end] === 0) end++;
      if (end - index > bestLength) {
        bestStart = index;
        bestLength = end - index;
      }
      index = end;
    }
    const hex: string[] = segments.map((segment: number): string => segment.toString(16));
    if (bestLength < 2) return hex.join(":");
    return `${hex.slice(0, bestStart).join(":")}::${hex.slice(bestStart + bestLength).join(":")}`;
  }
}
