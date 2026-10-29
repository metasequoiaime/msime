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

const title = "大屏单手时侧边显示数字键盘";

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
  /** 宿主报告 `touch_one_handed_side_panel`；鸿蒙的设置桥只在平板和可折叠手机上报告。 */
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
          ...(capability === undefined ? {} : { touch_one_handed_side_panel: capability }),
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
  "a Harmony host that does not report the side pad (%s) lists no switch for it",
  async (capability) => {
    renderSettings({ capability });
    await settingsFormReady();
    await screen.findByText("键盘高度");
    expect(screen.queryByRole("switch", { name: title })).toBeNull();
  },
);

test("a Harmony tablet turns the side number pad on and off through the shared preference", async () => {
  const { save } = renderSettings({ capability: true });
  await settingsFormReady();
  const pad = (await screen.findByRole("switch", { name: title })) as HTMLInputElement;
  // 缺键即空着。
  expect(pad.checked).toBe(false);

  fireEvent.click(pad);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].touch_one_handed_side_panel).toBe("number_pad");
  expect((screen.getByRole("switch", { name: title }) as HTMLInputElement).checked).toBe(true);

  fireEvent.click(screen.getByRole("switch", { name: title }));
  saveSettingsNow();
  await waitFor(() => expect(save.mock.calls.at(-1)?.[1].touch_one_handed_side_panel).toBe("off"));
});

test("a stored panel this build does not know reads as off", async () => {
  renderSettings({ capability: true, preferences: { touch_one_handed_side_panel: "text_edit" } });
  await settingsFormReady();
  const pad = (await screen.findByRole("switch", { name: title })) as HTMLInputElement;
  expect(pad.checked).toBe(false);
});
