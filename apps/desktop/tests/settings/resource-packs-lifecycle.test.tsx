// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useResourcePacks,
  type ResourcePackClient,
  type ResourcePackStatus,
} from "../../../../packages/ui/src/settings/resource-packs";
import type { LocalVoiceModelProgress } from "../../../../packages/ui/src/voice/local-models";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const missing: ResourcePackStatus[] = [
  { id: "handwriting", state: "missing", size: 100, schemes: [] },
];

function service() {
  const download = deferred<string>();
  const listeners: ((progress: LocalVoiceModelProgress) => void)[] = [];
  const client = {
    list: vi.fn(async () => missing),
    install: vi.fn(() => download.promise),
    cancel: vi.fn(async () => true),
    onProgress: vi.fn(async (listener: (progress: LocalVoiceModelProgress) => void) => {
      listeners.push(listener);
      return () => {};
    }),
  } satisfies ResourcePackClient;
  return { client, download, listeners };
}

test("旧 client 的下载结束不能释放新 client 的下载登记", async () => {
  const old = service(),
    next = service();
  const { result, rerender } = renderHook(({ client }) => useResourcePacks(client), {
    initialProps: { client: old.client },
  });
  await act(async () => {});
  act(() => result.current.install("handwriting"));
  rerender({ client: next.client });
  await act(async () => {});
  act(() => result.current.install("handwriting"));

  await act(async () => old.download.resolve("/synthetic/old"));
  act(() => result.current.ensure("handwriting"));
  expect(next.client.install).toHaveBeenCalledTimes(1);
  expect(result.current.progress.handwriting?.stage).toBe("download");
  await act(async () => next.download.resolve("/synthetic/next"));
});

test("同一 client 复用后不能采纳上一代迟到的列表", async () => {
  const old = service(),
    next = service();
  const list = deferred<ResourcePackStatus[]>();
  old.client.list.mockImplementationOnce(() => list.promise);
  const { result, rerender } = renderHook(({ client }) => useResourcePacks(client), {
    initialProps: { client: old.client },
  });
  rerender({ client: next.client });
  rerender({ client: old.client });
  await act(async () => {});
  expect(result.current.statuses).toEqual(missing);
  await act(async () => list.resolve([{ ...missing[0], state: "installed" }]));
  expect(result.current.statuses).toEqual(missing);
});

test("同一 client 复用后不能采纳上一代迟到的进度和下载失败", async () => {
  const old = service(),
    next = service();
  const { result, rerender } = renderHook(({ client }) => useResourcePacks(client), {
    initialProps: { client: old.client },
  });
  await act(async () => {});
  act(() => result.current.install("handwriting"));
  rerender({ client: next.client });
  rerender({ client: old.client });
  await act(async () => {});
  act(() => old.listeners[0]({ id: "handwriting", stage: "download", downloaded: 50, total: 100 }));
  expect(result.current.progress).toEqual({});
  await act(async () => old.download.reject({ code: "local_model_network" }));
  expect(result.current.errors).toEqual({});
});
