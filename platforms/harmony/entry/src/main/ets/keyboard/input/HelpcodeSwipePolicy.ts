/**
 * 26 键中文组字时的竖向滑动辅助码：上滑或下滑超过字母角标滑动的门槛，送出当前键的大写字母。
 * 只在按下时已经确认有全拼或双拼组字时接管；没有触发时，点按和长按仍走原来的路径。
 */
export class HelpcodeSwipePolicy {
  /** 与字母键角标滑动相同的物理门槛，单位 vp；严格大于才算。 */
  static readonly THRESHOLD_VP: number = 14;

  /** 竖向位移超过门槛且大于横向位移时，才算上滑或下滑辅助码。 */
  static swiped(downX: number, downY: number, currentX: number, currentY: number): boolean {
    const dx: number = currentX - downX;
    const dy: number = currentY - downY;
    return (
      Number.isFinite(dx) &&
      Number.isFinite(dy) &&
      Math.abs(dy) > HelpcodeSwipePolicy.THRESHOLD_VP &&
      Math.abs(dy) > Math.abs(dx)
    );
  }

  /** 辅助码手势送给引擎的字符：引擎从大小写读取辅助码，Shift 标志由调用方另传。 */
  static letter(letter: string): string {
    return letter.length === 1 && /^[a-zA-Z]$/.test(letter) ? letter.toUpperCase() : "";
  }
}

interface HelpcodeTouch {
  letter: string;
  downX: number;
  downY: number;
  triggered: boolean;
  held: boolean;
}

/**
 * 按手指跟踪一次辅助码滑动。点击可能先于抬手到达，因此触发后按键名会被吞掉；下一次按下同一键时清除。
 */
export class HelpcodeSwipeGesture {
  private readonly touches: Map<number, HelpcodeTouch> = new Map<number, HelpcodeTouch>();
  private readonly swallowed: Set<string> = new Set<string>();

  down(finger: number, letter: string, x: number, y: number, enabled: boolean): void {
    this.swallowed.delete(letter);
    this.touches.delete(finger);
    if (
      !enabled ||
      HelpcodeSwipePolicy.letter(letter).length === 0 ||
      !Number.isFinite(x) ||
      !Number.isFinite(y)
    ) {
      return;
    }
    this.touches.set(finger, {
      letter: letter,
      downX: x,
      downY: y,
      triggered: false,
      held: false,
    });
  }

  /** 手指移动过门槛时立即接管，返回要送出的字母；未触发返回空串。 */
  move(finger: number, x: number, y: number): string {
    const touch: HelpcodeTouch | undefined = this.touches.get(finger);
    if (
      touch === undefined ||
      touch.triggered ||
      touch.held ||
      !HelpcodeSwipePolicy.swiped(touch.downX, touch.downY, x, y)
    ) {
      return "";
    }
    touch.triggered = true;
    this.swallowed.add(touch.letter);
    return HelpcodeSwipePolicy.letter(touch.letter);
  }

  /** 长按已经接管为角标提示时，后续移动不再改成辅助码滑动。 */
  hold(finger: number): void {
    const touch: HelpcodeTouch | undefined = this.touches.get(finger);
    if (touch !== undefined && !touch.triggered) {
      touch.held = true;
    }
  }

  up(finger: number, x?: number, y?: number): string {
    const touch: HelpcodeTouch | undefined = this.touches.get(finger);
    if (
      touch !== undefined &&
      !touch.triggered &&
      !touch.held &&
      x !== undefined &&
      y !== undefined
    ) {
      this.move(finger, x, y);
    }
    this.touches.delete(finger);
    return touch !== undefined && touch.triggered ? HelpcodeSwipePolicy.letter(touch.letter) : "";
  }

  cancel(finger: number): void {
    this.touches.delete(finger);
  }

  tracking(finger: number): boolean {
    return this.touches.has(finger);
  }

  swallows(letter: string): boolean {
    return this.swallowed.has(letter);
  }

  clear(): void {
    this.touches.clear();
    this.swallowed.clear();
  }
}
