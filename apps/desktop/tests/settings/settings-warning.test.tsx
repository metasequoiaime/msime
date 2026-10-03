// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsWarning } from "@msime/ui";

afterEach(cleanup);

test("renders a settings warning as a status with the shared warning style", () => {
  render(<SettingsWarning>服务配置无效。</SettingsWarning>);

  const warning = screen.getByRole("status");
  expect(warning.textContent).toBe("服务配置无效。");
  expect(warning.className).toBe("mt-1.5 mb-0 text-[13px] leading-normal text-[#a2543a]");
});
