// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type SkinCatalog, type Snapshot } from "@msime/ui";

// 主题页皮肤卡片上的「删除」：页面只画按钮和确认框，能不能删由宿主（client-services 的 `delete_unselected`）回答。

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
beforeEach(() =>
  Object.defineProperty(document, "adoptedStyleSheets", {
    configurable: true,
    writable: true,
    value: [],
  }),
);

function skin(id: string, name: string): SkinCatalog["packages"][number] {
  return {
    id,
    name,
    version: "1",
    base: "system",
    author: "Example",
    description: `${name}的说明`,
    layouts: ["horizontal"],
    themes: ["dark", "light"],
    minWidthDip: 0,
    decorationTopDip: 0,
    decorationWidthDip: 0,
    toolbarStylesheet: null,
    preview: null,
    candidate: { dark: { surface: "#123456" }, light: { surface: "#abcdef" } },
  };
}

const both: SkinCatalog = {
  directory: "/synthetic/state/skins",
  issues: [],
  packages: [skin("sakura", "樱花"), skin("dusk", "黄昏")],
};
const onlyDusk: SkinCatalog = { ...both, packages: [both.packages[1]] };

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

function openSkinPage(client: Partial<SettingsClient>) {
  return render(
    <SettingsPage
      initialPage="skin"
      client={{ load: async () => initial, save: vi.fn(), ...client }}
    />,
  );
}

function card(name: string): HTMLElement {
  return screen.getByRole("article", { name });
}

test("没有删除能力的宿主不画「删除」", async () => {
  openSkinPage({ scanSkinCatalog: vi.fn().mockResolvedValue(both) });
  await screen.findByRole("article", { name: "樱花" });
  expect(within(card("樱花")).queryByRole("button", { name: "删除" })).toBeNull();
});

test("确认后按 id 删除，重扫目录，卡片随之消失并说明结果", async () => {
  const scanSkinCatalog = vi.fn().mockResolvedValueOnce(both).mockResolvedValue(onlyDusk);
  const deleteSkin = vi.fn().mockResolvedValue({ ...onlyDusk, synced: true });
  openSkinPage({ scanSkinCatalog, deleteSkin });
  await screen.findByRole("article", { name: "樱花" });

  fireEvent.click(within(card("樱花")).getByRole("button", { name: "删除" }));
  const dialog = await screen.findByRole("alertdialog");
  // 确认框说清楚云端副本怎么办：私人副本下次同步时一起删掉，公开发布的保留。
  expect(dialog.textContent).toContain("樱花");
  expect(dialog.textContent).toContain("下次同步");
  expect(dialog.textContent).toContain("公开发布");
  expect(deleteSkin).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));

  await waitFor(() => expect(deleteSkin).toHaveBeenCalledExactlyOnceWith("sakura"));
  await waitFor(() => expect(screen.queryByRole("article", { name: "樱花" })).toBeNull());
  expect(screen.getByRole("article", { name: "黄昏" })).not.toBeNull();
  expect((await screen.findByText(/已删除「樱花」/)).textContent).toContain("下次同步");
});

test("取消确认框什么都不删", async () => {
  const deleteSkin = vi.fn();
  openSkinPage({ scanSkinCatalog: vi.fn().mockResolvedValue(both), deleteSkin });
  await screen.findByRole("article", { name: "樱花" });
  fireEvent.click(within(card("樱花")).getByRole("button", { name: "删除" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "取消" }),
  );
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(deleteSkin).not.toHaveBeenCalled();
  expect(card("樱花")).not.toBeNull();
});

test("宿主答 candidate_skin_in_use 时提示先关掉这款皮肤，卡片保留", async () => {
  const deleteSkin = vi.fn().mockRejectedValue({ code: "candidate_skin_in_use" });
  openSkinPage({ scanSkinCatalog: vi.fn().mockResolvedValue(both), deleteSkin });
  await screen.findByRole("article", { name: "樱花" });
  fireEvent.click(within(card("樱花")).getByRole("button", { name: "删除" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "删除" }),
  );
  const notice = await screen.findByRole("alert");
  expect(notice.textContent).toContain("樱花");
  expect(notice.textContent).toContain("关掉");
  // 卡片没显示「使用中」（全局主题不是自定义）：打开再关掉开关会换成自定义主题，提示要说删完选回原来的主题。
  expect(notice.textContent).toContain("选回原来的主题");
  expect(card("樱花")).not.toBeNull();
});

test("卡片显示「使用中」时只提示关掉开关，不说选回主题", async () => {
  const deleteSkin = vi.fn().mockRejectedValue({ code: "candidate_skin_in_use" });
  const inUse: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "custom",
      custom_theme: { base: "system", candidate_skin: "sakura", candidate_skin_dark: "sakura" },
    },
  };
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: async () => inUse,
        save: vi.fn(),
        scanSkinCatalog: vi.fn().mockResolvedValue(both),
        deleteSkin,
      }}
    />,
  );
  await screen.findByRole("article", { name: "樱花" });
  await waitFor(() =>
    expect(within(card("樱花")).getByRole("switch").getAttribute("aria-checked")).toBe("true"),
  );
  fireEvent.click(within(card("樱花")).getByRole("button", { name: "删除" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "删除" }),
  );
  const notice = await screen.findByRole("alert");
  expect(notice.textContent).toContain("正在使用");
  expect(notice.textContent).not.toContain("选回原来的主题");
});
