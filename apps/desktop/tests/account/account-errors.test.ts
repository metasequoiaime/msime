import { expect, test } from "vitest";
import { accountMessage } from "../../../../packages/ui/src/account/account-errors";

test("deletion that needs a recent sign-in says so instead of the generic failure", () => {
  // Tauri 桌面的注销改用 delete_account_detailed 以后，会话不是 10 分钟内登录的是这个码，本机登录留着。
  expect(accountMessage({ code: "account_recent_login_required" })).toBe(
    "需要重新登录后再注销：请先退出登录，重新登录后再试。",
  );
  expect(accountMessage({ code: "account_network" })).toBe("网络连接失败，请检查网络后重试。");
  expect(accountMessage({ code: "account_unknown_code" })).toBe("账号服务暂不可用，请稍后再试。");
});
