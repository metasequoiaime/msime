import type { ReactNode } from "react";
import * as settings from "../settings/settings-style";

export interface SkinCardHeaderProps {
  title: ReactNode;
  theme: "dark" | "light";
  selected: boolean;
  /** 标题下的一行说明；不传就不占这一行。 */
  description?: ReactNode;
  details?: ReactNode;
  actions: ReactNode;
}

/** Shared header layout for built-in and external skin cards. */
export function SkinCardHeader({
  title,
  theme,
  selected,
  description,
  details,
  actions,
}: SkinCardHeaderProps) {
  return (
    <div className={settings.skinCardHeader} data-skin-card-header="">
      <div className={settings.skinCardBody}>
        <span className={settings.skinCardTitle}>
          {title}（{theme === "dark" ? "深色" : "浅色"}）
          {selected && <span className={settings.skinCardInUse}>使用中</span>}
        </span>
        {description && <span className={settings.skinCardDescription}>{description}</span>}
        {details}
      </div>
      <div className={settings.skinCardActions}>{actions}</div>
    </div>
  );
}
