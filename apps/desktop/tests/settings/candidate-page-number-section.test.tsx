// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePageNumberSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("page numbers default on and toggle independently", () => {
  const onChange = vi.fn();
  const { rerender } = render(<CandidatePageNumberSection value={undefined} onChange={onChange} />);
  const toggle = screen.getByLabelText("显示页码") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(false);
  rerender(<CandidatePageNumberSection value={false} onChange={onChange} />);
  expect((screen.getByLabelText("显示页码") as HTMLInputElement).checked).toBe(false);
  fireEvent.click(screen.getByLabelText("显示页码"));
  expect(onChange).toHaveBeenCalledWith(true);
});

// Linux 的 Fcitx5 候选窗跟 macOS 一样把 ‹ › 一起藏起来，行名照 macOS；IBus 面板的箭头属于面板，说明里讲清楚。
test("on Linux the switch names the page arrows and says the IBus panel keeps its own", () => {
  render(<CandidatePageNumberSection value={undefined} platform="linux" onChange={vi.fn()} />);
  expect(screen.queryByLabelText("显示页码")).toBeNull();
  expect((screen.getByLabelText("显示页码和翻页按钮") as HTMLInputElement).checked).toBe(true);
  expect(
    screen.getByText(
      "关闭后候选窗不再显示页码和 ‹ ›（IBus 面板自带的翻页箭头不受影响），仍可用键盘翻页；鼠标滚轮是否翻页见「输入 › 选词与翻页」。",
    ),
  ).toBeTruthy();
});

// Windows 候选窗首行和 macOS 一样把「1 / 3」和 ‹ › 一起藏起来，说明指向默认关闭的滚轮翻页；Android 候选栏没有箭头，只藏页码（#10709）。
test("Windows names the page arrows and Android keeps the plain page number title", () => {
  const { rerender } = render(
    <CandidatePageNumberSection value={false} platform="windows" onChange={vi.fn()} />,
  );
  expect((screen.getByLabelText("显示页码和翻页按钮") as HTMLInputElement).checked).toBe(false);
  expect(
    screen.getByText(
      "关闭后候选窗不再显示「1 / 3」和 ‹ ›，仍可用键盘翻页；鼠标滚轮翻页默认关闭，需要时在「输入 › 选词与翻页」里打开。",
    ),
  ).toBeTruthy();
  rerender(<CandidatePageNumberSection value={undefined} platform="android" onChange={vi.fn()} />);
  expect(screen.queryByLabelText("显示页码和翻页按钮")).toBeNull();
  expect((screen.getByLabelText("显示页码") as HTMLInputElement).checked).toBe(true);
});
