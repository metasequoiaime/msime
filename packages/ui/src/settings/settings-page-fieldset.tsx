import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsPageFieldsetProps {
  disabled: boolean;
  hidden: boolean;
  ariaLabel: string;
  className?: string;
  children: ReactNode;
}

/** 设置页共享的禁用、隐藏与分组容器。 */
export function SettingsPageFieldset({
  disabled,
  hidden,
  ariaLabel,
  className,
  children,
}: SettingsPageFieldsetProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label={ariaLabel} className={className}>
      <div className={settings.groups}>{children}</div>
    </fieldset>
  );
}
