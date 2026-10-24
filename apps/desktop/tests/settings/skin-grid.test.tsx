// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { SettingsPage, type Snapshot, type TouchKeyboardSkinDesign } from "@msime/ui";
import { SkinGrid } from "../../../../packages/ui/src/settings/skin-grid";
import type { ExternalSkin } from "../../../../packages/ui/src/skin/external-skins";
import { offeredThemeCatalog } from "../../../../packages/ui/src/theme/global-theme";
import { recordSkinSafely } from "../../../../packages/ui/src/settings/typing-statistics-model";
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

// 网格只画在鸿蒙手机上，所以按鸿蒙提供的主题排。
const themeCatalog = offeredThemeCatalog("harmony");
// 固定明暗的主题只在那种明暗下列出；测试默认在浅色下渲染。
const fitsMode = (appearance: string | null, dark: boolean) =>
  appearance === null || (appearance === "dark") === dark;
const builtinTitles = themeCatalog
  .filter((entry) => entry.id !== "custom" && fitsMode(entry.appearance, false))
  .map((e) => e.title);

// 和皮肤页一样把「内置」和「我的」两个分类的网格画在一起，两者共用同一组卡片数据。
function grid(props: Partial<Omit<ComponentProps<typeof SkinGrid>, "section">> = {}) {
  const onApply = vi.fn();
  const onSelectDesign = vi.fn();
  const shared = {
    themes: themeCatalog,
    globalTheme: "system",
    customTheme: undefined,
    packages: [sample],
    customDesign: { design, selected: false, onSelect: onSelectDesign },
    keyboardTheme: "light",
    onApply,
    ...props,
  } satisfies Omit<ComponentProps<typeof SkinGrid>, "section">;
  render(
    <>
      <SkinGrid section="builtin" {...shared} />
      <SkinGrid section="mine" {...shared} />
    </>,
  );
  return { onApply, onSelectDesign };
}

function cardNames(section: "builtin" | "mine"): string[] {
  const container = document.querySelector<HTMLElement>(`[data-skin-grid="${section}"]`)!;
  return (
    within(container)
      .queryAllByRole("button")
      // 标题跟在缩略图后面，缩略图上的按键标签是绘制的文字，对辅助技术隐藏。
      .map((button) => button.getAttribute("aria-label") ?? button.lastElementChild!.textContent!)
  );
}

// 与 Android 皮肤页一致：「内置」是目录里的主题和皮肤包，「我的」最前面是 AI 设计入口，然后是我的皮肤。
test("内置 lists the built-in themes then the packages, and 我的 starts with the AI tile before 我的皮肤", () => {
  grid({ onOpenAi: vi.fn() });
  expect(cardNames("builtin")).toEqual([...builtinTitles, "Sample skin"]);
  expect(cardNames("mine")).toEqual(["AI 设计皮肤，描述一句话生成", "我的皮肤"]);
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
  grid({ globalTheme: "night", keyboardTheme: "dark", onApplied: inUse });
  fireEvent.click(screen.getByRole("button", { name: "夜青" }));
  expect(inUse).not.toHaveBeenCalled();
});

test("keeps the catalog's 自定义 card under 内置 on a host without the keyboard skin editor", () => {
  grid({ customDesign: undefined });
  expect(cardNames("builtin")).toEqual(
    themeCatalog
      .filter((entry) => fitsMode(entry.appearance, false))
      .map((entry) => entry.title)
      .concat("Sample skin"),
  );
  expect(cardNames("mine")).toEqual([]);
});

