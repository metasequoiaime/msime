// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceModelPathSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("edits the manual model path", () => {
  const onChange = vi.fn();
  render(<VoiceModelPathSection path="/old/model.bin" onChange={onChange} />);

  fireEvent.change(screen.getByRole("textbox", { name: "Whisper 模型文件" }), {
    target: { value: "/new/model.bin" },
  });
  expect(onChange).toHaveBeenCalledWith("/new/model.bin");
  expect(screen.queryByRole("button", { name: "选择…" })).toBeNull();
});

test("updates from a selected file and ignores cancellation", async () => {
  const onChange = vi.fn();
  const pickPath = vi.fn().mockResolvedValueOnce("/picked/model.bin").mockResolvedValueOnce(null);
  render(<VoiceModelPathSection path="/old/model.bin" pickPath={pickPath} onChange={onChange} />);

  fireEvent.click(screen.getByRole("button", { name: "选择…" }));
  await vi.waitFor(() => expect(onChange).toHaveBeenCalledWith("/picked/model.bin"));
  onChange.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "选择…" }));
  await Promise.resolve();
  expect(onChange).not.toHaveBeenCalled();
});
