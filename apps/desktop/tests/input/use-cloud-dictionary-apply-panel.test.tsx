// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { useCloudDictionaryApplyPanel, type CloudDictionaryPanelClient } from "@msime/ui";

type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
const status: Response = { localVersion: "synthetic-local-version" };
const preview: Response = {
  previewToken: "synthetic-preview-token",
  snapshot: {
    cloudRevision: 7,
    sha256: "a".repeat(64),
    bytes: 128,
    records: 4,
    entries: 2,
    overlays: 1,
    positions: 1,
    selections: 0,
  },
};
function host(request = vi.fn().mockResolvedValue(status)): CloudDictionaryPanelClient {
  return { request, close: vi.fn() };
}
function mount(client: CloudDictionaryPanelClient) {
  let state!: ReturnType<typeof useCloudDictionaryApplyPanel>;
  function Probe({ client }: { client: CloudDictionaryPanelClient }) {
    state = useCloudDictionaryApplyPanel(client);
    return state.confirmation;
  }
  const view = render(<Probe client={client} />);
  return {
    get current() {
      return state;
    },
    replace(next: CloudDictionaryPanelClient) {
      view.rerender(<Probe client={next} />);
    },
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("StrictMode 初始化后每两秒轮询，客户端替换重建相位，卸载停止", async () => {
  vi.useFakeTimers();
  const first = host();
  const next = host();
  const { result, rerender, unmount } = renderHook(
    ({ client }) => useCloudDictionaryApplyPanel(client),
    { initialProps: { client: first }, reactStrictMode: true },
  );
  await act(async () => {});
  expect(first.request).toHaveBeenCalledTimes(2);
  expect(result.current.localVersion).toBe(status.localVersion);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1999);
  });
  expect(first.request).toHaveBeenCalledTimes(2);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(first.request).toHaveBeenCalledTimes(3);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1000);
  });
  rerender({ client: next });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1999);
  });
  expect(next.request).toHaveBeenCalledTimes(1);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(next.request).toHaveBeenCalledTimes(2);
  unmount();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(4000);
  });
  expect(first.request).toHaveBeenCalledTimes(3);
  expect(next.request).toHaveBeenCalledTimes(2);
});

test("慢于轮询间隔的状态仍可发布，读取完成前不重复请求", async () => {
  vi.useFakeTimers();
  let sequence = 0;
  const request = vi.fn(
    () =>
      new Promise<Response>((resolve) => {
        const version = `synthetic-version-${++sequence}`;
        setTimeout(() => resolve({ localVersion: version }), 2500);
      }),
  );
  const client = host(request);
  const { result } = renderHook(() => useCloudDictionaryApplyPanel(client));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2500);
  });
  expect(result.current.localVersion).toBe("synthetic-version-1");
  expect(request).toHaveBeenCalledTimes(1);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(4000);
  });
  expect(result.current.localVersion).toBe("synthetic-version-2");
  expect(request).toHaveBeenCalledTimes(2);
});

test.each(["resolve", "reject"] as const)(
  "状态读取的 %s 结束后允许重试，进行中不重复读取",
  async (outcome) => {
    const pending = deferred<Response>();
    const client = host(vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(status));
    const { result } = renderHook(() => useCloudDictionaryApplyPanel(client));
    await act(async () => {
      await result.current.refreshStatus();
    });
    expect(client.request).toHaveBeenCalledTimes(1);
    await act(async () => {
      if (outcome === "resolve") pending.resolve({ localVersion: "synthetic-first-version" });
      else pending.reject(new Error("synthetic failure"));
    });
    expect(result.current.localVersion).toBe(
      outcome === "resolve" ? "synthetic-first-version" : null,
    );
    await act(async () => {
      await result.current.refreshStatus();
    });
    expect(result.current.localVersion).toBe(status.localVersion);
    expect(client.request).toHaveBeenCalledTimes(2);
  },
);

test("替换客户端会关闭待处理的快照应用确认", async () => {
  const request = vi.fn().mockImplementation(async ({ operation }) => {
    if (operation === "snapshot_status") return status;
    if (operation === "snapshot_preview") return preview;
    return {};
  });
  const old = host(request);
  const next = host();
  const panel = mount(old);
  await waitFor(() => expect(panel.current.localVersion).toBe(status.localVersion));
  act(() => panel.current.download());
  await waitFor(() => expect(panel.current.preview).toEqual(preview.snapshot));
  act(() => {
    void panel.current.enqueue();
  });
  await screen.findByRole("alertdialog");
  panel.replace(next);
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(request).not.toHaveBeenCalledWith(
    expect.objectContaining({ operation: "snapshot_enqueue" }),
  );
});

