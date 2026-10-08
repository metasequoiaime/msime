// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { SettingsPage, type Snapshot, type TouchKeyboardSkinDesign } from "@msime/ui";
import { SkinGrid } from "../../../../packages/ui/src/settings/skin-grid";
import type { ExternalSkin } from "../../../../packages/ui/src/skin/external-skins";
import { themeCatalog } from "../../../../packages/ui/src/theme/global-theme";
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const design: TouchKeyboardSkinDesign = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0,
  monospaced: false,
};

const sample: ExternalSkin = {
  id: "sample",
  name: "Sample skin",
  version: "1",
  base: "paper",
  author: null,
  description: null,
  layouts: ["horizontal"],
  themes: ["dark", "light"],
  minWidthDip: 0,
  decorationTopDip: 0,
  decorationWidthDip: 0,
  toolbarStylesheet: null,
  preview: null,
  candidate: { dark: { surface: "#123456" }, light: { surface: "#abcdef" } },
};

const builtinTitles = themeCatalog.filter((entry) => entry.id !== "custom").map((e) => e.title);

function grid(props: Partial<ComponentProps<typeof SkinGrid>> = {}) {
  const onApply = vi.fn();
  const onSelectDesign = vi.fn();
  render(
    <SkinGrid
      globalTheme="system"
      customTheme={undefined}
      packages={[sample]}
      customDesign={{ design, selected: false, onSelect: onSelectDesign }}
      keyboardTheme="light"
      onApply={onApply}
      {...props}
    />,
  );
  return { onApply, onSelectDesign };
}

function cardNames(): string[] {
  const section = screen.getByRole("region", { name: "皮肤" });
  return (
    within(section)
      .getAllByRole("button")
      // 标题跟在缩略图后面，缩略图上的按键标签是绘制的文字，对辅助技术隐藏。
      .map((button) => button.getAttribute("aria-label") ?? button.lastElementChild!.textContent!)
  );
}

test("lists the built-in themes, then 我的皮肤, then the packages, with the AI tile last", () => {
  grid({ onOpenAi: vi.fn() });
  expect(cardNames()).toEqual([
    ...builtinTitles,
    "我的皮肤",
    "Sample skin",
    "AI 设计皮肤，描述一句话生成",
  ]);
  // 每张卡片都是设计里 390 × 292 画布上的键盘缩略图。
  const thumbnails = document.querySelectorAll("[data-skin-frame] svg[viewBox='0 0 390 292']");
  expect(thumbnails).toHaveLength(builtinTitles.length + 2);
});

// 每次应用皮肤的点按都会为「换装达人」徽章上报，id 与 Android 皮肤页记录的一致；已在使用的卡片不上报。
test("reports the applied skin's id for the badge", () => {
  const onApplied = vi.fn();
  const { onApply, onSelectDesign } = grid({ onApplied });
  fireEvent.click(screen.getByRole("button", { name: "纸白" }));
  fireEvent.click(screen.getByRole("button", { name: "我的皮肤" }));
  fireEvent.click(screen.getByRole("button", { name: "Sample skin" }));
  expect(onApply).toHaveBeenCalledTimes(2);
  expect(onSelectDesign).toHaveBeenCalledTimes(1);
  expect(onApplied.mock.calls).toEqual([["paper"], ["custom"], ["sample"]]);
  cleanup();

  const inUse = vi.fn();
  grid({ globalTheme: "night", onApplied: inUse });
  fireEvent.click(screen.getByRole("button", { name: "夜青" }));
  expect(inUse).not.toHaveBeenCalled();
});

test("keeps the catalog's 自定义 card on a host without the keyboard skin editor", () => {
  grid({ customDesign: undefined });
  expect(cardNames()).toEqual(themeCatalog.map((entry) => entry.title).concat("Sample skin"));
});

test("rings and checks only the theme in use", () => {
  grid({ globalTheme: "night" });
  const selected = screen.getByRole("button", { name: "夜青" });
  expect(selected.getAttribute("aria-pressed")).toBe("true");
  expect(selected.querySelector("[data-skin-check]")).not.toBeNull();
  expect(selected.querySelector("[data-skin-frame]")!.className).toContain(
    "0_0_0_4px_var(--accent-color)",
  );
  const other = screen.getByRole("button", { name: "纸白" });
  expect(other.getAttribute("aria-pressed")).toBe("false");
  expect(other.querySelector("[data-skin-check]")).toBeNull();
  expect(other.querySelector("[data-skin-frame]")!.className).toContain("0_0_0_1px_var(--p-hair)");
  expect(document.querySelectorAll("[data-skin-check]")).toHaveLength(1);
});

