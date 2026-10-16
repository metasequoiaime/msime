import type { EffectStyle } from "./plugin-preferences";

/** 对应 `client-services::plugins::PluginKind`。`script` 只在桌面（macOS、Windows、Linux）出现，页面只在宿主声明了 `script_plugins` 时展示它。 */
export type PluginKind =
  | "sound"
  | "music"
  | "command_table"
  | "effect"
  | "phrase_table"
  | "helpcode"
  | "wordbook"
  | "symbol_set"
  | "script";

/** 脚本插件能申请的能力：`msime_script_runtime::Capability` 的名字，顺序也与它一致。 */
export type ScriptCapability =
  | "read_composition"
  | "read_commit"
  | "read_candidates"
  | "rewrite_commit"
  | "rerank_candidates"
  | "annotate_candidates"
  | "filter_candidates"
  | "provide_commands";

export type PluginCommand = { trigger: string; title: string; template: string };

/** 脚本插件在 `/` 指令里声明的一条指令：`client-services::plugins::script_pack::ScriptCommand`。和指令表的一条不同，它没有模板，候选由插件给出。 */
export type ScriptCommand = { trigger: string; title: string };

/** 辅助码表的一条：`client-services::plugins::helpcode_pack::HelpcodeEntry`。 */
export type PluginHelpcodeEntry = { character: string; code: string };

/** 符号集的一组：`client-services::plugins::symbol_set::SymbolGroup`。 */
export type PluginSymbolGroup = {
  tab: "symbols" | "kaomoji";
  title: string;
  /** 没写时为空串。 */
  keywords: string;
  items: string[];
};

/** 短语表的一行：`client-services::plugins::phrase_table::PhraseRow`。 */
export type PluginPhrase = { key: string; text: string };

/** `client-services::plugins::scan` 列出的一个包：清单的各字段，并把该种类的内容平铺进来。 */
export type PluginPackage = {
  id: string;
  name: string;
  version: string;
  license: string;
  author: string | null;
  description: string | null;
  /** Shipped in the bundle rather than installed; cannot be removed. */
  builtin: boolean;
  kind: PluginKind;
  /** Sound packs: a sample per key class, or one sample a melody is played on. */
  mode?: "keys" | "sequence";
  /** Music packs. */
  tracks?: string[];
  /** Command tables; script packs that provide `/` commands list theirs here too, without a template. */
  commands?: (PluginCommand | ScriptCommand)[];
  /** 短语表。 */
  phrases?: PluginPhrase[];
  /** 辅助码表：码表文件名、条数和前几条。 */
  table?: string;
  entries?: number;
  preview?: PluginHelpcodeEntry[];
  /** 单词本：词表文件名、单词数和前几个单词。 */
  file?: string;
  word_count?: number;
  first_words?: string[];
  /** 符号集的各组。 */
  groups?: PluginSymbolGroup[];
  /** 特效包：`client-services::plugins::effect_pack::EffectPack`，从不为 `off`。 */
  style?: EffectStyle;
  intensity?: number;
  colors?: string[];
  duration_ms?: number | null;
  particles?: number | null;
  /** 脚本插件（`client-services::plugins::script_pack::ScriptPack`）：ABI 版本、模块文件名、声明的钩子、申请的能力、每个实例预留的内存页数和内容哈希。授权绑定在内容哈希上。 */
  abi?: number;
  module?: string;
  hooks?: string[];
  permissions?: ScriptCapability[];
  memory_pages?: number;
  content_hash?: string;
};

/** `script-grants.json` 的一条：client-services 的 `script_grants::ScriptGrantView`。 */
export type ScriptGrant = {
  id: string;
  /** 授权时包的内容哈希；与包现在的 `content_hash` 不同时授权已经作废。 */
  hash: string;
  enabled: boolean;
  granted: ScriptCapability[];
};

/** 输入法停用了的一个脚本插件（`<user_data>/script-health.json`）：模块不合规、初始化失败、出错太多或太慢。 */
export type ScriptHealth = {
  id: string;
  hash: string;
  disabled: "invalid" | "init" | "failed" | "slow";
};

/** 一条插件 `/` 指令在输入法里的状态：client-services 的 `script_commands::ResolvedCommand`。用户指令表永远赢，插件之间按执行顺序先到先得，相等或互为前缀都算冲突。 */
export type ScriptCommandState = { id: string; trigger: string; title: string } & (
  | { status: "active" }
  /** 和用户指令表里的 `/conflict` 冲突，归用户的指令。 */
  | { status: "shadowed_by_user_table"; conflict: string }
  /** 和执行顺序在前的插件 `conflict_id` 的 `/conflict` 冲突，归那个插件。 */
  | { status: "shadowed_by_plugin"; conflict_id: string; conflict: string }
);

/** 授权页读到的全部：client-services 的 `script_grants::ScriptGrantsView`。`plugins` 的顺序就是执行顺序。 */
export type ScriptGrantsResult = {
  revision: number;
  plugins: ScriptGrant[];
  health: ScriptHealth[];
  /** 启用、授予了「提供 / 指令」、清单能载入的插件声明的指令和各自的状态，按执行顺序。 */
  commands: ScriptCommandState[];
};

/** 插件详情可以链接过去的设置页：辅助码和快捷模式都在「输入」页。 */
export type PluginSettingsPage = "input" | "vocabulary";

/** A folder under the plugins directory that is not a loadable pack, and why. */
export type PluginIssue = { kind: PluginKind; folder: string; reason: string };

export type PluginCatalogResult = { packages: PluginPackage[]; issues: PluginIssue[] };

/** @ 模式名单中的一条：`client-services::plugins::mentions::MentionEntry`。 */
export type MentionEntry = { text: string; key: string };

/**
 * The host side of the 插件 page. The host resolves every directory itself: the page reads the catalog, asks the host to show its own picker for an import, and names a pack to remove by kind and id, never by path.
 */
export interface PluginClient {
  catalog(): Promise<PluginCatalogResult>;
  /** Shows the platform's picker for a pack folder or a `.zip` file and installs what was picked; null when the picker was closed. */
  importPack(source: "folder" | "archive"): Promise<PluginPackage | null>;
  /** 用系统选择器导入单个 UTF-8 辅助码表；取消时返回 null。 */
  importHelpcodeTable?(): Promise<PluginPackage | null>;
  remove(kind: PluginKind, id: string): Promise<void>;
  loadMentions(): Promise<MentionEntry[]>;
  saveMentions(entries: MentionEntry[]): Promise<void>;
  /** 脚本插件的授权和执行顺序；只有运行脚本插件的桌面宿主有这三个方法。 */
  scriptGrants?(): Promise<ScriptGrantsResult>;
  /** 授予、调整或撤销一个脚本插件的授权。`hash` 是页面展示给用户的那个内容哈希，包在这之后被换过时以 `plugin_script_changed` 拒绝。 */
  setScriptGrant?(
    id: string,
    hash: string,
    enabled: boolean,
    granted: ScriptCapability[],
  ): Promise<ScriptGrantsResult>;
  /** 调整执行顺序：`ids` 按给出的顺序排在最前。 */
  reorderScripts?(ids: string[]): Promise<ScriptGrantsResult>;
}
