// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SettingsFormFrame } from "@msime/ui";

test("provides the shared form boundary and reload control", () => {
  const onReload = vi.fn();
  render(
    <SettingsFormFrame showReload busy={false} onReload={onReload}>
      <input aria-label="测试字段" />
    </SettingsFormFrame>,
  );

  const form = screen.getByRole("form", { name: "设置" });
  // Changes save themselves; submitting (Enter in a lone field) must not navigate the page.
  const submit = new Event("submit", { bubbles: true, cancelable: true });
  form.dispatchEvent(submit);
  expect(submit.defaultPrevented).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  expect(onReload).toHaveBeenCalledOnce();
});
