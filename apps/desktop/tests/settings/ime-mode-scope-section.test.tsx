// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ImeModeScopeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("IME mode scope selector reports the chosen scope", () => {
  const onChange = vi.fn();
  render(<ImeModeScopeSection value="app" onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("中英文状态"), { target: { value: "global" } });
  expect(onChange).toHaveBeenCalledWith("global");
});

test("IME mode scope selector defaults to per-app state", () => {
  render(<ImeModeScopeSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("中英文状态") as HTMLSelectElement).value).toBe("app");
});
