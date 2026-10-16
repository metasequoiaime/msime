// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DesktopHandwriting } from "../../src/input/desktop-handwriting";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
function client() {
  return {
    close: vi.fn().mockResolvedValue(undefined),
    recognizeHandwriting: vi.fn().mockResolvedValue({ candidates: ["中"] }),
    copyHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
    submitHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
  };
}
function write() {
  const canvas = screen.getByLabelText("手写画布");
  for (const [type, point] of [
    ["pointerdown", 20],
    ["pointerup", 40],
  ] as const) {
    const event = new Event(type, { bubbles: true });
    Object.assign(event, {
      pointerId: 1,
      clientX: point,
      clientY: point,
      button: 0,
      isPrimary: true,
    });
    fireEvent(canvas, event);
  }
}

test("旧宿主在窗口缩放后重算候选字号并在卸载时移除监听", async () => {
  vi.stubGlobal("ResizeObserver", undefined);
  const view = render(<DesktopHandwriting client={client()} theme="light" platform="linux" />);
  write();
  const candidate = await screen.findByRole("button", { name: "中" });
  // 按钮出现时 effect 可能尚未执行；先等首次测量完成，再触发旧宿主的窗口回退。
  await waitFor(() => expect(candidate.style.fontSize).toBe("13px"));
  vi.spyOn(candidate, "getBoundingClientRect").mockReturnValue({ width: 40 } as DOMRect);
  act(() => window.dispatchEvent(new Event("resize")));
  expect(candidate.style.fontSize).toBe("20.8px");
  vi.mocked(candidate.getBoundingClientRect).mockClear();

  view.unmount();
  act(() => window.dispatchEvent(new Event("resize")));
  expect(candidate.getBoundingClientRect).not.toHaveBeenCalled();
});

test.each([false, "failure"])("macOS 设置试写在能力为 %s 时只识别并复制", async (permission) => {
  if (permission === false) vi.mocked(invoke).mockResolvedValue(false);
  else vi.mocked(invoke).mockRejectedValue(new Error("synthetic"));
  localStorage.setItem("msime.handwriting.activation", "input");
  const panel = client();
  render(<DesktopHandwriting client={panel} theme="light" platform="macos" />);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("handwriting_can_input"));
  expect(screen.queryByRole("button", { name: "输入" })).toBeNull();
  write();
  fireEvent.click(await screen.findByRole("button", { name: "中" }));
  await waitFor(() => expect(panel.copyHandwritingCandidate).toHaveBeenCalledExactlyOnceWith("中"));
  expect(panel.submitHandwritingCandidate).not.toHaveBeenCalled();
});

test("原生会话允许输入时保留手写上屏", async () => {
  vi.mocked(invoke).mockResolvedValue(true);
  const panel = client();
  render(<DesktopHandwriting client={panel} theme="light" platform="macos" />);
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  write();
  fireEvent.click(await screen.findByRole("button", { name: "中" }));
  await waitFor(() =>
    expect(panel.submitHandwritingCandidate).toHaveBeenCalledExactlyOnceWith("中"),
  );
});

test("旧客户端的能力回答不能给新面板开放输入", async () => {
  let resolve!: (allowed: boolean) => void;
  vi.mocked(invoke)
    .mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    )
    .mockResolvedValue(false);
  const previous = client();
  const next = client();
  const view = render(<DesktopHandwriting client={previous} theme="light" platform="macos" />);
  view.rerender(<DesktopHandwriting client={next} theme="light" platform="macos" />);
  await act(async () => resolve(true));
  expect(screen.queryByRole("button", { name: "输入" })).toBeNull();
  write();
  fireEvent.click(await screen.findByRole("button", { name: "中" }));
  await waitFor(() => expect(next.copyHandwritingCandidate).toHaveBeenCalledOnce());
  expect(previous.submitHandwritingCandidate).not.toHaveBeenCalled();
  expect(next.submitHandwritingCandidate).not.toHaveBeenCalled();
});

test.each(["windows", "linux"])("%s 沿用宿主原有手写输入能力", (platform) => {
  render(<DesktopHandwriting client={client()} theme="light" platform={platform} />);
  expect(screen.getByRole("button", { name: "输入" })).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});
