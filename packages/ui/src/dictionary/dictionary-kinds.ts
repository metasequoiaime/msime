import type {
  DictionaryLookupKind,
  LocalDictionaryKind,
  ResourceDictionaryKind,
} from "./dictionary-file";

export const localDictionaryKinds: [LocalDictionaryKind, string][] = [
  ["pinyin", "中文 · 拼音"],
  ["wubi", "中文 · 86 五笔"],
  ["wubi98", "中文 · 98 五笔"],
  ["english", "英文"],
  ["quick_phrase", "快捷短语"],
  ["cantonese", "中文 · 粤语"],
];

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
