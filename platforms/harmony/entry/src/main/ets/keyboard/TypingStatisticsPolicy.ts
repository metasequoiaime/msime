/** Source labels shared by the native aggregate typing-statistics store. */
export class TypingStatisticsPolicy {
  static source(
    scheme: string,
    profile: string,
    english: boolean,
    nineKey: boolean,
    localMode: string,
  ): string {
    if (localMode === "temporary_japanese") return "japanese";
    if (localMode !== "none" && localMode.length > 0) return "local";
    if (english) return "english";
    if (scheme === "quanpin") return nineKey ? "nineKey" : "quanpin";
    if (scheme === "wubi") return "wubi";
    if (scheme === "japanese") return "japanese";
    if (scheme === "korean") return "korean";
    if (scheme === "cantonese") return "cantonese";
    if (scheme === "zhuyin") return "zhuyin";
    if (scheme === "vietnamese") return "vietnamese";
    if (scheme === "tibetan") return "tibetan";
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
