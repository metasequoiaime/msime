import type { FormEventHandler, ReactNode } from "react";

export interface SettingsFormFrameProps {
  children: ReactNode;
  onSubmit: FormEventHandler<HTMLFormElement>;
  showReload: boolean;
  busy: boolean;
  onReload?: () => void | Promise<void>;
}

/** Owns the shared settings form boundary and its reload control. */
export function SettingsFormFrame({
  children,
  onSubmit,
  showReload,
  busy,
  onReload,
}: SettingsFormFrameProps) {
  return (
    <>
      <form onSubmit={onSubmit}>{children}</form>
      {showReload && onReload && (
        <button type="button" className="secondary" disabled={busy} onClick={onReload}>
          重新读取
        </button>
      )}
    </>
  );
}
