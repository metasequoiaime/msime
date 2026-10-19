// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HandwritingCandidateButton } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

test("手写候选按钮点击输入并用 Control+C 复制一次", () => {
  const choose = vi.fn();
  const copy = vi.fn();
  render(
    <HandwritingCandidateButton
      candidate="合成候选"
      copy
      disabled={false}
      onChoose={choose}
      onCopy={copy}
    />,
  );
  const button = screen.getByRole("button", { name: "合成候选" });

  fireEvent.click(button);
  fireEvent.keyDown(button, { key: "c", ctrlKey: true });
  fireEvent.keyDown(button, { key: "c", ctrlKey: true, repeat: true });

  expect(choose).toHaveBeenCalledOnce();
  expect(copy).toHaveBeenCalledOnce();
  expect(button.getAttribute("aria-keyshortcuts")).toBe("Control+c");
});

test("手写候选按钮忽略组合输入的复制快捷键", () => {
  const copy = vi.fn();
  render(
    <HandwritingCandidateButton
      candidate="合成候选"
      copy
      disabled={false}
      onChoose={vi.fn()}
      onCopy={copy}
    />,
  );
  fireEvent.keyDown(screen.getByRole("button", { name: "合成候选" }), {
    key: "c",
    ctrlKey: true,
    isComposing: true,
  });

  expect(copy).not.toHaveBeenCalled();
});

test("手写候选按钮按码点和实时宽度调整字号并在卸载时清理观察", async () => {
  const observe = vi.fn();
  const disconnect = vi.fn();
  let notify!: ResizeObserverCallback;
  const observer = { observe, disconnect };
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        notify = callback;
        return observer;
      }
    },
  );
  const measure = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockReturnValue({ width: 40 } as DOMRect);
  const view = render(
    <HandwritingCandidateButton candidate="𠀀" copy={false} disabled={false} onChoose={vi.fn()} />,
  );
  const button = screen.getByRole("button", { name: "𠀀" });
  await waitFor(() => expect(button.style.fontSize).toBe("20.8px"));

  measure.mockReturnValue({ width: 80 } as DOMRect);
  act(() => notify([], observer as unknown as ResizeObserver));
  expect(button.style.fontSize).toBe("34px");

  view.unmount();
  expect(disconnect).toHaveBeenCalledOnce();
  expect(observe).toHaveBeenCalledWith(button);
  expect(measure).toHaveBeenCalled();
});
