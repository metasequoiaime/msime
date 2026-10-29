// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import {
  canAddNineKeySidebarSymbol,
  nineKeySidebarAddableSymbols,
  nineKeySidebarDefaults,
  nineKeySidebarMaxSymbolCodePoints,
  nineKeySidebarMaxSymbols,
  nineKeySidebarMaxTextUtf16,
  nineKeySidebarStoredValue,
  nineKeySidebarSummary,
  nineKeySidebarSymbols,
  nineKeySidebarVisibleRows,
  numberPadSidebarSymbols,
  storedNineKeySidebar,
  validNineKeySidebar,
  validNineKeySidebarSymbol,
} from "../../../../packages/ui/src/keyboard/touch-key-layout-styles";
import { TouchKeyLayoutSettings } from "../../../../packages/ui/src/settings/touch-key-layout-settings";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import type { TouchKeyboardScheme } from "../../../../packages/ui/src/settings/touch-keyboard-scheme-helpers";
import {
  collectSettingsSearchTargets,
  revealSettingsSearchTarget,
} from "../../../../packages/ui/src/settings/settings-search-index";
// 设置页的出厂表和一屏项数与 HarmonyOS 键盘读同一组常量：键盘改了，这里跟着变红。
import { NineKeyLayout } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/input/NineKeyLayout";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

function snapshot(preferences: Record<string, unknown> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  } as Snapshot;
}

