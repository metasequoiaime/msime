// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityGalleryHeading } from "@msime/ui";

afterEach(cleanup);

test("shared gallery heading renders title, the note behind the title's info tip, and actions in the heading layout", () => {
  render(
    <CommunityGalleryHeading title="社区作品" note="合成说明">
      <button type="button">发布作品</button>
    </CommunityGalleryHeading>,
  );

  expect(screen.getByRole("heading", { name: "社区作品" })).toBeTruthy();
  // 说明收进标题旁的「?」，不在标题下常驻一行。
  expect(screen.getByRole("button", { name: "社区作品的说明：合成说明" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "发布作品" })).toBeTruthy();
  expect(
    screen
      .getByRole("heading")
      .closest("[class*='justify-between']")
      ?.contains(screen.getByRole("button", { name: "发布作品" })),
  ).toBe(true);
});
