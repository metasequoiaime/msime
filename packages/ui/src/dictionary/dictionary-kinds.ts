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
];

export const resourceDictionaryKinds: [ResourceDictionaryKind, string][] = [
  ["japanese", "日语"],
  ["korean_hanja", "韩语 · 汉字"],
  ["cantonese", "中文 · 粤语"],
  ["zhuyin", "中文 · 注音"],
  ["stroke", "中文 · 笔画"],
];

export function isResourceDictionaryKind(
  kind: DictionaryLookupKind,
): kind is ResourceDictionaryKind {
  return resourceDictionaryKinds.some(([value]) => value === kind);
}
