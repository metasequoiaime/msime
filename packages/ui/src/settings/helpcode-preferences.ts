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

export type HelpcodePreferences = {
  enabled: boolean;
  schema: HelpcodeSchema;
  show_in_candidate_window?: boolean;
};

export type HelpcodeKey = "quanpin_helpcode" | "shuangpin_helpcode";
/** 辅助码设置，以及选中辅助码表插件的 `plugins.helpcode_pack_*`。 */
export type HelpcodeSettings = Partial<Record<HelpcodeKey, HelpcodePreferences>> & {
  plugins?: PluginPreferences;
};

/** 一个已安装的辅助码表插件，供下拉框选用。 */
export type HelpcodePackOption = { id: string; name: string };

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
