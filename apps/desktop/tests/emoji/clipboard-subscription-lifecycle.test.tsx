// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { EmojiPanel, type EmojiPanelClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  localStorage.clear();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

function host() {
  let snapshot = ["合成初始历史"];
  const registrations: {
    listener: () => void;
    pending: ReturnType<typeof deferred<() => void>>;
    stop: ReturnType<typeof vi.fn<() => void>>;
  }[] = [];
  const clipboard = {
    list: vi.fn(async () => snapshot),
    onChanged: vi.fn((listener: () => void) => {
      const pending = deferred<() => void>();
      registrations.push({ listener, pending, stop: vi.fn() });
      return pending.promise;
    }),
  };
  const client = { close: async () => undefined, clipboard };
  return { client, clipboard, registrations, setSnapshot: (next: string[]) => (snapshot = next) };
}

function view(client: EmojiPanelClient) {
  return <EmojiPanel client={client} initialPage="clipboard" />;
}

test("lists immediately, polls until registered, then refreshes only from notifications", async () => {
  vi.useFakeTimers();
  const current = host();
  const mounted = render(view(current.client));
  expect(current.clipboard.onChanged).toHaveBeenCalledOnce();
  await act(async () => undefined);
  expect(screen.getByText("合成初始历史")).toBeDefined();
  expect(current.clipboard.list).toHaveBeenCalledOnce();

  // 验证契约中的 400 ms 轮询间隔；虚拟时钟推进到边界，不等待真实时间。
  current.setSnapshot(["合成轮询历史"]);
  await act(async () => vi.advanceTimersByTimeAsync(399));
  expect(current.clipboard.list).toHaveBeenCalledOnce();
  await act(async () => vi.advanceTimersByTimeAsync(1));
  expect(current.clipboard.list).toHaveBeenCalledTimes(2);
  expect(screen.getByText("合成轮询历史")).toBeDefined();

  const registration = current.registrations[0];
  current.setSnapshot(["合成注册补读"]);
  await act(async () => registration.pending.resolve(registration.stop));
  expect(screen.getByText("合成注册补读")).toBeDefined();
  expect(current.clipboard.list).toHaveBeenCalledTimes(3);
  await act(async () => vi.advanceTimersByTimeAsync(1200));
  expect(current.clipboard.list).toHaveBeenCalledTimes(3);

  current.setSnapshot(["合成通知历史"]);
  await act(async () => registration.listener());
  expect(screen.getByText("合成通知历史")).toBeDefined();
  expect(current.clipboard.list).toHaveBeenCalledTimes(4);
  mounted.unmount();
  expect(registration.stop).toHaveBeenCalledOnce();
  await act(async () => {
    registration.listener();
    await vi.advanceTimersByTimeAsync(400);
  });
  expect(current.clipboard.list).toHaveBeenCalledTimes(4);
});

test.each(["missing", "sync", "async"] as const)(
  "%s notification registration keeps visible polling and visibility refresh",
  async (mode) => {
    vi.useFakeTimers();
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    const current = host();
    const client = {
      ...current.client,
      clipboard: {
        ...current.clipboard,
        onChanged:
          mode === "missing"
            ? undefined
            : mode === "sync"
              ? () => {
                  throw new Error("synthetic registration unavailable");
                }
              : current.clipboard.onChanged,
      },
    };
    render(view(client));
    await act(async () => {
      if (mode === "async")
        current.registrations[0].pending.reject(new Error("synthetic registration unavailable"));
    });
    expect(screen.getByText("合成初始历史")).toBeDefined();
    const reads = current.clipboard.list.mock.calls.length;
    await act(async () => vi.advanceTimersByTimeAsync(800));
    expect(current.clipboard.list).toHaveBeenCalledTimes(reads);

    current.setSnapshot(["合成可见历史"]);
    visibility.mockReturnValue("visible");
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(screen.getByText("合成可见历史")).toBeDefined();
    expect(current.clipboard.list).toHaveBeenCalledTimes(reads + 1);
    await act(async () => vi.advanceTimersByTimeAsync(400));
    expect(current.clipboard.list).toHaveBeenCalledTimes(reads + 2);
  },
);

test("StrictMode releases late registrations after replay and unmount", async () => {
  const current = host();
  const mounted = render(<StrictMode>{view(current.client)}</StrictMode>);
  expect(current.registrations).toHaveLength(2);
  mounted.unmount();
  await act(async () => {
    for (const registration of current.registrations)
      registration.pending.resolve(registration.stop);
  });
  for (const registration of current.registrations)
    expect(registration.stop).toHaveBeenCalledOnce();
});

