// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceCommitModeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards commit mode changes", () => {
  const onChange = vi.fn();
  render(<VoiceCommitModeSection macos={false} value="tsf" onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("结果提交策略"), {
    target: { value: "ctrl_v" },
  });

  expect(onChange).toHaveBeenCalledWith("ctrl_v");
});

test("shows macOS permission guidance", () => {
  render(<VoiceCommitModeSection macos value="sendinput" onChange={vi.fn()} />);

  expect(screen.getByText(/系统事件权限/)).toBeTruthy();
});
