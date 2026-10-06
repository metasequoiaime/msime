// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PolishCredentialFieldsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards endpoint and token edits", () => {
  const onEndpointChange = vi.fn();
  const onTokenChange = vi.fn();
  render(
    <PolishCredentialFieldsSection
      endpoint="https://polish.example.test"
      token="synthetic-token"
      onEndpointChange={onEndpointChange}
      onTokenChange={onTokenChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("润色接口地址"), {
    target: { value: "https://updated.example.test" },
  });
  fireEvent.change(screen.getByLabelText("润色 API Token"), {
    target: { value: "updated-token" },
  });

  expect(onEndpointChange).toHaveBeenCalledWith("https://updated.example.test");
  expect(onTokenChange).toHaveBeenCalledWith("updated-token");
  expect(screen.getByText(/留空使用当前服务的默认地址/)).toBeTruthy();
  expect(screen.getByText("仅保存在本机设置中")).toBeTruthy();
});
