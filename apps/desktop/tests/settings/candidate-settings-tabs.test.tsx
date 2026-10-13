// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    candidate_font_size: 18,
    learning: true,
    chinese_punctuation: true,
  },
};

async function renderAppearance() {
  const save = vi.fn().mockImplementation(async (revision, preferences) => ({
    ...initial,
    revision: revision + 1,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save,
        host: testHost({ platform: "macos", candidate_font_controls: true }),
      }}
    />,
  );
  await settingsFormReady();
  return save;
}

test("预览下方四个标签只显示当前设置组，切换保留修改和共用预览", async () => {
  const save = await renderAppearance();
  const tabs = screen.getByRole("tablist", { name: "候选窗口设置分类" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["布局", "字体与大小", "窗口样式", "预编辑"]);
  expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
  expect(screen.getByRole("tabpanel", { name: "布局" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "字号" })).toBeNull();
  const preview = screen.getByRole("region", { name: "候选窗口预览" });
  expect(preview.compareDocumentPosition(tabs) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(preview.querySelector(".candidate-brand")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示水杉 logo" }));
  expect(preview.querySelector(".candidate-brand")).not.toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示水杉 logo" }));
  expect(preview.querySelector(".candidate-brand")).toBeNull();
  expect(preview.querySelector("[data-preview-reserve]")).toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "横向" }));
  expect(preview.querySelector('[data-preview-layout="horizontal"]')).not.toBeNull();
  expect(preview.querySelector("[data-preview-reserve]")).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  fireEvent.change(screen.getByRole("combobox", { name: "字号" }), { target: { value: "24" } });
  expect(preview.querySelector('[data-font-size="24"]')).not.toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "窗口样式" }));
  expect(screen.getByRole("button", { name: "皮肤、颜色与明暗" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "预编辑" }));
  fireEvent.change(screen.getByRole("combobox", { name: "候选窗口预编辑" }), {
    target: { value: "empty" },
  });
  expect(screen.getByRole("region", { name: "候选窗口预览" })).toBe(preview);
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  expect(screen.getByRole("combobox", { name: "字号" })).toHaveProperty("value", "24");
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  expect(screen.getByRole("tab", { name: "字体与大小" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(
    initial.revision + save.mock.calls.length - 1,
    expect.objectContaining({
      candidate_font_size: 24,
      candidate_preedit_style: "empty",
    }),
  );
});

test("标签支持方向键循环和 Home、End，焦点与面板关联同步", async () => {
  await renderAppearance();
  const layout = screen.getByRole("tab", { name: "布局" });
  layout.focus();
  fireEvent.keyDown(layout, { key: "ArrowLeft" });
  const preedit = screen.getByRole("tab", { name: "预编辑" });
  expect(document.activeElement).toBe(preedit);
  const panel = screen.getByRole("tabpanel", { name: "预编辑" });
  expect(preedit.getAttribute("aria-controls")).toBe(panel.id);
  expect(panel.getAttribute("aria-labelledby")).toBe(preedit.id);
  fireEvent.keyDown(preedit, { key: "ArrowRight" });
  expect(document.activeElement).toBe(layout);
  fireEvent.keyDown(layout, { key: "End" });
  expect(document.activeElement).toBe(preedit);
  fireEvent.keyDown(preedit, { key: "Home" });
  fireEvent.keyDown(layout, { key: "ArrowRight" });
  expect(document.activeElement).toBe(screen.getByRole("tab", { name: "字体与大小" }));
  expect(screen.getByRole("tabpanel", { name: "字体与大小" })).toBeTruthy();
});

test("手机候选栏保持现有设置布局", async () => {
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "ios", candidate_font_controls: true }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("tablist", { name: "候选窗口设置分类" })).toBeNull();
  expect(screen.getByRole("combobox", { name: "字号" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选栏预编辑" })).toBeTruthy();
});
