import { KeyGrid } from "./KeyboardScheme";

/** Source labels shared by the native aggregate typing-statistics store. */
export class TypingStatisticsPolicy {
  static isCurrentGeneration(requestGeneration: number, currentGeneration: number): boolean {
    return requestGeneration === currentGeneration;
  }

  static source(
    scheme: string,
    profile: string,
    english: boolean,
    keyGrid: number,
    localMode: string,
  ): string {
    if (localMode === "temporary_japanese") return "japanese";
    if (localMode !== "none" && localMode.length > 0) return "local";
    if (english) return "english";
    // 全拼按引擎当前的网格分九键、14 键和 26 键；`keyGrid` 取值见 `KeyGrid`。
    if (scheme === "quanpin") {
      if (keyGrid === KeyGrid.NINE_KEY) return "nineKey";
      return keyGrid === KeyGrid.FOURTEEN_KEY ? "fourteenKey" : "quanpin";
    }
    if (scheme === "wubi") return "wubi";
    if (scheme === "japanese") return "japanese";
    if (scheme === "korean") return "korean";
    if (scheme === "cantonese") return "cantonese";
    if (scheme === "zhuyin") return "zhuyin";
    if (scheme === "vietnamese") return "vietnamese";
    if (scheme === "tibetan") return "tibetan";
    if (scheme === "stroke") return "stroke";
    if (scheme === "shuangpin") {
      if (profile === "ziranma") return "ziranma";
      if (profile === "microsoft") return "microsoft";
      if (profile === "shoudao") return "shoudao";
      return "shuangpin";
    }
    return "unknown";
  }

  /**
   * Whether a character the keyboard released to the application counts as typed, the rule of Windows `ShouldCountPassthroughChar`: a printable character without Ctrl, Alt or the logo key. Shift is allowed, since it is how capitals and symbols are typed; control characters and DEL are editing, not text.
   */
  static countsPassthrough(character: number, ctrl: boolean, alt: boolean, logo: boolean): boolean {
    return !ctrl && !alt && !logo && character >= 0x20 && character !== 0x7f;
  }

  static day(date: Date): string {
    return `${String(date.getFullYear()).padStart(4, "0")}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  }

  /** Local hour, 0-23. Same calendar as `day`, so the two axes cannot disagree. */
  static hour(date: Date): number {
    return date.getHours();
  }
}
