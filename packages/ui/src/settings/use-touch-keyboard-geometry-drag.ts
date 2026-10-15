import {
  useRef,
  type Dispatch,
  type PointerEvent as ReactPointerEvent,
  type SetStateAction,
} from "react";
import { clamp } from "../core/number";
import type { Preferences } from "../index";
import { touchKeyboardGeometryPreferences } from "./touch-keyboard-geometry-preferences";
import { touchKeyboardSpacingLimits } from "./touch-keyboard-spacing";

type TouchGeometryDrag = {
  pointerId: number;
  x: number;
  y: number;
  key: number;
  row: number;
  axis: "key" | "row" | null;
};

/** Provides the drag handlers for the screen keyboard spacing preview. */
export function useTouchKeyboardGeometryDrag(
  draft: Preferences | undefined,
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>,
) {
  const dragRef = useRef<TouchGeometryDrag | null>(null);

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!draft || (event.pointerType === "mouse" && event.button !== 0)) return;
    const geometry = touchKeyboardGeometryPreferences(draft);
    dragRef.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      key: geometry.keySpacingTenths,
      row: geometry.rowSpacingTenths,
      axis: null,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId || !draft) return;
    const dx = event.clientX - drag.x;
    const dy = event.clientY - drag.y;
    if (!drag.axis && Math.abs(dx) + Math.abs(dy) < 4) return;
    drag.axis ??= Math.abs(dy) >= Math.abs(dx) ? "row" : "key";
    const delta = drag.axis === "row" ? dy : dx;
    const value = Math.round((drag.axis === "row" ? drag.row : drag.key) + (delta * 10) / 18);
    const limits = touchKeyboardSpacingLimits[drag.axis];
    const spacing = clamp(value, limits.min, limits.max);
    setDraft((current) =>
      current
        ? {
            ...current,
            ...(drag.axis === "row"
              ? { touch_row_spacing_tenths: spacing }
              : { touch_key_spacing_tenths: spacing }),
          }
        : current,
    );
  };

  const onPointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
  };

  return { onPointerDown, onPointerMove, onPointerUp, onPointerCancel: onPointerUp };
}
