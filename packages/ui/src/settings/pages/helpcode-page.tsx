import { SettingsGroupNote } from "../settings-group-note";
import { Fragment } from "react";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { pluginPreferences, type PluginPreferences } from "../plugin-preferences";
import { SettingsPageFieldset } from "../settings-page-fieldset";

export type HelpcodeSchema =
  | "lantian"
  | "ziranma"
  | "shouyou2_0"
  | "shouyouplus"
  | "xiaohe"
  | "jiajia"
  | "wubi86"
  | `custom/${string}`;

/** Metadata for a helper-code table discovered below the host's resource directory. */
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

/** 下拉框里辅助码表插件的值前缀：`pack:<插件 id>`。 */
const PACK_PREFIX = "pack:";

const packKeys = {
  quanpin_helpcode: "helpcode_pack_quanpin",
  shuangpin_helpcode: "helpcode_pack_shuangpin",
} as const;

/** The core's `default_quanpin_helpcode` / `default_shuangpin_helpcode`, used wherever a document carries no helpcode object for a scheme: this form and the candidate previews. */
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

const helpcodeSchemas: readonly (readonly [HelpcodeSchema, string])[] = [
  ["lantian", "蓝天小雨点"],
  ["ziranma", "自然码"],
  ["shouyou2_0", "首右2.0"],
  ["shouyouplus", "首右plus"],
  ["xiaohe", "小鹤"],
  ["jiajia", "加加"],
  ["wubi86", "五笔 86"],
];

function customSchemaLabel(schema: CustomHelpcodeSchema): string {
  if (schema.name && schema.name_en && schema.name !== schema.name_en) {
    return `${schema.name} (${schema.name_en})`;
  }
  return schema.name || schema.name_en || schema.file_stem || schema.schema;
}

export interface HelpcodeSettingsPageProps {
  value: HelpcodeSettings;
  mobile: boolean;
  showShiftEntry: boolean;
  disabled?: boolean;
  hidden?: boolean;
  /** Custom tables discovered by the host. They are optional for hosts without resource scanning. */
  customSchemas?: readonly CustomHelpcodeSchema[];
  /** 已安装的辅助码表插件；选中后替换该方案的辅助码方案。没有插件目录的宿主为空。 */
  packs?: readonly HelpcodePackOption[];
  /** HarmonyOS 手机的紧凑形式，类似 Android 的 `TypingPage`：分组只显示这一族（当前方案所用的那一族）的 辅助码方案，两族的其他控件都移进 更多选项 折叠区。 */
  activeFamily?: HelpcodeKey;
  onChange: (patch: HelpcodeSettings) => void;
}

/** 独立成页的辅助码表单：一个 fieldset 里只有「辅助码」这一组。设置窗口本身不用它，输入页直接放 `HelpcodeSettingsGroup`；它留给自己拼页面的宿主。 */
export function HelpcodeSettingsPage({
  disabled = false,
  hidden = false,
  ...group
}: HelpcodeSettingsPageProps) {
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="辅助码">
      <HelpcodeSettingsGroup {...group} />
    </SettingsPageFieldset>
  );
}

export type HelpcodeSettingsGroupProps = Omit<HelpcodeSettingsPageProps, "disabled" | "hidden">;

const families = [
  ["quanpin_helpcode", "全拼"],
  ["shuangpin_helpcode", "双拼"],
] as const;

