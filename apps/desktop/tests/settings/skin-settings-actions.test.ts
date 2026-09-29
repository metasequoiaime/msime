import { expect, test, vi } from "vitest";
import { createSkinSettingsActions, type Preferences } from "@msime/ui";

function draft(): Preferences {
  return {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    candidate_skin: "willow_green",
  };
}

test("creates skin selection and preview actions", () => {
  const setDraft = vi.fn();
  const setSkinPreviewThemes = vi.fn();
  const actions = createSkinSettingsActions({
    draft: draft(),
    candidatePreviewTheme: "dark",
    saveMobileKeyboardFeedback: vi.fn().mockResolvedValue(undefined),
    setDraft,
    setSkinPreviewThemes,
  });

  actions.onSelect("mist_blue");
  actions.onTogglePreview("willow_green");

  expect(setDraft).toHaveBeenCalledWith(expect.objectContaining({ candidate_skin: "mist_blue" }));
  expect(setSkinPreviewThemes).toHaveBeenCalledOnce();
});

test("saves the mobile candidate palette preference when available", () => {
  const save = vi.fn().mockResolvedValue(undefined);
  const feedback = {
    soundEnabled: true,
    hapticsEnabled: true,
    hapticStrength: "medium" as const,
    candidatePaletteFollowsDesktop: false,
  };
  const actions = createSkinSettingsActions({
    draft: draft(),
    candidatePreviewTheme: "light",
    mobileKeyboardFeedback: feedback,
    saveMobileKeyboardFeedback: save,
    setDraft: vi.fn(),
    setSkinPreviewThemes: vi.fn(),
  });

  actions.onCandidatePaletteChange(true);
  expect(save).toHaveBeenCalledWith({ ...feedback, candidatePaletteFollowsDesktop: true });
});
