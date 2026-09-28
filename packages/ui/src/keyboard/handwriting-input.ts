import type { InkPoint } from "./panels";
import type { PointerEvent } from "react";

export type Point = InkPoint;
export const MAX_HANDWRITING_STROKES = 32;
export const WINDOWS_HANDWRITING_STROKES = 64;
const MAX_CAPTURED_POINTS = 256;

export function appendInkPoint(points: Point[], point: Point, endpoint = false): Point[] {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return points;
  const next = { x: Math.round(point.x * 100) / 100, y: Math.round(point.y * 100) / 100 };
  const last = points[points.length - 1];
  if (last?.x === next.x && last?.y === next.y) return points;
  if (last && !endpoint && Math.abs(last.x - next.x) + Math.abs(last.y - next.y) < 0.5)
    return points;
  if (points.length < MAX_CAPTURED_POINTS) return [...points, next];
  return [...points.filter((_, index) => index % 2 === 0), last, next];
}

export function pointFromCoordinates(
  canvas: SVGSVGElement,
  event: { clientX: number; clientY: number },
): Point {
  const matrix = canvas.getScreenCTM?.();
  if (matrix && canvas.createSVGPoint) {
    try {
      const pointer = canvas.createSVGPoint();
      pointer.x = event.clientX;
      pointer.y = event.clientY;
      const local = pointer.matrixTransform(matrix.inverse());
      if (Number.isFinite(local.x) && Number.isFinite(local.y)) {
        return { x: Math.max(0, Math.min(420, local.x)), y: Math.max(0, Math.min(420, local.y)) };
      }
    } catch {
      /* A detached or non-invertible canvas uses the bounded fallback. */
    }
  }
  const rect = canvas.getBoundingClientRect();
  const width = rect.width || 420;
  const height = rect.height || 420;
  return {
    x: Math.max(0, Math.min(420, ((event.clientX - rect.left) / width) * 420)),
    y: Math.max(0, Math.min(420, ((event.clientY - rect.top) / height) * 420)),
  };
}

export function appendPointerSamples(
  points: Point[],
  event: PointerEvent<SVGSVGElement>,
  endpoint = false,
): Point[] {
  let next = points;
  for (const sample of event.nativeEvent.getCoalescedEvents?.() ?? []) {
    if (sample.pointerId === event.pointerId) {
      next = appendInkPoint(next, pointFromCoordinates(event.currentTarget, sample));
    }
  }
  return appendInkPoint(next, pointFromCoordinates(event.currentTarget, event), endpoint);
}
