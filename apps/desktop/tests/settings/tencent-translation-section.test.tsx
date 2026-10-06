// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TencentTranslationSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards credential edits and leaves choosing the service to 翻译服务", () => {
  const onSecretIdChange = vi.fn();
  const onSecretKeyChange = vi.fn();
  const onRegionChange = vi.fn();
  render(
    <TencentTranslationSection
      available
      secretId="synthetic-id"
      secretKey="synthetic-key"
      region="synthetic-region"
      onSecretIdChange={onSecretIdChange}
      onSecretKeyChange={onSecretKeyChange}
      onRegionChange={onRegionChange}
    />,
  );

  // 服务只能从「翻译服务」下拉框开启，所以它自己的设置里没有开关。
  expect(screen.queryByRole("switch")).toBeNull();
  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), {
    target: { value: "updated-id" },
  });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), {
    target: { value: "updated-key" },
  });
  fireEvent.change(screen.getByLabelText("腾讯云地域"), {
    target: { value: "updated-region" },
  });

  expect(onSecretIdChange).toHaveBeenCalledWith("updated-id");
  expect(onSecretKeyChange).toHaveBeenCalledWith("updated-key");
  expect(onRegionChange).toHaveBeenCalledWith("updated-region");
});

test("shows validation and missing credential warnings", () => {
  const { rerender } = render(
    <TencentTranslationSection
      available
      secretId="bad id"
      secretKey="synthetic-key"
      region="synthetic-region"
      credentialIssue="SecretId 只能包含字母、数字、下划线和连字符。"
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("status").textContent).toContain("SecretId");
  rerender(
    <TencentTranslationSection
      available
      secretId=""
      secretKey=""
      region="synthetic-region"
      showMissingCredentialsWarning
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );
  expect(screen.getByRole("status").textContent).toContain("未填写腾讯云凭据");
});

test("disables fields when candidate translations are unavailable", () => {
  render(
    <TencentTranslationSection
      available={false}
      secretId="synthetic-id"
      secretKey="synthetic-key"
      region="synthetic-region"
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );

  expect((screen.getByLabelText("腾讯云 SecretKey") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("腾讯云 SecretId") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("腾讯云地域") as HTMLInputElement).disabled).toBe(true);
});
