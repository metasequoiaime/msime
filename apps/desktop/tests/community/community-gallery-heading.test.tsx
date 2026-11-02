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
  expect(screen.getByRole("heading").closest(".justify-between")).not.toBeNull();
  // 没有 `help` 时标题旁不画「?」。
  expect(screen.queryByRole("button", { name: /的说明：/ })).toBeNull();
});

test("gallery help goes behind a ? next to the title instead of staying on the page", () => {
  render(<CommunityGalleryHeading title="社区插件" help="安装后在「我的插件」中选用" />);

  expect(screen.getByRole("heading", { name: "社区插件" })).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "社区插件的说明：安装后在「我的插件」中选用" }),
  ).toBeTruthy();
});
