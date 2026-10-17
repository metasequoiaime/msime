// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LinuxTencentCredentialsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("edits Tencent credentials and saves the entered values", () => {
  const onInputChange = vi.fn();
  const onSave = vi.fn();
  const { rerender } = render(
    <LinuxTencentCredentialsSection
      available
      status={{ tencent: null, tencentInvalid: false }}
      input={{ secretId: "", secretKey: "", region: "ap-guangzhou" }}
      busy={false}
      onInputChange={onInputChange}
      onSave={onSave}
      onClear={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByLabelText("腾讯云 SecretId"), {
    target: { value: "AKIDsynthetic" },
  });
  fireEvent.change(screen.getByLabelText("腾讯云 SecretKey"), {
    target: { value: "secret-synthetic" },
  });
  expect(onInputChange).toHaveBeenCalledWith({ secretId: "AKIDsynthetic" });
  expect(onInputChange).toHaveBeenCalledWith({ secretKey: "secret-synthetic" });
  rerender(
    <LinuxTencentCredentialsSection
      available
      status={{ tencent: null, tencentInvalid: false }}
      input={{ secretId: "AKIDsynthetic", secretKey: "secret-synthetic", region: "ap-guangzhou" }}
      busy={false}
      onInputChange={onInputChange}
      onSave={onSave}
      onClear={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect(onSave).toHaveBeenCalledWith({
    secretId: "AKIDsynthetic",
    secretKey: "secret-synthetic",
    region: "ap-guangzhou",
  });
});

test("shows clear controls and stored-state guidance when credentials exist", () => {
  const onClear = vi.fn();
  render(
    <LinuxTencentCredentialsSection
      available
      status={{ tencent: { region: "ap-shanghai" }, tencentInvalid: true }}
      input={{ secretId: "", secretKey: "", region: undefined }}
      busy={false}
      onInputChange={vi.fn()}
      onSave={vi.fn()}
      onClear={onClear}
    />,
  );

  expect(screen.getByText(/tencent-provider\.json 无效/)).toBeTruthy();
  expect(screen.getByDisplayValue("ap-shanghai")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "清除凭据" }));
  expect(onClear).toHaveBeenCalledOnce();
});

test("keeps 地域 under 更多选项, opens it only for an invalid region, and still saves it", () => {
  const onSave = vi.fn();
  const props = {
    available: true,
    status: { tencent: null, tencentInvalid: false },
    busy: false,
    onInputChange: vi.fn(),
    onSave,
    onClear: vi.fn(),
  };
  render(
    <LinuxTencentCredentialsSection
      {...props}
      input={{ secretId: "AKIDsynthetic", secretKey: "secret-synthetic", region: "ap-hongkong" }}
    />,
  );
  const fold = () => screen.getByLabelText("腾讯云地域").closest("details") as HTMLDetailsElement;
  expect(fold().textContent).toContain("更多选项");
  expect(fold().open).toBe(false);
  // 凭据和保存按钮不在折叠区里。
  expect(screen.getByLabelText("腾讯云 SecretId").closest("details")).toBeNull();
  expect(screen.getByRole("button", { name: "保存凭据" }).closest("details")).toBeNull();
  // 折叠着的地域照样随「保存」写进凭据文件。
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect(onSave).toHaveBeenCalledWith({
    secretId: "AKIDsynthetic",
    secretKey: "secret-synthetic",
    region: "ap-hongkong",
  });
  cleanup();

  render(
    <LinuxTencentCredentialsSection
      {...props}
      input={{ secretId: "", secretKey: "", region: "ap hongkong" }}
    />,
  );
  expect(fold().open).toBe(true);
});

test("已存凭据允许部分更新，空地域覆盖旧地域且忙碌时不能写入", () => {
  const onSave = vi.fn();
  const onClear = vi.fn();
  const props = {
    available: true,
    status: { tencent: { region: "ap-shanghai" }, tencentInvalid: false },
    input: { secretId: " synthetic-id ", secretKey: " ", region: "" },
    onInputChange: vi.fn(),
    onSave,
    onClear,
  };
  const view = render(<LinuxTencentCredentialsSection {...props} busy={false} />);
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect(onSave).toHaveBeenCalledExactlyOnceWith({ secretId: " synthetic-id ", region: "" });
  view.rerender(<LinuxTencentCredentialsSection {...props} busy />);
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  fireEvent.click(screen.getByRole("button", { name: "清除凭据" }));
  expect(onSave).toHaveBeenCalledOnce();
  expect(onClear).not.toHaveBeenCalled();
});

test("凭据能力不可用时只显示说明，不展示输入和附加测试控件", () => {
  render(
    <LinuxTencentCredentialsSection
      available={false}
      input={{ secretId: "", secretKey: "" }}
      busy={false}
      onInputChange={vi.fn()}
      onSave={vi.fn()}
      onClear={vi.fn()}
    >
      <button>合成测试入口</button>
    </LinuxTencentCredentialsSection>,
  );
  expect(screen.getByText(/设置页不保存不会生效的/)).toBeTruthy();
  expect(screen.queryByLabelText("腾讯云 SecretId")).toBeNull();
  expect(screen.queryByRole("button")).toBeNull();
});
