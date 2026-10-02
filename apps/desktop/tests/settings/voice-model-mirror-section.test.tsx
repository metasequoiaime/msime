// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceModelMirrorRow } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("shows a valid mirror and reports edits", () => {
  const onChange = vi.fn();
  render(<VoiceModelMirrorRow value="https://mirror.example.com" onChange={onChange} />);

  const input = screen.getByRole("textbox", { name: "模型下载镜像" }) as HTMLInputElement;
  expect(input.getAttribute("aria-invalid")).toBe("false");
  fireEvent.change(input, { target: { value: "https://mirror.example.com/cache" } });
  expect(onChange).toHaveBeenCalledWith("https://mirror.example.com/cache");
});

test("marks malformed mirrors invalid", () => {
  render(<VoiceModelMirrorRow value="ftp://bad.example.com" onChange={vi.fn()} />);
  expect(
    (screen.getByRole("textbox", { name: "模型下载镜像" }) as HTMLInputElement).getAttribute(
      "aria-invalid",
    ),
  ).toBe("true");
});
