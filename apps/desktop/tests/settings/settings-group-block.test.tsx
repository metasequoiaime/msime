// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsGroupBlock } from "@msime/ui";

afterEach(cleanup);

test("renders a settings group block with the shared block style", () => {
  render(<SettingsGroupBlock>设置内容</SettingsGroupBlock>);

  const block = screen.getByText("设置内容");
  expect(block.tagName).toBe("DIV");
  expect(block.className).toBe("min-w-0 [padding:var(--p-row-pad)]");
});

test("preserves block attributes and appends a local class", () => {
  render(
    <SettingsGroupBlock className="guide" role="group" aria-label="帮助" hidden>
      帮助内容
    </SettingsGroupBlock>,
  );

  const block = screen.getByRole("group", { hidden: true });
  expect(block.className).toBe("min-w-0 [padding:var(--p-row-pad)] guide");
  expect(block.hasAttribute("hidden")).toBe(true);
});
