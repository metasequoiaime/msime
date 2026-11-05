// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render } from "@testing-library/react";
import {
  MsimeMark,
  msimeEdgeWidth,
  msimeFramePath,
  msimeStrokePath,
  msimeStrokeWidth,
} from "../../../../packages/ui/src/core/brand-logo";
import { hSplashStroke, splashStroke } from "../../../../packages/ui/src/account/onboarding-style";

afterEach(cleanup);

const sources = import.meta.glob<string>(
  [
    "../../../../packages/ui/src/assets/msime.svg",
    "../../../../packages/ui/src/keyboard/screen-keyboard-preview.tsx",
  ],
  { eager: true, query: "?raw", import: "default" },
);
const raw = (path: string) => Object.entries(sources).find(([key]) => key.endsWith(path))![1];

const svg = new DOMParser().parseFromString(raw("assets/msime.svg"), "image/svg+xml");
const sourceRect = svg.querySelector("rect")!;
const sourceStroke = svg.querySelector("path")!;
const attr = (element: Element, name: string) => Number(element.getAttribute(name));

/** The commands of an absolute path, so two paths compare letter by letter and number by number. */
const tokens = (d: string) => d.match(/[A-Za-z]|-?\d*\.?\d+/g)!;

// msime.svg's frame is a 100 square at (5, 5); the code-drawn mark's frame is 120 at (-2.1569, 5.8335) in its 132 view box.
const scale = 1.2;
const origin = { x: -2.1569, y: 5.8335 };

test("the code-drawn frame is msime.svg's frame scaled 1.2", () => {
  expect(attr(sourceRect, "width") * scale).toBeCloseTo(120, 4);
  expect(attr(sourceRect, "height") * scale).toBeCloseTo(120, 4);
  expect(attr(sourceRect, "stroke-width") * scale).toBeCloseTo(msimeEdgeWidth, 4);
  const numbers = tokens(msimeFramePath)
    .filter((token) => !/[A-Za-z]/.test(token))
    .map(Number);
  const xs = numbers.filter((_, index) => index % 2 === 0);
  const ys = numbers.filter((_, index) => index % 2 === 1);
  expect(Math.min(...xs)).toBeCloseTo(origin.x, 3);
  expect(Math.min(...ys)).toBeCloseTo(origin.y, 3);
  expect(Math.max(...xs) - Math.min(...xs)).toBeCloseTo(120, 2);
  expect(Math.max(...ys) - Math.min(...ys)).toBeCloseTo(120, 2);
});

test("the code-drawn stroke is msime.svg's stroke mapped 1.2 into the frame", () => {
  const source = tokens(sourceStroke.getAttribute("d")!);
  const mapped = tokens(msimeStrokePath);
  expect(mapped.length).toBe(source.length);
  let coordinate = 0;
  source.forEach((token, index) => {
    if (/[A-Za-z]/.test(token)) {
      expect(token, "msime.svg's stroke uses only absolute commands").toMatch(/^[MLC]$/);
      expect(mapped[index]).toBe(token);
      return;
    }
    const o = coordinate % 2 === 0 ? origin.x : origin.y;
    expect(Number(mapped[index])).toBeCloseTo(o + (Number(token) - 5) * scale, 3);
    coordinate += 1;
  });
  expect(msimeStrokeWidth).toBeCloseTo(attr(sourceStroke, "stroke-width") * scale, 6);
  expect(sourceStroke.getAttribute("stroke-linecap")).toBe("round");
  // No join in msime.svg means SVG's default miter join with miter limit 4: the three vertices are sharp points.
  expect(sourceStroke.hasAttribute("stroke-linejoin")).toBe(false);
  expect(sourceStroke.hasAttribute("stroke-miterlimit")).toBe(false);
});

test("MsimeMark draws the stroke with msime.svg's width, round caps and miter joins", () => {
  const { container } = render(<MsimeMark size={44} />);
  const stroke = container.querySelectorAll("path")[1]!;
  expect(stroke.getAttribute("d")).toBe(msimeStrokePath);
  expect(Number(stroke.getAttribute("stroke-width"))).toBeCloseTo(9.6, 6);
  expect(stroke.getAttribute("stroke-linecap")).toBe("round");
  expect(stroke.hasAttribute("stroke-linejoin")).toBe(false);
  expect(stroke.hasAttribute("stroke-miterlimit")).toBe(false);
});

test("the splash strokes and the keyboard thumbnail's brand slot use the same stroke", () => {
  for (const classes of [splashStroke, hSplashStroke]) {
    expect(classes).toContain(`[stroke-width:${msimeStrokeWidth}]`);
    expect(classes).toContain("[stroke-linecap:round]");
    expect(classes).not.toContain("stroke-linejoin");
    expect(classes).not.toContain("stroke-miterlimit");
  }
  const preview = raw("keyboard/screen-keyboard-preview.tsx");
  const slot = preview.slice(
    preview.indexOf("d={msimeStrokePath}"),
    preview.indexOf("/>", preview.indexOf("d={msimeStrokePath}")),
  );
  expect(slot).toContain("strokeWidth={msimeStrokeWidth}");
  expect(slot).toContain('strokeLinecap="round"');
  expect(slot).not.toContain("strokeLinejoin");
});
