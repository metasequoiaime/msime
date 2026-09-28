// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DoubaoStreamEndpointSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects a preset endpoint and forwards its URL", () => {
  const onChange = vi.fn();
  render(<DoubaoStreamEndpointSection endpoint="" onChange={onChange} />);

  const select = screen.getByRole("combobox", { name: "流式接口" });
  fireEvent.change(select, { target: { value: "async" } });
  expect(onChange).toHaveBeenCalledWith(expect.stringContaining("wss://"));
});

test("keeps a custom endpoint represented as custom", () => {
  render(
    <DoubaoStreamEndpointSection endpoint="https://custom.example/stream" onChange={vi.fn()} />,
  );
  expect((screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement).value).toBe(
    "custom",
  );
});
