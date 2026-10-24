// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useMcpConnectState } from "../../../../packages/ui/src/settings/use-mcp-connect-state";
import type { McpServerStatus } from "../../../../packages/ui/src/settings/mcp-command-model";

function server(command = "/synthetic/msime-mcp"): McpServerStatus {
  return {
    command,
    installed: true,
    options: "/synthetic/options.json",
    config: "{}",
    clients: [
      {
        id: "cursor",
        path: "/synthetic/cursor.json",
        configured: true,
        stale: false,
        flags: ["--allow-write"],
      },
    ],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => window.localStorage.clear());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("连接状态可独立使用，各实例的选择与权限草稿互不覆盖", async () => {
  const capabilities = {
    status: async () => server(),
    install: vi.fn().mockResolvedValue("updated"),
    confirm: vi.fn().mockResolvedValue(true),
  };
  const { result } = renderHook(() => ({
    first: useMcpConnectState(capabilities),
    second: useMcpConnectState(capabilities),
  }));
  await waitFor(() => expect(result.current.first.server).toBeDefined());
  act(() => result.current.first.selectTab("cursor"));
  expect(result.current.first.flags).toEqual(["--allow-write"]);
  act(() => result.current.first.setFlag("--allow-dictionary-read", true));
  expect(result.current.first.flags).toEqual(["--allow-write", "--allow-dictionary-read"]);
  expect(result.current.first.outdated).toBe(true);
  act(() => result.current.first.selectTab("json"));
  act(() => result.current.first.selectTab("cursor"));
  expect(result.current.first.outdated).toBe(true);
  expect(result.current.second.shownTab).toBe("claude_code");
  act(() => result.current.second.selectTab("cursor"));
  expect(result.current.second.flags).toEqual(["--allow-write"]);
  expect(result.current.second.outdated).toBe(false);
  expect(JSON.parse(window.localStorage.getItem("msime.mcp.flags")!)).toEqual([
    "--allow-write",
    "--allow-dictionary-read",
  ]);
});

test.each(["install", "copyText"] as const)(
  "仅更换 %s 也重新读取，旧失败不能覆盖新状态",
  async (changed) => {
    const old = deferred<McpServerStatus>();
    const status = vi.fn().mockReturnValueOnce(old.promise).mockResolvedValue(server());
    const confirm = vi.fn().mockResolvedValue(true);
    const initial = { status, install: vi.fn(), copyText: vi.fn(), confirm };
    const { result, rerender } = renderHook(useMcpConnectState, { initialProps: initial });
    rerender({ ...initial, [changed]: vi.fn() });
    await waitFor(() => expect(result.current.server?.command).toBe("/synthetic/msime-mcp"));
    await act(async () => old.reject(new Error("synthetic-old-load")));
    expect(result.current.loadFailed).toBe(false);
    expect(status).toHaveBeenCalledTimes(2);
    act(() => result.current.selectTab("json"));
    expect(status).toHaveBeenCalledTimes(2);
  },
);

test("能力更换后的旧授权回答不能安装，也不能解除新操作的忙碌", async () => {
  const answer = deferred<boolean>();
  const oldInstall = vi.fn();
  const nextInstallResult = deferred<"updated">();
  const nextInstall = vi.fn(() => nextInstallResult.promise);
  const status = async () => server();
  const confirm = vi.fn().mockReturnValueOnce(answer.promise).mockResolvedValue(true);
  const { result, rerender } = renderHook(useMcpConnectState, {
    initialProps: { status, install: oldInstall, confirm },
  });
  await waitFor(() => expect(result.current.server).toBeDefined());
  act(() => result.current.write("cursor", ["--allow-write"]));
  expect(result.current.busyClient).toBe("cursor");
  rerender({ status, install: nextInstall, confirm });
  act(() => {
    result.current.write("cursor", []);
    result.current.write("cursor", []);
  });
  expect(nextInstall).toHaveBeenCalledTimes(1);
  await act(async () => answer.resolve(true));
  expect(oldInstall).not.toHaveBeenCalled();
  expect(result.current.busy).toBe(true);
  expect(result.current.busyClient).toBe("cursor");
  await act(async () => nextInstallResult.resolve("updated"));
  expect(result.current.busy).toBe(false);
  expect(result.current.busyClient).toBeUndefined();
  expect(result.current.result).toContain("已更新 Cursor");
  act(() => result.current.selectTab("json"));
  expect(result.current.result).toBeUndefined();
});

test("当前条目冲突经确认替换，成功后清草稿并读取宿主状态", async () => {
  const current = server();
  const refreshed = server("/synthetic/updated-msime-mcp");
  const status = vi.fn().mockResolvedValueOnce(current).mockResolvedValue(refreshed);
  const install = vi
    .fn()
    .mockRejectedValueOnce({ code: "mcp_entry_exists" })
    .mockResolvedValueOnce("updated");
  const confirm = vi.fn().mockResolvedValue(true);
  const { result } = renderHook(() => useMcpConnectState({ status, install, confirm }));
  await waitFor(() => expect(result.current.server).toBe(current));
  act(() => result.current.selectTab("cursor"));
  act(() => result.current.setFlag("--allow-write", false));
  expect(result.current.outdated).toBe(true);
  act(() => result.current.write("cursor", result.current.flags));
  await waitFor(() => expect(result.current.busy).toBe(false));
  expect(confirm).toHaveBeenCalledOnce();
  expect(confirm.mock.calls[0][0].title).toContain("替换 Cursor");
  expect(install.mock.calls).toEqual([
    ["cursor", false, []],
    ["cursor", true, []],
  ]);
  expect(result.current.server).toBe(refreshed);
  expect(result.current.flags).toEqual(["--allow-write"]);
  expect(result.current.outdated).toBe(false);
  expect(status).toHaveBeenCalledTimes(2);
});

test("独立复制状态只接受最近结果，成功提示到期且卸载释放计时", async () => {
  const oldCopy = deferred<void>();
  const copyText = vi.fn().mockReturnValueOnce(oldCopy.promise).mockResolvedValue(undefined);
  const status = async () => server();
  const confirm = vi.fn();
  const { result, unmount } = renderHook(() => useMcpConnectState({ status, copyText, confirm }));
  await waitFor(() => expect(result.current.server).toBeDefined());
  vi.useFakeTimers();
  act(() => {
    void result.current.copy("old", "synthetic-old");
  });
  await act(async () => result.current.copy("new", "synthetic-new"));
  await act(async () => oldCopy.reject(new Error("synthetic-old-copy")));
  expect(result.current.copyFailed).toBe(false);
  expect(result.current.copied).toBe("new");
  expect(result.current.copyFeedbackVisible).toBe(true);
  act(() => vi.advanceTimersByTime(1600));
  expect(result.current.copyFeedbackVisible).toBe(false);
  await act(async () => result.current.copy("last", "synthetic-last"));
  expect(vi.getTimerCount()).toBe(1);
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});
