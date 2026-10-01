// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SwitchRow } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled switch row and forwards its state", () => {
  const onChange = vi.fn();
  render(<SwitchRow title="云候选" checked={false} onChange={onChange} />);

  expect(screen.getByText("云候选")).toBeTruthy();
  const toggle = screen.getByRole("switch", { name: "云候选" }) as HTMLInputElement;
  expect(toggle.checked).toBe(false);

  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(true);
});

test("passes row and switch state through", () => {
  render(
    <SwitchRow
      title="云候选"
      description="向在线服务请求额外候选"
      aria-label="启用云候选"
      checked
      disabled
      hidden
      onChange={vi.fn()}
    />,
  );

  const toggle = screen.getByRole("switch", { name: "启用云候选", hidden: true });
  expect(toggle.hasAttribute("disabled")).toBe(true);
  expect(toggle.closest("div")?.hidden).toBe(true);
});
