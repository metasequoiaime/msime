/** 工具栏随鼠标移动时，屏幕坐标仍保持稳定。 */
export interface ToolbarDragFinger {
  readonly displayX: number;
  readonly displayY: number;
}

export interface ToolbarDragEvent {
  readonly offsetX: number;
  readonly offsetY: number;
  readonly fingerList: ToolbarDragFinger[];
}

export class ToolbarDragGesture {
  private origin: [number, number] | null = null;
  private fallbackOrigin: [number, number] = [0, 0];

  start(event: ToolbarDragEvent): void {
    this.origin = this.screenPosition(event);
    this.fallbackOrigin = [event.offsetX, event.offsetY];
  }

  update(event: ToolbarDragEvent): [number, number] | null {
    if (this.origin !== null) {
      const position: [number, number] | null = this.screenPosition(event);
      return position === null ? null : [position[0] - this.origin[0], position[1] - this.origin[1]];
    }
    if (!Number.isFinite(event.offsetX) || !Number.isFinite(event.offsetY)) return null;
    return [event.offsetX - this.fallbackOrigin[0], event.offsetY - this.fallbackOrigin[1]];
  }

  private screenPosition(event: ToolbarDragEvent): [number, number] | null {
    // 鼠标或触控板通常只有一个指针；其他输入来源可能没有指针。
    for (const finger of event.fingerList) {
      if (finger !== undefined && Number.isFinite(finger.displayX) && Number.isFinite(finger.displayY)) {
        return [finger.displayX, finger.displayY];
      }
    }
    return null;
  }
}
