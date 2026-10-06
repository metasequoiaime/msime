// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LicenseRows, UninstallSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("uninstall section opens the uninstall confirmation", () => {
  const onRequestUninstall = vi.fn();
  render(
    <UninstallSection
      uninstallInputSource={vi.fn(async () => {})}
      removeUserData={false}
      uninstallBusy={false}
      uninstallConfirmation={false}
      uninstallResult={null}
      onRemoveUserDataChange={vi.fn()}
      onRequestUninstall={onRequestUninstall}
      onConfirmUninstall={vi.fn()}
      onCancelUninstall={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "卸载…" }));
  expect(onRequestUninstall).toHaveBeenCalledOnce();
});

test("uninstall section renders and cancels an uninstall confirmation", () => {
  const onCancelUninstall = vi.fn();
  render(
    <UninstallSection
      uninstallInputSource={vi.fn(async () => {})}
      removeUserData
      uninstallBusy={false}
      uninstallConfirmation
      uninstallResult="success"
      onRemoveUserDataChange={vi.fn()}
      onRequestUninstall={vi.fn()}
      onConfirmUninstall={vi.fn()}
      onCancelUninstall={onCancelUninstall}
    />,
  );

  expect(screen.getByRole("alertdialog", { name: "确认卸载水杉输入法" })).toBeTruthy();
  expect(screen.getByText("输入法已移到废纸篓。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onCancelUninstall).toHaveBeenCalledOnce();
});

test("uninstall section asks for the input sources to be removed in System Settings first", () => {
  render(
    <UninstallSection
      uninstallInputSource={vi.fn(async () => {})}
      removeUserData={false}
      uninstallBusy={false}
      uninstallConfirmation
      uninstallResult="listed"
      onRemoveUserDataChange={vi.fn()}
      onRequestUninstall={vi.fn()}
      onConfirmUninstall={vi.fn()}
      onCancelUninstall={vi.fn()}
    />,
  );

  expect(screen.getByRole("alert").textContent).toContain("输入源");
  expect(screen.getByRole("button", { name: "确认卸载" })).toBeTruthy();
});

test("uninstall section draws nothing on a host that cannot uninstall", () => {
  const { container } = render(
    <UninstallSection
      removeUserData={false}
      uninstallBusy={false}
      uninstallConfirmation={false}
      uninstallResult={null}
      onRemoveUserDataChange={vi.fn()}
      onRequestUninstall={vi.fn()}
      onConfirmUninstall={vi.fn()}
      onCancelUninstall={vi.fn()}
    />,
  );

  expect(container.innerHTML).toBe("");
});

test("license rows open the third-party notices only when the host ships them", () => {
  const openThirdPartyLicenses = vi.fn(async () => {});
  const { unmount } = render(<LicenseRows openThirdPartyLicenses={openThirdPartyLicenses} />);

  expect(screen.getByText("© 2026 Metasequoia IME")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "第三方组件许可" }));
  expect(openThirdPartyLicenses).toHaveBeenCalledOnce();
  unmount();

  render(<LicenseRows />);
  expect(screen.getByText("© 2026 Metasequoia IME")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "第三方组件许可" })).toBeNull();
});
