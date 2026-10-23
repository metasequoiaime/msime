// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { Preferences, SettingsClient, Snapshot } from "@msime/ui";
import { useMountedRef } from "../../../../packages/ui/src/settings/use-mounted-ref";
import { useSettingsPersistence } from "../../../../packages/ui/src/settings/use-settings-persistence";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const initial: Snapshot = {
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
const refreshed: Snapshot = {
  ...initial,
  revision: 10,
  preferences: { ...initial.preferences, learning: false, candidate_page_size: 7 },
};
const oldDraft: Preferences = { ...initial.preferences, scheme: "wubi" };
const oldSaved: Snapshot = { ...initial, revision: 2, preferences: oldDraft };

function saveSnapshot(revision: number, preferences: Preferences): Promise<Snapshot> {
  return Promise.resolve({ ...initial, revision: revision + 1, preferences });
}

function mountPersistence(client: SettingsClient) {
  const notice = vi.fn();
  const { result, rerender, unmount } = renderHook(
    ({ client }: { client: SettingsClient }) => {
      const mounted = useMountedRef();
      const [snapshot, setSnapshot] = useState<Snapshot>();
      const [draft, setDraft] = useState<Preferences>();
      return {
        ...useSettingsPersistence({
          client,
          mobile: false,
          mounted,
          snapshot,
          draft,
          setSnapshot,
          setDraft,
          setBusy: vi.fn(),
          setError: vi.fn(),
          setNotice: notice,
          setRecoveredBackup: vi.fn(),
        }),
        edit: setDraft,
      };
    },
    { initialProps: { client } },
  );
  return {
    result,
    notice,
    unmount,
    replace: async (client: SettingsClient) => {
      await act(async () => rerender({ client }));
    },
    edit: (preferences: Preferences) => act(() => result.current.edit(preferences)),
    flush: () => {
      let operation!: Promise<void>;
      act(() => {
        operation = result.current.flush();
      });
      return operation;
    },
  };
}

test("新保存取消旧的已保存提示倒计时，并从最新成功响应重新计时", async () => {
  vi.useFakeTimers();
  const pending = deferred<Snapshot>();
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockResolvedValueOnce(oldSaved).mockReturnValueOnce(pending.promise),
  };
  const hook = mountPersistence(client);
  await act(async () => {});
  hook.edit(oldDraft);
  await act(async () => hook.flush());
  expect(hook.result.current.saveState).toBe("saved");
  act(() => vi.advanceTimersByTime(1000));

  const nextDraft = { ...oldDraft, learning: false };
  hook.edit(nextDraft);
  const saving = hook.flush();
  act(() => vi.advanceTimersByTime(1000));
  expect(hook.result.current.saveState).toBe("saving");
  await act(async () => {
    pending.resolve({ ...oldSaved, revision: 3, preferences: nextDraft });
    await saving;
  });
  act(() => vi.advanceTimersByTime(1999));
  expect(hook.result.current.saveState).toBe("saved");
  act(() => vi.advanceTimersByTime(1));
  expect(hook.result.current.saveState).toBe("idle");
});

test.each(["success", "failure", "conflict"] as const)(
  "ignores a stale save %s after switching away from and back to the same client",
  async (outcome) => {
    vi.useFakeTimers();
    const pending = deferred<Snapshot>();
    const clientA: SettingsClient = {
      load: vi.fn().mockResolvedValueOnce(initial).mockResolvedValue(refreshed),
      save: vi.fn().mockReturnValueOnce(pending.promise).mockImplementation(saveSnapshot),
    };
    const clientB: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save: vi.fn() };
    const hook = mountPersistence(clientA);
    await act(async () => {});
    hook.edit(oldDraft);
    const saving = hook.flush();
    expect(clientA.save).toHaveBeenCalledOnce();
    await hook.replace(clientB);
    await hook.replace(clientA);
    expect(hook.result.current.snapshotRef.current).toEqual(refreshed);

    await act(async () => {
      if (outcome === "success") pending.resolve(oldSaved);
      else pending.reject({ code: outcome === "conflict" ? "conflict" : "synthetic_failure" });
      await saving;
    });

    expect(clientA.save).toHaveBeenCalledOnce();
    expect(clientA.load).toHaveBeenCalledTimes(2);
    expect(hook.result.current.snapshotRef.current).toEqual(refreshed);
    expect(hook.result.current.draftRef.current).toEqual(refreshed.preferences);
    expect(hook.result.current.saveState).toBe("idle");
    expect(hook.result.current.saveError).toBe("");
    expect(hook.notice).not.toHaveBeenCalled();
  },
);

