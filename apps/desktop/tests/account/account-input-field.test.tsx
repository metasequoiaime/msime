// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountInputField } from "@msime/ui";

afterEach(cleanup);

test("shared account input field renders the account label and forwards edits", () => {
  const onChange = vi.fn();
  render(
    <AccountInputField label="合成字段" ariaLabel="合成输入" value="初始值" onChange={onChange} />,
  );

  fireEvent.change(screen.getByLabelText("合成输入"), { target: { value: "更新值" } });

  const input = screen.getByLabelText("合成输入");
  expect(input.className).toContain("rounded-lg");
  expect(input.closest("label")?.textContent).toContain("合成字段");
  expect(onChange).toHaveBeenCalledWith("更新值");
});
