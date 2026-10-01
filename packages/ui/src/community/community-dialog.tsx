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
