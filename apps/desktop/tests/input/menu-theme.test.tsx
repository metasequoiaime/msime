// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Preferences, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 4,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("the menu surface has its own theme override like the others", async () => {
  const save = vi.fn(async (_revision: number, _preferences: Preferences) => snapshot);
  render(<SettingsPage client={{ load: async () => snapshot, save }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));

  // Eight surfaces had an override and the menus did not, even though the
  // Windows host draws its own tray and candidate context menus.
  const select = await screen.findByLabelText("菜单主题");
  expect((select as HTMLSelectElement).value).toBe("follow");
  fireEvent.change(select, { target: { value: "light" } });

  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1].menu_theme).toBe("light");
});

test("every surface override offers the same three choices", async () => {
  render(<SettingsPage client={{ load: async () => snapshot, save: vi.fn() }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  const select = await screen.findByLabelText("菜单主题");
  const values = Array.from((select as HTMLSelectElement).options).map((option) => option.value);
  expect(values).toEqual(["follow", "dark", "light"]);
});
