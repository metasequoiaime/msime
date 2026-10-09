import { expect, test, vi } from "vitest";
import {
  cloudDictionaryCapabilities,
  isMobileHost,
} from "../../src/input/mobile-host-capabilities";

test("mobile capability checks use the declared host platform", () => {
  expect(isMobileHost("android")).toBe(true);
  expect(isMobileHost("ios")).toBe(true);
  expect(isMobileHost("macos")).toBe(false);
  expect(isMobileHost(undefined)).toBe(false);
});

test("cloud dictionary snapshots distinguish mobile queue and macOS native paths", () => {
  expect(cloudDictionaryCapabilities("android")).toEqual({ snapshot: true, snapshotNative: false });
  expect(cloudDictionaryCapabilities("ios")).toEqual({ snapshot: true, snapshotNative: false });
  expect(cloudDictionaryCapabilities("macos")).toEqual({ snapshot: true, snapshotNative: true });
  expect(cloudDictionaryCapabilities("windows")).toEqual({
    snapshot: false,
    snapshotNative: false,
  });
});

test("macOS restores use one native file picker", async () => {
  const request = vi.fn().mockResolvedValue({ saved: false });
  const macos = cloudDictionaryCapabilities("macos", request);
  const android = cloudDictionaryCapabilities("android", request);
  expect(typeof macos.chooseSnapshotRestore).toBe("function");
  expect(android.chooseSnapshotRestore).toBeUndefined();
  await expect(macos.chooseSnapshotRestore?.()).resolves.toEqual({ saved: false });
  expect(request).toHaveBeenCalledExactlyOnceWith({ operation: "snapshot_choose_restore" });
});
