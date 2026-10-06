// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityGalleryGrid } from "@msime/ui";

afterEach(cleanup);

test("renders the shared community gallery grid", () => {
  render(
    <CommunityGalleryGrid data-testid="grid">
      <span>卡片</span>
    </CommunityGalleryGrid>,
  );

  const grid = screen.getByTestId("grid");
  expect(grid.tagName).toBe("DIV");
  expect(grid.className).toBe("grid grid-cols-2 gap-x-3 gap-y-3.5 max-phone:grid-cols-1");
  expect(screen.getByText("卡片")).toBeTruthy();
});

test("forwards grid attributes and appends a local class", () => {
  render(
    <CommunityGalleryGrid className="compact" aria-label="社区卡片" data-testid="grid">
      内容
    </CommunityGalleryGrid>,
  );

  const grid = screen.getByTestId("grid");
  expect(grid.className).toContain("compact");
  expect(grid.getAttribute("aria-label")).toBe("社区卡片");
});
