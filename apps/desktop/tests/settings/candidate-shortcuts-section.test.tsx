// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateShortcutsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const navigation = {
  minus_equal: true,
  comma_period: false,
  brackets: false,
  tab: true,
  page_up_down: false,
  mouse_wheel: true,
  arrows: true,
};

test("candidate shortcuts report number-row selection changes", () => {
  const onNumberRowSelectionChange = vi.fn();
  render(
    <CandidateShortcutsSection
      navigation={navigation}
      numberRowSelection={true}
      showNumberRowSelection
      mobile={false}
      onNumberRowSelectionChange={onNumberRowSelectionChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("数字键选词"));
  expect(onNumberRowSelectionChange).toHaveBeenCalledWith(false);
  expect(screen.getByText("Space 或 1–9")).toBeTruthy();
});

test("candidate shortcuts only show enabled navigation rows", () => {
  render(
    <CandidateShortcutsSection
      navigation={{ ...navigation, arrows: false }}
      numberRowSelection={false}
      showNumberRowSelection={false}
      mobile
      onNumberRowSelectionChange={vi.fn()}
    />,
  );

  expect(screen.getByText("候选栏翻页")).toBeTruthy();
  expect(screen.getByText("鼠标滚轮")).toBeTruthy();
  expect(screen.queryByText("Space 或 1–9")).toBeNull();
});
