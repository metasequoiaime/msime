/**
 * 26 个字母键上那些可能输入角标提示而不是字母本身的触摸，移植自 Android `ImeLetterRows.bindLetterGestures` 的手势部分：滑动输入符号开启时沿所选方向滑过 SwipeHintPolicy.THRESHOLD_VP，或者无论该设置如何都按住 HOLD_MILLIS。两者都在手指抬起时输入提示；被取消的触摸什么也不输入。
 *
 * 触摸按手指分别跟踪，所以两个拇指按在两个键上各自保留状态，正如 Android 每个键各自保留状态。视图把 ArkUI 报告的事件喂给它，并负责绘制和输入；这里不涉及 ArkUI。
 */
import { SwipeHintPolicy, SwipeSymbolsDirection } from "./SwipeHintPolicy";

/** 按键在读取触摸所用坐标空间（vp）中的矩形。 */
export interface HintKeyRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

interface HintTouch {
  letter: string;
  hint: string;
  downY: number;
  rect: HintKeyRect;
  /** 滑动或长按已经触发，所以抬起时输入提示，按键自身的点击被吞掉。 */
  triggered: boolean;
  /** 手指离开按键超过 TOUCH_SLOP_VP；与 Android 的按下状态一样，回到键上也不会恢复：长按不再触发。 */
  outside: boolean;
}

export class LetterHintGesture {
  /** 长按多久才输入提示：平台 LongPressGesture 的默认值，正如 Android 使用系统的长按超时。 */
  static readonly HOLD_MILLIS: number = 500;
  /** 手指可以偏离按键多远仍算按住它，即 Android 的默认 touch slop。 */
  static readonly TOUCH_SLOP_VP: number = 8;

  private readonly touches: Map<number, HintTouch> = new Map<number, HintTouch>();
  /** 因为触摸已输入提示而不能再输入的字母；同一字母的下一次按下时清除，所以无论点击和抬起哪个先到都成立。 */
  private readonly swallowed: Set<string> = new Set<string>();

  /** 手指按在了一个带提示的字母键上。 */
  down(finger: number, letter: string, hint: string, y: number, rect: HintKeyRect): void {
    this.swallowed.delete(letter);
    this.touches.set(finger, {
      letter: letter,
      hint: hint,
      downY: y,
      rect: rect,
      triggered: false,
      outside: false,
    });
  }

  /** 手指移动到 (x, y)。若这次移动刚好完成滑动则返回提示，否则返回 null。 */
  move(
    finger: number,
    x: number,
    y: number,
    swipeEnabled: boolean,
    direction: SwipeSymbolsDirection,
  ): string | null {
    const touch: HintTouch | undefined = this.touches.get(finger);
    if (touch === undefined || touch.triggered) {
      return null;
    }
    if (!LetterHintGesture.near(touch.rect, x, y)) {
      touch.outside = true;
    }
    if (swipeEnabled && SwipeHintPolicy.swiped(direction, touch.downY, y)) {
      return this.trigger(touch);
    }
    return null;
  }

  /** 这根手指的长按计时器到期。若因此触发手势则返回提示；若触摸已结束、已经触发或已偏离按键则返回 null。 */
  hold(finger: number): string | null {
    const touch: HintTouch | undefined = this.touches.get(finger);
    if (touch === undefined || touch.triggered || touch.outside) {
      return null;
    }
    return this.trigger(touch);
  }

  /** 手指抬起：手势已触发时返回要输入的提示，否则返回 null，由按键自身的点击输入字母。 */
  up(finger: number): string | null {
    const touch: HintTouch | undefined = this.touches.get(finger);
    if (touch === undefined) {
      return null;
    }
    this.touches.delete(finger);
    return touch.triggered ? touch.hint : null;
  }

  /** 触摸被收走（取消，或被滑行输入接管）：不为它输入任何内容。 */
  cancel(finger: number): void {
    this.touches.delete(finger);
  }

  /** 这根手指是否按在一个正被跟踪的字母键上。 */
  tracking(finger: number): boolean {
    return this.touches.has(finger);
  }

  /** 这根手指按下的按键；未被跟踪时为 null。 */
  rect(finger: number): HintKeyRect | null {
    const touch: HintTouch | undefined = this.touches.get(finger);
    return touch === undefined ? null : touch.rect;
  }

  /** `letter` 的点击是否要忽略，因为它的触摸已经输入了提示。 */
  swallows(letter: string): boolean {
    return this.swallowed.has(letter);
  }

  /** 触摸下方的按键被替换时，清空所有状态。 */
  clear(): void {
    this.touches.clear();
    this.swallowed.clear();
  }

  private trigger(touch: HintTouch): string {
    touch.triggered = true;
    this.swallowed.add(touch.letter);
    return touch.hint;
  }

  private static near(rect: HintKeyRect, x: number, y: number): boolean {
    const slop: number = LetterHintGesture.TOUCH_SLOP_VP;
    return (
      x >= rect.left - slop &&
      x <= rect.left + rect.width + slop &&
      y >= rect.top - slop &&
      y <= rect.top + rect.height + slop
    );
  }
}
