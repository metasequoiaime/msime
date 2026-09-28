// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CustomTranslationSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards endpoint, key, and enable changes", () => {
  const onToggle = vi.fn();
  const onEndpointChange = vi.fn();
  const onApiKeyChange = vi.fn();
  render(
    <CustomTranslationSection
      enabled
      available
      endpoint="https://translate.example.test"
      apiKey="synthetic-key"
      onToggle={onToggle}
      onEndpointChange={onEndpointChange}
      onApiKeyChange={onApiKeyChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "自定义翻译服务" }));
  fireEvent.change(screen.getByLabelText("自定义翻译 Endpoint"), {
    target: { value: "https://updated.example.test" },
  });
  fireEvent.change(screen.getByLabelText("自定义翻译 API Key"), {
    target: { value: "updated-key" },
  });
  expect(onToggle).toHaveBeenCalledWith(false);
  expect(onEndpointChange).toHaveBeenCalledWith("https://updated.example.test");
  expect(onApiKeyChange).toHaveBeenCalledWith("updated-key");
});

test("shows endpoint validation and disables controls when unavailable", () => {
  render(
    <CustomTranslationSection
      enabled
      available={false}
      endpoint="bad"
      apiKey="synthetic-key"
      endpointIssue="请填写完整的接口地址。"
      onToggle={vi.fn()}
      onEndpointChange={vi.fn()}
      onApiKeyChange={vi.fn()}
    />,
  );

  expect(screen.queryByText("请填写完整的接口地址。")).toBeNull();
  expect((screen.getByLabelText("自定义翻译 Endpoint") as HTMLInputElement).disabled).toBe(true);
  expect(
    (screen.getByRole("checkbox", { name: "自定义翻译服务" }) as HTMLInputElement).disabled,
  ).toBe(true);
});
