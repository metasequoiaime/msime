import type { ReactNode } from "react";
import { SettingSectionTitle } from "./setting-section-title";

export interface SettingFieldProps {
  label: ReactNode;
  /** 解释：收进标题旁的「?」（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  description?: ReactNode;
  /** 状态、校验或错误：常驻标题下方。 */
  detail?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** Shared labeled field row used by settings sections with a control on the right. */
export function SettingField({
  label,
  description,
  detail,
  children,
  className = "section-header",
}: SettingFieldProps) {
  return (
    <label className={className}>
      {detail ? (
        <span className="flex min-w-0 flex-col">
          <SettingSectionTitle title={label} description={description} />
          <span className="input-setting-description">{detail}</span>
        </span>
      ) : (
        <SettingSectionTitle title={label} description={description} />
      )}
      {children}
    </label>
  );
}
