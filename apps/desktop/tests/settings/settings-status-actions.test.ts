import { expect, test, vi } from "vitest";
import { createSettingsStatusActions } from "@msime/ui";

test("creates recovery and startup notice actions", async () => {
  const recoverPreferences = vi.fn().mockResolvedValue(undefined);
  const openSettings = vi.fn().mockResolvedValue(undefined);
  const dismissInputSourceStartup = vi.fn();
  const actions = createSettingsStatusActions({
    recoverPreferences,
    inputSourceStartup: { openSettings },
    dismissInputSourceStartup,
  });

  actions.onRecover();
  await actions.onOpenSettings();
  actions.onDismiss();

  expect(recoverPreferences).toHaveBeenCalledOnce();
  expect(openSettings).toHaveBeenCalledOnce();
  expect(dismissInputSourceStartup).toHaveBeenCalledOnce();
});

test("keeps the startup settings actions safe when unavailable", async () => {
  const actions = createSettingsStatusActions({
    recoverPreferences: vi.fn().mockResolvedValue(undefined),
    dismissInputSourceStartup: vi.fn(),
  });

  expect(actions.onOpenSettings()).toBeUndefined();
});
