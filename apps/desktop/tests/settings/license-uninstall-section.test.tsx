// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LicenseUninstallSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("license section opens the uninstall confirmation", () => {
  const onRequestUninstall = vi.fn();
  render(
    <LicenseUninstallSection
      openThirdPartyLicenses={vi.fn(async () => {})}
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

test("license section renders and cancels an uninstall confirmation", () => {
  const onCancelUninstall = vi.fn();
  render(
    <LicenseUninstallSection
      openThirdPartyLicenses={undefined}
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
