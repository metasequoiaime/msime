// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SearchGlyph } from "@msime/ui";

afterEach(cleanup);

test("共享搜索图标保留默认路径和装饰性属性", () => {
  const { container } = render(<SearchGlyph />);
  const svg = container.querySelector("svg");
  expect(svg).toBeTruthy();
  expect(svg?.getAttribute("viewBox")).toBe("0 0 16 16");
  expect(svg?.getAttribute("stroke-width")).toBe("1.8");
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
  expect(svg?.getAttribute("focusable")).toBe("false");
  expect(svg?.querySelector("circle")?.getAttribute("cx")).toBe("7");
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("m11 11 3.5 3.5");
});

test("共享搜索图标支持调用点的尺寸、线宽和类名", () => {
  const { container } = render(<SearchGlyph size={16} strokeWidth={1.5} className="shrink-0" />);
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("width")).toBe("16");
  expect(svg?.getAttribute("height")).toBe("16");
  expect(svg?.getAttribute("stroke-width")).toBe("1.5");
  expect(svg?.classList.contains("shrink-0")).toBe(true);
});
