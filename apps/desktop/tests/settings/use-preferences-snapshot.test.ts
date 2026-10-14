// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { usePreferencesSnapshot, type Snapshot } from "@msime/ui";
import type { PreferencesSnapshotClient } from "../../../../packages/ui/src/settings/use-preferences-snapshot";

afterEach(cleanup);

const snapshot = (revision: number, theme: "light" | "dark"): Snapshot => ({
  format_version: 1,
  revision,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    theme,
  },
});

test("subscribes before loading and keeps only newer preference snapshots", async () => {
  let emit!: (value: Snapshot) => void;
  let resolve!: (value: Snapshot) => void;
  const order: string[] = [];
  const stop = vi.fn();
  const preferences = {
    onPreferencesChanged: async (listener: (value: Snapshot) => void) => {
      order.push("subscribe");
      emit = listener;
      return stop;
    },
    load: () => {
      order.push("load");
      return new Promise<Snapshot>((done) => {
        resolve = done;
      });
    },
  };
  const { result, unmount } = renderHook(() => usePreferencesSnapshot(preferences));

  await waitFor(() => expect(order).toEqual(["subscribe", "load"]));
  act(() => emit(snapshot(3, "dark")));
  expect(result.current?.revision).toBe(3);
  await act(async () => resolve(snapshot(2, "light")));
  expect(result.current?.revision).toBe(3);
  act(() => emit(snapshot(4, "light")));
  expect(result.current?.revision).toBe(4);
  unmount();
  expect(stop).toHaveBeenCalledOnce();
});

