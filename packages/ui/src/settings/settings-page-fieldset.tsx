import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsPageFieldsetProps {
  disabled: boolean;
  hidden: boolean;
  ariaLabel: string;
  children: ReactNode;
}

/** Shared disabled, hidden and grouped shell for settings pages. */
export function SettingsPageFieldset({
  disabled,
  hidden,
  ariaLabel,
  children,
}: SettingsPageFieldsetProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label={ariaLabel}>
      <div className={settings.groups}>{children}</div>
    </fieldset>
  );
}
