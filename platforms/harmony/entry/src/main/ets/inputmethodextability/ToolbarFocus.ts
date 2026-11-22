/** 焦点变化时串行更新系统状态面板，过期的窗口状态不能控制新焦点。 */
export interface FocusToolbarPanel {
  show(): Promise<void>;
  hide(): Promise<void>;
}

export class ToolbarFocus {
  private version: number = 0;
  private tail: Promise<void> = Promise.resolve();
  // A panel openToolbar is still sizing and placing; openToolbar shows it itself once it is ready.
  private settingUp: FocusToolbarPanel | undefined = undefined;

  private readonly report: (error: Object) => void;

  constructor(report: (error: Object) => void) { this.report = report; }

  invalidate(): void { this.version++; }

  /** 面板还在调整尺寸和位置时，焦点不能抢先显示它，否则首帧会先出现在系统默认位置再跳到右下角。 */
  beginSetup(panel: FocusToolbarPanel): void { this.settingUp = panel; }

  endSetup(panel: FocusToolbarPanel): void {
    if (this.settingUp === panel) this.settingUp = undefined;
  }

  async focus(panel: FocusToolbarPanel, isCurrent: () => boolean,
              fullscreen: () => Promise<boolean>): Promise<void> {
    const version: number = ++this.version;
    let hide: boolean = false;
    try {
      hide = await fullscreen();
    } catch (error) {
      // 拿不到窗口状态时仍尝试恢复工具栏，不把未知状态当作全屏。
      this.report(error as Object);
    }
    if (this.stale(version, panel, isCurrent)) return;
    const operation: Promise<void> = this.tail.then(async (): Promise<void> => {
      if (this.stale(version, panel, isCurrent)) return;
      if (hide) await panel.hide(); else await panel.show();
    });
    this.tail = operation.catch((error: Object): void => { this.report(error); });
    await this.tail;
  }

  private stale(version: number, panel: FocusToolbarPanel, isCurrent: () => boolean): boolean {
    return version !== this.version || panel === this.settingUp || !isCurrent();
  }
}
