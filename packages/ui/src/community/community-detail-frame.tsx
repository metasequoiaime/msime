import type { ReactNode } from "react";
import { CommunityBackButton } from "./community-gallery-controls";
import { CommunityErrorAlert } from "./community-error-alert";
import * as style from "./community-style";

export interface CommunityDetailFrameProps {
  backDisabled: boolean;
  onBack: () => void;
  error?: string;
  signInRequired?: boolean;
  onLogin?: () => void;
  children: ReactNode;
}

/** Shared navigation, error and surface shell for community gallery detail views. */
export function CommunityDetailFrame({
  backDisabled,
  onBack,
  error,
  signInRequired = false,
  onLogin,
  children,
}: CommunityDetailFrameProps) {
  return (
    <div className={style.page}>
      <CommunityBackButton disabled={backDisabled} onClick={onBack} />
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      <section className={`section ${style.detail}`}>{children}</section>
    </div>
  );
}
