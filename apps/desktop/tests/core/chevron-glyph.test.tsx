// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { ChevronGlyph } from "@msime/ui";

afterEach(cleanup);

test("共享右向图标保留路径和装饰性属性", () => {
  const { container } = render(<ChevronGlyph size={13} strokeWidth={2.6} />);
  const svg = container.querySelector("svg");
  expect(svg).toBeTruthy();
  expect(svg?.getAttribute("viewBox")).toBe("0 0 24 24");
  expect(svg?.getAttribute("stroke-width")).toBe("2.6");
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
  expect(svg?.getAttribute("focusable")).toBe("false");
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("m9 6 6 6-6 6");
});

test("共享右向图标支持调用点的尺寸和颜色类名", () => {
  const { container } = render(<ChevronGlyph size={14} className="flex-none text-muted" />);
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("width")).toBe("14");
  expect(svg?.getAttribute("height")).toBe("14");
  expect(svg?.classList.contains("flex-none")).toBe(true);
  expect(svg?.classList.contains("text-muted")).toBe(true);
});
