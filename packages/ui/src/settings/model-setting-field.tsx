import type { ReactNode } from "react";
import { ModelSelect } from "./model-select";
import { SettingField } from "./setting-field";

export interface ModelSettingFieldProps {
  label: ReactNode;
  inputLabel: string;
  description?: ReactNode;
  models: readonly string[];
  model: string;
  emptyLabel: string;
  onSelect: (model: string) => void;
}

/** Shared labeled model catalog select for legacy settings fields. */
export function ModelSettingField({
  label,
  inputLabel,
  description,
  models,
  model,
  emptyLabel,
  onSelect,
}: ModelSettingFieldProps) {
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
