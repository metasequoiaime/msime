import { errorCode } from "../core/error-code";
import { accountMessage } from "../account/account-errors";

/** 只按稳定错误码显示原因，避免把原始负载中的账号或词条写进提示。 */
export function cloudDictionaryErrorMessage(error: unknown, fallback: string): string {
  const code = errorCode(error);
  if (code === "unavailable") return fallback;
  if (code === "account_conflict" || code === "conflict")
    return "云词库已被其他设备更新，请刷新后重试。";
  if (code === "unauthorized") return "登录已失效，请重新登录。";
  if (code === "timeout" || code === "account_timeout") return "云词库请求超时，请稍后重试。";
  if (code?.startsWith("account_")) return accountMessage(error);
  return fallback === "无法访问云词库服务，请确认 provider 已连接"
    ? "云词库请求失败，请稍后重试。"
    : fallback;
}
