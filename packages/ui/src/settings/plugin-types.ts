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
  | "provide_commands"
  | "insert_candidates"
  | "persist_data";

export type PluginCommand = { trigger: string; title: string; template: string };

/** 脚本插件在 `/` 指令里声明的一条指令：`client-services::plugins::script_pack::ScriptCommand`。和指令表的一条不同，它没有模板，候选由插件给出。`arguments` 为 `extended` 时触发词后面另外收数字和运算符号；只收字母（默认）时不序列化。 */
export type ScriptCommand = { trigger: string; title: string; arguments?: "letters" | "extended" };

/** 脚本插件清单 `[[script.settings]]` 的一项：`script_pack::ScriptSetting`，`type` 之外的字段按类型平铺。 */
export type ScriptSetting = { key: string; title: string; description?: string } & (
  | { type: "bool"; default: boolean }
  | { type: "choice"; options: { value: string; title: string }[]; default: string }
  /** 整数，`min <= default <= max`。 */
  | { type: "number"; min: number; max: number; default: number }
  /** `max_length` 是 UTF-16 码元数，和 JS 字符串的 `length` 一致。 */
  | { type: "text"; max_length: number; default: string }
);

/** 一项设置的值：布尔、选项的 `value`、整数或文本。 */
export type ScriptSettingValue = boolean | string | number;

/** 插件详情页读到的一个脚本插件的状态：`script_settings::ScriptPluginStateView`。`values` 覆盖清单里的每一项，文件里不合规的值回到默认值；`storage_bytes` 是 `persist_data` 存储现在的字节数。 */
export type ScriptPluginState = {
  id: string;
  values: Record<string, ScriptSettingValue>;
  storage_bytes: number;
};

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

/** 设置页插件编辑器能写的四种类型：`client-services::plugins::authoring::AuthoredKind`。 */
export type AuthoredKind = "phrase_table" | "command_table" | "symbol_set" | "wordbook";

/** 编辑器里符号集的一组：`authoring::AuthoredGroup`。 */
export type AuthoredGroup = {
  tab: "symbols" | "kaomoji";
  title: string;
  keywords: string | null;
  items: string[];
};

/** 编辑器里单词本的一个单词：`authoring::AuthoredWord`。 */
export type AuthoredWord = { word: string; phonetic: string | null; gloss: string };

/** 编辑器与宿主交换的一个包：`authoring::AuthoredPack`。`id` 为 null 表示新建、由宿主生成；内容字段只带与 `kind` 对应的那一个。 */
export type AuthoredPack = {
  kind: AuthoredKind;
  id: string | null;
  name: string;
  version: string;
  license: string;
  author: string | null;
  description: string | null;
  phrases?: PluginPhrase[];
  commands?: PluginCommand[];
  groups?: AuthoredGroup[];
  words?: AuthoredWord[];
};

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
  /** 脚本插件清单 `[[script.settings]]`，按清单顺序；没有时不出现。 */
  settings?: ScriptSetting[];
  /** 脚本插件随包的只读数据文件名（`[script].data`）和合计字节数；没有时不出现。 */
  data?: string[];
  data_bytes?: number;
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

/** @ 模式名单中的一条：`client-services::plugins::mentions::MentionEntry`。`phone` 是移动端导入的联系人手机号，这一页不显示也不编辑，保存时原样带回去；页面上新加的条目没有它。 */
export type MentionEntry = { text: string; key: string; phone?: string };

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
  /** 设置页插件编辑器读出用户插件目录里的一个包；内置包、别的类型和没装的包以 `plugin_not_editable` 拒绝。宿主没有编辑器时为空，页面不显示新建和编辑。 */
  loadAuthoredPack?(kind: AuthoredKind, id: string): Promise<AuthoredPack>;
  /** 保存编辑器里的包，回答装好的包，与 `importPack` 的回答同形。`replace` 为假且同类型同 id 的包已经装着时以 `plugin_exists` 拒绝。 */
  saveAuthoredPack?(pack: AuthoredPack, replace: boolean): Promise<PluginPackage>;
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
  /** 插件详情页：一个脚本插件每项设置现在的值和持久存储的字节数。和下面两项一样，只有运行脚本插件的桌面宿主有。 */
  scriptPluginState?(id: string): Promise<ScriptPluginState>;
  /** 整份替换一个脚本插件的设置值，没写的项回到默认值；不合规时以 `plugin_script_setting_invalid` 拒绝，`detail` 说明是哪一项。改设置不作废授权。 */
  setScriptSettings?(
    id: string,
    values: Record<string, ScriptSettingValue>,
  ): Promise<ScriptPluginState>;
  /** 「清除数据」：删掉一个脚本插件的持久存储，回答清除之后的状态。 */
  clearScriptData?(id: string): Promise<ScriptPluginState>;
}
