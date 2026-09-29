import { expect, test } from "vitest";
import { clipboardHistoryEnabled } from "@msime/ui";

test("clipboard history is always enabled on iOS", () => {
  expect(clipboardHistoryEnabled(true)).toBe(true);
  expect(clipboardHistoryEnabled(true, { clipboard_history: false })).toBe(true);
});

test("desktop clipboard history follows the saved preference", () => {
  expect(clipboardHistoryEnabled(false)).toBe(false);
  expect(clipboardHistoryEnabled(false, { clipboard_history: false })).toBe(false);
  expect(clipboardHistoryEnabled(false, { clipboard_history: true })).toBe(true);
});
