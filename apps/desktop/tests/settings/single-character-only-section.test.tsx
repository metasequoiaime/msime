// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SingleCharacterOnlySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("single character switch is off when the preference is absent", () => {
  const onChange = vi.fn();
  render(<SingleCharacterOnlySection onChange={onChange} />);

  const toggle = screen.getByLabelText("只出单字") as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(true);
});

test("single character switch reflects the supplied value", () => {
  render(<SingleCharacterOnlySection value={true} onChange={vi.fn()} />);

  expect((screen.getByLabelText("只出单字") as HTMLInputElement).checked).toBe(true);
});
