import { expect, test, vi } from "vitest";
import { discoverFontReader } from "../../src/candidate/system-font-client";

test("browser does not probe the host", async () => {
  const invoke = vi.fn();
  expect(await discoverFontReader(false, invoke)).toBeUndefined();
  expect(invoke).not.toHaveBeenCalled();
});

test("unsupported hosts retain manual entry", async () => {
  expect(await discoverFontReader(true, vi.fn().mockResolvedValue(false))).toBeUndefined();
});

test("supported host enumerates lazily and refreshes on every read", async () => {
  const invoke = vi.fn().mockResolvedValueOnce(true).mockResolvedValue(["示例字体"]);
  const reader = await discoverFontReader(true, invoke);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("supports_font_catalog");
  expect(await reader!()).toEqual(["示例字体"]);
  await reader!();
  expect(invoke.mock.calls.slice(1)).toEqual([["list_font_families"], ["list_font_families"]]);
});

test("enumeration errors reach the existing retry UI", async () => {
  const invoke = vi.fn().mockResolvedValueOnce(true).mockRejectedValue(new Error("font_catalog"));
  const reader = await discoverFontReader(true, invoke);
  await expect(reader!()).rejects.toThrow("font_catalog");
});
