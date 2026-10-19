import { createContext, useContext, type ReactNode } from "react";
import type { SettingsPageId } from "./settings-page-registry";
import * as settings from "./settings-style";

/** 标明偏好表单归属，供设置项搜索定位；不生成额外布局。 */
export const SettingsSearchPage = createContext<SettingsPageId | undefined>(undefined);

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
  const page = useContext(SettingsSearchPage);
  return (
    <fieldset
      data-settings-page={page}
      disabled={disabled}
      hidden={hidden}
      aria-label={ariaLabel}
      className={className}
    >
      <div className={settings.groups}>{children}</div>
    </fieldset>
  );
}
