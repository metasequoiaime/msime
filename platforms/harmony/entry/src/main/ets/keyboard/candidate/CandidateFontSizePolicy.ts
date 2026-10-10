/** 候选栏字号的共享范围，供键盘宿主在读取旧文档时再次校正。 */
export class CandidateFontSizePolicy {
  static readonly MIN: number = 12;
  static readonly MAX: number = 32;

  static bounded(value: number): number {
    return Math.max(CandidateFontSizePolicy.MIN, Math.min(CandidateFontSizePolicy.MAX, value));
  }
}
