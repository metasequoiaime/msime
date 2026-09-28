// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DefaultImeModeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("default IME mode selector reports the chosen mode", () => {
  const onChange = vi.fn();
  render(<DefaultImeModeSection value="chinese" onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("默认中英文"), { target: { value: "english" } });
  expect(onChange).toHaveBeenCalledWith("english");
});

test("default IME mode selector defaults to Chinese", () => {
  render(<DefaultImeModeSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("默认中英文") as HTMLSelectElement).value).toBe("chinese");
});
