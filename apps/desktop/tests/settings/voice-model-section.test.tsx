// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceModelSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the provider model field and forwards changes", () => {
  const onChange = vi.fn();
  render(<VoiceModelSection value="model-v1" onChange={onChange} />);

  expect(screen.getByText("由语音服务选择对应模型")).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", { name: "识别模型" }), {
    target: { value: "model-v2" },
  });
  expect(onChange).toHaveBeenCalledWith("model-v2");
});
