// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityConfirmationActions } from "@msime/ui";

afterEach(cleanup);

test("renders the shared confirmation action layout", () => {
  render(
    <CommunityConfirmationActions data-testid="actions">
      <button type="button">确定</button>
    </CommunityConfirmationActions>,
  );

  const actions = screen.getByTestId("actions");
  expect(actions.tagName).toBe("DIV");
  expect(actions.className).toBe("flex flex-wrap items-center gap-2");
  expect(screen.getByText("确定")).toBeTruthy();
});

test("forwards action attributes and appends a local class", () => {
  render(
    <CommunityConfirmationActions className="compact" aria-label="确认操作" data-testid="actions">
      内容
    </CommunityConfirmationActions>,
  );

  const actions = screen.getByTestId("actions");
  expect(actions.className).toContain("compact");
  expect(actions.getAttribute("aria-label")).toBe("确认操作");
});
