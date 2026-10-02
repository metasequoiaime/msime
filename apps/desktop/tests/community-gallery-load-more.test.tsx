// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type LoadMoreProps = {
  hasMore: boolean;
  busy: boolean;
  loadingText: string;
  onLoadMore: () => void;
};

test("community gallery load more renders the action and loading status", () => {
  const LoadMore = (ui as unknown as { CommunityGalleryLoadMore: ComponentType<LoadMoreProps> })
    .CommunityGalleryLoadMore;
  expect(LoadMore).toBeDefined();

  const onLoadMore = vi.fn();
  render(<LoadMore hasMore busy loadingText="正在读取候选窗口皮肤…" onLoadMore={onLoadMore} />);

  expect(screen.getByRole("status").textContent).toContain("正在读取候选窗口皮肤…");
  const button = screen.getByRole("button", { name: "加载更多" });
  expect(button).toHaveProperty("disabled", true);
});

test("community gallery load more hides when there is no next page", () => {
  const LoadMore = (ui as unknown as { CommunityGalleryLoadMore: ComponentType<LoadMoreProps> })
    .CommunityGalleryLoadMore;
  const onLoadMore = vi.fn();
  render(<LoadMore hasMore={false} busy={false} loadingText="加载中" onLoadMore={onLoadMore} />);

  expect(screen.queryByRole("button", { name: "加载更多" })).toBeNull();
  expect(screen.queryByRole("status")).toBeNull();
  expect(onLoadMore).not.toHaveBeenCalled();
});
