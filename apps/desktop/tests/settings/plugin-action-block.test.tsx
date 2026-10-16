// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { PluginActionBlock } from "@msime/ui";

afterEach(cleanup);

test("renders the note and wraps plugin actions in the manager layout", () => {
  render(
    <PluginActionBlock note="说明文字">
      <button type="button">执行</button>
    </PluginActionBlock>,
  );

  expect(screen.getByText("说明文字").tagName).toBe("P");
  expect(screen.getByRole("button", { name: "执行" })).toBeTruthy();
  const button = screen.getByRole("button", { name: "执行" });
  expect(button.parentElement?.className).toContain("flex flex-wrap gap-1.5");
});

test("does not create an empty actions container", () => {
  render(<PluginActionBlock note="只有说明" />);

  expect(screen.getByText("只有说明")).toBeTruthy();
  expect(screen.getByText("只有说明").parentElement?.querySelector("div.flex")).toBeNull();
});
