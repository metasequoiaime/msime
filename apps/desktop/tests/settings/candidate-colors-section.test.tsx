// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateColorsSection, type CandidateColorPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: CandidateColorPreferences = {
  text: null,
  number: null,
  accent: null,
  selected: null,
  hover: null,
  surface: null,
  border: null,
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
  expect(onChange).toHaveBeenCalledWith("text", "#123456");

  view.rerender(
    <CandidateColorsSection
      preferences={{ ...preferences, text: "#123456" }}
      previewTheme="light"
      showRowColors
      showSelectionAppearance
      showBorderColor
      linux={false}
      onChange={onChange}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "跟随主题" }));
  expect(onChange).toHaveBeenLastCalledWith("text", null);
});

test("candidate color resets only clear a slot the custom theme sets, except surface and border", () => {
  const onChange = vi.fn();
  render(
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

  const accentReset = screen.getByRole("button", { name: "候选强调色跟随主题" });
  expect(accentReset.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(accentReset);
  expect(onChange).not.toHaveBeenCalled();

  const surfaceReset = screen.getByRole("button", { name: "候选表面色跟随主题" });
  expect(surfaceReset.hasAttribute("aria-pressed")).toBe(false);
  fireEvent.click(surfaceReset);
  expect(onChange).toHaveBeenLastCalledWith("surface", null);
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
  expect(screen.getByText(/候选窗口不支持强调或选中行颜色/)).toBeTruthy();
  expect(screen.getAllByText(/Fcitx5 经典界面/).length).toBeGreaterThan(0);
});
