// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  CommunityResourceScopeButtons,
  type CommunityResourceScope,
} from "../../../../packages/ui/src/community/community-resource-scope-buttons";

afterEach(cleanup);

test("renders the three resource scopes for desktop and mobile controls", () => {
  const onScopeChange = vi.fn<(scope: CommunityResourceScope) => void>();
  render(
    <CommunityResourceScopeButtons
      resourceLabel="词库"
      scope="saved"
      onScopeChange={onScopeChange}
    />,
  );

  const desktop = screen.getByRole("group", { name: "词库范围" });
  expect(desktop.querySelector("button[aria-pressed='true']")?.textContent).toBe("收藏");
  fireEvent.click(screen.getByRole("button", { name: "全部" }));
  expect(onScopeChange).toHaveBeenCalledWith("");

  expect(screen.getByRole("group", { name: "词库筛选范围" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "筛选范围：收藏" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "筛选范围：我的作品" }));
  expect(onScopeChange).toHaveBeenLastCalledWith("mine");
});
