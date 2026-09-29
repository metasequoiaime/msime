// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MobileKeyboardFeedbackSection, type MobileKeyboardFeedback } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const value: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: true,
  hapticStrength: "medium",
  englishSuggestions: true,
};

test("updates sound, haptics, and strength", () => {
  const onChange = vi.fn();
  render(
    <MobileKeyboardFeedbackSection
      value={value}
      busy={false}
      ios={false}
      canPreview
      onChange={onChange}
      onPreview={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "按键音" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, soundEnabled: false });

  fireEvent.click(screen.getByRole("switch", { name: "按键振动" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, hapticsEnabled: false });

  fireEvent.change(screen.getByRole("combobox", { name: "振动强度" }), {
    target: { value: "strong" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ ...value, hapticStrength: "strong" });
});

test("offers preview and iOS English suggestions", () => {
  const onChange = vi.fn();
  const onPreview = vi.fn();
  render(
    <MobileKeyboardFeedbackSection
      value={value}
      busy={false}
      ios
      canPreview
      onChange={onChange}
      onPreview={onPreview}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "试一下振动" }));
  expect(onPreview).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("switch", { name: "英文建议" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...value, englishSuggestions: false });
});

test("hides vibration controls when unavailable", () => {
  render(
    <MobileKeyboardFeedbackSection
      value={{ ...value, hapticsAvailable: false }}
      busy={false}
      ios={false}
      canPreview
      onChange={vi.fn()}
      onPreview={vi.fn()}
    />,
  );

  expect(screen.queryByRole("switch", { name: "按键振动" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "振动强度" })).toBeNull();
});
