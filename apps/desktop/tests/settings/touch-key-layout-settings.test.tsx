// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type InputScheme, type Snapshot } from "@msime/ui";
import {
  fourteenKeyBottomRows,
  nineKeyBottomRows,
  twentySixKeyBottomRows,
} from "../../../../packages/ui/src/keyboard/touch-key-layout-styles";
import {
  TouchKeyLayoutSettings,
  touchKeyLayoutGroups,
  touchKeyLayoutKind,
} from "../../../../packages/ui/src/settings/touch-key-layout-settings";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import {
  collectSettingsSearchTargets,
  revealSettingsSearchTarget,
} from "../../../../packages/ui/src/settings/settings-search-index";
// 设置页的示意图和 HarmonyOS 键盘读同一组排法：键盘的样式表改了，这里跟着变红。
import {
  StyledFace,
  TouchLayoutStylePolicy,
} from "../../../../platforms/harmony/entry/src/main/ets/keyboard/TouchLayoutStylePolicy";
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

function renderSettings({
  preferences,
  platform = "harmony",
  mobile = true,
  inputSchemes,
}: {
  preferences?: Record<string, unknown>;
  platform?: string;
  mobile?: boolean;
  inputSchemes?: InputScheme[];
} = {}) {
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
          platform,
          mobile_settings: mobile,
          panel_windows: !mobile,
          touch_toolbar_tools: mobile,
          ...(inputSchemes ? { input_schemes: inputSchemes } : {}),
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  return { save };
}

const entryRow = () => screen.queryByRole("button", { name: "按键布局" });

async function openKeyLayout() {
  await settingsFormReady();
  const row = await waitFor(() => {
    const found = entryRow();
    if (!found) throw new Error("no 按键布局 row");
    return found;
  });
  fireEvent.click(row);
  return screen.getByRole("dialog", { name: "按键布局" });
}

