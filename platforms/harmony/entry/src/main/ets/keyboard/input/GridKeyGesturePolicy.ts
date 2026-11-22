import { EighteenKeySwipePolicy } from "./EighteenKeySwipePolicy";
import { SwipeHintPolicy, SwipeSymbolsDirection } from "./SwipeHintPolicy";

export enum GridKeyGestureMode {
  PENDING = "pending",
  SYMBOL = "symbol",
  LETTER = "letter",
  HELD = "held",
  CANCELLED = "cancelled",
}

/** 抬手或点击这一次要输入什么。 */
export enum GridKeyCommit {
  /** 什么也不输入：另一方已经输入过、这次按压被取消或已由长按输入。 */
  NONE = "none",
  /** 整组，即普通点按。 */
  GROUP = "group",
  /** 横滑选中的精确字母。 */
  LETTER = "letter",
  /** 纵滑输入的角标。 */
  SYMBOL = "symbol",
}

/**
 * 14/18 键角标长按、纵滑和 14/18 键全拼及双拼横滑的统一一次按压状态机。
 *
 * 按手指 id 跟踪：只认按下这个键的那根手指，别的键上的手指（双拇指交替打字）不影响这次按压；只有另一根手指落在同一个键上时整次按压作废。ArkUI 的 `touches` 装的是全屏所有手指（抬手时也含正在抬起的那根），不能拿它的长度判断多指。
 *
 * ArkUI 送这个键的 `onClick` 与它的抬手 `onTouch` 谁先谁后没有契约（见 `EighteenKeySwipePolicy`），所以点击带着自己的坐标来问一次：先到的一方按自己的位置作出判定并输入，后到的一方只照这个判定行事，同一次按压不会输入两次。空闲时没滑开的点按由点击输入；组字中纵滑仍按点按，ArkUI 可能因为位移不送点击，所以抬手先到时由抬手输入整组。
 */
export class GridKeyGesturePolicy {
  private readonly exactLetters: string | null;
  private readonly hasSymbol: boolean;
  private pointer: number = -1;
  private downX: number = 0;
  private downY: number = 0;
  private direction: SwipeSymbolsDirection | null = null;
  private tapsOnly: boolean = false;
  private currentMode: GridKeyGestureMode = GridKeyGestureMode.PENDING;
  private selected: string = "";
  /** 抬手或点击已经作出判定。 */
  private decided: boolean = false;
  /** 抬手先到、判定为空闲点按：整组留给随后的点击输入。 */
  private clickOwesGroup: boolean = false;
  /** 这次按压的点击已经回答过；再来的点击没有自己的按压（例如读屏激活），按普通点按输入。 */
  private clickAnswered: boolean = false;

  constructor(exactLetters: string | null, hasSymbol: boolean) {
    this.exactLetters = exactLetters;
    this.hasSymbol = hasSymbol;
  }

  start(
    pointer: number,
    x: number,
    y: number,
    direction: SwipeSymbolsDirection | null,
    composing: boolean,
  ): void {
    this.pointer = pointer;
    this.downX = x;
    this.downY = y;
    this.direction = direction;
    this.tapsOnly = composing;
    this.currentMode = GridKeyGestureMode.PENDING;
    this.selected = "";
    this.decided = false;
    this.clickOwesGroup = false;
    this.clickAnswered = false;
  }

  /** `pointer` 落在这个键上时，是否是同一键上的第二根手指：已有另一根手指按着、按压还没判定。 */
  secondFinger(pointer: number): boolean {
    return this.pointer >= 0 && !this.decided && pointer !== this.pointer;
  }

  composing(): void {
    if (this.currentMode === GridKeyGestureMode.PENDING) this.tapsOnly = true;
  }

  /** 手指移动到 (x, y)。不是按着这个键的手指，或这次按压已经判定时，状态不变。 */
  move(pointer: number, x: number, y: number): GridKeyGestureMode {
    if (pointer < 0 || pointer !== this.pointer || this.decided) return this.currentMode;
    this.track(x, y);
    return this.currentMode;
  }

  hold(): boolean {
    if (
      this.pointer < 0 ||
      this.decided ||
      !this.hasSymbol ||
      this.currentMode !== GridKeyGestureMode.PENDING
    )
      return false;
    this.currentMode = GridKeyGestureMode.HELD;
    return true;
  }

  /** 按着这个键的手指在 (x, y) 抬起；别的手指抬起，或点击已经先判定时返回 NONE。 */
  up(pointer: number, x: number, y: number): GridKeyCommit {
    if (pointer < 0 || pointer !== this.pointer) return GridKeyCommit.NONE;
    this.pointer = -1;
    if (this.decided) return GridKeyCommit.NONE;
    this.decided = true;
    this.track(x, y);
    if (this.currentMode === GridKeyGestureMode.PENDING && !this.tapsOnly) {
      this.clickOwesGroup = true;
      return GridKeyCommit.NONE;
    }
    return this.commit();
  }

  /** 这个键在 (x, y) 收到点击。抬手还没到时按点击位置判定，之后的抬手不再输入。 */
  click(x: number, y: number): GridKeyCommit {
    if (this.clickAnswered) return GridKeyCommit.GROUP;
    this.clickAnswered = true;
    if (this.decided) {
      const owed: boolean = this.clickOwesGroup;
      this.clickOwesGroup = false;
      return owed ? GridKeyCommit.GROUP : GridKeyCommit.NONE;
    }
    this.decided = true;
    if (this.pointer >= 0) this.track(x, y);
    return this.commit();
  }

  cancel(): void {
    this.currentMode = GridKeyGestureMode.CANCELLED;
    this.selected = "";
    this.clickOwesGroup = false;
  }

  mode(): GridKeyGestureMode {
    return this.currentMode;
  }
  selectedLetter(): string {
    return this.selected;
  }
  consumed(): boolean {
    return this.currentMode !== GridKeyGestureMode.PENDING;
  }

  private commit(): GridKeyCommit {
    switch (this.currentMode) {
      case GridKeyGestureMode.PENDING:
        return GridKeyCommit.GROUP;
      case GridKeyGestureMode.LETTER:
        return GridKeyCommit.LETTER;
      case GridKeyGestureMode.SYMBOL:
        return GridKeyCommit.SYMBOL;
      default:
        return GridKeyCommit.NONE;
    }
  }

  private track(x: number, y: number): void {
    if (
      this.currentMode === GridKeyGestureMode.CANCELLED ||
      this.currentMode === GridKeyGestureMode.HELD ||
      this.currentMode === GridKeyGestureMode.SYMBOL
    ) {
      return;
    }
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      this.cancel();
      return;
    }
    const dx: number = x - this.downX;
    const dy: number = y - this.downY;
    const letter: string = EighteenKeySwipePolicy.letter(this.exactLetters ?? "", dx, dy);
    if (this.currentMode === GridKeyGestureMode.LETTER) {
      this.selected = letter;
    } else if (letter.length > 0) {
      this.currentMode = GridKeyGestureMode.LETTER;
      this.selected = letter;
    } else if (!this.tapsOnly && this.hasSymbol && this.vertical(this.direction, dx, dy)) {
      this.currentMode = GridKeyGestureMode.SYMBOL;
    }
  }

  private vertical(direction: SwipeSymbolsDirection | null, dx: number, dy: number): boolean {
    return (
      direction !== null && SwipeHintPolicy.swiped(direction, 0, dy) && Math.abs(dy) > Math.abs(dx)
    );
  }
}
