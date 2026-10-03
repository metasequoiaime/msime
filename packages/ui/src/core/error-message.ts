import { errorCode } from "./error-code";

/** Message shown when the host cannot read the saved preferences document. */
export const unreadablePreferencesMessage = "配置文件无法读取或版本较新，原文件已保留。";

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  switch (errorCode(error)) {
    case "conflict":
      return "设置已在其他窗口修改，未能保存。请重试或重新读取。";
    case "invalid":
      return "候选数量必须为 1 到 9。";
    case "frequency_invalid":
      return "调频触发频次和步长必须为 1 到 10。";
    case "mixed_input_invalid":
      return "中英混输触发字符数必须为 1 到 8。";
    case "key_conflict":
      return "以词定字和翻页不能使用同一组快捷键。";
    case "format":
      return unreadablePreferencesMessage;
  }
  return "无法访问设置，请重试。原有设置不会被自动重置。";
}
