// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  AccountConfirmation,
  type AccountConfirmationAction,
} from "../../../../packages/ui/src/account/account-confirmation";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test.each<[AccountConfirmationAction, string, string]>([
  ["logout", "确认退出登录", "退出登录后，云端功能和社区发布都需要重新登录才能使用。"],
  ["logout-all", "确认退出所有设备", "退出所有设备后，所有设备都需要重新登录。"],
  ["relogin", "确认重新登录", "清除本机登录状态后需要重新登录。"],
  ["delete", "确认注销账号", "注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。"],
])("explains and confirms %s", (action, confirmLabel, message) => {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  render(
    <AccountConfirmation action={action} busy={false} onConfirm={onConfirm} onCancel={onCancel} />,
  );

  expect(screen.getByRole("alertdialog").textContent).toContain(message);
  fireEvent.click(screen.getByRole("button", { name: confirmLabel }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onConfirm).toHaveBeenCalledTimes(1);
  expect(onCancel).toHaveBeenCalledTimes(1);
});

test("allows the mobile compact confirmation label", () => {
  render(
    <AccountConfirmation
      action="delete"
      busy={false}
      confirmLabel="确认"
      onConfirm={() => undefined}
      onCancel={() => undefined}
    />,
  );

  expect(screen.getByRole("button", { name: "确认" })).not.toBeNull();
});
