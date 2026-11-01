import { textToSpeech } from '@kit.CoreSpeechKit';
import { ReadAloudPolicy } from './ReadAloudPolicy';
import { KeyboardLog } from '../keyboard/KeyboardLog';

/**
 * 键盘长按菜单的「朗读」和设置应用背单词卡片的喇叭：用 Core Speech Kit 的 `textToSpeech` 读一个英文单词。
 *
 * 只用离线模式（`online: 1`）和随系统提供的中文声音聆小珊（`zh-CN`、`person` 13），它能读「中文语境下的英文」。英文的劳拉声音要先下载声音模型、会弹下载窗走网络，这里从不触发下载。引擎第一次朗读时才创建，之后一直用它；键盘和设置应用各在自己的进程里有一个。见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
 */
export class HarmonyReadAloud {
  private static engine: textToSpeech.TextToSpeechEngine | undefined = undefined;
  private static creating: Promise<textToSpeech.TextToSpeechEngine> | undefined = undefined;
  private static sequence: number = 0;
  // Bumped by every shutdown, so an engine whose createEngine was still pending at shutdown is closed on arrival instead of being kept.
  private static generation: number = 0;

  /** 这台设备有没有文本转语音能力。 */
  static available(): boolean {
    return canIUse('SystemCapability.AI.TextToSpeech');
  }

  /** 读 `text`（共享层 `msime_client_read_aloud_text` 答的英文）。读出来时得到 null，读不出来时得到给用户看的那句话。新的一次打断还在读的上一次。 */
  static async speak(text: string): Promise<string | null> {
    if (text.length === 0) {
      return null;
    }
    if (!HarmonyReadAloud.available()) {
      return ReadAloudPolicy.NO_ENGINE;
    }
    let engine: textToSpeech.TextToSpeechEngine;
    try {
      engine = await HarmonyReadAloud.ensure();
    } catch (error) {
      return ReadAloudPolicy.NO_ENGINE;
    }
    HarmonyReadAloud.sequence += 1;
    // 抢占模式（`queueMode` 1）：新点的词打断还在读的上一个，不排队。requestId 在同一个引擎里只能用一次。
    const extra: Record<string, Object> = {
      'queueMode': 1,
      'languageContext': 'zh-CN',
      'playType': 1,
    };
    const params: textToSpeech.SpeakParams = {
      requestId: `msime-read-aloud-${Date.now()}-${HarmonyReadAloud.sequence}`,
      extraParams: extra,
    };
    try {
      engine.speak(ReadAloudPolicy.spokenText(text), params);
      return null;
    } catch (error) {
      return ReadAloudPolicy.FAILED;
    }
  }

  /** 关掉引擎，正在读的也停下。键盘扩展或设置页销毁时调用；之后再 `speak` 会重新创建。 */
  static shutdown(): void {
    const engine: textToSpeech.TextToSpeechEngine | undefined = HarmonyReadAloud.engine;
    HarmonyReadAloud.engine = undefined;
    HarmonyReadAloud.creating = undefined;
    HarmonyReadAloud.generation += 1;
    if (engine !== undefined) {
      try {
        engine.shutdown();
      } catch (error) {
        // 引擎已经不在了。
      }
    }
  }

  private static ensure(): Promise<textToSpeech.TextToSpeechEngine> {
    const existing: textToSpeech.TextToSpeechEngine | undefined = HarmonyReadAloud.engine;
    if (existing !== undefined) {
      return Promise.resolve(existing);
    }
    const pending: Promise<textToSpeech.TextToSpeechEngine> | undefined = HarmonyReadAloud.creating;
    if (pending !== undefined) {
      return pending;
    }
    const params: textToSpeech.CreateEngineParams = {
      language: ReadAloudPolicy.ENGINE_LANGUAGE,
      person: ReadAloudPolicy.ENGINE_PERSON,
      online: ReadAloudPolicy.OFFLINE,
    };
    const generation: number = HarmonyReadAloud.generation;
    const created: Promise<textToSpeech.TextToSpeechEngine> = textToSpeech.createEngine(params)
      .then((engine: textToSpeech.TextToSpeechEngine): textToSpeech.TextToSpeechEngine => {
        if (generation !== HarmonyReadAloud.generation) {
          // The keyboard or the settings page went away while the engine was being created: close it rather than keep an engine nobody will shut down.
          try {
            engine.shutdown();
          } catch (error) {
            // Already gone.
          }
          throw new Error('read-aloud engine created after shutdown');
        }
        const listener: textToSpeech.SpeakListener = {
          onStart: (): void => {},
          onComplete: (): void => {},
          onStop: (): void => {},
          // Synthesis fails after speak() has already returned, so the user sees no notice for it; the log is what tells a device check why the word was silent.
          onError: (requestId: string, errorCode: number, errorMessage: string): void => {
            KeyboardLog.warn(`speech.HarmonyReadAloud ${requestId} failed: code ${errorCode}: ${errorMessage}`);
          },
        };
        engine.setListener(listener);
        HarmonyReadAloud.engine = engine;
        HarmonyReadAloud.creating = undefined;
        return engine;
      })
      .catch((error: Object): textToSpeech.TextToSpeechEngine => {
        if (generation === HarmonyReadAloud.generation) {
          HarmonyReadAloud.creating = undefined;
        }
        throw error as Error;
      });
    HarmonyReadAloud.creating = created;
    return created;
  }
}
