import { expect, test } from "vitest";
import {
  isValidAccountName,
  normalizeAccountName,
} from "../../../../packages/ui/src/account/account-labels";

test("normalizes account names before validation", () => {
  expect(normalizeAccountName("  水杉用户  ")).toBe("水杉用户");
  expect(isValidAccountName("  水杉用户  ")).toBe(true);
});

test("rejects empty, oversized, and control-character account names", () => {
  expect(isValidAccountName("   ")).toBe(false);
  expect(isValidAccountName("a".repeat(64))).toBe(true);
  expect(isValidAccountName("a".repeat(65))).toBe(false);
  expect(isValidAccountName("水杉\n用户")).toBe(false);
});
