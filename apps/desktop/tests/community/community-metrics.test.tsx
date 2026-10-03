// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityMetrics } from "@msime/ui";

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
