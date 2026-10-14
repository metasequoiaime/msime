// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { Preferences, SettingsClient, Snapshot } from "@msime/ui";
import { useMountedRef } from "../../../../packages/ui/src/settings/use-mounted-ref";
import { useSettingsPersistence } from "../../../../packages/ui/src/settings/use-settings-persistence";

afterEach(() => vi.restoreAllMocks());

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
const replacement: Snapshot = {
  ...initial,
  revision: 2,
  preferences: { ...initial.preferences, scheme: "wubi" },
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function renderPersistence(client: SettingsClient) {
  return renderHook(
    ({ client }) => {
      const mounted = useMountedRef();
      const [snapshot, setSnapshot] = useState<Snapshot>();
      const [draft, setDraft] = useState<Preferences>();
      const [busy, setBusy] = useState(false);
      const [error, setError] = useState("");
      const persistence = useSettingsPersistence({
        client,
        mobile: true,
        mounted,
        snapshot,
        draft,
        setSnapshot,
        setDraft,
        setBusy,
        setError,
        setNotice: vi.fn(),
        setRecoveredBackup: vi.fn(),
      });
      return { ...persistence, busy, error };
    },
    { initialProps: { client } },
  );
}

test.each(["success", "failure"] as const)(
  "旧宿主重载的 %s 不改变正在加载的新宿主状态",
  async (outcome) => {
    const pending = deferred<Snapshot>();
    const nextLoad = deferred<Snapshot>();
    const oldClient: SettingsClient = {
      load: vi.fn().mockResolvedValueOnce(initial).mockReturnValue(pending.promise),
      save: vi.fn(),
    };
    const nextClient: SettingsClient = { load: () => nextLoad.promise, save: vi.fn() };
    const { result, rerender } = renderPersistence(oldClient);
    await act(async () => {});
    let reload!: Promise<void>;
    act(() => {
      reload = result.current.reload();
    });
    rerender({ client: nextClient });

    await act(async () => {
      if (outcome === "success") pending.resolve(initial);
      else pending.reject(new Error("合成旧宿主失败"));
      await reload;
    });
    expect(result.current.snapshotRef.current).toBeUndefined();
    expect(result.current.draftRef.current).toBeUndefined();
    expect(result.current.busy).toBe(true);
    expect(result.current.error).toBe("");
    expect(result.current.loadFailed).toBe(false);

    await act(async () => {
      nextLoad.resolve(replacement);
    });
    expect(result.current.snapshotRef.current).toEqual(replacement);
    expect(result.current.busy).toBe(false);
  },
);

test("旧宿主重载晚于新宿主加载完成时不覆盖新偏好", async () => {
  const pending = deferred<Snapshot>();
  const oldClient: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(initial).mockReturnValue(pending.promise),
    save: vi.fn(),
  };
  const nextClient: SettingsClient = { load: async () => replacement, save: vi.fn() };
  const { result, rerender } = renderPersistence(oldClient);
  await act(async () => {});
  let reload!: Promise<void>;
  act(() => {
    reload = result.current.reload();
  });
  rerender({ client: nextClient });
  await act(async () => {});
  expect(result.current.snapshotRef.current).toEqual(replacement);

  await act(async () => {
    pending.resolve(initial);
    await reload;
  });
  expect(result.current.snapshotRef.current).toEqual(replacement);
  expect(result.current.draftRef.current).toEqual(replacement.preferences);
});

test("旧宿主挂起的重载不阻止新宿主重载，旧收尾也不解除新请求的互斥", async () => {
  const oldPending = deferred<Snapshot>();
  const nextPending = deferred<Snapshot>();
  const oldClient: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(initial).mockReturnValue(oldPending.promise),
    save: vi.fn(),
  };
  const nextClient: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(replacement).mockReturnValue(nextPending.promise),
    save: vi.fn(),
  };
  const { result, rerender } = renderPersistence(oldClient);
  await act(async () => {});
  let oldReload!: Promise<void>;
  act(() => {
    oldReload = result.current.reload();
  });
  rerender({ client: nextClient });
  await act(async () => {});
  let nextReload!: Promise<void>;
  act(() => {
    nextReload = result.current.reload();
  });
  expect(nextClient.load).toHaveBeenCalledTimes(2);

  await act(async () => {
    oldPending.resolve(initial);
    await oldReload;
  });
  expect(result.current.busy).toBe(true);
  let duplicate!: Promise<void>;
  act(() => {
    duplicate = result.current.reload();
  });
  expect(nextClient.load).toHaveBeenCalledTimes(2);

  const latest = { ...replacement, revision: 3 };
  await act(async () => {
    nextPending.resolve(latest);
    await Promise.all([nextReload, duplicate]);
  });
  expect(result.current.snapshotRef.current).toEqual(latest);
  expect(result.current.busy).toBe(false);
});

test("首次加载未完成时的重载结果不被首次加载迟到结果覆盖", async () => {
  const initialLoad = deferred<Snapshot>();
  const reloadLoad = deferred<Snapshot>();
  const client: SettingsClient = {
    load: vi.fn().mockReturnValueOnce(initialLoad.promise).mockReturnValueOnce(reloadLoad.promise),
    save: vi.fn(),
  };
  const { result } = renderPersistence(client);
  await act(async () => {});

  let reload!: Promise<void>;
  act(() => {
    reload = result.current.reload();
  });
  expect(client.load).toHaveBeenCalledTimes(2);

  await act(async () => {
    reloadLoad.resolve(replacement);
    await reload;
  });
  expect(result.current.snapshotRef.current).toEqual(replacement);

  await act(async () => {
    initialLoad.resolve(initial);
    await Promise.resolve();
  });
  expect(result.current.snapshotRef.current).toEqual(replacement);
  expect(result.current.draftRef.current).toEqual(replacement.preferences);
});
