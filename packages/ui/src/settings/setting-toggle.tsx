import type { ReactNode } from "react";
import { ControlledCheckbox } from "../core/controlled-checkbox";
import { SettingField } from "./setting-field";

export interface SettingToggleProps {
  label: ReactNode;
  description?: ReactNode;
  /** 状态或前置条件（「登录后可用」），常驻标题下方。 */
  detail?: ReactNode;
  checked: boolean;
  ariaLabel?: string;
  disabled?: boolean;
  rowClassName?: string;
  /** Render only the labeled row when the toggle is embedded in another section. */
  compact?: boolean;
  onChange: (checked: boolean) => void;
}

/** Shared labeled checkbox row for settings that toggle one preference. */
export function SettingToggle({
  label,
  description,
  detail,
  checked,
  ariaLabel,
  disabled,
  rowClassName,
  compact = false,
  onChange,
}: SettingToggleProps) {
  const row = (
    <SettingField
      label={label}
      description={description}
      detail={detail}
      className={`section-header${rowClassName ? ` ${rowClassName}` : ""}`}
    >
      <ControlledCheckbox
        className="toggle"
        aria-label={ariaLabel}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
    </SettingField>
  );
  return compact ? row : <div className="section">{row}</div>;
}
