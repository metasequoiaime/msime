// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityGalleryHeading } from "@msime/ui";

afterEach(cleanup);

test("shared gallery heading renders title, note, and actions in the heading layout", () => {
  render(
    <CommunityGalleryHeading title="社区作品" note="合成说明">
      <button type="button">发布作品</button>
    </CommunityGalleryHeading>,
  );

  expect(screen.getByRole("heading", { name: "社区作品" })).toBeTruthy();
  expect(screen.getByText("合成说明")).toBeTruthy();
  expect(screen.getByRole("button", { name: "发布作品" })).toBeTruthy();
  expect(screen.getByRole("heading").parentElement?.parentElement?.className).toContain(
    "justify-between",
  );
});
