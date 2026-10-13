// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AiApiTokenSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("显示服务地址并保留已有 Token 的输入行为", () => {
  const onTokenChange = vi.fn();
  render(
    <AiApiTokenSection
      origin="https://ai.example.test:443"
      endpoint="https://ai.example.test/v1/chat/completions"
      token="synthetic-token"
      onTokenChange={onTokenChange}
    />,
  );

  expect(screen.getByText("服务地址：https://ai.example.test/v1")).toBeTruthy();
  const token = screen.getByLabelText("AI API Token") as HTMLInputElement;
  expect(token.value).toBe("synthetic-token");
  fireEvent.change(token, { target: { value: "updated-token" } });
  expect(onTokenChange).toHaveBeenCalledWith("updated-token");
});

test.each([
  ["https://api.everyapi.ai:443/v1/chat/completions", "https://api.everyapi.ai/v1"],
  ["http://localhost:1234/v1/chat/completions", "http://localhost:1234/v1"],
  [
    "https://ai.example.test:8443/custom/messages?tenant=synthetic",
    "https://ai.example.test:8443/custom",
  ],
])("服务说明保留接口前缀和自定义端口：%s", (endpoint, address) => {
  render(
    <AiApiTokenSection
      origin={new URL(endpoint).origin}
      endpoint={endpoint}
      token=""
      onTokenChange={vi.fn()}
    />,
  );
  expect(screen.getByText(`服务地址：${address}`)).toBeTruthy();
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(false);
});

test("disables token input and explains an invalid endpoint", () => {
  render(<AiApiTokenSection origin={null} token="synthetic-token" onTokenChange={vi.fn()} />);

  expect(screen.getByText("请先在「更多选项」中填写有效的接口地址")).toBeTruthy();
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(true);
});

test("shows why the endpoint cannot carry a token", () => {
  render(
    <AiApiTokenSection
      origin={null}
      endpointHint="http:// 只能用于本机或局域网地址"
      token=""
      onTokenChange={vi.fn()}
    />,
  );

  expect(screen.getByText("http:// 只能用于本机或局域网地址")).toBeTruthy();
});
