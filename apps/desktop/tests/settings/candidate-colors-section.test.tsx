// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateColorsSection, type CandidateColorPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: CandidateColorPreferences = {
  candidate_text_color: null,
  candidate_number_color: null,
  candidate_accent_color: null,
  candidate_selected_color: null,
  candidate_hover_color: null,
  candidate_surface_color: null,
  candidate_border_color: null,
};

test("candidate color controls report an override and reset it to the theme", () => {
  const onChange = vi.fn();
  const view = render(
    <CandidateColorsSection
      preferences={preferences}
      previewTheme="light"
      showRowColors
      showSelectionAppearance
      showBorderColor
      linux={false}
      onChange={onChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("候选文字颜色"), { target: { value: "#123456" } });
  expect(onChange).toHaveBeenCalledWith("candidate_text_color", "#123456");

  view.rerender(
    <CandidateColorsSection
      preferences={{ ...preferences, candidate_text_color: "#123456" }}
      previewTheme="light"
      showRowColors
      showSelectionAppearance
      showBorderColor
      linux={false}
      onChange={onChange}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "跟随主题" }));
  expect(onChange).toHaveBeenLastCalledWith("candidate_text_color", null);
});

test("candidate color controls honor host capability limits", () => {
  render(
    <CandidateColorsSection
      preferences={preferences}
      previewTheme="dark"
      showRowColors={false}
      showSelectionAppearance={false}
      showBorderColor={false}
      linux
      onChange={vi.fn()}
    />,
  );

  expect(screen.queryByLabelText("候选强调色")).toBeNull();
  expect(screen.queryByLabelText("候选选中色")).toBeNull();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
  expect(screen.getByText(/候选面板不支持强调或选中行颜色/)).toBeTruthy();
  expect(screen.getAllByText(/Fcitx5 经典界面/).length).toBeGreaterThan(0);
});
