import { expect, test, vi } from "vitest";
import { createSettingsNavigationActions } from "@msime/ui";

test("creates available navigation and native actions", () => {
  const selectPage = vi.fn();
  const openPanel = vi.fn().mockResolvedValue(undefined);
  const openHandwriting = vi.fn().mockResolvedValue(undefined);
  const restoreDefaults = vi.fn().mockResolvedValue(undefined);
  const actions = createSettingsNavigationActions({
    selectPage,
    chatAvailable: true,
    openPanel,
    openHandwriting,
    restoreDefaults,
  });

  actions.onOpenChat?.();
  actions.onOpenAi();
  actions.onOpenHandwriting?.();
  actions.onRestoreDefaults();

  expect(selectPage).toHaveBeenNthCalledWith(1, "chat");
  expect(selectPage).toHaveBeenNthCalledWith(2, "ai");
  expect(openPanel).toHaveBeenCalledWith(openHandwriting);
  expect(restoreDefaults).toHaveBeenCalledOnce();
});

test("hides unavailable chat while retaining the handwriting error callback", () => {
  const openPanel = vi.fn().mockResolvedValue(undefined);
  const actions = createSettingsNavigationActions({
    selectPage: vi.fn(),
    chatAvailable: false,
    openPanel,
    restoreDefaults: vi.fn().mockResolvedValue(undefined),
  });

  expect(actions.onOpenChat).toBeUndefined();
  actions.onOpenHandwriting();
  expect(openPanel).toHaveBeenCalledWith(undefined);
});
