// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const base: Snapshot = {
  format_version: 1,
  revision: 9,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("inline preedit defaults to the raw keys the shared default declares", async () => {
  render(<SettingsPage client={{ load: async () => base, save: vi.fn() }} />);
  // A snapshot without the field must render PreeditStyle::default() == Raw.
  const select = (await screen.findByLabelText("行内预编辑")) as HTMLSelectElement;
  expect(select.value).toBe("raw");
  expect(Array.from(select.options).map((option) => option.value)).toEqual([
    "raw",
    "pinyin",
    "empty",
  ]);
});

test("inline preedit saves without disturbing the candidate-window preedit", async () => {
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...base, revision: 10, preferences }));
  render(<SettingsPage client={{ load: async () => base, save }} />);
  await settingsFormReady();
  fireEvent.change(screen.getByLabelText("行内预编辑"), { target: { value: "pinyin" } });
  saveSettingsNow();
  await vi.waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls[0][1];
  expect(saved.tsf_preedit_style).toBe("pinyin");
  // The sibling control is a separate preference and must not be written.
  expect(saved.candidate_preedit_style).toBeUndefined();
});

test("a stored inline preedit value is shown rather than the default", async () => {
  const stored: Snapshot = {
    ...base,
    preferences: { ...base.preferences, tsf_preedit_style: "empty" },
  };
  render(<SettingsPage client={{ load: async () => stored, save: vi.fn() }} />);
  const select = (await screen.findByLabelText("行内预编辑")) as HTMLSelectElement;
  expect(select.value).toBe("empty");
});
