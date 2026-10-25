import { afterEach, expect, test, vi } from "vitest";
import {
  DraftImageError,
  readDraftImage,
} from "../../../../packages/ui/src/skin/candidate-skin-images";
import { renderSkinPreview } from "../../../../packages/ui/src/skin/skin-preview-render";
import type { ExternalSkin } from "../../../../packages/ui/src/skin/external-skins";

afterEach(() => vi.unstubAllGlobals());

const png = [137, 80, 78, 71, 13, 10, 26, 10];
const jpeg = [255, 216, 255];
const skin: ExternalSkin = {
  id: "synthetic-preview",
  name: "合成预览",
  version: "1",
  base: "system",
  author: null,
  description: null,
  layouts: ["horizontal"],
  themes: ["light"],
  minWidthDip: 0,
  decorationTopDip: 0,
  decorationWidthDip: 0,
  toolbarStylesheet: null,
  preview: null,
  candidate: { dark: {}, light: {} },
};

function canvasBoundary(results: (number[] | null)[]) {
  const context = {
    scale() {},
    fillRect() {},
    save() {},
    beginPath() {},
    roundRect() {},
    fill() {},
    clip() {},
    restore() {},
    stroke() {},
    fillText() {},
    drawImage() {},
    measureText: () => ({ width: 12 }),
  };
  const toBlob = vi.fn((callback: BlobCallback, _type?: string, _quality?: number) => {
    const bytes = results.shift();
    if (bytes === undefined) throw new Error("合成编码结果已用尽");
    queueMicrotask(() => callback(bytes === null ? null : new Blob([new Uint8Array(bytes)])));
  });
  const canvas = { width: 0, height: 0, getContext: () => context, toBlob };
  vi.stubGlobal("document", { createElement: () => canvas });
  vi.stubGlobal(
    "Image",
    class {
      naturalWidth = 4096;
      naturalHeight = 2048;
      async decode() {}
    },
  );
  return { canvas, toBlob };
}

test.each([
  [png, "image/png", undefined],
  [jpeg, "image/jpeg", 0.88],
] as const)("草稿压缩保留 %s 的格式、质量与尺寸上限", async (source, type, quality) => {
  const { canvas, toBlob } = canvasBoundary([[1, 2, 3]]);
  const image = await readDraftImage(new Blob([new Uint8Array(source)]));
  expect(image).toMatchObject({
    contentType: type,
    width: 2048,
    height: 1024,
    data: "AQID",
    byteLength: 3,
    animated: false,
  });
  expect(canvas.width).toBe(2048);
  expect(toBlob).toHaveBeenCalledExactlyOnceWith(expect.any(Function), type, quality);
});

test("草稿编码没有结果时保留专用错误类型和文案", async () => {
  canvasBoundary([null]);
  const result = readDraftImage(new Blob([new Uint8Array(png)]));
  await expect(result).rejects.toBeInstanceOf(DraftImageError);
  await expect(result).rejects.toThrow("无法压缩这张图片。");
});

test("预览优先输出 PNG，只有超过 256 KiB 时才按原质量转 JPEG", async () => {
  const { toBlob } = canvasBoundary([new Array(256 * 1024 + 1).fill(0), jpeg]);
  const read = vi.fn();
  await expect(renderSkinPreview(skin, read)).resolves.toEqual(jpeg);
  expect(toBlob).toHaveBeenNthCalledWith(1, expect.any(Function), "image/png", undefined);
  expect(toBlob).toHaveBeenNthCalledWith(2, expect.any(Function), "image/jpeg", 0.85);
  expect(read).not.toHaveBeenCalled();
});

test("预览在 PNG 上限处直接返回，不触发 JPEG 编码", async () => {
  const bytes = new Array(256 * 1024).fill(7);
  const { toBlob } = canvasBoundary([bytes]);
  await expect(renderSkinPreview(skin, vi.fn())).resolves.toEqual(bytes);
  expect(toBlob).toHaveBeenCalledTimes(1);
});

test("预览编码没有结果时保留原错误，JPEG 仍超限时继续拒绝", async () => {
  canvasBoundary([null]);
  await expect(renderSkinPreview(skin, vi.fn())).rejects.toThrow("preview encoding failed");
  const oversized = new Array(256 * 1024 + 1).fill(0);
  canvasBoundary([oversized, oversized]);
  await expect(renderSkinPreview(skin, vi.fn())).rejects.toThrow("preview too large");
});
