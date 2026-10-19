// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";
import type { HandwritingInkEditorProps } from "@msime/ui";

afterEach(cleanup);

function props(overrides: Partial<HandwritingInkEditorProps> = {}): HandwritingInkEditorProps {
  return {
    strokes: [],
    drawing: false,
    disabled: false,
    recognizing: false,
    canUndo: false,
    canRedo: false,
    canClear: false,
    canRecognize: false,
    onStart: vi.fn(),
    onMove: vi.fn(),
    onEnd: vi.fn(),
    onCancel: vi.fn(),
    onUndo: vi.fn(),
    onRedo: vi.fn(),
    onClear: vi.fn(),
    ...overrides,
  };
}

test("公共入口提供手写笔迹编辑组件", () => {
  expect(ui.HandwritingInkEditor).toBeTypeOf("function");
});

test("空画布保留书写提示、辅助线与编辑快捷键", () => {
  render(<ui.HandwritingInkEditor {...props()} />);
  const canvas = screen.getByLabelText("手写画布");
  expect(canvas.getAttribute("viewBox")).toBe("0 0 420 420");
  expect(canvas.getAttribute("tabindex")).toBe("0");
  expect(canvas.getAttribute("data-drawing")).toBe("false");
  expect(canvas.querySelector('g[aria-hidden="true"] path')).not.toBeNull();
  expect(screen.getByText("请在这里书写")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重新识别" })).toBeNull();
  expect(screen.getByRole("button", { name: /撤销/ }).getAttribute("aria-keyshortcuts")).toBe(
    "Control+z Meta+z",
  );
  expect(screen.getByRole("button", { name: /重做/ }).getAttribute("aria-keyshortcuts")).toBe(
    "Control+Shift+z Meta+Shift+z Control+y",
  );
  for (const button of screen.getAllByRole<HTMLButtonElement>("button")) {
    expect(button.disabled).toBe(true);
  }
});

test("画布分别绘制单点和多点笔迹并转发全部指针生命周期", () => {
  const state = props({
    drawing: true,
    strokes: [
      { points: [{ x: 10, y: 20 }] },
      {
        points: [
          { x: 30, y: 40 },
          { x: 50, y: 60 },
        ],
      },
    ],
  });
  render(<ui.HandwritingInkEditor {...state} />);
  const canvas = screen.getByLabelText("手写画布");
  expect(canvas.getAttribute("data-drawing")).toBe("true");
  expect(canvas.querySelector("circle")?.getAttribute("cx")).toBe("10");
  expect(canvas.querySelector("circle")?.getAttribute("cy")).toBe("20");
  expect(canvas.querySelector("circle")?.getAttribute("r")).toBe("2");
  expect(canvas.querySelector("polyline")?.getAttribute("points")).toBe("30,40 50,60");
  expect(screen.queryByText("请在这里书写")).toBeNull();

  fireEvent.pointerDown(canvas);
  fireEvent.pointerMove(canvas);
  fireEvent.pointerUp(canvas);
  fireEvent.pointerCancel(canvas);
  fireEvent.lostPointerCapture(canvas);
  expect(state.onStart).toHaveBeenCalledOnce();
  expect(state.onMove).toHaveBeenCalledOnce();
  expect(state.onEnd).toHaveBeenCalledOnce();
  expect(state.onCancel).toHaveBeenCalledTimes(2);
});

test("可用编辑动作分别调用宿主回调，识别中显示对应状态", () => {
  const state = props({
    canUndo: true,
    canRedo: true,
    canClear: true,
    canRecognize: true,
    onRecognize: vi.fn(),
  });
  const view = render(<ui.HandwritingInkEditor {...state} />);
  fireEvent.click(screen.getByRole("button", { name: /撤销/ }));
  fireEvent.click(screen.getByRole("button", { name: /重做/ }));
  fireEvent.click(screen.getByRole("button", { name: /重写/ }));
  fireEvent.click(screen.getByRole("button", { name: "重新识别" }));
  expect(state.onUndo).toHaveBeenCalledOnce();
  expect(state.onRedo).toHaveBeenCalledOnce();
  expect(state.onClear).toHaveBeenCalledOnce();
  expect(state.onRecognize).toHaveBeenCalledOnce();

  view.rerender(<ui.HandwritingInkEditor {...state} recognizing canRecognize={false} />);
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "识别中…" }).disabled).toBe(true);
});

test("关闭状态禁用全部编辑操作并标记画布，不吞掉指针清理事件", () => {
  const state = props({
    disabled: true,
    canUndo: true,
    canRedo: true,
    canClear: true,
    canRecognize: true,
    onRecognize: vi.fn(),
  });
  render(<ui.HandwritingInkEditor {...state} />);
  const canvas = screen.getByLabelText("手写画布");
  expect(canvas.getAttribute("aria-disabled")).toBe("true");
  for (const button of screen.getAllByRole<HTMLButtonElement>("button")) {
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
  }
  expect(state.onUndo).not.toHaveBeenCalled();
  expect(state.onRedo).not.toHaveBeenCalled();
  expect(state.onClear).not.toHaveBeenCalled();
  expect(state.onRecognize).not.toHaveBeenCalled();
  fireEvent.lostPointerCapture(canvas);
  expect(state.onCancel).toHaveBeenCalledOnce();
});
