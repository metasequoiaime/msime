// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { AiLinuxProviderSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains where Linux keeps the AI provider credentials", () => {
  render(<AiLinuxProviderSection />);

  expect(screen.getByText(/ai-provider\.json/)).toBeTruthy();
  expect(screen.getByText(/凭据不保存在共享设置中/)).toBeTruthy();
});
