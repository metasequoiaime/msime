// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HelpcodeSettingsPage } from "@msime/ui";

afterEach(() => {
  vi.restoreAllMocks();
});

test("helper-code settings expose independent scheme updates", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
        shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
      }}
      mobile={false}
      showShiftEntry
      onChange={onChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "全拼辅助码" }));

  expect(onChange).toHaveBeenCalledWith({
    quanpin_helpcode: { enabled: false, schema: "ziranma", show_in_candidate_window: false },
  });
});
