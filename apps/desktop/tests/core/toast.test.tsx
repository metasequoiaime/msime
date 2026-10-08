// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ToastProvider, toastMillis, useToast } from "../../../../packages/ui/src/core/toast";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function CopyButton({ text = "已复制下载链接" }: { text?: string }) {
  const toast = useToast();
  return (
    <button type="button" onClick={() => toast(text)}>
      复制 {text}
    </button>
  );
}

test("a toast is announced politely and leaves after 1.6 seconds", () => {
  render(
    <ToastProvider>
      <CopyButton />
    </ToastProvider>,
  );
  const region = screen.getByRole("status");
  expect(region.getAttribute("aria-live")).toBe("polite");
  expect(region.textContent).toBe("");

  fireEvent.click(screen.getByRole("button"));
  expect(region.textContent).toBe("已复制下载链接");

  act(() => vi.advanceTimersByTime(toastMillis - 1));
  expect(region.textContent).toBe("已复制下载链接");
  act(() => vi.advanceTimersByTime(1));
  expect(region.textContent).toBe("");
});

test("a new toast replaces the one showing and restarts the clock", () => {
  render(
    <ToastProvider>
      <CopyButton text="已复制" />
      <CopyButton text="已保存" />
    </ToastProvider>,
  );
  const region = screen.getByRole("status");
  fireEvent.click(screen.getByRole("button", { name: "复制 已复制" }));
  act(() => vi.advanceTimersByTime(1000));
  fireEvent.click(screen.getByRole("button", { name: "复制 已保存" }));
  expect(region.textContent).toBe("已保存");

  act(() => vi.advanceTimersByTime(1000));
  expect(region.textContent).toBe("已保存");
  act(() => vi.advanceTimersByTime(toastMillis - 1000));
  expect(region.textContent).toBe("");
});

test("outside a provider showing a toast does nothing", () => {
  render(<CopyButton />);
  fireEvent.click(screen.getByRole("button"));
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.queryByText("已复制下载链接", { selector: "span" })).toBeNull();
});
