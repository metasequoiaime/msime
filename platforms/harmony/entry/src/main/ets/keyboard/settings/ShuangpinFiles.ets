import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import fs from '@ohos.file.fs';
import util from '@ohos.util';

/** 文件选择器只提供用户授权的 URI；容量限制与完整读写在宿主边界执行。 */
export class ShuangpinFiles {
  static async read(context: common.UIAbilityContext): Promise<string | undefined> {
    const options = new picker.DocumentSelectOptions();
    options.maxSelectNumber = 1;
    const chosen: string[] = await new picker.DocumentViewPicker(context).select(options);
    if (chosen.length === 0) return undefined;
    const file: fs.File = fs.openSync(chosen[0], fs.OpenMode.READ_ONLY);
    try {
      const size: number = fs.statSync(file.fd).size;
      if (size > 16384) throw new Error('码表文件不能超过 16 KB');
      const buffer = new ArrayBuffer(16385);
      let count: number = 0;
      while (count < buffer.byteLength) {
        const chunk = new ArrayBuffer(buffer.byteLength - count);
        const read: number = fs.readSync(file.fd, chunk);
        if (read <= 0) break;
        new Uint8Array(buffer).set(new Uint8Array(chunk, 0, read), count);
        count += read;
      }
      if (count > 16384) throw new Error('码表文件不能超过 16 KB');
      return util.TextDecoder.create('utf-8', { fatal: true }).decodeWithStream(new Uint8Array(buffer, 0, count));
    } finally { fs.closeSync(file); }
  }

  static async write(context: common.UIAbilityContext, text: string): Promise<boolean> {
    const options = new picker.DocumentSaveOptions();
    options.newFileNames = ['shuangpin-custom.json'];
    const chosen: string[] = await new picker.DocumentViewPicker(context).save(options);
    if (chosen.length === 0) return false;
    const file: fs.File = fs.openSync(chosen[0], fs.OpenMode.READ_WRITE | fs.OpenMode.CREATE | fs.OpenMode.TRUNC);
    try {
      const encoded: Uint8Array = new util.TextEncoder().encodeInto(text);
      let written: number = 0;
      while (written < encoded.byteLength) {
        const remaining: Uint8Array = encoded.slice(written);
        const count: number = fs.writeSync(file.fd, remaining.buffer);
        if (count <= 0) throw new Error('文件没有完整写入，请重试');
        written += count;
      }
      return true;
    } finally { fs.closeSync(file); }
  }
}
