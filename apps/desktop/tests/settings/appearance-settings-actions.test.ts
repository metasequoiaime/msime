import { expect, test, vi } from "vitest";
import { createAppearanceSettingsActions, type Preferences } from "@msime/ui";

function draft(): Preferences {
  return {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    candidate_follow_cursor: false,
  };
}

test("applies appearance preference patches to the current draft", () => {
  const setDraft = vi.fn();
  const actions = createAppearanceSettingsActions({
    draft: draft(),
    saveMobileKeyboardFeedback: vi.fn().mockResolvedValue(undefined),
    setDraft,
  });

  actions.onPreferencesChange({ candidate_follow_cursor: true });

  expect(setDraft).toHaveBeenCalledWith(expect.objectContaining({ candidate_follow_cursor: true }));
});

test("saves inline preedit through the mobile feedback client", () => {
  const save = vi.fn().mockResolvedValue(undefined);
  const feedback = {
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "light" as const,
    inlinePreedit: false,
  };
  const actions = createAppearanceSettingsActions({
    draft: draft(),
    mobileKeyboardFeedback: feedback,
    saveMobileKeyboardFeedback: save,
    setDraft: vi.fn(),
  });

  actions.onInlinePreeditChange(true);

  expect(save).toHaveBeenCalledWith({ ...feedback, inlinePreedit: true });
});
