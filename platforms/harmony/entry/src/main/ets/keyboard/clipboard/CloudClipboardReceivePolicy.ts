import { CloudClipboardItem } from "./CloudClipboardPolicy";

/** 自动接收云剪贴板的状态，只在本机剪贴板没有被用户更新时替换文本。 */
export class CloudClipboardReceivePolicy {
  private latestRemoteMillis: number = 0;
  private localText: string | null = null;
  private needsBaseline: boolean = true;

  begin(localText: string | null): void {
    this.latestRemoteMillis = 0;
    this.localText = localText;
    this.needsBaseline = localText !== "";
  }

  localChanged(localText: string | null): void {
    this.localText = localText;
    this.needsBaseline = true;
  }

  received(localText: string): void {
    this.localText = localText;
    this.needsBaseline = false;
  }

  currentText(): string | null {
    return this.localText;
  }

  receive(item: CloudClipboardItem | null, localText: string | null): boolean {
    if (localText !== this.localText) this.localChanged(localText);
    if (this.needsBaseline) {
      if (item !== null) {
        if (!Number.isFinite(Date.parse(item.updatedAt))) return false;
        this.markRemote(item);
      }
      this.needsBaseline = false;
      return false;
    }
    if (item === null || localText === null) return false;
    const remoteMillis: number = Date.parse(item.updatedAt);
    if (!Number.isFinite(remoteMillis) || remoteMillis <= this.latestRemoteMillis) return false;
    if (localText === item.text) {
      this.markRemote(item);
      return false;
    }
    return true;
  }

  markRemote(item: CloudClipboardItem): void {
    const remoteMillis: number = Date.parse(item.updatedAt);
    if (Number.isFinite(remoteMillis) && remoteMillis > this.latestRemoteMillis)
      this.latestRemoteMillis = remoteMillis;
  }
}
