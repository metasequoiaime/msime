import { expect, test, vi } from "vitest";
import { createSettingsStatusActions } from "@msime/ui";

test("creates recovery and startup notice actions", async () => {
  const recoverPreferences = vi.fn().mockResolvedValue(undefined);
  const openSettings = vi.fn().mockResolvedValue(undefined);
  const setInputSourceStartup = vi.fn();
  const actions = createSettingsStatusActions({
    recoverPreferences,
    inputSourceStartup: { openSettings },
    setInputSourceStartup,
  });

  actions.onRecover();
  await actions.onOpenSettings();
  actions.onDismiss();

  expect(recoverPreferences).toHaveBeenCalledOnce();
  expect(openSettings).toHaveBeenCalledOnce();
  expect(setInputSourceStartup).toHaveBeenCalledWith(null);
});

test("keeps the startup settings action safe when unavailable", async () => {
  const actions = createSettingsStatusActions({
    recoverPreferences: vi.fn().mockResolvedValue(undefined),
    setInputSourceStartup: vi.fn(),
  });

  expect(actions.onOpenSettings()).toBeUndefined();
});
