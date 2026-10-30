/**
 * 拼音九键字母层 1–9 键的上滑或下滑（#10229），移植自 Android 的 `NineKeySwipePolicy.gesture`：共享偏好 `touch_nine_key_swipe_digits` 选了方向时，沿这个方向滑过 24 vp 松手输入这个键的数字，往反方向滑过 24 vp 弹出和长按相同的数字和字母选项。Android 的阈值是本机「九键滑动距离」（默认 24 dp），鸿蒙与 iOS 固定 24，是同一个物理尺寸。
 *
 * 只看纵向位移，严格大于阈值才算，不看横向位移，与 Android 相同。一次触摸只判定一次：不是 `NONE` 就锁住，之后手指怎么移动（包括滑回原处）都不改结果；长按先弹出时这次触摸不再判定。1 键（@#）没有字母，反方向什么也不做，抬手照常是一次点按。
 *
 * ArkUI 送这个键的 `onClick` 与它的抬手 `onTouch` 谁先谁后没有契约，所以判定只在移动时作出，抬手和点击都只照这个判定行事：滑出去的这次触摸，点击一律吞掉。这里不涉及 ArkUI，坐标是视图读到的窗口坐标，单位 vp。
 */

/** 一次按压在滑动判定上的结果，与 Android 的 `NineKeySwipePolicy.Gesture` 同名同义。 */
export enum NineKeySwipe {
  /** 还没滑过阈值，或设置是关闭：照常是点按或长按。 */
  NONE = "none",
  /** 沿设置方向滑过阈值：松手输入键上的数字。 */
  DIGIT = "digit",
  /** 往反方向滑过阈值：弹出数字和字母选项。 */
  LETTERS = "letters",
}

export class NineKeySwipePolicy {
  /** 设置取值：关闭（默认），没有滑动手势。 */
  static readonly OFF: string = "off";
  /** 上滑输入数字、下滑弹出字母。 */
  static readonly UP: string = "up";
  /** 下滑输入数字、上滑弹出字母。 */
  static readonly DOWN: string = "down";
  /** 滑动阈值，单位 vp；只有严格大于它的纵向位移才算，两个方向共用。 */
  static readonly THRESHOLD_VP: number = 24;

  /** `touch_nine_key_swipe_digits` 的值：缺键、不是字符串或本版本不认识的写法都按关闭，与 client-core 宽松读取的规则相同。 */
  static mode(value: unknown): string {
    if (value === NineKeySwipePolicy.UP) {
      return NineKeySwipePolicy.UP;
    }
    return value === NineKeySwipePolicy.DOWN ? NineKeySwipePolicy.DOWN : NineKeySwipePolicy.OFF;
  }

  /**
   * 当前位置构成哪种滑动。
   *
   * @param mode `off`、`up` 或 `down`；其他取值按关闭处理
   * @param downY 按下时的纵坐标（vp，向下为正）
   * @param currentY 当前纵坐标（vp）
   * @param hasLetters 这个键有没有字母（1 键是「@#」，没有字母可弹）
   */
  static gesture(mode: string, downY: number, currentY: number, hasLetters: boolean): NineKeySwipe {
    if (mode !== NineKeySwipePolicy.UP && mode !== NineKeySwipePolicy.DOWN) {
      return NineKeySwipe.NONE;
    }
    if (!Number.isFinite(downY) || !Number.isFinite(currentY)) {
      return NineKeySwipe.NONE;
    }
    const upward: number = downY - currentY;
    const along: number = mode === NineKeySwipePolicy.UP ? upward : -upward;
    if (along > NineKeySwipePolicy.THRESHOLD_VP) {
      return NineKeySwipe.DIGIT;
    }
    if (hasLetters && -along > NineKeySwipePolicy.THRESHOLD_VP) {
      return NineKeySwipe.LETTERS;
    }
    return NineKeySwipe.NONE;
  }
}

