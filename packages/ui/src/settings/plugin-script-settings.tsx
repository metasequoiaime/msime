import { useState } from "react";
import { GroupList } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { SCRIPT_SETTINGS_NOTE, settingIssue, settingValue } from "./plugin-script-model";
import type { ScriptSetting, ScriptSettingValue } from "./plugin-types";
import { SelectRow } from "./select-row";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SwitchRow } from "./switch-row";
import { TextInputRow } from "./text-input-row";
import type { ScriptSettingsFailure } from "./use-script-plugin-state";

export interface ScriptSettingsFormProps {
  settings: readonly ScriptSetting[];
  /** 宿主读到的值（`ScriptPluginState.values`）。 */
  values: Readonly<Record<string, ScriptSettingValue>>;
  disabled: boolean;
  /** 整份保存；失败时抛出 `ScriptSettingsFailure`。 */
  onSave: (values: Record<string, ScriptSettingValue>) => Promise<void>;
}

/** 草稿里的值：数字项存输入框里的原文，保存时再换成整数，免得打到一半的「-」被吞掉。 */
type Draft = Record<string, ScriptSettingValue>;

function draftFrom(
  settings: readonly ScriptSetting[],
  values: Readonly<Record<string, ScriptSettingValue>>,
): Draft {
  return Object.fromEntries(
    settings.map((setting) => {
      const value = settingValue(setting, values);
      return [setting.key, setting.type === "number" ? String(value) : value];
    }),
  );
}

/** 交给宿主的值：数字项的原文是整数时换成数字，否则原样交给 `settingIssue` 报错。 */
function parsed(setting: ScriptSetting, raw: ScriptSettingValue): ScriptSettingValue {
  if (setting.type !== "number" || typeof raw !== "string") return raw;
  const trimmed = raw.trim();
  return /^-?\d+$/.test(trimmed) ? Number(trimmed) : Number.NaN;
}

/** 插件清单 `[[script.settings]]` 生成的设置表单：开关、下拉、整数和文本框，改完按「保存」整份写入。宿主拒绝某一项时把原因写在那一项下面。 */
export function ScriptSettingsForm({
  settings,
  values,
  disabled,
  onSave,
}: ScriptSettingsFormProps) {
  const [draft, setDraft] = useState<Draft>(() => draftFrom(settings, values));
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<ScriptSettingsFailure | null>(null);
  const [saved, setSaved] = useState(false);
  const next = Object.fromEntries(
    settings.map((setting) => [setting.key, parsed(setting, draft[setting.key])]),
  );
  const issues = Object.fromEntries(
    settings.map((setting) => [setting.key, settingIssue(setting, next[setting.key])]),
  );
  const dirty = settings.some((setting) => next[setting.key] !== settingValue(setting, values));
  const blocked = settings.some((setting) => issues[setting.key] !== null);
  const change = (key: string, value: ScriptSettingValue) => {
    setDraft((current) => ({ ...current, [key]: value }));
    setFailure(null);
    setSaved(false);
  };
  const save = async () => {
    setBusy(true);
    setFailure(null);
    try {
      await onSave(next);
      setSaved(true);
    } catch (error) {
      setFailure(error as ScriptSettingsFailure);
    } finally {
      setBusy(false);
    }
  };
  const describe = (setting: ScriptSetting) => {
    const error = issues[setting.key] ?? (failure?.key === setting.key ? failure.message : null);
    if (!error) return setting.description;
    return (
      <>
        {setting.description && <span className="block">{setting.description}</span>}
        <span className="block text-danger" role="alert">
          {error}
        </span>
      </>
    );
  };
  const locked = disabled || busy;
  return (
    <GroupList title="设置">
      <SettingsGroupNote>{SCRIPT_SETTINGS_NOTE}</SettingsGroupNote>
      {settings.map((setting) => {
        const value = draft[setting.key];
        switch (setting.type) {
          case "bool":
            return (
              <SwitchRow
                key={setting.key}
                title={setting.title}
                description={describe(setting)}
                checked={value === true}
                disabled={locked}
                onChange={(checked) => change(setting.key, checked)}
              />
            );
          case "choice":
            return (
              <SelectRow
                key={setting.key}
                title={setting.title}
                description={describe(setting)}
                aria-label={setting.title}
                value={String(value)}
                disabled={locked}
                onChange={(event) => change(setting.key, event.currentTarget.value)}
              >
                {setting.options.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.title}
                  </option>
                ))}
              </SelectRow>
            );
          case "number":
            return (
              <TextInputRow
                key={setting.key}
                title={setting.title}
                description={describe(setting)}
                label={setting.title}
                type="number"
                inputMode="numeric"
                min={setting.min}
                max={setting.max}
                step={1}
                value={String(value)}
                disabled={locked}
                onChange={(raw) => change(setting.key, raw)}
              />
            );
          case "text":
            return (
              <TextInputRow
                key={setting.key}
                title={setting.title}
                description={describe(setting)}
                label={setting.title}
                maxLength={setting.max_length}
                value={String(value)}
                disabled={locked}
                onChange={(text) => change(setting.key, text)}
              />
            );
        }
      })}
      <SettingsManagerBlock>
        {failure && failure.key === null && (
          <SettingsGroupNote inline className="text-danger">
            {failure.message}
          </SettingsGroupNote>
        )}
        {saved && !dirty && <SettingsGroupNote role="status">已保存。</SettingsGroupNote>}
        <SettingsManagerActions>
          <ActionButton
            action={save}
            disabled={locked || !dirty || blocked}
            ariaBusy={busy}
            label="保存设置"
          />
        </SettingsManagerActions>
      </SettingsManagerBlock>
    </GroupList>
  );
}
