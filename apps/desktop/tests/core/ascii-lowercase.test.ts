import { expect, test } from "vitest";
import { asciiLowercase } from "../../../../packages/ui/src/core/text";

test.each([
  ["", ""],
  ["ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz"],
  ["already-lowercase.exe", "already-lowercase.exe"],
  ["@AZ[\\]^_`az{|}~ 019\n", "@az[\\]^_`az{|}~ 019\n"],
  ["ÄİΣẞＡ中文👩‍💻CODE.EXE", "ÄİΣẞＡ中文👩‍💻code.exe"],
])("ASCII lowercase preserves the rest of %j", (value, expected) => {
  expect(asciiLowercase(value)).toBe(expected);
});
