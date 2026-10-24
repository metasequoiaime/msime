// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type SectionProps = {
  id: string;
  title: ReactNode;
  children: ReactNode;
  className?: string;
};

test("statistics section connects its heading and preserves a custom shell", () => {
  const Section = (ui as unknown as { StatisticsSection: ComponentType<SectionProps> })
    .StatisticsSection;
  expect(Section).toBeDefined();

  render(
    <Section id="statistics-test" title="测试统计" className="section m-0 @container">
      <p>内容</p>
    </Section>,
  );

  const section = screen.getByRole("region", { name: "测试统计" });
  expect(section.className).toContain("@container");
  expect(section.getAttribute("aria-labelledby")).toBe("statistics-test");
  expect(screen.getByRole("heading", { name: "测试统计", level: 2 }).id).toBe("statistics-test");
  expect(section.textContent).toContain("内容");
});
