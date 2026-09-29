/**
 * One-shot Shift, automatic Shift and double-tap Caps Lock, ported from
 * platforms/android/java/app/msime/client/EnglishLetterCaseState.java.
 *
 * The distinction that matters: a Shift the user pressed is consumed by the next letter, a Shift the
 * capitalization policy applied is not sticky either, and Caps Lock survives both.
 */
export enum LetterCaseMode {
  LOWERCASE = "lowercase",
  SHIFTED = "shifted",
  CAPS_LOCK = "caps_lock",
}

export class EnglishLetterCaseState {
  static readonly CAPS_LOCK_INTERVAL_MILLIS: number = 350;

  private currentMode: LetterCaseMode = LetterCaseMode.LOWERCASE;
  private automaticShift: boolean = false;
  private lastShiftTapMillis: number = -1;

  mode(): LetterCaseMode {
    return this.currentMode;
  }

  usesUppercase(): boolean {
    return this.currentMode !== LetterCaseMode.LOWERCASE;
  }

  isAutomatic(): boolean {
    return this.automaticShift;
  }

  reset(): void {
    this.currentMode = LetterCaseMode.LOWERCASE;
    this.automaticShift = false;
    this.lastShiftTapMillis = -1;
  }

  /** A second tap inside the interval locks; any other tap flips between lower and shifted. */
  toggle(uptimeMillis: number): void {
    if (uptimeMillis < 0) {
      throw new Error("Uptime must not be negative");
    }
    this.automaticShift = false;
    if (
      this.currentMode === LetterCaseMode.SHIFTED &&
      this.lastShiftTapMillis >= 0 &&
      uptimeMillis >= this.lastShiftTapMillis &&
      uptimeMillis - this.lastShiftTapMillis <= EnglishLetterCaseState.CAPS_LOCK_INTERVAL_MILLIS
    ) {
      this.currentMode = LetterCaseMode.CAPS_LOCK;
    } else {
      this.currentMode =
        this.currentMode === LetterCaseMode.LOWERCASE
          ? LetterCaseMode.SHIFTED
          : LetterCaseMode.LOWERCASE;
    }
    this.lastShiftTapMillis = uptimeMillis;
  }

  /** Returns whether the mode changed, so the caller knows to redraw. Caps Lock is never overridden. */
  applyAutomatic(shouldShift: boolean): boolean {
    if (this.currentMode === LetterCaseMode.CAPS_LOCK) {
      return false;
    }
    const previous: LetterCaseMode = this.currentMode;
    this.currentMode = shouldShift ? LetterCaseMode.SHIFTED : LetterCaseMode.LOWERCASE;
    this.automaticShift = shouldShift;
    this.lastShiftTapMillis = -1;
    return previous !== this.currentMode;
  }

  /** A letter spends a one-shot Shift. Caps Lock and lowercase are unaffected. */
  consumeLetter(): boolean {
    if (this.currentMode !== LetterCaseMode.SHIFTED) {
      return false;
    }
    this.currentMode = LetterCaseMode.LOWERCASE;
    this.automaticShift = false;
    this.lastShiftTapMillis = -1;
    return true;
  }

  keyText(): string {
    return this.currentMode === LetterCaseMode.CAPS_LOCK ? "⇪" : "⇧";
  }

  accessibilityLabel(englishMode: boolean): string {
    switch (this.currentMode) {
      case LetterCaseMode.LOWERCASE:
        return englishMode ? "大写" : "切换到英文大写";
      case LetterCaseMode.SHIFTED:
        return "大写";
      case LetterCaseMode.CAPS_LOCK:
        return "大写锁定";
      default:
        return "大写";
    }
  }

  accessibilityValue(): string {
    switch (this.currentMode) {
      case LetterCaseMode.LOWERCASE:
        return "关闭";
      case LetterCaseMode.SHIFTED:
        return this.automaticShift ? "自动开启" : "下一字母";
      case LetterCaseMode.CAPS_LOCK:
        return "开启";
      default:
        return "关闭";
    }
  }
}
