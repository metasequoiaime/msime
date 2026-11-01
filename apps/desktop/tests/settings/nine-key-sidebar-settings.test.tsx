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

type SettingsPageProps = Parameters<typeof SettingsPage>[0];

/** `link` 是宿主链接带来的那几项：初始页、它的子目标，或打开着时送来的路由。 */
function renderSettings(
  preferences: Record<string, unknown> = {},
  link: Omit<SettingsPageProps, "client"> = { initialPage: "screen-keyboard" },
) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  const client: SettingsPageProps["client"] = {
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
  };
  const view = render(<SettingsPage client={client} {...link} />);
  const route = (next: Omit<SettingsPageProps, "client">) =>
    view.rerender(<SettingsPage client={client} {...next} />);
  return { save, route };
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
  // 每项前有序号 1..N。
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3."]);
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
  // 删除和挪动后序号重排，仍是 1..N。
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3.", "4."]);
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

/** 每行前的序号，按 DOM 次序。 */
const ordinals = (dialog: HTMLElement) =>
  [...dialog.querySelectorAll("[data-sidebar-symbol] [data-sidebar-ordinal]")].map(
    (node) => node.textContent,
  );

/** 按序号排出来的次序，也就是此刻画面上从上到下看到的表。 */
const shownOrder = (dialog: HTMLElement) =>
  [...dialog.querySelectorAll<HTMLElement>("[data-sidebar-symbol]")]
    .map((row) => ({
      symbol: row.getAttribute("data-sidebar-symbol"),
      at: Number.parseInt(row.querySelector("[data-sidebar-ordinal]")?.textContent ?? "", 10),
    }))
    .sort((left, right) => left.at - right.at)
    .map((row) => row.symbol);

const rowFor = (dialog: HTMLElement, symbol: string) =>
  dialog.querySelector<HTMLElement>(`[data-sidebar-symbol="${symbol}"]`)!;

/** jsdom 不排版：每行 40px 高、自上而下排，`offsetTop` 是布局位置；`getBoundingClientRect` 照真浏览器带上行身上的 translateY，好让「量带着位移的行」这种写法在测试里露馅。 */
function layOutRows(dialog: HTMLElement) {
  for (const row of dialog.querySelectorAll<HTMLElement>("[data-sidebar-symbol]")) {
    const top = () => [...row.parentElement!.children].indexOf(row) * 40;
    Object.defineProperty(row, "offsetTop", { configurable: true, get: top });
    Object.defineProperty(row, "offsetHeight", { configurable: true, get: () => 40 });
    vi.spyOn(row, "getBoundingClientRect").mockImplementation(() => {
      const shift = Number.parseFloat(
        /translateY\((-?[\d.]+)px\)/.exec(row.style.transform)?.[1] ?? "0",
      );
      const y = top() + shift;
      return {
        top: y,
        height: 40,
        bottom: y + 40,
        left: 0,
        right: 300,
        width: 300,
        x: 0,
        y,
        toJSON: () => ({}),
      };
    });
  }
}

test("dragging the handle lifts the row, the others make way and the numbers follow the finger", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "：", "…"] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3.", "4."]);
  const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  // 手指下移 50：、 的中线到 70，过了 ； 的中线（60），没过 ： 的（100）。被拖行跟手，； 让上去一格，序号跟着换。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 70 });
  expect(rowFor(dialog, "、").style.transform).toBe("translateY(50px)");
  expect(rowFor(dialog, "；").style.transform).toBe("translateY(-40px)");
  expect(rowFor(dialog, "：").style.transform).toBe("");
  expect(shownOrder(dialog)).toEqual(["；", "、", "：", "…"]);
  expect(ordinals(dialog)).toEqual(["2.", "1.", "3.", "4."]);
  // 拖动中表本身不动，松手才改。
  expect(currentOrder(dialog)).toEqual(["、", "；", "：", "…"]);
  // 再下移到 110：中线 110 过了 ： 的中线，： 也让上去。落点只比布局中线，被拖行和让位行身上的位移都不算进去。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 110 });
  expect(shownOrder(dialog)).toEqual(["；", "：", "、", "…"]);
  expect(rowFor(dialog, "：").style.transform).toBe("translateY(-40px)");
  // 退回到 90：中线 90 没过 ： 的中线（100），： 回到原位。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 90 });
  expect(shownOrder(dialog)).toEqual(["；", "、", "：", "…"]);
  expect(rowFor(dialog, "：").style.transform).toBe("");
  // 拖出列表也只到最后一格，不拖出卡片。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 900 });
  expect(rowFor(dialog, "、").style.transform).toBe("translateY(120px)");
  expect(shownOrder(dialog)).toEqual(["；", "：", "…", "、"]);
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 110 });
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 110 });
  // 挪动不是交换：、 从第一位挪到第三位，其余依次上移。
  expect(currentOrder(dialog)).toEqual(["；", "：", "、", "…"]);
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3.", "4."]);
  expect(rowFor(dialog, "；").style.transform).toBe("");
  // 松手后再移动不挪。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 10 });
  expect(currentOrder(dialog)).toEqual(["；", "：", "、", "…"]);
});

