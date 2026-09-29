import { errorCode } from "../core/error-code";

export function chatError(error: unknown): string {
  switch (errorCode(error)) {
    case "account_unauthorized":
      return "登录后即可与 AI 对话。";
    case "account_invalid":
      return "消息或模型无效，请检查后重试。";
    case "account_rate_limited":
      return "操作过于频繁，请稍后再试。";
    case "account_unavailable":
      return "聊天服务暂不可用，请稍后重试。";
  }
  return "连接失败，请检查网络后重试。";
}
