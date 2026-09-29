import { expect, test, vi } from "vitest";
import { createAboutSettingsActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
  diagnostic_log: { server: false, tsf: false },
  telemetry_enabled: false,
};

test("creates about maintenance and preference actions", () => {
  const setDraft = vi.fn();
  const selectPage = vi.fn();
  const actions = createAboutSettingsActions({
    draft,
    diagnosticLog: { server: false, tsf: false },
    checkForUpdate: vi.fn().mockResolvedValue(undefined),
    chooseDataDirectory: vi.fn().mockResolvedValue(undefined),
    confirmUninstall: vi.fn().mockResolvedValue(undefined),
    selectPage,
    setDraft,
  });

  actions.onDiagnosticLogChange({ server: true });
  actions.onTelemetryChange(true);
  actions.onHelp();
  actions.onFeedback();

  expect(setDraft).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({ diagnostic_log: { server: true, tsf: false } }),
  );
  expect(setDraft).toHaveBeenNthCalledWith(2, expect.objectContaining({ telemetry_enabled: true }));
  expect(selectPage).toHaveBeenNthCalledWith(1, "help");
  expect(selectPage).toHaveBeenNthCalledWith(2, "feedback");
});

test("wraps async maintenance operations", () => {
  const checkForUpdate = vi.fn().mockResolvedValue(undefined);
  const chooseDataDirectory = vi.fn().mockResolvedValue(undefined);
  const confirmUninstall = vi.fn().mockResolvedValue(undefined);
  const actions = createAboutSettingsActions({
    diagnosticLog: { server: false, tsf: false },
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage: vi.fn(),
    setDraft: vi.fn(),
  });

  actions.onCheckForUpdate();
  actions.onChooseDataDirectory();
  actions.onConfirmUninstall();

  expect(checkForUpdate).toHaveBeenCalledOnce();
  expect(chooseDataDirectory).toHaveBeenCalledOnce();
  expect(confirmUninstall).toHaveBeenCalledOnce();
});
