import { SettingsGroupNote } from "../settings-group-note";
import { Fragment } from "react";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { SelectRow } from "../select-row";
import { ActionRow } from "../action-row";
import { SwitchRow } from "../switch-row";
import { pluginPreferences } from "../plugin-preferences";
import { SettingsPageFieldset } from "../settings-page-fieldset";

import {
  defaultHelpcode,
  defaultShuangpinHelpcodeModes,
  helpcodeSchemaOptions,
  type CustomHelpcodeSchema,
  type HelpcodeKey,
  type HelpcodePackOption,
  type HelpcodePreferences,
  type HelpcodeSchema,
  type HelpcodeSettings,
  type ShuangpinHelpcodeModes,
} from "../helpcode-preferences";

export { defaultHelpcode } from "../helpcode-preferences";
export type {
  CustomHelpcodeSchema,
  HelpcodeKey,
  HelpcodePackOption,
  HelpcodePreferences,
  HelpcodeSchema,
  HelpcodeSettings,
  ShuangpinHelpcodeModes,
} from "../helpcode-preferences";

/** 下拉框里辅助码表插件的值前缀：`pack:<插件 id>`。 */
const PACK_PREFIX = "pack:";

const packKeys = {
  quanpin_helpcode: "helpcode_pack_quanpin",
  shuangpin_helpcode: "helpcode_pack_shuangpin",
} as const;

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
  showTriggerKey?: boolean;
  /** 宿主已实现 Tab 手动双拼辅助码。 */
  showManualTab?: boolean;
  disabled?: boolean;
  hidden?: boolean;
  /** Custom tables discovered by the host. They are optional for hosts without resource scanning. */
  customSchemas?: readonly CustomHelpcodeSchema[];
  /** 已安装的辅助码表插件；选中后替换该方案的辅助码方案。没有插件目录的宿主为空。 */
  packs?: readonly HelpcodePackOption[];
  /** 当前方案所用的那一族。HarmonyOS 手机（`mobile`）用紧凑形式，类似 Android 的 `TypingPage`：分组只显示这一族的 辅助码方案，两族的其他控件都移进 更多选项 折叠区。桌面显示这一族的三行，另一族的三行收进 更多选项：另一族的设置对当前方案不起作用。 */
  activeFamily?: HelpcodeKey;
  onImport?: (family: HelpcodeKey) => Promise<void>;
  importBusy?: boolean;
  importStatus?: string;
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
  showTriggerKey = false,
  showManualTab = false,
  customSchemas = [],
  packs = [],
  activeFamily,
  onImport,
  importBusy = false,
  importStatus,
  onChange,
}: HelpcodeSettingsGroupProps) {
  const plugins = pluginPreferences(draft);
  const schemaOptions = [
    ...helpcodeSchemaOptions,
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

  const triggerRow = (key: HelpcodeKey, label: string) => {
    if (!showTriggerKey) return null;
    const current = currentOf(key);
    return (
      <SelectRow
        title={`${label}辅助码触发键`}
        description="组字末尾按所选标点，再输入一到两个辅助码字母。Shift 加字母仍可使用；微软双拼的分号优先补全韵母。"
        disabled={!current.enabled}
        value={current.trigger_key ?? ""}
        onChange={(event) =>
          onChange({
            [key]: {
              ...current,
              trigger_key: event.target.value as HelpcodePreferences["trigger_key"],
            },
          })
        }
      >
        <option value="">Shift + 字母</option>
        {[";", "/", "\\", "`", "="].map((key) => (
          <option key={key} value={key}>
            {key}
          </option>
        ))}
      </SelectRow>
    );
  };

  const importRow = (key: HelpcodeKey, label: string) =>
    onImport && (
      <ActionRow
        title="导入辅助码表"
        description="UTF-8 文本，最大 1 MiB，每行「字=码」，码为一到两个小写字母。"
        label={importBusy ? "导入中…" : "导入…"}
        ariaLabel={`导入${label}辅助码表`}
        ariaBusy={importBusy}
        disabled={importBusy}
        action={() => onImport(key)}
      />
    );
  const importNote = importStatus && (
    <SettingsGroupNote>
      <span role="status">{importStatus}</span>
    </SettingsGroupNote>
  );

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

  /** 双拼专属的辅助码触发方式；三组与 Windows v0.11.1 相同，互斥在写入端归一。 */
  const modesRows = (key: HelpcodeKey, label: string) => {
    if (key !== "shuangpin_helpcode") {
      return null;
    }
    const current = currentOf(key);
    const modes = { ...defaultShuangpinHelpcodeModes, ...current.modes };
    const setModes = (patch: Partial<ShuangpinHelpcodeModes>) =>
      onChange({ [key]: { ...current, modes: { ...modes, ...patch } } });
    const disabled = !current.enabled || (showManualTab && modes.manual_tab);
    return (
      <Fragment>
        {showManualTab && (
          <SwitchRow
            title={`${label}手动辅助码（Tab）`}
            description="按 Tab 后才输入辅助码；关闭后恢复其它触发设置"
            disabled={!current.enabled}
            checked={modes.manual_tab}
            onChange={(manual_tab) => setModes({ manual_tab })}
          />
        )}
        <SwitchRow
          title={`${label}末尾直接辅助码`}
          description="完整音节后直接接辅助码"
          disabled={disabled}
          checked={modes.direct}
          onChange={(direct) => setModes({ direct })}
        />
        <SwitchRow
          title={`${label}末尾间接触发（分号）`}
          description="完整音节后按分号，再输入一到两个辅助码字母；微软双拼的分号优先补全韵母"
          disabled={disabled}
          checked={modes.trigger_semicolon}
          onChange={(trigger_semicolon) => setModes({ trigger_semicolon })}
        />
        <SwitchRow
          title={`${label}末尾间接触发（反引号）`}
          description="完整音节后按反引号，再输入一到两个辅助码字母"
          disabled={disabled}
          checked={modes.trigger_backtick}
          onChange={(trigger_backtick) => setModes({ trigger_backtick })}
        />
        <SwitchRow
          title={`${label}末尾间接触发（大写字母）`}
          description="完整音节后输入大写字母作为辅助码"
          disabled={disabled}
          checked={modes.trigger_uppercase}
          onChange={(trigger_uppercase) => setModes({ trigger_uppercase })}
        />
        <SwitchRow
          title={`${label}句中间接辅助码`}
          description="触发键把它前面的音节标上辅助码，约束带进整句，后面还能接着打"
          disabled={disabled}
          checked={modes.mid_sentence}
          onChange={(mid_sentence) => setModes({ mid_sentence })}
        />
        <SwitchRow
          title={`${label}句中触发键（反引号）`}
          description="用反引号在句中标记辅助码"
          disabled={disabled || !modes.mid_sentence}
          checked={modes.mid_sentence_backtick}
          onChange={(mid_sentence_backtick) => setModes({ mid_sentence_backtick })}
        />
        <SwitchRow
          title={`${label}句中触发键（分号）`}
          description="用分号在句中标记辅助码；微软双拼的分号优先补全韵母"
          disabled={disabled || !modes.mid_sentence}
          checked={modes.mid_sentence_semicolon}
          onChange={(mid_sentence_semicolon) => setModes({ mid_sentence_semicolon })}
        />
        <SwitchRow
          title={`${label}句中触发键（大写字母）`}
          description="完整音节后输入大写字母在句中标记辅助码"
          disabled={disabled || !modes.mid_sentence}
          checked={modes.mid_sentence_uppercase}
          onChange={(mid_sentence_uppercase) => setModes({ mid_sentence_uppercase })}
        />
        <SwitchRow
          title={`${label}逐音节直接辅助码`}
          description="不要引导键，逐音节接辅码；三码自动识别，四码单独敲时直接定字"
          disabled={disabled}
          checked={modes.direct_helpcode}
          onChange={(direct_helpcode) => setModes({ direct_helpcode })}
        />
        <SwitchRow
          title={`${label}逐音节直接（/ 结束）`}
          description="用 / 结束一位或两位辅码"
          disabled={disabled || !modes.direct_helpcode}
          checked={modes.direct_helpcode_slash}
          onChange={(direct_helpcode_slash) => setModes({ direct_helpcode_slash })}
        />
        <SwitchRow
          title={`${label}逐音节直接（第二码大写结束）`}
          description="第二位辅码敲大写来结束一段"
          disabled={disabled || !modes.direct_helpcode}
          checked={modes.direct_helpcode_uppercase}
          onChange={(direct_helpcode_uppercase) => setModes({ direct_helpcode_uppercase })}
        />
      </Fragment>
    );
  };

  const shiftNote = showShiftEntry && (
    <SettingsGroupNote>
      全拼或双拼组字时，按 Shift
      再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、日语、韩语、粤拼、注音、笔画、越南语、藏文和快捷模式不使用辅助码。
    </SettingsGroupNote>
  );

  if (activeFamily && !mobile) {
    const activeLabel = activeFamily === "shuangpin_helpcode" ? "双拼" : "全拼";
    return (
      <GroupList title="辅助码">
        {shiftNote}
        {enabledRow(activeFamily, activeLabel)}
        {schemaRow(activeFamily, activeLabel, `${activeLabel}辅助码方案`)}
        {importRow(activeFamily, activeLabel)}
        {importNote}
        {triggerRow(activeFamily, activeLabel)}
        {displayRow(activeFamily, activeLabel)}
        {modesRows(activeFamily, activeLabel)}
        <MoreOptions>
          {families
            .filter(([key]) => key !== activeFamily)
            .map(([key, label]) => (
              <Fragment key={key}>
                {enabledRow(key, label)}
                {schemaRow(key, label, `${label}辅助码方案`)}
                {importRow(key, label)}
                {triggerRow(key, label)}
                {displayRow(key, label)}
              </Fragment>
            ))}
        </MoreOptions>
      </GroupList>
    );
  }

  if (activeFamily) {
    const activeLabel = activeFamily === "shuangpin_helpcode" ? "双拼" : "全拼";
    return (
      <GroupList title="辅助码">
        {schemaRow(activeFamily, activeLabel, "辅助码方案", activeLabel)}
        {importRow(activeFamily, activeLabel)}
        {importNote}
        <MoreOptions>
          {shiftNote}
          {families.map(([key, label]) => (
            <Fragment key={key}>
              {enabledRow(key, label)}
              {key !== activeFamily && schemaRow(key, label, `${label}辅助码方案`)}
              {triggerRow(key, label)}
              {displayRow(key, label)}
              {modesRows(key, label)}
            </Fragment>
          ))}
        </MoreOptions>
      </GroupList>
    );
  }

  return (
    <GroupList title="辅助码">
      {shiftNote}
      {importNote}
      {/* 全拼在前，和输入方案选择器「全拼、双拼」的顺序一致。 */}
      {families.map(([key, label]) => (
        <Fragment key={key}>
          {enabledRow(key, label)}
          {schemaRow(key, label, `${label}辅助码方案`)}
          {importRow(key, label)}
          {triggerRow(key, label)}
          {displayRow(key, label)}
          {modesRows(key, label)}
        </Fragment>
      ))}
    </GroupList>
  );
}
