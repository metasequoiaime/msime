import { useState } from "react";
import * as style from "./community-style";
import { CommunityTextareaField } from "./community-textarea-field";
import { CommunityConfirmationActions } from "./community-confirmation-actions";
import { ActionButton } from "../core/action-button";

/** What a report names, as `POST /v1/community/reports` and client-core's `CommunityReportKind` spell it. */
export type CommunityReportKind =
  | "skins"
  | "candidate-skins"
  | "plugins"
  | "dictionaries"
  | "replies";

/** The fixed reasons a report offers, in the order the dialog lists them; the label is what the moderators see. Mirrors client-core's `CommunityReportReason::ALL`. */
export const communityReportReasons = [
  "侵权/抄袭",
  "色情低俗",
  "违法违规",
  "垃圾广告",
  "恶意插件",
  "其他",
] as const;

export type CommunityReportReason = (typeof communityReportReasons)[number];

/** Longest optional detail the server accepts. */
export const communityReportDetailLimit = 1000;

/** Where the signed-in user's own published item stands with the moderators; sent only for one's own items. */
export type CommunityModeration = "approved" | "pending" | "removed";

/** The notice a gallery shows once a report was filed (a repeated report of the same item is accepted the same way). */
export const communityReportedNotice = "已收到举报，我们会尽快处理。";

/** The 已下架 badge on the owner's own item that moderators removed. Nothing else about moderation is shown: no reason and no pending state. */
export function CommunityRemovedBadge({
  owned,
  moderation,
}: {
  owned: boolean;
  moderation?: CommunityModeration | null;
}) {
  if (!owned || moderation !== "removed") return null;
  return (
    <span className={style.detailBadge} title="已被管理员下架，其他用户看不到这个作品">
      已下架
    </span>
  );
}

export interface CommunityReportSectionProps {
  actionBusy: boolean;
  /** Files the report; resolves true once the server accepted it, which closes the form. */
  onReport: (reason: CommunityReportReason, detail: string) => Promise<boolean>;
}

/** The 举报 entry under another user's item: a reason from the fixed list and an optional detail. */
export function CommunityReportSection({ actionBusy, onReport }: CommunityReportSectionProps) {
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState<CommunityReportReason | "">("");
  const [detail, setDetail] = useState("");
  const close = () => {
    setOpen(false);
    setReason("");
    setDetail("");
  };
  if (!open) {
    return (
      <ActionButton
        action={() => setOpen(true)}
        className="danger-text self-start pt-0"
        disabled={actionBusy}
        label="举报"
      />
    );
  }
  return (
    <div className={style.confirmation} role="alertdialog" aria-label="举报作品">
      <fieldset className="m-0 flex flex-col gap-1.5 border-0 p-0" disabled={actionBusy}>
        <legend className="mb-1.5 p-0">请选择举报原因</legend>
        {communityReportReasons.map((item) => (
          <label key={item} className="flex items-center gap-2">
            <input
              type="radio"
              name="community-report-reason"
              value={item}
              checked={reason === item}
              onChange={() => setReason(item)}
            />
            {item}
          </label>
        ))}
      </fieldset>
      <CommunityTextareaField
        label="补充说明（选填）"
        ariaLabel="举报补充说明"
        maxLength={communityReportDetailLimit}
        rows={3}
        value={detail}
        disabled={actionBusy}
        onChange={setDetail}
      />
      <CommunityConfirmationActions>
        <ActionButton
          action={() => {
            if (!reason) return;
            void onReport(reason, detail.trim()).then((reported) => {
              if (reported) close();
            });
          }}
          className="danger"
          disabled={actionBusy || !reason}
          ariaBusy={actionBusy}
          label="提交举报"
        />
        <ActionButton action={close} disabled={actionBusy} label="取消" />
      </CommunityConfirmationActions>
    </div>
  );
}
