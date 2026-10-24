// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HexColorRow } from "@msime/ui";

afterEach(cleanup);

function controlled(initial: string) {
  const changed = vi.fn();
  function Controlled() {
    const [value, setValue] = useState(initial);
    return (
      <HexColorRow
        title="合成颜色"
        value={value}
        onChange={(next) => {
          changed(next);
          setValue(next);
        }}
      />
    );
  }
  render(<Controlled />);
  return changed;
}

function textInput(title = "合成颜色") {
  return screen.getByRole("textbox", { name: `${title}色值` }) as HTMLInputElement;
}

function picker(title = "合成颜色") {
  return screen.getByLabelText(`${title}取色器`) as HTMLInputElement;
}

test("RGB 颜色显示原生色块和受控色值，输入保留现有约束", () => {
  render(<HexColorRow title="合成颜色" value="#A1B2C3" onChange={vi.fn()} />);
  expect(picker().type).toBe("color");
  expect(picker().value).toBe("#a1b2c3");
  expect(textInput().value).toBe("#A1B2C3");
  expect(textInput().getAttribute("aria-invalid")).toBe("false");
  expect(textInput().maxLength).toBe(9);
  expect(textInput().getAttribute("spellcheck")).toBe("false");
  expect(screen.queryByText(/%$/)).toBeNull();
});

test("取色替换 RGB 并保留 RGBA 的透明度，百分比只作视觉提示", () => {
  const changed = controlled("#A1B2C380");
  expect(picker().value).toBe("#a1b2c3");
  expect(screen.getByText("50%").getAttribute("aria-hidden")).toBe("true");
  fireEvent.change(picker(), { target: { value: "#abcdef" } });
  expect(changed).toHaveBeenLastCalledWith("#ABCDEF80");
  expect(textInput().value).toBe("#ABCDEF80");
});

test("没有透明度时取色不附加 alpha", () => {
  const changed = controlled("#A1B2C3");
  fireEvent.change(picker(), { target: { value: "#abcdef" } });
  expect(changed).toHaveBeenLastCalledWith("#ABCDEF");
  expect(textInput().value).toBe("#ABCDEF");
});

test.each([
  ["  #abc  ", "#AABBCC"],
  ["#abcd", "#AABBCCDD"],
  ["#a1b2c3", "#A1B2C3"],
  ["#a1b2c300", "#A1B2C300"],
])("文本输入 %s 经现有规则规范化为 %s", (input, normalized) => {
  const changed = controlled("#123456");
  fireEvent.change(textInput(), { target: { value: input } });
  expect(changed).toHaveBeenLastCalledWith(normalized);
  expect(textInput().value).toBe(normalized);
  expect(textInput().getAttribute("aria-invalid")).toBe("false");
});

test("不完整、非法和非十六进制文本只改草稿，失焦恢复当前颜色", () => {
  const changed = controlled("#123456");
  for (const input of ["#12", "#GGHHII", "transparent", "rgb(1,2,3)", ""]) {
    fireEvent.change(textInput(), { target: { value: input } });
    expect(textInput().value).toBe(input);
    expect(textInput().getAttribute("aria-invalid")).toBe("true");
    expect(picker().value).toBe("#123456");
    expect(changed).not.toHaveBeenCalled();
    fireEvent.blur(textInput());
    expect(textInput().value).toBe("#123456");
    expect(textInput().getAttribute("aria-invalid")).toBe("false");
  }
});

test("失焦恢复的是最新已接受颜色", () => {
  controlled("#123456");
  fireEvent.change(textInput(), { target: { value: "#abcdef" } });
  fireEvent.change(textInput(), { target: { value: "#ab" } });
  fireEvent.blur(textInput());
  expect(textInput().value).toBe("#ABCDEF");
});

test("调用方持有颜色，外部更新同步草稿并使用最新回调及透明度", () => {
  const first = vi.fn();
  const second = vi.fn();
  const { rerender } = render(<HexColorRow title="合成颜色" value="#12345680" onChange={first} />);
  fireEvent.change(picker(), { target: { value: "#abcdef" } });
  expect(first).toHaveBeenLastCalledWith("#ABCDEF80");
  expect(textInput().value).toBe("#12345680");
  fireEvent.change(textInput(), { target: { value: "#ab" } });
  rerender(<HexColorRow title="合成颜色" value="#65432100" onChange={second} />);
  expect(textInput().value).toBe("#65432100");
  expect(screen.getByText("0%")).toBeTruthy();
  fireEvent.change(picker(), { target: { value: "#abcdef" } });
  expect(second).toHaveBeenLastCalledWith("#ABCDEF00");
  expect(first).toHaveBeenCalledOnce();
});

test("多个颜色行的文本草稿独立", () => {
  const first = vi.fn();
  const second = vi.fn();
  render(
    <>
      <HexColorRow title="第一颜色" value="#123456" onChange={first} />
      <HexColorRow title="第二颜色" value="#654321" onChange={second} />
    </>,
  );
  fireEvent.change(textInput("第一颜色"), { target: { value: "#ab" } });
  expect(textInput("第二颜色").value).toBe("#654321");
  expect(textInput("第二颜色").getAttribute("aria-invalid")).toBe("false");
  expect(first).not.toHaveBeenCalled();
  expect(second).not.toHaveBeenCalled();
});