test("the dropped row slides from the finger into its slot", async () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "："] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 75 });
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 75 });
  expect(currentOrder(dialog)).toEqual(["；", "、", "："]);
  // 手指在格子下面 15px 松开：先停在那里，下一帧过渡回格子。
  expect(rowFor(dialog, "、").style.transform).toBe("translateY(15px)");
  await waitFor(() => expect(rowFor(dialog, "、").style.transform).toBe(""));
  expect(rowFor(dialog, "、").className).toContain("transition-transform");
});

test("deleting a row in the middle of a drag drops the drag", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "：", "…"] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 75 });
  // 另一指点了删除：拿起时量的几何已经对不上，这次拖动作废，表只少了删掉的那一项。
  fireEvent.click(within(dialog).getByRole("button", { name: "删除 …" }));
  expect(rowFor(dialog, "、").style.transform).toBe("");
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3."]);
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 110 });
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 110 });
  expect(currentOrder(dialog)).toEqual(["、", "；", "："]);
});

test("with reduced motion the dropped row lands in its slot at once", () => {
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({ matches: query === "(prefers-reduced-motion: reduce)" })),
  );
  try {
    renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "："] });
    fireEvent.click(letterRow(document.body)!);
    const dialog = editor("九键左侧符号");
    layOutRows(dialog);
    const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
    fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientY: 75 });
    fireEvent.pointerUp(handle, { pointerId: 1, clientY: 75 });
    expect(currentOrder(dialog)).toEqual(["；", "、", "："]);
    // 没有过渡就等不到 transitionend：松手当下就在格子里，也不留着浮起的样式。
    expect(rowFor(dialog, "、").style.transform).toBe("");
    expect(rowFor(dialog, "、").className).not.toContain("z-10");
  } finally {
    vi.unstubAllGlobals();
  }
});

test("the lifted row overrides the row template's own shadow so it does not stay flat", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "："] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  const handle = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  // 行模板的 `[box-shadow:var(--p-row-shadow)]` 排在 `shadow-*` 后面且各平台都是 none，浮起只能改写这个变量。
  const lifted = rowFor(dialog, "、").className.split(" ");
  expect(lifted.some((name) => name.startsWith("[--p-row-shadow:"))).toBe(true);
  expect(lifted.some((name) => name.startsWith("shadow-"))).toBe(false);
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 20 });
});

test("a second finger on another handle neither moves nor ends the drag", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "：", "…"] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  const first = within(dialog).getByRole("button", { name: "拖动调整 、 的顺序" });
  const other = within(dialog).getByRole("button", { name: "拖动调整 ： 的顺序" });
  fireEvent.pointerDown(first, { pointerId: 1, clientY: 20 });
  fireEvent.pointerMove(first, { pointerId: 1, clientY: 70 });
  // 第二根手指按在 ： 的把手上：它不拿起 ：，它的移动和抬起也不改 、 的位置、不提前松手。
  fireEvent.pointerDown(other, { pointerId: 2, clientY: 100 });
  fireEvent.pointerMove(other, { pointerId: 2, clientY: 150 });
  expect(rowFor(dialog, "、").style.transform).toBe("translateY(50px)");
  expect(rowFor(dialog, "：").style.transform).toBe("");
  fireEvent.pointerUp(other, { pointerId: 2, clientY: 150 });
  expect(currentOrder(dialog)).toEqual(["、", "；", "：", "…"]);
  expect(rowFor(dialog, "、").style.transform).toBe("translateY(50px)");
  fireEvent.pointerUp(first, { pointerId: 1, clientY: 70 });
  expect(currentOrder(dialog)).toEqual(["；", "、", "：", "…"]);
});

