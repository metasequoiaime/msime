import audio from '@ohos.multimedia.audio';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { HarmonyVoiceCaptureDevices } from './HarmonyVoiceCaptureDevices';
import { CaptureGeneration } from './CaptureGeneration';

export type PcmChunkHandler = (data: ArrayBuffer) => void;

/**
 * One 16 kHz mono PCM16 microphone stream for the recognizers that are handed audio rather than opening the microphone themselves: CoreSpeechKit in its write-audio mode and the on-device sherpa-onnx recognizer.
 *
 * Buffers are passed on as they arrive and are not kept here, written to disk or logged. `start` resolves false rather than throwing when `stop` was called while the capturer was being created, so a cancelled start never leaves a live microphone behind.
 */
export class HarmonyPcmCapture {
  private capturer: audio.AudioCapturer | undefined = undefined;
  private readCallback: ((data: ArrayBuffer) => void) | undefined = undefined;
  private readonly generations: CaptureGeneration = new CaptureGeneration();
  private releaseQueue: Promise<void> = Promise.resolve();

  isActive(): boolean {
    return this.capturer !== undefined;
  }

  async start(backend: string, device: string, onChunk: PcmChunkHandler): Promise<boolean> {
    // 先占用启动代数；停止请求若落在下面的 await 期间，不能被本次启动重新覆盖。
    const generation: number = this.generations.begin();
    await this.enqueueRelease();
    if (!this.generations.isCurrent(generation)) return false;
    // Routing is process-wide rather than per stream, so it is asserted before every capturer; a device that has gone away leaves the system default in place.
    await HarmonyVoiceCaptureDevices.select(backend, device);
    if (!this.generations.isCurrent(generation)) return false;
    const options: audio.AudioCapturerOptions = {
      streamInfo: {
        samplingRate: audio.AudioSamplingRate.SAMPLE_RATE_16000,
        channels: audio.AudioChannel.CHANNEL_1,
        sampleFormat: audio.AudioSampleFormat.SAMPLE_FORMAT_S16LE,
        encodingType: audio.AudioEncodingType.ENCODING_TYPE_RAW
      },
      capturerInfo: {
        source: audio.SourceType.SOURCE_TYPE_MIC,
        capturerFlags: 0
      }
    };
    const capturer: audio.AudioCapturer = await audio.createAudioCapturer(options);
    if (!this.generations.isCurrent(generation)) {
      await capturer.release();
      return false;
    }
    const readCallback: (data: ArrayBuffer) => void = (data: ArrayBuffer): void => {
      if (this.generations.isCurrent(generation) && data.byteLength > 0) onChunk(data);
    };
    this.capturer = capturer;
    this.readCallback = readCallback;
    capturer.on('readData', readCallback);
    try {
      await capturer.start();
    } catch (error) {
      await this.stop();
      throw new Error('Harmony 音频采集启动失败');
    }
    return this.generations.isCurrent(generation);
  }

  /** Stops delivering audio at once and releases the capturer. Safe to call at any time, including while `start` is still pending. */
  async stop(): Promise<void> {
    this.generations.invalidate();
    await this.enqueueRelease();
  }

  /** 串行释放，避免新启动越过仍在等待底层 stop/release 的旧采集器。 */
  private enqueueRelease(): Promise<void> {
    const release: Promise<void> = this.releaseQueue.then((): Promise<void> => this.releaseCurrent());
    this.releaseQueue = release;
    return release;
  }

  /** 释放当前采集器，不改变启动代数；启动流程用它清理上一代。 */
  private async releaseCurrent(): Promise<void> {
    const capturer: audio.AudioCapturer | undefined = this.capturer;
    const readCallback: ((data: ArrayBuffer) => void) | undefined = this.readCallback;
    this.capturer = undefined;
    this.readCallback = undefined;
    if (capturer === undefined) return;
    if (readCallback !== undefined) capturer.off('readData', readCallback);
    try {
      await capturer.stop();
    } catch (error) {
      KeyboardLog.error(`Harmony 音频停止失败：${describeError(error)}`);
    }
    try {
      await capturer.release();
    } catch (error) {
      KeyboardLog.error(`Harmony 音频释放失败：${describeError(error)}`);
    }
  }
}
