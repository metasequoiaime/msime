// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";
import type { VoiceResultsProps } from "@msime/ui";

afterEach(cleanup);

function props(overrides: Partial<VoiceResultsProps> = {}): VoiceResultsProps {
  return {
    text: "合成结果",
    exceedsSubmitLimit: false,
    inputAvailable: true,
    copyAvailable: true,
    busy: false,
    submitting: false,
    copying: false,
    onTextChange: vi.fn(),
    onSubmit: vi.fn(),
    onClear: vi.fn(),
    ...overrides,
  };
}

test("公共入口提供语音结果组件", () => {
  expect(ui.VoiceResults).toBeTypeOf("function");
});

test("受控文本保留编辑限制，录音和提交中仍可编辑", () => {
  const state = props({ busy: true, submitting: true });
  const view = render(<ui.VoiceResults {...state} />);
  const input = screen.getByRole<HTMLTextAreaElement>("textbox", { name: "识别结果" });
  expect(input.value).toBe("合成结果");
  expect(input.maxLength).toBe(4096);
  expect(input.rows).toBe(4);
  expect(input.placeholder).toBe("识别结果会显示在这里");
  expect(input.disabled).toBe(false);
  expect(input.readOnly).toBe(false);
  fireEvent.change(input, { target: { value: "合成新结果" } });
  expect(state.onTextChange).toHaveBeenCalledExactlyOnceWith("合成新结果");
  view.rerender(<ui.VoiceResults {...state} text="合成新结果" />);
  expect(input.value).toBe("合成新结果");
});

test("结果控件保持同级顺序，提交和复制转发原参数", () => {
  const state = props();
  const { container } = render(<ui.VoiceResults {...state} />);
  const input = screen.getByRole("textbox", { name: "识别结果" });
  const submit = screen.getByRole("button", { name: "提交到当前窗口" });
  const copy = screen.getByRole("button", { name: "复制结果" });
  const clear = screen.getByRole("button", { name: "清空结果" });
  expect(Array.from(container.children)).toEqual([input, submit, copy, clear]);
  expect(input.hasAttribute("aria-invalid")).toBe(false);
  expect(input.hasAttribute("aria-describedby")).toBe(false);
  fireEvent.click(submit);
  expect(state.onSubmit).toHaveBeenCalledExactlyOnceWith();
  vi.mocked(state.onSubmit).mockClear();
  fireEvent.click(copy);
  expect(state.onSubmit).toHaveBeenCalledExactlyOnceWith(true);
  fireEvent.click(clear);
  expect(state.onClear).toHaveBeenCalledOnce();
});

test("没有复制能力时不显示复制按钮", () => {
  render(<ui.VoiceResults {...props({ copyAvailable: false })} />);
  expect(screen.queryByRole("button", { name: "复制结果" })).toBeNull();
  expect(screen.getAllByRole("button")).toHaveLength(2);
});

test("超限提示关联文本框并阻止提交，复制与清空仍可用", () => {
  const state = props({ exceedsSubmitLimit: true });
  const { container } = render(<ui.VoiceResults {...state} />);
  const input = screen.getByRole("textbox", { name: "识别结果" });
  const status = screen.getByRole("status");
  expect(status.id).toBe("voice-result-limit");
  expect(input.getAttribute("aria-describedby")).toBe(status.id);
  expect(input.getAttribute("aria-invalid")).toBe("true");
  expect(Array.from(container.children).slice(0, 2)).toEqual([input, status]);
  fireEvent.click(screen.getByRole("button", { name: "提交到当前窗口" }));
  expect(state.onSubmit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "复制结果" }));
  expect(state.onSubmit).toHaveBeenCalledExactlyOnceWith(true);
  fireEvent.click(screen.getByRole("button", { name: "清空结果" }));
  expect(state.onClear).toHaveBeenCalledOnce();
});

test.each<[string, Partial<VoiceResultsProps>, boolean, boolean, boolean]>([
  ["空文本", { text: "" }, true, true, true],
  ["空闲", {}, false, false, false],
  ["无输入能力", { inputAvailable: false }, true, false, false],
  ["录音中", { busy: true }, true, true, true],
  ["提交中", { submitting: true }, true, true, false],
  ["超限", { exceedsSubmitLimit: true }, true, false, false],
])("%s 保留提交、复制与清空的不同禁用条件", (_, overrides, submitOff, copyOff, clearOff) => {
  render(<ui.VoiceResults {...props(overrides)} />);
  expect(screen.getByRole<HTMLButtonElement>("button", { name: /提交/ }).disabled).toBe(submitOff);
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "复制结果" }).disabled).toBe(
    copyOff,
  );
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "清空结果" }).disabled).toBe(
    clearOff,
  );
});

test.each([false, true])("处理中标签区分提交与复制（copying=%s）", (copying) => {
  render(<ui.VoiceResults {...props({ submitting: true, copying })} />);
  expect(
    screen.getByRole<HTMLButtonElement>("button", {
      name: copying ? "提交到当前窗口" : "正在提交…",
    }).disabled,
  ).toBe(true);
  expect(
    screen.getByRole<HTMLButtonElement>("button", {
      name: copying ? "正在复制…" : "复制结果",
    }).disabled,
  ).toBe(true);
});
