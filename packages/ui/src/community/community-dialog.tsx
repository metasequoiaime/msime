import type { FormEvent, KeyboardEvent, ReactNode } from "react";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";
import { CommunityErrorAlert } from "./community-error-alert";

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

export interface CommunityDialogFrameProps {
  title: string;
  ariaLabel: string;
  busy?: boolean;
  onClose: () => void;
  error?: string;
  signInRequired?: boolean;
  onLogin?: () => void;
  titleClassName?: string;
  children: ReactNode;
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void;
}

/** Shared backdrop, dialog header, and error area used by community dialogs. */
export function CommunityDialogFrame({
  title,
  ariaLabel,
  busy = false,
  onClose,
  error,
  signInRequired = false,
  onLogin,
  titleClassName,
  children,
  onSubmit,
  onKeyDown,
}: CommunityDialogFrameProps) {
  const content = (
    <>
      <CommunityDialogHeader
        title={title}
        titleClassName={titleClassName}
        busy={busy}
        onClose={onClose}
      />
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      {children}
    </>
  );

  return (
    <div className={style.backdrop}>
      {onSubmit ? (
        <form
          className={style.dialog}
          role="dialog"
          aria-modal="true"
          aria-label={ariaLabel}
          onSubmit={onSubmit}
          onKeyDown={onKeyDown}
        >
          {content}
        </form>
      ) : (
        <div
          className={style.dialog}
          role="dialog"
          aria-modal="true"
          aria-label={ariaLabel}
          onKeyDown={onKeyDown}
        >
          {content}
        </div>
      )}
    </div>
  );
}
