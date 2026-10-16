// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { normalizeFontCatalog } from "../../../../packages/ui/src/candidate/font-catalog";
import { testHost } from "../support/host";
afterEach(cleanup);
const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
  },
};

test("font catalogs are bounded, validated and deduplicated", () => {
  expect(normalizeFontCatalog(["Beta", "Alpha", "Alpha"])).toEqual(["Alpha", "Beta"]);
  for (const value of [null, {}, [""], [12], ["字".repeat(43)], Array(16385).fill("Font")])
    expect(() => normalizeFontCatalog(value)).toThrow("invalid font catalog");
});
test("search loads once and selects with keyboard without saving", async () => {
  const listFontFamilies = vi.fn().mockResolvedValue(["Alpha", "Beta", "示例字体"]),
    save = vi.fn();
  render(
    <SettingsPage
      initialPage="appearance"
      client={{ load: async () => initial, save, listFontFamilies }}
    />,
  );
  // 字体框在「候选字体」选「自定义…」后才出现。
  fireEvent.click(await screen.findByRole("radio", { name: "自定义…" }));
  const primary = await screen.findByLabelText("主字体");
  expect(listFontFamilies).not.toHaveBeenCalled();
  fireEvent.focus(primary);
  await screen.findByRole("option", { name: "Alpha" });
  fireEvent.change(primary, { target: { value: "bet" } });
  expect(
    within(screen.getByRole("listbox", { name: "主字体可用字体" })).getAllByRole("option"),
  ).toHaveLength(1);
  fireEvent.keyDown(primary, { key: "ArrowDown" });
  fireEvent.keyDown(primary, { key: "Enter" });
  expect((primary as HTMLInputElement).value).toBe("Beta");
  expect(save).not.toHaveBeenCalled();
  expect(primary.getAttribute("aria-expanded")).toBe("false");
  fireEvent.focus(primary);
  await screen.findByRole("option", { name: "Beta" });
  expect(listFontFamilies).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(primary, { key: "Escape" });
  expect(primary.getAttribute("aria-expanded")).toBe("false");
});
test("late results from a replaced reader cannot reappear", async () => {
  let resolve!: (fonts: string[]) => void;
  const old = () =>
    new Promise<string[]>((done) => {
      resolve = done;
    });
  const client = { load: async () => initial, save: vi.fn() };
  const view = render(
    <SettingsPage initialPage="appearance" client={{ ...client, listFontFamilies: old }} />,
  );
  // 字体框在「候选字体」选「自定义…」后才出现。
  fireEvent.click(await screen.findByRole("radio", { name: "自定义…" }));
  fireEvent.focus(await screen.findByLabelText("主字体"));
  expect(screen.getByText("正在读取字体列表。")).toBeDefined();
  const next = vi.fn().mockResolvedValue(["New font"]);
  view.rerender(
    <SettingsPage initialPage="appearance" client={{ ...client, listFontFamilies: next }} />,
  );
  await screen.findByRole("option", { name: "New font" });
  await act(async () => resolve(["Stale font"]));
  expect(screen.queryByRole("option", { name: "Stale font" })).toBeNull();
});
test("failed catalog retries while allowing manual font entry", async () => {
  const listFontFamilies = vi
    .fn()
    .mockRejectedValueOnce(Error("synthetic failure"))
    .mockResolvedValue([]);
  render(
    <SettingsPage
      initialPage="appearance"
      client={{ load: async () => initial, save: vi.fn(), listFontFamilies }}
    />,
  );
  // 字体框在「候选字体」选「自定义…」后才出现。
  fireEvent.click(await screen.findByRole("radio", { name: "自定义…" }));
  const input = await screen.findByLabelText("主字体");
  fireEvent.focus(input);
  await screen.findByText(/读取字体列表失败/);
  fireEvent.change(input, { target: { value: "手动字体" } });
  expect((input as HTMLInputElement).value).toBe("手动字体");
  fireEvent.click(screen.getByRole("button", { name: "刷新字体列表" }));
  await waitFor(() => expect(listFontFamilies).toHaveBeenCalledTimes(2));
  await screen.findByText("系统字体列表为空，可手动输入。");
});

test("disables the font catalog refresh while loading", async () => {
  let finish!: (fonts: string[]) => void;
  const listFontFamilies = vi.fn(() => new Promise<string[]>((resolve) => (finish = resolve)));
  render(
    <SettingsPage
      initialPage="appearance"
      client={{ load: async () => initial, save: vi.fn(), listFontFamilies }}
    />,
  );
  // 字体框在「候选字体」选「自定义…」后才出现。
  fireEvent.click(await screen.findByRole("radio", { name: "自定义…" }));
  const input = await screen.findByLabelText("主字体");
  fireEvent.focus(input);
  await screen.findByText("正在读取字体列表。");
  expect((screen.getByRole("button", { name: "刷新字体列表" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  await act(async () => finish(["示例字体"]));
});

test("large catalogs bound visible options without losing searchable entries", async () => {
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        listFontFamilies: async () => Array.from({ length: 101 }, (_, i) => `Font${i}`),
      }}
    />,
  );
  // 字体框在「候选字体」选「自定义…」后才出现。
  fireEvent.click(await screen.findByRole("radio", { name: "自定义…" }));
  const input = await screen.findByLabelText("主字体");
  fireEvent.focus(input);
  await screen.findByText("显示前 100 项，请输入名称缩小范围。");
  const list = screen.getByRole("listbox", { name: "主字体可用字体" });
  expect(within(list).getAllByRole("option")).toHaveLength(100);
  fireEvent.change(input, { target: { value: "Font100" } });
  expect(within(list).getAllByRole("option")).toHaveLength(1);
});

// 主字体框收起时（选着预设），「更多选项」里的英文字体框自己带着读取状态和刷新按钮，读取失败后仍能重试。
test("the English font field carries the catalog status and refresh while the main field is hidden", async () => {
  const listFontFamilies = vi
    .fn()
    .mockRejectedValueOnce(Error("synthetic failure"))
    .mockResolvedValue(["Alpha"]);
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        listFontFamilies,
        host: testHost({ platform: "macos", candidate_font_controls: true }),
      }}
    />,
  );
  const english = await screen.findByLabelText("英文字体");
  expect(screen.queryByLabelText("主字体")).toBeNull();
  // 读取状态和刷新不再单独占一行。
  expect(screen.queryByText("系统字体列表")).toBeNull();
  fireEvent.focus(english);
  await screen.findByText(/读取字体列表失败/);
  fireEvent.click(screen.getByRole("button", { name: "刷新字体列表" }));
  await waitFor(() => expect(listFontFamilies).toHaveBeenCalledTimes(2));
  fireEvent.focus(english);
  await screen.findByRole("option", { name: "Alpha" });
});
