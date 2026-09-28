// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PreeditSettingsSection, type PreeditSettingsPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: PreeditSettingsPreferences = {
  shuangpin_preedit_uses_raw: true,
  tsf_preedit_style: "raw",
  candidate_preedit_style: "pinyin",
};

test("desktop preedit selectors report each changed preference", () => {
  const onChange = vi.fn();
  render(
    <PreeditSettingsSection
      preferences={preferences}
      mobile={false}
      showShuangpinPreedit
      inlinePreedit={undefined}
      inlinePreeditBusy={false}
      onChange={onChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("双拼预编辑"), { target: { value: "pinyin" } });
  expect(onChange).toHaveBeenCalledWith({ shuangpin_preedit_uses_raw: false });
  fireEvent.change(screen.getByLabelText("行内预编辑"), { target: { value: "empty" } });
  expect(onChange).toHaveBeenCalledWith({ tsf_preedit_style: "empty" });
  fireEvent.change(screen.getByLabelText("候选窗预编辑"), { target: { value: "empty" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_preedit_style: "empty" });
});

test("touch preedit uses the host feedback switch", () => {
  const onInlinePreeditChange = vi.fn();
  render(
    <PreeditSettingsSection
      preferences={preferences}
      mobile
      showShuangpinPreedit={false}
      inlinePreedit={false}
      inlinePreeditBusy
      onChange={vi.fn()}
      onInlinePreeditChange={onInlinePreeditChange}
    />,
  );

  const inline = screen.getByLabelText("行内预编辑") as HTMLInputElement;
  expect(inline.type).toBe("checkbox");
  expect(inline.disabled).toBe(true);
  expect(screen.getByLabelText("候选栏预编辑")).toBeTruthy();
  expect(screen.queryByLabelText("双拼预编辑")).toBeNull();
});
