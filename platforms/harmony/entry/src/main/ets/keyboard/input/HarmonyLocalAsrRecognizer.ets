import { MessageEvents, worker } from '@kit.ArkTS';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { HarmonyPcmCapture, PcmChunkHandler } from './HarmonyPcmCapture';
import { LocalAsrPolicy, LocalAsrReply, LocalAsrRequest, LocalHotword } from './LocalAsrPolicy';
import { VoiceInputConfiguration } from './VoiceInputConfiguration';

export type LocalAsrResultHandler = (text: string, final: boolean) => void;
export type LocalAsrErrorHandler = (message: string) => void;
/** 经过 VAD 的整段模型：是否正在听到说话，只在变化时调用。 */
export type LocalAsrSpeechHandler = (speaking: boolean) => void;

const WORKER_SCRIPT: string = 'entry/ets/workers/LocalAsrWorker.ets';
const MAX_DURATION_MS: number = 120000;
/** How long a loaded model stays resident after a session; the worker is terminated after that, which is what returns the memory. */
const IDLE_RELEASE_MS: number = 120000;
const HOTWORD_LIMIT: number = 200;

interface HotwordsRequest {
  options: Object;
  limit: number;
}

interface HotwordsValue {
  hotwords: LocalHotword[];
}

interface HotwordsResponse {
  ok: boolean;
  value?: HotwordsValue;
  error?: string;
}

interface CorrectRequest {
  text: string;
  hotwords: LocalHotword[];
}

interface CorrectValue {
  text: string;
}

interface CorrectResponse {
  ok: boolean;
  value?: CorrectValue;
  error?: string;
}

interface NormalizeNumbersRequest {
  text: string;
  kind: string;
}

/**
 * 经 sherpa-onnx 的设备端听写：麦克风在这里采集，模型在 `workers/LocalAsrWorker.ets` 里运行，什么都不离开设备。
 *
 * 模型目录来自 `voice_input.asr_model_path`，用户在共享设置页选中已安装的模型时写入。会话开始时从宿主接口取用户自己的拼音词：原生吃热词的模型在加载时拿到它们，清单写 `pinyin` 的模型的最终文本经 `msime_client_voice_hotword_correct` 纠正。最终文本随后经 `msime_client_voice_normalize_numbers` 按模型 kind 把汉字数字改写成数字（只改写不自带 ITN 的 transducer），中间结果不改写。
 */
export class HarmonyLocalAsrRecognizer {
  private readonly capture: HarmonyPcmCapture = new HarmonyPcmCapture();
  private worker: worker.ThreadWorker | undefined = undefined;
  private sessionId: number = 0;
  private active: boolean = false;
  private finishing: boolean = false;
  /** Whether this session's `start` has reached the worker; until it has, a `finish` would arrive before the session it ends. */
  private requestPosted: boolean = false;
  private hotwords: LocalHotword[] = [];
  private resultHandler: LocalAsrResultHandler | undefined = undefined;
  private errorHandler: LocalAsrErrorHandler | undefined = undefined;
  private speechHandler: LocalAsrSpeechHandler | undefined = undefined;
  private timeoutHandle: number | undefined = undefined;
  private idleHandle: number | undefined = undefined;

  isActive(): boolean {
    return this.active;
  }

  /**
   * 开始一次听写。`hotwordOptions` 是词库调用要用的已准备好的宿主选项，为 null 时跳过热词而不是让会话失败。`onSpeech` 收到 VAD 说话状态的变化，只有经过 VAD 的整段模型会报。
   */
  async start(config: VoiceInputConfiguration, hotwordOptions: Object | null, onResult: LocalAsrResultHandler,
    onError: LocalAsrErrorHandler, onSpeech: LocalAsrSpeechHandler | undefined = undefined,
    onChunk: PcmChunkHandler | undefined = undefined): Promise<void> {
    try {
      if (this.active) throw new Error('语音识别正在进行');
      const directory: string = LocalAsrPolicy.modelUnderRoot(
        config.asr_model_path, config.local_model_root);
      if (directory.length === 0) throw new Error('请先在设置中下载并选择本地语音模型');
      const id: number = this.sessionId + 1;
      this.sessionId = id;
      this.active = true;
      this.finishing = false;
      this.requestPosted = false;
      this.hotwords = [];
      this.resultHandler = onResult;
      this.errorHandler = onError;
      this.speechHandler = onSpeech;
      this.clearIdle();
      const recognizer: worker.ThreadWorker = this.ensureWorker();
      this.hotwords = await this.fetchHotwords(hotwordOptions);
      if (id !== this.sessionId) return;
      const request: LocalAsrRequest = {
        type: 'start', id: id, directory: directory, root: config.local_model_root ?? '',
        language: config.language ?? '',
        hotwords: this.hotwords.map((word: LocalHotword): string => word.text), threads: 0, pcm: null,
        scratch: config.local_model_scratch ?? '', stopSensitivity: config.local_stop_sensitivity ?? 0
      };
      recognizer.postMessage(request);
      this.requestPosted = true;
      if (this.finishing) {
        // 热词读取期间已被释放，尚未录音，直接结束会话。
        this.postFinish(id);
        return;
      }
      this.timeoutHandle = setTimeout((): void => {
        if (id === this.sessionId && this.active) this.stop();
      }, MAX_DURATION_MS);
      try {
        await this.capture.start(config.capture_backend, config.capture_device, (data: ArrayBuffer): void => {
          // 电平只在本次会话还在录音时报，与 `sendAudio` 的判断一致；旁听者不保存也不改动数据。
          if (onChunk !== undefined && id === this.sessionId && !this.finishing) onChunk(data);
          this.sendAudio(id, data);
        });
      } catch (error) {
        if (id !== this.sessionId) return;
        this.fail(id, `语音采集启动失败：${describeError(error)}`);
        throw new Error('语音采集启动失败');
      }
    } catch (error) {
      KeyboardLog.error('keyboard.input.HarmonyLocalAsrRecognizer.start failed');
      throw error as Error;
    }
  }

