import type { ReactNode, TextareaHTMLAttributes } from "react";
import * as settings from "./settings-style";

export interface SettingsTextareaFieldProps extends Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  "value" | "onChange" | "aria-label"
> {
  label: ReactNode;
  description?: ReactNode;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
}

/** Shared labeled textarea field used by grouped settings forms. */
export function SettingsTextareaField({
  label,
  description,
  ariaLabel,
  value,
  onChange,
  className = settings.promptInput,
  ...textareaProps
}: SettingsTextareaFieldProps) {
  return (
    <label className={settings.field}>
      <span>
        <span data-row-title="">{label}</span>
        {description !== undefined && <> {description}</>}
      </span>
      <textarea
        {...textareaProps}
        className={className}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
