// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DoubaoAuthModeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the selected auth mode and forwards changes", () => {
  const onChange = vi.fn();
  render(<DoubaoAuthModeSection value="api_key" linux={false} onChange={onChange} />);

  expect(screen.getByText("新版控制台使用单 API Key；旧版使用 App ID + Access Token")).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "豆包鉴权方式" }), {
    target: { value: "legacy" },
  });
  expect(onChange).toHaveBeenCalledWith("legacy");
});

test("uses provider wording on Linux", () => {
  render(<DoubaoAuthModeSection value="legacy" linux onChange={vi.fn()} />);
  expect(screen.getByText("语音服务必须与此模式匹配")).toBeTruthy();
});
