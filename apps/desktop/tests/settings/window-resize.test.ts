import { expect, test } from "vitest";
import { windowResizeEdge } from "@msime/ui";

const bounds = { left: 0, top: 0, right: 800, bottom: 600 };

test.each([
  [{ clientX: 1, clientY: 1 }, "nw"],
  [{ clientX: 799, clientY: 1 }, "ne"],
  [{ clientX: 1, clientY: 599 }, "sw"],
  [{ clientX: 799, clientY: 599 }, "se"],
  [{ clientX: 400, clientY: 1 }, "n"],
  [{ clientX: 400, clientY: 599 }, "s"],
  [{ clientX: 1, clientY: 300 }, "w"],
  [{ clientX: 799, clientY: 300 }, "e"],
] as const)("maps pointer %j to resize edge %s", (point, expected) => {
  expect(windowResizeEdge(point, bounds)).toBe(expected);
});

test("keeps the window interior out of resize mode", () => {
  expect(windowResizeEdge({ clientX: 400, clientY: 300 }, bounds)).toBeNull();
});

test("allows hosts to use a different edge threshold", () => {
  expect(windowResizeEdge({ clientX: 12, clientY: 300 }, bounds, 16)).toBe("w");
});
