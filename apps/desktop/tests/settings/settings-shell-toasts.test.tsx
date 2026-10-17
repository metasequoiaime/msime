// @vitest-environment jsdom
import { expect, test } from "vitest";
import { render } from "@testing-library/react";

import { SettingsShellToasts } from "../../../../packages/ui/src/settings/settings-shell-toasts";

test("禁用时只渲染子内容，不挂载 toast 区域", () => {
  const { container } = render(
    <SettingsShellToasts enabled={false}>
      <span data-testid="child">内容</span>
    </SettingsShellToasts>,
  );

  expect(container.querySelector('[data-testid="child"]')).not.toBeNull();
  expect(container.querySelector('[role="status"]')).toBeNull();
});

test("启用时提供带 polite live region 的 toast provider", () => {
  const { container } = render(
    <SettingsShellToasts enabled>
      <span data-testid="child">内容</span>
    </SettingsShellToasts>,
  );

  expect(container.querySelector('[data-testid="child"]')).not.toBeNull();
  expect(container.querySelector('[role="status"]')?.getAttribute("aria-live")).toBe("polite");
});
