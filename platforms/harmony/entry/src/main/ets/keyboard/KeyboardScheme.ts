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
const TIBETAN: SchemeDefinition = {
  id: "TIBETAN",
  preferenceId: "tibetan",
  engineScheme: "tibetan",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "藏文 26 键",
  glyph: "藏",
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
  static readonly KOREAN: SchemeDefinition = KOREAN;
  static readonly CANTONESE: SchemeDefinition = CANTONESE;
  static readonly ZHUYIN: SchemeDefinition = ZHUYIN;
  static readonly VIETNAMESE: SchemeDefinition = VIETNAMESE;
  static readonly TIBETAN: SchemeDefinition = TIBETAN;

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
    // Appended, as the shared `TouchKeyboardScheme::ALL` appends it, so the existing cards keep their places.
    KOREAN,
    CANTONESE,
    ZHUYIN,
    VIETNAMESE,
    TIBETAN,
  ];

  /** 26 键符号层按键实际发出的字符：藏文方案下第三排的 `=` 换成威利叠写用的 `+`，让组字中的叠写（如 `pad+ma`）走标点路由交给引擎；符号面板直接写入编辑框，不能用来叠写。其他方案原样发出。 */
  static symbolRowKey(symbol: string, tibetan: boolean): string {
    return tibetan && symbol === "=" ? "+" : symbol;
  }

  /** 偏好 `wubi_profile` 的两个取值：方案仍是 `wubi`，版本是旁边的独立字段，就像 `shuangpin_profile` 之于 `shuangpin`。 */
  static readonly WUBI_86: string = "wubi86";
  static readonly WUBI_98: string = "wubi98";

  /** 只认 `wubi98`，缺省、未知值和非字符串一律按 86 版，与共享 `Preferences` 的缺省一致。 */
  static normalizedWubiProfile(value: string | null | undefined): string {
    return value === KeyboardScheme.WUBI_98 ? KeyboardScheme.WUBI_98 : KeyboardScheme.WUBI_86;
  }

  /** 方案名：五笔只有一个方案入口，标题跟随 `wubi_profile` 显示「86 五笔」或「98 五笔」；其它方案就是 `title`。 */
  static title(scheme: SchemeDefinition, wubiProfile: string | null | undefined): string {
    if (
      scheme === WUBI &&
      KeyboardScheme.normalizedWubiProfile(wubiProfile) === KeyboardScheme.WUBI_98
    ) {
      return "98 五笔";
    }
    return scheme.title;
  }

  /** 角标：五笔跟随 `wubi_profile` 显示「86」或「98」；其它方案就是 `badge`。 */
  static badge(scheme: SchemeDefinition, wubiProfile: string | null | undefined): string {
    if (
      scheme === WUBI &&
      KeyboardScheme.normalizedWubiProfile(wubiProfile) === KeyboardScheme.WUBI_98
    ) {
      return "98";
    }
    return scheme.badge;
  }

  /** 用户还没挑选时键盘显示的方案，与共享的 `TouchKeyboardScheme::DEFAULT_ENABLED` 一致：粤语、注音、越南语和藏文由用户自己打开，所以没有存过列表的设备仍是原来那套键盘。 */
  static readonly DEFAULT_ENABLED: SchemeDefinition[] = KeyboardScheme.SCHEMES.filter(
    (candidate: SchemeDefinition): boolean =>
      candidate !== CANTONESE &&
      candidate !== ZHUYIN &&
      candidate !== VIETNAMESE &&
      candidate !== TIBETAN,
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
    if (selected === null || selected === applied) {
      return null;
    }
    return KeyboardScheme.mapping(selected, currentLastChineseScheme, currentProfile);
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
    // 日语、韩语、越南语和藏文替换中文方案但本身不是中文方案，所以「中文」回到被它们替换掉的那个方案。
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
      case 8:
        return "tibetan";
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
