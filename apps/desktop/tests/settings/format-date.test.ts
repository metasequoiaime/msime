import { expect, test } from "vitest";
import { formatZhDate } from "@msime/ui";

test("formats valid dates with the shared Chinese date locale", () => {
  expect(formatZhDate("2026-10-04T12:00:00Z")).toBe(
    new Date("2026-10-04T12:00:00Z").toLocaleDateString("zh-CN"),
  );
});

test("returns an empty label for invalid dates", () => {
  expect(formatZhDate("not-a-date")).toBe("");
});
