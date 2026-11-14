import { CloudClipboardBoundListing } from './CloudClipboardClient';
import { CloudClipboardItem, CloudClipboardPolicy, CloudClipboardSendOutcome, CloudClipboardState } from './CloudClipboardPolicy';
import { CloudClipboardReceivePolicy } from './CloudClipboardReceivePolicy';

interface CloudClipboardReceiveHost {
  visible: () => boolean;
  readAllowed: () => Promise<boolean>;
  list: () => Promise<CloudClipboardBoundListing | null>;
  matches: (binding: string) => boolean;
  read: () => Promise<string | null>;
  write: (text: string) => Promise<boolean>;
  send: (text: string, binding: string) => Promise<CloudClipboardSendOutcome>;
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
  private generation: number = 0;

  constructor(host: CloudClipboardReceiveHost) {
    this.host = host;
  }

  start(): void {
    this.stop();
    this.active = true;
    this.schedule(0);
  }

  stop(): void {
    this.generation++;
    this.active = false;
    this.busy = false;
    if (this.timer >= 0) {
      clearTimeout(this.timer);
      this.timer = -1;
    }
    this.binding = null;
    this.initialized = false;
  }

  private current(generation: number): boolean {
    return this.active && generation === this.generation && this.host.visible();
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
    this.busy = true;
    const generation: number = this.generation;
    let delay: number = 30000;
    try {
      if (!await this.host.readAllowed() || !this.current(generation)) return;
      const localText: string | null = await this.host.read();
      if (!this.current(generation)) return;
      const locallyChanged: boolean = this.initialized && localText !== this.policy.currentText();
      if (!this.initialized) {
        this.policy.begin(localText);
        this.initialized = true;
      } else {
        if (localText !== this.policy.currentText()) this.policy.localChanged(localText);
      }
      const listing: CloudClipboardBoundListing | null = await this.host.list();
      if (!this.current(generation)) return;
      const sameBinding: boolean = listing !== null && listing.binding !== null && listing.binding === this.binding;
      if (listing !== null && listing.binding !== null && listing.binding !== this.binding) {
        this.binding = listing.binding;
        this.policy.begin(localText);
      }
      if (listing !== null && listing.binding !== null && listing.state === CloudClipboardState.READY) {
        delay = 3000;
        if (!this.host.matches(listing.binding) || !this.current(generation)) return;
        if (!await this.host.readAllowed() || !this.current(generation)) return;
        const latest: CloudClipboardItem | null = listing.items.length > 0 ? listing.items[0] : null;
        const current: string | null = await this.host.read();
        if (!this.current(generation) || !this.host.matches(listing.binding)) return;
        if (sameBinding && (locallyChanged || current !== localText) && current !== null
          && CloudClipboardPolicy.validText(current)) {
          // 只发送本轮观察到的新复制；先建立水位，失败也不延后补传旧文本。
          this.policy.localChanged(current);
          this.policy.receive(latest, current);
          if (latest === null || latest.text !== current) await this.host.send(current, listing.binding);
          return;
        }
        if (this.policy.receive(latest, current) && latest !== null && await this.host.write(latest.text)) {
          if (!this.current(generation)) return;
          this.policy.markRemote(latest);
          this.policy.received(latest.text);
        }
      }
    } catch (_) {
      // 网络、剪贴板权限和宿主销毁都在下一轮重试，不向输入过程抛错。
    } finally {
      if (generation === this.generation) {
        this.busy = false;
        this.schedule(delay);
      }
    }
  }
}
