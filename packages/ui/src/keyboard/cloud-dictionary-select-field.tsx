import type { ReactNode } from "react";

export interface CloudDictionarySelectFieldProps {
  label: ReactNode;
  ariaLabel: string;
  value: string;
  disabled?: boolean;
  labelClassName?: string;
  selectClassName?: string;
  onChange: (value: string) => void;
  children?: ReactNode;
}

/** Shared labeled select field used by cloud dictionary panels. */
export function CloudDictionarySelectField({
  label,
  ariaLabel,
  value,
  disabled = false,
  labelClassName,
  selectClassName,
  onChange,
  children,
}: CloudDictionarySelectFieldProps) {
  return (
    <label className={labelClassName}>
      {label}
      <select
        className={selectClassName}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        disabled={disabled}
      >
        {children}
      </select>
    </label>
  );
}