test("rings and checks only the theme in use", () => {
  grid({ globalTheme: "night", keyboardTheme: "dark" });
  const selected = screen.getByRole("button", { name: "夜青" });
  expect(selected.getAttribute("aria-pressed")).toBe("true");
  const check = selected.querySelector("[data-skin-check] svg");
  expect(check).not.toBeNull();
  expect(check?.getAttribute("width")).toBe("14");
  expect(check?.getAttribute("stroke-width")).toBe("2.6");
  expect(check?.getAttribute("aria-hidden")).toBe("true");
  expect(check?.getAttribute("focusable")).toBe("false");
  expect(selected.querySelector("[data-skin-frame]")!.className).toContain(
    "0_0_0_4px_var(--accent-color)",
  );
  const other = screen.getByRole("button", { name: "墨" });
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

// 浅色、深色两个槽位里的皮肤包都算在用，两种明暗下都列出、都打勾；再点一次只把它从自己的槽位取下。
test("skins in the light and the dark slot are both in use, and tapping one again takes it out", () => {
  const night: ExternalSkin = { ...sample, id: "midnight", name: "Midnight skin", base: "night" };
  const customTheme = {
    base: "night",
    candidate_skin: "sample",
    candidate_skin_dark: "midnight",
  } as const;
  for (const keyboardTheme of ["light", "dark"] as const) {
    grid({ globalTheme: "custom", customTheme, packages: [sample, night], keyboardTheme });
    for (const name of ["Sample skin", "Midnight skin"])
      expect(screen.getByRole("button", { name }).getAttribute("aria-pressed")).toBe("true");
    expect(document.querySelectorAll("[data-skin-check]")).toHaveLength(2);
    cleanup();
  }

  const onApplied = vi.fn();
  const { onApply } = grid({
    globalTheme: "custom",
    customTheme,
    packages: [sample, night],
    keyboardTheme: "dark",
    onApplied,
  });
  fireEvent.click(screen.getByRole("button", { name: "Midnight skin" }));
  expect(onApply).toHaveBeenLastCalledWith({
    global_theme: "custom",
    custom_theme: { base: "night", candidate_skin: "sample", candidate_skin_dark: null },
  });
  expect(onApplied).not.toHaveBeenCalled();
});

// 浅色模式下不列深色主题；已选的深色主题在浅色下按「跟随系统」画，选中的也就是「跟随系统」。深色模式反过来。
test("each mode offers only its own themes and shows a theme of the other mode as 跟随系统", () => {
  grid({ globalTheme: "night" });
  for (const name of ["水杉", "夜青", "墨"])
    expect(screen.queryByRole("button", { name })).toBeNull();
  expect(screen.getByRole("button", { name: "跟随系统" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  cleanup();

  grid({ globalTheme: "paper", keyboardTheme: "dark" });
  for (const name of ["浅色", "纸白"]) expect(screen.queryByRole("button", { name })).toBeNull();
  // 皮肤包不按明暗过滤：base 是浅色主题的包在深色下照样列出。
  expect(screen.getByRole("button", { name: "Sample skin" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "跟随系统" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
});

test("a dark skin goes into the dark slot and keeps the light one", () => {
  const night: ExternalSkin = { ...sample, id: "midnight", name: "Midnight skin", base: "night" };
  const { onApply } = grid({
    globalTheme: "custom",
    customTheme: { base: "paper", candidate_skin: "sample" },
    packages: [sample, night],
    keyboardTheme: "dark",
  });
  fireEvent.click(screen.getByRole("button", { name: "Midnight skin" }));
  expect(onApply).toHaveBeenLastCalledWith({
    global_theme: "custom",
    custom_theme: { base: "night", candidate_skin: "sample", candidate_skin_dark: "midnight" },
  });
  // 自定义卡片去掉两个槽位的皮肤。
  cleanup();
  const custom = grid({
    globalTheme: "custom",
    customTheme: { base: "night", candidate_skin: "sample", candidate_skin_dark: "midnight" },
    packages: [sample, night],
    customDesign: undefined,
  });
  fireEvent.click(screen.getByRole("button", { name: "自定义" }));
  expect(custom.onApply).toHaveBeenLastCalledWith({
    global_theme: "custom",
    custom_theme: { base: "night", candidate_skin: null, candidate_skin_dark: null },
  });
});

test("tapping a card applies it at once", () => {
  const { onApply, onSelectDesign } = grid({
    customTheme: { base: "light", candidate_colors: { text: "#112233" } },
  });
  fireEvent.click(screen.getByRole("button", { name: "纸白" }));
  expect(onApply).toHaveBeenLastCalledWith({ global_theme: "paper" });
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

function communitySkins() {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    setCategory: vi.fn(),
    finishTrial: vi.fn(),
  };
}

const skinTab = (name: string) =>
  within(screen.getByRole("tablist", { name: "皮肤分类" })).getByRole("tab", { name });

test("skin usage statistics rejection is handled", async () => {
  const recordSkin = vi.fn().mockRejectedValue(new Error("synthetic statistics failure"));
  const onApplied = vi.fn((id: string) => recordSkinSafely({ recordSkin }, id));
  grid({ onApplied });
  expect(() => fireEvent.click(screen.getByRole("button", { name: "纸白" }))).not.toThrow();
  expect(onApplied).toHaveBeenCalledWith("paper");
  await waitFor(() => expect(recordSkin).toHaveBeenCalledWith("paper"));
});

test("the HarmonyOS phone's 皮肤 page is only the grid, without 更多选项", async () => {
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
  // 颜色模式、在线皮肤、自定义主题和各界面的明暗覆盖都不在手机皮肤页上。
  expect(within(page).queryByText("更多选项")).toBeNull();
  expect(within(page).queryByRole("radiogroup", { name: "颜色模式" })).toBeNull();
  expect(within(page).queryByText("自定义主题")).toBeNull();
  expect(within(page).queryByText("高级")).toBeNull();
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
    // AI 设计入口在「我的」分类里，与 Android 皮肤页一致。
    fireEvent.click(skinTab("我的"));
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

test("the AI view closes locally when its history entry is gone", async () => {
  const previous = window.history.state;
  window.history.replaceState(null, "");
  const back = vi.spyOn(window.history, "back").mockImplementation(() => undefined);
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
    fireEvent.click(skinTab("我的"));
    fireEvent.click(screen.getByRole("button", { name: /AI 设计皮肤/ }));
    const view = screen.getByRole("dialog", { name: "AI 皮肤抽卡" });
    window.history.replaceState({ page: "skin" }, "");

    fireEvent.click(within(view).getByRole("button", { name: "返回" }));

    expect(back).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "AI 皮肤抽卡" })).toBeNull();
  } finally {
    window.history.replaceState(previous, "");
  }
});

test("tapping the chosen 我的皮肤 card again opens the editor as a full-screen view", async () => {
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
    fireEvent.click(skinTab("我的"));
    const page = screen.getByRole("group", { name: "皮肤" });
    const card = () => within(page).getByRole("button", { name: /我的皮肤/ });
    // 第一次点选中它，再点一次打开编辑器。
    if (card().getAttribute("aria-pressed") !== "true") fireEvent.click(card());
    expect(card().getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(card());
    const view = screen.getByRole("dialog", { name: "设计我的皮肤" });
    expect(window.history.state).toMatchObject({ page: "skin", skinSubpage: "editor" });
    // 编辑器不在「皮肤」页里就地展开，按钮也不会变成「收起自定义编辑器」。
    expect(within(page).queryByLabelText("自定义皮肤编辑器")).toBeNull();
    expect(within(page).queryByRole("button", { name: "收起自定义编辑器" })).toBeNull();
    expect(within(view).getByLabelText("自定义皮肤编辑器")).toBeTruthy();
    // 返回按钮取代了「完成」。
    expect(within(view).queryByRole("button", { name: "完成" })).toBeNull();
    await act(async () => {
      fireEvent.click(within(view).getByRole("button", { name: "返回" }));
      await new Promise((resolve) => window.addEventListener("popstate", resolve, { once: true }));
    });
    expect(screen.queryByRole("dialog", { name: "设计我的皮肤" })).toBeNull();
  } finally {
    window.history.replaceState(previous, "");
  }
});

test("the HarmonyOS grid leaves out 原生 and marks 跟随系统 when the document says native", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue({
          ...initial,
          preferences: { ...initial.preferences, global_theme: "native" },
        }),
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        ...skinClients(),
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "皮肤" });
  expect(within(page).queryByRole("button", { name: "原生" })).toBeNull();
  expect(within(page).getByRole("button", { name: "跟随系统" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
});

test("only the iOS carousel offers 原生, right after 跟随系统", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "ios" }),
      }}
    />,
  );
  await settingsFormReady();
  const titles = within(screen.getByRole("region", { name: "主题列表" }))
    .getAllByRole("article")
    .map((article) => article.getAttribute("aria-label"));
  expect(titles.slice(0, 3)).toEqual(["跟随系统", "原生", "水杉"]);
  // 「原生」卡有自己的说明和系统键盘预览（系统蓝、扁平同色按键），和「跟随系统」卡分得开。
  const carouselRegion = screen.getByRole("region", { name: "主题列表" });
  const native = within(carouselRegion).getByRole("article", { name: "原生" });
  expect(within(native).getByText("iOS 自带键盘的样子，跟随系统明暗，不跟季节")).toBeTruthy();
  const nativePreview = native.querySelector("svg[data-preview-skin]");
  expect(nativePreview?.getAttribute("data-preview-skin")).toBe("native");
  const nativeTheme = nativePreview?.getAttribute("data-preview-theme");
  expect(nativePreview?.querySelector("rect")?.getAttribute("fill")).toBe(
    nativeTheme === "dark" ? "#222223" : "#E2E3E8",
  );
  const systemPreview = within(carouselRegion)
    .getByRole("article", { name: "跟随系统" })
    .querySelector("svg[data-preview-skin]");
  expect(systemPreview?.querySelector("rect")?.getAttribute("fill")).not.toBe(
    nativePreview?.querySelector("rect")?.getAttribute("fill"),
  );
  cleanup();

  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue({
          ...initial,
          preferences: { ...initial.preferences, global_theme: "native" },
        }),
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
      }}
    />,
  );
  await settingsFormReady();
  const carousel = screen.getByRole("region", { name: "主题列表" });
  expect(within(carousel).queryByRole("article", { name: "原生" })).toBeNull();
  const system = within(carousel).getByRole("article", { name: "跟随系统" });
  expect(within(system).getByRole("switch").getAttribute("aria-checked")).toBe("true");
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
  // 「内置 / 我的 / 社区」分类只在手机上；2-in-1 和桌面一样是轮播加设置分组。
  expect(screen.queryByRole("tablist", { name: "皮肤分类" })).toBeNull();
  cleanup();

  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
        communitySkins: communitySkins(),
      }}
    />,
  );
  await settingsFormReady();
  expect(
    within(screen.getByRole("group", { name: "主题" })).getByRole("region", { name: "主题列表" }),
  ).toBeTruthy();
  expect(screen.queryByRole("tablist", { name: "皮肤分类" })).toBeNull();
  expect(document.querySelector("[data-skin-grid]")).toBeNull();
});

