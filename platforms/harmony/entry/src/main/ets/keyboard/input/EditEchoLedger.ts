/**
 * 分辨编辑器发来的 `textChange` 是输入法自己那次写入的迟到回声，还是外部（用户点了别处、应用自己改了文字）的变化；不依赖 ArkUI，便于在 node 下测试。
 *
 * 回声的来历按 OpenHarmony 的实现核对过：编辑器每次文字或选区变化都调 `InputMethodController.OnSelectionChange(text, start, end)`，它和上一次报告的文字与选区完全相同时直接丢弃，否则发给输入法，输入法侧先回调 `textChange`（参数是编辑器的整段文字）再回调 `selectionChange`。所以：插入、删除、改预上屏文字、`sendKeyFunction` 让编辑器换行、`moveCursorSync` 移动光标，各自产生一次回声；什么都没改的调用（光标已在开头时删除、光标已在边上时移动、没有预上屏文字时结束预上屏、应用不改文字的 GO/SEARCH 动作）不产生回声。一次调用会不会改动编辑器，输入法在调用时往往判断不了。
 *
 * 因此每次可能改动编辑器的调用都先记一笔，回声按发出的顺序到达，到一条就消掉最早的一笔。没等来回声的那一笔不能一直留着，否则之后一次真正的外部修改会被它吞掉，组字因此不被结束；每一笔只在 `ECHO_TIMEOUT_MS` 内有效，过期即作废。时限取得宽松：回声晚于它到达时会被当成外部修改去结束组字，这正是这套记账要防的事；而一笔多留的代价只是这段时间里的一次外部修改不结束组字。
 */
export class EditEchoLedger {
  /** 一笔待确认的写入等它的回声最多等这么久（毫秒）。 */
  static readonly ECHO_TIMEOUT_MS: number = 1000;
  /** 最多同时记这么多笔，再多就丢掉最早的，免得一个从不回声的编辑器让记录无限增长。 */
  static readonly MAX_PENDING: number = 16;

  private readonly reservedAt: number[] = [];

  /** 即将改动编辑器：记一笔，等它的回声。 */
  reserve(now: number): void {
    if (this.reservedAt.length >= EditEchoLedger.MAX_PENDING) {
      this.reservedAt.shift();
    }
    this.reservedAt.push(now);
  }

  /** 刚记的那次改动没有发生（调用抛错或编辑器拒绝），撤回最近的一笔。 */
  release(): void {
    this.reservedAt.pop();
  }

  /** 换了编辑器或会话结束：之前的回声都不再属于当前编辑器。 */
  clear(): void {
    this.reservedAt.length = 0;
  }

  /** 处理一次 `textChange`：是输入法自己某次写入的回声就消掉一笔并返回 true，是外部修改返回 false。 */
  acknowledge(now: number): boolean {
    this.expire(now);
    if (this.reservedAt.length === 0) {
      return false;
    }
    this.reservedAt.shift();
    return true;
  }

  /** 仍在等回声的笔数，过期的不算。 */
  pending(now: number): number {
    this.expire(now);
    return this.reservedAt.length;
  }

  /** 丢掉过期的记录；时钟往回调过的记录也一并丢掉，免得它因此永不过期。 */
  private expire(now: number): void {
    const live: number[] = this.reservedAt.filter(
      (reservedAt: number): boolean =>
        now - reservedAt >= 0 && now - reservedAt <= EditEchoLedger.ECHO_TIMEOUT_MS,
    );
    this.reservedAt.length = 0;
    this.reservedAt.push(...live);
  }
}
