// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TraditionalChineseOutputSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("traditional output switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<TraditionalChineseOutputSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("繁体输出"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("traditional output switch is disabled by default", () => {
  render(<TraditionalChineseOutputSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("繁体输出") as HTMLInputElement).checked).toBe(false);
});

test.each(["cantonese", "zhuyin"] as const)(
  "%s says the switch does not change its Traditional output",
  (scheme) => {
    render(<TraditionalChineseOutputSection value={false} scheme={scheme} onChange={vi.fn()} />);

    expect(screen.getByText(/粤拼与注音直接输出繁体，此开关不影响它们/)).toBeTruthy();
  },
);

test("Stroke says the switch does not convert the character picked", () => {
  render(<TraditionalChineseOutputSection value={false} scheme="stroke" onChange={vi.fn()} />);

  expect(screen.getByText(/笔画按所选的字原样输出，此开关不影响它/)).toBeTruthy();
  expect(screen.queryByText(/粤拼与注音/)).toBeNull();
});

test("pinyin schemes keep the plain description", () => {
  render(<TraditionalChineseOutputSection value={false} scheme="quanpin" onChange={vi.fn()} />);

  expect(screen.queryByText(/粤拼与注音/)).toBeNull();
  expect(screen.queryByText(/笔画/)).toBeNull();
});
