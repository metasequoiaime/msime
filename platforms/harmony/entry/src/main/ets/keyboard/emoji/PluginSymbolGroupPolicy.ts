import { EmojiItem, EmojiSymbolGroup, MAX_GROUP_CODE_UNITS } from "./EmojiCatalogModel";

/** 单组最多接收的条目数，与插件清单的上限一致。 */
export const MAX_PLUGIN_SYMBOL_ITEMS: number = 512;
/** 单个条目的 UTF-16 长度上限，与插件清单的上限一致。 */
export const MAX_PLUGIN_SYMBOL_TEXT_UNITS: number = 64;
/** 关键词串的长度上限；清单限制为 256 字节，这里按 UTF-16 留足余量。 */
export const MAX_PLUGIN_SYMBOL_KEYWORD_UNITS: number = 1024;

const SYMBOLS_TAB: string = "symbols";
const KAOMOJI_TAB: string = "kaomoji";

/** `msime_client_emoji_catalog_request` 在 `list_plugin_symbol_groups` 下回传的一组，字段名与接口一致。 */
export interface PluginSymbolGroupDocument {
  pack: string;
  pack_name: string;
  tab: string;
  title: string;
  keywords: string;
  items: string[];
}

/** 校验过的一组插件符号。 */
export interface PluginSymbolGroup {
  readonly pack: string;
  readonly packName: string;
  /** `symbols` 或 `kaomoji`。 */
  readonly tab: string;
  readonly title: string;
  /** 空串表示插件没有给关键词。 */
  readonly keywords: string;
  readonly items: string[];
}

/**
 * 颜文字页和符号页标签栏上的一个标签。
 *
 * 内置标签照旧向 Engine 分页读取（`catalogGroup` / `catalogParent`），插件标签的条目已经随清单读进来，放在 `items` 里。`key` 在同一页内唯一：插件名或组名与内置分类重名时，选中态和 ArkUI 的 `ForEach` 键都不会串。
 */
export interface EmojiPanelGroupTab {
  readonly key: string;
  readonly title: string;
  /** 内置颜文字组名；符号页和插件标签为空。 */
  readonly catalogGroup: string;
  /** 内置符号的上级分类；颜文字页和插件标签为空。 */
  readonly catalogParent: string;
  readonly plugin: boolean;
  /** 标签里显示的条目；插件条目的 `annotation` 为空：插件没有逐项的注释，组的关键词不冒充条目自己的名字。 */
  readonly items: EmojiItem[];
  /** 只供搜索的插件条目，`annotation` 是所在组的关键词；内置标签为空。 */
  readonly searchItems: EmojiItem[];
}

function isUsableName(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_GROUP_CODE_UNITS &&
    value.trim().length > 0
  );
}

function isUsableItem(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_PLUGIN_SYMBOL_TEXT_UNITS &&
    value.trim().length > 0
  );
}

function builtInTab(key: string, title: string, group: string, parent: string): EmojiPanelGroupTab {
  return {
    key: key,
    title: title,
    catalogGroup: group,
    catalogParent: parent,
    plugin: false,
    items: [],
    searchItems: [],
  };
}

/** 一组插件条目；`annotation` 给 `keywords` 时用于搜索，给空串时用于显示。 */
function pluginItems(group: PluginSymbolGroup, annotation: string): EmojiItem[] {
  return group.items.map((text: string): EmojiItem => ({
    text: text,
    annotation: annotation,
    group: group.title,
  }));
}

/**
 * 把符号集插件的组并进 2in1 表情面板的颜文字页和符号页。
 *
 * 规则与 Tauri 面板（`append_plugin_symbol_groups`）一致：插件组一律排在内置组之后；`symbols` 组以插件名为上级分类，每个插件一个分类；`kaomoji` 组逐组排在内置颜文字组之后；不跨插件、也不与内置目录去重。
 */
