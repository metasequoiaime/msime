// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage, ToastProvider, type AccountClient, type AccountUser } from "@msime/ui";
import { AccountAvatar } from "../../../../packages/ui/src/account/account-avatar";
import { MobileAccountProfilePage } from "../../../../packages/ui/src/account/account-profile-pages";
import { accountNameResult, presentAccountName } from "../support/account-name";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const user = {
  id: "synthetic-display-user",
  displayName: "原始合成昵称",
  preferredName: "共享展示昵称",
  createdAt: "2026-01-01T00:00:00Z",
};

function client(): AccountClient {
  return {
    status: async () => ({ user }),
    providers: async () => ({ email: true, phone: false }),
    profile: async () => ({ user, providers: ["email"] }),
    requestCode: vi.fn(),
    login: vi.fn(),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
  };
}

test.each([undefined, "ios", "android", "harmony"] as const)(
  "%s 账号卡片直接显示共享结果，昵称校验不可用时也不重建展示规则",
  async (platform) => {
    const account = client();
    account.namePresentation = vi.fn().mockRejectedValue({ code: "account_unavailable" });
    render(
      <ToastProvider>
        <AccountPage client={account} platform={platform} />
      </ToastProvider>,
    );
    const card = await screen.findByRole("button", { name: "编辑个人资料" });
    expect(within(card).getByText(user.preferredName)).not.toBeNull();
    expect(within(card).queryByText(user.displayName)).toBeNull();
  },
);

test("Harmony 资料页与昵称行使用共享显示名，编辑框仍保留原始昵称", async () => {
  render(
    <ToastProvider>
      <AccountPage client={client()} platform="harmony" />
    </ToastProvider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  expect(await screen.findByRole("heading", { name: user.preferredName })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: `昵称，${user.preferredName}` }));
  expect(screen.getByRole("textbox", { name: "编辑社区昵称" })).toHaveProperty(
    "value",
    user.displayName,
  );
});

test("没有头像时使用共享显示名的完整首码点，并允许规范化草稿覆盖预览", () => {
  const account: AccountUser = { ...user, preferredName: "😀合成昵称" };
  const { container, rerender } = render(<AccountAvatar user={account} size="small" />);
  expect(container.textContent).toBe("😀");
  rerender(<AccountAvatar user={account} name="草稿昵称" size="small" />);
  expect(container.textContent).toBe("草");
});

test("资料页草稿等待或校验失败时使用已保存的共享头像首字，成功后才预览规范化结果", async () => {
  let reject!: (reason: unknown) => void;
  const pending = new Promise<never>((_resolve, fail) => {
    reject = fail;
  });
  const account = client();
  account.namePresentation = async (name) => {
    if (name === user.displayName) return accountNameResult("规范合成昵称");
    if (name === "  待检查昵称  ") return pending;
    return accountNameResult("新规范昵称");
  };
  render(
    <MobileAccountProfilePage
      client={account}
      user={user}
      profile={{ user, providers: ["email"] }}
      onBack={vi.fn()}
      onSignedOut={vi.fn()}
      onProfileUpdated={vi.fn()}
    />,
  );
  expect(await screen.findByText("规")).not.toBeNull();
  const input = screen.getByRole("textbox", { name: "编辑社区昵称" });
  fireEvent.change(input, { target: { value: "  待检查昵称  " } });
  expect(screen.getByText("共")).not.toBeNull();
  await act(async () => reject({ code: "account_unavailable" }));
  expect(await screen.findByText("昵称校验暂不可用，请稍后重试。")).not.toBeNull();
  expect(screen.getByText("共")).not.toBeNull();
  fireEvent.change(input, { target: { value: "  新草稿  " } });
  expect(await screen.findByText("新")).not.toBeNull();
});
