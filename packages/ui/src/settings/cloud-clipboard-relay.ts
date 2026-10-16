import type { Preferences } from "../core/preferences-model";
import type { CloudClipboardRelayPreferences } from "./cloud-clipboard-relay-preferences";
export type { CloudClipboardRelayPreferences } from "./cloud-clipboard-relay-preferences";

const MAX_URL_BYTES = 2048;
const MIN_TOKEN_CHARS = 16;
const MAX_TOKEN_CHARS = 256;
const LOOPBACK_HOSTS = ["localhost", "127.0.0.1", "[::1]"];

/** 中继地址的问题，没有问题时为空。规则与 `crates/client-core/src/preferences.rs` 的 `valid_relay_url` 相同：https（http 只放行本机回环地址）、有主机名、可以带路径前缀，不带查询串、片段和用户信息。只看 `new URL` 的结果不够：末尾单独的 `?`、`#` 在这里解析成空串，Rust 那边却算作带了查询串或片段，保存会失败，所以按原文再查一遍。 */
export function cloudClipboardRelayUrlIssue(url: string): string {
  if (!url) return "请填写中继地址。";
  if (new TextEncoder().encode(url).length > MAX_URL_BYTES) return "中继地址过长。";
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(url)) return "中继地址不能包含控制字符。";
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return "请填写以 https:// 开头的完整中继地址。";
  }
  const authority = url.slice(url.indexOf("://") + 3);
  if (!url.includes("://") || authority.startsWith("/") || !parsed.hostname) {
    return "请填写以 https:// 开头的完整中继地址。";
  }
  if (parsed.username || parsed.password) {
    return "中继地址不能包含用户名或密码，令牌请填在下面。";
  }
  if (url.includes("?") || url.includes("#")) return "中继地址不能带查询串或片段。";
  if (parsed.protocol === "https:") return "";
  if (parsed.protocol === "http:") {
    return LOOPBACK_HOSTS.includes(parsed.hostname)
      ? ""
      : "中继必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。";
  }
  return "请填写以 https:// 开头的完整中继地址。";
}

/** 中继令牌的问题，没有问题时为空：16 到 256 个可打印 ASCII 字符，不含空格。 */
export function cloudClipboardRelayTokenIssue(token: string): string {
  if (!token) return "请填写中继令牌。";
  if (!/^[\x21-\x7e]*$/.test(token)) return "中继令牌只能包含可打印的 ASCII 字符，不能有空格。";
  if (token.length < MIN_TOKEN_CHARS || token.length > MAX_TOKEN_CHARS) {
    return `中继令牌应为 ${MIN_TOKEN_CHARS} 到 ${MAX_TOKEN_CHARS} 个字符。`;
  }
  return "";
}

/** 偏好里是否配置了中继；配置了就只走中继。 */
export function cloudClipboardRelayConfigured(preferences: Preferences | undefined): boolean {
  return Boolean(preferences?.cloud_clipboard_relay?.url);
}

/** 写入或清除中继配置后的偏好。清除时去掉这个键而不是写空值，与 Rust 没配置时不写进文档的形状一致，保存后草稿和快照才相等。 */
export function withCloudClipboardRelay(
  preferences: Preferences,
  relay: CloudClipboardRelayPreferences | undefined,
): Preferences {
  if (relay) return { ...preferences, cloud_clipboard_relay: relay };
  const next = { ...preferences };
  delete next.cloud_clipboard_relay;
  return next;
}
