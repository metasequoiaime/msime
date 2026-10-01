/**
 * Keyboard schemes the host exposes, ported from
 * platforms/android/java/app/msime/android/KeyboardScheme.java.
 *
 * Java spells this as an enum carrying fields. ArkTS enums hold only a value, so each scheme is a
 * frozen record and SCHEMES preserves the declaration order the Apple hosts also rely on.
 */
import { SchemeTraits } from "./SchemeTraits";

export interface SchemeDefinition {
  readonly id: string;
  readonly preferenceId: string;
  readonly engineScheme: string;
  readonly shuangpinProfile: string | null;
  readonly touchKeyboardLayout: string;
  readonly title: string;
  readonly glyph: string;
  readonly badge: string;
}

/** Complete preference values needed for one compare-and-swap update. */
export interface PreferenceMapping {
  readonly scheme: string;
  readonly lastChineseScheme: string;
  readonly shuangpinProfile: string;
  readonly touchKeyboardLayout: string;
}

const QUANPIN: SchemeDefinition = {
  id: "QUANPIN",
  preferenceId: "quanpin",
  engineScheme: "quanpin",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "全拼 26 键",
  glyph: "拼",
  badge: "26",
};
const QUANPIN_NINE_KEY: SchemeDefinition = {
  id: "QUANPIN_NINE_KEY",
  preferenceId: "nine_key",
  engineScheme: "quanpin",
  shuangpinProfile: null,
  touchKeyboardLayout: "nine_key",
  title: "全拼 9 键",
  glyph: "拼",
  badge: "9",
};
const XIAOHE: SchemeDefinition = {
  id: "XIAOHE",
  preferenceId: "xiaohe",
  engineScheme: "shuangpin",
  shuangpinProfile: "xiaohe",
  touchKeyboardLayout: "twenty_six_key",
  title: "小鹤双拼",
  glyph: "鹤",
  badge: "双",
};
const ZIRANMA: SchemeDefinition = {
  id: "ZIRANMA",
  preferenceId: "ziranma",
  engineScheme: "shuangpin",
  shuangpinProfile: "ziranma",
  touchKeyboardLayout: "twenty_six_key",
  title: "自然码双拼",
  glyph: "自",
  badge: "双",
};
const MICROSOFT: SchemeDefinition = {
  id: "MICROSOFT",
  preferenceId: "microsoft",
  engineScheme: "shuangpin",
  shuangpinProfile: "microsoft",
  touchKeyboardLayout: "twenty_six_key",
  title: "微软双拼",
  glyph: "微",
  badge: "双",
};
const SHOUDAO: SchemeDefinition = {
  id: "SHOUDAO",
  preferenceId: "shoudao",
  engineScheme: "shuangpin",
  shuangpinProfile: "shoudao",
  touchKeyboardLayout: "twenty_six_key",
  title: "首道双拼",
  glyph: "S",
  badge: "双",
};
const WUBI: SchemeDefinition = {
  id: "WUBI",
  preferenceId: "wubi",
  engineScheme: "wubi",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "86 五笔",
  glyph: "五",
  badge: "86",
};
const JAPANESE_NINE_KEY: SchemeDefinition = {
  id: "JAPANESE_NINE_KEY",
  preferenceId: "japanese_nine_key",
  engineScheme: "japanese",
  shuangpinProfile: null,
  touchKeyboardLayout: "nine_key",
  title: "日语 9 键",
  glyph: "あ",
  badge: "9",
};
const JAPANESE: SchemeDefinition = {
  id: "JAPANESE",
  preferenceId: "japanese",
  engineScheme: "japanese",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "日语 26 键",
  glyph: "あ",
  badge: "26",
};
const KOREAN: SchemeDefinition = {
  id: "KOREAN",
  preferenceId: "korean",
  engineScheme: "korean",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "韩语 26 键",
  glyph: "한",
  badge: "26",
};
const CANTONESE: SchemeDefinition = {
  id: "CANTONESE",
  preferenceId: "cantonese",
  engineScheme: "cantonese",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "粤拼 26 键",
  glyph: "粤",
  badge: "26",
};
const ZHUYIN: SchemeDefinition = {
  id: "ZHUYIN",
  preferenceId: "zhuyin",
  engineScheme: "zhuyin",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "大千注音",
  glyph: "注",
  badge: "大千",
};
const VIETNAMESE: SchemeDefinition = {
  id: "VIETNAMESE",
  preferenceId: "vietnamese",
  engineScheme: "vietnamese",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "越南语 26 键",
  glyph: "越",
  badge: "26",
};
const HANDWRITING: SchemeDefinition = {
  id: "HANDWRITING",
  preferenceId: "handwriting",
  engineScheme: "quanpin",
  shuangpinProfile: null,
  touchKeyboardLayout: "handwriting",
  title: "手写",
  glyph: "写",
  badge: "手",
};
const THOUGHTFUL_REPLY: SchemeDefinition = {
  id: "THOUGHTFUL_REPLY",
  preferenceId: "thoughtful_reply",
  engineScheme: "quanpin",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "高情商回复",
  glyph: "聊",
  badge: "AI",
};

