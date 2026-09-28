// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ScreenKeyboardCommunitySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("describes community skins and opens the community", () => {
  const onOpen = vi.fn();
  render(<ScreenKeyboardCommunitySection onOpen={onOpen} />);

  expect(screen.getByText("社区皮肤")).toBeTruthy();
  expect(screen.getByText("看看别人做的键盘皮肤，可以直接试用或保存")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "去社区发现皮肤" }));
  expect(onOpen).toHaveBeenCalledOnce();
});
