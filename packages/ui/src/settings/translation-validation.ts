export function tencentCredentialIssue(
  secretId: string,
  secretKey: string,
  region: string,
): string {
  if (secretId.length > 4096 || secretKey.length > 4096) return "凭据过长。";
  if (secretId && !/^[A-Za-z0-9_-]+$/.test(secretId)) {
    return "SecretId 只能包含字母、数字、下划线和连字符。";
  }
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(secretKey)) return "SecretKey 不能包含控制字符。";
  if (region.length > 64) return "地域过长。";
  if (region && !/^[A-Za-z0-9-]+$/.test(region)) {
    return "地域只能包含字母、数字和连字符。";
  }
  return "";
}

export function translationEndpointIssue(endpoint: string): string {
  if (!endpoint) return "请填写完整的接口地址。";
  if (new TextEncoder().encode(endpoint).length > 2048) return "接口地址过长。";
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(endpoint)) return "接口地址不能包含控制字符。";
  let parsed: URL;
  try {
    parsed = new URL(endpoint);
  } catch {
    return "请填写以 http:// 或 https:// 开头的完整接口地址。";
  }
  if (parsed.username || parsed.password || parsed.hash || !parsed.hostname) {
    return "接口地址不能包含凭据或片段。";
  }
  if (parsed.protocol === "https:") return "";
  if (
    parsed.protocol === "http:" &&
    ["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname)
  ) {
    return "";
  }
  if (parsed.protocol !== "http:") {
    return "请填写以 http:// 或 https:// 开头的完整接口地址。";
  }
  return "远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址。";
}
