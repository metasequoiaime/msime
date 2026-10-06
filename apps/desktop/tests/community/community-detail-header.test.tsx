// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityDetailHeader } from "../../../../packages/ui/src/community/community-detail-header";

afterEach(cleanup);

test("renders shared detail title, metadata, owner badge, moderation badge, and description", () => {
  render(
    <CommunityDetailHeader
      title="示例皮肤"
      note="皮肤 · 示例作者"
      owned
      moderation="removed"
      description="公开说明"
    />,
  );

  expect(screen.getByRole("heading", { name: "示例皮肤" })).not.toBeNull();
  expect(screen.getByText("皮肤 · 示例作者")).not.toBeNull();
  expect(screen.getByText("我的作品")).not.toBeNull();
  expect(screen.getByText("已下架")).not.toBeNull();
  expect(screen.getByText("公开说明")).not.toBeNull();
});

test("does not render owner or moderation badges for another user's item", () => {
  render(<CommunityDetailHeader title="示例插件" note="插件 · 作者" owned={false} />);

  expect(screen.queryByText("我的作品")).toBeNull();
  expect(screen.queryByText("已下架")).toBeNull();
});
