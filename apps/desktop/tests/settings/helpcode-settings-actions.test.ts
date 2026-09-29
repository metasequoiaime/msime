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
  const actions = createHelpcodeSettingsActions({ draft, setDraft });

  actions.onChange({ quanpin_helpcode: { enabled: false, schema: "ziranma" } });

  expect(setDraft).toHaveBeenCalledWith(
    expect.objectContaining({
      quanpin_helpcode: { enabled: false, schema: "ziranma" },
      scheme: "quanpin",
    }),
  );
});

test("does not update before preferences have loaded", () => {
  const setDraft = vi.fn();
  const actions = createHelpcodeSettingsActions({ draft: undefined, setDraft });

  actions.onChange({ shuangpin_helpcode: { enabled: true, schema: "lantian" } });

  expect(setDraft).not.toHaveBeenCalled();
});
