import type { ReactNode } from "react";
import { ActionButton } from "../core/action-button";
import { CommunityRatingButtons } from "./community-rating-buttons";
import { CommunityUnpublishConfirmation } from "./community-unpublish-confirmation";

export interface CommunitySkinModerationSectionProps {
  owned: boolean;
  actionBusy: boolean;
  ratingDescription: string;
  unpublishMessage: ReactNode;
  unpublishDisabled?: boolean;
  /** Whether the owner can take the skin down; a private candidate skin is only in its owner's library, so there is nothing to take down. */
  unpublishable?: boolean;
  confirmUnpublish: boolean;
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  confirmationActionsClassName?: string;
  /** 下架按钮的完整样式类名；资源详情页用它保留自己的间距样式。 */
  unpublishButtonClassName?: string;
  /** The take-down button's text, for a gallery of something other than skins. */
  unpublishLabel?: string;
  /** The take-down confirmation's accessible name. */
  unpublishConfirmLabel?: string;
}

/** 社区皮肤、插件和资源详情共用的评分与作者作品下架控件。 */
export function CommunitySkinModerationSection({
  owned,
  actionBusy,
  ratingDescription,
  unpublishMessage,
  unpublishDisabled = false,
  unpublishable = true,
  confirmUnpublish,
  onRate,
  onRequestUnpublish,
  onUnpublish,
  onCancelUnpublish,
  confirmationActionsClassName,
  unpublishButtonClassName = "danger-text pt-0",
  unpublishLabel = "下架这款皮肤",
  unpublishConfirmLabel = "确认下架皮肤",
}: CommunitySkinModerationSectionProps) {
  return (
    <>
      {!owned && (
        <CommunityRatingButtons
          description={ratingDescription}
          disabled={actionBusy}
          onRate={onRate}
        />
      )}
      {owned && unpublishable && (
        <ActionButton
          action={onRequestUnpublish}
          className={unpublishButtonClassName}
          disabled={actionBusy || unpublishDisabled}
          label={unpublishLabel}
        />
      )}
      {confirmUnpublish && (
        <CommunityUnpublishConfirmation
          ariaLabel={unpublishConfirmLabel}
          message={unpublishMessage}
          actionBusy={actionBusy}
          onConfirm={onUnpublish}
          onCancel={onCancelUnpublish}
          actionsClassName={confirmationActionsClassName}
        />
      )}
    </>
  );
}
