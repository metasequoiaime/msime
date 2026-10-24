// @vitest-environment jsdom
import { useState } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useTouchKeyboardSkinEditorState, type TouchKeyboardSkinDesign } from "@msime/ui";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

function controlled(initial: TouchKeyboardSkinDesign = { ...defaultTouchKeyboardSkinDesign }) {
  const changed = vi.fn();
  const hook = renderHook(() => {
    const [design, setDesign] = useState(initial);
    return {
      design,
      ...useTouchKeyboardSkinEditorState({
        design,
        onChange: (next) => {
          changed(next);
          setDesign(next);
        },
      }),
    };
  });
  return { ...hook, changed };
}

test("初始历史为空，空撤销和重做不通知调用方", () => {
  const { result, changed } = controlled();
  expect(result.current.canUndo).toBe(false);
  expect(result.current.canRedo).toBe(false);
  act(() => {
    result.current.undo();
    result.current.redo();
  });
  expect(changed).not.toHaveBeenCalled();
});

test("应用完整设计先规范化，不改输入或当前受控值", () => {
  const initial = Object.freeze({ ...defaultTouchKeyboardSkinDesign });
  const changed = vi.fn();
  const { result } = renderHook(() =>
    useTouchKeyboardSkinEditorState({ design: initial, onChange: changed }),
  );
  const next = Object.freeze({
    ...initial,
    background: 0x1234567,
    cornerRadius: 99,
    borderWidth: -4,
    shadow: Number.NaN,
    keyOpacity: 0.1,
  });
  act(() => result.current.apply(next));
  expect(changed).toHaveBeenCalledWith(
    expect.objectContaining({
      background: 0x234567,
      cornerRadius: 20,
      borderWidth: 0,
      shadow: 0,
      keyOpacity: 0.25,
    }),
  );
  expect(initial.cornerRadius).toBe(defaultTouchKeyboardSkinDesign.cornerRadius);
  expect(next.cornerRadius).toBe(99);
  expect(result.current.canUndo).toBe(true);
});

test("补丁保留其他设计字段，并允许清掉可选字段", () => {
  const { result } = controlled({
    ...defaultTouchKeyboardSkinDesign,
    photo: "data:image/png;base64,YQ==",
    gradientEnd: 0x123456,
    keyShape: "ticket",
  });
  act(() => result.current.patch({ gradientEnd: undefined, cornerRadius: 7 }));
  expect(result.current.design).toMatchObject({
    photo: "data:image/png;base64,YQ==",
    keyShape: "ticket",
    cornerRadius: 7,
  });
  expect(result.current.design.gradientEnd).toBeUndefined();
});

test("规范化后无变化不通知或记历史", () => {
  const { result, changed } = controlled();
  act(() =>
    result.current.patch({ background: defaultTouchKeyboardSkinDesign.background + 0x1000000 }),
  );
  expect(changed).not.toHaveBeenCalled();
  expect(result.current.canUndo).toBe(false);
});

test("撤销和重做依次恢复完整快照", () => {
  const { result } = controlled();
  const first = result.current.design;
  act(() => result.current.patch({ cornerRadius: 2, pattern: 1 }));
  const second = result.current.design;
  act(() => result.current.patch({ cornerRadius: 3, keyMaterial: "glass" }));
  const third = result.current.design;
  act(() => result.current.undo());
  expect(result.current.design).toEqual(second);
  act(() => result.current.undo());
  expect(result.current.design).toEqual(first);
  expect(result.current.canUndo).toBe(false);
  act(() => result.current.redo());
  expect(result.current.design).toEqual(second);
  act(() => result.current.redo());
  expect(result.current.design).toEqual(third);
  expect(result.current.canRedo).toBe(false);
});

test("撤销后无变化编辑保留重做，实际分叉编辑清掉重做", () => {
  const { result, changed } = controlled();
  act(() => result.current.patch({ cornerRadius: 2 }));
  act(() => result.current.undo());
  const calls = changed.mock.calls.length;
  act(() => result.current.apply({ ...result.current.design }));
  expect(changed).toHaveBeenCalledTimes(calls);
  expect(result.current.canRedo).toBe(true);
  act(() => result.current.patch({ cornerRadius: 4 }));
  expect(result.current.canRedo).toBe(false);
  act(() => result.current.undo());
  expect(result.current.design).toEqual(defaultTouchKeyboardSkinDesign);
});

