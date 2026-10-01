// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SelectRow } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled row and forwards select changes", () => {
  const onChange = vi.fn();
  render(
    <SelectRow title="语言" aria-label="目标语言" value="en" onChange={onChange}>
      <option value="en">英语</option>
      <option value="ja">日语</option>
    </SelectRow>,
  );

  expect(screen.getByText("语言")).toBeTruthy();
  const select = screen.getByLabelText("目标语言") as HTMLSelectElement;
  expect(select.value).toBe("en");

  fireEvent.change(select, { target: { value: "ja" } });
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange.mock.calls[0][0]).toEqual(expect.objectContaining({ type: "change" }));
});

test("keeps native select props and options intact", () => {
  render(
    <SelectRow title="方案" disabled value="one" onChange={vi.fn()}>
      <option value="one">一</option>
    </SelectRow>,
  );

  expect((screen.getByRole("combobox") as HTMLSelectElement).disabled).toBe(true);
  expect(screen.getByRole("option", { name: "一" })).toBeTruthy();
});
