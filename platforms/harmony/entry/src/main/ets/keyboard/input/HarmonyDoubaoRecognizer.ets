import audio from '@ohos.multimedia.audio';
import webSocket from '@ohos.net.webSocket';
import util from '@ohos.util';
import client, { DoubaoFrameResult } from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { VoiceRecognitionPolicy } from './VoiceRecognitionPolicy';
import { HarmonyVoiceCaptureDevices } from './HarmonyVoiceCaptureDevices';
import { VoiceOutcome, VoiceResponsePolicy } from './VoiceResponsePolicy';
import { VoiceInputConfiguration } from './VoiceInputConfiguration';
import { TextPolicy } from '../TextPolicy';

const PCM_CHUNK_BYTES: number = 6400;
const MAX_PCM_QUEUE_BYTES: number = 16000 * 2 * 10;
const SESSION_TIMEOUT_MS: number = 120000;

interface DoubaoResponseResult {
  text?: string;
}

interface DoubaoResponseBody {
  result?: DoubaoResponseResult;
  code?: number;
  error?: Object | null;
}

interface DoubaoResponseDocument {
  result?: DoubaoResponseResult;
  payload_msg?: DoubaoResponseBody;
  code?: number;
  error?: Object | null;
}

interface DoubaoStartRequestBody {
  model_name: string;
  enable_itn: boolean;
  enable_punc: boolean;
  enable_ddc: boolean;
  show_utterances: boolean;
  result_type: string;
  corpus?: DoubaoCorpus;
}

interface DoubaoCorpus {
  boosting_table_id: string;
}

interface DoubaoStartRequest {
  user: DoubaoUser;
  audio: DoubaoAudio;
  request: DoubaoStartRequestBody;
}

interface DoubaoUser {
  uid: string;
}

interface DoubaoAudio {
  format: string;
  codec: string;
  rate: number;
  bits: number;
  channel: number;
}

export type DoubaoResultHandler = (text: string, final: boolean) => void;
export type DoubaoErrorHandler = (message: string) => void;

/** Streams Harmony's native PCM capturer to the same bounded Doubao protocol as Windows. */
export class HarmonyDoubaoRecognizer {
  private socket: webSocket.WebSocket | undefined = undefined;
  private capturer: audio.AudioCapturer | undefined = undefined;
  private readCallback: ((data: ArrayBuffer) => void) | undefined = undefined;
  private sendChain: Promise<boolean> = Promise.resolve(true);
  private queuedBytes: number = 0;
  private pending: Uint8Array = new Uint8Array(0);
  private sequence: number = 2;
  private sessionId: string = '';
  private stopping: boolean = false;
  private settled: boolean = false;
  // The socket object exists while connect() is pending, but it cannot accept a frame yet.
  // Keeping this separate prevents stop() from sending an end frame before the start frame.
  private connected: boolean = false;
  private timeoutHandle: number | undefined = undefined;
  private resultHandler: DoubaoResultHandler | undefined = undefined;
  private errorHandler: DoubaoErrorHandler | undefined = undefined;
  private sessionToken: number = 0;

  isActive(): boolean {
    return this.socket !== undefined || this.capturer !== undefined;
  }

