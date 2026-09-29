import { expect, test, vi } from "vitest";
import { createSettingsExternalActions } from "@msime/ui";

test("creates fixed documentation, system-settings, issue, and Telegram actions", () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  const actions = createSettingsExternalActions({
    mobile: true,
    canOpenExternalUrl: true,
    openExternalUrl,
    issuesUrl: "https://example.com/issues",
    openSystemKeyboardSettings,
  });

  actions.onOpenDocumentation?.();
  actions.onOpenSystemKeyboardSettings?.();
  actions.onOpenIssues();
  actions.onOpenTelegram();

  expect(openExternalUrl).toHaveBeenNthCalledWith(1, "https://msime.app/docs/");
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
  expect(openExternalUrl).toHaveBeenNthCalledWith(2, "https://example.com/issues");
  expect(openExternalUrl).toHaveBeenNthCalledWith(3, "https://t.me/msimegroup");
});

test("hides unavailable capability actions", () => {
  const actions = createSettingsExternalActions({
    mobile: false,
    canOpenExternalUrl: false,
    openExternalUrl: vi.fn().mockResolvedValue(undefined),
    issuesUrl: "https://example.com/issues",
  });

  expect(actions.onOpenDocumentation).toBeUndefined();
  expect(actions.onOpenSystemKeyboardSettings).toBeUndefined();
});
