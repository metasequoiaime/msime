/**
 * 「朗读」在鸿蒙上的几项固定取值和提示，写成纯 TypeScript，`tests/run.sh` 测得到；出声在 `HarmonyReadAloud.ets`。
 */
export class ReadAloudPolicy {
  /** 菜单里这一项的标题。 */
  static readonly MENU_TITLE: string = "朗读";
  static readonly NO_ENGINE: string = "系统没有可用的朗读语音";
  static readonly FAILED: string = "无法朗读";
  /** 只用随系统提供的中文声音聆小珊：英文的劳拉声音要下载，从不触发下载。 */
  static readonly ENGINE_LANGUAGE: string = "zh-CN";
  static readonly ENGINE_PERSON: number = 13;
  /** `CreateEngineParams.online` 为 1 是离线模式。 */
  static readonly OFFLINE: number = 1;

  /**
   * 交给引擎的文字：前面加 `[h2]`，让引擎把它当成单词读，不逐个字母念。`text` 是共享层答的英文，只有字母、空格和 `-'.`，不会和引擎的其他标记冲突。
   */
  static spokenText(text: string): string {
    return `[h2]${text}`;
  }

  /** 交给 `msime_client_read_aloud_text` 的请求：候选原文和显示着的释义（没有时给空串，背单词卡片把词头当作候选）。 */
  static textRequest(candidate: string, gloss: string): string {
    return JSON.stringify({ candidate: candidate, gloss: gloss });
  }

  /** `msime_client_read_aloud_text` 的应答里要读的文字；没有能读的或应答不对时为 null。 */
  static textFromReply(raw: string): string | null {
    try {
      const reply: Record<string, Object> = JSON.parse(raw) as Record<string, Object>;
      const value: Object | undefined = reply["value"];
      return reply["ok"] === true && typeof value === "string" && value.length > 0 ? value : null;
    } catch (error) {
      return null;
    }
  }

  /** 设置页经桥接发来的 `read_aloud` 请求里要读的文字；不是 `{text: 非空字符串}` 时为 null。真正读之前设置桥还要按共享层的规则再核对一遍。 */
  static requestText(payload: string): string | null {
    try {
      const value: Object = JSON.parse(payload) as Object;
      const text: Object | undefined = (value as Record<string, Object>)["text"];
      return typeof text === "string" && text.length > 0 && text.length <= 64 ? text : null;
    } catch (error) {
      return null;
    }
  }
}
