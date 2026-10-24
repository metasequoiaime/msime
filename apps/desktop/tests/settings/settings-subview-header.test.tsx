// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { SettingsSubViewHeader } from "@msime/ui";

afterEach(cleanup);

test("settings subview header renders a titled view without a back action", () => {
  render(<SettingsSubViewHeader title="词库详情" />);
  expect(screen.getByRole("heading", { level: 2, name: "词库详情" })).toBeTruthy();
  expect(screen.queryByRole("button")).toBeNull();
});

test("settings subview header forwards back button semantics and marker", () => {
  const onBack = vi.fn();
  const ref = createRef<HTMLButtonElement>();
  render(
    <SettingsSubViewHeader
      title="声音与效果"
      back={{
        label: "我的插件",
        ariaLabel: "返回我的插件",
        onClick: onBack,
        disabled: true,
        ref,
        marker: "data-plugin-back",
      }}
    />,
  );
  const button = screen.getByRole("button", { name: "返回我的插件" });
  expect(button).toHaveProperty("disabled", true);
  expect(button.getAttribute("data-plugin-back")).toBe("");
  expect(ref.current).toBe(button);
  fireEvent.click(button);
  expect(onBack).not.toHaveBeenCalled();
});
