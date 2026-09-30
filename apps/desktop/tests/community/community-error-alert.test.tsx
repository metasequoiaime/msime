// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommunityErrorAlert } from "../../../../packages/ui/src/community/community-error-alert";
import { CommunityDetailStatus } from "../../../../packages/ui/src/community/community-detail-status";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the error and offers sign-in only when requested", () => {
  const onLogin = vi.fn();
  const { rerender } = render(
    <CommunityErrorAlert message="需要登录" signInRequired onLogin={onLogin} />,
  );

  fireEvent.click(screen.getByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledTimes(1);

  rerender(<CommunityErrorAlert message="请求失败" signInRequired />);
  expect(screen.queryByRole("button", { name: "去登录" })).toBeNull();
  expect(screen.getByRole("alert").textContent).toContain("请求失败");
});

test("renders shared download, rating, loading, and action status", () => {
  render(
    <CommunityDetailStatus
      downloads={1234}
      ratingCount={8}
      ratingAverage={4.5}
      myRating={5}
      detailBusy
      actionNotice="已评分"
    />,
  );

  expect(screen.getByText(/1,234 人下载/)).not.toBeNull();
  expect(screen.getByText("我的评分：5 星")).not.toBeNull();
  expect(screen.getByText("正在读取详情…")).not.toBeNull();
  expect(screen.getByText("已评分")).not.toBeNull();
});
