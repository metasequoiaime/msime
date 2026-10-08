import { utf8Length } from "../keyboard/Utf8";

export const MAX_SESSION_BYTES: number = 64 * 1024;

export function sessionFitsStorage(value: string): boolean {
  return value.length > 0 && utf8Length(value) <= MAX_SESSION_BYTES;
}

export function sessionWriteComplete(value: string, written: number): boolean {
  return written === utf8Length(value);
}

type JsonObject = Record<string, unknown>;

function jsonObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function parseObject(text: string): JsonObject | null {
  try {
    const value: unknown = JSON.parse(text);
    return jsonObject(value) ? value : null;
  } catch {
    return null;
  }
}

/**
 * 把原生匿名账号的 `anonymous-session.json` 读成 `AccountCloudBridge` 保存的扁平会话文档。
 *
 * 原生客户端（以及其他所有客户端）保存的是 `{tokens: {access_token, refresh_token, token_type, expires_in, user}, expires_at_unix_ms}`；桥接保存的是 `{access_token, refresh_token, token_type, expires_at, user}`。这里只转换结构：桥接会像校验自己的会话一样校验各字段，所以不符合这个结构的内容一律返回 null，而不是转换一半。
 */
export function anonymousSessionForBridge(saved: string): string | null {
  const value: JsonObject | null = parseObject(saved);
  if (
    value === null ||
    !jsonObject(value.tokens) ||
    !Number.isSafeInteger(value.expires_at_unix_ms)
  ) {
    return null;
  }
  const tokens: JsonObject = value.tokens;
  return JSON.stringify({
    access_token: tokens.access_token,
    refresh_token: tokens.refresh_token,
    token_type: tokens.token_type,
    expires_at: value.expires_at_unix_ms,
    user: tokens.user,
  });
}

/**
 * 桥接轮换后的会话，按原生格式写回，这样原生客户端仍把它读作本设备的匿名会话。
 *
 * `expires_in` 是会话在 `now` 时刻剩余的时长，取整秒且至少为一，因为原生格式两者都带，从 `expires_at_unix_ms` 读取过期时间，并拒绝 `expires_in` 为零的已保存会话。`value` 不是桥接会话时返回 null，存储会拒绝写入它。
 */
export function anonymousSessionForStorage(value: string, now: number): string | null {
  const session: JsonObject | null = parseObject(value);
  if (session === null || !jsonObject(session.user) || !Number.isSafeInteger(session.expires_at)) {
    return null;
  }
  const expiresAt: number = session.expires_at as number;
  if (expiresAt < 0) {
    return null;
  }
  return JSON.stringify({
    tokens: {
      access_token: session.access_token,
      refresh_token: session.refresh_token,
      token_type: session.token_type,
      expires_in: Math.max(1, Math.round((expiresAt - now) / 1000)),
      user: session.user,
    },
    expires_at_unix_ms: expiresAt,
  });
}