  async start(config: VoiceInputConfiguration, onResult: DoubaoResultHandler,
    onError: DoubaoErrorHandler): Promise<void> {
    if (this.socket !== undefined || this.capturer !== undefined) {
      throw new Error('豆包语音识别正在进行');
    }
    if (!this.validConfig(config)) {
      throw new Error('请先配置有效的豆包接口、资源 ID 和凭据');
    }
    this.resultHandler = onResult;
    this.errorHandler = onError;
    const token: number = this.sessionToken + 1;
    this.sessionToken = token;
    this.sessionId = util.generateRandomUUID(false);
    this.stopping = false;
    this.settled = false;
    this.connected = false;
    this.pending = new Uint8Array(0);
    this.queuedBytes = 0;
    this.sequence = 2;
    this.sendChain = Promise.resolve(true);
    const socket: webSocket.WebSocket = webSocket.createWebSocket();
    this.socket = socket;
    socket.on('error', (): void => {
      if (this.isCurrent(token, socket)) void this.fail('豆包语音连接失败，请检查网络和凭据。', token);
    });
    socket.on('close', (_error: Object, _result: webSocket.CloseResult): void => {
      if (!this.isCurrent(token, socket) || this.settled) return;
      if (this.stopping) {
        // Closed after stop() without a last frame: finish with the text so far instead of hanging.
        if (this.resultHandler !== undefined) this.resultHandler('', true);
        void this.complete(token, socket);
      } else {
        void this.fail('豆包语音连接已关闭。', token);
      }
    });
    socket.on('message', (_error: Object, value: string | ArrayBuffer): void => {
      if (this.isCurrent(token, socket)) this.receive(value, token, socket);
    });
    this.timeoutHandle = setTimeout((): void => {
      if (this.isCurrent(token, socket)) void this.fail('豆包语音识别超时。', token);
    }, SESSION_TIMEOUT_MS);
    try {
      const connected: boolean = await socket.connect(config.asr_endpoint, this.headers(config));
      if (!connected || !this.isCurrent(token, socket) || this.settled) throw new Error('connect failed');
      if (this.stopping) {
        await this.complete(token, socket);
        return;
      }
      const start: DoubaoStartRequest = {
        user: { uid: 'metasequoia-ime' },
        audio: { format: 'pcm', codec: 'raw', rate: 16000, bits: 16, channel: 1 },
        request: {
          model_name: 'bigmodel',
          enable_itn: config.doubao_enable_itn,
          enable_punc: config.doubao_enable_punc,
          enable_ddc: config.doubao_enable_ddc,
          show_utterances: false,
          result_type: 'full'
        }
      };
      if (config.doubao_boosting_table_id.length > 0) {
        start.request.corpus = { boosting_table_id: config.doubao_boosting_table_id };
      }
      const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
      const payload: ArrayBuffer = encoder.encode(JSON.stringify(start)).buffer as ArrayBuffer;
      const frame: ArrayBuffer = client.doubaoEncodeFrame(1, 1, 1, payload);
      if (!await socket.send(frame)) throw new Error('start frame rejected');
      this.connected = true;
      if (this.stopping) {
        await this.complete(token, socket);
        return;
      }
      await this.startCapturer(config, token, socket);
    } catch (error) {
      if (!this.isCurrent(token, socket)) return;
      // A user stop may race the connection attempt. It is a normal cancellation, not a
      // startup failure; report neither an error nor a phantom microphone session.
      if (this.stopping) {
        await this.complete(token, socket);
        return;
      }
      await this.fail(`豆包语音启动失败：${describeError(error)}`, token);
      throw new Error('豆包语音启动失败');
    }
  }

  stop(): void {
    if (this.socket === undefined || this.stopping || this.settled) return;
    this.stopping = true;
    void this.finishFrame();
  }

  cancel(): void {
    this.sessionToken += 1;
    if (this.socket === undefined && this.capturer === undefined) return;
    this.stopping = true;
    this.settled = true;
    this.clearTimeout();
    void this.releaseCapturer();
    const socket: webSocket.WebSocket | undefined = this.socket;
    this.socket = undefined;
    if (socket !== undefined) socket.close().catch((): void => undefined);
    this.resultHandler = undefined;
    this.errorHandler = undefined;
    this.pending = new Uint8Array(0);
  }

  shutdown(): void {
    this.cancel();
  }

