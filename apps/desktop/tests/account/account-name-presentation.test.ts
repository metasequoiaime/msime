import { expect, test } from "vitest";
import { parseAccountNamePresentation } from "../../../../packages/ui/src/account/account-name-presentation";
import { accountNameResult } from "../support/account-name";

test("保留共享函数的规范化、合法性和动态上限，不在界面重建规则", () => {
  const value = accountNameResult("合成🌲", {
    valid: false,
    maximum_chars: 12,
    maximum_utf16_units: 24,
  });
  expect(parseAccountNamePresentation(value)).toEqual(value);
});

test.each([
  null,
  [],
  {},
  { ...accountNameResult("合成"), valid: "true" },
  { ...accountNameResult("合成"), normalized: 1 },
  { ...accountNameResult("合成"), preferred: null },
  accountNameResult("\ud800"),
  { ...accountNameResult("合成"), preferred: "\udfff" },
  accountNameResult("合成", { maximum_chars: 0 }),
  accountNameResult("合成", { maximum_chars: 1.5 }),
  accountNameResult("合成", { maximum_utf16_units: 63 }),
  accountNameResult("合成", { maximum_utf16_units: 16385 }),
])("拒绝损坏或不完整的共享昵称结果 %#", (value) => {
  expect(() => parseAccountNamePresentation(value)).toThrow(
    expect.objectContaining({ code: "account_unavailable" }),
  );
});