test("clears the previous snapshot when requested client changes", async () => {
  const first = { load: async () => snapshot(1, "dark") };
  const second = { load: async () => snapshot(2, "light") };
  const { result, rerender } = renderHook(
    ({ preferences }) => usePreferencesSnapshot(preferences, true),
    { initialProps: { preferences: first } },
  );

  await waitFor(() => expect(result.current?.revision).toBe(1));
  rerender({ preferences: second });
  expect(result.current).toBeNull();
  await waitFor(() => expect(result.current?.revision).toBe(2));
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

function service() {
  const registrations: {
    listener: (value: Snapshot) => void;
    pending: ReturnType<typeof deferred<() => void>>;
    stop: ReturnType<typeof vi.fn<() => void>>;
  }[] = [];
  const loading = deferred<Snapshot>();
  const preferences: PreferencesSnapshotClient = {
    onPreferencesChanged: vi.fn(function (this: PreferencesSnapshotClient, listener) {
      expect(this).toBe(preferences);
      const pending = deferred<() => void>();
      registrations.push({ listener, pending, stop: vi.fn() });
      return pending.promise;
    }),
    load: vi.fn(() => loading.promise),
  };
  return { preferences, registrations, loading };
}

test("注册未完成时不加载，加载未完成时卸载立即释放监听", async () => {
  const host = service();
  const hook = renderHook(() => usePreferencesSnapshot(host.preferences));
  expect(host.registrations).toHaveLength(1);
  expect(host.preferences.load).not.toHaveBeenCalled();
  const registration = host.registrations[0];
  await act(async () => registration.pending.resolve(registration.stop));
  expect(host.preferences.load).toHaveBeenCalledOnce();
  expect(hook.result.current).toBeNull();
  hook.unmount();
  expect(registration.stop).toHaveBeenCalledOnce();
  await act(async () => host.loading.resolve(snapshot(2, "dark")));
  expect(hook.result.current).toBeNull();
  expect(registration.stop).toHaveBeenCalledOnce();
});

test.each(["更换宿主", "卸载"])("%s 后晚到注册释放且不发起旧宿主加载", async (exit) => {
  const previous = service();
  const next = service();
  const hook = renderHook(({ preferences }) => usePreferencesSnapshot(preferences, true), {
    initialProps: { preferences: previous.preferences },
  });
  const stale = previous.registrations[0];
  act(() => stale.listener(snapshot(1, "dark")));
  if (exit === "更换宿主") {
    hook.rerender({ preferences: next.preferences });
    const current = next.registrations[0];
    await act(async () => current.pending.resolve(current.stop));
    await act(async () => next.loading.resolve(snapshot(2, "light")));
  } else hook.unmount();
  const before = hook.result.current;
  await act(async () => {
    stale.listener(snapshot(99, "dark"));
    stale.pending.resolve(stale.stop);
  });
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(previous.preferences.load).not.toHaveBeenCalled();
  expect(hook.result.current).toEqual(before);
  if (exit === "更换宿主") {
    hook.unmount();
    expect(next.registrations[0].stop).toHaveBeenCalledOnce();
    expect(stale.stop).toHaveBeenCalledOnce();
  }
});

test("宿主更换后旧加载结果无效，默认保留最后快照直到新加载完成", async () => {
  const previous = service();
  const next = service();
  const hook = renderHook(({ preferences }) => usePreferencesSnapshot(preferences), {
    initialProps: { preferences: previous.preferences },
  });
  const stale = previous.registrations[0];
  await act(async () => stale.pending.resolve(stale.stop));
  act(() => stale.listener(snapshot(8, "dark")));
  hook.rerender({ preferences: next.preferences });
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(hook.result.current?.revision).toBe(8);
  await act(async () => previous.loading.resolve(snapshot(99, "dark")));
  expect(hook.result.current?.revision).toBe(8);
  const current = next.registrations[0];
  await act(async () => current.pending.resolve(current.stop));
  await act(async () => next.loading.resolve(snapshot(1, "light")));
  expect(hook.result.current?.revision).toBe(1);
});

test("重置选项变化重建监听并阻止旧事件与晚到加载", async () => {
  const host = service();
  const hook = renderHook(({ reset }) => usePreferencesSnapshot(host.preferences, reset), {
    initialProps: { reset: false },
  });
  const stale = host.registrations[0];
  await act(async () => stale.pending.resolve(stale.stop));
  act(() => stale.listener(snapshot(5, "dark")));
  hook.rerender({ reset: true });
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(hook.result.current).toBeNull();
  await act(async () => {
    stale.listener(snapshot(99, "dark"));
    host.loading.resolve(snapshot(6, "dark"));
  });
  expect(hook.result.current).toBeNull();
  const current = host.registrations[1];
  await act(async () => current.pending.resolve(current.stop));
  expect(hook.result.current?.revision).toBe(6);
});

test("StrictMode 两次立即注册，仅当前注册启动加载和采纳事件", async () => {
  const host = service();
  const hook = renderHook(() => usePreferencesSnapshot(host.preferences), { wrapper: StrictMode });
  expect(host.registrations).toHaveLength(2);
  const [stale, current] = host.registrations;
  await act(async () => {
    stale.pending.resolve(stale.stop);
    current.pending.resolve(current.stop);
  });
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(current.stop).not.toHaveBeenCalled();
  expect(host.preferences.load).toHaveBeenCalledOnce();
  act(() => stale.listener(snapshot(99, "dark")));
  expect(hook.result.current).toBeNull();
  act(() => current.listener(snapshot(4, "light")));
  await act(async () => host.loading.resolve(snapshot(3, "dark")));
  expect(hook.result.current?.revision).toBe(4);
  hook.unmount();
  expect(stale.stop).toHaveBeenCalledOnce();
  expect(current.stop).toHaveBeenCalledOnce();
});

test.each(["同步抛错", "异步拒绝", "缺少能力"])("订阅%s 时继续加载快照", async (failure) => {
  const preferences: PreferencesSnapshotClient = { load: vi.fn(async () => snapshot(2, "light")) };
  if (failure !== "缺少能力") {
    preferences.onPreferencesChanged = vi.fn(() => {
      if (failure === "同步抛错") throw new Error("synthetic failure");
      return Promise.reject(new Error("synthetic failure"));
    });
  }
  const hook = renderHook(() => usePreferencesSnapshot(preferences));
  if (failure === "同步抛错") expect(preferences.load).toHaveBeenCalledOnce();
  await waitFor(() => expect(hook.result.current?.revision).toBe(2));
  expect(preferences.load).toHaveBeenCalledOnce();
});

test("首次加载拒绝时保留监听已经采纳的快照", async () => {
  const host = service();
  const hook = renderHook(() => usePreferencesSnapshot(host.preferences));
  const registration = host.registrations[0];
  await act(async () => registration.pending.resolve(registration.stop));
  act(() => registration.listener(snapshot(4, "light")));
  await act(async () => host.loading.reject(new Error("synthetic failure")));
  expect(hook.result.current?.revision).toBe(4);
  act(() => registration.listener(snapshot(4, "dark")));
  expect(hook.result.current?.preferences.theme).toBe("light");
});
