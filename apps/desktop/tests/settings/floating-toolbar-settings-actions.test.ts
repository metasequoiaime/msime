import { expect, test, vi } from "vitest";
import { createFloatingToolbarSettingsActions, type Preferences } from "@msime/ui";

const draft = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
  floating_toolbar: { enabled: true, font_size: 18 },
} as unknown as Preferences;

test("updates floating toolbar preferences through a functional draft action", () => {
  const setDraft = vi.fn();
  const actions = createFloatingToolbarSettingsActions({ setDraft });

  actions.onChange({ enabled: false });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft).floating_toolbar).toMatchObject({ enabled: false, font_size: 18 });
});

test("preserves the latest floating toolbar draft", () => {
  const setDraft = vi.fn();
  const actions = createFloatingToolbarSettingsActions({ setDraft });

  actions.onChange({ font_size: 22 });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater({ ...draft, candidate_page_size: 9 })).toMatchObject({
    candidate_page_size: 9,
    floating_toolbar: { font_size: 22 },
  });
});
