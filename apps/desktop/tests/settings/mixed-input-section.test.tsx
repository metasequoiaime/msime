// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MixedInputSection, type MixedInputPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: MixedInputPreferences = {
  english: true,
  minimum_prefix: 5,
  emoji: false,
  kaomoji: false,
};

test("mixed input preserves the threshold while toggling independent switches", () => {
  const onChange = vi.fn();
  render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  const threshold = screen.getByRole("combobox", { name: "触发字符数" }) as HTMLSelectElement;
  fireEvent.change(threshold, { target: { value: "8" } });
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, minimum_prefix: 8 });

  fireEvent.click(screen.getByRole("checkbox", { name: /^emoji 混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, emoji: true });
  fireEvent.click(screen.getByRole("checkbox", { name: /^颜文字混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, kaomoji: true });
});

test("disabling English mixed input disables its threshold without changing its value", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  fireEvent.click(screen.getByRole("checkbox", { name: /^中英混输/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, english: false });
  view.rerender(
    <MixedInputSection preferences={{ ...preferences, english: false }} onChange={onChange} />,
  );
  const threshold = screen.getByRole("combobox", { name: "触发字符数" }) as HTMLSelectElement;
  expect(threshold.disabled).toBe(true);
  expect(threshold.value).toBe("5");
});
