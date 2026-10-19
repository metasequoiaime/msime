import { fireEvent, isInaccessible, screen, within } from "@testing-library/react";
import { expect } from "vitest";

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
  const currentTitle: Record<string, string> = {
    方案: "输入方案",
    中英文: "中英模式",
    选词与翻页: "选词与翻页",
    选词翻页: "选词与翻页",
    候选与联想: "候选与联想",
    候选联想: "候选与联想",
    输出: "输出设置",
    进阶: "高级选项",
    标点: "标点设置",
    查词库: "查找词库",
    云词库: "云端词库",
    版本与更新: "版本更新",
    帮助与反馈: "帮助与反馈",
    帮助反馈: "帮助与反馈",
    许可与隐私: "许可与隐私",
    许可隐私: "许可与隐私",
    维护与诊断: "维护诊断",
    "AI 助手": "AI助手",
  };
  const tab = screen.queryByRole("tab", { name: currentTitle[title] ?? title });
  if (tab) fireEvent.click(tab);
}

/** 桌面词库页分「查找词库」「管理词库」两个顶部标签（#8557）：查询和编辑词条在「查找词库」，导入导出、个人词库文件、云词库和清除学习数据在「管理词库」，用户要先点标签才看得到。和 `selectSettingsTab` 不同，这里找不到标签就直接失败；手机词库页直列，没有这组标签，不要对手机宿主调用。 */
export function selectDictionaryTab(title: "查词库" | "查找词库" | "管理词库"): void {
  const tabs = screen.getByRole("tablist", { name: "词库设置分类" });
  fireEvent.click(within(tabs).getByRole("tab", { name: title === "查词库" ? "查找词库" : title }));
}

/** `findByText` 不排除隐藏元素，收起的标签页里的文字也找得到；这里找到后再确认它没有被 `hidden` 等隐藏的祖先藏住，断言的是用户在当前标签页上真的看得到这段文字。 */
export async function findVisibleText(text: string): Promise<HTMLElement> {
  const element = await screen.findByText(text);
  expect(isInaccessible(element), `「${text}」在页面上，但被隐藏的祖先藏住了`).toBe(false);
  return element;
}

/** 桌面语音页把密钥、接口地址和润色收在「服务配置」子视图里，测试先点进去。 */
export function openVoiceServiceConfig(): HTMLElement {
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));
  return screen.getByRole("region", { name: "服务配置" });
}
