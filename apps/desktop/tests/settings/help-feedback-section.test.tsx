// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HelpFeedbackSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("help and feedback links navigate to their pages", () => {
  const onHelp = vi.fn();
  const onFeedback = vi.fn();
  render(<HelpFeedbackSection visible onHelp={onHelp} onFeedback={onFeedback} />);

  fireEvent.click(screen.getByRole("button", { name: "使用帮助" }));
  fireEvent.click(screen.getByRole("button", { name: "反馈问题与建议" }));
  expect(onHelp).toHaveBeenCalledOnce();
  expect(onFeedback).toHaveBeenCalledOnce();
});

test("help and feedback links stay hidden when unavailable", () => {
  const { container } = render(
    <HelpFeedbackSection visible={false} onHelp={vi.fn()} onFeedback={vi.fn()} />,
  );

  expect(container.firstChild).toBeNull();
});
