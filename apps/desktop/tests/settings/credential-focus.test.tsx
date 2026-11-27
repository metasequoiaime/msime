// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SecretInput } from "../../../../packages/ui/src/core/secret-input";
import { PasswordSettingField } from "../../../../packages/ui/src/settings/password-setting-field";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("macOS 设置窗口失焦时释放密码焦点，保留值", () => {
  render(
    <div data-settings-shell data-platform="mac">
      <PasswordSettingField label="凭据" inputLabel="凭据" value="synthetic" onChange={vi.fn()} />
    </div>,
  );
  const input = screen.getByLabelText<HTMLInputElement>("凭据");
  input.focus();

  fireEvent.blur(window);

  expect(document.activeElement).not.toBe(input);
  expect(input.type).toBe("password");
  expect(input.value).toBe("synthetic");
});

test("macOS 设置从密码切到明文前先失焦", () => {
  render(
    <div data-settings-shell data-platform="mac">
      <SecretInput label="凭据" value="synthetic" onChange={vi.fn()} />
    </div>,
  );
  const input = screen.getByLabelText<HTMLInputElement>("凭据");
  const typesAtBlur: string[] = [];
  input.addEventListener("blur", () => typesAtBlur.push(input.type));
  input.focus();

  fireEvent.click(screen.getByRole("button", { name: "显示凭据" }));

  expect(typesAtBlur).toEqual(["password"]);
  expect(input.type).toBe("text");
  expect(input.value).toBe("synthetic");
});

test("macOS 设置切页会在密码节点移除之前主动失焦", () => {
  const { unmount } = render(
    <div data-settings-shell data-platform="mac">
      <SecretInput label="凭据" value="" onChange={vi.fn()} />
    </div>,
  );
  const input = screen.getByLabelText<HTMLInputElement>("凭据");
  input.focus();
  const blur = vi.spyOn(input, "blur");

  unmount();

  expect(blur).toHaveBeenCalledOnce();
});

test("Linux 设置窗口失焦保持原来的凭据编辑状态", () => {
  render(
    <div data-settings-shell data-platform="linux">
      <SecretInput label="凭据" value="" onChange={vi.fn()} />
    </div>,
  );
  const input = screen.getByLabelText<HTMLInputElement>("凭据");
  input.focus();

  fireEvent.blur(window);

  expect(document.activeElement).toBe(input);
});