test("历史只保留最近 30 步，撤销和重做都在边界停下", () => {
  const { result, changed } = controlled();
  for (let n = 1; n <= 35; n++) act(() => result.current.patch({ background: n }));
  for (let n = 0; n < 30; n++) act(() => result.current.undo());
  expect(result.current.design.background).toBe(5);
  expect(result.current.canUndo).toBe(false);
  const afterUndo = changed.mock.calls.length;
  act(() => result.current.undo());
  expect(changed).toHaveBeenCalledTimes(afterUndo);
  for (let n = 0; n < 30; n++) act(() => result.current.redo());
  expect(result.current.design.background).toBe(35);
  expect(result.current.canRedo).toBe(false);
});

test("重置默认设计走同一历史流程，可撤销；已经默认时不通知", () => {
  const initial = { ...defaultTouchKeyboardSkinDesign, pattern: 3 as const };
  const { result, changed } = controlled(initial);
  act(() => result.current.reset());
  expect(result.current.design).toEqual(defaultTouchKeyboardSkinDesign);
  const calls = changed.mock.calls.length;
  act(() => result.current.reset());
  expect(changed).toHaveBeenCalledTimes(calls);
  act(() => result.current.undo());
  expect(result.current.design).toEqual(initial);
});

test.each([
  { background: 0xffffff, keyBackground: 0xffffff, keyForeground: 0, accent: 0 },
  { background: 0, keyBackground: 0, keyForeground: 0xffffff, accent: 0xffffff },
  { background: 0, keyBackground: 0xffffff, keyForeground: 0, accent: 0 },
])("优化对比度按背景 $background 与键帽 $keyBackground 取原有最差值规则", (colors) => {
  const initial = {
    ...defaultTouchKeyboardSkinDesign,
    background: colors.background,
    keyBackground: colors.keyBackground,
    gradientEnd: 0x777777,
    keyForeground: 0x123456,
    accent: 0x123456,
  };
  const { result } = controlled(initial);
  act(() => result.current.optimizeContrast());
  expect(result.current.design).toMatchObject(colors);
  expect(result.current.design.gradientEnd).toBe(0x777777);
  act(() => result.current.undo());
  expect(result.current.design).toEqual(initial);
});

test("设计由外部持有，重渲染使用最新设计和回调并保留历史", () => {
  const first = vi.fn();
  const second = vi.fn();
  const { result, rerender } = renderHook((props) => useTouchKeyboardSkinEditorState(props), {
    initialProps: { design: { ...defaultTouchKeyboardSkinDesign }, onChange: first },
  });
  act(() => result.current.patch({ cornerRadius: 2 }));
  expect(first).toHaveBeenCalledOnce();
  const replacement = {
    ...defaultTouchKeyboardSkinDesign,
    keyShape: "ticket" as const,
    cornerRadius: 3,
  };
  rerender({ design: replacement, onChange: second });
  expect(result.current.canUndo).toBe(true);
  act(() => result.current.patch({ shadow: 0.1 }));
  expect(second).toHaveBeenLastCalledWith(
    expect.objectContaining({ keyShape: "ticket", cornerRadius: 3, shadow: 0.1 }),
  );
  rerender({ design: second.mock.calls.at(-1)![0], onChange: second });
  act(() => result.current.undo());
  expect(second).toHaveBeenLastCalledWith(expect.objectContaining(replacement));
  expect(first).toHaveBeenCalledOnce();
});

test("独立编辑实例不共享历史", () => {
  const first = controlled();
  const second = controlled();
  act(() => first.result.current.patch({ cornerRadius: 2 }));
  expect(first.result.current.canUndo).toBe(true);
  expect(second.result.current.canUndo).toBe(false);
  expect(second.changed).not.toHaveBeenCalled();
});
