import { EighteenKeySwipePolicy } from "./EighteenKeySwipePolicy";
import { SwipeHintPolicy, SwipeSymbolsDirection } from "./SwipeHintPolicy";

export enum GridKeyGestureMode {
  PENDING = "pending",
  SYMBOL = "symbol",
  LETTER = "letter",
  OPTIONS = "options",
  CANCELLED = "cancelled",
}

/** 14/18 键纵滑、反向选项和 18 键双拼横滑的统一一次按压状态机。 */
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
  }

  composing(): void {
    if (this.currentMode === GridKeyGestureMode.PENDING) this.tapsOnly = true;
  }

  move(pointer: number, pointers: number, x: number, y: number): GridKeyGestureMode {
    if (
      this.currentMode === GridKeyGestureMode.CANCELLED ||
      this.currentMode === GridKeyGestureMode.OPTIONS
    ) {
      return this.currentMode;
    }
    if (this.invalid(pointer, pointers, x, y)) {
      this.cancel();
      return this.currentMode;
    }
    const dx: number = x - this.downX;
    const dy: number = y - this.downY;
    if (this.currentMode === GridKeyGestureMode.LETTER) {
      this.selected = EighteenKeySwipePolicy.letter(this.exactLetters ?? "", dx, dy);
      return this.currentMode;
    }
    const letter: string = EighteenKeySwipePolicy.letter(this.exactLetters ?? "", dx, dy);
    if (letter.length > 0) {
      this.currentMode = GridKeyGestureMode.LETTER;
      this.selected = letter;
    } else if (!this.tapsOnly && this.hasSymbol && this.vertical(this.direction, dx, dy)) {
      this.currentMode = GridKeyGestureMode.SYMBOL;
    } else if (!this.tapsOnly && this.vertical(this.opposite(this.direction), dx, dy)) {
      this.currentMode = GridKeyGestureMode.OPTIONS;
    }
    return this.currentMode;
  }

  hold(): boolean {
    if (this.currentMode !== GridKeyGestureMode.PENDING || this.tapsOnly) return false;
    this.currentMode = GridKeyGestureMode.OPTIONS;
    return true;
  }

  up(pointer: number, pointers: number, x: number, y: number): GridKeyGestureMode {
    if (this.currentMode === GridKeyGestureMode.PENDING) this.move(pointer, pointers, x, y);
    return this.currentMode;
  }

  cancel(): void {
    this.currentMode = GridKeyGestureMode.CANCELLED;
    this.selected = "";
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
  forcesTap(): boolean {
    return this.currentMode === GridKeyGestureMode.PENDING && this.tapsOnly;
  }

  private vertical(direction: SwipeSymbolsDirection | null, dx: number, dy: number): boolean {
    return (
      direction !== null && SwipeHintPolicy.swiped(direction, 0, dy) && Math.abs(dy) > Math.abs(dx)
    );
  }

  private opposite(direction: SwipeSymbolsDirection | null): SwipeSymbolsDirection | null {
    if (direction === null) return null;
    return direction === SwipeSymbolsDirection.UP
      ? SwipeSymbolsDirection.DOWN
      : SwipeSymbolsDirection.UP;
  }

  private invalid(pointer: number, pointers: number, x: number, y: number): boolean {
    return pointers !== 1 || pointer !== this.pointer || !Number.isFinite(x) || !Number.isFinite(y);
  }
}
