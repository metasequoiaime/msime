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
  ["cantonese", "中文 · 粤语"],
  ["zhuyin", "中文 · 注音"],
  ["stroke", "中文 · 笔画"],
];

export function isResourceDictionaryKind(
  kind: DictionaryLookupKind,
): kind is ResourceDictionaryKind {
  return resourceDictionaryKinds.some(([value]) => value === kind);
}
