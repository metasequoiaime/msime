// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { downloadBlob } from "../../../../packages/ui/src/core/download-blob";

const createObjectURL = vi.fn();
const revokeObjectURL = vi.fn();

beforeEach(() => {
  vi.useFakeTimers();
  createObjectURL.mockReset().mockReturnValue("blob:synthetic-export");
  revokeObjectURL.mockReset();
  vi.stubGlobal("URL", { createObjectURL, revokeObjectURL });
});

afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("下载时链接已挂载，下载后移除链接并延迟释放文件", () => {
  const blob = new Blob(["合成词条\tsynthetic\n"], { type: "text/plain;charset=utf-8" });
  const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    expect(this.isConnected).toBe(true);
    expect(this.href).toBe("blob:synthetic-export");
    expect(this.download).toBe("合成词库.tsv");
    expect(revokeObjectURL).not.toHaveBeenCalled();
  });

  downloadBlob(blob, "合成词库.tsv");

  expect(createObjectURL).toHaveBeenCalledExactlyOnceWith(blob);
  expect(click).toHaveBeenCalledOnce();
  expect(document.querySelectorAll("a[download]")).toHaveLength(0);
  expect(revokeObjectURL).not.toHaveBeenCalled();
  vi.runAllTimers();
  expect(revokeObjectURL).toHaveBeenCalledExactlyOnceWith("blob:synthetic-export");
});

test.each(["append", "click"])("%s 失败时仍回收链接和文件，并保留错误", (stage) => {
  const error = new Error("synthetic download failure");
  const anchor = document.createElement("a");
  vi.spyOn(document, "createElement").mockReturnValue(anchor);
  if (stage === "append") {
    vi.spyOn(document.body, "appendChild").mockImplementation(() => {
      throw error;
    });
  } else {
    vi.spyOn(anchor, "click").mockImplementation(() => {
      throw error;
    });
  }

  expect(() => downloadBlob(new Blob(["synthetic"]), "example.json")).toThrow(error);
  expect(anchor.isConnected).toBe(false);
  vi.runAllTimers();
  expect(revokeObjectURL).toHaveBeenCalledExactlyOnceWith("blob:synthetic-export");
});

test("连续下载分别释放各自的文件", () => {
  createObjectURL.mockReturnValueOnce("blob:first").mockReturnValueOnce("blob:second");
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

  downloadBlob(new Blob(["first"]), "first.json");
  downloadBlob(new Blob(["second"]), "second.ndjson");

  expect(document.querySelectorAll("a[download]")).toHaveLength(0);
  vi.runAllTimers();
  expect(revokeObjectURL.mock.calls).toEqual([["blob:first"], ["blob:second"]]);
});
