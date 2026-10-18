import type { ReactNode } from "react";
import { MascotIllustration } from "../core/mascot";
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
  // 社区读不到（多半是网络）时画断网的水杉娘，列表为空时画找不到东西的水杉娘；要求登录不算出错，不配图。
  const mascot = error && !signInRequired ? "offline" : !error && empty ? "empty" : null;
  return (
    <>
      {mascot && <MascotIllustration scene={mascot} className="mx-auto mt-4 block" />}
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      {notice}
      {!error && empty}
    </>
  );
}
