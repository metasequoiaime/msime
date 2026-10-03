// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsErrorMessage } from "@msime/ui";

afterEach(cleanup);

test("renders arbitrary settings error content as an alert", () => {
  render(
    <SettingsErrorMessage>
      配置无效。 <button type="button">修复</button>
    </SettingsErrorMessage>,
  );

  const error = screen.getByRole("alert");
  expect(error.className).toBe("error");
  expect(error.textContent).toContain("配置无效");
  expect(screen.getByRole("button", { name: "修复" })).not.toBeNull();
});
