import type { ReactNode, Ref } from "react";
import { InfoTip } from "../core/info-tip";
import * as settings from "./settings-style";

export interface SettingsSubViewBackProps {
  label: string;
  ariaLabel: string;
  onClick: () => void;
  disabled?: boolean;
  ref?: Ref<HTMLButtonElement>;
  /** 可选的 DOM 标记，供返回后焦点恢复使用。 */
  marker?: string;
}

export interface SettingsSubViewHeaderProps {
  title: string;
  /** 这一页是做什么的、怎么用：收进标题旁的「?」，不在标题下常驻一段（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  note?: ReactNode;
  back?: SettingsSubViewBackProps;
}

/** 设置子视图共用的标题栏；返回历史和确认逻辑仍由调用方提供。 */
export function SettingsSubViewHeader({ title, note, back }: SettingsSubViewHeaderProps) {
  const markerProps = back?.marker ? { [back.marker]: "" } : {};
  return (
    <div className={settings.subViewHeader}>
      {back && (
        <button
          ref={back.ref}
          type="button"
          className={settings.backLink}
          aria-label={back.ariaLabel}
          disabled={back.disabled}
          onClick={back.onClick}
          {...markerProps}
        >
          <span aria-hidden="true">‹ </span>
          {back.label}
        </button>
      )}
      {note ? (
        <div className="flex min-w-0 items-center gap-1">
          <h2 className={settings.subViewTitle}>{title}</h2>
          <InfoTip subject={title}>{note}</InfoTip>
        </div>
      ) : (
        <h2 className={settings.subViewTitle}>{title}</h2>
      )}
    </div>
  );
}
