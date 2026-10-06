// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityCard } from "@msime/ui";

afterEach(cleanup);

test("renders a community card with the shared card surface", () => {
  render(
    <CommunityCard aria-label="社区卡片" data-testid="card">
      内容
    </CommunityCard>,
  );

  const card = screen.getByTestId("card");
  expect(card.tagName).toBe("BUTTON");
  expect(card.getAttribute("type")).toBe("button");
  expect(card.className).toBe(
    "flex min-w-0 flex-col items-stretch gap-2 overflow-hidden rounded-[19px] border border-edge bg-card p-2.5 text-left [color:var(--p-text)] shadow-card hover:border-edge-strong",
  );
  expect(screen.getByText("内容")).toBeTruthy();
});

test("forwards button attributes and appends a local class", () => {
  render(
    <CommunityCard className="compact" aria-label="可操作卡片" data-testid="card">
      内容
    </CommunityCard>,
  );

  const card = screen.getByTestId("card");
  expect(card.className).toContain("compact");
  expect(card.getAttribute("aria-label")).toBe("可操作卡片");
});
