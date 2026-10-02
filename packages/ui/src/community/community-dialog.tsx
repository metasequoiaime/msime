import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityDialogHeaderProps {
  title: string;
  busy?: boolean;
  onClose: () => void;
  titleClassName?: string;
}

/** Shared heading and close control used by community dialogs. */
export function CommunityDialogHeader({
  title,
  busy = false,
  onClose,
  titleClassName,
}: CommunityDialogHeaderProps) {
  return (
    <div className={style.dialogHeading}>
      <h2 className={titleClassName}>{title}</h2>
      <button
        type="button"
        className={style.dialogClose}
        disabled={busy}
        onClick={onClose}
        aria-label="关闭发布窗口"
      >
        ×
      </button>
    </div>
  );
}

export interface CommunityDialogActionsProps {
  busy?: boolean;
  onClose: () => void;
  children: ReactNode;
}

/** Shared footer layout and cancel control used by community dialogs. */
export function CommunityDialogActions({
  busy = false,
  onClose,
  children,
}: CommunityDialogActionsProps) {
  return (
    <div className={style.dialogActions}>
      <button type="button" className="secondary" disabled={busy} onClick={onClose}>
        取消
      </button>
      {children}
    </div>
  );
}