  private async startCapturer(config: VoiceInputConfiguration, token: number,
                              socket: webSocket.WebSocket): Promise<void> {
    // Before the capturer exists, because the routing choice is process-wide rather than a property
    // of one stream. A device that has gone away leaves the system default in place.
    await HarmonyVoiceCaptureDevices.select(config.capture_backend, config.capture_device);
    if (!this.isCurrent(token, socket) || this.settled || this.stopping) {
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
    if (!this.isCurrent(token, socket) || this.settled || this.stopping) {
      await capturer.release();
      return;
    }
    this.capturer = capturer;
    const readCallback: (data: ArrayBuffer) => void = (data: ArrayBuffer): void => {
      this.onAudio(data, token);
    };
    this.readCallback = readCallback;
    capturer.on('readData', readCallback);
    // `stop()` may have arrived after the capturer was created but before `start()`;
    // do not bring a microphone up for a session that is already finishing.
    if (!this.isCurrent(token, socket) || this.settled || this.stopping) {
      capturer.off('readData', readCallback);
      if (this.capturer === capturer) {
        this.readCallback = undefined;
        this.capturer = undefined;
      }
      await capturer.release();
      return;
    }
    try {
      await capturer.start();
    } catch (error) {
      capturer.off('readData', readCallback);
      if (this.capturer === capturer) {
        this.readCallback = undefined;
        this.capturer = undefined;
      }
      await capturer.release();
      throw new Error('Harmony 音频采集启动失败');
    }
  }

  private onAudio(data: ArrayBuffer, token: number): void {
    if (token !== this.sessionToken || this.settled || this.stopping || data.byteLength === 0) return;
    const bytes: Uint8Array = new Uint8Array(data);
    if (this.queuedBytes + this.pending.length + bytes.length > MAX_PCM_QUEUE_BYTES) {
      void this.fail('豆包语音音频队列已满。', this.sessionToken);
      return;
    }
    const merged: Uint8Array = new Uint8Array(this.pending.length + bytes.length);
    merged.set(this.pending, 0);
    merged.set(bytes, this.pending.length);
    this.pending = merged;
    while (this.pending.length >= PCM_CHUNK_BYTES && !this.stopping && !this.settled) {
      const chunk: ArrayBuffer = this.pending.slice(0, PCM_CHUNK_BYTES).buffer as ArrayBuffer;
      this.pending = this.pending.slice(PCM_CHUNK_BYTES);
      this.queueFrame(client.doubaoEncodeFrame(2, 1, this.sequence, chunk), PCM_CHUNK_BYTES,
        this.sessionToken);
      this.sequence += 1;
    }
  }

  private queueFrame(frame: ArrayBuffer, bytes: number, token: number): void {
    const socket: webSocket.WebSocket | undefined = this.socket;
    if (socket === undefined || this.settled || !this.isCurrent(token, socket)) return;
    this.queuedBytes += bytes;
    this.sendChain = this.sendChain.then((ok: boolean): Promise<boolean> => {
      if (!ok || this.settled) return Promise.resolve(false);
      return socket.send(frame);
    }).then((ok: boolean): boolean => {
      this.queuedBytes = Math.max(0, this.queuedBytes - bytes);
      if (!ok && this.isCurrent(token, socket)) {
        void this.fail('豆包语音音频发送失败。', token);
      }
      return ok;
    }).catch((): boolean => {
      this.queuedBytes = Math.max(0, this.queuedBytes - bytes);
      if (this.isCurrent(token, socket)) void this.fail('豆包语音音频发送失败。', token);
      return false;
    });
  }

  private async finishFrame(): Promise<void> {
    await this.releaseCapturer();
    const socket: webSocket.WebSocket | undefined = this.socket;
    if (socket === undefined || this.settled || !this.connected) return;
    const token: number = this.sessionToken;
    const pending: ArrayBuffer = this.pending.buffer.slice(
      this.pending.byteOffset, this.pending.byteOffset + this.pending.byteLength) as ArrayBuffer;
    this.pending = new Uint8Array(0);
    const finalFrame: ArrayBuffer = client.doubaoEncodeFrame(2, 3, -this.sequence, pending);
    this.sequence += 1;
    const sent: boolean = await this.sendChain;
    if (!this.isCurrent(token, socket) || this.settled) return;
    if (!sent || !await socket.send(finalFrame)) {
      await this.fail('豆包语音识别结束帧发送失败。', token);
    }
  }

  private receive(value: string | ArrayBuffer, token: number, socket: webSocket.WebSocket): void {
    if (!this.isCurrent(token, socket)) return;
    if (this.settled || typeof value === 'string') {
      if (typeof value === 'string') void this.fail('豆包返回了无效的二进制响应。', token);
      return;
    }
    const decoded: DoubaoFrameResult | null = client.doubaoDecodeFrame(value);
    if (decoded === null || decoded.payload.length > 1024 * 1024) {
      void this.fail('豆包返回了无效响应。', token);
      return;
    }
    const outcome: VoiceOutcome = VoiceResponsePolicy.streamingFrame(decoded.payload, decoded.last);
    if (outcome.failure.length > 0) {
      void this.fail(outcome.failure, token);
      return;
    }
    if (outcome.text.length > 0 && this.resultHandler !== undefined) {
      this.resultHandler(outcome.text, outcome.last);
    }
    if (outcome.last) {
      // A final frame with no text still ends the recording; leaving it would keep listening.
      if (outcome.text.length === 0 && this.resultHandler !== undefined) {
        this.resultHandler('', true);
      }
      void this.complete(token, socket);
    }
  }

  private async complete(token: number, activeSocket: webSocket.WebSocket): Promise<void> {
    if (!this.isCurrent(token, activeSocket) || this.settled) return;
    this.settled = true;
    this.connected = false;
    this.clearTimeout();
    await this.releaseCapturer();
    const socket: webSocket.WebSocket | undefined = this.socket;
    this.socket = undefined;
    if (socket !== undefined) await socket.close().catch((): boolean => false);
    this.resultHandler = undefined;
    this.errorHandler = undefined;
  }

  private async fail(message: string, token: number): Promise<void> {
    if (token !== this.sessionToken || this.settled) return;
    this.settled = true;
    this.connected = false;
    this.clearTimeout();
    await this.releaseCapturer();
    const socket: webSocket.WebSocket | undefined = this.socket;
    this.socket = undefined;
    if (socket !== undefined) await socket.close().catch((): boolean => false);
    const handler: DoubaoErrorHandler | undefined = this.errorHandler;
    this.resultHandler = undefined;
    this.errorHandler = undefined;
    if (handler !== undefined) handler(message);
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
      KeyboardLog.error(`Harmony 音频停止失败：${describeError(error)}`);
    }
    try {
      await capturer.release();
    } catch (error) {
      KeyboardLog.error(`Harmony 音频释放失败：${describeError(error)}`);
    }
  }

