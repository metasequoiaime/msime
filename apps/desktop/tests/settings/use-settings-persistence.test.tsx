// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { Preferences, Snapshot, SettingsClient } from "@msime/ui";
import {
  SETTINGS_SYNC_NOTICE_MS,
  useSettingsPersistence,
  useTransientSyncNotice,
} from "../../../../packages/ui/src/settings/use-settings-persistence";

afterEach(() => {
  vi.restoreAllMocks();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("does not start a second settings load while reload is in flight", async () => {
  let hidden = false;
  vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
  const initialLoad = deferred<Snapshot>();
  const reloadLoad = deferred<Snapshot>();
  const client: SettingsClient = {
    load: vi
      .fn<SettingsClient["load"]>()
      .mockReturnValueOnce(initialLoad.promise)
      .mockReturnValue(reloadLoad.promise),
    save: vi.fn(),
  };

  const { result } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    return useSettingsPersistence({
      client,
      mobile: true,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
    });
  });

  await act(async () => {
    initialLoad.resolve(snapshot);
    await Promise.resolve();
  });
  expect(client.load).toHaveBeenCalledOnce();

  const manualReload = result.current.reload();
  expect(client.load).toHaveBeenCalledTimes(2);
  act(() => {
    hidden = true;
    document.dispatchEvent(new Event("visibilitychange"));
    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
  });

  await waitFor(() => expect(client.load).toHaveBeenCalledTimes(2));
  reloadLoad.resolve(snapshot);
  await manualReload;
});

test("clears the previous settings while a replacement client is loading", async () => {
  const oldLoad = deferred<Snapshot>();
  const nextLoad = deferred<Snapshot>();
  const oldClient: SettingsClient = {
    load: vi.fn(() => oldLoad.promise),
    save: vi.fn(),
  };
  const nextClient: SettingsClient = {
    load: vi.fn(() => nextLoad.promise),
    save: vi.fn(),
  };
  let activeClient = oldClient;
  const setBusy = vi.fn();

  const { result, rerender } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    return useSettingsPersistence({
      client: activeClient,
      mobile: false,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy,
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
    });
  });

  await act(async () => {
    oldLoad.resolve(snapshot);
    await Promise.resolve();
  });
  expect(result.current.snapshotRef.current).toEqual(snapshot);
  expect(result.current.draftRef.current).toEqual(snapshot.preferences);

  activeClient = nextClient;
  rerender();

  expect(result.current.snapshotRef.current).toBeUndefined();
  expect(result.current.draftRef.current).toBeUndefined();
  expect(setBusy).toHaveBeenLastCalledWith(true);

  await act(async () => {
    nextLoad.resolve({
      ...snapshot,
      revision: 2,
      preferences: { ...snapshot.preferences, scheme: "wubi" },
    });
    await Promise.resolve();
  });
  expect(result.current.snapshotRef.current?.revision).toBe(2);
});

test("does not let a save from the previous client restore stale settings", async () => {
  const oldLoad = deferred<Snapshot>();
  const oldSave = deferred<Snapshot>();
  const nextLoad = deferred<Snapshot>();
  const oldClient: SettingsClient = {
    load: vi.fn(() => oldLoad.promise),
    save: vi.fn(() => oldSave.promise),
  };
  const nextClient: SettingsClient = {
    load: vi.fn(() => nextLoad.promise),
    save: vi.fn(),
  };
  let activeClient = oldClient;
  let edit!: (value: Preferences) => void;

  const { result, rerender } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    edit = (value) => setDraft(value);
    return useSettingsPersistence({
      client: activeClient,
      mobile: false,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
    });
  });

  await act(async () => {
    oldLoad.resolve(snapshot);
    await Promise.resolve();
  });
  act(() => edit({ ...snapshot.preferences, scheme: "wubi" }));
  const flush = result.current.flush();
  await waitFor(() => expect(oldClient.save).toHaveBeenCalledOnce());

  activeClient = nextClient;
  rerender();
  expect(result.current.snapshotRef.current).toBeUndefined();

  await act(async () => {
    oldSave.resolve({
      ...snapshot,
      revision: 2,
      preferences: { ...snapshot.preferences, scheme: "wubi" },
    });
    await flush;
  });
  expect(result.current.snapshotRef.current).toBeUndefined();
  expect(result.current.draftRef.current).toBeUndefined();

  nextLoad.resolve({
    ...snapshot,
    revision: 3,
    preferences: { ...snapshot.preferences, scheme: "japanese" },
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(result.current.snapshotRef.current?.revision).toBe(3);
});

