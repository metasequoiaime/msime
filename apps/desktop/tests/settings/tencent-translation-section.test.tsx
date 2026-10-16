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

test("keeps 地域 under 更多选项 and opens it only when the stored region is invalid", () => {
  const props = {
    available: true,
    secretId: "synthetic-id",
    secretKey: "synthetic-key",
    onSecretIdChange: vi.fn(),
    onSecretKeyChange: vi.fn(),
    onRegionChange: vi.fn(),
  };
  render(<TencentTranslationSection {...props} region="ap-guangzhou" />);
  const fold = () => screen.getByLabelText("腾讯云地域").closest("details") as HTMLDetailsElement;
  expect(fold().textContent).toContain("更多选项");
  expect(fold().open).toBe(false);
  // 凭据不在折叠区里。
  expect(screen.getByLabelText("腾讯云 SecretId").closest("details")).toBeNull();
  cleanup();

  render(<TencentTranslationSection {...props} region="ap guangzhou" />);
  expect(fold().open).toBe(true);
});

test("on macOS the missing credential warning says offline translation keeps working", () => {
  render(
    <TencentTranslationSection
      available
      secretId=""
      secretKey=""
      region="ap-guangzhou"
      showMissingCredentialsWarning
      offlineFallback="system"
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );
  const warning = screen.getByRole("status").textContent;
  expect(warning).toContain("填好之前候选仍用离线翻译");
  expect(warning).not.toContain("不会有任何结果");
});

test("on Windows and HarmonyOS the missing credential warning only says no online request is made", () => {
  // 那里没有系统翻译，但 host-api 照样把装好的离线词库语言放进查询，所以既不是「离线翻译」，也不是「没有任何结果」。
  render(
    <TencentTranslationSection
      available
      secretId=""
      secretKey=""
      region="ap-guangzhou"
      showMissingCredentialsWarning
      offlineFallback="dictionary"
      onSecretIdChange={vi.fn()}
      onSecretKeyChange={vi.fn()}
      onRegionChange={vi.fn()}
    />,
  );
  const warning = screen.getByRole("status").textContent;
  expect(warning).toContain("填好之前不会请求在线翻译");
  expect(warning).not.toContain("不会有任何结果");
  expect(warning).not.toContain("离线翻译。");
});
