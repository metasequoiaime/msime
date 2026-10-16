/** Small, host-neutral pieces of the Windows candidate presentation contract. */
export class CandidatePresentationPolicy {
  static badge(source: number): string {
    if (source === 2) return " ☁️";
    if (source === 3) return " 🤖";
    return "";
  }

  /** Engine 候选来源 12：AI 占位行（「…」等待中，「—」没有结果），画成次要色，不加标记（#6888）。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。 */
  static aiPlaceholder(source: number): boolean {
    return source === 12;
  }
}
