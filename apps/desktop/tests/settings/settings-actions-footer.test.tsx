// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsActionsFooter } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders dirty state and dispatches restore and save actions", () => {
  const onRestoreDefaults = vi.fn();
  const onSave = vi.fn();
  render(
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSave();
      }}
    >
      <SettingsActionsFooter
        busy={false}
        dirty
        canSave
        showRestoreDefaults
        onRestoreDefaults={onRestoreDefaults}
      />
    </form>,
  );

  expect(screen.getByText("有未保存的修改")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "恢复默认设置" }));
  fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
  expect(onRestoreDefaults).toHaveBeenCalledTimes(1);
  expect(onSave).toHaveBeenCalledTimes(1);
});

test("disables actions while busy or when there are no valid edits", () => {
  render(
    <SettingsActionsFooter
      busy
      dirty={false}
      canSave={false}
      showRestoreDefaults
      onRestoreDefaults={vi.fn()}
    />,
  );

  expect((screen.getByRole("button", { name: "恢复默认设置" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect((screen.getByRole("button", { name: "处理中…" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
});
