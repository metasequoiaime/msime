// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: { enabled: true, asr_provider: "doubao", language: "zh-CN" },
  },
};

async function mount(initialPage: "input" | "expression" | "voice", platform = "macos") {
  const save = vi.fn().mockImplementation(async (revision, preferences) => ({
    ...initial,
    revision: revision + 1,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: async () => initial,
        save,
        host: testHost({ platform }),
        candidateEnglishGloss: true,
      }}
    />,
  );
  await settingsFormReady();
  return save;
}

test("输入分区只显示当前面板，切换和离页保留草稿与标签选择", async () => {
  const save = await mount("input");
  const tabs = screen.getByRole("tablist", { name: "输入设置分类" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["方案", "中英文", "选词与翻页", "候选与联想", "输出", "进阶"]);
  expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
  expect(screen.queryByRole("switch", { name: "繁体输出" })).toBeNull();
  fireEvent.click(within(tabs).getByRole("tab", { name: "输出" }));
  const output = screen.getByRole("switch", { name: "繁体输出" });
  fireEvent.click(output);
  fireEvent.click(within(tabs).getByRole("tab", { name: "方案" }));
  fireEvent.click(within(tabs).getByRole("tab", { name: "输出" }));
  expect(screen.getByRole("switch", { name: "繁体输出" })).toBe(output);
  expect(output).toHaveProperty("checked", true);
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.getByRole("tabpanel", { name: "输出" })).toBeTruthy();
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save.mock.calls.at(-1)?.[1]).toEqual(
    expect.objectContaining({ traditional_chinese_output: true }),
  );
});

test("标点与翻译按功能拆分，方向键与 Home、End 同步焦点和面板", async () => {
  await mount("expression");
  const tabs = screen.getByRole("tablist", { name: "标点与翻译设置分类" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["标点", "候选与释义", "候选翻译", "翻译服务"]);
  const first = within(tabs).getByRole("tab", { name: "标点" });
  first.focus();
  fireEvent.keyDown(first, { key: "ArrowLeft" });
  const last = within(tabs).getByRole("tab", { name: "翻译服务" });
  expect(document.activeElement).toBe(last);
  expect(last.getAttribute("aria-controls")).toBe(
    screen.getByRole("tabpanel", { name: "翻译服务" }).id,
  );
  fireEvent.keyDown(last, { key: "ArrowRight" });
  expect(document.activeElement).toBe(first);
  fireEvent.keyDown(first, { key: "End" });
  expect(document.activeElement).toBe(last);
  fireEvent.keyDown(last, { key: "Home" });
  fireEvent.keyDown(first, { key: "ArrowRight" });
  expect(screen.getByRole("tabpanel", { name: "候选与释义" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
});

test("语音能力过滤标签，润色切换保留未保存的输入框", async () => {
  await mount("voice");
  const tabs = screen.getByRole("tablist", { name: "语音输入设置分类" });
  expect(within(tabs).queryByRole("tab", { name: "录音设备" })).toBeNull();
  expect(screen.queryByRole("region", { name: "文本润色" })).toBeNull();
  fireEvent.click(within(tabs).getByRole("tab", { name: "润色" }));
  const polish = screen.getByRole("region", { name: "文本润色" });
  fireEvent.click(within(polish).getByRole("switch", { name: "启用文本润色" }));
  const provider = within(polish).getByLabelText("文本润色服务提供商");
  fireEvent.click(within(tabs).getByRole("tab", { name: "录音与结果" }));
  expect(screen.getByRole("region", { name: "录音行为" })).toBeTruthy();
  fireEvent.click(within(tabs).getByRole("tab", { name: "润色" }));
  expect(
    within(screen.getByRole("region", { name: "文本润色" })).getByLabelText("文本润色服务提供商"),
  ).toBe(provider);
});

test.each(["input", "expression", "voice"] as const)("手机 %s 页保留原布局", async (page) => {
  await mount(page, "ios");
  expect(screen.queryByRole("tablist", { name: /设置分类/ })).toBeNull();
  if (page === "input") expect(screen.getByRole("switch", { name: "繁体输出" })).toBeTruthy();
  if (page === "expression")
    expect(screen.getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
  if (page === "voice") expect(screen.getByRole("region", { name: "录音行为" })).toBeTruthy();
});
