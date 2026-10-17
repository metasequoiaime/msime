// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { modelPickerModel, useModelPickerRows } from "@msime/ui";

afterEach(cleanup);

test("纯模型去重已知列表并判断当前值是否需要自定义输入", () => {
  expect(modelPickerModel(["a", "", "a", "b"], "a")).toEqual({
    known: ["a", "b"],
    listKey: "a\nb",
    custom: false,
  });
  expect(modelPickerModel(["a", "b"], "other").custom).toBe(true);
});

test("模型选择状态转发选择和编辑，并在列表变化后清除展开意图", () => {
  const onChange = vi.fn();
  const hook = renderHook(({ models, model }) => useModelPickerRows({ models, model, onChange }), {
    initialProps: { models: ["a", "b"], model: "a" },
  });

  expect(hook.result.current.custom).toBe(false);
  act(() => hook.result.current.select(""));
  expect(hook.result.current.custom).toBe(true);
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.edit("b"));
  expect(onChange).toHaveBeenLastCalledWith("b");
  expect(hook.result.current.custom).toBe(true);
  hook.rerender({ models: ["a", "b", "c"], model: "a" });
  expect(hook.result.current.custom).toBe(false);
});