test("a screen reader moves a row with the hidden 上移 and 下移 buttons and keeps focus on it", () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "："] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  // 首行没有上移，末行没有下移，与 Android 把手上的无障碍操作相同；按钮不进 Tab 次序。
  expect(within(dialog).queryByRole("button", { name: "上移 、" })).toBeNull();
  expect(within(dialog).queryByRole("button", { name: "下移 ：" })).toBeNull();
  const down = within(dialog).getByRole("button", { name: "下移 、" });
  expect(down.tabIndex).toBe(-1);
  down.focus();
  fireEvent.click(down);
  expect(currentOrder(dialog)).toEqual(["；", "、", "："]);
  expect(ordinals(dialog)).toEqual(["1.", "2.", "3."]);
  expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "下移 、" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "下移 、" }));
  expect(currentOrder(dialog)).toEqual(["；", "：", "、"]);
  // 挪到末尾后没有下移了，焦点落在同一行的上移上。
  expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "上移 、" }));
  // 键盘在把手上按 ↓ 挪完，焦点仍在挪过去那一行的把手上。
  const handle = within(dialog).getByRole("button", { name: "拖动调整 ； 的顺序" });
  handle.focus();
  fireEvent.keyDown(handle, { key: "ArrowDown" });
  expect(currentOrder(dialog)).toEqual(["：", "；", "、"]);
  expect(document.activeElement).toBe(
    within(dialog).getByRole("button", { name: "拖动调整 ； 的顺序" }),
  );
});

test("dragging near the bottom edge stops scrolling once the row is pinned at the end of the list", async () => {
  renderPage({ touch_nine_key_sidebar_symbols: ["、", "；", "："] });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  // 页面是 400px 高的滚动区，内容 1600px，列表从 100px 处开始；下面还有可加入的符号和按钮，页面还能往下滚。
  const list = dialog.querySelector("ol")!;
  Object.defineProperty(dialog, "clientHeight", { configurable: true, value: 400 });
  Object.defineProperty(dialog, "scrollHeight", { configurable: true, value: 1600 });
  vi.spyOn(dialog, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 400,
    bottom: 400,
    left: 0,
    right: 300,
    width: 300,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  vi.spyOn(list, "getBoundingClientRect").mockImplementation(() => {
    const y = 100 - dialog.scrollTop;
    return {
      top: y,
      height: 120,
      bottom: y + 120,
      left: 0,
      right: 300,
      width: 300,
      x: 0,
      y,
      toJSON: () => ({}),
    };
  });
  const handle = within(dialog).getByRole("button", { name: "拖动调整 ； 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 160 });
  // 手指进了下沿边缘带，但 ； 只能再往下走一格：夹在列表下沿后不再滚，行留在手指附近。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 390 });
  expect(rowFor(dialog, "；").style.transform).toBe("translateY(40px)");
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(dialog.scrollTop).toBe(0);
  expect(rowFor(dialog, "；").style.transform).toBe("translateY(40px)");
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 390 });
  expect(currentOrder(dialog)).toEqual(["、", "：", "；"]);
});

test("dragging near the bottom edge scrolls the page and the row keeps up", async () => {
  const thirty = Array.from({ length: 30 }, (_, index) => String(index));
  renderPage({ touch_nine_key_sidebar_symbols: thirty });
  fireEvent.click(letterRow(document.body)!);
  const dialog = editor("九键左侧符号");
  layOutRows(dialog);
  // 页面是 400px 高的滚动区，内容 1600px；列表上沿随滚动上移。
  const list = dialog.querySelector("ol")!;
  Object.defineProperty(dialog, "clientHeight", { configurable: true, value: 400 });
  Object.defineProperty(dialog, "scrollHeight", { configurable: true, value: 1600 });
  vi.spyOn(dialog, "getBoundingClientRect").mockReturnValue({
    top: 0,
    height: 400,
    bottom: 400,
    left: 0,
    right: 300,
    width: 300,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  vi.spyOn(list, "getBoundingClientRect").mockImplementation(() => {
    const y = -dialog.scrollTop;
    return {
      top: y,
      height: 1200,
      bottom: y + 1200,
      left: 0,
      right: 300,
      width: 300,
      x: 0,
      y,
      toJSON: () => ({}),
    };
  });
  const handle = within(dialog).getByRole("button", { name: "拖动调整 0 的顺序" });
  fireEvent.pointerDown(handle, { pointerId: 1, clientY: 20 });
  // 手指停在下沿边缘带里，不再移动：页面一帧帧往下滚，被拖行跟着滚过的距离往下走，越过的行让位。
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 390 });
  const offset = () =>
    Number.parseFloat(/translateY\((-?[\d.]+)px\)/.exec(rowFor(dialog, "0").style.transform)![1]);
  expect(offset()).toBe(370);
  // 被拖行的位移始终是手指移动的 370 加上页面滚过的距离；状态更新晚一帧画出来，所以等两者对上。
  await waitFor(() => {
    expect(dialog.scrollTop).toBeGreaterThan(60);
    expect(offset()).toBe(370 + dialog.scrollTop);
  });
  fireEvent.pointerUp(handle, { pointerId: 1, clientY: 390 });
  const scrolled = dialog.scrollTop;
  // 松手就停。
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(dialog.scrollTop).toBe(scrolled);
  expect(currentOrder(dialog).indexOf("0")).toBeGreaterThan(9);
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

// 鸿蒙键盘长按九键左侧符号栏（#10223）：链接带上那张表的共享偏好名，与 Android `edit_sidebar_key` 相同。
test("a link to the letter table opens 按键布局 at the 9 key card with the letter editor open", async () => {
  // 用户在用 26 键，字母表的编辑页仍从 9 键分段进。
  renderSettings(
    { touch_nine_key_sidebar_symbols: ["，", "。", "！"] },
    { initialPage: "screen-keyboard", initialTarget: "touch_nine_key_sidebar_symbols" },
  );
  await settingsFormReady();
  const letters = await waitFor(() => editor("九键左侧符号"));
  expect(currentOrder(letters)).toEqual(["，", "。", "！"]);
  expect(window.history.state?.keyLayout).toBe(true);
  expect(window.history.state?.nineKeySidebar).toBe("letter");
  // 返回关掉编辑页，停在 9 键分段的「按键布局」上，什么也没写。
  window.history.back();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "九键左侧符号" })).toBeNull());
  const dialog = screen.getByRole("dialog", { name: "按键布局" });
  expect(visible(letterRow(dialog))).toBe(true);
});

