/** 拼音九键字母层的四向 flick：上、左、右依次输入键面前三个字母，下滑无动作。 */
export enum NineKeyFlick {
  NONE = "none",
  FIRST = "first",
  SECOND = "second",
  THIRD = "third",
}

export class NineKeyFlickPolicy {
  static readonly DEFAULT_THRESHOLD_VP: number = 24;
  static readonly MIN_THRESHOLD_VP: number = 14;
  static readonly MAX_THRESHOLD_VP: number = 48;
  static readonly THRESHOLD_STEP_VP: number = 2;

  static letter(
    downX: number,
    downY: number,
    currentX: number,
    currentY: number,
    letters: string,
    threshold: number,
  ): NineKeyFlick {
    if (
      letters.length === 0 ||
      !Number.isFinite(downX) ||
      !Number.isFinite(downY) ||
      !Number.isFinite(currentX) ||
      !Number.isFinite(currentY) ||
      !Number.isFinite(threshold) ||
      threshold < 0
    ) {
      return NineKeyFlick.NONE;
    }
    const dx: number = currentX - downX;
    const dy: number = currentY - downY;
    const horizontal: number = Math.abs(dx);
    const vertical: number = Math.abs(dy);
    if (Math.max(horizontal, vertical) <= threshold) return NineKeyFlick.NONE;
    const index: number = vertical >= horizontal ? (dy < 0 ? 0 : -1) : dx < 0 ? 1 : 2;
    if (index < 0 || index >= letters.length) return NineKeyFlick.NONE;
    return index === 0
      ? NineKeyFlick.FIRST
      : index === 1
        ? NineKeyFlick.SECOND
        : NineKeyFlick.THIRD;
  }

  static character(letters: string, flick: NineKeyFlick): string {
    const index: number =
      flick === NineKeyFlick.FIRST
        ? 0
        : flick === NineKeyFlick.SECOND
          ? 1
          : flick === NineKeyFlick.THIRD
            ? 2
            : -1;
    return index >= 0 && index < letters.length ? letters.charAt(index).toLowerCase() : "";
  }

  static threshold(value: unknown): number {
    const numeric: number = typeof value === "number" ? value : Number.NaN;
    if (
      !Number.isFinite(numeric) ||
      numeric < this.MIN_THRESHOLD_VP ||
      numeric > this.MAX_THRESHOLD_VP ||
      (numeric - this.MIN_THRESHOLD_VP) % this.THRESHOLD_STEP_VP !== 0
    ) {
      return this.DEFAULT_THRESHOLD_VP;
    }
    return numeric;
  }
}

interface FlickPress {
  finger: number;
  downX: number;
  downY: number;
  letters: string;
  threshold: number;
  result: NineKeyFlick;
  held: boolean;
  active: boolean;
  consumed: boolean;
}

export class NineKeyFlickGesture {
  private readonly presses: Map<string, FlickPress> = new Map<string, FlickPress>();

  down(
    key: string,
    finger: number,
    x: number,
    y: number,
    letters: string,
    threshold: number,
    enabled: boolean = true,
  ): void {
    this.presses.set(key, {
      finger,
      downX: x,
      downY: y,
      letters,
      threshold,
      result: NineKeyFlick.NONE,
      held: false,
      active: enabled,
      consumed: false,
    });
  }

  move(key: string, finger: number, x: number, y: number): NineKeyFlick {
    const press: FlickPress | undefined = this.presses.get(key);
    if (
      press === undefined ||
      !press.active ||
      press.finger !== finger ||
      press.held ||
      press.consumed
    )
      return NineKeyFlick.NONE;
    if (
      ![x, y, press.downX, press.downY, press.threshold].every(Number.isFinite) ||
      press.threshold < 0
    )
      return NineKeyFlick.NONE;
    if (Math.max(Math.abs(x - press.downX), Math.abs(y - press.downY)) <= press.threshold)
      return NineKeyFlick.NONE;
    press.consumed = true;
    press.result = NineKeyFlickPolicy.letter(
      press.downX,
      press.downY,
      x,
      y,
      press.letters,
      press.threshold,
    );
    return press.result;
  }

  up(key: string, finger: number): NineKeyFlick {
    const press: FlickPress | undefined = this.presses.get(key);
    if (press === undefined || !press.active || press.finger !== finger) return NineKeyFlick.NONE;
    press.active = false;
    return press.result;
  }

  cancel(key: string): void {
    const press: FlickPress | undefined = this.presses.get(key);
    if (press !== undefined) press.active = false;
  }

  hold(key: string): boolean {
    const press: FlickPress | undefined = this.presses.get(key);
    if (press === undefined) return true;
    if (press.consumed) return false;
    press.held = true;
    return true;
  }

  swallows(key: string): boolean {
    const press: FlickPress | undefined = this.presses.get(key);
    return press !== undefined && press.consumed;
  }

  letters(key: string): string {
    return this.presses.get(key)?.letters ?? "";
  }

  clear(): void {
    this.presses.clear();
  }
}
