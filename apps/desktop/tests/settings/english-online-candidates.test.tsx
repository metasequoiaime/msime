// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Preferences, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";

afterEach(cleanup);

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: false,
    chinese_punctuation: true,
  },
};

test("英文在线授权默认关闭，用户开启后才保存", async () => {
  const save = vi.fn(async (_revision: number, preferences: Preferences) => ({
    ...snapshot,
    revision: 2,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save,
        host: testHost({ platform: "macos", english_online_candidates: true }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(AI|AI 辅助)$/ }));
  const toggle = screen.getByRole("switch", { name: "英文模式也请求 AI / 云候选" });
  expect((toggle as HTMLInputElement).checked).toBe(false);
  expect(screen.getByText(/当前字母串与前文发送给已启用的 AI/)).toBeTruthy();
  fireEvent.click(toggle);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].english_online_candidates).toBe(true);
});

test.each(["linux", "harmony", "windows", "ios", "android"])(
  "%s 未接通英文在线输入时不显示开关",
  async (platform) => {
    render(
      <SettingsPage
        client={{
          load: async () => snapshot,
          save: async () => snapshot,
          host: testHost({ platform }),
        }}
      />,
    );
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: /^(AI|AI 辅助)$/ }));
    expect(screen.queryByRole("switch", { name: "英文模式也请求 AI / 云候选" })).toBeNull();
  },
);
