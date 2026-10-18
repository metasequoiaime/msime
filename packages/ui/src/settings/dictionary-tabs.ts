/** 桌面词库页的顶部分类；手机词库页按平台设计稿直列，不显示这组标签。 */
export const dictionaryTabs = [
  { id: "lookup", title: "查找词库" },
  { id: "manage", title: "管理词库" },
] as const;

export const cloudDictionaryTab = { id: "cloud", title: "云端词库" } as const;

export type DictionaryTab = (typeof dictionaryTabs)[number]["id"] | typeof cloudDictionaryTab.id;
