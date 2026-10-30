// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type MobileKeyboardFeedback, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

// 与 Android `KeyboardOptionsPage` 逐字相同。
const title = "分离式键盘";
const description =
  "默认仅在大屏横屏时生效：26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入；右半边也有 G 和 V";
const portraitTitle = "竖屏也分离";
const portraitDescription = "平板或折叠屏内屏竖放时也分离；回到窄外屏后自动使用完整键盘";
const thresholdTitle = "分离启用宽度";
const gapTitle = "分离中间空隙";

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

const feedback: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: false,
  hapticStrength: "medium",
};

function renderSettings({
  capability,
  preferences,
}: {
  /** 宿主报告 `touch_keyboard_split`；鸿蒙的设置桥只在平板和可折叠手机上报告。 */
  capability?: boolean;
  preferences?: Record<string, unknown>;
}) {
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
          ...(capability === undefined ? {} : { touch_keyboard_split: capability }),
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        mobileKeyboardFeedback: {
          load: vi.fn().mockResolvedValue(feedback),
          save: vi.fn(async (next: MobileKeyboardFeedback) => next),
        },
      }}
    />,
  );
  return { save };
}

test.each([undefined, false])(
  "a Harmony host that does not report the split keyboard (%s) lists none of its rows",
  async (capability) => {
    renderSettings({ capability });
    await settingsFormReady();
    await screen.findByText("键盘高度");
    expect(screen.queryByRole("switch", { name: title })).toBeNull();
    expect(screen.queryByRole("switch", { name: portraitTitle })).toBeNull();
    expect(screen.queryByRole("slider", { name: thresholdTitle })).toBeNull();
    expect(screen.queryByRole("slider", { name: gapTitle })).toBeNull();
  },
);

test("a Harmony tablet shows the Android wording, defaults and bounds, with the dependents off until the switch is on", async () => {
  renderSettings({ capability: true });
  await settingsFormReady();
  const main = (await screen.findByRole("switch", { name: title })) as HTMLInputElement;
  expect(main.checked).toBe(false);
  expect(screen.getByText(description)).toBeTruthy();
  expect(screen.getByText(portraitDescription)).toBeTruthy();
  const portrait = screen.getByRole("switch", { name: portraitTitle }) as HTMLInputElement;
  const threshold = screen.getByRole("slider", { name: thresholdTitle }) as HTMLInputElement;
  const gap = screen.getByRole("slider", { name: gapTitle }) as HTMLInputElement;
  expect(portrait.disabled).toBe(true);
  expect(threshold.disabled).toBe(true);
  expect(gap.disabled).toBe(true);
  // 缺键按 client-core 的默认值读：600 dp、25%。
  expect(threshold.value).toBe("600");
  expect([threshold.min, threshold.max, threshold.step]).toEqual(["480", "1000", "20"]);
  expect(gap.value).toBe("25");
  expect([gap.min, gap.max, gap.step]).toEqual(["10", "40", "5"]);
});

test("each split row writes its own shared preference", async () => {
  const { save } = renderSettings({ capability: true });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("switch", { name: title }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].touch_keyboard_split).toBe(true);

  await waitFor(() =>
    expect((screen.getByRole("switch", { name: portraitTitle }) as HTMLInputElement).disabled).toBe(
      false,
    ),
  );
  fireEvent.click(screen.getByRole("switch", { name: portraitTitle }));
  fireEvent.change(screen.getByRole("slider", { name: thresholdTitle }), {
    target: { value: "720" },
  });
  fireEvent.change(screen.getByRole("slider", { name: gapTitle }), { target: { value: "35" } });
  saveSettingsNow();
  await waitFor(() => expect(save.mock.calls.at(-1)?.[1].touch_keyboard_split_gap).toBe(35));
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.touch_keyboard_split).toBe(true);
  expect(saved.touch_keyboard_split_portrait).toBe(true);
  expect(saved.touch_keyboard_split_threshold).toBe(720);
});

test("stored values are shown as they are", async () => {
  renderSettings({
    capability: true,
    preferences: {
      touch_keyboard_split: true,
      touch_keyboard_split_portrait: true,
      touch_keyboard_split_threshold: 800,
      touch_keyboard_split_gap: 15,
    },
  });
  await settingsFormReady();
  expect(((await screen.findByRole("switch", { name: title })) as HTMLInputElement).checked).toBe(
    true,
  );
  expect((screen.getByRole("switch", { name: portraitTitle }) as HTMLInputElement).checked).toBe(
    true,
  );
  expect((screen.getByRole("slider", { name: thresholdTitle }) as HTMLInputElement).value).toBe(
    "800",
  );
  expect((screen.getByRole("slider", { name: gapTitle }) as HTMLInputElement).value).toBe("15");
});
