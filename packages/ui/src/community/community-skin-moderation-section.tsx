import * as style from "./community-style";
import { ActionButton } from "../core/action-button";
import { CommunityRatingButtons } from "./community-rating-buttons";

export interface CommunitySkinModerationSectionProps {
  owned: boolean;
  actionBusy: boolean;
  ratingDescription: string;
  unpublishMessage: string;
  unpublishDisabled?: boolean;
  /** Whether the owner can take the skin down; a private candidate skin is only in its owner's library, so there is nothing to take down. */
  unpublishable?: boolean;
  confirmUnpublish: boolean;
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  confirmationActionsClassName?: string;
  /** The take-down button's text, for a gallery of something other than skins. */
  unpublishLabel?: string;
  /** The take-down confirmation's accessible name. */
  unpublishConfirmLabel?: string;
}

/** Shared rating and owned-skin removal controls used by both community skin galleries. */
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
          className="danger-text pt-0"
          disabled={actionBusy || unpublishDisabled}
          label={unpublishLabel}
        />
      )}
      {confirmUnpublish && (
        <div className={style.confirmation} role="alertdialog" aria-label={unpublishConfirmLabel}>
          <p>{unpublishMessage}</p>
          <div className={confirmationActionsClassName}>
            <ActionButton
              action={onUnpublish}
              ariaBusy={actionBusy}
              className="danger"
              disabled={actionBusy}
              label="确认下架"
            />
            <ActionButton
              action={onCancelUnpublish}
              ariaBusy={actionBusy}
              disabled={actionBusy}
              label="取消"
            />
          </div>
        </div>
      )}
    </>
  );
}
