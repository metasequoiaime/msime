// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { McpConnectSection } from "../../../../packages/ui/src/settings/mcp-connect";

const status = async () => ({
  command: "/synthetic/msime-mcp",
  installed: true,
  options: "/synthetic/options.json",
  config: "{}",
  clients: [],
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function setup() {
  const copyText = vi.fn().mockResolvedValue(undefined);
  const view = render(<McpConnectSection status={status} copyText={copyText} />);
  const copy = await screen.findByRole("button", { name: "复制命令" });
  vi.useFakeTimers();
  return { ...view, copy, copyText };
}

test("同一 MCP 命令再次复制后，提示从最后一次成功重新计时", async () => {
  const { copy, copyText } = await setup();
  await act(async () => {
    fireEvent.click(copy);
  });
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
  act(() => vi.advanceTimersByTime(1000));
  await act(async () => {
    fireEvent.click(copy);
  });
  act(() => vi.advanceTimersByTime(600));
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
  act(() => vi.advanceTimersByTime(1000));
  expect(screen.getByRole("button", { name: "复制命令" })).toBe(copy);
  expect(copyText).toHaveBeenCalledTimes(2);
});

test("复制另一条 MCP 命令替换提示，并在卸载时释放计时器", async () => {
  const { copy, unmount } = await setup();
  await act(async () => {
    fireEvent.click(copy);
  });
  act(() => vi.advanceTimersByTime(1000));
  const remove = screen.getByRole("button", { name: "复制移除命令" });
  await act(async () => {
    fireEvent.click(remove);
  });
  expect(screen.getByRole("button", { name: "复制命令" })).toBe(copy);
  act(() => vi.advanceTimersByTime(600));
  expect(screen.getByRole("button", { name: "已复制" })).toBe(remove);
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});

test("旧客户端的复制完成后不显示提示或创建计时器", async () => {
  const { copy, copyText, rerender } = await setup();
  let finish!: () => void;
  copyText.mockReturnValue(
    new Promise<void>((resolve) => {
      finish = resolve;
    }),
  );
  fireEvent.click(copy);
  rerender(<McpConnectSection status={status} copyText={vi.fn()} />);
  await act(async () => {
    finish();
  });
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
  expect(vi.getTimerCount()).toBe(0);
});

test.each(["同步", "异步"])("MCP 复制%s失败显示固定提示，并允许重试", async (failure) => {
  const { copy, copyText } = await setup();
  copyText.mockImplementationOnce(() => {
    const error = new Error("synthetic-copy-detail");
    if (failure === "同步") throw error;
    return Promise.reject(error);
  });
  await act(async () => fireEvent.click(copy));
  expect(screen.getByRole("alert").textContent).toBe("复制失败，请重试。");
  expect(screen.queryByText(/synthetic-copy-detail/)).toBeNull();
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
  expect(vi.getTimerCount()).toBe(0);

  await act(async () => fireEvent.click(copy));
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
  expect(screen.queryByRole("alert")).toBeNull();
});

test("旧客户端迟到的复制失败不污染新客户端", async () => {
  const { copy, copyText, rerender } = await setup();
  let reject!: (error: Error) => void;
  copyText.mockReturnValueOnce(
    new Promise<void>((_, fail) => {
      reject = fail;
    }),
  );
  fireEvent.click(copy);
  const nextCopy = vi.fn().mockResolvedValue(undefined);
  rerender(<McpConnectSection status={status} copyText={nextCopy} />);
  await act(async () => reject(new Error("synthetic-old-copy-detail")));
  expect(screen.queryByRole("alert")).toBeNull();
  expect(vi.getTimerCount()).toBe(0);

  await act(async () => fireEvent.click(copy));
  expect(nextCopy).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
});

test("较早复制的迟到成功不掩盖最近一次复制失败", async () => {
  const { copy, copyText } = await setup();
  let finish!: () => void;
  copyText
    .mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    )
    .mockRejectedValueOnce(new Error("synthetic-latest-copy-detail"));
  fireEvent.click(copy);
  await act(async () => fireEvent.click(copy));
  expect(screen.getByRole("alert").textContent).toBe("复制失败，请重试。");
  await act(async () => finish());
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
  expect(screen.getByRole("alert").textContent).toBe("复制失败，请重试。");
  expect(vi.getTimerCount()).toBe(0);
});

test("复制成功后再次失败，立即移除旧成功提示", async () => {
  const { copy, copyText } = await setup();
  await act(async () => fireEvent.click(copy));
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
  copyText.mockRejectedValueOnce(new Error("synthetic-retry-copy-detail"));
  await act(async () => fireEvent.click(copy));
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
  expect(screen.getByRole("alert").textContent).toBe("复制失败，请重试。");
  act(() => vi.advanceTimersByTime(1600));
  expect(screen.getByRole("alert").textContent).toBe("复制失败，请重试。");
});

test("较早复制的迟到失败不覆盖最近一次成功", async () => {
  const { copy, copyText } = await setup();
  let reject!: (error: Error) => void;
  copyText.mockReturnValueOnce(
    new Promise<void>((_, fail) => {
      reject = fail;
    }),
  );
  fireEvent.click(copy);
  await act(async () => fireEvent.click(copy));
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
  await act(async () => reject(new Error("synthetic-old-copy-detail")));
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);
});

test.each(["成功", "失败"])("更换宿主后清除旧复制%s反馈", async (outcome) => {
  const { copy, copyText, rerender } = await setup();
  if (outcome === "失败") copyText.mockRejectedValueOnce(new Error("synthetic-copy-detail"));
  await act(async () => fireEvent.click(copy));
  if (outcome === "失败") expect(screen.getByRole("alert")).toBeTruthy();
  else expect(screen.getByRole("button", { name: "已复制" })).toBe(copy);

  rerender(<McpConnectSection status={status} copyText={vi.fn()} />);
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
});
