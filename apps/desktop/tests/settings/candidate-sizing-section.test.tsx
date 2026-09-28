// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateSizingSection, type CandidateSizingPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: CandidateSizingPreferences = {
  candidate_font_size: 18,
  candidate_preedit_font_size: 15,
};

test("candidate sizing controls report typed preference patches", () => {
  const onChange = vi.fn();
  render(
    <CandidateSizingSection
      preferences={preferences}
      mobile={false}
      showFontControls
      showPreeditFont
      onChange={onChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("候选窗字号"), { target: { value: "20" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_font_size: 20 });
});

test("candidate sizing controls honor host capability limits", () => {
  render(
    <CandidateSizingSection
      preferences={preferences}
      mobile
      showFontControls={false}
      showPreeditFont={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.queryByLabelText("候选栏字号")).toBeNull();
  expect(screen.queryByLabelText("候选栏预编辑字号")).toBeNull();
});
