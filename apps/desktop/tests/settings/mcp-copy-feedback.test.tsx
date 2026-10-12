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
