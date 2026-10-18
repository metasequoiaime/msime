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
  ).toEqual(["输入方案", "中英模式", "选词翻页", "候选联想", "输出设置", "高级选项"]);
  expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
  expect(screen.queryByRole("radiogroup", { name: "中文输出" })).toBeNull();
  fireEvent.click(within(tabs).getByRole("tab", { name: "输出设置" }));
  const output = screen.getByRole("radio", { name: "繁体" });
  fireEvent.click(output);
  fireEvent.click(within(tabs).getByRole("tab", { name: "输入方案" }));
  fireEvent.click(within(tabs).getByRole("tab", { name: "输出设置" }));
  expect(screen.getByRole("radio", { name: "繁体" })).toBe(output);
  expect(output).toHaveProperty("checked", true);
  fireEvent.click(screen.getByRole("button", { name: /^(标点与翻译|标点翻译)$/ }));
  fireEvent.click(screen.getByRole("button", { name: /^(输入|输入设置)$/ }));
  expect(screen.getByRole("tabpanel", { name: "输出设置" })).toBeTruthy();
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
  ).toEqual(["标点设置", "候选释义", "翻译服务"]);
  const first = within(tabs).getByRole("tab", { name: "标点设置" });
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
  // 「显示英文释义」和「候选词翻译」画的是同一行释义，合在「候选释义」一个标签里。
  const gloss = screen.getByRole("tabpanel", { name: "候选释义" });
  expect(within(gloss).getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
  expect(within(gloss).getByRole("switch", { name: /^候选词翻译/ })).toBeTruthy();
});

// 语音页不再分标签：主页面只留常用几项，其余收在「服务配置」子视图里。子视图来回切换时两边都保持挂载，润色里没保存的输入照旧留着。
test("语音页用服务配置子视图代替标签，来回切换保留未保存的输入框", async () => {
  await mount("voice");
  expect(screen.queryByRole("tablist", { name: "语音输入设置分类" })).toBeNull();
  expect(screen.queryByRole("region", { name: "文本润色" })).toBeNull();
  // 这个宿主列不出录音设备，子视图里也没有这一组。
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));
  expect(screen.queryByRole("region", { name: "录音设备" })).toBeNull();
  const polish = screen.getByRole("region", { name: "文本润色" });
  fireEvent.click(within(polish).getByRole("switch", { name: "启用文本润色" }));
  const provider = within(polish).getByLabelText("文本润色服务提供商");
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(screen.getByRole("region", { name: "录音行为" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "服务配置" }));
  expect(
    within(screen.getByRole("region", { name: "文本润色" })).getByLabelText("文本润色服务提供商"),
  ).toBe(provider);
});

test.each(["input", "expression", "voice"] as const)("手机 %s 页保留原布局", async (page) => {
  await mount(page, "ios");
  expect(screen.queryByRole("tablist", { name: /设置分类/ })).toBeNull();
  if (page === "input") expect(screen.getByRole("radiogroup", { name: "中文输出" })).toBeTruthy();
  if (page === "expression")
    expect(screen.getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
  if (page === "voice") expect(screen.getByRole("region", { name: "录音行为" })).toBeTruthy();
});
