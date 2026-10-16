import { fireEvent, screen } from "@testing-library/react";

/** Resolves once the loaded settings form is on screen, the point from which sidebar clicks are no longer overtaken by the initial page selection. */
export async function settingsFormReady(): Promise<HTMLElement> {
  return screen.findByRole("form", { name: "设置" });
}

/** Saves the pending edits now instead of waiting out the autosave delay, the way leaving the window does. */
export function saveSettingsNow(): void {
  fireEvent.blur(window);
}

/** 桌面测试先进入对应功能分区，手机没有子标签时保持原位置。 */
export function selectSettingsTab(title: string): void {
  const tab = screen.queryByRole("tab", { name: title });
  if (tab) fireEvent.click(tab);
}

/** 桌面语音页把密钥、接口地址、润色、提示音细分和录音设备收在「服务配置」子视图里，测试先点进去。 */
export function openVoiceServiceConfig(): HTMLElement {
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));
  return screen.getByRole("region", { name: "服务配置" });
}
