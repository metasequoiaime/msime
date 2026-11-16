import pasteboard from '@ohos.pasteboard';
import { ClipboardHistoryCapture, ClipboardHistoryCaptureHost } from './ClipboardHistoryCapture';

/** 系统读取本身仍核对 READ_PASTEBOARD；这里不请求权限，不在开启前导入已有正文。 */
class HarmonyClipboardCaptureHost implements ClipboardHistoryCaptureHost {
  private readonly board: pasteboard.SystemPasteboard = pasteboard.getSystemPasteboard();
  private readonly canCapture: () => boolean;
  private readonly canRead: () => Promise<boolean>;
  private readonly store: (text: string) => boolean;

  constructor(canCapture: () => boolean, canRead: () => Promise<boolean>, store: (text: string) => boolean) {
    this.canCapture = canCapture;
    this.canRead = canRead;
    this.store = store;
  }

  allowed(): boolean { return this.canCapture(); }
  permitted(): Promise<boolean> { return this.canRead(); }
  watch(listener: () => void): void { this.board.on('update', listener); }
  unwatch(listener: () => void): void { this.board.off('update', listener); }
  save(text: string): boolean { return this.store(text); }

  async read(): Promise<string> {
    if (!this.allowed()) return '';
    const data: pasteboard.PasteData = await this.board.getData();
    if (!this.allowed() || !data.hasType(pasteboard.MIMETYPE_TEXT_PLAIN)) return '';
    return data.getPrimaryText() ?? '';
  }
}

export class HarmonyClipboardHistoryCapture extends ClipboardHistoryCapture {
  constructor(allowed: () => boolean, permitted: () => Promise<boolean>, save: (text: string) => boolean) {
    super(new HarmonyClipboardCaptureHost(allowed, permitted, save));
  }
}
