// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceEndpointSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards endpoint edits", () => {
  const onChange = vi.fn();
  render(<VoiceEndpointSection value="https://voice.example.test" onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("识别接口地址"), {
    target: { value: "https://updated.example.test" },
  });

  expect(onChange).toHaveBeenCalledWith("https://updated.example.test");
  expect(screen.getByText(/留空使用当前服务的默认地址/)).toBeTruthy();
});
