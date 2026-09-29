// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TencentTranslationSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards provider toggle and credential edits", () => {
  const onToggle = vi.fn();
  const onSecretIdChange = vi.fn();
  const onSecretKeyChange = vi.fn();
  const onRegionChange = vi.fn();
  render(
    <TencentTranslationSection
      enabled
      available
      secretId="synthetic-id"
      secretKey="synthetic-key"
      region="synthetic-region"
      onToggle={onToggle}
      onSecretIdChange={onSecretIdChange}
      onSecretKeyChange={onSecretKeyChange}
      onRegionChange={onRegionChange}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "腾讯云机器翻译" }));
  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), {
    target: { value: "updated-id" },
  });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), {
    target: { value: "updated-key" },
  });
  fireEvent.change(screen.getByLabelText("腾讯云地域"), {
    target: { value: "updated-region" },
  });

  expect(onToggle).toHaveBeenCalledWith(false);
  expect(onSecretIdChange).toHaveBeenCalledWith("updated-id");
  expect(onSecretKeyChange).toHaveBeenCalledWith("updated-key");
  expect(onRegionChange).toHaveBeenCalledWith("updated-region");
});

test("shows validation and missing credential warnings", () => {
  const { rerender } = render(
    <TencentTranslationSection
      enabled
      available
      secretId="bad id"
      secretKey="synthetic-key"
      region="synthetic-region"
      credentialIssue="SecretId 只能包含字母、数字、下划线和连字符。"
      onToggle={vi.fn()}
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("status").textContent).toContain("SecretId");
  rerender(
    <TencentTranslationSection
      enabled
      available
      secretId=""
      secretKey=""
      region="synthetic-region"
      showMissingCredentialsWarning
      onToggle={vi.fn()}
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
      enabled
      available={false}
      secretId="synthetic-id"
      secretKey="synthetic-key"
      region="synthetic-region"
      onToggle={vi.fn()}
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );

  expect(
    (screen.getByRole("switch", { name: "腾讯云机器翻译" }) as HTMLInputElement).disabled,
  ).toBe(true);
  expect((screen.getByLabelText("腾讯云 SecretId") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("腾讯云地域") as HTMLInputElement).disabled).toBe(true);
});
