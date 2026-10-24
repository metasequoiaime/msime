// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CloseGlyph } from "@msime/ui";

afterEach(cleanup);

test("共享关闭图标保留默认路径和装饰性属性", () => {
  const { container } = render(<CloseGlyph />);
  const svg = container.querySelector("svg");
  expect(svg).toBeTruthy();
  expect(svg?.getAttribute("viewBox")).toBe("0 0 24 24");
  expect(svg?.getAttribute("stroke-width")).toBe("2.6");
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
  expect(svg?.getAttribute("focusable")).toBe("false");
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("M6 6l12 12M18 6 6 18");
});

test("共享关闭图标支持调用点的尺寸和类名", () => {
  const { container } = render(<CloseGlyph size={10} className="shrink-0" />);
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("width")).toBe("10");
  expect(svg?.getAttribute("height")).toBe("10");
  expect(svg?.classList.contains("shrink-0")).toBe(true);
});
