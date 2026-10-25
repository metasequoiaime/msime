import { expect, test, vi } from "vitest";
import { canvasToBytes } from "../../../../packages/ui/src/core/canvas-to-bytes";

test("等待浏览器完成编码后交回真实 Blob 字节，并透传格式和质量", async () => {
  let complete: BlobCallback = () => {
    throw new Error("合成编码回调尚未登记");
  };
  const toBlob = vi.fn((callback: BlobCallback) => {
    complete = callback;
  });
  const createError = vi.fn(() => new Error("合成编码错误"));
  const result = canvasToBytes({ toBlob } as unknown as HTMLCanvasElement, {
    type: "image/jpeg",
    quality: 0.75,
    createError,
  });
  expect(toBlob).toHaveBeenCalledExactlyOnceWith(expect.any(Function), "image/jpeg", 0.75);
  complete(new Blob([new Uint8Array([0, 127, 255])]));
  await expect(result).resolves.toEqual(new Uint8Array([0, 127, 255]));
  expect(createError).not.toHaveBeenCalled();
});

test("只有浏览器返回空结果时才创建并拒绝调用方的错误", async () => {
  const error = new RangeError("合成空编码结果");
  const createError = vi.fn(() => error);
  let complete: BlobCallback = () => {};
  const canvas = {
    toBlob(callback: BlobCallback) {
      complete = callback;
    },
  } as HTMLCanvasElement;
  const result = canvasToBytes(canvas, { type: "image/png", createError });
  expect(createError).not.toHaveBeenCalled();
  complete(null);
  await expect(result).rejects.toBe(error);
  expect(createError).toHaveBeenCalledTimes(1);
});

test("Blob 读取拒绝时保留原错误，不替换成编码错误", async () => {
  const failure = new Error("合成读取失败");
  const blob = new Blob(["synthetic"]);
  vi.spyOn(blob, "arrayBuffer").mockRejectedValueOnce(failure);
  const createError = vi.fn(() => new Error("合成编码失败"));
  const canvas = { toBlob: (callback: BlobCallback) => callback(blob) } as HTMLCanvasElement;
  await expect(canvasToBytes(canvas, { type: "image/png", createError })).rejects.toBe(failure);
  expect(createError).not.toHaveBeenCalled();
});

test("浏览器编码同步抛错时通过 Promise 拒绝交回原错误", async () => {
  const failure = new Error("合成画布不可编码");
  const canvas = {
    toBlob() {
      throw failure;
    },
  } as unknown as HTMLCanvasElement;
  await expect(
    canvasToBytes(canvas, { type: "image/png", createError: () => new Error("合成空结果") }),
  ).rejects.toBe(failure);
});
