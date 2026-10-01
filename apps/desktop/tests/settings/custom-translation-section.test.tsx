// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CustomTranslationSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards endpoint and key changes and leaves choosing the service to 翻译服务", () => {
  const onEndpointChange = vi.fn();
  const onApiKeyChange = vi.fn();
  render(
    <CustomTranslationSection
      available
      endpoint="https://translate.example.test"
      apiKey="synthetic-key"
      onEndpointChange={onEndpointChange}
      onApiKeyChange={onApiKeyChange}
    />,
  );

  // The service is turned on only from the 翻译服务 select, so its own settings carry no switch.
  expect(screen.queryByRole("switch")).toBeNull();
  fireEvent.change(screen.getByLabelText("自定义翻译 Endpoint"), {
    target: { value: "https://updated.example.test" },
  });
  fireEvent.change(screen.getByLabelText("自定义翻译 API Key"), {
    target: { value: "updated-key" },
  });
  expect(onEndpointChange).toHaveBeenCalledWith("https://updated.example.test");
  expect(onApiKeyChange).toHaveBeenCalledWith("updated-key");
});

test("shows endpoint validation and disables controls when unavailable", () => {
  render(
    <CustomTranslationSection
      available={false}
      endpoint="bad"
      apiKey="synthetic-key"
      endpointIssue="请填写完整的接口地址。"
      onEndpointChange={vi.fn()}
      onApiKeyChange={vi.fn()}
    />,
  );

  expect(screen.queryByText("请填写完整的接口地址。")).toBeNull();
  expect((screen.getByLabelText("自定义翻译 Endpoint") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("自定义翻译 API Key") as HTMLInputElement).disabled).toBe(true);
});
