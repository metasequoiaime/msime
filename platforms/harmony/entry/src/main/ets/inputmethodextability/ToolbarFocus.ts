/** 编辑器绑定时串行请求显示系统状态面板，过期的焦点不能再动新焦点的面板。 */
export interface FocusToolbarPanel {
  show(): Promise<void>;
}

export class ToolbarFocus {
  private version: number = 0;
  private tail: Promise<void> = Promise.resolve();
  // `openToolbar` 仍在调整尺寸和位置的面板；准备好后由 `openToolbar` 自己显示。
  private settingUp: FocusToolbarPanel | undefined = undefined;

  private readonly report: (error: Object) => void;

  constructor(report: (error: Object) => void) { this.report = report; }

  invalidate(): void { this.version++; }

  /** 面板还在调整尺寸和位置时，焦点不能抢先显示它，否则首帧会先出现在系统默认位置再跳到右下角。 */
  beginSetup(panel: FocusToolbarPanel): void { this.settingUp = panel; }

  endSetup(panel: FocusToolbarPanel): void {
    if (this.settingUp === panel) this.settingUp = undefined;
  }

  /** 每次绑定都请求显示，不看缓存的可见状态：系统切换窗口时可能自行收起状态面板。排在前面的请求还没完成时，轮到自己前已被新焦点、解绑或换面板取代的请求直接丢弃。 */
  async focus(panel: FocusToolbarPanel, isCurrent: () => boolean): Promise<void> {
    const version: number = ++this.version;
    const operation: Promise<void> = this.tail.then(async (): Promise<void> => {
      if (this.stale(version, panel, isCurrent)) return;
      await panel.show();
    });
    this.tail = operation.catch((error: Object): void => { this.report(error); });
    await this.tail;
  }

  private stale(version: number, panel: FocusToolbarPanel, isCurrent: () => boolean): boolean {
    return version !== this.version || panel === this.settingUp || !isCurrent();
  }
}
