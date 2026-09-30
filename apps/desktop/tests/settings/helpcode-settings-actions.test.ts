import { expect, test, vi } from "vitest";
import { createHelpcodeSettingsActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
};

test("merges helper-code patches into the current draft", () => {
  const setDraft = vi.fn();
  const actions = createHelpcodeSettingsActions({ setDraft });

  actions.onChange({ quanpin_helpcode: { enabled: false, schema: "ziranma" } });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft)).toMatchObject({
    quanpin_helpcode: { enabled: false, schema: "ziranma" },
    scheme: "quanpin",
  });
});

test("merges helper-code patches into the latest draft", () => {
  const setDraft = vi.fn();
  const actions = createHelpcodeSettingsActions({ setDraft });

  actions.onChange({ shuangpin_helpcode: { enabled: true, schema: "lantian" } });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater({ ...draft, candidate_page_size: 9 })).toMatchObject({
    candidate_page_size: 9,
    shuangpin_helpcode: { enabled: true, schema: "lantian" },
  });
});
