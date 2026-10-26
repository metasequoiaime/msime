import type { Preferences } from "../core/preferences-model";
import type {
  CloudClipboardRelayPreferences,
  CloudClipboardWebDavPreferences,
} from "./cloud-clipboard-relay-preferences";
export type {
  CloudClipboardRelayPreferences,
  CloudClipboardWebDavPreferences,
} from "./cloud-clipboard-relay-preferences";

const MAX_URL_BYTES = 2048;
const MIN_TOKEN_CHARS = 16;
const MAX_TOKEN_CHARS = 256;
const MAX_WEBDAV_CREDENTIAL_BYTES = 256;
const LOOPBACK_HOSTS = ["localhost", "127.0.0.1", "[::1]"];

/** 地址校验提示里随用途变化的说法：中继地址，或 WebDAV 的服务器地址。 */
interface EndpointWords {
  name: string;
  server: string;
  credentials: string;
}

const RELAY_WORDS: EndpointWords = { name: "中继地址", server: "中继", credentials: "令牌" };
const WEBDAV_WORDS: EndpointWords = {
  name: "服务器地址",
  server: "WebDAV 服务器",
  credentials: "账号和应用密码",
};

/** 地址的问题，没有问题时为空。规则与 `crates/client-core/src/preferences.rs` 的 `valid_relay_url` 相同：https（http 只放行本机回环地址）、有主机名、可以带路径前缀，不带查询串、片段和用户信息。只看 `new URL` 的结果不够：末尾单独的 `?`、`#` 在这里解析成空串，Rust 那边却算作带了查询串或片段，保存会失败，所以按原文再查一遍。中继和 WebDAV 网盘用同一条规则，只是提示的说法不同。 */
function endpointIssue(url: string, words: EndpointWords): string {
  const full = `请填写以 https:// 开头的完整${words.name}。`;
  if (!url) return `请填写${words.name}。`;
  if (new TextEncoder().encode(url).length > MAX_URL_BYTES) return `${words.name}过长。`;
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(url)) return `${words.name}不能包含控制字符。`;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return full;
  }
  const authority = url.slice(url.indexOf("://") + 3);
  if (!url.includes("://") || authority.startsWith("/") || !parsed.hostname) {
    return full;
  }
  if (parsed.username || parsed.password) {
    return `${words.name}不能包含用户名或密码，${words.credentials}请填在下面。`;
  }
  if (url.includes("?") || url.includes("#")) return `${words.name}不能带查询串或片段。`;
  if (parsed.protocol === "https:") return "";
  if (parsed.protocol === "http:") {
    return LOOPBACK_HOSTS.includes(parsed.hostname)
      ? ""
      : `${words.server}必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。`;
  }
  return full;
}

/** 中继地址的问题，没有问题时为空，规则见 {@link endpointIssue}。 */
export function cloudClipboardRelayUrlIssue(url: string): string {
  return endpointIssue(url, RELAY_WORDS);
}

/** WebDAV 服务器地址（用户选的文件夹）的问题，没有问题时为空。规则与中继地址相同。 */
export function cloudClipboardWebDavUrlIssue(url: string): string {
  return endpointIssue(url, WEBDAV_WORDS);
}

// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

/** WebDAV 账号的问题，没有问题时为空：1 到 256 字节，不含控制字符，也不含冒号（HTTP Basic 鉴权用冒号分隔账号和密码）。与 Rust 的 `valid_webdav_username` 相同。 */
export function cloudClipboardWebDavUsernameIssue(username: string): string {
  if (!username) return "请填写网盘账号。";
  if (new TextEncoder().encode(username).length > MAX_WEBDAV_CREDENTIAL_BYTES) {
    return "网盘账号过长。";
  }
  if (CONTROL.test(username)) return "网盘账号不能包含控制字符。";
  if (username.includes(":")) return "网盘账号不能包含冒号。";
  return "";
}

/** WebDAV 应用密码的问题，没有问题时为空：1 到 256 字节，不含控制字符。与 Rust 的 `valid_webdav_password` 相同。 */
export function cloudClipboardWebDavPasswordIssue(password: string): string {
  if (!password) return "请填写应用密码。";
  if (new TextEncoder().encode(password).length > MAX_WEBDAV_CREDENTIAL_BYTES) {
    return "应用密码过长。";
  }
  if (CONTROL.test(password)) return "应用密码不能包含控制字符。";
  return "";
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

/** 偏好里是否配置了中继或 WebDAV 网盘；配置了就只走它。 */
export function cloudClipboardRelayConfigured(preferences: Preferences | undefined): boolean {
  return Boolean(
    preferences?.cloud_clipboard_relay?.url || preferences?.cloud_clipboard_webdav?.url,
  );
}

/** 写入或清除中继配置后的偏好。写入中继时去掉 WebDAV 网盘（两者只能二选一，Rust 会拒绝同时配置的文档）；清除时去掉这两个键而不是写空值，与 Rust 没配置时不写进文档的形状一致，保存后草稿和快照才相等。 */
export function withCloudClipboardRelay(
  preferences: Preferences,
  relay: CloudClipboardRelayPreferences | undefined,
): Preferences {
  const next = { ...preferences };
  delete next.cloud_clipboard_webdav;
  if (relay) return { ...next, cloud_clipboard_relay: relay };
  delete next.cloud_clipboard_relay;
  return next;
}

/** 写入 WebDAV 网盘配置后的偏好，同时去掉中继；传 `undefined` 时只去掉网盘。 */
export function withCloudClipboardWebDav(
  preferences: Preferences,
  webdav: CloudClipboardWebDavPreferences | undefined,
): Preferences {
  const next = { ...preferences };
  if (webdav) {
    delete next.cloud_clipboard_relay;
    return { ...next, cloud_clipboard_webdav: webdav };
  }
  delete next.cloud_clipboard_webdav;
  return next;
}
