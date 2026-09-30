import type { ReactNode } from "react";

export interface SettingsFormFrameProps {
  children: ReactNode;
  showReload: boolean;
  busy: boolean;
  onReload?: () => void | Promise<void>;
}

/** Owns the shared settings form boundary and its reload control. */
export function SettingsFormFrame({
  children,
  showReload,
  busy,
  onReload,
}: SettingsFormFrameProps) {
  return (
    <>
      {/* Changes save themselves, so submitting does nothing: Enter in a lone text field would otherwise submit and navigate the page. */}
      <form aria-label="设置" onSubmit={(event) => event.preventDefault()}>
        {children}
      </form>
      {showReload && onReload && (
        <button type="button" className="secondary" disabled={busy} onClick={onReload}>
          重新读取
        </button>
      )}
    </>
  );
}