function renderSettings(preferences: Record<string, unknown> = {}) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save,
        host: testHost({
          platform: "harmony",
          mobile_settings: true,
          panel_windows: false,
          touch_toolbar_tools: true,
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  return { save };
}

async function openKeyLayout() {
  await settingsFormReady();
  const row = await waitFor(() => {
    const found = screen.queryByRole("button", { name: "按键布局" });
    if (!found) throw new Error("no 按键布局 row");
    return found;
  });
  fireEvent.click(row);
  return screen.getByRole("dialog", { name: "按键布局" });
}

/** 直接画子页，`onChange` 收到的就是写进偏好的那一项。 */
function renderPage(
  preferences: Partial<Preferences> = {},
  scheme: TouchKeyboardScheme = "nine_key",
) {
  const onChange = vi.fn();
  const view = render(
    <TouchKeyLayoutSettings
      preferences={preferences as Preferences}
      scheme={scheme}
      groups={{ twentySix: true, fourteen: true, nine: true }}
      open
      onChange={onChange}
    />,
  );
  return { onChange, rerender: view.rerender };
}

const chooseKind = (root: HTMLElement, label: string) =>
  fireEvent.click(
    within(within(root).getByRole("radiogroup", { name: "键盘类型" })).getByRole("radio", {
      name: label,
    }),
  );

/** 读屏念的描述：`aria-describedby` 指向的各段文字按次序拼起来。 */
const description = (node: HTMLElement | null) =>
  (node?.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");

const visible = (node: Element | null) => node !== null && !node.closest("[hidden]");

const letterRow = (root: HTMLElement) =>
  within(root).queryByRole("button", { name: "九键左侧符号" });
const digitRow = (root: HTMLElement) =>
  within(root).queryByRole("button", { name: "数字键盘左侧符号" });

const editor = (name: string) => screen.getByRole("dialog", { name });

const currentOrder = (dialog: HTMLElement) =>
  [...dialog.querySelectorAll("[data-sidebar-symbol]")].map((node) =>
    node.getAttribute("data-sidebar-symbol"),
  );

const saveButton = (dialog: HTMLElement) =>
  within(dialog).getByRole("button", { name: "保存" }) as HTMLButtonElement;

const visiblePreview = (root: HTMLElement) => {
  const previews = [...root.querySelectorAll<HTMLElement>("[data-key-layout-preview]")].filter(
    (node) => !node.closest("[hidden]"),
  );
  if (previews.length !== 1) throw new Error(`expected one preview, got ${previews.length}`);
  return previews[0];
};

const highlightedCodes = (root: HTMLElement) =>
  [...visiblePreview(root).querySelectorAll("[data-highlighted]")].map((key) =>
    key.getAttribute("data-key-code"),
  );

const follows = (earlier: Node, later: Node) =>
  (earlier.compareDocumentPosition(later) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;

test("the factory tables are HarmonyOS's own and the limits match client-core", () => {
  // 鸿蒙字母层送 ASCII 给 Engine，设置页写成中文标点的样子；数字层就是键盘那张表。
  const fullWidth: Record<string, string> = { ",": "，", ".": "。", "?": "？" };
  expect(NineKeyLayout.sideMarks().map((mark) => fullWidth[mark])).toEqual(
    nineKeySidebarDefaults.letter,
  );
  expect(NineKeyLayout.digitSidebar()).toEqual(nineKeySidebarDefaults.digit);
  expect(nineKeySidebarVisibleRows).toBe(NineKeyLayout.LETTER_SIDEBAR_VISIBLE_ROWS);
  expect([
    nineKeySidebarMaxSymbols,
    nineKeySidebarMaxSymbolCodePoints,
    nineKeySidebarMaxTextUtf16,
  ]).toEqual([30, 4, 200]);
  expect(nineKeySidebarAddableSymbols).toHaveLength(51);
  expect(new Set(nineKeySidebarAddableSymbols).size).toBe(51);
  // 空表、缺键和坏值都读成出厂表。
  expect(nineKeySidebarSymbols({} as Preferences)).toEqual(["，", "。", "？"]);
  expect(
    nineKeySidebarSymbols({ touch_nine_key_sidebar_symbols: [] } as unknown as Preferences),
  ).toEqual(["，", "。", "？"]);
  expect(
    numberPadSidebarSymbols({
      touch_nine_key_digit_sidebar_symbols: "+ -",
    } as unknown as Preferences),
  ).toEqual(["+", "-", "*", "/"]);
  expect(
    nineKeySidebarSymbols({
      touch_nine_key_sidebar_symbols: ["、", 3, ""],
    } as unknown as Preferences),
  ).toEqual(["、"]);
  // 存着的非空表与出厂表逐项相同时按出厂表读（不描边），与 Android `NineKeySidebarPolicy.custom` 相同。
  expect(
    storedNineKeySidebar(
      { touch_nine_key_sidebar_symbols: ["，", "。", "？"] } as Preferences,
      "letter",
    ),
  ).toEqual([]);
  expect(
    storedNineKeySidebar(
      { touch_nine_key_digit_sidebar_symbols: ["+", "-", "*", "/"] } as Preferences,
      "digit",
    ),
  ).toEqual([]);
  expect(
    storedNineKeySidebar(
      { touch_nine_key_sidebar_symbols: ["。", "，", "？"] } as Preferences,
      "letter",
    ),
  ).toEqual(["。", "，", "？"]);
});

test("the summary, the per-item rule and the whole-table rule follow Android", () => {
  expect(nineKeySidebarSummary(["，", "。", "？"])).toBe("， 。 ？");
  expect(nineKeySidebarSummary(["+", "-", "*", "/"])).toBe("+ - * /");
  expect(nineKeySidebarSummary(["，", "。", "？", "！", "、", "；", "：", "…"])).toBe(
    "， 。 ？ ！ 等 8 个",
  );
  // 按码位数，不按字素：👍🏽 两个码位可以，👨‍👩‍👧 五个码位不行；……、:-) 可以。
  for (const symbol of ["……", ":-)", "👍🏽", "#+=", "（）"])
    expect([symbol, validNineKeySidebarSymbol(symbol)]).toEqual([symbol, true]);
  for (const symbol of ["", "a b", "a\u3000b", "a\u00a0b", "\t", "a\u0007", "👨‍👩‍👧", "12345"])
    expect([symbol, validNineKeySidebarSymbol(symbol)]).toEqual([symbol, false]);
  expect(validNineKeySidebar([])).toBe(false);
  expect(validNineKeySidebar(["、", "、"])).toBe(false);
  const thirty = Array.from({ length: 30 }, (_, index) => String(index));
  expect(validNineKeySidebar(thirty)).toBe(true);
  expect(validNineKeySidebar([...thirty, "x"])).toBe(false);
  // 25 项、每项四个码位的 emoji（8 个 UTF-16 单元），项数没超，连起来 224 个单元超了。
  const long = Array.from({ length: 25 }, (_, index) =>
    String.fromCodePoint(0x1f600 + index).repeat(4),
  );
  expect(long.join(" ").length).toBeGreaterThan(nineKeySidebarMaxTextUtf16);
  expect(validNineKeySidebar(long)).toBe(false);
  expect(canAddNineKeySidebarSymbol(["、"], "、")).toBe(false);
  expect(canAddNineKeySidebarSymbol(thirty, "x")).toBe(false);
  expect(canAddNineKeySidebarSymbol(["、"], "；")).toBe(true);
  // 等于出厂表（逐项、同顺序）写空数组，否则写用户的表。
  expect(nineKeySidebarStoredValue("letter", ["，", "。", "？"])).toEqual([]);
  expect(nineKeySidebarStoredValue("letter", ["。", "，", "？"])).toEqual(["。", "，", "？"]);
  expect(nineKeySidebarStoredValue("digit", ["+", "-", "*", "/"])).toEqual([]);
  expect(nineKeySidebarStoredValue("digit", ["+", "-", "*"])).toEqual(["+", "-", "*"]);
});

test("the letter row sits in the 9 key card after the style note, the digit row in 数字键盘", () => {
  renderPage({ touch_nine_key_style: "tall_return" });
  const page = document.body;
  const letter = letterRow(page)!;
  const digit = digitRow(page)!;
  // 9 键分段里：样式胶囊、说明之后是「九键左侧符号」，右侧是出厂表的摘要。
  expect(visible(letter)).toBe(true);
  expect(description(letter)).toBe("， 。 ？");
  const card = visiblePreview(page).closest("section")!;
  expect(card.contains(letter)).toBe(true);
  expect(
    follows(page.querySelector("[data-key-layout-panel='nine'] [data-key-layout-note]")!, letter),
  ).toBe(true);
  // 「数字键盘」一组里：空格位置胶囊和组末说明之后。
  const pad = within(page).getByRole("group", { name: "数字键盘" }).closest("section")!;
  expect(pad.contains(digit)).toBe(true);
  expect(follows(pad.querySelector("[data-key-layout-note]")!, digit)).toBe(true);
  expect(description(digit)).toBe("+ - * /");
  // 字母表那一行只跟着 9 键分段，数字表那一行任何分段都在。
  chooseKind(page, "26 键");
  expect(visible(letterRow(page))).toBe(false);
  expect(visible(digitRow(page))).toBe(true);
});

test("the letter editor adds, removes, reorders and saves only its own preference", () => {
  const { onChange } = renderPage();
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  expect(dialog.textContent).toContain("拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。");
  // 编辑页不画键盘预览。
  expect(dialog.querySelector("[data-key-layout-preview]")).toBeNull();
  expect(currentOrder(dialog)).toEqual(["，", "。", "？"]);
  // 可加入的常用符号里没有已经在表里的项，顺序照 Android。
  const addable = within(dialog)
    .getAllByRole("button", { name: /^添加 / })
    .map((button) => button.textContent);
  expect(addable).toEqual(
    nineKeySidebarAddableSymbols.filter((symbol) => !["，", "。", "？"].includes(symbol)),
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "添加 、" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "添加 ……" }));
  expect(currentOrder(dialog)).toEqual(["，", "。", "？", "、", "……"]);
  expect(within(dialog).queryByRole("button", { name: "添加 、" })).toBeNull();
  fireEvent.click(within(dialog).getByRole("button", { name: "删除 。" }));
  // 读屏和键盘用 ↑ ↓ 挪一格。
  fireEvent.keyDown(within(dialog).getByRole("button", { name: "拖动调整 …… 的顺序" }), {
    key: "ArrowUp",
  });
  expect(currentOrder(dialog)).toEqual(["，", "？", "……", "、"]);
  fireEvent.keyDown(within(dialog).getByRole("button", { name: "拖动调整 ， 的顺序" }), {
    key: "ArrowUp",
  });
  expect(currentOrder(dialog)).toEqual(["，", "？", "……", "、"]);
  // 手动输入一项：去掉首尾空白，含空白、超过四个码位或重复的加不进去。
  const input = within(dialog).getByRole("textbox", { name: "手动输入一个符号" });
  const addTyped = within(dialog).getByRole("button", {
    name: "添加输入的符号",
  }) as HTMLButtonElement;
  for (const bad of ["", "a b", "12345", "、", "　"]) {
    fireEvent.change(input, { target: { value: bad } });
    expect([bad, addTyped.disabled]).toEqual([bad, true]);
  }
  fireEvent.change(input, { target: { value: "  :-)  " } });
  expect(addTyped.disabled).toBe(false);
  fireEvent.click(addTyped);
  expect(currentOrder(dialog)).toEqual(["，", "？", "……", "、", ":-)"]);
  expect((input as HTMLInputElement).value).toBe("");
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.click(saveButton(dialog));
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenLastCalledWith({
    touch_nine_key_sidebar_symbols: ["，", "？", "……", "、", ":-)"],
  });
  // 保存后回到「按键布局」。
  expect(screen.queryByRole("dialog", { name: "九键左侧符号" })).toBeNull();
});

test("dragging the handle moves the item to where the finger is", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "：", "…"] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  // jsdom 不排版：每行 40px 高，自上而下排。
  for (const [index, row] of [...dialog.querySelectorAll("[data-sidebar-symbol]")].entries())
    vi.spyOn(row, "getBoundingClientRect").mockReturnValue({
      top: index * 40,
      height: 40,
      bottom: index * 40 + 40,
      left: 0,
      right: 300,
      width: 300,
      x: 0,
      y: index * 40,
      toJSON: () => ({}),
    });
  const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  // 过了第二行的中线（60）、没过第三行的中线（100）：、 挪到第三位。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 90 });
  expect(currentOrder(dialog)).toEqual(["；", "：", "、", "…"]);
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 90 });
  // 松手以后再移动不挪。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 10 });
  expect(currentOrder(dialog)).toEqual(["；", "：", "、", "…"]);
});

