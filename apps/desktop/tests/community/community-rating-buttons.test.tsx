// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunityRatingButtons } from "../../../../packages/ui/src/community/community-rating-buttons";

afterEach(cleanup);

test("renders five rating actions and reports the selected stars", () => {
  const onRate = vi.fn();

  render(
    <CommunityRatingButtons
      description="我的评分（可重新选择）"
      disabled={false}
      onRate={onRate}
    />,
  );

  expect(screen.getByLabelText("我的评分").textContent).toContain("我的评分（可重新选择）");
  expect(screen.getAllByRole("button")).toHaveLength(5);
  fireEvent.click(screen.getByRole("button", { name: "评 4 星" }));
  expect(onRate).toHaveBeenCalledWith(4);
});

test("disables every rating action while busy", () => {
  const onRate = vi.fn();

  render(<CommunityRatingButtons description="评分" disabled onRate={onRate} />);

  expect(screen.getAllByRole("button").every((button) => button.hasAttribute("disabled"))).toBe(
    true,
  );
});