export class KeyboardScheme {
  static readonly QUANPIN: SchemeDefinition = QUANPIN;
  static readonly QUANPIN_NINE_KEY: SchemeDefinition = QUANPIN_NINE_KEY;
  static readonly XIAOHE: SchemeDefinition = XIAOHE;
  static readonly ZIRANMA: SchemeDefinition = ZIRANMA;
  static readonly MICROSOFT: SchemeDefinition = MICROSOFT;
  static readonly SHOUDAO: SchemeDefinition = SHOUDAO;
  static readonly WUBI: SchemeDefinition = WUBI;
  static readonly JAPANESE_NINE_KEY: SchemeDefinition = JAPANESE_NINE_KEY;
  static readonly JAPANESE: SchemeDefinition = JAPANESE;
  static readonly HANDWRITING: SchemeDefinition = HANDWRITING;
  static readonly THOUGHTFUL_REPLY: SchemeDefinition = THOUGHTFUL_REPLY;
  static readonly KOREAN: SchemeDefinition = KOREAN;
  static readonly CANTONESE: SchemeDefinition = CANTONESE;
  static readonly ZHUYIN: SchemeDefinition = ZHUYIN;
  static readonly VIETNAMESE: SchemeDefinition = VIETNAMESE;

  /** Declaration order is the fixed order the pickers render. */
  static readonly SCHEMES: SchemeDefinition[] = [
    QUANPIN,
    QUANPIN_NINE_KEY,
    XIAOHE,
    ZIRANMA,
    MICROSOFT,
    SHOUDAO,
    WUBI,
    JAPANESE_NINE_KEY,
    JAPANESE,
    HANDWRITING,
    THOUGHTFUL_REPLY,
    // Appended, as the shared `TouchKeyboardScheme::ALL` appends it, so the existing cards keep their places.
    KOREAN,
    CANTONESE,
    ZHUYIN,
    VIETNAMESE,
  ];

  /** What a keyboard shows before the user picks any, as the shared `TouchKeyboardScheme::DEFAULT_ENABLED` has it: Cantonese, Zhuyin and Vietnamese are turned on by the user, so a device without a stored list keeps the keyboard it always had. */
  static readonly DEFAULT_ENABLED: SchemeDefinition[] = KeyboardScheme.SCHEMES.filter(
    (candidate: SchemeDefinition): boolean =>
      candidate !== CANTONESE && candidate !== ZHUYIN && candidate !== VIETNAMESE,
  );

  /** The dictionary file an Engine scheme (by wire name) cannot type without, or null when it needs none. Cantonese and Zhuyin read their own lexicon from the language-dictionaries directory beside the Engine resources; the file names are the ones the Engine looks for. */
  static languageDictionary(engineScheme: string): string | null {
    if (engineScheme === "cantonese") {
      return "cantonese.db";
    }
    if (engineScheme === "zhuyin") {
      return "zhuyin.db";
    }
    return null;
  }

  /** Drops the schemes whose dictionary `installed` says is missing; falls back to 全拼 rather than an empty keyboard. */
  static withInstalledDictionaries(
    enabled: SchemeDefinition[],
    installed: (file: string) => boolean,
  ): SchemeDefinition[] {
    const available: SchemeDefinition[] = enabled.filter((candidate: SchemeDefinition): boolean => {
      const file: string | null = KeyboardScheme.languageDictionary(candidate.engineScheme);
      return file === null || installed(file);
    });
    return available.length === 0 ? [QUANPIN] : available;
  }

  static fromPreferenceId(value: string | null): SchemeDefinition | null {
    if (value === null) {
      return null;
    }
    for (const candidate of KeyboardScheme.SCHEMES) {
      if (candidate.preferenceId === value) {
        return candidate;
      }
    }
    return null;
  }

  /** Resolves preference IDs in the fixed order and ignores unknown duplicates. */
  static enabledFromPreferenceIds(ids: string[] | null): SchemeDefinition[] {
    if (ids === null) {
      return KeyboardScheme.DEFAULT_ENABLED;
    }
    const enabled: SchemeDefinition[] = [];
    for (const candidate of KeyboardScheme.SCHEMES) {
      if (ids.includes(candidate.preferenceId)) {
        enabled.push(candidate);
      }
    }
    return enabled.length === 0 ? [QUANPIN] : enabled;
  }

  /** Shared selection is authoritative; otherwise preserve the applied scheme or use first enabled. */
  static resolveEnabledSelection(
    applied: SchemeDefinition | null,
    selectedPreferenceId: string | null,
    enabled: SchemeDefinition[] | null,
  ): SchemeDefinition {
    const available: SchemeDefinition[] =
      enabled === null || enabled.length === 0 ? [QUANPIN] : enabled;
    const selected: SchemeDefinition | null = KeyboardScheme.fromPreferenceId(selectedPreferenceId);
    if (selected !== null && available.includes(selected)) {
      return selected;
    }
    if (selectedPreferenceId === null && applied !== null && available.includes(applied)) {
      return applied;
    }
    return available[0];
  }

