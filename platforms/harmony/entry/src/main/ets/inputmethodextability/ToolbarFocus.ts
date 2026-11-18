/** 焦点变化时串行更新系统状态面板，过期的窗口状态不能控制新焦点。 */
export interface FocusToolbarPanel {
  show(): Promise<void>;
  hide(): Promise<void>;
}

export class ToolbarFocus {
  private version: number = 0;
  private tail: Promise<void> = Promise.resolve();

  private readonly report: (error: Object) => void;

  constructor(report: (error: Object) => void) { this.report = report; }

  invalidate(): void { this.version++; }

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
    if (version !== this.version || !isCurrent()) return;
    const operation: Promise<void> = this.tail.then(async (): Promise<void> => {
      if (version !== this.version || !isCurrent()) return;
      if (hide) await panel.hide(); else await panel.show();
    });
    this.tail = operation.catch((error: Object): void => { this.report(error); });
    await this.tail;
  }
}
