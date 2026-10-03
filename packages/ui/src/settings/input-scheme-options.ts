import type {
  ChineseScheme,
  EditionInfo,
  HostCapabilities,
  InputScheme,
  VietnamesePreferences,
} from "../index";

export type ChineseInputScheme = ChineseScheme;

export const chineseInputSchemeOptions = [
  { value: "quanpin", label: "全拼" },
  { value: "shuangpin", label: "双拼" },
  { value: "wubi", label: "五笔" },
  { value: "cantonese", label: "粤拼" },
  { value: "zhuyin", label: "注音" },
] as const satisfies readonly { value: ChineseInputScheme; label: string }[];

export const japaneseInputSchemeOptions = [{ value: "romaji", label: "罗马音" }] as const;

export const koreanInputSchemeOptions = [{ value: "dubeolsik", label: "两套式" }] as const;

export const cantoneseInputSchemeOptions = [{ value: "jyutping", label: "粤拼" }] as const;

export const zhuyinLayoutOptions = [{ value: "dachen", label: "大千" }] as const;

export const vietnameseInputMethodOptions = [
  { value: "telex", label: "Telex" },
  { value: "vni", label: "VNI" },
] as const satisfies readonly {
  value: NonNullable<VietnamesePreferences["input_method"]>;
  label: string;
}[];

/** 藏文只有 EWTS（扩展威利转写）一种输入法，选择器只作说明。 */
export const tibetanInputSchemeOptions = [{ value: "ewts", label: "威利转写" }] as const;

export const vietnameseToneStyleOptions = [
  { value: "modern", label: "新式 hoà" },
  { value: "classic", label: "旧式 hòa" },
] as const satisfies readonly {
  value: NonNullable<VietnamesePreferences["tone_style"]>;
  label: string;
}[];

/** 不是中文的输入模式：选中其中一个时把中文方案记进 `last_chinese_scheme`，点「中文」回到它。 */
export const nonChineseSchemes = ["japanese", "korean", "vietnamese", "tibetan"] as const;

export function isChineseScheme(scheme: InputScheme): scheme is ChineseScheme {
  return !(nonChineseSchemes as readonly InputScheme[]).includes(scheme);
}

/** The schemes offered when there is no host (the browser preview and tests). */
export const baseInputSchemes: readonly InputScheme[] = [
  "quanpin",
  "shuangpin",
  "wubi",
  "japanese",
  "korean",
];

const knownInputSchemes: readonly InputScheme[] = [
  ...baseInputSchemes,
  "cantonese",
  "zhuyin",
  "vietnamese",
  "tibetan",
];

/** The schemes the host offers, or `baseInputSchemes` without a host. A value the page has no label for is dropped rather than shown as an unlabelled option. */
export function supportedInputSchemes(host?: HostCapabilities): readonly InputScheme[] {
  return host
    ? host.input_schemes.filter((scheme) => knownInputSchemes.includes(scheme))
    : baseInputSchemes;
}

/** 文档里的方案宿主不提供时 host-api 实际运行的方案：记住的中文方案可用就用它，否则用版本的默认方案（`defaultScheme`，full 和没有宿主时是全拼）。 */
export function fallbackChineseScheme(
  lastChineseScheme: ChineseScheme | null | undefined,
  supported: readonly InputScheme[],
  defaultScheme: ChineseScheme = "quanpin",
): ChineseScheme {
  return lastChineseScheme && supported.includes(lastChineseScheme)
    ? lastChineseScheme
    : defaultScheme;
}

/** 版本的默认中文方案；没有版本信息（full，以及没有宿主时）是全拼。 */
export function editionDefaultChineseScheme(edition?: EditionInfo): ChineseScheme {
  const scheme = edition?.default_scheme;
  return scheme && isChineseScheme(scheme) ? scheme : "quanpin";
}

/** 只有一个方案的版本的那个方案；full、没有宿主和有多个方案的版本返回 undefined。这样的版本没有可选的方案，设置页隐藏方案选择。 */
export function singleEditionScheme(edition?: EditionInfo): InputScheme | undefined {
  return edition?.input_schemes.length === 1 ? edition.input_schemes[0] : undefined;
}

/** 版本是否提供全拼或双拼，也就是辅助码有没有用处：Engine 只在这两个方案下使用辅助码（五笔的辅助码在 client-core 里始终关闭）。没有版本信息时提供。 */
export function editionUsesHelpcode(edition?: EditionInfo): boolean {
  return (
    !edition ||
    edition.input_schemes.includes("quanpin") ||
    edition.input_schemes.includes("shuangpin")
  );
}
