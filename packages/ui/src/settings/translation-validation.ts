import { utf8ByteLength } from "../core/text";

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
  if (utf8ByteLength(endpoint) > 2048) return "接口地址过长。";
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

/** DeepL 官方 API 的两个主机，与 crates/client-core/src/translation.rs 的 `is_deepl_api_endpoint` 一致，两边都按 shared/contracts/custom-translation/cases.json 的用例核对。 */
const DEEPL_API_HOSTS = ["api.deepl.com", "api-free.deepl.com"];

/** 地址是否指向 DeepL 官方 API：https、默认端口、主机名恰好是 `api.deepl.com` 或 `api-free.deepl.com`。共享层对这种地址改用 DeepL 的请求格式。 */
export function isDeepLApiEndpoint(endpoint: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(endpoint.trim());
  } catch {
    return false;
  }
  return (
    parsed.protocol === "https:" && parsed.port === "" && DEEPL_API_HOSTS.includes(parsed.hostname)
  );
}

/** 自定义翻译的 API Key 问题：DeepL 官方地址没有密钥时共享层不构造请求，键盘不会发出翻译，所以要在设置页说出来，而不是保存后静默没有释义。 */
export function customTranslationKeyIssue(endpoint: string, apiKey: string): string {
  if (isDeepLApiEndpoint(endpoint) && !apiKey.trim()) {
    return "DeepL 官方 API 需要填写 Auth Key；留空时不会发出翻译请求。";
  }
  return "";
}
