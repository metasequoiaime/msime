// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("switching to Chinese restores the remembered Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="japanese" lastChineseScheme="shuangpin" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "shuangpin" });
});

test("switching to Japanese remembers the current Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="wubi" lastChineseScheme="wubi" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("日文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "wubi", scheme: "japanese" });
});

test("switching to Korean remembers the current Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="shuangpin" lastChineseScheme="quanpin" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("韩文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "shuangpin", scheme: "korean" });
});

test("switching between Japanese and Korean keeps the remembered Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="japanese" lastChineseScheme="wubi" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("韩文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "wubi", scheme: "korean" });
});

test("switching from Korean to Chinese restores the remembered Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="korean" lastChineseScheme="wubi" onChange={onChange} />);

  expect((screen.getByLabelText("韩文") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "wubi" });
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

test("switching to Vietnamese remembers the current Cantonese scheme", () => {
  const onChange = vi.fn();
  render(
    <InputModeSection
      scheme="cantonese"
      lastChineseScheme="quanpin"
      supportedSchemes={allSchemes}
      onChange={onChange}
    />,
  );

  expect((screen.getByLabelText("中文") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByLabelText("越南文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "cantonese", scheme: "vietnamese" });
});

test("switching from Vietnamese to Chinese restores a remembered Zhuyin scheme", () => {
  const onChange = vi.fn();
  render(
    <InputModeSection
      scheme="vietnamese"
      lastChineseScheme="zhuyin"
      supportedSchemes={allSchemes}
      onChange={onChange}
    />,
  );

  expect((screen.getByLabelText("越南文") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "zhuyin" });
});

test("switching from Vietnamese to Tibetan keeps the remembered Chinese scheme", () => {
  const onChange = vi.fn();
  render(
    <InputModeSection
      scheme="vietnamese"
      lastChineseScheme="wubi"
      supportedSchemes={allSchemes}
      onChange={onChange}
    />,
  );

  expect((screen.getByLabelText("藏文") as HTMLInputElement).disabled).toBe(false);
  fireEvent.click(screen.getByLabelText("藏文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "wubi", scheme: "tibetan" });
});

test("a Tibetan document on a host without it shows the scheme host-api falls back to", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="tibetan" lastChineseScheme="wubi" onChange={onChange} />);

  const tibetan = screen.getByLabelText("藏文") as HTMLInputElement;
  expect(tibetan.checked).toBe(true);
  expect(tibetan.disabled).toBe(true);
  expect(screen.getByText("此平台暂不支持藏文，已回退到五笔")).toBeTruthy();
  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "wubi" });
});

test("a host without Vietnamese shows 越南文 disabled with the reason", () => {
  render(<InputModeSection scheme="quanpin" onChange={vi.fn()} />);

  expect((screen.getByLabelText("越南文") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("日文") as HTMLInputElement).disabled).toBe(false);
  expect(screen.getByText(/此平台暂不支持越南文/)).toBeTruthy();
});

test("a Vietnamese document on a host without it shows the scheme host-api falls back to", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="vietnamese" lastChineseScheme="zhuyin" onChange={onChange} />);

  const vietnamese = screen.getByLabelText("越南文") as HTMLInputElement;
  expect(vietnamese.checked).toBe(true);
  expect(vietnamese.disabled).toBe(true);
  // Zhuyin is not offered either, so host-api runs 全拼.
  expect(screen.getByText("此平台暂不支持越南文，已回退到全拼")).toBeTruthy();
  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "quanpin" });
});
