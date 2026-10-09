/**
 * Google 登录回环监听上一次连接的请求头：按字节累积，读到第一个空行（`\r\n\r\n`）为止。
 *
 * 读到的文本交给 `msime_client_google_loopback` 的 reply 去判定，这里只负责截断：超过上限（plan 给的 `max_request_bytes`）或含非 ASCII 字节时，当作没读到完整请求头，reply 收到 null 后回 404，监听继续等下一个连接。浏览器发来的回跳请求行是百分号编码的 ASCII，所以非 ASCII 字节只可能来自别的东西。
 */
export class GoogleLoopbackRequestHead {
  private readonly maximum: number;
  private readonly bytes: number[] = [];
  private complete: boolean = false;
  private refused: boolean = false;

  constructor(maximum: number) {
    this.maximum = maximum;
  }

  /** 追加一段收到的字节。返回 true 表示可以回复了：读到了空行，或已经超限。 */
  push(chunk: Uint8Array): boolean {
    if (this.complete || this.refused) {
      return true;
    }
    for (let index = 0; index < chunk.length; index++) {
      const byte: number = chunk[index];
      if (this.bytes.length >= this.maximum || byte >= 0x80) {
        this.refused = true;
        return true;
      }
      this.bytes.push(byte);
      const length: number = this.bytes.length;
      if (
        length >= 4 &&
        this.bytes[length - 4] === 0x0d &&
        this.bytes[length - 3] === 0x0a &&
        this.bytes[length - 2] === 0x0d &&
        this.bytes[length - 1] === 0x0a
      ) {
        this.complete = true;
        return true;
      }
    }
    return false;
  }

  /** 完整的请求头（含结尾的空行）；还没读完、超限或含非 ASCII 字节时为 null。 */
  head(): string | null {
    if (!this.complete || this.refused) {
      return null;
    }
    let text: string = "";
    for (let index = 0; index < this.bytes.length; index++) {
      text += String.fromCharCode(this.bytes[index]);
    }
    return text;
  }
}
