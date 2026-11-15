// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountVerificationForm } from "../../../../packages/ui/src/account/account-verification-form";

afterEach(cleanup);

test("the shared verification form keeps page and sheet controls distinct", () => {
  const updateTarget = vi.fn();
  const setCode = vi.fn();
  const requestCode = vi.fn();
  const signIn = vi.fn();
  const verification = {
    channel: "email" as const,
    target: "synthetic@example.test",
    updateTarget,
    code: "123456",
    setCode,
    challenge: { challengeId: "synthetic-challenge", expiresIn: 300 },
    resendSeconds: 0,
    expired: false,
  };

  const { rerender } = render(
    <AccountVerificationForm
      variant="page"
      verification={verification}
      busy={false}
      onRequestCode={requestCode}
      onSignIn={signIn}
    />,
  );
  expect(screen.getByRole("button", { name: "获取验证码" })).not.toBeNull();
  expect(screen.getByText("邮箱地址")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "登录" }));
  expect(signIn).toHaveBeenCalledOnce();

  fireEvent.keyDown(screen.getByRole("textbox", { name: "6 位验证码" }), { key: "Enter" });
  expect(signIn).toHaveBeenCalledTimes(2);

  fireEvent.keyDown(screen.getByRole("textbox", { name: "6 位验证码" }), {
    key: "Enter",
    keyCode: 229,
  });
  expect(signIn).toHaveBeenCalledTimes(2);

  rerender(
    <AccountVerificationForm
      variant="sheet"
      verification={verification}
      busy={false}
      onRequestCode={requestCode}
      onSignIn={signIn}
    />,
  );
  expect(screen.getByRole("button", { name: "重新发送验证码" })).not.toBeNull();
  expect(screen.getByRole("textbox", { name: "邮箱地址" }).getAttribute("placeholder")).toBe(
    "name@example.com",
  );
  expect(screen.getByText("邮箱地址").className).toContain("sr-only");
  fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "next@example.test" },
  });
  expect(updateTarget).toHaveBeenCalledWith("next@example.test");
  expect(screen.getByText("验证码只用于本次登录，请勿向他人透露。")).not.toBeNull();
});
