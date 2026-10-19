// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";
import type { HandwritingResultsProps } from "@msime/ui";

afterEach(cleanup);

function props(overrides: Partial<HandwritingResultsProps> = {}): HandwritingResultsProps {
  return {
    candidates: [],
    mode: "copy",
    copyAvailable: true,
    inputAvailable: true,
    disabled: false,
    notice: "合成状态提示",
    onModeChange: vi.fn(),
    onNavigate: vi.fn(),
    onChoose: vi.fn(),
    ...overrides,
  };
}

test("公共入口提供手写识别结果组件", () => {
  expect(ui.HandwritingResults).toBeTypeOf("function");
});

test("空结果保留隐藏的书写引导，模型插槽与状态提示仍可访问", () => {
  render(
    <ui.HandwritingResults {...props()}>
      <p>合成模型提示</p>
    </ui.HandwritingResults>,
  );
  expect(screen.getByRole("heading", { name: "识别结果" })).toBeTruthy();
  expect(within(screen.getByRole("group", { name: "识别候选" })).queryAllByRole("button")).toEqual(
    [],
  );
  const hint = screen.getByText("写下一个字，候选会出现在这里");
  expect(hint.closest('[aria-hidden="true"]')).not.toBeNull();
  expect(screen.getByText("合成模型提示").closest('[aria-hidden="true"]')).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("合成状态提示");
});

test.each([
  ["copy", true, true, "输入候选 合", undefined],
  ["input", true, true, "复制候选 合", true],
  ["copy", true, false, undefined, undefined],
  ["input", false, true, undefined, undefined],
] as const)(
  "%s 模式按复制 %s 与输入 %s 能力转发候选和模式动作",
  (mode, copyAvailable, inputAvailable, sideLabel, sideCopy) => {
    const state = props({ candidates: ["合"], mode, copyAvailable, inputAvailable });
    render(<ui.HandwritingResults {...state} />);
    const modes = within(screen.getByRole("group", { name: "点击手写候选的操作" }));
    const copyMode = modes.getByRole<HTMLButtonElement>("button", { name: "复制" });
    expect(copyMode.disabled).toBe(!copyAvailable);
    expect(copyMode.getAttribute("aria-pressed")).toBe(String(mode === "copy"));
    if (copyAvailable) {
      fireEvent.click(copyMode);
      expect(state.onModeChange).toHaveBeenCalledWith("copy");
    }
    if (inputAvailable) {
      const inputMode = modes.getByRole("button", { name: "输入" });
      expect(inputMode.getAttribute("aria-pressed")).toBe(String(mode === "input"));
      fireEvent.click(inputMode);
      expect(state.onModeChange).toHaveBeenCalledWith("input");
    } else {
      expect(modes.queryByRole("button", { name: "输入" })).toBeNull();
    }

    const grid = within(screen.getByRole("group", { name: "识别候选" }));
    const candidate = grid.getByRole("button", { name: "合" });
    fireEvent.click(candidate);
    expect(state.onChoose).toHaveBeenCalledWith("合", mode === "copy");
    vi.mocked(state.onChoose).mockClear();
    fireEvent.keyDown(candidate, { key: "c", ctrlKey: true });
    if (copyAvailable) expect(state.onChoose).toHaveBeenCalledExactlyOnceWith("合", true);
    else expect(state.onChoose).not.toHaveBeenCalled();
    vi.mocked(state.onChoose).mockClear();

    if (sideLabel) {
      fireEvent.click(grid.getByRole("button", { name: sideLabel }));
      if (sideCopy) expect(state.onChoose).toHaveBeenCalledExactlyOnceWith("合", true);
      else expect(state.onChoose).toHaveBeenCalledExactlyOnceWith("合");
    } else {
      expect(grid.getAllByRole("button")).toHaveLength(1);
    }
    expect(screen.queryByText("写下一个字，候选会出现在这里")).toBeNull();
  },
);

test("禁用状态覆盖模式、候选及辅助操作和复制快捷键", () => {
  const state = props({ candidates: ["合"], disabled: true });
  render(<ui.HandwritingResults {...state} />);
  for (const button of screen.getAllByRole<HTMLButtonElement>("button")) {
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
  }
  fireEvent.keyDown(screen.getByRole("button", { name: "合" }), { key: "c", ctrlKey: true });
  expect(state.onChoose).not.toHaveBeenCalled();
  expect(state.onModeChange).not.toHaveBeenCalled();
});

test("候选导航回调保留网格 currentTarget 与候选事件目标", () => {
  const navigate = vi.fn();
  render(
    <ui.HandwritingResults
      {...props({ candidates: ["合", "成"] })}
      onNavigate={(event) => navigate(event.key, event.currentTarget, event.target)}
    />,
  );
  const grid = screen.getByRole("group", { name: "识别候选" });
  const candidate = within(grid).getByRole("button", { name: "合" });
  fireEvent.keyDown(candidate, { key: "ArrowRight" });
  expect(navigate).toHaveBeenCalledExactlyOnceWith("ArrowRight", grid, candidate);
});
