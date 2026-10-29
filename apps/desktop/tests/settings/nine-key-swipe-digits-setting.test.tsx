// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type InputScheme, type Snapshot } from "@msime/ui";

// 九键滑动输入数字（#10229）：共享 React 设置页给鸿蒙手机画「手势」组里的这一行，读写共享偏好 `touch_nine_key_swipe_digits`。

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const TITLE = "九键滑动输入数字";

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

/** 鸿蒙手机上的选择行是一个打开选项单的按钮，行标题在 `data-row-title` 里。 */
function pickerRow(title: string) {
  return screen
    .queryAllByRole("button")
    .find((button) => button.querySelector("[data-row-title]")?.textContent === title);
}

async function pageReady() {
  await settingsFormReady();
  await screen.findAllByText("键盘高度", undefined, { timeout: 3000 });
}

function choose(label: string) {
  fireEvent.click(pickerRow(TITLE)!);
  const sheet = screen.getByRole("dialog", { name: TITLE });
  fireEvent.click(within(sheet).getByRole("button", { name: label }));
}

test("the phone keyboard page lists 九键滑动输入数字 in 手势 and reads a missing value as 关闭", async () => {
  renderSettings();
  await pageReady();
  const row = await waitFor(() => {
    const found = pickerRow(TITLE);
    if (!found) throw new Error(`no ${TITLE} row`);
    return found;
  });
  const gestures = screen.getByRole("region", { name: "手势" });
  expect(gestures.contains(row)).toBe(true);
  expect(row.textContent).toContain("关闭");
  expect(row.textContent).toContain("沿所选方向滑动输入数字，反方向弹出字母。");
  // 选项单的标题和三项文字三端相同。
  fireEvent.click(row);
  const sheet = screen.getByRole("dialog", { name: TITLE });
  expect(
    within(sheet)
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((text) => ["关闭", "上滑", "下滑"].includes(text ?? "")),
  ).toEqual(["关闭", "上滑", "下滑"]);
});

test("choosing a direction writes the shared preference, and 关闭 is written as off", async () => {
  const { save } = renderSettings();
  await pageReady();
  await waitFor(() => expect(pickerRow(TITLE)).toBeTruthy());
  choose("上滑");
  expect(pickerRow(TITLE)!.textContent).toContain("上滑");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].touch_nine_key_swipe_digits).toBe("up");
  choose("下滑");
  saveSettingsNow();
  await waitFor(() => expect(save.mock.calls.at(-1)?.[1].touch_nine_key_swipe_digits).toBe("down"));
  choose("关闭");
  saveSettingsNow();
  await waitFor(() => expect(save.mock.calls.at(-1)?.[1].touch_nine_key_swipe_digits).toBe("off"));
});

test("a stored direction is shown, and a value this page does not know reads as 关闭", async () => {
  for (const [stored, shown] of [
    ["down", "下滑"],
    ["up", "上滑"],
    ["off", "关闭"],
    ["sideways", "关闭"],
  ] as const) {
    renderSettings({ preferences: { touch_nine_key_swipe_digits: stored } });
    await pageReady();
    const row = await waitFor(() => {
      const found = pickerRow(TITLE);
      if (!found) throw new Error(`no ${TITLE} row`);
      return found;
    });
    expect(row.textContent).toContain(shown);
    cleanup();
    window.history.replaceState({}, "");
  }
});

test("editions without the pinyin nine-key keyboard have no 九键滑动输入数字 row", async () => {
  for (const inputSchemes of [
    ["wubi"],
    ["stroke"],
    ["shuangpin"],
    ["japanese"],
  ] as InputScheme[][]) {
    renderSettings({ inputSchemes, preferences: { scheme: inputSchemes[0] } });
    await pageReady();
    expect(pickerRow(TITLE)).toBeUndefined();
    expect(screen.queryByText(TITLE)).toBeNull();
    cleanup();
    window.history.replaceState({}, "");
  }
});

test("desktop hosts have no 九键滑动输入数字 row", async () => {
  for (const [platform, mobile] of [
    ["harmony", false],
    ["windows", false],
  ] as const) {
    renderSettings({ platform, mobile });
    await pageReady();
    expect(screen.queryByText(TITLE)).toBeNull();
    expect(screen.queryByLabelText(TITLE)).toBeNull();
    cleanup();
    window.history.replaceState({}, "");
  }
});
