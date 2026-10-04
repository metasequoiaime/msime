// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityMetrics, CommunityRatingMetrics } from "@msime/ui";

afterEach(cleanup);

test("renders shared community metrics styling", () => {
  render(<CommunityMetrics>下载与评分</CommunityMetrics>);

  const metrics = screen.getByText("下载与评分");
  expect(metrics.tagName).toBe("P");
  expect(metrics.className).toBe("m-0 text-xs leading-relaxed text-muted");
});

test("forwards metric attributes and appends a local class", () => {
  render(
    <CommunityMetrics className="compact" aria-label="指标" role="status" data-testid="metrics">
      内容
    </CommunityMetrics>,
  );

  const metrics = screen.getByTestId("metrics");
  expect(metrics.className).toContain("compact");
  expect(metrics.getAttribute("aria-label")).toBe("指标");
  expect(metrics.getAttribute("role")).toBe("status");
});

test("formats the shared count and rating metrics for different count labels", () => {
  const { rerender } = render(
    <CommunityRatingMetrics count={1234} countLabel="下载" ratingCount={8} ratingAverage={4.5} />,
  );
  expect(screen.getByText("1,234 人下载 · 4.5 分 · 8 人评分")).not.toBeNull();

  rerender(
    <CommunityRatingMetrics count={12} countLabel="收藏" ratingCount={2} ratingAverage={5} />,
  );
  expect(screen.getByText("12 人收藏 · 5.0 分 · 2 人评分")).not.toBeNull();

  rerender(
    <CommunityRatingMetrics count={0} countLabel="下载" ratingCount={0} ratingAverage={0} />,
  );
  expect(screen.getByText("0 人下载 · 暂无评分 · 0 人评分")).not.toBeNull();
});
