import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunitySelectFieldProps {
  label: ReactNode;
  ariaLabel: string;
  value: string;
  disabled?: boolean;
  children: ReactNode;
  onChange: (value: string) => void;
}

/** Shared styled select field used by community forms. */
export function CommunitySelectField({
  label,
  ariaLabel,
  value,
  disabled,
  children,
  onChange,
}: CommunitySelectFieldProps) {
  return (
    <label className={style.field}>
      {label}
      <select
        className={style.fieldControl}
        aria-label={ariaLabel}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      >
        {children}
      </select>
    </label>
  );
}
