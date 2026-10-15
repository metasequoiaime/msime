// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { useState, type PointerEvent } from "react";
import { afterEach, expect, test } from "vitest";
import type { Preferences } from "@msime/ui";
import { useTouchKeyboardGeometryDrag } from "../../../../packages/ui/src/settings/use-touch-keyboard-geometry-drag";

afterEach(cleanup);

const initial: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

test("captures current spacing on each press and keeps it during draft changes", () => {
  const { result } = renderHook(() => {
    const [draft, setDraft] = useState<Preferences | undefined>(initial);
    return { draft, setDraft, ...useTouchKeyboardGeometryDrag(draft, setDraft) };
  });
  const currentTarget = document.createElement("div");
  const pointer = (clientX: number, clientY: number) =>
    ({
      pointerId: 1,
      pointerType: "touch",
      button: 0,
      clientX,
      clientY,
      currentTarget,
    }) as PointerEvent<HTMLDivElement>;

  act(() => result.current.onPointerDown(pointer(100, 100)));
  act(() => result.current.onPointerMove(pointer(82, 100)));
  expect(result.current.draft?.touch_key_spacing_tenths).toBe(50);
  act(() => result.current.onPointerUp(pointer(82, 100)));

  act(() =>
    result.current.setDraft({
      ...initial,
      touch_key_spacing_tenths: 35,
      touch_row_spacing_tenths: 80,
    }),
  );
  act(() => result.current.onPointerDown(pointer(100, 100)));
  act(() =>
    result.current.setDraft((draft) =>
      draft ? { ...draft, touch_key_spacing_tenths: 55, learning: false } : draft,
    ),
  );
  act(() => result.current.onPointerMove(pointer(118, 100)));
  expect(result.current.draft?.touch_key_spacing_tenths).toBe(45);
  expect(result.current.draft?.touch_row_spacing_tenths).toBe(80);
  expect(result.current.draft?.learning).toBe(false);
  act(() => result.current.onPointerUp(pointer(118, 100)));

  act(() => result.current.onPointerDown(pointer(100, 100)));
  act(() => result.current.onPointerMove(pointer(100, 118)));
  expect(result.current.draft?.touch_row_spacing_tenths).toBe(90);
});

test.each([
  [-1000, 0, 30, undefined],
  [1000, 0, 60, undefined],
  [0, -1000, undefined, 40],
  [0, 1000, undefined, 100],
])("bounds spacing after a drag by %s/%s", (x, y, key, row) => {
  const { result } = renderHook(() => {
    const [draft, setDraft] = useState<Preferences | undefined>(initial);
    return { draft, ...useTouchKeyboardGeometryDrag(draft, setDraft) };
  });
  const currentTarget = document.createElement("div");
  const pointer = (clientX: number, clientY: number) =>
    ({
      pointerId: 1,
      pointerType: "touch",
      clientX,
      clientY,
      currentTarget,
    }) as PointerEvent<HTMLDivElement>;
  act(() => result.current.onPointerDown(pointer(0, 0)));
  act(() => result.current.onPointerMove(pointer(x, y)));
  expect(result.current.draft?.touch_key_spacing_tenths).toBe(key);
  expect(result.current.draft?.touch_row_spacing_tenths).toBe(row);
  expect(result.current.draft?.learning).toBe(true);
});