// 与 Android 皮肤页一致，鸿蒙手机皮肤页顶部是「内置 / 我的 / 社区」：内置是目录里的主题，我的是 AI 设计入口和自己的皮肤，没有保存的设计时说明去社区获取。
test("the HarmonyOS phone's 皮肤 page splits into 内置, 我的 and 社区 tabs", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        ...skinClients(),
        communitySkins: communitySkins(),
      }}
    />,
  );
  await settingsFormReady();
  const tabs = within(screen.getByRole("tablist", { name: "皮肤分类" })).getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual(["内置", "我的", "社区"]);
  expect(skinTab("内置").getAttribute("aria-selected")).toBe("true");

  const page = screen.getByRole("group", { name: "皮肤" });
  const builtin = within(page).getByRole("tabpanel", { name: "内置" });
  expect(within(builtin).getByRole("button", { name: "跟随系统" })).toBeTruthy();
  expect(within(page).queryByRole("button", { name: /我的皮肤/ })).toBeNull();
  expect(within(page).queryByRole("button", { name: /AI 设计皮肤/ })).toBeNull();
  // 社区不再是网格里的第一页预览加「浏览更多皮肤」，而是单独的分类。
  expect(screen.queryByRole("button", { name: "浏览更多皮肤" })).toBeNull();

  fireEvent.click(skinTab("我的"));
  const mine = within(page).getByRole("tabpanel", { name: "我的" });
  expect(
    within(mine)
      .getAllByRole("button")
      .map((button) => button.getAttribute("aria-label") ?? button.lastElementChild!.textContent!),
  ).toEqual(["AI 设计皮肤，描述一句话生成", "我的皮肤"]);
  expect(within(mine).getByText("还没有保存的皮肤。可到社区获取喜欢的皮肤。")).toBeTruthy();
  expect(within(page).queryByRole("button", { name: "跟随系统" })).toBeNull();
});

