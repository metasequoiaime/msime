import { NativeReplyPolicy } from "./NativeReplyPolicy";

interface DiscardReply {
  status: string;
}

interface DiscardEnvelope {
  ok: boolean;
  value?: DiscardReply;
}

/** 取消的后台评分仍须轮询到结束，才能归还模型；这里不接受或发布候选。 */
export class PauseRerankDiscard {
  static readonly POLL_MS: number = 16;
  private handle: number = 0;
  private token: number = 0;
  private timer: number = -1;

  private pollNative: (handle: number, token: number) => string;
  private currentHandle: () => number;
  private schedule: (callback: () => void, delay: number) => number;
  private cancelTimer: (timer: number) => void;

  constructor(
    pollNative: (handle: number, token: number) => string,
    currentHandle: () => number,
    schedule: (callback: () => void, delay: number) => number,
    cancelTimer: (timer: number) => void,
  ) {
    this.pollNative = pollNative;
    this.currentHandle = currentHandle;
    this.schedule = schedule;
    this.cancelTimer = cancelTimer;
  }

  waiting(handle: number): boolean {
    if (this.handle !== handle) this.clear();
    return this.token !== 0;
  }

  discard(handle: number, token: number): void {
    if (handle === 0 || token === 0 || (this.handle === handle && this.token === token)) return;
    this.clear();
    this.handle = handle;
    this.token = token;
    this.poll(handle, token);
  }

  private poll(handle: number, token: number): void {
    // 旧定时器即使已进入队列，也不得清掉后来任务的定时器或访问重建后的句柄。
    if (this.handle !== handle || this.token !== token) return;
    if (this.currentHandle() !== handle) {
      this.clear();
      return;
    }
    this.timer = -1;
    let value: DiscardReply | null = null;
    try {
      value = NativeReplyPolicy.successfulValue<DiscardReply>(
        JSON.parse(this.pollNative(handle, token)) as DiscardEnvelope,
      );
    } catch (_) {
      // 读不懂回复不代表已归还模型，继续丢弃到明确结束或句柄销毁。
    }
    if (value === null || value.status !== "done") {
      this.timer = this.schedule((): void => {
        this.poll(handle, token);
      }, PauseRerankDiscard.POLL_MS);
    } else {
      this.clear();
    }
  }

  private clear(): void {
    if (this.timer >= 0) this.cancelTimer(this.timer);
    this.timer = -1;
    this.handle = 0;
    this.token = 0;
  }
}
