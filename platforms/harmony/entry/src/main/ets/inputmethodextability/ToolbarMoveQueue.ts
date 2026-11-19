/** 串行移动面板；已有请求未完成时只保留最新位置。 */
export interface ToolbarMovable {
  moveTo(x: number, y: number): Promise<void>;
}

export class ToolbarMoveQueue {
  private panel: ToolbarMovable | undefined = undefined;
  private pending: [number, number] | undefined = undefined;
  private inFlight: [number, number] | undefined = undefined;
  private confirmed: [number, number] = [0, 0];
  private generation: number = 0;
  private busy: boolean = false;
  private readonly onError: (error: Object) => void;

  constructor(onError: (error: Object) => void) { this.onError = onError; }

  attach(panel: ToolbarMovable, initial: [number, number]): void {
    this.generation++;
    this.panel = panel;
    this.confirmed = initial;
    this.inFlight = undefined;
    this.pending = undefined;
    this.busy = false;
  }

  detach(): void {
    this.generation++;
    this.panel = undefined;
    this.inFlight = undefined;
    this.pending = undefined;
    this.busy = false;
  }

  /** 用实际可见位置作为新手势起点，替换尚未处理的旧移动。 */
  rebase(position: [number, number]): void {
    if (this.panel === undefined) return;
    this.confirmed = position;
    this.pending = undefined;
    if (this.busy) this.pending = position;
  }

  /** 无法读取窗口位置时，以最近一次请求的位置作为起点。 */
  anchor(): [number, number] { return this.pending ?? this.inFlight ?? this.confirmed; }

  move(position: [number, number]): void {
    if (this.panel === undefined) return;
    this.pending = position;
    if (!this.busy) void this.drain(this.generation);
  }

  private async drain(generation: number): Promise<void> {
    if (this.busy || this.panel === undefined || generation !== this.generation) return;
    this.busy = true;
    while (generation === this.generation && this.panel !== undefined && this.pending !== undefined) {
      const panel: ToolbarMovable = this.panel;
      const position: [number, number] = this.pending;
      this.pending = undefined;
      this.inFlight = position;
      try {
        await panel.moveTo(position[0], position[1]);
        if (generation === this.generation) this.confirmed = position;
      } catch (error) {
        if (generation === this.generation) this.onError(error as Object);
      }
      if (generation === this.generation) this.inFlight = undefined;
    }
    if (generation === this.generation) this.busy = false;
  }
}
