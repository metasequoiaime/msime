import { errorCode } from "../core/error-code";

/** `setupCommand` 是本版本的首次配置命令（`linuxEditionNames`），full 是 `msime-linux-setup`。 */
export function linuxSetupFailureMessage(
  error: unknown,
  setupCommand = "msime-linux-setup",
): string {
  const code = errorCode(error);
  if (code === "setup_unavailable")
    return `找不到 ${setupCommand}，请确认安装完整，或在终端运行 ${setupCommand}。`;
  if (code === "setup_directory_exists") return "配置目录已存在但不完整。请先移走它，再重新配置。";
  if (code === "setup_running") return "配置已在进行中。";
  if (code === "setup_timeout") return "配置超时，请检查网络后重试。";
  return "配置未完成，请展开下面的「显示详细输出」查看原因。";
}
