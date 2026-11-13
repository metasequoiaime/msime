import { CloudClipboardItem } from "./CloudClipboardPolicy";

/** 自动接收云剪贴板的状态，只在本机剪贴板没有被用户更新时替换文本。 */
export class CloudClipboardReceivePolicy {
  private latestRemoteMillis: number = 0;
  private localText: string = "";
  private receiverOwned: boolean = false;

  begin(localText: string): void {
    this.localText = localText;
    this.receiverOwned = false;
  }

  localChanged(localText: string): void {
    this.localText = localText;
    this.receiverOwned = false;
  }

  received(localText: string): void {
    this.localText = localText;
    this.receiverOwned = true;
  }

  currentText(): string {
    return this.localText;
  }

  receive(item: CloudClipboardItem, localText: string): boolean {
    const remoteMillis: number = Date.parse(item.updatedAt);
    if (!Number.isFinite(remoteMillis) || remoteMillis <= this.latestRemoteMillis) return false;
    if (localText.length > 0 && !this.receiverOwned) return false;
    if (localText === item.text) return false;
    return true;
  }

  markRemote(item: CloudClipboardItem): void {
    const remoteMillis: number = Date.parse(item.updatedAt);
    if (Number.isFinite(remoteMillis)) this.latestRemoteMillis = remoteMillis;
  }
}
