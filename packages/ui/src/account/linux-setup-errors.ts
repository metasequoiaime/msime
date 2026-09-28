import { errorCode } from "../core/error-code";

export function linuxSetupFailureMessage(error: unknown): string {
  const code = errorCode(error);
  if (code === "setup_unavailable")
    return "找不到 msime-linux-setup，请确认安装完整，或在终端运行 msime-linux-setup。";
  if (code === "setup_directory_exists") return "配置目录已存在但不完整。请先移走它，再重新配置。";
  if (code === "setup_running") return "配置已在进行中。";
  if (code === "setup_timeout") return "配置超时，请检查网络后重试。";
  return "配置未完成，请查看上面的输出。";
}
