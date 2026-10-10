/**
 * Keyboard schemes the host exposes, ported from
 * platforms/android/java/app/msime/android/KeyboardScheme.java.
 *
 * Java spells this as an enum carrying fields. ArkTS enums hold only a value, so each scheme is a
 * frozen record and SCHEMES preserves the declaration order the Apple hosts also rely on.
 */
import { AppEdition } from "./AppEdition";
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
// 笔画：五个笔画键加通配键，键盘由 KeyboardView 按方案画成九键外框里的笔画面板，所以布局仍记作 twenty_six_key（与大千注音同理），不会触发九键拼音的数字串解码。
const STROKE: SchemeDefinition = {
  id: "STROKE",
  preferenceId: "stroke",
  engineScheme: "stroke",
  shuangpinProfile: null,
  touchKeyboardLayout: "twenty_six_key",
  title: "笔画",
  glyph: "笔",
  badge: "5",
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
// 写中文的方案（版本表里的方案名），与 client-core 的 `ChineseScheme::of` 相同；提供其中任何一个的版本才有手写。
const CHINESE_ENGINE_SCHEMES: string[] = [
  "quanpin",
  "shuangpin",
  "wubi",
  "cantonese",
  "zhuyin",
  "stroke",
];

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
  static readonly STROKE: SchemeDefinition = STROKE;

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
    STROKE,
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

  /** 本版本的键盘是否提供这个入口：入口背后的方案在本版本里时提供。手写面板写出的是汉字，所以只在提供中文方案的版本里有（full、拼音版、五笔版），日文、越南文和藏文版没有。与 client-core 的 `Edition::offers_touch_scheme` 和 Android 的 `KeyboardScheme.offeredBy` 一致。 */
  static offeredBy(scheme: SchemeDefinition, edition: AppEdition = AppEdition.current()): boolean {
    if (scheme !== HANDWRITING) {
      return edition.offers(scheme.engineScheme);
    }
    return CHINESE_ENGINE_SCHEMES.some((engineScheme: string): boolean =>
      edition.offers(engineScheme),
    );
  }

  /** 选中这个入口时写进偏好 `scheme`、交给 Engine 的方案。手写写的是本版本的默认方案（full 是全拼）：识别由平台识别器完成，手写面板背后的 Engine 只需要跑一个本版本提供的方案，否则 host-api 会把它当作本版本不含的方案回退。 */
  static engineSchemeOf(
    scheme: SchemeDefinition,
    edition: AppEdition = AppEdition.current(),
  ): string {
    return scheme === HANDWRITING ? edition.defaultScheme : scheme.engineScheme;
  }

  /** 偏好里的方案本版本没有、或一个入口都没剩下时退回的入口：本版本提供全拼时是全拼 26 键（与引入版本之前相同），否则是本版本默认方案的 26 键入口（五笔版是五笔）。 */
  static fallback(edition: AppEdition = AppEdition.current()): SchemeDefinition {
    if (KeyboardScheme.offeredBy(QUANPIN, edition)) {
      return QUANPIN;
    }
    const offered: SchemeDefinition[] = KeyboardScheme.SCHEMES.filter(
      (candidate: SchemeDefinition): boolean =>
        candidate !== HANDWRITING && KeyboardScheme.offeredBy(candidate, edition),
    );
    for (const candidate of offered) {
      if (
        candidate.engineScheme === edition.defaultScheme &&
        candidate.touchKeyboardLayout === "twenty_six_key"
      ) {
        return candidate;
      }
    }
    return offered.length > 0 ? offered[0] : HANDWRITING;
  }

  /** 文档里没有启用列表时键盘显示的方案，与共享的 `TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED` 一致：粤语、注音、越南语、藏文和笔画由用户自己打开，所以没有存过列表的设备仍是原来那套键盘。新装只启用中文方案（`TouchKeyboardScheme::DEFAULT_ENABLED`），由 client-core 显式写进文档，不经过这里。本版本不提供的入口不在里面。只有一个方案的版本例外：越南文版、藏文版的入口就是这个版本本身，与 client-core 的 `TouchKeyboardSchemePreferences::for_edition` 和 Android 的 `KeyboardScheme.enabledFromPreferenceIds` 一致。 */
  static defaultEnabled(edition: AppEdition): SchemeDefinition[] {
    const optInEnabled: boolean = !edition.offersSchemeChoice();
    return KeyboardScheme.SCHEMES.filter(
      (candidate: SchemeDefinition): boolean =>
        (optInEnabled ||
          (candidate !== CANTONESE &&
            candidate !== ZHUYIN &&
            candidate !== VIETNAMESE &&
            candidate !== TIBETAN &&
            candidate !== STROKE)) &&
        KeyboardScheme.offeredBy(candidate, edition),
    );
  }

  static readonly DEFAULT_ENABLED: SchemeDefinition[] = KeyboardScheme.defaultEnabled(
    AppEdition.current(),
  );

  /** The dictionary file an Engine scheme (by wire name) cannot type without, or null when it needs none. Cantonese, Zhuyin and Stroke read their own lexicon from the language-dictionaries directory beside the Engine resources; the file names are the ones the Engine looks for. */
  static languageDictionary(engineScheme: string): string | null {
    if (engineScheme === "cantonese") {
      return "msime-cantonese.db";
    }
    if (engineScheme === "zhuyin") {
      return "msime-zhuyin.db";
    }
    if (engineScheme === "stroke") {
      return "msime-stroke.db";
    }
    return null;
  }

  /** 去掉 `installed` 说词典没装的方案；一个都不剩时退回本版本的默认入口（full 是全拼），而不是一个空键盘。 */
  static withInstalledDictionaries(
    enabled: SchemeDefinition[],
    installed: (file: string) => boolean,
    edition: AppEdition = AppEdition.current(),
  ): SchemeDefinition[] {
    const available: SchemeDefinition[] = enabled.filter((candidate: SchemeDefinition): boolean => {
      const file: string | null = KeyboardScheme.languageDictionary(candidate.engineScheme);
      return file === null || installed(file);
    });
    return available.length === 0 ? [KeyboardScheme.fallback(edition)] : available;
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

  /** 按固定顺序解析偏好里的入口 id，忽略不认识的和重复的；本版本不提供的入口（比如 full 那边存下的拼音落到五笔版）也不算，一个都不剩时退回本版本的默认入口。 */
  static enabledFromPreferenceIds(
    ids: string[] | null,
    edition: AppEdition = AppEdition.current(),
  ): SchemeDefinition[] {
    if (ids === null) {
      return edition === AppEdition.current()
        ? KeyboardScheme.DEFAULT_ENABLED
        : KeyboardScheme.defaultEnabled(edition);
    }
    const enabled: SchemeDefinition[] = [];
    for (const candidate of KeyboardScheme.SCHEMES) {
      if (ids.includes(candidate.preferenceId) && KeyboardScheme.offeredBy(candidate, edition)) {
        enabled.push(candidate);
      }
    }
    return enabled.length === 0 ? [KeyboardScheme.fallback(edition)] : enabled;
  }

  /**
   * 键盘「输入方式」面板里列出的方案：`schemes` 里的双拼只留一种，其余方案原样、按原顺序保留。
   *
   * 留下的那一种依次取：`selected` 本身是双拼且在 `schemes` 里时就是它；否则是偏好 `shuangpin_profile`（`profile`，缺省或不认识时按小鹤，与 `mapping` 的规整相同）对应的那一种；它不在 `schemes` 里时取 `schemes` 里第一种双拼。大多数人只用一种双拼，四种都列出来会把手写挤到第二页（#6450）；换双拼方案在设置的「双拼」子菜单里。与 Android 的 `KeyboardScheme.pickerSchemes`、iOS 的 `InputSchemePreference.pickerSchemes` 一致。
   */
  static pickerSchemes(
    schemes: SchemeDefinition[],
    selected: SchemeDefinition | null,
    profile: string | null | undefined,
  ): SchemeDefinition[] {
    let kept: SchemeDefinition | null = null;
    if (selected !== null && selected.shuangpinProfile !== null && schemes.includes(selected)) {
      kept = selected;
    } else {
      const configured: string = KeyboardScheme.normalizedProfile(profile ?? null);
      for (const candidate of schemes) {
        if (candidate.shuangpinProfile === null) {
          continue;
        }
        if (kept === null) {
          kept = candidate;
        }
        if (candidate.shuangpinProfile === configured) {
          kept = candidate;
          break;
        }
      }
    }
    return schemes.filter(
      (candidate: SchemeDefinition): boolean =>
        candidate.shuangpinProfile === null || candidate === kept,
    );
  }

  /** Shared selection is authoritative; otherwise preserve the applied scheme or use first enabled. */
  static resolveEnabledSelection(
    applied: SchemeDefinition | null,
    selectedPreferenceId: string | null,
    enabled: SchemeDefinition[] | null,
    edition: AppEdition = AppEdition.current(),
  ): SchemeDefinition {
    const available: SchemeDefinition[] =
      enabled === null || enabled.length === 0 ? [KeyboardScheme.fallback(edition)] : enabled;
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
    edition: AppEdition = AppEdition.current(),
  ): SchemeDefinition {
    // 手写写进偏好的是本版本的默认方案，见 `engineSchemeOf`；full 是全拼。
    if (
      scheme === KeyboardScheme.engineSchemeOf(HANDWRITING, edition) &&
      touchLayout === "handwriting"
    ) {
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
    return KeyboardScheme.fallback(edition);
  }

  static mapping(
    scheme: SchemeDefinition,
    currentLastChineseScheme: string | null,
    currentProfile: string | null,
    edition: AppEdition = AppEdition.current(),
  ): PreferenceMapping {
    const engineScheme: string = KeyboardScheme.engineSchemeOf(scheme, edition);
    let profile: string = KeyboardScheme.normalizedProfile(currentProfile);
    if (scheme.shuangpinProfile !== null) {
      profile = scheme.shuangpinProfile;
    }
    let lastChinese: string =
      KeyboardScheme.isChineseScheme(currentLastChineseScheme) && currentLastChineseScheme !== null
        ? currentLastChineseScheme
        : KeyboardScheme.isChineseScheme(edition.defaultScheme)
          ? edition.defaultScheme
          : "quanpin";
    // 日语、韩语、越南语和藏文替换中文方案但本身不是中文方案，所以「中文」回到被它们替换掉的那个方案。
    if (KeyboardScheme.isChineseScheme(engineScheme)) {
      lastChinese = engineScheme;
    }
    return {
      scheme: engineScheme,
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
      case 9:
        return "stroke";
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
