import { expect, test } from "vitest";
import { accountMessage } from "../../../../packages/ui/src/account/account-errors";
import contract from "../../../../shared/contracts/account-errors/cases.json";

// 各平台共用的账号错误码用例（shared/contracts/account-errors/cases.json）：Rust 发的每个码在共享账号页上说成 `shared` 那一句，Windows 原生设置的 AccountPageModel 跑同一份。共享页不读 Retry-After。
test.each(contract.cases)("$code reads as the shared sentence", ({ code, shared }) => {
  expect(accountMessage({ code })).toBe(shared);
});

test("an error without a code is the generic failure", () => {
  expect(accountMessage(new Error("synthetic"))).toBe("账号服务暂不可用，请稍后再试。");
});
