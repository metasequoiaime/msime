import { expect, test } from "vitest";
import { bytesToBase64 } from "../../../../packages/ui/src/core/base64";

test.each([
  [[], ""],
  [[102], "Zg=="],
  [[102, 111], "Zm8="],
  [[102, 111, 111], "Zm9v"],
  [[102, 111, 111, 98], "Zm9vYg=="],
  [[102, 111, 111, 98, 97], "Zm9vYmE="],
  [[102, 111, 111, 98, 97, 114], "Zm9vYmFy"],
  [[0, 1, 255, 254, 253], "AAH//v0="],
] as const)("encodes the bytes %j with standard padding", (bytes, encoded) => {
  expect(bytesToBase64(bytes)).toBe(encoded);
  expect(bytesToBase64(new Uint8Array(bytes))).toBe(encoded);
});

test.each([24575, 24576, 24577, 49154, 3 * 1024 * 1024 + 2])(
  "encodes %s bytes across chunks without extra padding or a stack overflow",
  (length) => {
    const bytes = Uint8Array.from({ length }, (_, index) => (index * 31 + 7) & 0xff);
    // 对照整段一次编码，独立检查分块边界和填充。
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    const expected = btoa(binary);
    expect(bytesToBase64(bytes)).toBe(expected);
    expect(bytesToBase64(Object.freeze(Array.from(bytes)))).toBe(expected);
  },
);

test("encodes only the selected view without changing the backing bytes", () => {
  const backing = new Uint8Array([99, 0, 1, 255, 88]);
  expect(bytesToBase64(backing.subarray(1, 4))).toBe("AAH/");
  expect(Array.from(backing)).toEqual([99, 0, 1, 255, 88]);
});