test("restoring the factory table writes an empty array, and an invalid table cannot be saved", () => {
  const { onChange } = renderPage({ touch_nine_key_sidebar_symbols: ["、", "……"] });
  expect(description(letterRow(document.body))).toBe("、 ……");
  fireEvent.click(letterRow(document.body)!);
  let dialog = editor("九键左侧符号");
  expect(currentOrder(dialog)).toEqual(["、", "……"]);
  // 全删光不能保存。
  fireEvent.click(within(dialog).getByRole("button", { name: "删除 、" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "删除 ……" }));
  expect(currentOrder(dialog)).toEqual([]);
  expect(saveButton(dialog).disabled).toBe(true);
  fireEvent.click(within(dialog).getByRole("button", { name: "恢复默认" }));
  expect(currentOrder(dialog)).toEqual(["，", "。", "？"]);
  expect(saveButton(dialog).disabled).toBe(false);
  fireEvent.click(saveButton(dialog));
  expect(onChange).toHaveBeenLastCalledWith({ touch_nine_key_sidebar_symbols: [] });
  // 返回不保存。
  fireEvent.click(letterRow(document.body)!);
  dialog = editor("九键左侧符号");
  fireEvent.click(within(dialog).getByRole("button", { name: "添加 ！" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "返回" }));
  expect(screen.queryByRole("dialog", { name: "九键左侧符号" })).toBeNull();
  expect(onChange).toHaveBeenCalledTimes(1);
});