test("the 我的 tab lists the saved designs and drops the empty note", async () => {
  const fox = { ...design, background: 0x1b2340 };
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        ...skinClients(),
        customSkinLibrary: {
          load: vi.fn().mockResolvedValue([{ id: "fox", name: "月光狐狸茶屋", design: fox }]),
          mutate: vi.fn(),
        },
      }}
    />,
  );
  await settingsFormReady();
  // 没有社区服务的宿主只有前两个分类。
  expect(
    within(screen.getByRole("tablist", { name: "皮肤分类" }))
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["内置", "我的"]);
  fireEvent.click(skinTab("我的"));
  const mine = screen.getByRole("tabpanel", { name: "我的" });
  expect(await within(mine).findByRole("button", { name: "月光狐狸茶屋" })).toBeTruthy();
  expect(within(mine).queryByText("还没有保存的皮肤。可到社区获取喜欢的皮肤。")).toBeNull();
});

// 皮肤库里的设计（在「社区」获取的皮肤也存在这里）在「我的」里排在我的皮肤之后，两种明暗下都列出；正在用的那款打勾，我的皮肤不再同时打勾。
test("saved designs follow 我的皮肤, the one in use is checked, and tapping one applies it", () => {
  const panda = { ...design, accent: 0x1f6b4a };
  const fox = { ...design, background: 0x1b2340 };
  const savedDesigns = [
    { id: "panda", name: "竹林熊猫茶会", design: panda },
    { id: "fox", name: "月光狐狸茶屋", design: fox },
  ];
  for (const keyboardTheme of ["light", "dark"] as const) {
    const onSelectSavedDesign = vi.fn();
    grid({
      globalTheme: "custom",
      customTheme: { base: "paper", keyboard: fox },
      customDesign: { design: fox, selected: true, onSelect: vi.fn() },
      savedDesigns,
      onSelectSavedDesign,
      keyboardTheme,
    });
    expect(cardNames("mine")).toEqual(["我的皮肤", "竹林熊猫茶会", "月光狐狸茶屋"]);
    expect(screen.getByRole("button", { name: "月光狐狸茶屋" }).getAttribute("aria-pressed")).toBe(
      "true",
    );
    for (const name of ["我的皮肤", "竹林熊猫茶会"])
      expect(screen.getByRole("button", { name }).getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(screen.getByRole("button", { name: "竹林熊猫茶会" }));
    expect(onSelectSavedDesign).toHaveBeenLastCalledWith(panda);
    cleanup();
  }
});
