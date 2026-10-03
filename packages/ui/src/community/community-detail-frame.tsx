import type { ReactNode } from "react";
import { CommunityBackButton } from "./community-gallery-controls";
import { CommunityErrorAlert } from "./community-error-alert";
import { CommunityPageShell } from "./community-page-shell";
import * as style from "./community-style";

export interface CommunityDetailFrameProps {
  backDisabled: boolean;
  onBack: () => void;
  backAriaLabel?: string;
  error?: string;
  signInRequired?: boolean;
  onLogin?: () => void;
  children: ReactNode;
}

/** Shared navigation, error and surface shell for community gallery detail views. */
export function CommunityDetailFrame({
  backDisabled,
  onBack,
  backAriaLabel,
  error,
  signInRequired = false,
  onLogin,
  children,
}: CommunityDetailFrameProps) {
  return (
    <CommunityPageShell>
      <CommunityBackButton disabled={backDisabled} onClick={onBack} ariaLabel={backAriaLabel} />
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      <section className={`section ${style.detail}`}>{children}</section>
    </CommunityPageShell>
  );
}
