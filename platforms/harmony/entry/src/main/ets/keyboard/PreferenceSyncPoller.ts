/**
 * 在键盘仍显示时，定期给跨进程偏好镜像一次检查机会。
 *
 * 设置应用和输入法扩展没有共同的事件循环，设置页保存后只能先写共享清单。轮询只在
 * 键盘显示期间运行，实际的文件读取和会话重建由调用方按空闲状态决定。
 */
export class PreferenceSyncPoller {
  private timer: number = -1;
  private active: boolean = false;

  constructor(
    private readonly canPoll: () => boolean,
    private readonly poll: () => void,
    private readonly delayMs: number = 500,
    private readonly schedule: (callback: () => void, delay: number) => number = (
      callback: () => void,
      delay: number,
    ): number => setTimeout(callback, delay),
    private readonly cancel: (timer: number) => void = (timer: number): void => clearTimeout(timer),
  ) {}

  start(): void {
    this.active = true;
    this.scheduleNext();
  }

  stop(): void {
    this.active = false;
    if (this.timer >= 0) {
      this.cancel(this.timer);
      this.timer = -1;
    }
  }

  private scheduleNext(): void {
    if (!this.active || this.timer >= 0) return;
    this.timer = this.schedule((): void => {
      this.timer = -1;
      if (!this.active) return;
      if (this.canPoll()) this.poll();
      this.scheduleNext();
    }, this.delayMs);
  }
}
