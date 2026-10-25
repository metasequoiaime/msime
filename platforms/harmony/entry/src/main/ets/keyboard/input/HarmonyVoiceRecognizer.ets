import { speechRecognizer } from '@kit.CoreSpeechKit';
import { VoiceRecognitionPolicy } from './VoiceRecognitionPolicy';
import { HarmonyDoubaoRecognizer } from './HarmonyDoubaoRecognizer';
import { HarmonyHttpAsrRecognizer } from './HarmonyHttpAsrRecognizer';
import { HarmonyVoicePolisher } from './HarmonyVoicePolisher';
import { DoubaoCredentialPolicy, VoiceInputConfiguration } from './VoiceInputConfiguration';
import { HttpAsrConfigurationPolicy } from './HttpAsrConfigurationPolicy';
import { HarmonyLocalAsrRecognizer, LocalAsrSpeechHandler } from './HarmonyLocalAsrRecognizer';
import { HarmonyPcmCapture } from './HarmonyPcmCapture';
import { LocalAsrPolicy, PcmFrameSlicer, SpeechSentenceAccumulator } from './LocalAsrPolicy';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { VoiceSessionPolicy } from './VoiceSessionPolicy';

export type VoiceResultHandler = (text: string, final: boolean) => void;
export type VoiceErrorHandler = (message: string) => void;

/**
 * Harmony's native speech boundary. For the `system` provider the microphone is captured here and written to CoreSpeechKit in its write-audio mode (`recognitionMode: 0`); the `local` provider runs an installed sherpa-onnx model on the device. No raw microphone data is copied into the Engine, written to disk, or included in logs. The recognizer is recreated only after shutdown, and callbacks are accepted only for the active bounded session.
 */
export class HarmonyVoiceRecognizer {
  private engine: speechRecognizer.SpeechRecognitionEngine | undefined = undefined;
  private engineLanguage: string = '';
  private sessionId: string = '';
  private generation: number = 0;
  private resultHandler: VoiceResultHandler | undefined = undefined;
  private errorHandler: VoiceErrorHandler | undefined = undefined;
  private readonly doubao: HarmonyDoubaoRecognizer = new HarmonyDoubaoRecognizer();
  private readonly httpAsr: HarmonyHttpAsrRecognizer = new HarmonyHttpAsrRecognizer();
  private readonly polisher: HarmonyVoicePolisher = new HarmonyVoicePolisher();
  private readonly local: HarmonyLocalAsrRecognizer = new HarmonyLocalAsrRecognizer();
  private readonly capture: HarmonyPcmCapture = new HarmonyPcmCapture();
  private readonly slicer: PcmFrameSlicer = new PcmFrameSlicer();
  private readonly sentences: SpeechSentenceAccumulator = new SpeechSentenceAccumulator();
  private polishGeneration: number = 0;
  private lastResultText: string = '';
  private finalDelivered: boolean = false;

