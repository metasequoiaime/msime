/**
 * 双拼 18 键两个字母的键上的左右横滑：左滑取左字母、右滑取右字母，以大写精确标记交给 `pressGridKey`，引擎只在网格 5 把大写当作这一组里的那个小写字母。规则与 Android 的 `EighteenKeySwipePolicy`、iOS 的 `EighteenKeySwipeGestureRecognizer` 相同：横向位移超过 24 vp 且大于纵向的 1.5 倍才算横滑，与 Android 的 24 dp、iOS 的 24 pt 是同一个物理尺寸，正如 `SwipeHintPolicy` 把 Android 的 14 dp 写成 14 vp。
 *
 * 一旦越过阈值，这次按压就不再是点按：回到起点、同一键上落下第二根手指或系统取消都什么也不输入，反向拖过另一侧可以重选。长按先生效时不再横滑；横滑先接管时长按也不再弹出字母。抬手那一点也判一次，快速一甩只在抬手时才过阈值的，同样取那个字母。
 *
 * ArkUI 送这个键的 `onClick` 与它的抬手 `onTouch` 谁先谁后没有契约，所以点击也带着自己的坐标来问一次（`click`）：先到的一方按抬手位置作出判定，后到的一方只照这个判定行事，同一次按压不会既输入字母又输入整组。这里不涉及 ArkUI，坐标是视图读到的窗口坐标，单位 vp。
 */
export class EighteenKeySwipePolicy {
  /** 横滑阈值，单位 vp；只有严格大于它的横向位移才算。 */
  static readonly THRESHOLD_VP: number = 24;
  /** 横向位移至少是纵向的多少倍；同样只有严格大于才算。 */
  static readonly HORIZONTAL_RATIO: number = 1.5;

  /** 这一位移选中的字母，大写；阈值以内、纵向为主、坐标无效或键上不是两个小写字母时为空串。 */
  static letter(letters: string, dx: number, dy: number): string {
    if (
      !EighteenKeySwipePolicy.swipeable(letters) ||
      !Number.isFinite(dx) ||
      !Number.isFinite(dy)
    ) {
      return "";
    }
    if (
      Math.abs(dx) <= EighteenKeySwipePolicy.THRESHOLD_VP ||
      Math.abs(dx) <= EighteenKeySwipePolicy.HORIZONTAL_RATIO * Math.abs(dy)
    ) {
      return "";
    }
    return letters.charAt(dx < 0 ? 0 : 1).toUpperCase();
  }

  /** 只有两个小写字母的键可以横滑；单字母键（含微软双拼的 `L;`）只点按。 */
  static swipeable(letters: string): boolean {
    return (
      letters.length === 2 &&
      EighteenKeySwipePolicy.lower(letters.charCodeAt(0)) &&
      EighteenKeySwipePolicy.lower(letters.charCodeAt(1))
    );
  }

  private static lower(code: number): boolean {
    return code >= 97 && code <= 122;
  }
}

/** 点击到达时这个键该输入什么。 */
export interface EighteenKeyTap {
  /** 送整组，即普通点按。 */
  group: boolean;
  /** 送这个大写精确标记；空串表示不送。 */
  letter: string;
}

interface SwipePress {
  finger: number;
  downX: number;
  downY: number;
  /** 横滑接管了这次按压，或它被取消：之后的点击不再输入整组。 */
  consumed: boolean;
  /** 长按先生效，这次按压不再横滑。 */
  held: boolean;
  /** 抬手或点击已经作出判定。 */
  decided: boolean;
  /** 当前位移选中的字母，大写；空串表示没有。 */
  selected: string;
}

/** 双拼 18 键各键上正在进行的按压，按键上的字母（`FourteenKey.letters`）区分，两个拇指按在两个键上互不影响。 */
export class EighteenKeySwipeGesture {
  private readonly presses: Map<string, SwipePress> = new Map<string, SwipePress>();

  /** 手指按在 `letters` 键上。同一键上已有另一根手指按着时，这是第二根手指：整次按压作废。 */
  down(letters: string, finger: number, x: number, y: number): void {
    const current: SwipePress | undefined = this.presses.get(letters);
    if (current !== undefined && !current.decided && current.finger !== finger) {
      EighteenKeySwipeGesture.abandon(current);
      return;
    }
    this.presses.set(letters, {
      finger: finger,
      downX: x,
      downY: y,
      consumed: false,
      held: false,
      decided: false,
      selected: "",
    });
  }

  /** 手指移动到 (x, y)，返回此刻选中的字母供键面显示，空串表示显示原来的键面；不是按着这个键的手指时返回 null。 */
  move(letters: string, finger: number, x: number, y: number): string | null {
    const press: SwipePress | undefined = this.presses.get(letters);
    if (press === undefined || press.finger !== finger) {
      return null;
    }
    if (press.decided || press.held) {
      return "";
    }
    EighteenKeySwipeGesture.track(letters, press, x, y);
    return press.selected;
  }

  /** 手指在 (x, y) 抬起，返回要输入的大写字母；空串表示这次抬手不输入，点按交给 `click`。 */
  up(letters: string, finger: number, x: number, y: number): string {
    const press: SwipePress | undefined = this.presses.get(letters);
    if (press === undefined || press.finger !== finger || press.decided) {
      return "";
    }
    press.decided = true;
    if (!press.held) {
      EighteenKeySwipeGesture.track(letters, press, x, y);
    }
    return press.selected;
  }

  /** 这个键在 (x, y) 收到点击。抬手还没到时按点击位置判定，结果与抬手判定相同，之后的抬手不再输入。 */
  click(letters: string, x: number, y: number): EighteenKeyTap {
    const press: SwipePress | undefined = this.presses.get(letters);
    if (press === undefined) {
      return { group: true, letter: "" };
    }
    if (press.consumed || press.held) {
      return { group: false, letter: "" };
    }
    if (!press.decided) {
      press.decided = true;
      EighteenKeySwipeGesture.track(letters, press, x, y);
      if (press.consumed) {
        return { group: false, letter: press.selected };
      }
    }
    return { group: true, letter: "" };
  }

  /** 长按到时。横滑已经接管这次按压时返回 false，不弹出字母；否则从此不再横滑。 */
  hold(letters: string): boolean {
    const press: SwipePress | undefined = this.presses.get(letters);
    if (press === undefined) {
      return true;
    }
    if (press.consumed) {
      return false;
    }
    press.held = true;
    press.selected = "";
    return true;
  }

  /** 系统取消了这次触摸：什么也不输入，随后的点击也不算。 */
  cancel(letters: string): void {
    const press: SwipePress | undefined = this.presses.get(letters);
    if (press !== undefined) {
      EighteenKeySwipeGesture.abandon(press);
    }
  }

  /** 触摸下的键被替换或键盘换了方案：丢弃全部按压。 */
  clear(): void {
    this.presses.clear();
  }

  private static track(letters: string, press: SwipePress, x: number, y: number): void {
    press.selected = EighteenKeySwipePolicy.letter(letters, x - press.downX, y - press.downY);
    if (press.selected.length > 0) {
      press.consumed = true;
    }
  }

  private static abandon(press: SwipePress): void {
    press.consumed = true;
    press.decided = true;
    press.selected = "";
  }
}
