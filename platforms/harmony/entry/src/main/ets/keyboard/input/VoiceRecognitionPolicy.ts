import { TextPolicy } from "../TextPolicy";

/** Bounds and normalizes the text crossing the native speech boundary. */
export const VOICE_MAX_TEXT: number = 4096;
export const VOICE_MAX_LANGUAGE: number = 32;

export class VoiceRecognitionPolicy {
  /** 原生语音结果只有字符串才可交给编辑器，其他类型视为过期结果。 */
  static accepted(value: unknown): string | null {
    return typeof value === 'string' ? VoiceRecognitionPolicy.result(value as string) : null;
  }

  static language(value: string): string {
    const trimmed: string = value.trim();
    if (trimmed.length === 0) {
      return 'zh-CN';
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

  static result(value: string): string {
    if (!TextPolicy.validUnicode(value)) return '';
    let text: string = value.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, '');
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
