import { Fragment } from "react";
import * as settings from "../settings-style";
import { GroupList, Row, Select, Switch } from "../../core/platform-controls";

export type HelpcodeSchema =
  | "lantian"
  | "ziranma"
  | "shouyou2_0"
  | "shouyouplus"
  | "xiaohe"
  | "jiajia"
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
export type HelpcodeSettings = Partial<Record<HelpcodeKey, HelpcodePreferences>>;

/** The core's `default_quanpin_helpcode` / `default_shuangpin_helpcode`, used wherever a document carries no helpcode object for a scheme: this form and the candidate previews. */
export const defaultHelpcode: Readonly<Record<HelpcodeKey, HelpcodePreferences>> = {
  quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
  shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
};

const helpcodeSchemas: readonly (readonly [HelpcodeSchema, string])[] = [
  ["lantian", "蓝天小雨点"],
  ["ziranma", "自然码"],
  ["shouyou2_0", "首右2.0"],
  ["shouyouplus", "首右plus"],
  ["xiaohe", "小鹤"],
  ["jiajia", "加加"],
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
  onChange: (patch: HelpcodeSettings) => void;
}

/** The shared helper-code settings form used by desktop and mobile hosts: the 辅助码 group, shown on the 输入 page (the former `helpcode` route opens that page). */
export function HelpcodeSettingsPage({
  value: draft,
  mobile,
  showShiftEntry,
  disabled = false,
  hidden = false,
  customSchemas = [],
  onChange,
}: HelpcodeSettingsPageProps) {
  const schemaOptions = [
    ...helpcodeSchemas,
    ...customSchemas.map((schema): readonly [HelpcodeSchema, string] => [
      schema.schema,
      customSchemaLabel(schema),
    ]),
  ];

  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="辅助码">
      <div className={settings.groups}>
        <GroupList title="辅助码">
          {showShiftEntry && (
            <p className={settings.groupNote}>
              全拼或双拼组字时，按 Shift
              再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、日语和本地输入模式不使用辅助码。
            </p>
          )}
          {(
            [
              ["shuangpin_helpcode", "双拼"],
              ["quanpin_helpcode", "全拼"],
            ] as const
          ).map(([key, label]) => {
            const current = {
              ...defaultHelpcode[key],
              ...draft[key],
            } as Required<HelpcodePreferences>;
            // Named after its own scheme, the way the reference window names these: both rows are on the page at once, so one shared wording left two switches with the same accessible name and nothing to tell a screen reader -- or a test -- which one it had.
            const display = mobile
              ? `在候选栏中显示${label}辅助码`
              : `在候选窗口中显示${label}辅助码`;
            return (
              <Fragment key={key}>
                <Row title={`${label}辅助码`}>
                  <Switch
                    checked={current.enabled}
                    onChange={(enabled) => onChange({ [key]: { ...current, enabled } })}
                  />
                </Row>
                <Row title={`${label}辅助码方案`}>
                  <Select
                    disabled={!current.enabled}
                    value={current.schema}
                    onChange={(event) =>
                      onChange({
                        [key]: { ...current, schema: event.target.value as HelpcodeSchema },
                      })
                    }
                  >
                    {schemaOptions
                      // Keep a previously selected table visible if the resource directory was
                      // changed or is temporarily unavailable during settings startup.
                      .concat(
                        current.schema.startsWith("custom/") &&
                          !schemaOptions.some(([schema]) => schema === current.schema)
                          ? [[current.schema, current.schema] as const]
                          : [],
                      )
                      .map(([schema, name]) => (
                        <option key={schema} value={schema}>
                          {name}
                        </option>
                      ))}
                  </Select>
                </Row>
                <Row title={display}>
                  <Switch
                    checked={current.show_in_candidate_window}
                    onChange={(show_in_candidate_window) =>
                      onChange({ [key]: { ...current, show_in_candidate_window } })
                    }
                  />
                </Row>
              </Fragment>
            );
          })}
        </GroupList>
      </div>
    </fieldset>
  );
}