test("does not let a pending save from the previous client block the replacement client", async () => {
  const oldLoad = deferred<Snapshot>();
  const oldSave = deferred<Snapshot>();
  const nextLoad = deferred<Snapshot>();
  const nextSaved = {
    ...snapshot,
    revision: 3,
    preferences: { ...snapshot.preferences, scheme: "japanese" as const },
  };
  const oldClient: SettingsClient = {
    load: vi.fn(() => oldLoad.promise),
    save: vi.fn(() => oldSave.promise),
  };
  const nextClient: SettingsClient = {
    load: vi.fn(() => nextLoad.promise),
    save: vi.fn().mockResolvedValue(nextSaved),
  };
  let activeClient = oldClient;
  let edit!: (value: Preferences) => void;

  const { result, rerender } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    edit = (value) => setDraft(value);
    return useSettingsPersistence({
      client: activeClient,
      mobile: false,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
    });
  });

  await act(async () => {
    oldLoad.resolve(snapshot);
    await Promise.resolve();
  });
  act(() => edit({ ...snapshot.preferences, scheme: "wubi" }));
  const oldFlush = result.current.flush();
  await waitFor(() => expect(oldClient.save).toHaveBeenCalledOnce());

  activeClient = nextClient;
  rerender();
  nextLoad.resolve({
    ...snapshot,
    revision: 2,
    preferences: { ...snapshot.preferences, scheme: "quanpin" },
  });
  await act(async () => {
    await Promise.resolve();
  });
  act(() => edit({ ...snapshot.preferences, scheme: "japanese" }));
  await result.current.flush();
  expect(nextClient.save).toHaveBeenCalledOnce();

  oldSave.resolve({
    ...snapshot,
    revision: 2,
    preferences: { ...snapshot.preferences, scheme: "wubi" },
  });
  await oldFlush;
});

test("retries an autosave that races with another window during unmount", async () => {
  const latest: Snapshot = {
    ...snapshot,
    revision: 2,
    preferences: { ...snapshot.preferences, learning: false },
  };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(snapshot).mockResolvedValueOnce(latest),
    save: vi
      .fn()
      .mockRejectedValueOnce({ code: "conflict" })
      .mockImplementation(async (revision, preferences) => ({
        ...latest,
        revision: revision + 1,
        preferences,
      })),
  };
  let edit!: (value: Preferences) => void;

  const { unmount } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    edit = (value) => setDraft(value);
    return useSettingsPersistence({
      client,
      mobile: false,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
    });
  });

  await act(async () => {
    await Promise.resolve();
  });
  act(() => edit({ ...snapshot.preferences, scheme: "wubi" }));
  unmount();

  await waitFor(() => expect(client.save).toHaveBeenCalledTimes(2));
  expect(client.load).toHaveBeenCalledTimes(2);
  expect(client.save).toHaveBeenLastCalledWith(2, {
    ...latest.preferences,
    scheme: "wubi",
  });
});

test("其他窗口改了设置的提示过一会儿自动收起，别的提示留着", () => {
  vi.useFakeTimers();
  try {
    const { result } = renderHook(() => {
      const [notice, setNotice] = useState("设置已从其他窗口更新。");
      useTransientSyncNotice(notice, setNotice);
      return { notice, setNotice };
    });
    act(() => vi.advanceTimersByTime(SETTINGS_SYNC_NOTICE_MS - 1));
    expect(result.current.notice).toBe("设置已从其他窗口更新。");
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.notice).toBe("");

    act(() => result.current.setNotice("设置同时在其他窗口修改，已合并。"));
    act(() => vi.advanceTimersByTime(SETTINGS_SYNC_NOTICE_MS));
    expect(result.current.notice).toBe("");

    // 别的提示（比如修复配置文件之后的那句）不会被收起；同步提示收起前换成别的提示时也不会把它清掉。
    act(() => result.current.setNotice("设置已从其他窗口更新。"));
    act(() => result.current.setNotice("已用备份恢复设置。"));
    act(() => vi.advanceTimersByTime(SETTINGS_SYNC_NOTICE_MS * 2));
    expect(result.current.notice).toBe("已用备份恢复设置。");
  } finally {
    vi.useRealTimers();
  }
});
