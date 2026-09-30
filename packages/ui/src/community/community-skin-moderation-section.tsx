import * as style from "./community-style";

export interface CommunitySkinModerationSectionProps {
  owned: boolean;
  actionBusy: boolean;
  ratingDescription: string;
  unpublishMessage: string;
  unpublishDisabled?: boolean;
  confirmUnpublish: boolean;
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  confirmationActionsClassName?: string;
}

/** Shared rating and owned-skin removal controls used by both community skin galleries. */
export function CommunitySkinModerationSection({
  owned,
  actionBusy,
  ratingDescription,
  unpublishMessage,
  unpublishDisabled = false,
  confirmUnpublish,
  onRate,
  onRequestUnpublish,
  onUnpublish,
  onCancelUnpublish,
  confirmationActionsClassName,
}: CommunitySkinModerationSectionProps) {
  return (
    <>
      {!owned && (
        <div
          className={`${style.divided} [&>p]:mt-0 [&>p]:mb-2.5 [&>p]:text-xs [&>p]:text-secondary`}
          aria-label="我的评分"
        >
          <p>{ratingDescription}</p>
          <div className="grid grid-cols-5 gap-1.5">
            {[1, 2, 3, 4, 5].map((stars) => (
              <button
                key={stars}
                type="button"
                className="secondary min-w-0 px-[5px]"
                disabled={actionBusy}
                aria-label={`评 ${stars} 星`}
                onClick={() => onRate(stars)}
              >
                {stars} 星
              </button>
            ))}
          </div>
        </div>
      )}
      {owned && (
        <button
          type="button"
          className="danger-text pt-0"
          disabled={actionBusy || unpublishDisabled}
          onClick={onRequestUnpublish}
        >
          下架这款皮肤
        </button>
      )}
      {confirmUnpublish && (
        <div className={style.confirmation} role="alertdialog" aria-label="确认下架皮肤">
          <p>{unpublishMessage}</p>
          <div className={confirmationActionsClassName}>
            <button type="button" className="danger" disabled={actionBusy} onClick={onUnpublish}>
              确认下架
            </button>
            <button
              type="button"
              className="secondary"
              disabled={actionBusy}
              onClick={onCancelUnpublish}
            >
              取消
            </button>
          </div>
        </div>
      )}
    </>
  );
}
