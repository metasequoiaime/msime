import { expect, test } from "vitest";
import { formatZhNumber, formatZhPercent } from "@msime/ui";

test("formats shared Chinese UI counts", () => {
  expect(formatZhNumber(0)).toBe("0");
  expect(formatZhNumber(12_345_678)).toBe("12,345,678");
});

test("formats shared Chinese percentage labels", () => {
  expect(formatZhPercent(3, 8)).toBe("37.5%");
  expect(formatZhPercent(0, 0)).toBe("—");
});
