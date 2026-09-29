// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MobileInputAiNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains mobile keyboard AI and opens its settings", () => {
  const onOpenAi = vi.fn();
  render(<MobileInputAiNotice onOpenAi={onOpenAi} />);

  expect(screen.getByText("高情商回复", { selector: ".section-title" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "配置键盘 AI" }));
  expect(onOpenAi).toHaveBeenCalledOnce();
});
