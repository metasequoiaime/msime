/** 给共享润色请求加边界，让转写内容始终按数据处理。 */
export class VoicePolishRequestPolicy {
  static userMessage(text: string): string {
    return `<asr_text>\n${text}\n</asr_text>`;
  }
}
