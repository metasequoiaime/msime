import audio from '@ohos.multimedia.audio';
import http from '@ohos.net.http';
import { installNoRedirectGuard } from '../../network/HarmonyHttpSecurity';
import util from '@ohos.util';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { VoiceRecognitionPolicy } from './VoiceRecognitionPolicy';
import { HarmonyVoiceCaptureDevices } from './HarmonyVoiceCaptureDevices';
import { VoiceOutcome, VoiceResponsePolicy } from './VoiceResponsePolicy';
import { VoiceInputConfiguration } from './VoiceInputConfiguration';
import { HTTP_ASR_CHAT_AUDIO, HttpAsrConfigurationPolicy } from './HttpAsrConfigurationPolicy';

const SAMPLE_RATE: number = 16000;
const SAMPLE_BYTES: number = 2;
const MAX_DURATION_MS: number = 120000;
const MAX_AUDIO_BYTES: number = SAMPLE_RATE * SAMPLE_BYTES * 120;
const MAX_RESPONSE_BYTES: number = 1024 * 1024;

interface AsrResponse {
  text?: string;
}

export type HttpAsrResultHandler = (text: string, final: boolean) => void;
export type HttpAsrErrorHandler = (message: string) => void;

/** Captures one bounded WAV and sends it to an OpenAI-compatible transcription endpoint. */
export class HarmonyHttpAsrRecognizer {
  private capturer: audio.AudioCapturer | undefined = undefined;
  private readCallback: ((data: ArrayBuffer) => void) | undefined = undefined;
  private request: http.HttpRequest | undefined = undefined;
  private chunks: Uint8Array[] = [];
  private audioBytes: number = 0;
  private sessionToken: number = 0;
  private stopping: boolean = false;
  private settled: boolean = false;
  // True from a valid start() until the session settles, so stop() during capturer setup is not lost.
  private running: boolean = false;
  // True once startCapturer() has returned for this session.
  private capturing: boolean = false;
  private timeoutHandle: number | undefined = undefined;
  private resultHandler: HttpAsrResultHandler | undefined = undefined;
  private errorHandler: HttpAsrErrorHandler | undefined = undefined;

  isActive(): boolean {
    return this.running || this.capturer !== undefined || this.request !== undefined;
  }

  async start(config: VoiceInputConfiguration, onResult: HttpAsrResultHandler,
    onError: HttpAsrErrorHandler): Promise<void> {
    if (this.isActive()) throw new Error('语音识别正在进行');
    if (!this.validConfig(config)) throw new Error('请先配置有效的语音接口、模型和凭据');
    const token: number = this.sessionToken + 1;
    this.sessionToken = token;
    this.resultHandler = onResult;
    this.errorHandler = onError;
    this.chunks = [];
    this.audioBytes = 0;
    this.stopping = false;
    this.settled = false;
    this.running = true;
    this.capturing = false;
    this.timeoutHandle = setTimeout((): void => {
      if (token !== this.sessionToken || this.settled) return;
      // Reaching the limit while recording transcribes what was said; a stuck setup or stop fails.
      if (this.capturing && !this.stopping) {
        this.stop();
      } else {
        void this.fail('语音识别超时。', token);
      }
    }, MAX_DURATION_MS);
    try {
      await this.startCapturer(config, token);
    } catch (error) {
      if (token !== this.sessionToken) return;
      await this.fail(`语音采集启动失败：${describeError(error)}`, token);
      throw new Error('语音采集启动失败');
    }
    if (token !== this.sessionToken || this.settled) return;
    this.capturing = true;
    // A stop() that arrived during setup was only recorded; honour it now.
    if (this.stopping) void this.finish(token);
  }

  stop(): void {
    if (!this.isActive() || this.stopping || this.settled) return;
    this.stopping = true;
    if (this.capturing) void this.finish(this.sessionToken);
  }

  cancel(): void {
    this.sessionToken += 1;
    this.stopping = true;
    this.settled = true;
    this.running = false;
    this.clearTimeout();
    void this.releaseCapturer();
    const request: http.HttpRequest | undefined = this.request;
    this.request = undefined;
    if (request !== undefined) request.destroy();
    this.chunks = [];
    this.audioBytes = 0;
    this.resultHandler = undefined;
    this.errorHandler = undefined;
  }

  shutdown(): void {
    this.cancel();
  }