test("a full table offers nothing more to add", () => {
  const thirty = Array.from({ length: 30 }, (_, index) => String(index));
  renderPage({ touch_nine_key_digit_sidebar_symbols: thirty });
  expect(description(digitRow(document.body))).toBe("0 1 2 3 等 30 个");
  fireEvent.click(digitRow(document.body)!);
  const dialog = editor("数字键盘左侧符号");
  expect(dialog.textContent).toContain("所有九宫格数字键盘共用，#+= 固定在最后。");
  expect(dialog.textContent).toContain("最多 30 个符号。");
  for (const button of within(dialog).getAllByRole("button", { name: /^添加 / }))
    expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(saveButton(dialog).disabled).toBe(false);
});

test("the digit editor writes touch_nine_key_digit_sidebar_symbols, an empty array for the factory table", () => {
  const { onChange } = renderPage({}, "quanpin");
  // 数字表那一行在 26 键分段也在。
  fireEvent.click(digitRow(document.body)!);
  let dialog = editor("数字键盘左侧符号");
  expect(currentOrder(dialog)).toEqual(["+", "-", "*", "/"]);
  fireEvent.click(within(dialog).getByRole("button", { name: "添加 =" }));
  fireEvent.click(saveButton(dialog));
  expect(onChange).toHaveBeenLastCalledWith({
    touch_nine_key_digit_sidebar_symbols: ["+", "-", "*", "/", "="],
  });
  fireEvent.click(digitRow(document.body)!);
  dialog = editor("数字键盘左侧符号");
  fireEvent.click(saveButton(dialog));
  expect(onChange).toHaveBeenLastCalledWith({ touch_nine_key_digit_sidebar_symbols: [] });
  expect(onChange).toHaveBeenCalledTimes(2);
});

