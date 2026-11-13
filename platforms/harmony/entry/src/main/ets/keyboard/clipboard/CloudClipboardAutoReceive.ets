import { CloudClipboardBoundListing } from './CloudClipboardClient';
import { CloudClipboardItem, CloudClipboardState } from './CloudClipboardPolicy';
import { CloudClipboardReceivePolicy } from './CloudClipboardReceivePolicy';

interface CloudClipboardReceiveHost {
  visible: () => boolean;
  readAllowed: () => Promise<boolean>;
  list: () => Promise<CloudClipboardBoundListing | null>;
  read: () => Promise<string>;
  write: (text: string) => Promise<boolean>;
}

/** 键盘可见期间轮询云端，并把新条目写入系统剪贴板；失败或离开键盘时不保留请求。 */
export class CloudClipboardAutoReceive {
  private readonly host: CloudClipboardReceiveHost;
  private readonly policy: CloudClipboardReceivePolicy = new CloudClipboardReceivePolicy();
  private timer: number = -1;
  private active: boolean = false;
  private busy: boolean = false;
  private binding: string | null = null;
  private initialized: boolean = false;

  constructor(host: CloudClipboardReceiveHost) {
    this.host = host;
  }

  start(): void {
    this.stop();
    this.active = true;
    this.schedule(0);
  }

  stop(): void {
    this.active = false;
    this.busy = false;
    if (this.timer >= 0) {
      clearTimeout(this.timer);
      this.timer = -1;
    }
    this.binding = null;
    this.initialized = false;
  }

  private schedule(delay: number): void {
    if (!this.active) return;
    if (this.timer >= 0) clearTimeout(this.timer);
    this.timer = setTimeout((): void => {
      this.timer = -1;
      this.poll();
    }, delay);
  }

  private async poll(): Promise<void> {
    if (!this.active || this.busy) {
      this.schedule(3000);
      return;
    }
    if (!this.host.visible()) {
      this.schedule(30000);
      return;
    }
    if (!await this.host.readAllowed()) {
      this.schedule(30000);
      return;
    }
    this.busy = true;
    let delay: number = 30000;
    try {
      const localText: string = await this.host.read();
      if (!this.initialized) {
        this.policy.begin(localText);
        this.initialized = true;
      } else {
        const known: string = localText;
        if (known !== this.lastKnownText()) this.policy.localChanged(known);
      }
      const listing: CloudClipboardBoundListing | null = await this.host.list();
      if (listing !== null && listing.binding !== null && listing.binding !== this.binding) {
        this.binding = listing.binding;
        this.policy.begin(localText);
      }
      if (listing !== null && listing.state === CloudClipboardState.READY && listing.items.length > 0) {
        delay = 3000;
        const latest: CloudClipboardItem = listing.items[0];
        const current: string = await this.host.read();
        if (this.policy.receive(latest, current) && await this.host.write(latest.text)) {
          this.policy.markRemote(latest);
          this.policy.received(latest.text);
        }
      }
    } catch (_) {
      // 网络、剪贴板权限和宿主销毁都在下一轮重试，不向输入过程抛错。
    } finally {
      this.busy = false;
      this.schedule(delay);
    }
  }

  private lastKnownText(): string {
    // The policy owns the text; this helper avoids retaining a second clipboard copy.
    return this.policy.currentText();
  }
}
