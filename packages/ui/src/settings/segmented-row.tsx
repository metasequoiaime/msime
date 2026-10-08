import type { ReactNode } from "react";
import { PickerRow, Row, Segmented, textOf } from "../core/platform-controls";
import { useOptionalSettingsForm } from "./settings-form-context";

export interface SegmentedRowOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface SegmentedRowProps<T extends string> {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  options: readonly SegmentedRowOption<T>[];
  value: T;
  disabled?: boolean;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  onChange: (value: T) => void;
}

/** 呈现分段单选控件的设置行；在 HarmonyOS 手机上改为整行点开选择面板。 */
export function SegmentedRow<T extends string>({
  title,
  description,
  hidden,
  options,
  value,
  disabled,
  onChange,
  ...labels
}: SegmentedRowProps<T>) {
  const form = useOptionalSettingsForm();
  if (form?.settingsPlatform === "harmony") {
    const current = options.find((option) => option.value === value);
    return (
      <PickerRow
        title={title}
        description={description}
        hidden={hidden}
        disabled={disabled}
        valueLabel={current ? textOf(current.label) : ""}
        options={options.map((option) => ({
          value: option.value,
          label: textOf(option.label),
          selected: option.value === value,
          disabled: option.disabled,
        }))}
        onSelect={(next) => {
          const chosen = options.find((option) => option.value === next);
          if (chosen && chosen.value !== value) onChange(chosen.value);
        }}
        control={(titleId) => (
          <Segmented
            options={options}
            value={value}
            disabled={disabled}
            onChange={onChange}
            {...labels}
            aria-labelledby={
              labels["aria-labelledby"] ?? (labels["aria-label"] ? undefined : titleId)
            }
          />
        )}
      />
    );
  }
  return (
    <Row title={title} description={description} hidden={hidden}>
      <Segmented
        options={options}
        value={value}
        disabled={disabled}
        onChange={onChange}
        {...labels}
      />
    </Row>
  );
}
