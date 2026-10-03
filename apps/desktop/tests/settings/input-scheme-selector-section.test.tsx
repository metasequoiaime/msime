// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputSchemeSelectorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the selector in a row, forwards radio changes and honors hidden state", () => {
  const onChange = vi.fn();
  const { rerender } = render(<InputSchemeSelectorSection value="quanpin" onChange={onChange} />);

  expect((screen.getByRole("radio", { name: "全拼" }) as HTMLInputElement).checked).toBe(true);
  expect(screen.getByRole("radiogroup")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "双拼" }));
  expect(onChange).toHaveBeenCalledWith("shuangpin");

  rerender(<InputSchemeSelectorSection hidden value="quanpin" onChange={onChange} />);
  expect(screen.getByText("输入方案").closest("div")?.hasAttribute("hidden")).toBe(true);
});

const allSchemes = [
  "quanpin",
  "shuangpin",
  "wubi",
  "japanese",
  "korean",
  "cantonese",
  "zhuyin",
  "vietnamese",
  "tibetan",
] as const;

test("offers 粤拼 and 注音 where the host supports them", () => {
  const onChange = vi.fn();
  render(
    <InputSchemeSelectorSection
      value="quanpin"
      supportedSchemes={allSchemes}
      onChange={onChange}
    />,
  );

  expect(screen.getAllByRole("radio").map((radio) => radio.closest("label")?.textContent)).toEqual([
    "全拼",
    "双拼",
    "五笔",
    "粤拼",
    "注音",
  ]);
  fireEvent.click(screen.getByRole("radio", { name: "粤拼" }));
  expect(onChange).toHaveBeenCalledWith("cantonese");
  fireEvent.click(screen.getByRole("radio", { name: "注音" }));
  expect(onChange).toHaveBeenCalledWith("zhuyin");
  expect(screen.queryByText(/此平台暂不支持/)).toBeNull();
});

test("schemes the host does not offer are disabled with the reason", () => {
  render(<InputSchemeSelectorSection value="wubi" onChange={vi.fn()} />);
  expect((screen.getByRole("radio", { name: "粤拼" }) as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByRole("radio", { name: "注音" }) as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByRole("radio", { name: "五笔" }) as HTMLInputElement).disabled).toBe(false);
  expect(screen.getByText("此平台暂不支持粤拼、注音")).toBeTruthy();
});

test("an unsupported selected scheme stays selected and names the fallback", () => {
  render(
    <InputSchemeSelectorSection
      value="cantonese"
      lastChineseScheme="shuangpin"
      onChange={vi.fn()}
    />,
  );

  const cantonese = screen.getByRole("radio", { name: "粤拼" }) as HTMLInputElement;
  expect(cantonese.checked).toBe(true);
  expect(cantonese.disabled).toBe(true);
  expect(screen.getByText("此平台暂不支持粤拼、注音，已回退到双拼")).toBeTruthy();
});