  /**
   * `hotwordOptions` 是已准备好的宿主选项，`local` 提供方用它读取用户自己的词作热词，为 null 时跳过。`onSpeech` 只有 `local` 提供方中经过 VAD 的整段模型会调用，报告此刻是否听到说话。
   */
  async start(language: string, config: VoiceInputConfiguration, onResult: VoiceResultHandler,
    onError: VoiceErrorHandler, hotwordOptions: Object | null = null,
    onSpeech: LocalAsrSpeechHandler | undefined = undefined): Promise<void> {
    try {
      // 一项凭据都没填的豆包（出厂配置）照旧交给系统识别；填过的按豆包走，缺项直接报出来。
      const doubaoProvider: boolean = DoubaoCredentialPolicy.attempted(config);
      const doubaoMissing: string | null = DoubaoCredentialPolicy.missing(config);
      if (doubaoMissing !== null) throw new Error(doubaoMissing);
      const httpProvider: boolean = HttpAsrConfigurationPolicy.supported(config.asr_provider);
      if (doubaoProvider && this.doubao.isActive()) throw new Error('豆包语音识别正在进行');
      if (httpProvider && this.httpAsr.isActive()) throw new Error('语音识别正在进行');
      const localProvider: boolean = config.asr_provider === 'local';
      if (localProvider && this.local.isActive()) throw new Error('语音识别正在进行');
      if (localProvider && !LocalAsrPolicy.usesLocalModel(
        config.asr_provider, config.asr_model_path, config.local_model_root)) {
        throw new Error('请先在设置中下载并选择本地语音模型');
      }
      // 各识别器共用麦克风，键盘只能有一个录音会话；必须检查全部提供方，避免配置切换时旧会话仍在上传或解码，而新提供方又开始录音。
      if (!VoiceSessionPolicy.canStart(this.doubao.isActive(), this.httpAsr.isActive(),
        this.local.isActive(), this.capture.isActive(),
        this.engine !== undefined && this.engine.isBusy())) {
        throw new Error('语音识别正在进行');
      }
      if (config.asr_provider !== 'doubao' && !httpProvider && !localProvider && config.asr_provider !== 'system'
        && config.asr_provider.length > 0) {
        throw new Error('当前 Harmony 尚未支持该云端语音服务');
      }
      if (this.engine !== undefined && this.engine.isBusy()) {
        throw new Error('语音识别正在进行');
      }
      this.resultHandler = onResult;
      this.errorHandler = onError;
      this.lastResultText = '';
      this.finalDelivered = false;
      this.polishGeneration += 1;
      const deliver: VoiceResultHandler = (text: string, final: boolean): void => {
        this.deliver(config, text, final);
      };
      if (doubaoProvider) {
        await this.doubao.start(config, deliver, onError);
        return;
      }
      if (httpProvider) {
        await this.httpAsr.start(config, deliver, onError);
        return;
      }
      if (localProvider) {
        await this.local.start(config, hotwordOptions, deliver, onError, onSpeech);
        return;
      }
      this.generation = (this.generation + 1) % 1000000000;
      this.sessionId = VoiceRecognitionPolicy.sessionId(this.generation);
      const session: string = this.sessionId;
      const requestedLanguage: string = VoiceRecognitionPolicy.language(language);
      if (this.engine !== undefined && VoiceRecognitionPolicy.engineLanguageChanged(
        this.engineLanguage, requestedLanguage)) {
        try {
          this.engine.shutdown();
        } catch (error) {
          // 框架可能已经释放了空闲引擎。
        }
        this.engine = undefined;
        this.engineLanguage = '';
      }
      if (this.engine === undefined) {
        const created: speechRecognizer.SpeechRecognitionEngine =
          await speechRecognizer.createEngine({
          language: requestedLanguage,
          // 鸿蒙离线识别使用平台专有的 `CoreSpeechKit`。
          online: 1,
          extraParams: { recognizerMode: 'short' }
        });
        // 系统仍在创建引擎时也可能调用 `shutdown()`；销毁后不再安装迟到的引擎，避免继续占用麦克风服务。
        if (session !== this.sessionId || this.resultHandler === undefined) {
          try {
            created.shutdown();
          } catch (error) {
            // 引擎创建尚未完成时，系统可能已将它释放。
          }
          return;
        }
        this.engine = created;
        this.engineLanguage = requestedLanguage;
        this.engine.setListener({
          onStart: (): void => {},
          onEvent: (): void => {},
          onResult: (session: string, result: speechRecognizer.SpeechRecognitionResult): void => {
            if (session !== this.sessionId || this.resultHandler === undefined) {
              return;
            }
            // `isFinal` 只结束一句，`isLast` 才结束整个会话；此前各句继续保留并拼接到结果前。
            const whole: string = this.sentences.accept(VoiceRecognitionPolicy.result(result.result), result.isFinal);
            this.deliver(config, whole, result.isLast);
          },
          onComplete: (session: string): void => {
            if (session !== this.sessionId) return;
            // `CoreSpeechKit` 可能在未调用 `stop()` 时完成会话；返回结果前先释放采集器并使会话失效，避免回调再次取消已结束的会话。
            this.finishSystemSession(session);
            this.deliver(config, this.lastResultText, true);
          },
          onError: (session: string, code: number, message: string): void => {
            if (session !== this.sessionId) return;
            // `CoreSpeechKit` 错误会终止写音频会话；先停止采集器再通知视图，避免视图已空闲但识别器仍占用麦克风，导致后续启动一直忙碌。
            const handler: VoiceErrorHandler | undefined = this.errorHandler;
            this.cancelSystemSession();
            this.resultHandler = undefined;
            this.errorHandler = undefined;
            if (handler !== undefined) handler(`语音识别失败（${code}）`);
          }
        });
      }
      this.sentences.reset();
      this.slicer.reset();
      if (session !== this.sessionId || this.engine === undefined || this.resultHandler === undefined) {
        return;
      }
      // 模式 `0` 为写音频模式：识别器不自行打开麦克风，只接收 `writeAudio` 提交的音频。
      this.engine.startListening({
        sessionId: session,
        audioInfo: {
          audioType: 'pcm', sampleRate: 16000, soundChannel: 1, sampleBit: 16
        },
        extraParams: { recognitionMode: 0, recognizerMode: 'short' }
      });
      try {
        await this.capture.start(config.capture_backend, config.capture_device, (data: ArrayBuffer): void => {
          this.writeSystemAudio(session, data);
        });
      } catch (error) {
        if (session !== this.sessionId) return;
        KeyboardLog.error(`系统语音采集启动失败：${describeError(error)}`);
        this.cancelSystemSession();
        throw new Error('语音采集启动失败');
      }
    } catch (error) {
      KeyboardLog.error('语音识别启动失败');
      throw error as Error;
    }
  }

