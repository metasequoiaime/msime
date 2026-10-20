// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { EmojiPanel } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.useRealTimers();
});

/** 临时消息以胶囊提示浮在内容上方，文案与 macOS 原生表情面板一致（「已复制」后两个空格）。 */
function toast() {
  return document.querySelector("[data-panel-toast]")?.textContent ?? null;
}

async function setup() {
  const copyText = vi.fn().mockResolvedValue(undefined);
  const client = {
    close: async () => {},
    copyText,
    loadCatalog: async () => ({
      emoji: [{ title: "合成目录", icon: "★", items: [{ text: "★", keywords: "星" }] }],
      kaomoji: [],
      symbols: [],
    }),
  };
  const view = render(<EmojiPanel client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
  const item = await screen.findByRole("button", { name: "★" });
  vi.useFakeTimers();
  return { ...view, client, copyText, item };
}

test("表情重复复制从最后一次成功重新计时", async () => {
  const { item } = await setup();
  await act(async () => {
    fireEvent.click(item);
  });
  expect(toast()).toBe("已复制  ★");
  // 胶囊外层是 nowrap，会把连续空格并成一个；文字所在的元素要用 pre 保留两个空格。
  expect(document.querySelector("[data-panel-toast] span")?.className).toContain("whitespace-pre");
  act(() => vi.advanceTimersByTime(1000));
  await act(async () => {
    fireEvent.click(item);
  });
  act(() => vi.advanceTimersByTime(600));
  expect(toast()).toBe("已复制  ★");
  act(() => vi.advanceTimersByTime(1000));
  expect(toast()).toBeNull();
  expect(screen.getByText("点击项目即可复制")).toBeDefined();
});

test("复制失败替换临时提示后错误持续显示", async () => {
  const { item, copyText } = await setup();
  await act(async () => {
    fireEvent.click(item);
  });
  act(() => vi.advanceTimersByTime(1000));
  copyText.mockRejectedValueOnce(new Error("synthetic copy failure"));
  await act(async () => {
    fireEvent.click(item);
  });
  const notice = screen.getByText("无法访问剪贴板");
  act(() => vi.advanceTimersByTime(10000));
  expect(notice.textContent).toBe("无法访问剪贴板");
  expect(vi.getTimerCount()).toBe(0);
});

test("表情提示在更换客户端和卸载时取消计时器", async () => {
  const { item, client, rerender, unmount } = await setup();
  await act(async () => {
    fireEvent.click(item);
  });
  await act(async () => {
    rerender(<EmojiPanel client={{ ...client }} />);
  });
  expect(toast()).toBeNull();
  expect(vi.getTimerCount()).toBe(0);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "★" }));
  });
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});