  /** The Engine preference mapping needed when the shared picker changed the fallback. */
  static mappingForRuntimeSelection(
    applied: SchemeDefinition | null,
    selected: SchemeDefinition | null,
    currentLastChineseScheme: string | null,
    currentProfile: string | null,
  ): PreferenceMapping | null {
    if (selected === null || selected === THOUGHTFUL_REPLY || selected === applied) {
      return null;
    }
    return KeyboardScheme.mapping(selected, currentLastChineseScheme, currentProfile);
  }

  static fromHostSelection(
    value: string | null,
    thoughtfulEnabled: boolean,
    engineSelection: SchemeDefinition,
  ): SchemeDefinition {
    if (thoughtfulEnabled && value === THOUGHTFUL_REPLY.id && engineSelection === QUANPIN) {
      return THOUGHTFUL_REPLY;
    }
    return engineSelection;
  }

  static fromPreferences(
    scheme: string,
    profile: string | null,
    touchLayout: string,
  ): SchemeDefinition {
    if (scheme === "quanpin" && touchLayout === "handwriting") {
      return HANDWRITING;
    }
    if (scheme === "quanpin" && touchLayout === "nine_key") {
      return QUANPIN_NINE_KEY;
    }
    if (scheme === "japanese" && touchLayout === "nine_key") {
      return JAPANESE_NINE_KEY;
    }
    if (scheme === "shuangpin") {
      for (const candidate of KeyboardScheme.SCHEMES) {
        if (profile !== null && profile === candidate.shuangpinProfile) {
          return candidate;
        }
      }
      return XIAOHE;
    }
    for (const candidate of KeyboardScheme.SCHEMES) {
      if (
        candidate !== THOUGHTFUL_REPLY &&
        candidate.shuangpinProfile === null &&
        candidate.engineScheme === scheme &&
        candidate.touchKeyboardLayout !== "nine_key"
      ) {
        return candidate;
      }
    }
    return QUANPIN;
  }

  static mapping(
    scheme: SchemeDefinition,
    currentLastChineseScheme: string | null,
    currentProfile: string | null,
  ): PreferenceMapping {
    let profile: string = KeyboardScheme.normalizedProfile(currentProfile);
    if (scheme.shuangpinProfile !== null) {
      profile = scheme.shuangpinProfile;
    }
    let lastChinese: string =
      KeyboardScheme.isChineseScheme(currentLastChineseScheme) && currentLastChineseScheme !== null
        ? currentLastChineseScheme
        : "quanpin";
    // Japanese, Korean and Vietnamese replace the Chinese scheme without becoming one, so the one they replaced is what 中文 goes back to.
    if (KeyboardScheme.isChineseScheme(scheme.engineScheme)) {
      lastChinese = scheme.engineScheme;
    }
    return {
      scheme: scheme.engineScheme,
      lastChineseScheme: lastChinese,
      shuangpinProfile: profile,
      touchKeyboardLayout: scheme.touchKeyboardLayout,
    };
  }

  /**
   * The Engine's runtime scheme id as the name the shared policies compare against.
   *
   * The view is handed a number by the Engine view and the policies are written in terms of the
   * scheme names the preference document uses; without this the two silently fail to match, and a
   * policy that refuses something for Japanese never refuses it at all.
   */
  static engineSchemeName(id: number): string {
    switch (id) {
      case 1:
        return "shuangpin";
      case 2:
        return "wubi";
      case 3:
        return "japanese";
      case 4:
        return "korean";
      case 5:
        return "cantonese";
      case 6:
        return "zhuyin";
      case 7:
        return "vietnamese";
      default:
        return "quanpin";
    }
  }

  /** Handwriting is a Chinese typing face, not the English, symbol or local-utility keyboard. */
  static usesHandwritingFace(
    scheme: SchemeDefinition,
    english: boolean,
    symbols: boolean,
    localMode: string,
  ): boolean {
    return (
      scheme.touchKeyboardLayout === "handwriting" && !english && !symbols && localMode === "none"
    );
  }

  /** Local utilities take literal alphabetic keys even when the saved touch layout is nine-key. */
  static usesNineKeyFace(nineKey: boolean, localMode: string): boolean {
    return nineKey && localMode === "none";
  }

  private static isChineseScheme(value: string | null): boolean {
    return value !== null && SchemeTraits.isChinese(SchemeTraits.fromName(value));
  }

  private static normalizedProfile(value: string | null): string {
    if (value === "ziranma" || value === "microsoft" || value === "shoudao") {
      return value;
    }
    return "xiaohe";
  }
}
