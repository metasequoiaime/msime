import { KeyboardGeometry } from './KeyboardGeometry';

/**
 * Converts ArkUI pan offsets (vp) into a clamped status-bar position (px).
 *
 * The input method panel owns the window, while the toolbar component owns the gesture. Keeping the
 * conversion here makes the boundary deterministic and prevents a drag from moving the panel off
 * the display when a finger ends outside the screen.
 */
export class FloatingToolbarDragPolicy {
  static position(origin: readonly [number, number], offsetVp: readonly [number, number],
                  density: number, screenWidth: number, screenHeight: number,
                  toolbarWidth: number, toolbarHeight: number): [number, number] {
    if (!Number.isFinite(density) || density <= 0 || !Number.isFinite(screenWidth)
        || !Number.isFinite(screenHeight) || !Number.isFinite(toolbarWidth)
        || !Number.isFinite(toolbarHeight)) {
      return [Math.round(origin[0]), Math.round(origin[1])];
    }
    const x: number = origin[0] + offsetVp[0] * density;
    const y: number = origin[1] + offsetVp[1] * density;
    const maxX: number = Math.max(0, screenWidth - toolbarWidth);
    const maxY: number = Math.max(0, screenHeight - toolbarHeight);
    return [
      Math.round(KeyboardGeometry.bounded(x, 0, maxX)),
      Math.round(KeyboardGeometry.bounded(y, 0, maxY))
    ];
  }
}
