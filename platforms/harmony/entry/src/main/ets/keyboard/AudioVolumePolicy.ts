/** 音效和背景音乐共用的百分比音量到播放增益的转换。 */
export class AudioVolumePolicy {
  static gain(volume: number): number {
    return Math.min(Math.max(volume, 0), 100) / 100;
  }
}
