// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type InputScheme, type Snapshot } from "@msime/ui";

// 九键 flick 阈值（#11244）：共享 React 设置页给鸿蒙手机画滑块，读写共享偏好 `touch_nine_key_flick_threshold`。

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const TITLE = "九键 flick 阈值";

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

async function pageReady() {
  await settingsFormReady();
  await screen.findAllByText("键盘高度", undefined, { timeout: 3000 });
}

function thresholdSlider() {
  return screen.getByRole("slider", { name: TITLE });
}

test("手机键盘页在手势中显示九键 flick 阈值，并读取缺省值 24", async () => {
  renderSettings();
  await pageReady();
  const slider = await waitFor(thresholdSlider);
  const gestures = screen.getByRole("region", { name: "手势" });
  expect(gestures.contains(slider)).toBe(true);
  expect(slider.getAttribute("min")).toBe("14");
  expect(slider.getAttribute("max")).toBe("48");
  expect(slider.getAttribute("step")).toBe("2");
  expect((slider as HTMLInputElement).value).toBe("24");
  expect(slider.getAttribute("aria-valuetext")).toBe("24 dp");
});

test("调整滑块写入共享九键 flick 阈值", async () => {
  const { save } = renderSettings();
  await pageReady();
  const slider = await waitFor(thresholdSlider);
  fireEvent.change(slider, { target: { value: "32" } });
  expect((slider as HTMLInputElement).value).toBe("32");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].touch_nine_key_flick_threshold).toBe(32);
});

test("存储值超出范围或不在步进上时回退到 24", async () => {
  for (const stored of [13, 25, 49, "24"]) {
    renderSettings({ preferences: { touch_nine_key_flick_threshold: stored } });
    await pageReady();
    expect(((await waitFor(thresholdSlider)) as HTMLInputElement).value).toBe("24");
    cleanup();
    window.history.replaceState({}, "");
  }
});

test("没有全拼九键版本时不显示九键 flick 阈值", async () => {
  for (const inputSchemes of [
    ["wubi"],
    ["stroke"],
    ["shuangpin"],
    ["japanese"],
  ] as InputScheme[][]) {
    renderSettings({ inputSchemes, preferences: { scheme: inputSchemes[0] } });
    await pageReady();
    expect(screen.queryByRole("slider", { name: TITLE })).toBeNull();
    expect(screen.queryByText(TITLE)).toBeNull();
    cleanup();
    window.history.replaceState({}, "");
  }
});

test("桌面宿主不显示九键 flick 阈值", async () => {
  for (const [platform, mobile] of [
    ["harmony", false],
    ["windows", false],
  ] as const) {
    renderSettings({ platform, mobile });
    await pageReady();
    expect(screen.queryByRole("slider", { name: TITLE })).toBeNull();
    expect(screen.queryByText(TITLE)).toBeNull();
    cleanup();
    window.history.replaceState({}, "");
  }
});
