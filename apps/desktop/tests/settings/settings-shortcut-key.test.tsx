// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsShortcutKey } from "@msime/ui";

afterEach(cleanup);

test("renders a shortcut key with the shared key style", () => {
  render(<SettingsShortcutKey>Ctrl+K</SettingsShortcutKey>);

  const key = screen.getByText("Ctrl+K");
  expect(key.tagName).toBe("KBD");
  expect(key.className).toBe(
    "min-w-30 rounded-[5px] border border-edge bg-[var(--button-secondary-bg)] px-2 py-1 text-center font-[inherit] text-xs text-body",
  );
});

test("forwards key attributes and appends a local class", () => {
  render(
    <SettingsShortcutKey className="compact" aria-label="快捷键" data-testid="shortcut">
      Ctrl+K
    </SettingsShortcutKey>,
  );

  const key = screen.getByTestId("shortcut");
  expect(key.getAttribute("aria-label")).toBe("快捷键");
  expect(key.className).toContain("compact");
});
