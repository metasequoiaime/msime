import { expect, test } from "vitest";
import { formatZhNumber } from "@msime/ui";

test("formats shared Chinese UI counts", () => {
  expect(formatZhNumber(0)).toBe("0");
  expect(formatZhNumber(12_345_678)).toBe("12,345,678");
});
