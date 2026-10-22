// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";
import { selectSettingsTab, saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { testHost } from "../support/host";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function mount(client: Partial<SettingsClient>) {
  return render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), ...client }} />);
}

test("a macOS host edits the Vim mode list in the 中英文 group and saves it to the shared preferences", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  mount({ host: testHost({ platform: "macos", vim_mode_apps: true }), save });
  await settingsFormReady();
  selectSettingsTab("中英文");
  const group = await screen.findByRole("group", { name: "Vim 模式" });
  expect(group.textContent).toContain("名单是空的");

  const input = screen.getByLabelText("应用 Bundle ID");
  fireEvent.change(input, { target: { value: "not an app" } });
  // macOS 的 bundle id 保留大小写、只去掉两侧空白；带空格的标识被校验拒绝。
  fireEvent.change(input, { target: { value: " com.apple.Terminal " } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(await screen.findByText("com.apple.Terminal")).toBeTruthy();

  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(3, {
    ...initial.preferences,
    vim_mode_apps: ["com.apple.Terminal"],
  });

  fireEvent.click(
    screen.getByRole("button", { name: "把 com.apple.Terminal 从 Vim 模式名单移除" }),
  );
  saveSettingsNow();
  await vi.waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(4, { ...initial.preferences, vim_mode_apps: [] }),
  );
});

test("the Vim mode list is hidden on hosts without the capability", async () => {
  mount({ host: testHost({ platform: "windows", app_input_mode_rules: true }), save: vi.fn() });
  await settingsFormReady();
  selectSettingsTab("中英文");
  expect(await screen.findByRole("group", { name: "应用例外" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "Vim 模式" })).toBeNull();
});
