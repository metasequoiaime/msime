import type { ReactNode } from "react";
import { ActionButton } from "./action-button";

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
        <ActionButton action={onReload} disabled={busy} label="重新读取" />
      )}
    </>
  );
}