test("a link that arrives while settings are open opens the digit editor over 按键布局", async () => {
  const { save, route } = renderSettings({}, { initialPage: "input" });
  await settingsFormReady();
  expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull();
  route({
    initialPage: "input",
    route: { page: "screen-keyboard", target: "touch_nine_key_digit_sidebar_symbols", nonce: 1 },
  });
  const digits = await waitFor(() => editor("数字键盘左侧符号"));
  expect(currentOrder(digits)).toEqual([...nineKeySidebarDefaults.digit]);
  expect(window.history.state?.page).toBe("screen-keyboard");
  expect(window.history.state?.keyLayout).toBe(true);
  expect(window.history.state?.nineKeySidebar).toBe("digit");
  // 返回一步关编辑页，再一步关「按键布局」，回到屏幕键盘页本身。
  window.history.back();
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "数字键盘左侧符号" })).toBeNull(),
  );
  expect(screen.getByRole("dialog", { name: "按键布局" })).toBeTruthy();
  window.history.back();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  // 只导航，不写偏好。
  saveSettingsNow();
  expect(save).not.toHaveBeenCalled();
});

test("the link opens the editor once: reopening 按键布局 by hand does not bring it back", async () => {
  renderSettings(
    {},
    { initialPage: "screen-keyboard", initialTarget: "touch_nine_key_digit_sidebar_symbols" },
  );
  await settingsFormReady();
  await waitFor(() => editor("数字键盘左侧符号"));
  window.history.back();
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "数字键盘左侧符号" })).toBeNull(),
  );
  window.history.back();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  const dialog = await openKeyLayout();
  expect(dialog).toBeTruthy();
  expect(screen.queryByRole("dialog", { name: "数字键盘左侧符号" })).toBeNull();
});

test("a target the page does not know only opens 屏幕键盘", async () => {
  const { route } = renderSettings({}, { initialPage: "input" });
  await settingsFormReady();
  route({
    initialPage: "input",
    route: { page: "screen-keyboard", target: "touch_number_row", nonce: 2 },
  });
  await waitFor(() => expect(screen.queryByRole("button", { name: "按键布局" })).toBeTruthy());
  expect(window.history.state?.page).toBe("screen-keyboard");
  expect(window.history.state?.keyLayout).toBeUndefined();
  expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull();
});

test("an edit request switches to the 9 key card for the letter table and needs a 9 key group", () => {
  const view = render(
    <TouchKeyLayoutSettings
      preferences={{} as Preferences}
      scheme="quanpin"
      groups={{ twentySix: true, fourteen: true, nine: true }}
      open
      editRequest={{ kind: "letter", serial: 1 }}
      onChange={vi.fn()}
    />,
  );
  expect(editor("九键左侧符号")).toBeTruthy();
  const card = document.querySelector('[data-key-layout-panel="nine"]');
  expect(visible(card)).toBe(true);
  view.unmount();
  // 版本没有 9 键时字母表无处可改，只停在「按键布局」；数字表照样打开。
  const without = render(
    <TouchKeyLayoutSettings
      preferences={{} as Preferences}
      scheme="quanpin"
      groups={{ twentySix: true, fourteen: true, nine: false }}
      open
      editRequest={{ kind: "letter", serial: 1 }}
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("dialog", { name: "九键左侧符号" })).toBeNull();
  without.rerender(
    <TouchKeyLayoutSettings
      preferences={{} as Preferences}
      scheme="quanpin"
      groups={{ twentySix: true, fourteen: true, nine: false }}
      open
      editRequest={{ kind: "digit", serial: 2 }}
      onChange={vi.fn()}
    />,
  );
  expect(editor("数字键盘左侧符号")).toBeTruthy();
});
