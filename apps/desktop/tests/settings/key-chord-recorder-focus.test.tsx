// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { KeyChordRecorder } from "../../../../packages/ui/src/settings/key-chord-recorder";

afterEach(cleanup);

test("点击录键后接管键盘焦点，当前焦点收到的组合键能够提交", () => {
  const commit = vi.fn();
  render(
    <>
      <input aria-label="其他输入框" />
      <KeyChordRecorder value="" onCommit={commit} />
    </>,
  );
  act(() => screen.getByRole("textbox").focus());
  const recorder = screen.getByRole("button", { name: "录制快捷键" });
  fireEvent.click(recorder);
  expect(document.activeElement).toBe(recorder);
  fireEvent.keyDown(document.activeElement!, { code: "ControlLeft", ctrlKey: true });
  expect(screen.getByText("Ctrl+")).toBeTruthy();
  expect(commit).not.toHaveBeenCalled();
  fireEvent.keyDown(document.activeElement!, { code: "KeyR", ctrlKey: true, altKey: true });
  expect(commit).toHaveBeenCalledExactlyOnceWith("Ctrl+Alt+KeyR");
  expect(recorder.getAttribute("aria-pressed")).toBe("false");
});

test("转移焦点会取消录键，之后的按键不写入绑定", () => {
  const commit = vi.fn();
  render(
    <>
      <KeyChordRecorder value="Ctrl+KeyE" onCommit={commit} />
      <input aria-label="其他输入框" />
    </>,
  );
  const recorder = screen.getByRole("button", { name: "录制快捷键" });
  fireEvent.click(recorder);
  act(() => screen.getByRole("textbox").focus());
  expect(recorder.getAttribute("aria-pressed")).toBe("false");
  expect(screen.getByText("Ctrl+KeyE")).toBeTruthy();
  fireEvent.keyDown(document.activeElement!, { code: "KeyR", ctrlKey: true });
  expect(commit).not.toHaveBeenCalled();
});

test("点击另一个录键控件时只保留新控件录制", () => {
  const firstCommit = vi.fn();
  const secondCommit = vi.fn();
  render(
    <>
      <KeyChordRecorder value="Ctrl+KeyE" onCommit={firstCommit} />
      <KeyChordRecorder value="" onCommit={secondCommit} />
    </>,
  );
  const [first, second] = screen.getAllByRole("button", { name: "录制快捷键" });
  fireEvent.click(first);
  fireEvent.click(second);
  expect(document.activeElement).toBe(second);
  expect(first.getAttribute("aria-pressed")).toBe("false");
  expect(second.getAttribute("aria-pressed")).toBe("true");
  fireEvent.keyDown(document.activeElement!, { code: "Escape" });
  expect(second.getAttribute("aria-pressed")).toBe("false");
  expect(firstCommit).not.toHaveBeenCalled();
  expect(secondCommit).not.toHaveBeenCalled();
});
