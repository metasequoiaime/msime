// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AiApiTokenSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("shows the normalized origin and forwards token changes", () => {
  const onTokenChange = vi.fn();
  render(
    <AiApiTokenSection
      origin="https://ai.example.test:443"
      token="synthetic-token"
      onTokenChange={onTokenChange}
    />,
  );

  expect(screen.getByText("只用于 https://ai.example.test:443")).toBeTruthy();
  const token = screen.getByLabelText("AI API Token") as HTMLInputElement;
  expect(token.value).toBe("synthetic-token");
  fireEvent.change(token, { target: { value: "updated-token" } });
  expect(onTokenChange).toHaveBeenCalledWith("updated-token");
});

test("disables token input and explains an invalid endpoint", () => {
  render(<AiApiTokenSection origin={null} token="synthetic-token" onTokenChange={vi.fn()} />);

  expect(screen.getByText("请先在「更多选项」中填写有效的 HTTPS 接口地址")).toBeTruthy();
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(true);
});
