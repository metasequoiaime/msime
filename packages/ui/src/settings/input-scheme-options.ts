import type { ChineseScheme, HostCapabilities, InputScheme, VietnamesePreferences } from "../index";

export type ChineseInputScheme = ChineseScheme;

export const chineseInputSchemeOptions = [
  { value: "quanpin", label: "全拼" },
  { value: "shuangpin", label: "双拼" },
  { value: "wubi", label: "五笔" },
  { value: "cantonese", label: "粤拼" },
  { value: "zhuyin", label: "注音" },
  { value: "stroke", label: "笔画" },
] as const satisfies readonly { value: ChineseInputScheme; label: string }[];

export const japaneseInputSchemeOptions = [{ value: "romaji", label: "罗马音" }] as const;

export const koreanInputSchemeOptions = [{ value: "dubeolsik", label: "两套式" }] as const;

export const cantoneseInputSchemeOptions = [{ value: "jyutping", label: "粤拼" }] as const;

export const zhuyinLayoutOptions = [{ value: "dachen", label: "大千" }] as const;

export const strokeLayoutOptions = [{ value: "hspnz", label: "横竖撇点折" }] as const;

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
  "stroke",
];

/** The schemes the host offers, or `baseInputSchemes` without a host. A value the page has no label for is dropped rather than shown as an unlabelled option. */
export function supportedInputSchemes(host?: HostCapabilities): readonly InputScheme[] {
  return host
    ? host.input_schemes.filter((scheme) => knownInputSchemes.includes(scheme))
    : baseInputSchemes;
}

/** The scheme host-api runs when the document names one the host does not offer: the remembered Chinese scheme when it is offered, else 全拼. */
export function fallbackChineseScheme(
  lastChineseScheme: ChineseScheme | null | undefined,
  supported: readonly InputScheme[],
): ChineseScheme {
  return lastChineseScheme && supported.includes(lastChineseScheme) ? lastChineseScheme : "quanpin";
}
