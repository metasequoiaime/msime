import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import { ActionButton } from "../core/action-button";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityDetailSummary } from "./community-detail-summary";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityReportSection } from "./community-report";
import { CommunitySkinCategorySelect } from "./community-skin-category";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import type { CommunityReportReason } from "./community-report-model";
import type { CommunitySkin, CommunitySkinTrial } from "./community-skin-model";
import * as style from "./community-style";

export interface CommunitySkinDetailProps {
  skin: CommunitySkin;
  theme: "light" | "dark";
  /** 手机详情使用带工具栏的真实键盘比例；桌面详情保留桌面预览。 */
  phonePreview?: boolean;
  /** HarmonyOS 手机按当前触屏方案绘制 26/14/18/9 键，其他宿主省略。 */
  touchRows?: PreviewKey[][];
  actionBusy: boolean;
  detailBusy: boolean;
  actionNotice: string;
  confirmUnpublish: boolean;
  trial: CommunitySkinTrial | null;
  error: string;
  signInRequired: boolean;
  onLogin?: () => void;
  onBack: () => void;
  onDownload: () => void;
  onFinishTrial: (keep: boolean) => void;
  onChangeCategory: (category: NonNullable<CommunitySkin["category"]>) => void;
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  onReport?: (reason: CommunityReportReason, detail: string) => Promise<boolean>;
}

/** 社区皮肤详情的预览、试用、评分、分类和管理操作。 */
export function CommunitySkinDetail({
  skin,
  theme,
  phonePreview = false,
  touchRows,
  actionBusy,
  detailBusy,
  actionNotice,
  confirmUnpublish,
  trial,
  error,
  signInRequired,
  onLogin,
  onBack,
  onDownload,
  onFinishTrial,
  onChangeCategory,
  onRate,
  onRequestUnpublish,
  onUnpublish,
  onCancelUnpublish,
  onReport,
}: CommunitySkinDetailProps) {
  return (
    <CommunityDetailFrame
      backDisabled={actionBusy}
      onBack={onBack}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
    >
      <div className={style.detailStage}>
        <ScreenKeyboardPreview
          theme={theme}
          skin="custom"
          customDesign={skin.design}
          phone={phonePreview}
          touchRows={touchRows}
        />
      </div>
      <CommunityDetailSummary
        title={skin.name}
        noteParts={[communitySkinCategoryLabel(skin.category), skin.author]}
        owned={skin.owned}
        moderation={skin.moderation}
        description={skin.description}
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
        myRating={skin.my_rating}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        loadingText="正在读取皮肤详情…"
      />
      {!trial && (
        <ActionButton
          action={onDownload}
          className={`primary ${style.action}`}
          disabled={actionBusy || detailBusy}
          label="下载并试用"
        />
      )}
      {trial && (
        <div
          className={`${style.divided} grid grid-cols-2 gap-2 [&>p]:col-span-full [&>p]:mt-0 [&>p]:mb-2.5 [&>p]:text-xs [&>p]:text-secondary`}
          aria-label="皮肤试用"
        >
          <p>正在试用：{trial.name}</p>
          <ActionButton
            action={() => onFinishTrial(false)}
            className="secondary"
            disabled={actionBusy}
            label="恢复原皮肤"
          />
          <ActionButton
            action={() => onFinishTrial(true)}
            className="primary"
            disabled={actionBusy}
            label="保留使用"
          />
        </div>
      )}
      {skin.owned && (
        <CommunitySkinCategorySelect
          ariaLabel="修改分类"
          value={skin.category ?? "other"}
          disabled={actionBusy || detailBusy}
          onChange={onChangeCategory}
        />
      )}
      <CommunityModerationSection
        owned={skin.owned}
        actionBusy={actionBusy}
        ratingDescription="我的评分（下载后可评，可重新选择）"
        unpublishMessage={`下架后其他用户无法再下载，已下载的本地皮肤会保留。确定下架“${skin.name}”吗？`}
        unpublishDisabled={Boolean(trial)}
        confirmUnpublish={confirmUnpublish}
        onRate={onRate}
        onRequestUnpublish={onRequestUnpublish}
        onUnpublish={onUnpublish}
        onCancelUnpublish={onCancelUnpublish}
      />
      {!skin.owned && onReport && (
        <CommunityReportSection actionBusy={actionBusy} onReport={onReport} />
      )}
    </CommunityDetailFrame>
  );
}
