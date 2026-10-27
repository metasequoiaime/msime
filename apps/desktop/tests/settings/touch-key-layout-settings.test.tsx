// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type InputScheme, type Snapshot } from "@msime/ui";
import {
  fourteenKeyBottomRows,
  nineKeyBottomRows,
  nineKeyTallReturnPreview,
  twentySixKeyBottomRows,
} from "../../../../packages/ui/src/keyboard/touch-key-layout-styles";
import { touchKeyLayoutGroups } from "../../../../packages/ui/src/settings/touch-key-layout-settings";
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

/** 选项的名字：标签里单选框旁边那段字，不含读屏跳过的底行示意图。 */
const optionName = (radio: HTMLElement) => radio.parentElement?.textContent ?? "";

const optionLabels = (dialog: HTMLElement, group: string) =>
  within(within(dialog).getByRole("radiogroup", { name: group }))
    .getAllByRole("radio")
    .map(optionName);

const checkedOption = (dialog: HTMLElement, group: string) => {
  const radio = within(within(dialog).getByRole("radiogroup", { name: group }))
    .getAllByRole("radio")
    .find((node) => (node as HTMLInputElement).checked);
  return radio ? optionName(radio) : undefined;
};

test("the phone keyboard page lists 按键布局 in 布局 and opens it as a sub page", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  const groups = [...dialog.querySelectorAll("[data-group-title]")].map((node) => node.textContent);
  expect(groups).toEqual(["26 键", "14 键与 18 键", "9 键"]);
  expect(optionLabels(dialog, "26 键")).toEqual(["标准", "逗号句号在空格两侧", "长空格", "表情键"]);
  expect(optionLabels(dialog, "14 键与 18 键")).toEqual(["标准", "长空格", "表情键"]);
  expect(optionLabels(dialog, "9 键")).toEqual(["标准", "底栏带标点", "表情键", "右侧大回车"]);
  // 文档里没有这几个键时都是标准，数字行关着。
  for (const group of ["26 键", "14 键与 18 键", "9 键"])
    expect(checkedOption(dialog, group)).toBe("标准");
  const numberRow = within(dialog).getByRole("switch", { name: "顶部数字行" }) as HTMLInputElement;
  expect(numberRow.checked).toBe(false);
  expect(within(dialog).getByText(/字母上方加一行 1 到 0/)).toBeTruthy();
  // 返回关上子页，回到键盘页。
  fireEvent.click(within(dialog).getByRole("button", { name: "返回" }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  expect(entryRow()).toBeTruthy();
});

test("settings search finds the sub page's rows under 按键布局 and opens the sub page", async () => {
  renderSettings();
  await settingsFormReady();
  await waitFor(() => {
    if (!entryRow()) throw new Error("no 按键布局 row");
  });
  // 子页关着时它的选项也在索引里，路径带上子页名和组名。
  const targets = () =>
    collectSettingsSearchTargets(document.body, new Map([["screen-keyboard", "屏幕键盘"]]));
  const numberRow = targets().find((target) => target.title === "顶部数字行");
  expect(numberRow?.path).toEqual(["屏幕键盘", "按键布局"]);
  const longSpace = targets().filter((target) => target.title === "长空格");
  expect(longSpace.map((target) => target.path)).toEqual([
    ["屏幕键盘", "按键布局", "26 键"],
    ["屏幕键盘", "按键布局", "14 键与 18 键"],
  ]);
  expect(targets().some((target) => target.title === "底栏带标点")).toBe(true);
  expect(targets().find((target) => target.title === "右侧大回车")?.path).toEqual([
    "屏幕键盘",
    "按键布局",
    "9 键",
  ]);
  expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull();
  // 命中子页里的行时打开子页；再命中正文里的行时关上子页回到正文。
  revealSettingsSearchTarget(numberRow!);
  const dialog = await screen.findByRole("dialog", { name: "按键布局" });
  expect(within(dialog).getByRole("switch", { name: "顶部数字行" })).toBeTruthy();
  expect(entryRow()).toBeNull();
  const height = targets().find((target) => target.title === "键盘高度");
  expect(height).toBeTruthy();
  revealSettingsSearchTarget(height!);
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "按键布局" })).toBeNull());
  expect(entryRow()).toBeTruthy();
});

