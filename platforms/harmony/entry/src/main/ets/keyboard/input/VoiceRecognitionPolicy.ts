import { TextPolicy } from "../TextPolicy";

/** Bounds and normalizes the text crossing the native speech boundary. */
export const VOICE_MAX_TEXT: number = 4096;
export const VOICE_MAX_LANGUAGE: number = 32;
/** 结果直接进入编辑器后，聆听界面显示「已识别：…」的时长，即 Android 的 `ImeVoiceEntry.DONE_NOTICE_MILLIS`。 */
export const VOICE_DONE_NOTICE_MS: number = 1200;
/** 最后一次听到话语后多久触屏录音自行结束，即 Android 的 `ImeVoiceEntry.SILENCE_STOP_MILLIS`。 */
export const VOICE_SILENCE_STOP_MS: number = 1500;

/**
 * 开始聆听时编辑器所处的状态，只以代次保存，不持有任何编辑器文本：键盘挂到另一个输入框或重启时 `editor` 变化，编辑器报告文本或选区变化时 `context` 变化。
 */
export interface VoiceTarget {
  readonly editor: number;
  readonly context: number;
}

export class VoiceRecognitionPolicy {
  /** 原生语音结果只有字符串才可交给编辑器，其他类型视为过期结果。 */
  static accepted(value: unknown): string | null {
    return typeof value === "string" ? VoiceRecognitionPolicy.result(value as string) : null;
  }

  static language(value: string): string {
    const trimmed: string = value.trim();
    if (trimmed.length === 0) {
      return "zh-CN";
    }
    return trimmed.slice(0, VOICE_MAX_LANGUAGE);
  }

  static engineLanguageChanged(current: string, next: string): boolean {
    return VoiceRecognitionPolicy.language(current) !== VoiceRecognitionPolicy.language(next);
  }

  static sessionId(generation: number): string {
    const bounded: number = Math.max(1, Math.floor(generation)) % 1000000000;
    return `msime-voice-${bounded}`;
  }

  /**
   * 最终结果能否直接进入编辑器，即 Android 的 `ImeVoiceEntry.delivered` 规则：聆听可能持续一分钟，若期间用户移动了光标、应用改写了输入框或开始了组合输入，结果不会落到光标此刻所在的位置，而是留给用户显式插入。
   */
  static insertsDirectly(
    captured: VoiceTarget | null,
    current: VoiceTarget,
    composing: boolean,
  ): boolean {
    return (
      captured !== null &&
      !composing &&
      captured.editor === current.editor &&
      captured.context === current.context
    );
  }

  /**
   * 最终结果能否不经审阅界面直接上屏（之后还要过 `insertsDirectly` 的编辑框检查）：只有触屏上的流式录音、且语音界面仍开着时才行，这正是显示聆听界面的那种录音，与 Android 键盘内只对 LOCAL / STREAMING 识别器直接上屏一致。上传式服务商（OpenAI 兼容）的录音在审阅界面上用「停止录音」结束，结果也留在那里等「提交」；2in1 总是审阅。
   */
  static skipsReview(touch: boolean, streamed: boolean, voiceSurfaceOpen: boolean): boolean {
    return touch && streamed && voiceSurfaceOpen;
  }

  /**
   * 触屏键盘离开语音界面时是否放弃正在进行的录音，与 Android `ImeVoiceEntry` 在聆听视图移除时取消识别一致：聆听界面没有停止按钮，按键上也没有录音标识，继续录音会让麦克风在看不见的地方一直开着，结果还会留到下次打开语音时变成过期内容。2in1 的语音面板由 `SurfaceRoutingPolicy.endsVoice` 负责。
   */
  static cancelsOnLeave(touch: boolean, recording: boolean, leavingVoiceSurface: boolean): boolean {
    return touch && recording && leavingVoiceSurface;
  }

  static result(value: string): string {
    if (!TextPolicy.validUnicode(value)) return "";
    let text: string = value.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "");
    text = text.trim();
    let bounded: string = text.slice(0, VOICE_MAX_TEXT);
    // 避免按 UTF-16 截断时留下孤立的高代理项。
    if (
      bounded.length > 0 &&
      bounded.charCodeAt(bounded.length - 1) >= 0xd800 &&
      bounded.charCodeAt(bounded.length - 1) <= 0xdbff
    ) {
      bounded = bounded.slice(0, -1);
    }
    return bounded;
  }
}