  private async startCapturer(config: VoiceInputConfiguration, token: number): Promise<void> {
    // Before the capturer exists, because the routing choice is process-wide rather than a property
    // of one stream. A device that has gone away leaves the system default in place.
    await HarmonyVoiceCaptureDevices.select(config.capture_backend, config.capture_device);
    if (token !== this.sessionToken || this.settled || this.stopping) {
      return;
    }
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
    if (token !== this.sessionToken || this.settled || this.stopping) {
      await capturer.release();
      return;
    }
    const readCallback: (data: ArrayBuffer) => void = (data: ArrayBuffer): void => {
      this.readAudio(data, token);
    };
    this.capturer = capturer;
    this.readCallback = readCallback;
    capturer.on('readData', readCallback);
    try {
      await capturer.start();
    } catch (error) {
      capturer.off('readData', readCallback);
      if (this.capturer === capturer) {
        this.capturer = undefined;
        this.readCallback = undefined;
      }
      await capturer.release();
      throw new Error('Harmony 音频采集启动失败');
    }
  }

  private readAudio(data: ArrayBuffer, token: number): void {
    if (token !== this.sessionToken || this.stopping || this.settled || data.byteLength === 0) return;
    let length: number = data.byteLength;
    const full: boolean = this.audioBytes + length >= MAX_AUDIO_BYTES;
    if (full) {
      // Keep the whole-sample prefix that fits, then stop and transcribe instead of discarding.
      length = Math.floor((MAX_AUDIO_BYTES - this.audioBytes) / SAMPLE_BYTES) * SAMPLE_BYTES;
    }
    if (length > 0) {
      const copy: Uint8Array = new Uint8Array(length);
      copy.set(new Uint8Array(data, 0, length));
      this.chunks.push(copy);
      this.audioBytes += copy.length;
    }
    if (full) this.stop();
  }

  private async finish(token: number): Promise<void> {
    await this.releaseCapturer();
    if (token !== this.sessionToken || this.settled) return;
    if (this.audioBytes === 0) {
      await this.fail('没有录到语音。', token);
      return;
    }
    const wav: ArrayBuffer = this.wavBytes();
    const config: VoiceInputConfiguration | undefined = this.currentConfig;
    if (config === undefined) {
      await this.fail('语音配置已失效。', token);
      return;
    }
    // The upload is bounded by the request's own connect/read timeouts, not the recording limit.
    this.clearTimeout();
    await this.send(config, wav, token);
  }

  private currentConfig: VoiceInputConfiguration | undefined = undefined;