  /** Ends the recording; the final text arrives through the result handler once the worker has decoded the tail. */
  stop(): void {
    if (!this.active || this.finishing) return;
    this.finishing = true;
    this.clearMaxDuration();
    const id: number = this.sessionId;
    void this.capture.stop().then((): void => {
      if (id === this.sessionId && this.requestPosted) this.postFinish(id);
    });
  }

  cancel(): void {
    const id: number = this.sessionId;
    this.sessionId = id + 1;
    this.clearMaxDuration();
    void this.capture.stop();
    // The worker runs sherpa synchronously while decoding an offline VAD segment. Its message
    // queue cannot deliver the `cancel` request until that decode returns, so posting a cancel
    // alone leaves the model and CPU alive behind a dismissed keyboard. Terminate the worker now;
    // stale replies are rejected by the session id and the next dictation creates a clean worker.
    if (this.active) this.terminateWorker();
    this.settle();
  }

  shutdown(): void {
    this.cancel();
    this.clearIdle();
    this.terminateWorker();
  }

  private postFinish(id: number): void {
    try {
      if (this.worker === undefined) return;
      const request: LocalAsrRequest = {
        type: 'finish', id: id, directory: '', root: '', language: '', hotwords: [], threads: 0, pcm: null,
        scratch: '', stopSensitivity: 0
      };
      this.worker.postMessage(request);
    } catch (error) {
      KeyboardLog.error('keyboard.input.HarmonyLocalAsrRecognizer.postFinish failed');
      this.fail(id, '本地语音识别结束失败');
    }
  }

  private ensureWorker(): worker.ThreadWorker {
    if (this.worker !== undefined) return this.worker;
    const created: worker.ThreadWorker = new worker.ThreadWorker(WORKER_SCRIPT);
    created.onmessage = (event: MessageEvents): void => {
      this.handle(event.data as LocalAsrReply);
    };
    created.onerror = (): void => {
      // A worker that died took its model with it; the next session starts a fresh one.
      if (this.worker === created) this.worker = undefined;
      if (this.active) this.fail(this.sessionId, '本地语音识别失败');
    };
    this.worker = created;
    return created;
  }

  private sendAudio(id: number, data: ArrayBuffer): void {
    try {
      if (id !== this.sessionId || this.finishing || this.worker === undefined) return;
      // 采集器可能复用缓冲区，先复制再把副本转移给工作线程，避免再次复制。
      const copy: ArrayBuffer = data.slice(0);
      const request: LocalAsrRequest = {
        type: 'audio', id: id, directory: '', root: '', language: '', hotwords: [], threads: 0, pcm: copy,
        scratch: '', stopSensitivity: 0
      };
      this.worker.postMessage(request, [copy]);
    } catch (error) {
      KeyboardLog.error('keyboard.input.HarmonyLocalAsrRecognizer.sendAudio failed');
      this.fail(id, '本地语音音频传递失败');
    }
  }

