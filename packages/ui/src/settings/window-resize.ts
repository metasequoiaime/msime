import type { WindowResizeEdge } from "../keyboard/window-host";

export interface WindowResizePoint {
  clientX: number;
  clientY: number;
}

export interface WindowResizeBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Returns the resize edge or corner occupied by a pointer, or null for the window interior. */
export function windowResizeEdge(
  point: WindowResizePoint,
  bounds: WindowResizeBounds,
  edge = 8,
): WindowResizeEdge | null {
  const north = point.clientY - bounds.top < edge;
  const south = bounds.bottom - point.clientY < edge;
  const west = point.clientX - bounds.left < edge;
  const east = bounds.right - point.clientX < edge;
  return north && east
    ? "ne"
    : north && west
      ? "nw"
      : south && east
        ? "se"
        : south && west
          ? "sw"
          : north
            ? "n"
            : south
              ? "s"
              : east
                ? "e"
                : west
                  ? "w"
                  : null;
}
