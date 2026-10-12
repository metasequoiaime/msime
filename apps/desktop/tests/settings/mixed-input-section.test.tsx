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

  fireEvent.click(screen.getByRole("switch", { name: /^emoji 混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, emoji: true });
  fireEvent.click(screen.getByRole("switch", { name: /^颜文字混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, kaomoji: true });
});

test("disabling English mixed input disables its threshold without changing its value", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  fireEvent.click(screen.getByRole("switch", { name: /^中英混输/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, english: false });
  view.rerender(
    <MixedInputSection preferences={{ ...preferences, english: false }} onChange={onChange} />,
  );
  const threshold = screen.getByRole("combobox", { name: "触发字符数" }) as HTMLSelectElement;
  expect(threshold.disabled).toBe(true);
  expect(threshold.value).toBe("5");
});

test("Japanese mixed input is off when the document predates it and toggles on its own", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  const japanese = screen.getByRole("switch", { name: /^日文混输（实验）/ }) as HTMLInputElement;
  expect(japanese.checked).toBe(false);
  fireEvent.click(japanese);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, japanese: true });

  view.rerender(
    <MixedInputSection preferences={{ ...preferences, japanese: true }} onChange={onChange} />,
  );
  expect(
    (screen.getByRole("switch", { name: /^日文混输（实验）/ }) as HTMLInputElement).checked,
  ).toBe(true);
  fireEvent.click(screen.getByRole("switch", { name: /^日文混输（实验）/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, japanese: false });
});

test("an edition without temporary Japanese does not list Japanese mixed input", () => {
  render(
    <MixedInputSection
      preferences={{ ...preferences, japanese: true }}
      temporaryJapanese={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.queryByRole("switch", { name: /^日文混输（实验）/ })).toBeNull();
  expect(screen.getByRole("switch", { name: /^颜文字混输/ })).toBeTruthy();
});