  private async send(config: VoiceInputConfiguration, wav: ArrayBuffer, token: number): Promise<void> {
    const endpoint: string = HttpAsrConfigurationPolicy.endpoint(config);
    const model: string = HttpAsrConfigurationPolicy.model(config);
    const credential: string = HttpAsrConfigurationPolicy.token(config);
    const boundary: string = `----msime-${util.generateRandomUUID(false).replace(/-/g, '')}`;
    // 按请求格式挑请求构造：chat_audio 是带 Base64 录音的 JSON，其余是 multipart。
    const chat: boolean =
      HttpAsrConfigurationPolicy.requestFormat(config.asr_provider) === HTTP_ASR_CHAT_AUDIO;
    const body: ArrayBuffer | string = chat
      ? HttpAsrConfigurationPolicy.chatAudioBody(model,
        new util.Base64Helper().encodeToStringSync(new Uint8Array(wav)))
      : this.multipart(boundary, model, config.asr_provider, config.language, wav);
    const request: http.HttpRequest = http.createHttp();
    installNoRedirectGuard(request);
    this.request = request;
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: {
          Accept: 'application/json',
          'Content-Type': chat ? 'application/json' : `multipart/form-data; boundary=${boundary}`,
          Authorization: `Bearer ${credential}`
        },
        connectTimeout: 5000,
        readTimeout: 60000,
        extraData: body
      });
      if (token !== this.sessionToken || this.settled) return;
      const outcome: VoiceOutcome = VoiceResponsePolicy.batchResult(response.responseCode,
        typeof response.result === 'string' ? response.result as string : null);
      if (outcome.failure.length > 0) {
        await this.fail(outcome.failure, token);
        return;
      }
      await this.complete(outcome.text, token);
    } catch (error) {
      if (token === this.sessionToken && !this.settled) {
        await this.fail(`语音服务请求失败：${describeError(error)}`, token);
      }
    } finally {
      if (this.request === request) this.request = undefined;
      request.destroy();
    }
  }

  private async complete(text: string, token: number): Promise<void> {
    if (token !== this.sessionToken || this.settled) return;
    this.settled = true;
    this.running = false;
    this.clearTimeout();
    this.chunks = [];
    this.audioBytes = 0;
    const handler: HttpAsrResultHandler | undefined = this.resultHandler;
    this.resultHandler = undefined;
    this.errorHandler = undefined;
    if (handler !== undefined) handler(text, true);
  }

  private async fail(message: string, token: number): Promise<void> {
    if (token !== this.sessionToken || this.settled) return;
    this.settled = true;
    this.running = false;
    this.clearTimeout();
    await this.releaseCapturer();
    const request: http.HttpRequest | undefined = this.request;
    this.request = undefined;
    if (request !== undefined) request.destroy();
    this.chunks = [];
    this.audioBytes = 0;
    const handler: HttpAsrErrorHandler | undefined = this.errorHandler;
    this.resultHandler = undefined;
    this.errorHandler = undefined;
    if (handler !== undefined) handler(message);
  }

  private wavBytes(): ArrayBuffer {
    const result: Uint8Array = new Uint8Array(44 + this.audioBytes);
    this.writeAscii(result, 0, 'RIFF');
    this.write32(result, 4, 36 + this.audioBytes);
    this.writeAscii(result, 8, 'WAVEfmt ');
    this.write32(result, 16, 16);
    this.write16(result, 20, 1);
    this.write16(result, 22, 1);
    this.write32(result, 24, SAMPLE_RATE);
    this.write32(result, 28, SAMPLE_RATE * SAMPLE_BYTES);
    this.write16(result, 32, SAMPLE_BYTES);
    this.write16(result, 34, 16);
    this.writeAscii(result, 36, 'data');
    this.write32(result, 40, this.audioBytes);
    let offset: number = 44;
    for (const chunk of this.chunks) {
      result.set(chunk, offset);
      offset += chunk.length;
    }
    return result.buffer as ArrayBuffer;
  }

  private multipart(boundary: string, model: string, provider: string, language: string, wav: ArrayBuffer): ArrayBuffer {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    const transcriptionLanguage: string = HttpAsrConfigurationPolicy.transcriptionLanguage(provider, language);
    let fields: string = `--${boundary}\r\nContent-Disposition: form-data; name="model"\r\n\r\n${model}`;
    if (transcriptionLanguage.length > 0) {
      fields += `\r\n--${boundary}\r\nContent-Disposition: form-data; name="language"\r\n\r\n${transcriptionLanguage}`;
    }
    const head: Uint8Array = encoder.encode(
      fields
      + `\r\n--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="speech.wav"\r\n`
      + 'Content-Type: audio/wav\r\n\r\n');
    const tail: Uint8Array = encoder.encode(`\r\n--${boundary}--\r\n`);
    const audio: Uint8Array = new Uint8Array(wav);
    const result: Uint8Array = new Uint8Array(head.length + audio.length + tail.length);
    result.set(head, 0);
    result.set(audio, head.length);
    result.set(tail, head.length + audio.length);
    return result.buffer as ArrayBuffer;
  }

  private validConfig(config: VoiceInputConfiguration): boolean {
    this.currentConfig = config;
    return HttpAsrConfigurationPolicy.valid(config);
  }

  private writeAscii(target: Uint8Array, offset: number, value: string): void {
    for (let index = 0; index < value.length; index++) target[offset + index] = value.charCodeAt(index);
  }

  private write16(target: Uint8Array, offset: number, value: number): void {
    target[offset] = value & 0xff;
    target[offset + 1] = (value >>> 8) & 0xff;
  }

  private write32(target: Uint8Array, offset: number, value: number): void {
    target[offset] = value & 0xff;
    target[offset + 1] = (value >>> 8) & 0xff;
    target[offset + 2] = (value >>> 16) & 0xff;
    target[offset + 3] = (value >>> 24) & 0xff;
  }

  private async releaseCapturer(): Promise<void> {
    const capturer: audio.AudioCapturer | undefined = this.capturer;
    this.capturer = undefined;
    if (capturer === undefined) return;
    if (this.readCallback !== undefined) capturer.off('readData', this.readCallback);
    this.readCallback = undefined;
    try {
      await capturer.stop();
    } catch (error) {
      KeyboardLog.error(`Harmony HTTP ASR 音频停止失败：${describeError(error)}`);
    }
    try {
      await capturer.release();
    } catch (error) {
      KeyboardLog.error(`Harmony HTTP ASR 音频释放失败：${describeError(error)}`);
    }
  }

  private clearTimeout(): void {
    if (this.timeoutHandle !== undefined) clearTimeout(this.timeoutHandle);
    this.timeoutHandle = undefined;
  }
}
