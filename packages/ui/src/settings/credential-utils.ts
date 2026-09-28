import { errorCode } from "../core/error-code";

export function providerCredentialErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "provider_credentials_invalid_endpoint":
      return "接口地址必须是完整的 HTTPS 地址，且不能包含用户名、密码或 # 片段。";
    case "provider_credentials_invalid_model":
      return "请先填写模型。";
    case "provider_credentials_invalid_provider":
      return "请先选择服务商。";
    case "provider_credentials_invalid_token":
    case "provider_credentials_invalid_secret":
      return "凭据只能包含可见的 ASCII 字符，且不能是示例占位值。";
    case "provider_credentials_token_required":
      return "请填写凭据。";
    case "provider_credentials_invalid_region":
      return "地域只能包含小写字母、数字和连字符，例如 ap-guangzhou。";
    case "provider_credentials_too_many_profiles":
      return "已保存的 AI 服务商过多，请先清除不再使用的凭据。";
    case "provider_credentials_existing_invalid":
      return "现有配置文件不是仅限当前用户读写的有效 JSON，请修复或删除后重试。";
    case "provider_credentials_location":
      return "无法确定用户配置目录，请检查 HOME 或 XDG_CONFIG_HOME。";
    default:
      return "无法写入凭据文件，请检查用户配置目录的权限。";
  }
}

export function aiCredentialOrigin(endpoint: string): string | null {
  if (!endpoint || endpoint.length > 2048 || /[\u0000-\u001f\u007f]/.test(endpoint)) return null;
  try {
    const url = new URL(endpoint.trim());
    if (url.protocol !== "https:" || !url.hostname || url.username || url.password || url.hash) {
      return null;
    }
    return `https://${url.hostname.toLowerCase()}:${url.port || "443"}`;
  } catch {
    return null;
  }
}

export function tencentSecretConfigured(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) return false;
  if (trimmed.startsWith("<") && trimmed.endsWith(">")) return false;
  return !trimmed.startsWith("FAKESECRET_");
}
