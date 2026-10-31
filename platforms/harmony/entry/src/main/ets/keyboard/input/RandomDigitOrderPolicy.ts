/**
 * 中文数字九宫格的临时随机键序（#10254），与 Android `RandomDigitOrderPolicy` 一一对应：键位 0–8 是网格的 1–9，键位 9 是底行的 0。
 *
 * 开关只属于键盘这一次运行，不存进偏好：工具栏的「随」点一下打开、再点一下关闭。打开后第一次在随机资格成立时取键面才打乱，离开数字页不取新随机数，回来沿用上次排列；每输入一个数字重新打乱，随机数恰好给出原排列时也强制换一次序。关掉后恢复 1234567890。放在视图旁边的纯 TypeScript 里，`tests/run.sh` 能用固定随机源检查它。
 */
export class RandomDigitOrderPolicy {
  /** 标准键序：1–9 在网格，0 在底行。 */
  static readonly STANDARD: readonly number[] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];

  private readonly nextInt: (bound: number) => number;
  private readonly digits: number[] = RandomDigitOrderPolicy.STANDARD.slice();
  private on: boolean = false;
  private prepared: boolean = false;

  /** @param nextInt 返回 [0, bound) 里的一个整数；测试传固定序列，键盘用 `Math.random`。 */
  constructor(nextInt?: (bound: number) => number) {
    this.nextInt = nextInt ?? ((bound: number): number => Math.floor(Math.random() * bound));
  }

  enabled(): boolean {
    return this.on;
  }

  setEnabled(value: boolean): void {
    if (this.on === value) {
      return;
    }
    this.on = value;
    this.prepared = false;
  }

  /** 键位 `slot` 此刻画哪个数字。没开或没有随机资格（不在中文数字页）时是标准键序，不取随机数。 */
  digit(slot: number, eligible: boolean): number {
    if (!Number.isInteger(slot) || slot < 0 || slot >= this.digits.length) {
      throw new Error(`Invalid digit slot: ${slot}`);
    }
    if (!this.on || !eligible) {
      return (slot + 1) % 10;
    }
    if (!this.prepared) {
      this.shuffle();
    }
    return this.digits[slot];
  }

  /** 十个键位按顺序的数字，同 `digit` 的规则。 */
  faces(eligible: boolean): number[] {
    const faces: number[] = [];
    for (let slot = 0; slot < this.digits.length; slot++) {
      faces.push(this.digit(slot, eligible));
    }
    return faces;
  }

  /** 一个数字上屏之后：开着且有资格时重新打乱。 */
  afterDigit(eligible: boolean): void {
    if (this.on && eligible) {
      this.shuffle();
    }
  }

  private shuffle(): void {
    const previous: number[] = this.digits.slice();
    for (let i = this.digits.length - 1; i > 0; i--) {
      const other: number = this.nextInt(i + 1);
      const temporary: number = this.digits[i];
      this.digits[i] = this.digits[other];
      this.digits[other] = temporary;
    }
    // 即使随机数恰好给出原排列，这次点击也要产生新的键序。
    if (previous.every((value: number, index: number): boolean => value === this.digits[index])) {
      const first: number = this.digits[0];
      this.digits[0] = this.digits[1];
      this.digits[1] = first;
    }
    this.prepared = true;
  }
}
