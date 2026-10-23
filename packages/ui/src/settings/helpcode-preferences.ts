import type { PluginPreferences } from "./plugin-preferences";

export type HelpcodeSchema =
  | "lantian"
  | "ziranma"
  | "shouyou2_0"
  | "shouyouplus"
  | "xiaohe"
  | "jiajia"
  | "wubi86"
  | `custom/${string}`;

/** 宿主资源目录中发现的自定义辅助码表元数据。 */
export type CustomHelpcodeSchema = {
  schema: `custom/${string}`;
  file_stem: string;
  name: string;
  name_en: string;
};

/** 双拼辅助码的触发方式（Windows v0.11.1 `[helpcode]` 组的双拼专属键）；只有 `shuangpin_helpcode` 一族读它。设置页按末尾直接、末尾间接、句中间接和逐音节直接列出开关。 */
export type ShuangpinHelpcodeModes = {
  /** 仅按 Tab 后进入辅助码，保留其它触发偏好。 */
  manual_tab: boolean;
  /** 末尾直接：完整音节后直接接辅码。 */
  direct: boolean;
  /** 末尾间接触发键：反引号。 */
  trigger_backtick: boolean;
  /** 末尾间接触发键：分号。 */
  trigger_semicolon: boolean;
  /** 末尾间接触发键：段首大写。 */
  trigger_uppercase: boolean;
  /** 句中（间接）辅助码总开关。 */
  mid_sentence: boolean;
  mid_sentence_backtick: boolean;
  mid_sentence_semicolon: boolean;
  mid_sentence_uppercase: boolean;
  /** 句中（直接）「万象式」总开关。 */
  direct_helpcode: boolean;
  direct_helpcode_slash: boolean;
  direct_helpcode_uppercase: boolean;
};

export type HelpcodePreferences = {
  enabled: boolean;
  schema: HelpcodeSchema;
  show_in_candidate_window?: boolean;
  trigger_key?: "" | ";" | "/" | "\\" | "`" | "=";
  /** 双拼专属的触发方式；缺省时按 `defaultShuangpinHelpcodeModes`。 */
  modes?: ShuangpinHelpcodeModes;
};

export type HelpcodeKey = "quanpin_helpcode" | "shuangpin_helpcode";
/** 辅助码设置，以及选中辅助码表插件的 `plugins.helpcode_pack_*`。 */
export type HelpcodeSettings = Partial<Record<HelpcodeKey, HelpcodePreferences>> & {
  plugins?: PluginPreferences;
};

/** 一个已安装的辅助码表插件，供下拉框选用。 */
export type HelpcodePackOption = { id: string; name: string };

/** 与 client-core `ShuangpinHelpcodeModes::default`（Windows v0.11.1 出厂值）一致。 */
export const defaultShuangpinHelpcodeModes: ShuangpinHelpcodeModes = {
  manual_tab: false,
  direct: true,
  trigger_backtick: false,
  trigger_semicolon: true,
  trigger_uppercase: false,
  mid_sentence: false,
  mid_sentence_backtick: true,
  mid_sentence_semicolon: false,
  mid_sentence_uppercase: false,
  direct_helpcode: false,
  direct_helpcode_slash: true,
  direct_helpcode_uppercase: false,
};

/** 与核心的全拼、双拼辅助码默认值一致，设置页和候选预览共用。 */
export const defaultHelpcode: Readonly<Record<HelpcodeKey, HelpcodePreferences>> = {
  quanpin_helpcode: {
    enabled: true,
    schema: "ziranma",
    show_in_candidate_window: false,
  },
  shuangpin_helpcode: {
    enabled: true,
    schema: "lantian",
    show_in_candidate_window: true,
    modes: defaultShuangpinHelpcodeModes,
  },
};

export const helpcodeSchemaOptions: readonly (readonly [HelpcodeSchema, string])[] = [
  ["lantian", "蓝天小雨点"],
  ["ziranma", "自然码"],
  ["shouyou2_0", "首右2.0"],
  ["shouyouplus", "首右plus"],
  ["xiaohe", "小鹤"],
  ["jiajia", "加加"],
  ["wubi86", "五笔 86"],
];