test("choosing styles and the number row saves the four shared preferences", async () => {
  const { save } = renderSettings();
  const dialog = await openKeyLayout();
  const choose = (group: string, label: string) => {
    // 读屏念的名字只有样式名，示意图被跳过。
    fireEvent.click(
      within(within(dialog).getByRole("radiogroup", { name: group })).getByRole("radio", {
        name: label,
      }),
    );
  };
  choose("26 键", "长空格");
  choose("14 键与 18 键", "表情键");
  choose("9 键", "右侧大回车");
  fireEvent.click(within(dialog).getByRole("switch", { name: "顶部数字行" }));
  expect(checkedOption(dialog, "26 键")).toBe("长空格");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.touch_twenty_six_key_style).toBe("long_space");
  expect(saved.touch_fourteen_key_style).toBe("emoji");
  expect(saved.touch_nine_key_style).toBe("tall_return");
  expect(saved.touch_number_row).toBe(true);
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
  expect(checkedOption(dialog, "26 键")).toBe("表情键");
  expect(checkedOption(dialog, "14 键与 18 键")).toBe("标准");
  expect(checkedOption(dialog, "9 键")).toBe("右侧大回车");
  expect(
    (within(dialog).getByRole("switch", { name: "顶部数字行" }) as HTMLInputElement).checked,
  ).toBe(true);
});

test("each option's preview draws the keyboard's bottom row for that style, key by key", async () => {
  renderSettings();
  const dialog = await openKeyLayout();
  const preview = (group: string, label: string) => {
    const option = within(within(dialog).getByRole("radiogroup", { name: group }))
      .getByRole("radio", { name: label })
      .closest("label");
    const row = option?.querySelector("[data-key-layout-preview]");
    if (!row) throw new Error(`no preview for ${label}`);
    return [...row.children].map((key) => [
      key.textContent,
      Number(key.getAttribute("data-weight")),
    ]);
  };
  expect(preview("26 键", "逗号句号在空格两侧")).toEqual([
    ["123", 1.25],
    ["符", 1.05],
    ["，", 1],
    ["空格", 3],
    ["。", 1],
    ["中", 1.05],
    ["↵", 1.9],
  ]);
  expect(preview("26 键", "长空格")).toEqual([
    ["123", 1.25],
    ["，", 1],
    ["空格", 5.05],
    ["中", 1.05],
    ["↵", 1.9],
  ]);
  expect(preview("9 键", "表情键").map(([label]) => label)).toEqual([
    "符",
    "123",
    "😀",
    "空格",
    "中",
    "↵",
  ]);
  // 右侧大回车画两行：网格最后一行（左列 ？！、右列跨两行的回车）和底行（符 | 123 | 空格 | 中，回车下伸的位置是占位）。
  const tall = within(within(dialog).getByRole("radiogroup", { name: "9 键" }))
    .getByRole("radio", { name: "右侧大回车" })
    .closest("label")
    ?.querySelector("[data-key-layout-frame]");
  if (!tall) throw new Error("no frame preview for 右侧大回车");
  const frame = [...tall.querySelectorAll("[data-key-layout-preview]")].map((row) =>
    [...row.children].map((key) => [
      key.textContent,
      Number(key.getAttribute("data-weight")),
      Number(key.getAttribute("data-row-span")),
    ]),
  );
  expect(frame).toEqual([
    [
      ["？！", 1.15, 1],
      ["PQRS", 2, 1],
      ["TUV", 2, 1],
      ["WXYZ", 2, 1],
      ["↵", 1.15, 2],
    ],
    [
      ["符", 1.15, 1],
      ["123", 1.25, 1],
      ["空格", 3.7, 1],
      ["中", 1.05, 1],
      ["", 1.15, 1],
    ],
  ]);
  // 跨两行的回车高两行加一道行距，下边距收回一行，所在行不变高。
  const returnCap = tall.querySelector("[data-row-span='2']") as HTMLElement;
  expect(returnCap.style.height).toBe("51px");
  expect(returnCap.style.marginBottom).toBe("-27px");
});

test("the preview rows are the HarmonyOS keyboard's rows, shares included", () => {
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
  expect(nineKeyTallReturnPreview[1].slice(0, -1)).toEqual(
    nineKeyBottomRows.tall_return.slice(0, -1),
  );
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
  const groups = [...dialog.querySelectorAll("[data-group-title]")].map((node) => node.textContent);
  expect(groups).toEqual(["26 键", "14 键与 18 键"]);
  expect(within(dialog).getByRole("switch", { name: "顶部数字行" })).toBeTruthy();
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
