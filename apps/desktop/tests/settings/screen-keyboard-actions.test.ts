import { expect, test, vi } from "vitest";
import { createScreenKeyboardActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
};

test("updates screen-keyboard preferences and editor state", () => {
  const setDraft = vi.fn();
  const setShowTouchSkinEditor = vi.fn();
  const actions = createScreenKeyboardActions({
    draft,
    openCommunity: vi.fn(),
    setShowTouchSkinEditor,
    saveMobileKeyboardFeedback: vi.fn().mockResolvedValue(undefined),
    resetTouchKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    openPanel: vi.fn().mockResolvedValue(undefined),
    setDraft,
  });

  actions.onScreenKeyboardThemeChange("dark");
  actions.onSkinSelect("custom");
  actions.onDesignChange({ rows: [] } as never);
  actions.onUseDesign();
  actions.onToggleEditor();
  actions.onCloseEditor();

  expect(setDraft).toHaveBeenCalledWith(expect.objectContaining({ screen_keyboard_theme: "dark" }));
  expect(setDraft).toHaveBeenCalledWith(expect.objectContaining({ touch_keyboard_skin: "custom" }));
  expect(setShowTouchSkinEditor).toHaveBeenCalledTimes(2);
});

test("saves tablet keyboard preference and opens the native panel", () => {
  const save = vi.fn().mockResolvedValue(undefined);
  const openPanel = vi.fn().mockResolvedValue(undefined);
  const feedback = {
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "light" as const,
    tabletFullKeys: false,
  };
  const openScreenKeyboard = vi.fn().mockResolvedValue(undefined);
  const actions = createScreenKeyboardActions({
    draft,
    openCommunity: vi.fn(),
    setShowTouchSkinEditor: vi.fn(),
    saveMobileKeyboardFeedback: save,
    mobileKeyboardFeedback: feedback,
    resetTouchKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    openScreenKeyboard,
    openPanel,
    setDraft: vi.fn(),
  });

  actions.onTabletFullKeysChange(true);
  actions.openScreenKeyboard?.();

  expect(save).toHaveBeenCalledWith({ ...feedback, tabletFullKeys: true });
  expect(openPanel).toHaveBeenCalledWith(openScreenKeyboard);
});
