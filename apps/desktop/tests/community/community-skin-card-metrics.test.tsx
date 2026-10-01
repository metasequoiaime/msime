// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { CommunitySkinCardMetrics } from "../../../../packages/ui/src/community/community-skin-card-metrics";

test("renders localized download and rating metrics", () => {
  render(<CommunitySkinCardMetrics downloads={12345} ratingCount={67} ratingAverage={4.5} />);

  expect(screen.getByText("↓ 12,345")).toBeTruthy();
  expect(screen.getByText("☆ 4.5 分")).toBeTruthy();
});