test("a package in use wins over the custom design it is drawn with", () => {
  grid({
    globalTheme: "custom",
    customTheme: { base: "paper", candidate_skin: "sample", keyboard: design },
    customDesign: { design, selected: true, onSelect: vi.fn() },
  });
  expect(screen.getByRole("button", { name: "Sample skin" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  expect(screen.getByRole("button", { name: "我的皮肤" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
});

test("tapping a card applies it at once", () => {
  const { onApply, onSelectDesign } = grid({
    customTheme: { base: "light", candidate_colors: { text: "#112233" } },
  });
  fireEvent.click(screen.getByRole("button", { name: "夜青" }));
  expect(onApply).toHaveBeenLastCalledWith({ global_theme: "night" });
  fireEvent.click(screen.getByRole("button", { name: "Sample skin" }));
  expect(onApply).toHaveBeenLastCalledWith({
    global_theme: "custom",
    custom_theme: {
      base: "paper",
      candidate_colors: { text: "#112233" },
      candidate_skin: "sample",
    },
  });
  fireEvent.click(screen.getByRole("button", { name: "我的皮肤" }));
  expect(onSelectDesign).toHaveBeenCalledOnce();
  // 已在使用的卡片保持不动。
  onApply.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "跟随系统" }));
  expect(onApply).not.toHaveBeenCalled();
});

test("draws the AI tile only when the flow can be opened", () => {
  grid();
  expect(screen.queryByRole("button", { name: /AI 设计皮肤/ })).toBeNull();
  cleanup();
  const onOpenAi = vi.fn();
  grid({ onOpenAi });
  fireEvent.click(screen.getByRole("button", { name: /AI 设计皮肤/ }));
  expect(onOpenAi).toHaveBeenCalledOnce();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    candidate_layout: "horizontal",
    learning: true,
    chinese_punctuation: true,
  },
};

function skinClients() {
  return {
    customTouchKeyboardSkins: true,
    customSkinLibrary: { load: vi.fn().mockResolvedValue([]), mutate: vi.fn() },
    aiSkins: { generate: vi.fn(), cancel: vi.fn().mockResolvedValue(undefined) },
  };
}

test("the HarmonyOS phone's 皮肤 page is the grid, with its settings under 更多选项", async () => {
  const save = vi.fn().mockImplementation(async (_revision: number, preferences) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: testHost({ platform: "harmony" }),
        ...skinClients(),
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "皮肤" });
  expect(within(page).queryByRole("region", { name: "主题列表" })).toBeNull();
  const more = within(page).getByText("更多选项").closest("details")!;
  expect(within(more).getByRole("radiogroup", { name: "颜色模式" })).toBeTruthy();
  expect(within(more).getByText("自定义主题")).toBeTruthy();
  expect(within(more).getByText("高级")).toBeTruthy();
  fireEvent.click(within(page).getByRole("button", { name: "夜青" }));
  expect(within(page).getByRole("button", { name: "夜青" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(3, expect.objectContaining({ global_theme: "night" })),
  );
});

test("the AI tile opens AI 皮肤抽卡 as a full-screen view the back gesture closes", async () => {
  const previous = window.history.state;
  window.history.replaceState(null, "");
  try {
    render(
      <SettingsPage
        initialPage="skin"
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: testHost({ platform: "harmony" }),
          ...skinClients(),
        }}
      />,
    );
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: /AI 设计皮肤/ }));
    const view = screen.getByRole("dialog", { name: "AI 皮肤抽卡" });
    expect(window.history.state).toMatchObject({ page: "skin", skinSubpage: "ai" });
    // 该视图位于页面的 fieldset 之外，所以进行中的设置保存不会禁用它。
    expect(view.closest("fieldset")).toBeNull();
    expect(within(view).getByRole("button", { name: "返回" })).toBeTruthy();
    await act(async () => {
      window.history.back();
      await new Promise((resolve) => window.addEventListener("popstate", resolve, { once: true }));
    });
    expect(screen.queryByRole("dialog", { name: "AI 皮肤抽卡" })).toBeNull();
  } finally {
    window.history.replaceState(previous, "");
  }
});

test("desktop hosts and the HarmonyOS 2-in-1 keep the carousel", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: false }),
        ...skinClients(),
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "主题" });
  expect(within(page).getByRole("region", { name: "主题列表" })).toBeTruthy();
  expect(page.querySelector("[data-skin-grid]")).toBeNull();
});