test("leaving the clipboard page stops polling and reopening starts a new subscription", async () => {
  vi.useFakeTimers();
  const current = host();
  render(view(current.client));
  await act(async () => undefined);
  const old = current.registrations[0];
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  await act(async () => undefined);
  const homeRegistration = current.registrations[1];
  const homeReads = current.clipboard.list.mock.calls.length;
  await act(async () => {
    old.pending.resolve(old.stop);
    await vi.advanceTimersByTimeAsync(800);
  });
  expect(old.stop).toHaveBeenCalledOnce();
  expect(current.clipboard.list).toHaveBeenCalledTimes(homeReads);

  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  await act(async () => undefined);
  const reopened = current.registrations[2];
  const reopenedReads = current.clipboard.list.mock.calls.length;
  current.setSnapshot(["合成重新打开历史"]);
  await act(async () => vi.advanceTimersByTimeAsync(400));
  expect(current.clipboard.list).toHaveBeenCalledTimes(reopenedReads + 1);
  expect(screen.getByText("合成重新打开历史")).toBeDefined();
  await act(async () => {
    homeRegistration.pending.resolve(homeRegistration.stop);
    reopened.pending.resolve(reopened.stop);
  });
  expect(homeRegistration.stop).toHaveBeenCalledOnce();
  const registeredReads = current.clipboard.list.mock.calls.length;
  await act(async () => {
    old.listener();
    homeRegistration.listener();
    await vi.advanceTimersByTimeAsync(800);
  });
  expect(current.clipboard.list).toHaveBeenCalledTimes(registeredReads);
});

test("a replaced client cannot apply old reads or notifications and releases its late handle", async () => {
  const old = host();
  const current = host();
  const oldRead = deferred<string[]>();
  old.clipboard.list.mockImplementation(() => oldRead.promise);
  current.setSnapshot(["合成新宿主历史"]);
  const mounted = render(view(old.client));
  await act(async () => undefined);
  mounted.rerender(view(current.client));
  await act(async () => undefined);
  expect(screen.getByText("合成新宿主历史")).toBeDefined();

  const oldRegistration = old.registrations[0];
  await act(async () => {
    oldRead.resolve(["合成旧宿主历史"]);
    oldRegistration.pending.resolve(oldRegistration.stop);
    oldRegistration.listener();
  });
  expect(oldRegistration.stop).toHaveBeenCalledOnce();
  expect(screen.queryByText("合成旧宿主历史")).toBeNull();
  expect(screen.getByText("合成新宿主历史")).toBeDefined();
  expect(old.clipboard.list).toHaveBeenCalledOnce();
  expect(current.clipboard.list).toHaveBeenCalledOnce();
});

test("registration and repeated notifications invalidate an in-flight read but coalesce the follow-up", async () => {
  const current = host();
  const first = deferred<string[]>();
  const next = deferred<string[]>();
  current.clipboard.list
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => next.promise);
  render(view(current.client));
  await act(async () => undefined);
  const registration = current.registrations[0];
  await act(async () => {
    registration.pending.resolve(registration.stop);
    registration.listener();
    registration.listener();
    registration.listener();
  });
  expect(current.clipboard.list).toHaveBeenCalledOnce();
  await act(async () => first.resolve(["合成过期快照"]));
  expect(current.clipboard.list).toHaveBeenCalledTimes(2);
  expect(screen.queryByText("合成过期快照")).toBeNull();
  await act(async () => next.resolve(["合成最新快照"]));
  expect(screen.getByText("合成最新快照")).toBeDefined();
  expect(current.clipboard.list).toHaveBeenCalledTimes(2);
});

test("polling waits while a clipboard mutation is pending and resumes afterward", async () => {
  vi.useFakeTimers();
  const current = host();
  const deletion = deferred<void>();
  const remove = vi.fn(() => deletion.promise);
  render(view({ ...current.client, clipboard: { ...current.clipboard, remove } }));
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: "删除此条历史记录" }));
  expect(remove).toHaveBeenCalledWith("合成初始历史");
  await act(async () => vi.advanceTimersByTimeAsync(800));
  expect(current.clipboard.list).toHaveBeenCalledOnce();
  current.setSnapshot([]);
  await act(async () => deletion.resolve());
  expect(screen.queryByText("合成初始历史")).toBeNull();
  expect(current.clipboard.list).toHaveBeenCalledTimes(2);
  await act(async () => vi.advanceTimersByTimeAsync(400));
  expect(current.clipboard.list).toHaveBeenCalledTimes(3);
});