export class PluginSymbolGroupPolicy {
  /** 校验 native 回传的组；坏组跳过而不是整份作废，插件读不出来时面板仍只显示内置内容。组数不设上限：每个包最多 32 组，装多少包由用户决定，截断只会让后面的包悄悄消失。 */
  static parse(values: unknown): PluginSymbolGroup[] {
    if (!Array.isArray(values)) {
      return [];
    }
    const groups: PluginSymbolGroup[] = [];
    for (const value of values) {
      if (value === null || typeof value !== "object") {
        continue;
      }
      const candidate = value as Partial<Record<keyof PluginSymbolGroupDocument, unknown>>;
      if (
        !isUsableName(candidate.pack) ||
        !isUsableName(candidate.pack_name) ||
        !isUsableName(candidate.title) ||
        (candidate.tab !== SYMBOLS_TAB && candidate.tab !== KAOMOJI_TAB) ||
        !Array.isArray(candidate.items)
      ) {
        continue;
      }
      const keywords: string =
        typeof candidate.keywords === "string" &&
        candidate.keywords.length <= MAX_PLUGIN_SYMBOL_KEYWORD_UNITS
          ? candidate.keywords
          : "";
      const items: string[] = [];
      for (const item of candidate.items) {
        if (items.length === MAX_PLUGIN_SYMBOL_ITEMS) {
          break;
        }
        if (isUsableItem(item)) {
          items.push(item);
        }
      }
      if (items.length === 0) {
        continue;
      }
      groups.push({
        pack: candidate.pack,
        packName: candidate.pack_name,
        tab: candidate.tab,
        title: candidate.title,
        keywords: keywords,
        items: items,
      });
    }
    return groups;
  }

  /**
   * 符号页的标签：先是内置上级分类，再是每个插件一个分类。
   *
   * 内置部分每个上级分类只留第一对，标题沿用该分类的第一个子类——此前标签栏用上级分类做 `ForEach` 键，同一上级的后续子类本来就不会画出来，这里只是把它写明。插件分类以插件名为标题，内容是该插件全部 `symbols` 组按清单顺序首尾相接；面板没有子类一层，组标题不单独显示。
   */
  static symbolTabs(
    builtIn: EmojiSymbolGroup[],
    plugins: PluginSymbolGroup[],
  ): EmojiPanelGroupTab[] {
    const tabs: EmojiPanelGroupTab[] = [];
    const parents: string[] = [];
    for (const group of builtIn) {
      if (parents.includes(group.parent)) {
        continue;
      }
      parents.push(group.parent);
      tabs.push(builtInTab(`builtin:${group.parent}`, group.title, "", group.parent));
    }
    const packs: string[] = [];
    const packTitles: string[] = [];
    const packItems: EmojiItem[][] = [];
    const packSearchItems: EmojiItem[][] = [];
    for (const group of plugins) {
      if (group.tab !== SYMBOLS_TAB) {
        continue;
      }
      let index: number = packs.indexOf(group.pack);
      if (index < 0) {
        index = packs.length;
        packs.push(group.pack);
        packTitles.push(group.packName);
        packItems.push([]);
        packSearchItems.push([]);
      }
      for (const item of pluginItems(group, "")) {
        packItems[index].push(item);
      }
      for (const item of pluginItems(group, group.keywords)) {
        packSearchItems[index].push(item);
      }
    }
    for (let index: number = 0; index < packs.length; index++) {
      tabs.push({
        key: `pack:${packs[index]}`,
        title: packTitles[index],
        catalogGroup: "",
        catalogParent: "",
        plugin: true,
        items: packItems[index],
        searchItems: packSearchItems[index],
      });
    }
    return tabs;
  }

  /** 颜文字页的标签：内置组在前，插件的 `kaomoji` 组按回传顺序逐组排在后面。 */
  static kaomojiTabs(builtIn: string[], plugins: PluginSymbolGroup[]): EmojiPanelGroupTab[] {
    const tabs: EmojiPanelGroupTab[] = builtIn.map((group: string): EmojiPanelGroupTab =>
      builtInTab(`builtin:${group}`, group, group, ""),
    );
    plugins.forEach((group: PluginSymbolGroup, index: number): void => {
      if (group.tab !== KAOMOJI_TAB) {
        return;
      }
      tabs.push({
        key: `pack:${group.pack}:${index}`,
        title: group.title,
        catalogGroup: "",
        catalogParent: "",
        plugin: true,
        items: pluginItems(group, ""),
        searchItems: pluginItems(group, group.keywords),
      });
    });
    return tabs;
  }

  /** 插件标签里的全部条目，按标签顺序排列，供搜索接在内置条目之后；条目的 `annotation` 是所在组的关键词，只用于匹配。 */
  static pluginItems(tabs: EmojiPanelGroupTab[]): EmojiItem[] {
    const items: EmojiItem[] = [];
    for (const tab of tabs) {
      for (const item of tab.searchItems) {
        items.push(item);
      }
    }
    return items;
  }
}
