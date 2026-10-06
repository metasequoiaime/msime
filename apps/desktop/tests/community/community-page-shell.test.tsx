// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityPageShell } from "@msime/ui";

afterEach(cleanup);

test("renders the shared community page shell", () => {
  render(<CommunityPageShell data-testid="shell">内容</CommunityPageShell>);

  const shell = screen.getByTestId("shell");
  expect(shell.tagName).toBe("DIV");
  expect(shell.className).toBe("flex flex-col gap-3.5");
  expect(screen.getByText("内容")).toBeTruthy();
});

test("forwards shell attributes and appends a local class", () => {
  render(
    <CommunityPageShell className="compact" aria-label="社区页面" data-testid="shell">
      内容
    </CommunityPageShell>,
  );

  const shell = screen.getByTestId("shell");
  expect(shell.className).toContain("compact");
  expect(shell.getAttribute("aria-label")).toBe("社区页面");
});
