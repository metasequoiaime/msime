// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { FormEvent } from "react";
import { SettingsFormFrame } from "@msime/ui";

test("provides the shared form boundary and reload control", () => {
  const onSubmit = vi.fn((event: FormEvent<HTMLFormElement>) => event.preventDefault());
  const onReload = vi.fn();
  render(
    <SettingsFormFrame onSubmit={onSubmit} showReload busy={false} onReload={onReload}>
      <input aria-label="测试字段" />
    </SettingsFormFrame>,
  );

  fireEvent.submit(screen.getByRole("textbox", { name: "测试字段" }));
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));

  expect(onSubmit).toHaveBeenCalledOnce();
  expect(onReload).toHaveBeenCalledOnce();
});
