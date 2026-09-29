// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HelpcodeSettingsPage } from "@msime/ui";

afterEach(() => {
  cleanup();
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

  fireEvent.click(screen.getByRole("switch", { name: "全拼辅助码" }));

  expect(onChange).toHaveBeenCalledWith({
    quanpin_helpcode: { enabled: false, schema: "ziranma", show_in_candidate_window: false },
  });
});

test("helper-code settings include discovered custom schemas", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
        shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
      }}
      mobile={false}
      showShiftEntry
      customSchemas={[
        {
          schema: "custom/synthetic",
          file_stem: "synthetic",
          name: "合成辅助码",
          name_en: "Synthetic",
        },
        { schema: "custom/english", file_stem: "english", name: "", name_en: "English" },
        { schema: "custom/fallback", file_stem: "fallback", name: "", name_en: "" },
      ]}
      onChange={onChange}
    />,
  );

  expect(screen.getAllByRole("option", { name: "合成辅助码 (Synthetic)" })).toHaveLength(2);
  expect(screen.getAllByRole("option", { name: "English" })).toHaveLength(2);
  expect(screen.getAllByRole("option", { name: "fallback" })).toHaveLength(2);

  fireEvent.change(screen.getByRole("combobox", { name: "全拼辅助码方案" }), {
    target: { value: "custom/synthetic" },
  });
  expect(onChange).toHaveBeenCalledWith({
    quanpin_helpcode: {
      enabled: true,
      schema: "custom/synthetic",
      show_in_candidate_window: false,
    },
  });
});

test("helper-code settings retain an unavailable custom selection", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: {
          enabled: true,
          schema: "custom/no-longer-installed",
          show_in_candidate_window: false,
        },
      }}
      mobile={false}
      showShiftEntry={false}
      onChange={onChange}
    />,
  );

  expect(screen.getByRole("option", { name: "custom/no-longer-installed" })).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "全拼辅助码方案" }) as HTMLSelectElement).value).toBe(
    "custom/no-longer-installed",
  );
});
