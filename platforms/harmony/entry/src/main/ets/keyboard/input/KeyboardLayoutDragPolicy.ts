import { KeyboardGeometry } from "../KeyboardGeometry";

/** 高度条把手的纯拖动计算，用于调整实时键盘的高度。 */
export class KeyboardLayoutDragPolicy {
  /** 拖动达到的高度调整：起始调整值减去手指的总垂直位移。屏幕上向上移动为负，因此让键盘变高，1vp 对 1vp，并受共享偏好上下限约束。 */
  static height(baseVp: number, offsetY: number): number {
    return KeyboardGeometry.heightAdjustment(Math.round(baseVp - offsetY));
  }
}