  stop(): void {
    if (this.doubao.isActive()) {
      this.doubao.stop();
      return;
    }
    if (this.httpAsr.isActive()) {
      this.httpAsr.stop();
      return;
    }
    if (this.local.isActive()) {
      this.local.stop();
      return;
    }
    if (this.engine !== undefined && this.sessionId.length > 0) {
      const session: string = this.sessionId;
      void this.capture.stop().then((): void => {
        if (session !== this.sessionId || this.engine === undefined) return;
        // The last partial frame is padded with silence, since writeAudio accepts only whole frames.
        const tail: Uint8Array | null = this.slicer.flush();
        if (tail !== null) this.engine.writeAudio(session, tail);
        this.engine.finish(session);
      });
    }
  }

  cancel(): void {
    this.polishGeneration += 1;
    this.polisher.cancel();
    this.doubao.cancel();
    this.httpAsr.cancel();
    this.local.cancel();
    this.cancelSystemSession();
    this.lastResultText = '';
    this.finalDelivered = false;
  }

  shutdown(): void {
    this.polishGeneration += 1;
    this.polisher.cancel();
    this.doubao.shutdown();
    this.httpAsr.shutdown();
    this.local.shutdown();
    try {
      this.cancel();
    } catch (error) {
      // A recognizer that already completed rejects cancel; shutdown must still release it.
      this.sessionId = '';
    }
    if (this.engine !== undefined) {
      try {
        this.engine.shutdown();
      } catch (error) {
        // The service may already have released a failed engine.
      }
      this.engine = undefined;
      this.engineLanguage = '';
    }
    this.resultHandler = undefined;
    this.errorHandler = undefined;
  }

  private writeSystemAudio(session: string, data: ArrayBuffer): void {
    try {
      if (session !== this.sessionId || this.engine === undefined) return;
      for (const frame of this.slicer.push(data)) {
        this.engine.writeAudio(session, frame);
      }
    } catch (error) {
      KeyboardLog.error('keyboard.input.HarmonyVoiceRecognizer.writeSystemAudio failed');
      throw error as Error;
    }
  }

  private cancelSystemSession(): void {
    try {
      void this.capture.stop();
      this.slicer.reset();
      this.sentences.reset();
      const session: string = this.sessionId;
      this.sessionId = '';
      if (this.engine !== undefined && session.length > 0 && this.engine.isBusy()) {
        this.engine.cancel(session);
      }
    } catch (error) {
      KeyboardLog.error('keyboard.input.HarmonyVoiceRecognizer.cancelSystemSession failed');
      throw error as Error;
    }
  }

  private finishSystemSession(session: string): void {
    if (session !== this.sessionId) return;
    void this.capture.stop();
    this.slicer.reset();
    this.sentences.reset();
    this.sessionId = '';
  }

  private deliver(config: VoiceInputConfiguration, text: string, final: boolean): void {
    const handler: VoiceResultHandler | undefined = this.resultHandler;
    if (handler === undefined) return;
    const normalized: string = VoiceRecognitionPolicy.result(text);
    if (!final) {
      if (normalized.length > 0) this.lastResultText = normalized;
      handler(normalized, false);
      return;
    }
    if (this.finalDelivered) return;
    this.finalDelivered = true;
    const finalText: string = normalized.length > 0 ? normalized : this.lastResultText;
    if (finalText.length === 0 || (!config.polish_enabled && !config.polish_text)) {
      handler(finalText, true);
      return;
    }
    const polishGeneration: number = ++this.polishGeneration;
    void this.polisher.polish(config, finalText).then((polished: string | null): void => {
      if (polishGeneration !== this.polishGeneration || this.resultHandler === undefined) return;
      handler(polished ?? finalText, true);
    }).catch((): void => {
      if (polishGeneration === this.polishGeneration && this.resultHandler !== undefined) {
        handler(finalText, true);
      }
    });
  }
}
