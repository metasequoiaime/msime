// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsRowStack } from "@msime/ui";

afterEach(cleanup);

test("renders a settings row stack with the shared stack style", () => {
  render(<SettingsRowStack>设置行</SettingsRowStack>);

  const stack = screen.getByText("设置行");
  expect(stack.tagName).toBe("DIV");
  expect(stack.className).toBe(
    "flex min-w-0 flex-col gap-[var(--p-row-gap)] [&>:not([hidden])~:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)] harmony:[&>:not([hidden])~:not([hidden])]:[border-top:0] harmony:[&>:not([hidden])~:not([hidden])]:[background-image:linear-gradient(var(--p-row-divider),var(--p-row-divider))] harmony:[&>:not([hidden])~:not([hidden])]:[background-position:right_top] harmony:[&>:not([hidden])~:not([hidden])]:[background-size:calc(100%_-_var(--p-row-divider-inset))_var(--p-row-divider-w)] harmony:[&>:not([hidden])~:not([hidden])]:[background-repeat:no-repeat]",
  );
});

test("preserves group attributes and appends a local class", () => {
  render(
    <SettingsRowStack className="compact" role="group" aria-label="输入设置">
      设置内容
    </SettingsRowStack>,
  );

  const stack = screen.getByRole("group", { name: "输入设置" });
  expect(stack.className).toContain("compact");
});
