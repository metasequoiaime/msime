import type { ReactNode } from "react";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

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
      <ActionButton
        action={onClose}
        className={style.dialogClose}
        disabled={busy}
        ariaLabel="关闭发布窗口"
        label="×"
      />
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
      <ActionButton action={onClose} disabled={busy} label="取消" />
      {children}
    </div>
  );
}
