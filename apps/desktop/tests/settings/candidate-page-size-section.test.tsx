// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePageSizeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate page size is a ticked 3-9 slider that reports the chosen size", () => {
  const onChange = vi.fn();
  const { container } = render(
    <CandidatePageSizeSection value={5} fixed={false} onChange={onChange} />,
  );

  const slider = screen.getByRole("slider", { name: "每页候选项数量" }) as HTMLInputElement;
  expect([slider.min, slider.max, slider.step, slider.value]).toEqual(["3", "9", "1", "5"]);
  // One mark per size, and the value read out after the track.
  expect(container.querySelectorAll(".platform-slider-ticks > [aria-hidden] > span")).toHaveLength(
    7,
  );
  expect(slider.closest("span.flex")?.textContent).toBe("5");

  fireEvent.change(slider, { target: { value: "7" } });
  expect(onChange).toHaveBeenCalledWith(7);
});

test("a stored size below the offered range widens the slider instead of being rewritten", () => {
  render(<CandidatePageSizeSection value={1} fixed={false} onChange={vi.fn()} />);

  const slider = screen.getByRole("slider", { name: "每页候选项数量" }) as HTMLInputElement;
  expect([slider.min, slider.max, slider.value]).toEqual(["1", "9", "1"]);
});

test("a host that picks a tenth candidate with 0 offers sizes up to ten", () => {
  const onChange = vi.fn();
  render(<CandidatePageSizeSection value={9} fixed={false} max={10} onChange={onChange} />);

  const slider = screen.getByRole("slider", { name: "每页候选项数量" }) as HTMLInputElement;
  expect([slider.min, slider.max, slider.value]).toEqual(["3", "10", "9"]);
  fireEvent.change(slider, { target: { value: "10" } });
  expect(onChange).toHaveBeenCalledWith(10);
});

test("a stored ten on a host that shows nine widens the slider instead of being rewritten", () => {
  render(<CandidatePageSizeSection value={10} fixed={false} max={9} onChange={vi.fn()} />);

  const slider = screen.getByRole("slider", { name: "每页候选项数量" }) as HTMLInputElement;
  expect([slider.min, slider.max, slider.value]).toEqual(["3", "10", "10"]);
});

test("candidate page size hides when the host fixes the page", () => {
  render(<CandidatePageSizeSection value={5} fixed onChange={vi.fn()} />);

  expect(screen.queryByLabelText("每页候选项数量")).toBeNull();
});