test("a custom letter table redraws the 9 key preview's left column and outlines it", () => {
  const { rerender } = renderPage();
  const page = document.body;
  // 出厂表：左列是热力图那三格，不描边。
  expect(visiblePreview(page).querySelector("[data-key-stack]")).toBeNull();
  expect(highlightedCodes(page)).toEqual([]);
  const props = {
    scheme: "nine_key" as const,
    groups: { twentySix: true, fourteen: true, nine: true },
    open: true,
    onChange: vi.fn(),
  };
  rerender(
    <TouchKeyLayoutSettings
      {...props}
      preferences={{ touch_nine_key_sidebar_symbols: ["、", "……", "；", "："] } as Preferences}
    />,
  );
  // 自定义：左列是跨三行的一格，里面是一屏的前三项，每项描边；右列的 ！ 不描。
  const stack = visiblePreview(page).querySelector<HTMLElement>("[data-key-stack]")!;
  expect(stack.getAttribute("data-row-span")).toBe("3");
  expect([...stack.children].map((key) => key.textContent)).toEqual(["、", "……", "；"]);
  expect(highlightedCodes(page)).toEqual(["SoftPunctuation", "SoftPunctuation", "SoftPunctuation"]);
  const rows = [...visiblePreview(page).querySelectorAll("[data-key-layout-row]")];
  expect(rows[1].children[0].textContent).toBe("");
  expect(rows[2].children[0].textContent).toBe("");
  expect(rows[2].children[4].textContent).toBe("！");
  expect(rows[2].children[4].hasAttribute("data-highlighted")).toBe(false);
  // 右侧大回车：不再接 ！，回车照样描边。
  rerender(
    <TouchKeyLayoutSettings
      {...props}
      preferences={
        {
          touch_nine_key_style: "tall_return",
          touch_nine_key_sidebar_symbols: ["、"],
        } as Preferences
      }
    />,
  );
  const single = visiblePreview(page).querySelector<HTMLElement>("[data-key-stack]")!;
  expect([...single.children].map((key) => key.textContent)).toEqual(["、"]);
  expect(visiblePreview(page).textContent).not.toContain("？！");
  expect(highlightedCodes(page)).toEqual(["SoftPunctuation", "Enter"]);
  // 数字表不影响 9 键预览。
  rerender(
    <TouchKeyLayoutSettings
      {...props}
      preferences={{ touch_nine_key_digit_sidebar_symbols: ["="] } as Preferences}
    />,
  );
  expect(visiblePreview(page).querySelector("[data-key-stack]")).toBeNull();
});

test("the editors save through the settings form and settings search finds both rows", async () => {
  const { save } = renderSettings({ touch_keyboard_layout: "nine_key" });
  const dialog = await openKeyLayout();
  fireEvent.click(letterRow(dialog)!);
  const letters = editor("九键左侧符号");
  fireEvent.click(within(letters).getByRole("button", { name: "删除 ？" }));
  fireEvent.click(saveButton(letters));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "九键左侧符号" })).toBeNull());
  // 回到「按键布局」，摘要和预览跟着变。
  expect(description(letterRow(dialog))).toBe("， 。");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.touch_nine_key_sidebar_symbols).toEqual(["，", "。"]);
  expect(saved.touch_nine_key_digit_sidebar_symbols ?? []).toEqual([]);

  const targets = () =>
    collectSettingsSearchTargets(document.body, new Map([["screen-keyboard", "屏幕键盘"]]));
  const letter = targets().find((target) => target.title === "九键左侧符号");
  expect(letter?.path).toEqual(["屏幕键盘", "按键布局", "9 键"]);
  const digit = targets().find((target) => target.title === "数字键盘左侧符号");
  expect(digit?.path).toEqual(["屏幕键盘", "按键布局", "数字键盘"]);
  // 停在 26 键时命中「九键左侧符号」，切到 9 键。
  chooseKind(dialog, "26 键");
  expect(visible(letterRow(dialog))).toBe(false);
  revealSettingsSearchTarget(letter!);
  await waitFor(() => expect(visible(letterRow(dialog))).toBe(true));
});

test("the system back gesture closes the editor first and leaves 按键布局 open", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  fireEvent.click(digitRow(dialog)!);
  expect(editor("数字键盘左侧符号")).toBeTruthy();
  expect(window.history.state?.nineKeySidebar).toBe("digit");
  expect(window.history.state?.keyLayout).toBe(true);
  // 返回按钮经历史记录退一步。
  window.history.back();
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "数字键盘左侧符号" })).toBeNull(),
  );
  expect(screen.getByRole("dialog", { name: "按键布局" })).toBeTruthy();
});
