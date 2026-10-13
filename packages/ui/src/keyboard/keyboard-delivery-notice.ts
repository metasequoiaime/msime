export const MACOS_KEYBOARD_GUIDANCE = "先点击目标应用的输入框，再点击这里的按键";

/** 只按宿主的受控错误码解释失败，不展示原始错误或输入内容。 */
export function keyboardDeliveryNotice(error: unknown, platform?: string): string {
  const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
  if (platform === "macos") {
    if (code === "permission_denied")
      return "无法发送按键：请在系统设置 → 隐私与安全性 → 辅助功能中允许水杉输入法，然后重新打开屏幕键盘";
    if (code === "no_input_target")
      return "尚未选择输入位置：请先点击其他应用的输入框，再点击这里的按键";
    if (code === "input_target_changed")
      return "输入位置已变化，本次按键未发送：请点击目标输入框后重新输入";
  }
  return "按键发送失败，后续排队按键已取消，请确认输入位置后继续";
}