  private headers(config: VoiceInputConfiguration): webSocket.WebSocketRequestOptions {
    const requestId: string = util.generateRandomUUID(false);
    if (config.doubao_auth_mode === 'legacy') {
      return { header: { 'x-api-resource-id': config.asr_resource_id, 'x-api-request-id': requestId,
        'x-api-app-key': config.asr_app_key, 'x-api-access-key': config.asr_token } };
    }
    return { header: { 'x-api-resource-id': config.asr_resource_id, 'x-api-request-id': requestId,
      'x-api-key': config.asr_token } };
  }

  private validConfig(config: VoiceInputConfiguration): boolean {
    return TextPolicy.validAuthority(config.asr_endpoint, ['wss://'])
      && config.asr_token.length > 0 && config.asr_token.length <= 8192
      && config.asr_resource_id.length > 0 && config.asr_resource_id.length <= 256
      && (config.doubao_auth_mode !== 'legacy' || config.asr_app_key.length > 0);
  }

  private clearTimeout(): void {
    if (this.timeoutHandle !== undefined) clearTimeout(this.timeoutHandle);
    this.timeoutHandle = undefined;
  }

  private isCurrent(token: number, socket: webSocket.WebSocket): boolean {
    return token === this.sessionToken && this.socket === socket;
  }
}
