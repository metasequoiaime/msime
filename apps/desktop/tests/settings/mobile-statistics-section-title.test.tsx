// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { MobileStatisticsSectionTitle } from "../../../../packages/ui/src/settings/mobile-statistics-section-title";

afterEach(cleanup);

test("mobile statistics section title keeps the heading and trailing control in one row", () => {
  render(
    <MobileStatisticsSectionTitle
      title="按键热力图"
      trailing={<button type="button">切换</button>}
    />,
  );

  const heading = screen.getByRole("heading", { level: 3, name: "按键热力图" });
  expect(heading.parentElement?.className).toContain("justify-between");
  expect(screen.getByRole("button", { name: "切换" })).toBeTruthy();
});
