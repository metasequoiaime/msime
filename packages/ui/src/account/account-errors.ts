import { errorCode } from "../core/error-code";

export function isAccountCancellation(error: unknown): boolean {
  if (errorCode(error) === "account_cancelled") return true;
  if (
    typeof DOMException !== "undefined" &&
    error instanceof DOMException &&
    error.name === "AbortError"
  )
    return true;
  if (!(error instanceof Error)) return false;
  const message = error.message.trim().toLowerCase();
  return (
    error.name === "AbortError" ||
    message === "cancelled" ||
    message === "the operation was aborted."
  );
}

export function accountMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "account_invalid":
      return "填写的内容无效，请检查后重试。";
    case "account_unauthorized":
      return "登录已失效，请重新登录。";
    case "account_conflict":
      return "云端设置已被其他设备更新，请刷新后重新确认。";
    case "account_rate_limited":
      return "操作过于频繁，请稍后再试。";
    case "account_storage":
      return "无法安全读取登录状态，请检查设备安全设置。";
    case "account_cancelled":
      return "操作已取消，请重试。";
  }
  return "账号服务暂不可用，请稍后再试。";
}