interface SwipePress {
  finger: number;
  downY: number;
  /** 按下时读到的设置；这次触摸按它走完，设置页改了下一次触摸生效。 */
  mode: string;
  hasLetters: boolean;
  /** 已经锁定的判定。 */
  result: NineKeySwipe;
  /** 长按先弹出，这次触摸不再判定滑动。 */
  held: boolean;
  /** 手指还按着：抬手或取消之后为 false，只剩下吞掉随后那次点击的用处。 */
  active: boolean;
}

/** 九键各键上正在进行的按压，按键的输入字符（`NineKey.input`）区分，两个拇指按在两个键上互不影响。 */
export class NineKeySwipeGesture {
  private readonly presses: Map<string, SwipePress> = new Map<string, SwipePress>();

  /** 手指按在 `key` 键上，这一键从这次按压重新开始判定：同一键上原来那根手指的移动和抬手从此不再算，键被替换时漏掉的抬手也不会留下来挡住新的按压。 */
  down(key: string, finger: number, y: number, mode: string, hasLetters: boolean): void {
    this.presses.set(key, {
      finger: finger,
      downY: y,
      mode: NineKeySwipePolicy.mode(mode),
      hasLetters: hasLetters,
      result: NineKeySwipe.NONE,
      held: false,
      active: true,
    });
  }

  /** 手指移动到纵坐标 y。只在判定刚刚锁定的这一下返回 `DIGIT` 或 `LETTERS`，让视图换键面或弹出选项一次；其余时候返回 `NONE`。 */
  move(key: string, finger: number, y: number): NineKeySwipe {
    const press: SwipePress | undefined = this.presses.get(key);
    if (
      press === undefined ||
      !press.active ||
      press.finger !== finger ||
      press.held ||
      press.result !== NineKeySwipe.NONE
    ) {
      return NineKeySwipe.NONE;
    }
    press.result = NineKeySwipePolicy.gesture(press.mode, press.downY, y, press.hasLetters);
    if (press.result === NineKeySwipe.LETTERS) {
      // 弹出的选项换掉了键面，这次触摸归弹窗：之后的抬手什么也不做，点击照样吞掉。
      press.active = false;
    }
    return press.result;
  }

  /** 手指抬起，返回这次触摸锁定的判定：`DIGIT` 由视图输入数字，`NONE` 照常是点按（弹出字母的那次触摸在 `move` 时就已结束，这里也是 `NONE`）。不是按着这个键的手指时返回 `NONE`。 */
  up(key: string, finger: number): NineKeySwipe {
    const press: SwipePress | undefined = this.presses.get(key);
    if (press === undefined || !press.active || press.finger !== finger) {
      return NineKeySwipe.NONE;
    }
    press.active = false;
    return press.result;
  }

  /** 系统取消了这次触摸，返回取消前的判定；什么也不输入，滑出去的这次触摸随后的点击照样吞掉。 */
  cancel(key: string): NineKeySwipe {
    const press: SwipePress | undefined = this.presses.get(key);
    if (press === undefined || !press.active) {
      return NineKeySwipe.NONE;
    }
    press.active = false;
    return press.result;
  }

  /** 长按到时。滑动已经接管这次触摸时返回 false，不弹出选项；否则从此不再判定滑动。 */
  hold(key: string): boolean {
    const press: SwipePress | undefined = this.presses.get(key);
    if (press === undefined) {
      return true;
    }
    if (press.result !== NineKeySwipe.NONE) {
      return false;
    }
    press.held = true;
    return true;
  }

  /** 这个键收到点击：这次触摸已经滑出去（数字或字母）就返回 true，点击不再输入。抬手先到还是点击先到都一样。 */
  swallows(key: string): boolean {
    const press: SwipePress | undefined = this.presses.get(key);
    return press !== undefined && press.result !== NineKeySwipe.NONE;
  }

  /** 触摸下的键被替换或键盘换了层：丢弃全部按压。 */
  clear(): void {
    this.presses.clear();
  }
}
