// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceStreamPreeditSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards the streaming preedit toggle", () => {
  const onChange = vi.fn();
  render(<VoiceStreamPreeditSection enabled onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("流式预编辑"));

  expect(onChange).toHaveBeenCalledWith(false);
  expect(screen.getByText(/识别服务支持时显示实时识别片段/)).toBeTruthy();
});
