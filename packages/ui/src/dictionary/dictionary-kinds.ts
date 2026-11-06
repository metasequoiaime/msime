import type {
  DictionaryLookupKind,
  LocalDictionaryKind,
  ResourceDictionaryKind,
} from "./dictionary-file";

export const localDictionaryKinds: [LocalDictionaryKind, string][] = [
  ["pinyin", "中文 · 拼音"],
  ["wubi", "中文 · 86 五笔"],
  ["wubi98", "中文 · 98 五笔"],
  ["wubixsj", "中文 · 新世纪五笔"],
  ["english", "英文"],
  ["quick_phrase", "快捷短语"],
  ["cantonese", "中文 · 粤语"],
];

/** 词库页「词库」下拉里列出的个人词库种类。鸿蒙的设置应用和输入法各有一份 Engine 日志，「我的词条」经 `msime_client_user_words_mirror` 交换，而它与 WebDAV 共用种类映射，还不交换新世纪五笔的词：在设置应用里加的新世纪词到不了键盘，所以鸿蒙上先不列这一类，等那条通道认它时再放开。 */
export function shownLocalDictionaryKinds(harmony: boolean): [LocalDictionaryKind, string][] {
  return harmony
    ? localDictionaryKinds.filter(([kind]) => kind !== "wubixsj")
    : localDictionaryKinds;
}

export const resourceDictionaryKinds: [ResourceDictionaryKind, string][] = [
  ["japanese", "日语"],
  ["korean_hanja", "韩语 · 汉字"],
  ["cantonese", "中文 · 粤语（基础词库）"],
  ["zhuyin", "中文 · 注音"],
  ["stroke", "中文 · 笔画"],
];

/** 查询下拉里资源词库选项的值。粤语既有个人词库也有发布的基础词库，两者同名，所以资源选项带上前缀，不和个人词库的值撞上。 */
export function resourceDictionaryOption(kind: ResourceDictionaryKind): string {
  return `resource:${kind}`;
}

/** 下拉选项值对应的资源词库；不是资源选项时为 `null`。 */
export function resourceDictionaryFromOption(value: string): ResourceDictionaryKind | null {
  return (
    resourceDictionaryKinds.find(([kind]) => resourceDictionaryOption(kind) === value)?.[0] ?? null
  );
}

export function isResourceDictionaryKind(
  kind: DictionaryLookupKind,
): kind is ResourceDictionaryKind {
  return resourceDictionaryKinds.some(([value]) => value === kind);
}
