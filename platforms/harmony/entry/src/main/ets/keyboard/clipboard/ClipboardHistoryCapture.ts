import { ClipboardHistoryPolicy } from "./ClipboardHistoryPolicy";

export interface ClipboardHistoryCaptureHost {
  allowed(): boolean;
  permitted(): Promise<boolean>;
  watch(listener: () => void): void;
  unwatch(listener: () => void): void;
  read(): Promise<string>;
  save(text: string): boolean;
}

class ClipboardCopy {
  ready: boolean = false;
  text: string = "";
}

/** 每次通知立即请求正文，保存按事件顺序进行；停止后迟到的权限与读取结果全部失效。 */
export class ClipboardHistoryCapture {
  static readonly CAPACITY: number = 128;
  private generation: number = 0;
  private active: boolean = false;
  private listening: boolean = false;
  private pending: ClipboardCopy[] = [];
  private retry: number = -1;
  private readonly host: ClipboardHistoryCaptureHost;
  private readonly changed: () => void = (): void => this.capture();

  constructor(host: ClipboardHistoryCaptureHost) {
    this.host = host;
  }

  async start(): Promise<void> {
    if (this.active) return;
    this.active = true;
    const generation: number = ++this.generation;
    try {
      const permitted: boolean = await this.host.permitted();
      if (!this.active || generation !== this.generation) return;
      if (!permitted) {
        this.stop();
        return;
      }
      this.host.watch(this.changed);
      this.listening = true;
    } catch (_) {
      if (generation === this.generation) this.stop();
    }
  }

  stop(): void {
    this.active = false;
    this.generation++;
    const wasListening: boolean = this.listening;
    this.listening = false;
    this.pending = [];
    if (this.retry >= 0) clearTimeout(this.retry);
    this.retry = -1;
    if (wasListening) this.host.unwatch(this.changed);
  }

  discardIfDisallowed(): void {
    if (!this.host.allowed()) {
      this.pending = [];
      if (this.retry >= 0) clearTimeout(this.retry);
      this.retry = -1;
    }
  }

  private capture(): void {
    if (!this.active || !this.host.allowed()) {
      this.discardIfDisallowed();
      return;
    }
    const generation: number = this.generation;
    const copy: ClipboardCopy = new ClipboardCopy();
    if (this.pending.length === ClipboardHistoryCapture.CAPACITY) this.pending.shift();
    this.pending.push(copy);
    // 不等上一条读取或保存，避免多次通知最后才去读系统当前正文。
    this.host
      .read()
      .then((text: string): void => {
        if (generation !== this.generation || !this.active) return;
        copy.text = ClipboardHistoryPolicy.acceptable(text) ? text : "";
        copy.ready = true;
        this.flush();
      })
      .catch((): void => {
        if (generation !== this.generation || !this.active) return;
        copy.ready = true;
        this.flush();
      });
  }

  private flush(): void {
    if (!this.active || !this.host.allowed()) {
      this.discardIfDisallowed();
      return;
    }
    while (this.pending.length > 0 && this.pending[0].ready) {
      const copy: ClipboardCopy = this.pending[0];
      if (copy.text.length > 0 && !this.host.save(copy.text)) {
        if (this.retry < 0) {
          this.retry = setTimeout((): void => {
            this.retry = -1;
            this.flush();
          }, 100);
        }
        return;
      }
      this.pending.shift();
    }
  }
}
