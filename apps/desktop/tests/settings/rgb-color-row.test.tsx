// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { RgbColorRow } from "@msime/ui";

afterEach(cleanup);

function picker(title = "合成颜色") {
  return screen.getByLabelText(title) as HTMLInputElement;
}

test("显示规范化六位颜色和原生取色器", () => {
  render(<RgbColorRow title="合成颜色" value={0xabc} onChange={vi.fn()} />);
  expect(screen.getByText("#000ABC")).toBeTruthy();
  expect(picker().type).toBe("color");
  expect(picker().value).toBe("#000abc");
});

test("取色器把十六进制字符串转换为整数颜色", () => {
  const onChange = vi.fn();
  render(<RgbColorRow title="合成颜色" value={0x123456} onChange={onChange} />);
  fireEvent.change(picker(), { target: { value: "#abcdef" } });
  expect(onChange).toHaveBeenCalledWith(0xabcdef);
});

test("隐藏交给标准设置行且仍保留颜色控件", () => {
  render(<RgbColorRow title="隐藏颜色" value={0x123456} hidden onChange={vi.fn()} />);
  const row = screen.getByText("隐藏颜色").closest("div");
  expect(row?.hasAttribute("hidden")).toBe(true);
  expect(picker("隐藏颜色")).toBeTruthy();
});

test("多个颜色行互不共享受控回调", () => {
  const first = vi.fn();
  const second = vi.fn();
  render(
    <>
      <RgbColorRow title="第一颜色" value={0x123456} onChange={first} />
      <RgbColorRow title="第二颜色" value={0x654321} onChange={second} />
    </>,
  );
  fireEvent.change(picker("第一颜色"), { target: { value: "#abcdef" } });
  expect(first).toHaveBeenCalledWith(0xabcdef);
  expect(second).not.toHaveBeenCalled();
});

test("外部受控值更新色块和文本显示", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <RgbColorRow title="合成颜色" value={0x123456} onChange={onChange} />,
  );
  rerender(<RgbColorRow title="合成颜色" value={0xfedcba} onChange={onChange} />);
  expect(screen.getByText("#FEDCBA")).toBeTruthy();
  expect(picker().value).toBe("#fedcba");
});
