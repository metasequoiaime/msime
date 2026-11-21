/** 焦点变化时串行请求显示状态面板，过期请求不能作用于新焦点。 */
export interface FocusToolbarPanel {
  show(): Promise<void>;
}

export class ToolbarFocus {
  private version: number = 0;
  private tail: Promise<void> = Promise.resolve();
  private readonly report: (error: Object) => void;

  constructor(report: (error: Object) => void) { this.report = report; }

  invalidate(): void { this.version++; }

  async focus(panel: FocusToolbarPanel, isCurrent: () => boolean): Promise<void> {
    const version: number = ++this.version;
    const operation: Promise<void> = this.tail.then(async (): Promise<void> => {
      if (version !== this.version || !isCurrent()) return;
      await panel.show();
    });
    this.tail = operation.catch((error: Object): void => { this.report(error); });
    await this.tail;
  }
}
