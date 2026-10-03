// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type SettingsPageFieldsetProps = {
  disabled: boolean;
  hidden: boolean;
  ariaLabel: string;
  children: ReactNode;
};

test("shared settings page fieldset preserves page state and group layout", () => {
  const Fieldset = (
    ui as unknown as {
      SettingsPageFieldset: ComponentType<SettingsPageFieldsetProps>;
    }
  ).SettingsPageFieldset;
  expect(Fieldset).toBeDefined();

  const { rerender } = render(
    <Fieldset disabled={false} hidden={false} ariaLabel="合成设置页">
      <p>合成内容</p>
    </Fieldset>,
  );

  const fieldset = screen.getByRole("group", { name: "合成设置页" });
  expect(fieldset).not.toHaveProperty("disabled", true);
  expect(fieldset.querySelector(".mt-6.flex.flex-col")).not.toBeNull();
  expect(screen.getByText("合成内容")).toBeTruthy();

  rerender(
    <Fieldset disabled hidden ariaLabel="合成设置页">
      <p>合成内容</p>
    </Fieldset>,
  );
  expect(fieldset).toHaveProperty("disabled", true);
  expect(fieldset).toHaveProperty("hidden", true);
});
