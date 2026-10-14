// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { StrictMode, useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { Preferences, SettingsClient, Snapshot } from "@msime/ui";
import { useSettingsPersistence } from "../../../../packages/ui/src/settings/use-settings-persistence";
import { useMountedRef } from "../../../../packages/ui/src/settings/use-mounted-ref";

afterEach(cleanup);

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

function host() {
  const registrations: {
    listener: (value: Snapshot) => void;
    pending: ReturnType<typeof deferred<() => void>>;
    stop: ReturnType<typeof vi.fn<() => void>>;
  }[] = [];
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    onPreferencesChanged: vi.fn(function (this: SettingsClient, listener) {
      expect(this).toBe(client);
      const pending = deferred<() => void>();
      registrations.push({ listener, pending, stop: vi.fn() });
      return pending.promise;
    }),
  };
  return { client, registrations };
}

function harness(client: SettingsClient, strict = false) {
  const setError = vi.fn();
  const setNotice = vi.fn();
  const hook = renderHook(
    ({ client }: { client: SettingsClient }) => {
      const mounted = useMountedRef();
      const [snapshot, setSnapshot] = useState<Snapshot>();
      const [draft, setDraft] = useState<Preferences>();
      return useSettingsPersistence({
        client,
        mobile: false,
        mounted,
        snapshot,
        draft,
        setSnapshot,
        setDraft,
        setBusy: vi.fn(),
        setError,
        setNotice,
        setRecoveredBackup: vi.fn(),
      });
    },
    { initialProps: { client }, wrapper: strict ? StrictMode : undefined },
  );
  return { ...hook, setError, setNotice };
}

test.each(["首次加载", "重载"])("%s 的迟到旧快照不覆盖较新的偏好通知", async (phase) => {
  const { client, registrations } = host();
  const pending = deferred<Snapshot>();
  client.load =
    phase === "首次加载"
      ? vi.fn(() => pending.promise)
      : vi.fn().mockResolvedValueOnce(initial).mockReturnValueOnce(pending.promise);
  const hook = harness(client);
  await act(async () => {});
  let reload: Promise<void> | undefined;
  if (phase === "重载")
    act(() => {
      reload = hook.result.current.reload();
    });
  const latest: Snapshot = {
    ...initial,
    revision: 2,
    preferences: { ...initial.preferences, scheme: "wubi" },
  };
  act(() => registrations[0].listener(latest));
  expect(hook.result.current.snapshotRef.current).toEqual(latest);
  await act(async () => {
    pending.resolve(initial);
    await reload;
  });
  expect(hook.result.current.snapshotRef.current).toEqual(latest);
  expect(hook.result.current.draftRef.current).toEqual(latest.preferences);
});

test("当前监听立即注册、接收更新，并在卸载时只释放一次", async () => {
  const { client, registrations } = host();
  const hook = harness(client);
  expect(registrations).toHaveLength(1);
  const registration = registrations[0];
  await act(async () => registration.pending.resolve(registration.stop));
  const changed = { ...initial, revision: 2 };
  act(() => registration.listener(changed));
  expect(hook.result.current.snapshotRef.current).toEqual(changed);
  expect(registration.stop).not.toHaveBeenCalled();
  hook.unmount();
  expect(registration.stop).toHaveBeenCalledOnce();
  act(() => registration.listener({ ...initial, revision: 3 }));
  expect(hook.result.current.snapshotRef.current).toEqual(changed);
});

test.each(["更换宿主", "卸载"])("%s 后晚到的注册立即释放且旧事件无效", async (exit) => {
  const previous = host();
  const next = host();
  const hook = harness(previous.client);
  await act(async () => {});
  const registration = previous.registrations[0];
  if (exit === "更换宿主") {
    hook.rerender({ client: next.client });
    await act(async () => {});
  } else hook.unmount();
  const before = hook.result.current.snapshotRef.current;
  hook.setNotice.mockClear();
  await act(async () => {
    registration.listener({ ...initial, revision: 99 });
    registration.pending.resolve(registration.stop);
  });
  expect(hook.result.current.snapshotRef.current).toEqual(before);
  expect(hook.setNotice).not.toHaveBeenCalled();
  expect(registration.stop).toHaveBeenCalledOnce();
  if (exit === "更换宿主") {
    const current = next.registrations[0];
    await act(async () => current.pending.resolve(current.stop));
    act(() => current.listener({ ...initial, revision: 2 }));
    expect(hook.result.current.snapshotRef.current?.revision).toBe(2);
    hook.unmount();
    expect(current.stop).toHaveBeenCalledOnce();
    expect(registration.stop).toHaveBeenCalledOnce();
  }
});

test("更换宿主时释放已完成的注册且不因普通重渲染重新订阅", async () => {
  const previous = host();
  const next = host();
  const hook = harness(previous.client);
  const registration = previous.registrations[0];
  await act(async () => registration.pending.resolve(registration.stop));
  hook.rerender({ client: previous.client });
  expect(previous.registrations).toHaveLength(1);
  hook.rerender({ client: next.client });
  expect(registration.stop).toHaveBeenCalledOnce();
  await act(async () => {});
  act(() => registration.listener({ ...initial, revision: 99 }));
  expect(hook.result.current.snapshotRef.current).toEqual(initial);
});

test("StrictMode 保留两次立即注册并释放失效监听", async () => {
  const { client, registrations } = host();
  const hook = harness(client, true);
  expect(registrations).toHaveLength(2);
  const [stale, current] = registrations;
  await act(async () => {
    stale.pending.resolve(stale.stop);
    current.pending.resolve(current.stop);
  });
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(current.stop).not.toHaveBeenCalled();
  act(() => stale.listener({ ...initial, revision: 99 }));
  expect(hook.result.current.snapshotRef.current).toEqual(initial);
  act(() => current.listener({ ...initial, revision: 2 }));
  expect(hook.result.current.snapshotRef.current?.revision).toBe(2);
  hook.unmount();
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(current.stop).toHaveBeenCalledOnce();
});

test("注册拒绝保持静默且不阻止初始加载", async () => {
  const { client, registrations } = host();
  const hook = harness(client);
  await act(async () => registrations[0].pending.reject(new Error("synthetic failure")));
  expect(hook.result.current.snapshotRef.current).toEqual(initial);
  expect(hook.setError).not.toHaveBeenCalled();
  expect(hook.result.current.loadFailed).toBe(false);
});
