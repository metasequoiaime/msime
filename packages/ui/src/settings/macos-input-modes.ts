import type { EditionInfo } from "../core/host-capabilities";
import type { InputScheme } from "../core/input-scheme";

/** macOS 输入法列表的读取与系统设置入口，见 `SettingsClient.macosInputModes`。 */
export interface MacosInputModesClient {
  /** 用户已经加入输入法列表的本输入法模式（完整标识符）；读不到列表时为 `null`。 */
  enabled(): Promise<string[] | null>;
  /** 打开系统设置里添加输入法的那一页。 */
  openSettings(): Promise<void>;
}

/** 一个菜单栏入口：输入法菜单里显示的名字、它属于哪个方案（`null` 表示不随方案出现），以及系统设置「添加」对话框把它归在哪个语言下。 */
export interface MacosInputModeEntry {
  mode: string;
  name: string;
  scheme: InputScheme | null;
  language: string;
}

/** 顺序与输入法菜单一致，名字与 `platforms/macos/resources/*.lproj/InfoPlist.strings` 一致，语言对应 Info.plist.in 里各模式的 `TISIntendedLanguage`。 */
export const macosInputModeEntries: readonly MacosInputModeEntry[] = [
  { mode: "Hans", name: "水杉全拼", scheme: null, language: "简体中文" },
  { mode: "Shuangpin", name: "水杉双拼", scheme: "shuangpin", language: "简体中文" },
  { mode: "Wubi", name: "水杉五笔", scheme: "wubi", language: "简体中文" },
  { mode: "Cantonese", name: "水杉粤语", scheme: "cantonese", language: "粤语" },
  { mode: "Zhuyin", name: "水杉注音", scheme: "zhuyin", language: "繁体中文" },
  { mode: "Japanese", name: "水杉日语", scheme: "japanese", language: "日语" },
  { mode: "Korean", name: "水杉韩语", scheme: "korean", language: "韩语" },
  { mode: "Vietnamese", name: "水杉越南语", scheme: "vietnamese", language: "越南语" },
  { mode: "Tibetan", name: "水杉藏语", scheme: "tibetan", language: "藏语" },
  { mode: "Stroke", name: "水杉笔画", scheme: "stroke", language: "简体中文" },
  { mode: "Roman", name: "水杉英文", scheme: null, language: "简体中文" },
];

export const macosInputModeProductName = "水杉输入法";

/**
 * 本版本的菜单栏入口，与 `platforms/macos/scripts/edition_bundle.py` 生成的 Info.plist 一致：本版本的方案对应的模式加上「英」，主模式 `Hans` 总在。版本不含全拼时，`Hans` 用默认方案模式的名字（五笔版是「水杉五笔」，日文版是「水杉日语」），默认方案自己的模式不再单列；默认方案不在「简体中文」下时（日文、越南文、藏文版），`Hans` 和「水杉英文」都登记在那个方案的语言下。模式名是方案名，不带版本的产品名；仍以产品名开头的旧式名字才换成版本名。full（`edition` 缺省）就是上面这张表。
 */
export function macosInputModeEntriesFor(edition?: EditionInfo): readonly MacosInputModeEntry[] {
  if (!edition) return macosInputModeEntries;
  const name = edition.display_name ?? macosInputModeProductName;
  const primary = edition.input_schemes.includes("quanpin")
    ? null
    : (macosInputModeEntries.find((entry) => entry.scheme === edition.default_scheme) ?? null);
  return macosInputModeEntries.flatMap((entry) => {
    if (primary && entry.mode === primary.mode) return [];
    if (entry.scheme !== null && !edition.input_schemes.includes(entry.scheme)) return [];
    const source = entry.mode === "Hans" && primary ? primary : entry;
    const language =
      (entry.mode === "Hans" || entry.mode === "Roman") && primary
        ? primary.language
        : entry.language;
    return [{ ...entry, name: source.name.replace(macosInputModeProductName, name), language }];
  });
}

/** 点了「打开键盘设置」之后多久内反复读取列表：用户在系统设置里添加时，设置窗口不一定会重新获得焦点。 */
export const INPUT_MODE_RECHECK_MS = 3000;
export const INPUT_MODE_RECHECK_WINDOW_MS = 120_000;

export interface MacosInputModeGroup {
  language: string;
  entries: readonly MacosInputModeEntry[];
}

/** 只列宿主提供且尚未加入系统列表的入口；系统标识已经由宿主按本版 bundle id 筛过。 */
export function missingMacosInputModeEntries(
  enabled: readonly string[],
  inputSchemes: readonly InputScheme[],
  edition?: EditionInfo,
): readonly MacosInputModeEntry[] {
  return macosInputModeEntriesFor(edition).filter(
    (entry) =>
      (entry.scheme === null || inputSchemes.includes(entry.scheme)) &&
      !enabled.some((identifier) => identifier.endsWith(`.${entry.mode}`)),
  );
}

/** 语言按首次出现的顺序排列，组内保留输入法菜单的入口顺序。 */
export function groupMacosInputModeEntries(
  entries: readonly MacosInputModeEntry[],
): readonly MacosInputModeGroup[] {
  const groups = new Map<string, MacosInputModeEntry[]>();
  for (const entry of entries)
    groups.set(entry.language, [...(groups.get(entry.language) ?? []), entry]);
  return [...groups].map(([language, members]) => ({ language, entries: members }));
}
