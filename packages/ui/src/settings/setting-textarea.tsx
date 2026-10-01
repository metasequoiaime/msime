import type { ReactNode } from "react";

export interface SettingTextareaProps {
  label: ReactNode;
  description?: ReactNode;
  ariaLabel: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}

/** Shared stacked textarea field used by prompt and other multiline settings. */
export function SettingTextarea({
  label,
  description,
  ariaLabel,
  value,
  placeholder,
  onChange,
}: SettingTextareaProps) {
  return (
    <div className="section">
      <label className="section-title">
        {label}
        {description !== undefined && <small>{description}</small>}
      </label>
      <textarea
        aria-label={ariaLabel}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
