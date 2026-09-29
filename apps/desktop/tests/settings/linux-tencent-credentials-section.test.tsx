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