/** 读屏念的描述：`aria-describedby` 指向的各段文字按次序拼起来。 */
const description = (node: HTMLElement | null) =>
  (node?.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");

/** 键盘类型分段控件；只有一种类型时不画，返回 null。 */
const kindControl = (dialog: HTMLElement) =>
  within(dialog).queryByRole("radiogroup", { name: "键盘类型" });

const kindLabels = (dialog: HTMLElement) =>
  within(kindControl(dialog)!)
    .getAllByRole("radio")
    .map((radio) => radio.parentElement?.textContent ?? "");

const checkedKind = (dialog: HTMLElement) => {
  const control = kindControl(dialog);
  if (!control) return undefined;
  const radio = within(control)
    .getAllByRole("radio")
    .find((node) => (node as HTMLInputElement).checked);
  return radio?.parentElement?.textContent ?? undefined;
};

const chooseKind = (dialog: HTMLElement, label: string) =>
  fireEvent.click(within(kindControl(dialog)!).getByRole("radio", { name: label }));

/** 当前分段（唯一没有隐藏的那一块）里的样式胶囊。 */
const chipGroup = (dialog: HTMLElement) => {
  const groups = within(dialog)
    .getAllByRole("group")
    .filter((node) => node.getAttribute("aria-label")?.endsWith("样式"));
  if (groups.length !== 1) throw new Error(`expected one visible chip row, got ${groups.length}`);
  return groups[0];
};

const chipLabels = (dialog: HTMLElement) =>
  within(chipGroup(dialog))
    .getAllByRole("button")
    .map((chip) => chip.textContent);

const pressedChip = (dialog: HTMLElement) =>
  within(chipGroup(dialog))
    .getAllByRole("button")
    .filter((chip) => chip.getAttribute("aria-pressed") === "true")
    .map((chip) => chip.textContent);

const chooseChip = (dialog: HTMLElement, label: string) =>
  fireEvent.click(within(chipGroup(dialog)).getByRole("button", { name: label }));

/** 当前分段的整块预览：读屏跳过它，所以按 data 属性找，隐藏的分段不算。 */
const visiblePreview = (dialog: HTMLElement) => {
  const previews = [...dialog.querySelectorAll<HTMLElement>("[data-key-layout-preview]")].filter(
    (node) => !node.closest("[hidden]"),
  );
  if (previews.length !== 1)
    throw new Error(`expected one visible preview, got ${previews.length}`);
  return previews[0];
};

const previewRows = (dialog: HTMLElement) =>
  [...visiblePreview(dialog).querySelectorAll("[data-key-layout-row]")].map((row) =>
    [...row.children].map((key) => key.textContent),
  );

/** 预览里描边高亮的键，按键 id 列出。 */
const highlightedCodes = (dialog: HTMLElement) =>
  [...visiblePreview(dialog).querySelectorAll("[data-highlighted]")].map((key) =>
    key.getAttribute("data-key-code"),
  );

const note = (dialog: HTMLElement) =>
  [...dialog.querySelectorAll("[data-key-layout-note]")]
    .filter((node) => !node.closest("[hidden]"))
    .map((node) => node.textContent);

test("the phone keyboard page lists 按键布局 in 布局 and opens it as a sub page", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  // 分段控件只列版本提供的键盘类型，全拼用户默认停在 26 键。
  expect(kindLabels(dialog)).toEqual(["26 键", "14/18 键", "9 键"]);
  expect(checkedKind(dialog)).toBe("26 键");
  expect(chipLabels(dialog)).toEqual(["标准", "逗号句号在空格两侧", "长空格", "表情键"]);
  // 文档里没有这几个键时都是标准：标准不高亮、没有说明，数字行关着。
  expect(pressedChip(dialog)).toEqual(["标准"]);
  expect(highlightedCodes(dialog)).toEqual([]);
  expect(note(dialog)).toEqual([]);
  expect(description(chipGroup(dialog))).toBe("");
  const numberRow = within(dialog).getByRole("switch", { name: "顶部数字行" }) as HTMLInputElement;
  expect(numberRow.checked).toBe(false);
  expect(description(numberRow)).toBe("在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。");
  // 原来的三条组末说明已经由各样式的说明取代。
  expect(dialog.textContent).not.toContain("长空格：");
  // 切分段只换预览和胶囊。
  chooseKind(dialog, "14/18 键");
  expect(chipLabels(dialog)).toEqual(["标准", "长空格", "表情键"]);
  chooseKind(dialog, "9 键");
  expect(chipLabels(dialog)).toEqual(["标准", "底栏带标点", "表情键", "右侧大回车"]);
  // 9 键时照样列数字行开关（它对 26/14/18 生效）。
  expect(within(dialog).getByRole("switch", { name: "顶部数字行" })).toBeTruthy();
  // 返回关上子页，回到键盘页。
  fireEvent.click(within(dialog).getByRole("button", { name: "返回" }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  expect(entryRow()?.textContent).toContain("各类键盘的底行样式和顶部数字行");
  expect(description(entryRow())).toBe("各类键盘的底行样式和顶部数字行");
  // 再打开时分段回到当前在用的键盘类型。
  fireEvent.click(entryRow()!);
  expect(checkedKind(screen.getByRole("dialog", { name: "按键布局" }))).toBe("26 键");
});

test("the sub page opens on the key type the user is typing with", async () => {
  expect(touchKeyLayoutKind("quanpin", undefined, undefined)).toBe("twentySix");
  expect(touchKeyLayoutKind("cantonese", undefined, undefined)).toBe("twentySix");
  expect(touchKeyLayoutKind("xiaohe", "twenty_six_key", undefined)).toBe("twentySix");
  expect(touchKeyLayoutKind("ziranma", "eighteen_key", undefined)).toBe("fourteen");
  expect(touchKeyLayoutKind("wubi", undefined, "fourteen_key")).toBe("fourteen");
  expect(touchKeyLayoutKind("wubi", undefined, "twenty_six_key")).toBe("twentySix");
  expect(touchKeyLayoutKind("fourteen_key", undefined, undefined)).toBe("fourteen");
  expect(touchKeyLayoutKind("eighteen_key", undefined, undefined)).toBe("fourteen");
  expect(touchKeyLayoutKind("nine_key", undefined, undefined)).toBe("nine");
  expect(touchKeyLayoutKind("nine_key_shuangpin", undefined, undefined)).toBe("nine");
  expect(touchKeyLayoutKind("stroke", undefined, undefined)).toBe("nine");
  expect(touchKeyLayoutKind("japanese", undefined, undefined)).toBeNull();
  expect(touchKeyLayoutKind("handwriting", undefined, undefined)).toBeNull();
  for (const [preferences, kind] of [
    [{ touch_keyboard_layout: "nine_key" }, "9 键"],
    [
      {
        touch_keyboard_schemes: { enabled: ["quanpin", "eighteen_key"], selected: "eighteen_key" },
      },
      "14/18 键",
    ],
    [{ scheme: "shuangpin", touch_shuangpin_layout: "fourteen_key" }, "14/18 键"],
    // 判断不了（日文 26 键）时停在第一段。
    [
      { touch_keyboard_schemes: { enabled: ["quanpin", "japanese"], selected: "japanese" } },
      "26 键",
    ],
  ] as const) {
    renderSettings({ preferences });
    const dialog = await openKeyLayout();
    expect(checkedKind(dialog)).toBe(kind);
    // 切分段不写偏好。
    chooseKind(dialog, kind === "9 键" ? "26 键" : "9 键");
    saveSettingsNow();
    cleanup();
    window.history.replaceState({}, "");
  }
});

test("choosing styles and the number row saves the four shared preferences", async () => {
  const { save } = renderSettings();
  const dialog = await openKeyLayout();
  chooseChip(dialog, "长空格");
  expect(pressedChip(dialog)).toEqual(["长空格"]);
  chooseKind(dialog, "14/18 键");
  chooseChip(dialog, "表情键");
  chooseKind(dialog, "9 键");
  chooseChip(dialog, "右侧大回车");
  fireEvent.click(within(dialog).getByRole("switch", { name: "顶部数字行" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.touch_twenty_six_key_style).toBe("long_space");
  expect(saved.touch_fourteen_key_style).toBe("emoji");
  expect(saved.touch_nine_key_style).toBe("tall_return");
  expect(saved.touch_number_row).toBe(true);
});

test("clicking a chip calls onChange with its own preference, and the chosen chip does nothing", () => {
  const onChange = vi.fn();
  render(
    <TouchKeyLayoutSettings
      preferences={{ touch_nine_key_style: "emoji" } as Preferences}
      scheme="nine_key"
      groups={{ twentySix: true, fourteen: true, nine: true }}
      open
      onChange={onChange}
    />,
  );
  const page = document.body;
  expect(checkedKind(page)).toBe("9 键");
  // 点已选中的胶囊不做事。
  chooseChip(page, "表情键");
  expect(onChange).not.toHaveBeenCalled();
  chooseChip(page, "底栏带标点");
  expect(onChange).toHaveBeenLastCalledWith({ touch_nine_key_style: "comma_period" });
  chooseKind(page, "26 键");
  // 切分段不写偏好。
  expect(onChange).toHaveBeenCalledTimes(1);
  chooseChip(page, "逗号句号在空格两侧");
  expect(onChange).toHaveBeenLastCalledWith({ touch_twenty_six_key_style: "comma_period" });
  chooseKind(page, "14/18 键");
  chooseChip(page, "长空格");
  expect(onChange).toHaveBeenLastCalledWith({ touch_fourteen_key_style: "long_space" });
  fireEvent.click(within(page).getByRole("switch", { name: "顶部数字行" }));
  expect(onChange).toHaveBeenLastCalledWith({ touch_number_row: true });
  expect(onChange).toHaveBeenCalledTimes(4);
});

test("a stored style is shown, and one this page does not know reads as 标准", async () => {
  renderSettings({
    preferences: {
      touch_twenty_six_key_style: "emoji",
      touch_fourteen_key_style: "split_bar",
      touch_nine_key_style: "tall_return",
      touch_number_row: true,
    },
  });
  const dialog = await openKeyLayout();
  expect(pressedChip(dialog)).toEqual(["表情键"]);
  chooseKind(dialog, "14/18 键");
  expect(pressedChip(dialog)).toEqual(["标准"]);
  chooseKind(dialog, "9 键");
  expect(pressedChip(dialog)).toEqual(["右侧大回车"]);
  expect(
    (within(dialog).getByRole("switch", { name: "顶部数字行" }) as HTMLInputElement).checked,
  ).toBe(true);
});

test("the note says only the chosen style's side effect, word for word across platforms", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  const notes: [string, string, string | undefined][] = [
    ["26 键", "标准", undefined],
    ["26 键", "逗号句号在空格两侧", undefined],
    ["26 键", "长空格", "符号键移到工具栏。"],
    ["26 键", "表情键", "长按 123 打开符号。"],
    ["14/18 键", "标准", undefined],
    ["14/18 键", "长空格", undefined],
    ["14/18 键", "表情键", "长按 123 打开符号。"],
    ["9 键", "标准", undefined],
    ["9 键", "底栏带标点", undefined],
    ["9 键", "表情键", undefined],
    ["9 键", "右侧大回车", "回车竖占两行，符号键在左下角。"],
  ];
  for (const [kind, style, text] of notes) {
    chooseKind(dialog, kind);
    chooseChip(dialog, style);
    // 没有说明时不画这一行（不留空位），有说明时读屏把它当作这排胶囊的描述。
    expect(note(dialog)).toEqual(text ? [text] : []);
    expect(description(chipGroup(dialog))).toBe(text ?? "");
  }
});

test("the preview draws the whole keyboard and outlines the keys the style changes", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  // 26 键：三行字母（⇧ z–m ⌫）加所选样式的底行；读屏跳过整个预览。
  expect(visiblePreview(dialog).getAttribute("aria-hidden")).toBe("true");
  expect(previewRows(dialog)).toEqual([
    [..."qwertyuiop"],
    ["", ..."asdfghjkl", ""],
    ["⇧", ..."zxcvbnm", "⌫"],
    ["123", "符", "空格", "，", "中", "↵"],
  ]);
  const highlights: [string, string, string[]][] = [
    ["26 键", "标准", []],
    ["26 键", "逗号句号在空格两侧", ["Comma", "Period"]],
    ["26 键", "长空格", ["Space"]],
    ["26 键", "表情键", ["SoftEmoji"]],
    ["14/18 键", "标准", []],
    ["14/18 键", "长空格", ["Space"]],
    ["14/18 键", "表情键", ["SoftEmoji"]],
    ["9 键", "标准", []],
    ["9 键", "底栏带标点", ["Comma", "Period"]],
    ["9 键", "表情键", ["SoftEmoji"]],
    ["9 键", "右侧大回车", ["Enter"]],
  ];
  for (const [kind, style, codes] of highlights) {
    chooseKind(dialog, kind);
    chooseChip(dialog, style);
    expect([kind, style, highlightedCodes(dialog)]).toEqual([kind, style, codes]);
  }
  // 回车用强调色；14/18 键画 14 键的字母格。
  chooseKind(dialog, "14/18 键");
  expect(
    visiblePreview(dialog).querySelector("[data-key-code='Enter']")?.getAttribute("data-key-tone"),
  ).toBe("return");
  expect(previewRows(dialog).slice(0, 3)).toEqual([
    ["QW", "ER", "TY", "UI", "OP"],
    ["AS", "DF", "GH", "JK", "L"],
    ["符", "ZX", "CV", "BN", "M", "⌫"],
  ]);
  // 9 键「右侧大回车」：左侧符号栏、九宫格，回车竖跨最后两行（下一行同一位置是占位），符 在左下角。
  chooseKind(dialog, "9 键");
  expect(previewRows(dialog)).toEqual([
    ["，", "@#", "ABC", "DEF", "⌫"],
    ["。", "GHI", "JKL", "MNO", "分词"],
    ["？！", "PQRS", "TUV", "WXYZ", "↵"],
    ["符", "123", "空格", "中", ""],
  ]);
  const tall = visiblePreview(dialog).querySelector("[data-key-code='Enter']");
  expect(tall?.getAttribute("data-row-span")).toBe("2");
  chooseChip(dialog, "标准");
  expect(previewRows(dialog).at(-1)).toEqual(["符", "123", "空格", "中", "↵"]);
});

test("the number row is drawn over 26 and 14/18 keys when it is on, never over 9 keys", async () => {
  renderSettings({
    preferences: {
      touch_keyboard_schemes: { enabled: ["quanpin", "eighteen_key"], selected: "eighteen_key" },
    },
  });
  const dialog = await openKeyLayout();
  const numberRow = () =>
    visiblePreview(dialog).querySelector("[data-key-layout-row='number']")?.textContent ?? null;
  // 用户在用 18 键时 14/18 键画 18 键的字母格。
  expect(checkedKind(dialog)).toBe("14/18 键");
  expect(previewRows(dialog)[0]).toEqual(["Q", "WE", "RT", "Y", "U", "IO", "P"]);
  expect(numberRow()).toBeNull();
  fireEvent.click(within(dialog).getByRole("switch", { name: "顶部数字行" }));
  expect(numberRow()).toBe("1234567890");
  chooseKind(dialog, "26 键");
  expect(numberRow()).toBe("1234567890");
  expect(previewRows(dialog)).toHaveLength(5);
  chooseKind(dialog, "9 键");
  expect(numberRow()).toBeNull();
  expect(previewRows(dialog)).toHaveLength(4);
});

test("settings search finds every style under its key type and opens that segment", async () => {
  renderSettings();
  await settingsFormReady();
  await waitFor(() => {
    if (!entryRow()) throw new Error("no 按键布局 row");
  });
  // 子页关着时它的样式也在索引里，路径带上子页名和键盘类型。
  const targets = () =>
    collectSettingsSearchTargets(document.body, new Map([["screen-keyboard", "屏幕键盘"]]));
  const numberRow = targets().find((target) => target.title === "顶部数字行");
  expect(numberRow?.path).toEqual(["屏幕键盘", "按键布局"]);
  const longSpace = targets().filter((target) => target.title === "长空格");
  expect(longSpace.map((target) => target.path)).toEqual([
    ["屏幕键盘", "按键布局", "26 键"],
    ["屏幕键盘", "按键布局", "14/18 键"],
  ]);
  expect(targets().some((target) => target.title === "底栏带标点")).toBe(true);
  const tall = targets().find((target) => target.title === "右侧大回车");
  expect(tall?.path).toEqual(["屏幕键盘", "按键布局", "9 键"]);
  expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull();
  // 命中别的分段里的样式时打开子页并切到那一段；再命中正文里的行时关上子页回到正文。
  revealSettingsSearchTarget(tall!);
  const dialog = await screen.findByRole("dialog", { name: "按键布局" });
  await waitFor(() => expect(checkedKind(dialog)).toBe("9 键"));
  expect(chipLabels(dialog)).toContain("右侧大回车");
  expect(entryRow()).toBeNull();
  const height = targets().find((target) => target.title === "键盘高度");
  expect(height).toBeTruthy();
  revealSettingsSearchTarget(height!);
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  expect(entryRow()).toBeTruthy();
});

test("the bottom rows are the HarmonyOS keyboard's rows, shares included", () => {
  const round = (value: number) => Math.round(value * 100) / 100;
  for (const style of ["standard", "comma_period", "long_space", "emoji"] as const)
    expect(twentySixKeyBottomRows[style].map((key) => round(key.weight))).toEqual(
      TouchLayoutStylePolicy.designRow(true, false, StyledFace.TWENTY_SIX, style).map((entry) =>
        round(entry.share),
      ),
    );
  for (const style of ["standard", "long_space", "emoji"] as const)
    expect(fourteenKeyBottomRows[style].map((key) => round(key.weight))).toEqual(
      TouchLayoutStylePolicy.designRow(false, false, StyledFace.FOURTEEN, style).map((entry) =>
        round(entry.share),
      ),
    );
  for (const style of ["standard", "comma_period", "emoji"] as const)
    expect(nineKeyBottomRows[style].map((key) => round(key.weight))).toEqual(
      NineKeyLayout.actionRow(false, false, style).map((action) => round(action.weight)),
    );
  // 右侧大回车：符 在左列下方、回车在右列，中间一段就是键盘网格下方的那一行。
  expect(nineKeyBottomRows.tall_return.map((key) => round(key.weight))).toEqual([
    NineKeyLayout.SIDE_SHARE,
    ...NineKeyLayout.actionRow(false, false, "tall_return").map((action) => round(action.weight)),
    NineKeyLayout.SIDE_SHARE,
  ]);
  // 123 和 中 占整行的比例与标准九键底行相同，空格占剩下的部分。
  const fraction = (row: readonly { weight: number }[], index: number) =>
    round(row[index].weight / row.reduce((sum, key) => sum + key.weight, 0));
  expect(nineKeyBottomRows.tall_return.map((key) => key.code)).toEqual([
    "SoftSymbol",
    "SoftLayer",
    "Space",
    "SoftLanguage",
    "Enter",
  ]);
  expect(fraction(nineKeyBottomRows.tall_return, 1)).toBe(fraction(nineKeyBottomRows.standard, 1));
  expect(fraction(nineKeyBottomRows.tall_return, 3)).toBe(fraction(nineKeyBottomRows.standard, 3));
});

test("groups follow the key types the edition offers", async () => {
  expect(touchKeyLayoutGroups(["wubi"])).toEqual({ twentySix: true, fourteen: true, nine: false });
  expect(touchKeyLayoutGroups(["stroke"])).toEqual({
    twentySix: false,
    fourteen: false,
    nine: true,
  });
  expect(touchKeyLayoutGroups(["cantonese"])).toEqual({
    twentySix: true,
    fourteen: false,
    nine: false,
  });
  expect(touchKeyLayoutGroups(["japanese", "korean"])).toEqual({
    twentySix: false,
    fourteen: false,
    nine: false,
  });
  renderSettings({ inputSchemes: ["wubi"] });
  const dialog = await openKeyLayout();
  expect(kindLabels(dialog)).toEqual(["26 键", "14/18 键"]);
  expect(within(dialog).getByRole("switch", { name: "顶部数字行" })).toBeTruthy();
  cleanup();
  window.history.replaceState({}, "");
  // 只有一种类型（笔画版只有 9 键）时不画分段控件，也没有数字行开关。
  renderSettings({ inputSchemes: ["stroke"], preferences: { scheme: "stroke" } });
  const nine = await openKeyLayout();
  expect(kindControl(nine)).toBeNull();
  expect(chipLabels(nine)).toEqual(["标准", "底栏带标点", "表情键", "右侧大回车"]);
  expect(within(nine).queryByRole("switch", { name: "顶部数字行" })).toBeNull();
});

test("an edition without Chinese key types has no 按键布局 row", async () => {
  renderSettings({ inputSchemes: ["japanese"], preferences: { scheme: "japanese" } });
  await settingsFormReady();
  await screen.findByLabelText("键盘高度", undefined, { timeout: 3000 });
  expect(entryRow()).toBeNull();
});

test("hosts other than the HarmonyOS phone keyboard have no 按键布局 row", async () => {
  for (const [platform, mobile] of [
    ["harmony", false],
    ["android", true],
    ["windows", false],
  ] as const) {
    renderSettings({ platform, mobile });
    await settingsFormReady();
    await screen.findByLabelText("键盘高度", undefined, { timeout: 3000 });
    expect(entryRow()).toBeNull();
    cleanup();
  }
});
