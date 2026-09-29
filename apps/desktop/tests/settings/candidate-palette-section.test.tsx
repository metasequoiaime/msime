// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePaletteSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("changes whether the mobile candidate strip follows desktop colors", () => {
  const onChange = vi.fn();
  render(<CandidatePaletteSection value={false} busy={false} onChange={onChange} />);

  fireEvent.click(screen.getByRole("checkbox", { name: "使用桌面候选皮肤" }));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("disables the switch while feedback settings are saving", () => {
  render(<CandidatePaletteSection value={true} busy={true} onChange={vi.fn()} />);

  expect(
    (screen.getByRole("checkbox", { name: "使用桌面候选皮肤" }) as HTMLInputElement).disabled,
  ).toBe(true);
});
