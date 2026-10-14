// @vitest-environment jsdom

import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useWindowState, type UseWindowStateOptions } from "@msime/ui";

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

test("StrictMode starts one current subscription and releases it on unmount", async () => {
  let publish!: (value: boolean) => void;
  const stop = vi.fn();
  const setError = vi.fn();
  const subscribe = vi.fn<NonNullable<UseWindowStateOptions["client"]["onWindowStateChanged"]>>(
    async (listener) => {
      publish = listener;
      return stop;
    },
  );
  const client = { onWindowStateChanged: subscribe };
  const { result, rerender, unmount } = renderHook(() => useWindowState({ client, setError }), {
    wrapper: ({ children }) => <StrictMode>{children}</StrictMode>,
  });

  expect(result.current).toBe(false);
  await waitFor(() => expect(subscribe).toHaveBeenCalledOnce());
  act(() => publish(true));
  expect(result.current).toBe(true);
  act(() => publish(false));
  expect(result.current).toBe(false);
  rerender();
  await act(async () => {});
  expect(subscribe).toHaveBeenCalledOnce();
  unmount();
  expect(stop).toHaveBeenCalledOnce();
  expect(setError).not.toHaveBeenCalled();
});

test("unmount before the setup microtask skips registration", async () => {
  const subscribe = vi.fn(async () => vi.fn());
  const setError = vi.fn();
  const { unmount } = renderHook(() =>
    useWindowState({ client: { onWindowStateChanged: subscribe }, setError }),
  );

  unmount();
  await act(async () => {});
  expect(subscribe).not.toHaveBeenCalled();
  expect(setError).not.toHaveBeenCalled();
});

test("a registration finishing after unmount is released once", async () => {
  const pending = deferred<() => void>();
  const stop = vi.fn();
  const setError = vi.fn();
  let reportError!: () => void;
  const subscribe = vi.fn<NonNullable<UseWindowStateOptions["client"]["onWindowStateChanged"]>>(
    (_listener, onError) => {
      reportError = onError!;
      return pending.promise;
    },
  );
  const { unmount } = renderHook(() =>
    useWindowState({ client: { onWindowStateChanged: subscribe }, setError }),
  );
  await waitFor(() => expect(subscribe).toHaveBeenCalledOnce());
  unmount();
  await act(async () => {
    reportError();
    pending.resolve(stop);
    await pending.promise;
  });

  expect(stop).toHaveBeenCalledOnce();
  expect(setError).not.toHaveBeenCalled();
});

test("replacing the host resets state and ignores its late events and registration", async () => {
  const pending = deferred<() => void>();
  const oldStop = vi.fn();
  const newStop = vi.fn();
  const setError = vi.fn();
  let oldPublish!: (value: boolean) => void;
  let oldError!: () => void;
  const oldSubscribe = vi.fn<NonNullable<UseWindowStateOptions["client"]["onWindowStateChanged"]>>(
    (listener, onError) => {
      oldPublish = listener;
      oldError = onError!;
      return pending.promise;
    },
  );
  const client: UseWindowStateOptions["client"] = { onWindowStateChanged: oldSubscribe };
  const { result, rerender, unmount } = renderHook(
    ({ client }: { client: UseWindowStateOptions["client"] }) =>
      useWindowState({ client, setError }),
    { initialProps: { client } },
  );
  await waitFor(() => expect(oldSubscribe).toHaveBeenCalledOnce());
  act(() => oldPublish(true));
  expect(result.current).toBe(true);
  rerender({ client: {} });
  expect(result.current).toBe(false);
  act(() => {
    oldPublish(true);
    oldError();
  });
  expect(result.current).toBe(false);
  expect(setError).not.toHaveBeenCalled();

  let newPublish!: (value: boolean) => void;
  const newSubscribe = vi.fn<NonNullable<UseWindowStateOptions["client"]["onWindowStateChanged"]>>(
    async (listener) => {
      newPublish = listener;
      return newStop;
    },
  );
  rerender({ client: { onWindowStateChanged: newSubscribe } });
  await waitFor(() => expect(newSubscribe).toHaveBeenCalledOnce());
  act(() => newPublish(true));
  await act(async () => {
    pending.resolve(oldStop);
    await pending.promise;
    oldPublish(false);
    oldError();
  });
  expect(result.current).toBe(true);
  expect(oldStop).toHaveBeenCalledOnce();
  expect(setError).not.toHaveBeenCalled();
  unmount();
  expect(oldStop).toHaveBeenCalledOnce();
  expect(newStop).toHaveBeenCalledOnce();
});

test.each(["sync", "async"])("current %s setup failures use the fixed message", async (failure) => {
  const setError = vi.fn();
  const subscribe = vi.fn(() => {
    const error = new Error("synthetic host detail");
    if (failure === "sync") throw error;
    return Promise.reject(error);
  });
  const { result } = renderHook(() =>
    useWindowState({ client: { onWindowStateChanged: subscribe }, setError }),
  );

  await waitFor(() =>
    expect(setError).toHaveBeenCalledExactlyOnceWith("无法读取窗口状态，请重试。"),
  );
  expect(result.current).toBe(false);
});

test("a replaced host's late setup failure cannot report an error", async () => {
  const pending = deferred<() => void>();
  const setError = vi.fn();
  const subscribe = vi.fn(() => pending.promise);
  const client: UseWindowStateOptions["client"] = { onWindowStateChanged: subscribe };
  const { rerender } = renderHook(
    ({ client }: { client: UseWindowStateOptions["client"] }) =>
      useWindowState({ client, setError }),
    { initialProps: { client } },
  );
  await waitFor(() => expect(subscribe).toHaveBeenCalledOnce());
  rerender({ client: {} });
  await act(async () => {
    pending.reject(new Error("synthetic stale detail"));
    await pending.promise.catch(() => undefined);
  });
  expect(setError).not.toHaveBeenCalled();
});

test("current host error callbacks report and normal state updates still work", async () => {
  const setError = vi.fn();
  let publish!: (value: boolean) => void;
  let reportError!: () => void;
  const subscribe = vi.fn<NonNullable<UseWindowStateOptions["client"]["onWindowStateChanged"]>>(
    async (listener, onError) => {
      publish = listener;
      reportError = onError!;
      return () => {};
    },
  );
  const client = { onWindowStateChanged: subscribe };
  const { result } = renderHook(() => useWindowState({ client, setError }));
  await waitFor(() => expect(subscribe).toHaveBeenCalledOnce());
  act(() => reportError());
  expect(setError).toHaveBeenCalledExactlyOnceWith("无法读取窗口状态，请重试。");
  act(() => publish(true));
  expect(result.current).toBe(true);
});
