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

  const diagnosticUpdater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const telemetryUpdater = setDraft.mock.calls[1][0] as (value: Preferences) => Preferences;
  expect(diagnosticUpdater(draft)).toMatchObject({ diagnostic_log: { server: true, tsf: false } });
  expect(telemetryUpdater(draft)).toMatchObject({ telemetry_enabled: true });
  expect(selectPage).toHaveBeenNthCalledWith(1, "help");
  expect(selectPage).toHaveBeenNthCalledWith(2, "feedback");
});

test("applies diagnostic updates to the latest draft", () => {
  const setDraft = vi.fn();
  const actions = createAboutSettingsActions({
    checkForUpdate: vi.fn().mockResolvedValue(undefined),
    chooseDataDirectory: vi.fn().mockResolvedValue(undefined),
    confirmUninstall: vi.fn().mockResolvedValue(undefined),
    selectPage: vi.fn(),
    setDraft,
  });

  actions.onDiagnosticLogChange({ server: true });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater({ ...draft, candidate_page_size: 9 })).toMatchObject({
    candidate_page_size: 9,
    diagnostic_log: { server: true, tsf: false },
  });
});

test("wraps async maintenance operations", () => {
  const checkForUpdate = vi.fn().mockResolvedValue(undefined);
  const chooseDataDirectory = vi.fn().mockResolvedValue(undefined);
  const confirmUninstall = vi.fn().mockResolvedValue(undefined);
  const actions = createAboutSettingsActions({
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
