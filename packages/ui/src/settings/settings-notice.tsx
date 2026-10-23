import type { ReactNode } from "react";

export interface SettingsNoticeProps {
  children: ReactNode;
  role?: "status";
  /** 有它时提示行尾带一个关闭按钮，点了就收起这条提示。 */
  onDismiss?: () => void;
}

/** Shared notice styling for settings messages. */
export function SettingsNotice({ children, role, onDismiss }: SettingsNoticeProps) {
  if (!onDismiss) {
    return (
      <p className="notice" role={role}>
        {children}
      </p>
    );
  }
  return (
    <div className="notice flex items-start gap-2" role={role}>
      <p className="m-0 min-w-0 flex-1">{children}</p>
      <button
        type="button"
        className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded border-0 bg-transparent p-0 text-[var(--text-secondary)] hover:bg-[var(--surface-raised)]"
        aria-label="关闭提示"
        onClick={onDismiss}
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.6"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </button>
    </div>
  );
}