/** 桌面和触屏宿主共用的「辅助码」组，放在输入页的进阶区（旧的 `helpcode` 路由会打开输入页）。 */
export function HelpcodeSettingsGroup({
  value: draft,
  mobile,
  showShiftEntry,
  customSchemas = [],
  packs = [],
  activeFamily,
  onChange,
}: HelpcodeSettingsGroupProps) {
  const plugins = pluginPreferences(draft);
  const schemaOptions = [
    ...helpcodeSchemas,
    ...customSchemas.map((schema): readonly [HelpcodeSchema, string] => [
      schema.schema,
      customSchemaLabel(schema),
    ]),
  ];
  const currentOf = (key: HelpcodeKey) =>
    ({
      ...defaultHelpcode[key],
      ...draft[key],
    }) as Required<HelpcodePreferences>;

  const enabledRow = (key: HelpcodeKey, label: string) => {
    const current = currentOf(key);
    return (
      <SwitchRow
        title={`${label}辅助码`}
        checked={current.enabled}
        onChange={(enabled) => onChange({ [key]: { ...current, enabled } })}
      />
    );
  };

  // `title` 是这一行显示的文字；下拉框始终以自己所属的族命名，所以只写着 辅助码方案 的紧凑行仍能告诉屏幕阅读器（或测试）它设置的是哪一族。
  const schemaRow = (key: HelpcodeKey, label: string, title: string, description?: string) => {
    const current = currentOf(key);
    const packKey = packKeys[key];
    const selectedPack = plugins[packKey];
    const packOptions = packs
      .map((pack): readonly [string, string] => [
        `${PACK_PREFIX}${pack.id}`,
        `${pack.name}（插件）`,
      ])
      .concat(
        selectedPack && !packs.some((pack) => pack.id === selectedPack)
          ? [[`${PACK_PREFIX}${selectedPack}`, `${selectedPack}（插件，未找到）`] as const]
          : [],
      );
    return (
      <SelectRow
        title={title}
        description={description}
        aria-label={title === `${label}辅助码方案` ? undefined : `${label}辅助码方案`}
        disabled={!current.enabled}
        value={selectedPack ? `${PACK_PREFIX}${selectedPack}` : current.schema}
        onChange={(event) => {
          const value = event.target.value;
          if (value.startsWith(PACK_PREFIX)) {
            onChange({
              plugins: { ...plugins, [packKey]: value.slice(PACK_PREFIX.length) },
            });
            return;
          }
          // 选内置或自定义方案时不再使用辅助码表插件。
          onChange({
            [key]: { ...current, schema: value as HelpcodeSchema },
            ...(selectedPack ? { plugins: { ...plugins, [packKey]: "" } } : {}),
          });
        }}
      >
        {schemaOptions
          // 资源目录被更换或在设置启动期间暂时不可用时，仍保留之前选中的码表，让它继续显示在列表中。
          .concat(
            current.schema.startsWith("custom/") &&
              !schemaOptions.some(([schema]) => schema === current.schema)
              ? [[current.schema, current.schema] as const]
              : [],
          )
          .map(([schema, name]): readonly [string, string] => [schema, name])
          .concat(packOptions)
          .map(([schema, name]) => (
            <option key={schema} value={schema}>
              {name}
            </option>
          ))}
      </SelectRow>
    );
  };

  const displayRow = (key: HelpcodeKey, label: string) => {
    const current = currentOf(key);
    // Named after its own scheme, the way the reference window names these: both rows are on the page at once, so one shared wording left two switches with the same accessible name and nothing to tell a screen reader -- or a test -- which one it had.
    const display = mobile ? `在候选栏中显示${label}辅助码` : `在候选窗口中显示${label}辅助码`;
    return (
      <SwitchRow
        title={display}
        checked={current.show_in_candidate_window}
        onChange={(show_in_candidate_window) =>
          onChange({ [key]: { ...current, show_in_candidate_window } })
        }
      />
    );
  };

  const shiftNote = showShiftEntry && (
    <SettingsGroupNote>
      全拼或双拼组字时，按 Shift
      再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、日语、韩语、粤拼、注音、笔画、越南语、藏文和快捷模式不使用辅助码。
    </SettingsGroupNote>
  );

  if (activeFamily) {
    const activeLabel = activeFamily === "shuangpin_helpcode" ? "双拼" : "全拼";
    return (
      <GroupList title="辅助码">
        {schemaRow(activeFamily, activeLabel, "辅助码方案", activeLabel)}
        <MoreOptions>
          {shiftNote}
          {families.map(([key, label]) => (
            <Fragment key={key}>
              {enabledRow(key, label)}
              {key !== activeFamily && schemaRow(key, label, `${label}辅助码方案`)}
              {displayRow(key, label)}
            </Fragment>
          ))}
        </MoreOptions>
      </GroupList>
    );
  }

  return (
    <GroupList title="辅助码">
      {shiftNote}
      {/* 全拼在前，和输入方案选择器「全拼、双拼」的顺序一致。 */}
      {families.map(([key, label]) => (
        <Fragment key={key}>
          {enabledRow(key, label)}
          {schemaRow(key, label, `${label}辅助码方案`)}
          {displayRow(key, label)}
        </Fragment>
      ))}
    </GroupList>
  );
}
