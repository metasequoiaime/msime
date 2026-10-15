// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  usePreferenceRecovery,
  type Preferences,
  type PreferencesRecovery,
  type Snapshot,
} from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

const preferences = {} as Preferences;
const snapshot = { revision: 1, preferences } as Snapshot;

test("ignores a same-tick duplicate restore-defaults action", async () => {
  const pending = deferred<Preferences>();
  const loadDefaultPreferences = vi.fn().mockReturnValue(pending.promise);
  const client = { loadDefaultPreferences };
  const { result } = renderHook(() =>
    usePreferenceRecovery({
      client,
      busy: false,
      snapshotRef: { current: snapshot },
      draftRef: { current: preferences },
      setSnapshot: vi.fn(),
      setDraft: vi.fn(),
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
      confirm: vi.fn().mockResolvedValue(true),
    }),
  );

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.restoreDefaults();
    second = result.current.restoreDefaults();
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(loadDefaultPreferences).toHaveBeenCalledOnce();
  pending.resolve(preferences);
  await act(async () => {
    await first;
    await second;
  });
});

test("ignores a late recovery result after the settings client changes", async () => {
  const pending = deferred<PreferencesRecovery>();
  const recoverPreferences = vi.fn().mockReturnValue(pending.promise);
  const setBusy = vi.fn();
  const setSnapshot = vi.fn();
  const clientA = { recoverPreferences };
  const clientB = { recoverPreferences: vi.fn() };
  const options = (client: typeof clientA) => ({
    client,
    busy: false,
    snapshotRef: { current: snapshot },
    draftRef: { current: preferences },
    setSnapshot,
    setDraft: vi.fn(),
    setBusy,
    setError: vi.fn(),
    setNotice: vi.fn(),
    setRecoveredBackup: vi.fn(),
    confirm: vi.fn().mockResolvedValue(true),
  });
  const { result, rerender } = renderHook(
    ({ client }: { client: typeof clientA }) => usePreferenceRecovery(options(client)),
    { initialProps: { client: clientA } },
  );

  let recovery!: Promise<void>;
  act(() => {
    recovery = result.current.recoverPreferences();
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(recoverPreferences).toHaveBeenCalledOnce();

  rerender({ client: clientB });
  pending.resolve({
    snapshot: { ...snapshot, revision: 2 },
    backupPath: "/synthetic/preferences.backup",
    salvaged: true,
  });
  await act(async () => {
    await recovery;
  });

  expect(setSnapshot).not.toHaveBeenCalled();
  expect(setBusy).toHaveBeenCalledWith(true);
  expect(setBusy).not.toHaveBeenCalledWith(false);
});

test("ignores a late restore-defaults result after the settings client changes", async () => {
  const pending = deferred<Preferences>();
  const loadDefaultPreferences = vi.fn().mockReturnValue(pending.promise);
  const setDraft = vi.fn();
  const clientA = { loadDefaultPreferences };
  const clientB = { loadDefaultPreferences: vi.fn() };
  const options = (client: typeof clientA) => ({
    client,
    busy: false,
    snapshotRef: { current: snapshot },
    draftRef: { current: preferences },
    setSnapshot: vi.fn(),
    setDraft,
    setBusy: vi.fn(),
    setError: vi.fn(),
    setNotice: vi.fn(),
    setRecoveredBackup: vi.fn(),
    confirm: vi.fn().mockResolvedValue(true),
  });
  const { result, rerender } = renderHook(
    ({ client }: { client: typeof clientA }) => usePreferenceRecovery(options(client)),
    { initialProps: { client: clientA } },
  );

  let restore!: Promise<void>;
  act(() => {
    restore = result.current.restoreDefaults();
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(loadDefaultPreferences).toHaveBeenCalledOnce();

  rerender({ client: clientB });
  pending.resolve(preferences);
  await act(async () => {
    await restore;
  });

  expect(setDraft).not.toHaveBeenCalled();
});
