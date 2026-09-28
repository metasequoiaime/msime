// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { AiLinuxProviderSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains Linux provider credentials and renders its test controls", () => {
  render(
    <AiLinuxProviderSection>
      <button type="button">测试 AI 辅助配置</button>
    </AiLinuxProviderSection>,
  );

  expect(screen.getByText("Linux AI provider")).toBeTruthy();
  expect(screen.getByText(/ai-provider\.json/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "测试 AI 辅助配置" })).toBeTruthy();
});
