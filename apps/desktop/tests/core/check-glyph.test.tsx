// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CheckGlyph } from "@msime/ui";

afterEach(cleanup);

test("共享勾选图标保留默认装饰属性和路径", () => {
  const { container } = render(<CheckGlyph />);
  const svg = container.querySelector("svg");
  expect(svg).toBeTruthy();
  expect(svg?.getAttribute("viewBox")).toBe("0 0 24 24");
  expect(svg?.getAttribute("stroke-width")).toBe("3");
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("m5 12.5 4.5 4.5L19 7.5");
});

test("共享勾选图标支持引导层的尺寸和线宽", () => {
  const { container } = render(<CheckGlyph size={18} strokeWidth={2.4} className="guide-check" />);
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("width")).toBe("18");
  expect(svg?.getAttribute("height")).toBe("18");
  expect(svg?.getAttribute("stroke-width")).toBe("2.4");
  expect(svg?.classList.contains("guide-check")).toBe(true);
});
