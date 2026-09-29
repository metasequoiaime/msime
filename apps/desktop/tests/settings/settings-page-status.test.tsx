// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsPageStatus } from "@msime/ui";

test("composes the initial loading status", () => {
  render(
    <SettingsPageStatus
      busy
      draft={undefined}
      error=""
      notice=""
      recoveredBackup=""
      canRecover={false}
      onRecover={vi.fn()}
      macos={false}
      inputSourceStartup={null}
      onOpenSettings={vi.fn()}
      onDismiss={vi.fn()}
      onError={vi.fn()}
    />,
  );

  expect(screen.getByRole("status").textContent).toBe("正在读取设置…");
});