test.each(["resolve", "reject"] as const)(
  "客户端替换后旧状态的 %s 不发布，也不释放新读取",
  async (outcome) => {
    vi.useFakeTimers();
    const old = deferred<Response>();
    const current = deferred<Response>();
    const first = host(vi.fn().mockReturnValue(old.promise));
    const next = host(vi.fn().mockReturnValueOnce(current.promise).mockResolvedValue(status));
    const { result, rerender } = renderHook(({ client }) => useCloudDictionaryApplyPanel(client), {
      initialProps: { client: first },
    });
    rerender({ client: next });
    expect(next.request).toHaveBeenCalledTimes(1);
    await act(async () => {
      if (outcome === "resolve") old.resolve({ localVersion: "synthetic-stale-version" });
      else old.reject(new Error("synthetic failure"));
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(result.current.localVersion).toBeNull();
    expect(result.current.notice).toBe("先获取本机词库版本，再下载并预览云端快照");
    expect(next.request).toHaveBeenCalledTimes(1);
    await act(async () => current.resolve(status));
    expect(result.current.localVersion).toBe(status.localVersion);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(next.request).toHaveBeenCalledTimes(2);
  },
);

test.each(["resolve", "reject"] as const)(
  "状态读取期间仍能下载，旧状态的 %s 不覆盖操作结果",
  async (outcome) => {
    vi.useFakeTimers();
    const pending = deferred<Response>();
    const client = host(
      vi
        .fn()
        .mockResolvedValueOnce(status)
        .mockReturnValueOnce(pending.promise)
        .mockResolvedValueOnce(preview)
        .mockResolvedValue(status),
    );
    const { result } = renderHook(() => useCloudDictionaryApplyPanel(client));
    await act(async () => {});
    act(() => {
      void result.current.refreshStatus();
    });
    await act(async () => result.current.download());
    expect(result.current.preview).toEqual(preview.snapshot);
    expect(result.current.busy).toBe(false);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(client.request).toHaveBeenCalledTimes(3);
    await act(async () => {
      if (outcome === "resolve") pending.resolve({ localVersion: "synthetic-stale-version" });
      else pending.reject(new Error("synthetic failure"));
    });
    expect(result.current.localVersion).toBe(status.localVersion);
    expect(result.current.notice).toBe("云端快照已校验，请确认后替换本机词库");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(client.request).toHaveBeenCalledTimes(4);
  },
);

test.each(["resolve", "reject"] as const)(
  "客户端更换隔离旧预览的 %s，旧完成不解除新忙碌",
  async (outcome) => {
    const old = deferred<Response>();
    const current = deferred<Response>();
    const first = host(vi.fn().mockResolvedValueOnce(status).mockReturnValueOnce(old.promise));
    const next = host(
      vi
        .fn()
        .mockResolvedValueOnce({ localVersion: "synthetic-next-version" })
        .mockReturnValueOnce(current.promise),
    );
    const { result, rerender } = renderHook(({ client }) => useCloudDictionaryApplyPanel(client), {
      initialProps: { client: first },
    });
    await waitFor(() => expect(result.current.localVersion).toBe(status.localVersion));
    act(() => result.current.download());
    rerender({ client: next });
    await waitFor(() => expect(result.current.localVersion).toBe("synthetic-next-version"));
    act(() => result.current.download());
    await act(async () => {
      if (outcome === "resolve") old.resolve(preview);
      else old.reject(new Error("synthetic failure"));
    });
    expect(result.current.preview).toBeNull();
    expect(result.current.busy).toBe(true);
    await act(async () => current.resolve(preview));
    expect(result.current.preview).toEqual(preview.snapshot);
    expect(result.current.busy).toBe(false);
    expect(result.current.notice).toBe("云端快照已校验，请确认后替换本机词库");
  },
);

test("下载同步防重且忙碌时不轮询，丢弃预览同时清理令牌", async () => {
  vi.useFakeTimers();
  const pending = deferred<Response>();
  const request = vi.fn().mockResolvedValueOnce(status).mockReturnValueOnce(pending.promise);
  const client = host(request);
  const { result } = renderHook(() => useCloudDictionaryApplyPanel(client));
  await act(async () => {});
  act(() => {
    result.current.download();
    result.current.download();
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2000);
  });
  expect(request).toHaveBeenCalledTimes(2);
  await act(async () => pending.resolve(preview));
  expect(result.current.previewToken).toBe(preview.previewToken);
  act(() => result.current.discardPreview());
  expect(result.current.preview).toBeNull();
  expect(result.current.previewToken).toBeNull();
  await act(async () => {
    await result.current.enqueue();
  });
  expect(request).toHaveBeenCalledTimes(2);
});

test("没有本机版本时不下载，无效预览显示原失败提示", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce({})
    .mockResolvedValueOnce(status)
    .mockResolvedValueOnce({ snapshot: preview.snapshot });
  const client = host(request);
  const { result } = renderHook(() => useCloudDictionaryApplyPanel(client));
  await act(async () => {});
  act(() => result.current.download());
  expect(request).toHaveBeenCalledTimes(1);
  expect(result.current.notice).toBe("尚未获取本机词库版本，请先打开水杉键盘");
  await act(async () => {
    await result.current.refreshStatus();
  });
  await act(async () => result.current.download());
  expect(result.current.preview).toBeNull();
  expect(result.current.notice).toBe("下载云词库快照失败");
  expect(result.current.busy).toBe(false);
});
