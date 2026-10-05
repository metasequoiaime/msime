import { MessageEvents, worker } from '@kit.ArkTS';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { HarmonyPcmCapture } from './HarmonyPcmCapture';
import { LocalAsrPolicy, LocalAsrReply, LocalAsrRequest } from './LocalAsrPolicy';
import { VoiceInputConfiguration } from './VoiceInputConfiguration';

export type LocalAsrResultHandler = (text: string, final: boolean) => void;
export type LocalAsrErrorHandler = (message: string) => void;

const WORKER_SCRIPT: string = 'entry/ets/workers/LocalAsrWorker.ets';
const MAX_DURATION_MS: number = 120000;
/** How long a loaded model stays resident after a session; the worker is terminated after that, which is what returns the memory. */
const IDLE_RELEASE_MS: number = 120000;
const HOTWORD_LIMIT: number = 200;

/** A user dictionary word as `msime_client_voice_hotwords` returns it. */
export interface LocalHotword {
  text: string;
  pinyin: string;
}

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

/**
 * On-device dictation through sherpa-onnx: the microphone is captured here, the model runs in `workers/LocalAsrWorker.ets`, and nothing leaves the device.
 *
 * The model directory comes from `voice_input.asr_model_path`, which the shared settings page sets when the user picks an installed model. The user's own pinyin words are fetched from the host API at session start; a model that takes hotwords natively gets them at load, and one whose manifest says `pinyin` has its final text corrected through `msime_client_voice_hotword_correct`.
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
  private timeoutHandle: number | undefined = undefined;
  private idleHandle: number | undefined = undefined;

  isActive(): boolean {
    return this.active;
  }

  /**
   * Starts one dictation. `hotwordOptions` are the prepared host options the dictionary calls take; null skips hotwords rather than failing the session.
   */
  async start(config: VoiceInputConfiguration, hotwordOptions: Object | null, onResult: LocalAsrResultHandler,
    onError: LocalAsrErrorHandler): Promise<void> {
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
    this.clearIdle();
    const recognizer: worker.ThreadWorker = this.ensureWorker();
    this.hotwords = await this.fetchHotwords(hotwordOptions);
    if (id !== this.sessionId) return;
    const request: LocalAsrRequest = {
      type: 'start', id: id, directory: directory, root: config.local_model_root ?? '',
      language: config.language ?? '',
      hotwords: this.hotwords.map((word: LocalHotword): string => word.text), threads: 0, pcm: null
    };
    recognizer.postMessage(request);
    this.requestPosted = true;
    if (this.finishing) {
      // Released while the hotwords were still being read: nothing was recorded, so the session ends at once.
      this.postFinish(id);
      return;
    }
    this.timeoutHandle = setTimeout((): void => {
      if (id === this.sessionId && this.active) this.stop();
    }, MAX_DURATION_MS);
    try {
      await this.capture.start(config.capture_backend, config.capture_device, (data: ArrayBuffer): void => {
        this.sendAudio(id, data);
      });
    } catch (error) {
      if (id !== this.sessionId) return;
      this.fail(id, `语音采集启动失败：${describeError(error)}`);
      throw new Error('语音采集启动失败');
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
    if (this.worker === undefined) return;
    const request: LocalAsrRequest = {
      type: 'finish', id: id, directory: '', root: '', language: '', hotwords: [], threads: 0, pcm: null
    };
    this.worker.postMessage(request);
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
    if (id !== this.sessionId || this.finishing || this.worker === undefined) return;
    // The capturer may reuse its buffer, so a copy is what crosses to the worker, and it is transferred rather than copied again.
    const copy: ArrayBuffer = data.slice(0);
    const request: LocalAsrRequest = {
      type: 'audio', id: id, directory: '', root: '', language: '', hotwords: [], threads: 0, pcm: copy
    };
    this.worker.postMessage(request, [copy]);
  }

  private handle(reply: LocalAsrReply): void {
    // Id 0 is a failure the worker could not attribute to a request; it belongs to whatever is running.
    const id: number = reply.id === 0 ? this.sessionId : reply.id;
    if (id !== this.sessionId || !this.active) return;
    if (reply.type === 'partial') {
      const handler: LocalAsrResultHandler | undefined = this.resultHandler;
      if (handler !== undefined && !this.finishing) handler(reply.text, false);
      return;
    }
    if (reply.type === 'final') {
      const text: string = reply.correct ? this.correct(reply.text) : reply.text;
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
    this.scheduleIdle();
  }

  /** The user's own pinyin words, read on a native worker thread. Any failure means no hotwords, never no dictation. */
  private async fetchHotwords(options: Object | null): Promise<LocalHotword[]> {
    if (options === null) return [];
    try {
      const request: HotwordsRequest = { options: options, limit: HOTWORD_LIMIT };
      const response: HotwordsResponse =
        JSON.parse(await client.voiceHotwords(JSON.stringify(request))) as HotwordsResponse;
      if (!response.ok || response.value === undefined) {
        KeyboardLog.warn(`voice hotwords unavailable: ${response.error ?? 'unknown'}`);
        return [];
      }
      return response.value.hotwords;
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
      return response.ok && response.value !== undefined ? response.value.text : text;
    } catch (error) {
      KeyboardLog.warn(`voice hotword correction unavailable: ${describeError(error)}`);
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
