import type { ReactNode } from "react";
import { ModelSelect } from "./model-select";
import { SelectRow } from "./select-row";
import { SettingField } from "./setting-field";
import { useOptionalSettingsForm } from "./settings-form-context";

export interface ModelSettingFieldProps {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  models: readonly string[];
  model: string;
  emptyLabel: string;
  onSelect: (model: string) => void;
}

/** 共用的预置模型选择。HarmonyOS 手机上是一个点开选择面板的选择行，与同组其他选择一致；其他平台保持原来由所在卡片排版的带标签字段。 */
export function ModelSettingField({
  label,
  inputLabel,
  description,
  models,
  model,
  emptyLabel,
  onSelect,
}: ModelSettingFieldProps) {
  const form = useOptionalSettingsForm();
  if (form?.settingsPlatform === "harmony") {
    return (
      <SelectRow
        title={label}
        description={description}
        aria-label={inputLabel}
        value={models.includes(model) ? model : ""}
        onChange={(event) => {
          if (event.target.value) onSelect(event.target.value);
        }}
      >
        <option value="">{emptyLabel}</option>
        {models.map((entry) => (
          <option key={entry} value={entry}>
            {entry}
          </option>
        ))}
      </SelectRow>
    );
  }
  return (
    <SettingField label={label} description={description}>
      <ModelSelect
        models={models}
        model={model}
        ariaLabel={inputLabel}
        emptyLabel={emptyLabel}
        onSelect={onSelect}
      />
    </SettingField>
  );
}
