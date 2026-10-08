/**
 * 在字母键上滑动输入其角标符号（滑动输入符号），移植自 `platforms/android/java/app/msime/android/keyboard/SwipeHintPolicy.java`：手指从按下处沿所选方向（向下或向上）移动超过 14 vp，抬起时输入该键的角标（`LetterHintTable`），预览气泡也改为显示角标。长按同样输入角标，不论开关和方向如何设置。哪根手指在哪个键上、何时触发，由 `LetterHintGesture` 跟踪。
 */
/** 「滑动方向」的取值，命名和拼写与 Android 的 `platform.android.swipe_symbols_direction` 存储的一致。 */
export enum SwipeSymbolsDirection {
  DOWN = "down",
  UP = "up",
}

export class SwipeHintPolicy {
  /** 滑动阈值，单位 vp；只有严格大于它的距离才算。 */
  static readonly THRESHOLD_VP: number = 14;

  /** 本版本认识的已存储方向，其他任何值取 `fallback`。 */
  static direction(
    value: string | null | undefined,
    fallback: SwipeSymbolsDirection = SwipeSymbolsDirection.DOWN,
  ): SwipeSymbolsDirection {
    if (value === SwipeSymbolsDirection.DOWN) {
      return SwipeSymbolsDirection.DOWN;
    }
    if (value === SwipeSymbolsDirection.UP) {
      return SwipeSymbolsDirection.UP;
    }
    return fallback;
  }

  /** 手指是否已沿 `direction` 移动超过阈值；y 向下增长，单位 vp。 */
  static swiped(direction: SwipeSymbolsDirection, downY: number, currentY: number): boolean {
    const moved: number =
      direction === SwipeSymbolsDirection.UP ? downY - currentY : currentY - downY;
    return moved > SwipeHintPolicy.THRESHOLD_VP;
  }
}