test("ignores stale conflict recovery after reusing the same client", async () => {
  vi.useFakeTimers();
  const recovery = deferred<Snapshot>();
  const clientA: SettingsClient = {
    load: vi
      .fn()
      .mockResolvedValueOnce(initial)
      .mockReturnValueOnce(recovery.promise)
      .mockResolvedValue(refreshed),
    save: vi.fn().mockRejectedValueOnce({ code: "conflict" }).mockImplementation(saveSnapshot),
  };
  const clientB: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save: vi.fn() };
  const hook = mountPersistence(clientA);
  await act(async () => {});
  hook.edit(oldDraft);
  const saving = hook.flush();
  await act(async () => {});
  expect(clientA.load).toHaveBeenCalledTimes(2);
  await hook.replace(clientB);
  await hook.replace(clientA);
  await act(async () => {
    recovery.resolve({
      ...initial,
      revision: 4,
      preferences: { ...initial.preferences, scheme: "japanese" },
    });
    await saving;
  });

  expect(hook.result.current.snapshotRef.current).toEqual(refreshed);
  expect(hook.result.current.draftRef.current).toEqual(refreshed.preferences);
  expect(hook.result.current.saveState).toBe("idle");
  expect(clientA.save).toHaveBeenCalledOnce();
});

test("a stale save cannot consume a reused client's pending notification or save status", async () => {
  vi.useFakeTimers();
  const oldRequest = deferred<Snapshot>();
  const newRequest = deferred<Snapshot>();
  let changed!: (value: Snapshot) => void;
  const clientA: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(initial).mockResolvedValue(refreshed),
    save: vi
      .fn()
      .mockReturnValueOnce(oldRequest.promise)
      .mockReturnValueOnce(newRequest.promise)
      .mockImplementation(saveSnapshot),
    onPreferencesChanged: async (listener) => {
      changed = listener;
      return () => {};
    },
  };
  const clientB: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save: vi.fn() };
  const hook = mountPersistence(clientA);
  await act(async () => {});
  hook.edit(oldDraft);
  const oldSave = hook.flush();
  await hook.replace(clientB);
  await hook.replace(clientA);
  const newDraft = { ...refreshed.preferences, candidate_page_size: 9 };
  hook.edit(newDraft);
  const newSave = hook.flush();
  const notification: Snapshot = {
    ...refreshed,
    revision: 12,
    preferences: { ...newDraft, scheme: "japanese" },
  };
  act(() => changed(notification));
  await act(async () => {
    oldRequest.resolve(oldSaved);
    await oldSave;
  });
  const duringNewSave = {
    snapshot: hook.result.current.snapshotRef.current,
    draft: hook.result.current.draftRef.current,
    state: hook.result.current.saveState,
    saves: vi.mocked(clientA.save).mock.calls.length,
    notices: hook.notice.mock.calls.length,
  };
  await act(async () => {
    newRequest.resolve({ ...refreshed, revision: 11, preferences: newDraft });
    await newSave;
  });

  expect(duringNewSave).toEqual({
    snapshot: refreshed,
    draft: newDraft,
    state: "saving",
    saves: 2,
    notices: 0,
  });
  expect(hook.result.current.snapshotRef.current).toEqual(notification);
  expect(hook.result.current.draftRef.current).toEqual(notification.preferences);
});

test("an old queued flush cannot start a save for a later use of the same client", async () => {
  vi.useFakeTimers();
  const pending = deferred<Snapshot>();
  const clientA: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(initial).mockResolvedValue(refreshed),
    save: vi.fn().mockReturnValueOnce(pending.promise).mockImplementation(saveSnapshot),
  };
  const clientB: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save: vi.fn() };
  const hook = mountPersistence(clientA);
  await act(async () => {});
  hook.edit(oldDraft);
  const saving = hook.flush();
  const queued = hook.flush();
  await hook.replace(clientB);
  await hook.replace(clientA);
  const newDraft = { ...refreshed.preferences, candidate_page_size: 9 };
  hook.edit(newDraft);
  await act(async () => {
    pending.reject({ code: "synthetic_failure" });
    await saving;
    await queued;
  });

  expect(clientA.save).toHaveBeenCalledOnce();
  expect(hook.result.current.snapshotRef.current).toEqual(refreshed);
  expect(hook.result.current.draftRef.current).toEqual(newDraft);
  await act(async () => vi.advanceTimersByTimeAsync(400));
  expect(clientA.save).toHaveBeenCalledTimes(2);
  expect(clientA.save).toHaveBeenLastCalledWith(10, newDraft);
});

test("ordinary unmount still writes edits made during an in-flight save", async () => {
  vi.useFakeTimers();
  const pending = deferred<Snapshot>();
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockReturnValueOnce(pending.promise).mockImplementation(saveSnapshot),
  };
  const hook = mountPersistence(client);
  await act(async () => {});
  hook.edit(oldDraft);
  const saving = hook.flush();
  const latestDraft = { ...oldDraft, candidate_page_size: 9 };
  hook.edit(latestDraft);
  hook.unmount();
  await act(async () => {
    pending.resolve(oldSaved);
    await saving;
  });

  expect(client.save).toHaveBeenCalledTimes(2);
  expect(client.save).toHaveBeenLastCalledWith(2, latestDraft);
});
