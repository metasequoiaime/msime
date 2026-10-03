// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsManagerActions } from "@msime/ui";

afterEach(cleanup);

test("renders manager actions in a div with the shared action layout", () => {
  render(<SettingsManagerActions>操作</SettingsManagerActions>);

  const actions = screen.getByText("操作");
  expect(actions.tagName).toBe("DIV");
  expect(actions.className).toBe("flex flex-wrap gap-1.5 [&_.secondary]:mt-0 [&_.primary]:mt-0");
});

test("supports an inline action container with a local class", () => {
  render(
    <SettingsManagerActions as="span" className="shrink-0" role="group">
      行内操作
    </SettingsManagerActions>,
  );

  const actions = screen.getByRole("group");
  expect(actions.tagName).toBe("SPAN");
  expect(actions.className).toContain("shrink-0");
});
