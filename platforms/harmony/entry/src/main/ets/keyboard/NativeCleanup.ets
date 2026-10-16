import fs from '@ohos.file.fs';
import audio from '@ohos.multimedia.audio';
import { KeyboardLog } from './KeyboardLog';

/** 清理失败单独记录，不打断后续释放，也不覆盖原始操作的失败。 */
export class NativeCleanup {
  static close(file: fs.File): void {
    try {
      fs.closeSync(file);
    } catch {
      KeyboardLog.error('native file cleanup failed');
    }
  }

  static detach(capturer: audio.AudioCapturer, callback: (data: ArrayBuffer) => void): void {
    try {
      capturer.off('readData', callback);
    } catch {
      KeyboardLog.error('audio read callback cleanup failed');
    }
  }
}
