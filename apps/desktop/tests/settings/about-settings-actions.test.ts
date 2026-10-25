import { expect, test, vi } from "vitest";
import { createAboutSettingsActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
  diagnostic_log: { server: false, tsf: false },
  usage_reporting: true,
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
  actions.onTelemetryChange(false);
  actions.onHelp();
  actions.onFeedback();

  const diagnosticUpdater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const telemetryUpdater = setDraft.mock.calls[1][0] as (value: Preferences) => Preferences;
  expect(diagnosticUpdater(draft)).toMatchObject({ diagnostic_log: { server: true, tsf: false } });
  expect(telemetryUpdater(draft)).toMatchObject({ usage_reporting: false });
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

test("command line candidates and diagnostic logs share diagnostic_log without overwriting each other", () => {
  const setDraft = vi.fn();
  const actions = createAboutSettingsActions({
    checkForUpdate: vi.fn().mockResolvedValue(undefined),
    chooseDataDirectory: vi.fn().mockResolvedValue(undefined),
    confirmUninstall: vi.fn().mockResolvedValue(undefined),
    selectPage: vi.fn(),
    setDraft,
  });

  actions.onCommandLineCandidatesChange(true);
  actions.onDiagnosticLogChange({ tsf: true });

  const candidatesUpdater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const diagnosticUpdater = setDraft.mock.calls[1][0] as (value: Preferences) => Preferences;
  expect(
    candidatesUpdater({ ...draft, diagnostic_log: { server: true, tsf: false } }).diagnostic_log,
  ).toEqual({ server: true, tsf: false, command_line_candidates: true });
  // 开日志时不能把已经打开的命令行开关冲掉。
  expect(
    diagnosticUpdater({
      ...draft,
      diagnostic_log: { server: false, tsf: false, command_line_candidates: true },
    }).diagnostic_log,
  ).toEqual({ server: false, tsf: true, command_line_candidates: true });
  // 草稿里还没有 diagnostic_log 时，只写入这一项和两个日志开关的缺省值。
  const { diagnostic_log: _omitted, ...withoutDiagnostics } = draft;
  expect(candidatesUpdater(withoutDiagnostics).diagnostic_log).toEqual({
    server: false,
    tsf: false,
    command_line_candidates: true,
  });
  expect(candidatesUpdater(undefined as unknown as Preferences)).toBeUndefined();
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
