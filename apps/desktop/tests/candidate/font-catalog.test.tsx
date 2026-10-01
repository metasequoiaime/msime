// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { normalizeFontCatalog } from "../../../../packages/ui/src/candidate/font-catalog";
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
  const primary = await screen.findByLabelText("候选窗主字体");
  expect(listFontFamilies).not.toHaveBeenCalled();
  fireEvent.focus(primary);
  await screen.findByRole("option", { name: "Alpha" });
  fireEvent.change(primary, { target: { value: "bet" } });
  expect(
    within(screen.getByRole("listbox", { name: "候选窗主字体可用字体" })).getAllByRole("option"),
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
  fireEvent.focus(await screen.findByLabelText("候选窗主字体"));
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
  const input = await screen.findByLabelText("候选窗主字体");
  fireEvent.focus(input);
  await screen.findByText(/读取字体列表失败/);
  fireEvent.change(input, { target: { value: "手动字体" } });
  expect((input as HTMLInputElement).value).toBe("手动字体");
  fireEvent.click(screen.getByRole("button", { name: "刷新字体列表" }));
  await waitFor(() => expect(listFontFamilies).toHaveBeenCalledTimes(2));
  await screen.findByText("系统字体列表为空，可手动输入。");
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
  const input = await screen.findByLabelText("候选窗主字体");
  fireEvent.focus(input);
  await screen.findByText("显示前 100 项，请输入名称缩小范围。");
  const list = screen.getByRole("listbox", { name: "候选窗主字体可用字体" });
  expect(within(list).getAllByRole("option")).toHaveLength(100);
  fireEvent.change(input, { target: { value: "Font100" } });
  expect(within(list).getAllByRole("option")).toHaveLength(1);
});
