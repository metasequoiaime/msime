// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsActionsFooter, type SettingsActionsFooterProps } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function renderFooter(props: Partial<SettingsActionsFooterProps> = {}) {
  render(
    <SettingsActionsFooter
      busy={false}
      saveState="idle"
      saveError=""
      showRestoreDefaults
      onRestoreDefaults={vi.fn()}
      onRetry={vi.fn()}
      {...props}
    />,
  );
  return screen.getByRole("contentinfo");
}

test("has no save button and reports the automatic save quietly", () => {
  const onRestoreDefaults = vi.fn();
  const footer = renderFooter({ saveState: "saving", onRestoreDefaults });
  expect(Array.from(footer.children).map((child) => child.textContent)).toEqual([
    "恢复默认设置",
    "正在保存…",
  ]);
  expect(footer.querySelector("button[type=submit]")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "恢复默认设置" }));
  expect(onRestoreDefaults).toHaveBeenCalledOnce();
});

test("shows 已保存 after a save and nothing while idle", () => {
  renderFooter({ saveState: "saved" });
  expect(screen.getByText("已保存")).toBeTruthy();
  cleanup();
  const footer = renderFooter();
  expect(footer.querySelector("span")?.textContent).toBe("");
});

test("a failed save shows its reason with 重试 and 重新读取", () => {
  const onRetry = vi.fn();
  const onReload = vi.fn();
  const footer = renderFooter({
    saveState: "failed",
    saveError: "磁盘已满",
    onRetry,
    onReload,
  });
  expect(screen.getByRole("alert").textContent).toBe("磁盘已满");
  expect(Array.from(footer.children).map((child) => child.textContent)).toEqual([
    "恢复默认设置",
    "磁盘已满",
    "重试",
    "重新读取",
  ]);
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  expect(onRetry).toHaveBeenCalledOnce();
  expect(onReload).toHaveBeenCalledOnce();
  for (const name of ["重试", "重新读取", "恢复默认设置"])
    expect(screen.getByRole("button", { name }).className).toBe("secondary");
});

test("disables the actions while busy", () => {
  renderFooter({ busy: true, saveState: "failed", saveError: "x", onReload: vi.fn() });
  for (const name of ["恢复默认设置", "重试", "重新读取"])
    expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
});

test("a page that edits no preferences leaves 恢复默认设置 out of the row", () => {
  const footer = renderFooter({ showRestoreDefaults: false, saveState: "saved" });
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();
  expect(Array.from(footer.children).map((child) => child.textContent)).toEqual(["已保存"]);
});
