import type { ReactNode } from "react";
import { CommunityErrorAlert } from "./community-error-alert";

export interface CommunityGalleryFeedbackProps {
  error?: string;
  signInRequired?: boolean;
  onLogin?: () => void;
  notice?: ReactNode;
  empty?: ReactNode;
}

/** Shared error, action notice and empty-state ordering for community galleries. */
export function CommunityGalleryFeedback({
  error,
  signInRequired = false,
  onLogin,
  notice,
  empty,
}: CommunityGalleryFeedbackProps) {
  return (
    <>
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      {notice}
      {!error && empty}
    </>
  );
}
