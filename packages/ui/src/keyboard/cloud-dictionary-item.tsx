import type { ReactNode } from "react";
import * as cloud from "./cloud-panel-style";

export interface CloudDictionaryItemProps {
  ariaLabel: string;
  busy?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
  actions?: ReactNode;
}

/** Shared interactive item shell used by cloud dictionary entries and candidates. */
export function CloudDictionaryItem({
  ariaLabel,
  busy = false,
  disabled = false,
  onClick,
  children,
  actions,
}: CloudDictionaryItemProps) {
  return (
    <article className={cloud.dictionaryItem}>
      <button
        type="button"
        className={cloud.dictionaryItemMain}
        aria-label={ariaLabel}
        onClick={onClick}
        disabled={busy || disabled}
      >
        {children}
      </button>
      {actions && <div className={cloud.dictionaryItemActions}>{actions}</div>}
    </article>
  );
}
