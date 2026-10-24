// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { PlusGlyph } from "@msime/ui";

afterEach(cleanup);

test("共享加号图标的 compact 变体保留词库行几何", () => {
  const { container } = render(<PlusGlyph />);
  const svg = container.querySelector("svg");
  expect(svg).toBeTruthy();
  expect(svg?.getAttribute("viewBox")).toBe("0 0 16 16");
  expect(svg?.getAttribute("stroke-width")).toBe("1.6");
  expect(svg?.getAttribute("aria-hidden")).toBe("true");
  expect(svg?.getAttribute("focusable")).toBe("false");
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("M8 2.5v11M2.5 8h11");
});

test("共享加号图标的 row 变体支持语言行尺寸和线宽", () => {
  const { container } = render(
    <PlusGlyph variant="row" size={20} strokeWidth={2.2} className="shrink-0" />,
  );
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe("0 0 24 24");
  expect(svg?.getAttribute("width")).toBe("20");
  expect(svg?.getAttribute("height")).toBe("20");
  expect(svg?.getAttribute("stroke-width")).toBe("2.2");
  expect(svg?.classList.contains("shrink-0")).toBe(true);
  expect(svg?.querySelector("path")?.getAttribute("d")).toBe("M12 5v14M5 12h14");
});
