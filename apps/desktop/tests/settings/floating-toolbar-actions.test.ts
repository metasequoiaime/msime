import { expect, test, vi } from "vitest";
import { createFloatingToolbarActions, type Preferences } from "@msime/ui";

function preferences(): Preferences {
  return {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    floating_toolbar: {
      enabled: false,
      english_mode: true,
      fullwidth: true,
      punctuation: true,
      character_set: true,
      emoji: true,
      handwriting: false,
      screen_keyboard: true,
      voice: false,
      settings: true,
      scale_percent: 100,
      font_size: 18,
    },
  };
}

test("floating toolbar actions update only the requested preference", () => {
  const draft = preferences();
  const setDraft = vi.fn();
  const actions = createFloatingToolbarActions({
    draft,
    preferences: draft.floating_toolbar!,
    setDraft,
  });

  actions.onEnabledChange(true);
  actions.onScaleChange(125);
  actions.onFontSizeChange(22);
  actions.onComponentChange("voice", true);

  expect(setDraft).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({ floating_toolbar: expect.objectContaining({ enabled: true }) }),
  );
  expect(setDraft).toHaveBeenNthCalledWith(
    2,
    expect.objectContaining({ floating_toolbar: expect.objectContaining({ scale_percent: 125 }) }),
  );
  expect(setDraft).toHaveBeenNthCalledWith(
    3,
    expect.objectContaining({ floating_toolbar: expect.objectContaining({ font_size: 22 }) }),
  );
  expect(setDraft).toHaveBeenNthCalledWith(
    4,
    expect.objectContaining({ floating_toolbar: expect.objectContaining({ voice: true }) }),
  );
});

test("does not update before preferences have loaded", () => {
  const setDraft = vi.fn();
  const actions = createFloatingToolbarActions({
    draft: undefined,
    preferences: preferences().floating_toolbar!,
    setDraft,
  });

  actions.onEnabledChange(true);
  expect(setDraft).not.toHaveBeenCalled();
});
