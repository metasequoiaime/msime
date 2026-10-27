/**
 * 判断编辑器回报的一次选区变化之后，还要不要再向编辑器要一次光标前的上文；不依赖 ArkUI，便于在 node 下测试。
 *
 * 每次 `getForwardSync` 都是一次同步跨进程调用，要等宿主应用的 UI 线程作答。直接英文里每个字母都是一次上屏：上屏后会话已经按上屏后的上文匹配过邮箱后缀，编辑器随后回报的 `textChange`、`selectionChange` 只是这次上屏的回声，再读一遍读到的还是同一段文字。这里记下最近一次读上文时输入法自己发出过几次改动，回声到齐、之后没有新的改动时，那次读到的就是编辑器现在的上文。
 *
 * 异步改动（`insertText`、`sendKeyFunction`）什么时候落进编辑器不知道，在它的回声到齐之前读到的上文不算数；回声被当成外部修改、读失败、这时不该读上文，也都不算数，下一次选区回报照常重读。
 */
export class CaretContextEcho {
  /** 输入法自己发出的改动次数。 */
  private edits: number = 0;
  /** 最近一次读到可信上文时的 `edits`；-1 表示没有可信的那一次。 */
  private readAt: number = -1;
  /** 发出过异步改动，回声还没有到齐。 */
  private asyncPending: boolean = false;
  /** 刚到的 `textChange` 是输入法自己改动的回声；只给紧接着的那次 `selectionChange` 用。 */
  private ownEcho: boolean = false;

  /** 即将改动编辑器（与 `EditEchoLedger.reserve` 同时调用）。`asynchronous` 表示调用返回时改动不一定已经落进编辑器。 */
  edited(asynchronous: boolean): void {
    this.edits++;
    if (asynchronous) {
      this.asyncPending = true;
    }
  }

  /** 刚读过光标前的上文，并按它更新了结果。 */
  read(): void {
    this.readAt = this.asyncPending ? -1 : this.edits;
  }

  /** 结果不是按刚读到的上文算的：没读、读失败，或者结果被直接收起。 */
  unread(): void {
    this.readAt = -1;
  }

  /** 编辑器回报了一次 `textChange`。`own` 是 `EditEchoLedger.acknowledge` 的结果，`settled` 表示确认之后不再有等回声的改动。 */
  textChanged(own: boolean, settled: boolean): void {
    this.ownEcho = own;
    if (!own) {
      this.readAt = -1;
    } else if (settled) {
      this.asyncPending = false;
    }
  }

  /**
   * 编辑器回报了一次 `selectionChange`。返回 true 表示它是输入法自己改动的回声、回声已经到齐，并且最近一次读上文之后输入法没有再改动编辑器，那次读到的上文就是现在的上文，不必再读。
   */
  selectionChanged(settled: boolean): boolean {
    const own: boolean = this.ownEcho;
    this.ownEcho = false;
    return own && settled && this.readAt >= 0 && this.readAt === this.edits;
  }

  /** 换了编辑器或会话结束：之前的改动和读到的上文都不再属于当前编辑器。 */
  reset(): void {
    this.readAt = -1;
    this.asyncPending = false;
    this.ownEcho = false;
  }
}
