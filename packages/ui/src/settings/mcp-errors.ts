import { errorCode } from "../core/error-code";

export function mcpFailureMessage(error: unknown, name: string): string {
  switch (errorCode(error)) {
    case "mcp_client_missing":
      return `没有找到 ${name} 的配置目录。请先安装并打开一次 ${name}。`;
    case "mcp_config_invalid":
      return `${name} 的配置文件不是有效的 JSON，已保持原样。请先修正该文件。`;
    case "mcp_server_missing":
      return "没有找到 msime-mcp，请重新安装输入法。";
    case "mcp_options_missing":
      return "输入法尚未完成初始化，请先完成设置向导。";
  }
  return `无法写入 ${name} 的配置文件。`;
}