  private handle(reply: LocalAsrReply): void {
    // Id 0 is a failure the worker could not attribute to a request; it belongs to whatever is running.
    const id: number = reply.id === 0 ? this.sessionId : reply.id;
    if (id !== this.sessionId || !this.active) return;
    if (reply.type === 'started') {
      // sherpa-onnx 在工作线程首次建模时加载原生库，此时刷新崩溃记录器的模块快照。
      try { client.telemetryNativeRefreshModules(); } catch (error) {
        KeyboardLog.warn(`崩溃模块快照刷新失败：${describeError(error)}`);
      }
      return;
    }
    if (reply.type === 'partial') {
      const handler: LocalAsrResultHandler | undefined = this.resultHandler;
      if (handler !== undefined && !this.finishing) handler(reply.text, false);
      return;
    }
    if (reply.type === 'speech') {
      const speech: LocalAsrSpeechHandler | undefined = this.speechHandler;
      if (speech !== undefined && !this.finishing) speech(reply.speaking === true);
      return;
    }
    if (reply.type === 'final') {
      const corrected: string = reply.correct ? this.correct(reply.text) : reply.text;
      const text: string = this.normalizeNumbers(corrected, reply.kind);
      const handler: LocalAsrResultHandler | undefined = this.resultHandler;
      this.settle();
      if (handler !== undefined) handler(text, true);
      return;
    }
    if (reply.type === 'error') {
      this.fail(id, reply.text.length > 0 ? reply.text : '本地语音识别失败');
    }
  }

  private fail(id: number, message: string): void {
    if (id !== this.sessionId || !this.active) return;
    this.sessionId = id + 1;
    this.clearMaxDuration();
    void this.capture.stop();
    this.terminateWorker();
    const handler: LocalAsrErrorHandler | undefined = this.errorHandler;
    this.settle();
    if (handler !== undefined) handler(message);
  }

  private settle(): void {
    this.active = false;
    this.finishing = false;
    this.requestPosted = false;
    this.hotwords = [];
    this.resultHandler = undefined;
    this.errorHandler = undefined;
    this.speechHandler = undefined;
    this.scheduleIdle();
  }

  /** The user's own pinyin words, read on a native worker thread. Any failure means no hotwords, never no dictation. */
  private async fetchHotwords(options: Object | null): Promise<LocalHotword[]> {
    if (options === null) return [];
    try {
      const request: HotwordsRequest = { options: options, limit: HOTWORD_LIMIT };
      const response: HotwordsResponse =
        JSON.parse(await client.voiceHotwords(JSON.stringify(request))) as HotwordsResponse;
      if (response.ok !== true || response.value === undefined || response.value === null) {
        KeyboardLog.warn(`voice hotwords unavailable: ${response.error ?? 'unknown'}`);
        return [];
      }
      const hotwords: LocalHotword[] | null = LocalAsrPolicy.hotwords(response.value.hotwords);
      return hotwords ?? [];
    } catch (error) {
      KeyboardLog.warn(`voice hotwords unavailable: ${describeError(error)}`);
      return [];
    }
  }

  private correct(text: string): string {
    if (text.length === 0 || this.hotwords.length === 0) return text;
    try {
      const request: CorrectRequest = { text: text, hotwords: this.hotwords };
      const response: CorrectResponse =
        JSON.parse(client.voiceHotwordCorrect(JSON.stringify(request))) as CorrectResponse;
      return response.ok === true ? LocalAsrPolicy.correctedText(response.value, text) : text;
    } catch (error) {
      KeyboardLog.warn(`voice hotword correction unavailable: ${describeError(error)}`);
      return text;
    }
  }

  /** 汉字数字改写成数字；kind 不需要改写时原生层原样返回，任何失败都保留原文。 */
  private normalizeNumbers(text: string, kind: string): string {
    if (text.length === 0 || kind.length === 0) return text;
    try {
      const request: NormalizeNumbersRequest = { text: text, kind: kind };
      const response: CorrectResponse =
        JSON.parse(client.voiceNormalizeNumbers(JSON.stringify(request))) as CorrectResponse;
      return response.ok === true ? LocalAsrPolicy.correctedText(response.value, text) : text;
    } catch (error) {
      KeyboardLog.warn(`voice number normalization unavailable: ${describeError(error)}`);
      return text;
    }
  }

  private scheduleIdle(): void {
    this.clearIdle();
    if (this.worker === undefined) return;
    this.idleHandle = setTimeout((): void => {
      this.idleHandle = undefined;
      if (!this.active) this.terminateWorker();
    }, IDLE_RELEASE_MS);
  }

  private terminateWorker(): void {
    const running: worker.ThreadWorker | undefined = this.worker;
    this.worker = undefined;
    if (running === undefined) return;
    try {
      running.terminate();
    } catch (error) {
      KeyboardLog.warn(`local voice worker did not terminate: ${describeError(error)}`);
    }
  }

  private clearMaxDuration(): void {
    if (this.timeoutHandle !== undefined) clearTimeout(this.timeoutHandle);
    this.timeoutHandle = undefined;
  }

  private clearIdle(): void {
    if (this.idleHandle !== undefined) clearTimeout(this.idleHandle);
    this.idleHandle = undefined;
  }
}
