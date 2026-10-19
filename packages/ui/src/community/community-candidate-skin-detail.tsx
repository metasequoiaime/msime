import { ActionButton } from "../core/action-button";
import { communityLicenseLine } from "./community-helpers";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityDetailSummary } from "./community-detail-summary";
import { CommunityInstallActions } from "./community-install-actions";
import { CommunityMetrics } from "./community-metrics";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityReportSection } from "./community-report";
import { CommunitySkinCategorySelect } from "./community-skin-category";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import type {
  CandidateSkinCategory,
  CandidateSkinVisibility,
  CommunityCandidateSkin,
} from "./community-candidate-skin-model";
import type { CommunityReportReason } from "./community-report-model";
import {
  CandidateSkinPreviewImage,
  type CandidateSkinPreviewLoader,
} from "./community-candidate-skin-preview";
import * as style from "./community-style";
export {
  CandidateSkinPreviewImage,
  type CandidateSkinPreviewLoader,
} from "./community-candidate-skin-preview";

export interface CommunityCandidateSkinDetailProps {
  skin: CommunityCandidateSkin;
  load: CandidateSkinPreviewLoader;
  actionBusy: boolean;
  detailBusy: boolean;
  actionNotice: string;
  error: string;
  signInRequired: boolean;
  installed: boolean;
  confirmReplace: boolean;
  confirmUnpublish: boolean;
  onLogin?: () => void;
  onBack: () => void;
  onInstall: (replace: boolean) => void;
  onCancelReplace: () => void;
  onOpenSkinPage?: () => void;
  onChangeVisibility: (visibility: CandidateSkinVisibility) => void;
  onChangeCategory: (category: CandidateSkinCategory) => void;
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  onReport?: (reason: CommunityReportReason, detail: string) => Promise<boolean>;
}

/** 候选窗口皮肤详情的预览、安装、可见性、分类和社区管理操作。 */
export function CommunityCandidateSkinDetail({
  skin,
  load,
  actionBusy,
  detailBusy,
  actionNotice,
  error,
  signInRequired,
  installed,
  confirmReplace,
  confirmUnpublish,
  onLogin,
  onBack,
  onInstall,
  onCancelReplace,
  onOpenSkinPage,
  onChangeVisibility,
  onChangeCategory,
  onRate,
  onRequestUnpublish,
  onUnpublish,
  onCancelUnpublish,
  onReport,
}: CommunityCandidateSkinDetailProps) {
  const license = communityLicenseLine(skin.license);
  return (
    <CommunityDetailFrame
      backDisabled={actionBusy}
      onBack={onBack}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
    >
      <CandidateSkinPreviewImage
        id={skin.id}
        revision={skin.updated_at}
        name={skin.name}
        load={load}
        className={style.detailStage}
      />
      <CommunityDetailSummary
        title={skin.name}
        noteParts={[
          communitySkinCategoryLabel(skin.category),
          skin.author,
          skin.version && `v${skin.version}`,
        ]}
        owned={skin.owned}
        moderation={skin.moderation}
        ownedLabel={skin.visibility === "private" ? "私有" : "我的作品"}
        description={skin.description}
        metrics={license && <CommunityMetrics>{license}</CommunityMetrics>}
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
        myRating={skin.my_rating}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        loadingText="正在读取皮肤详情…"
      />
      <CommunityInstallActions
        installed={installed}
        installedContent={
          <>
            <CommunityActionNotice>已安装到外部皮肤。</CommunityActionNotice>
            {onOpenSkinPage && (
              <ActionButton
                action={onOpenSkinPage}
                className={`primary ${style.action}`}
                label="去启用"
              />
            )}
          </>
        }
        actionBusy={actionBusy}
        detailBusy={detailBusy}
        confirmReplace={confirmReplace}
        replaceAriaLabel="确认替换皮肤"
        replaceMessage={<>已存在同名皮肤“{skin.package_id}”，安装会整体替换它。</>}
        onInstall={onInstall}
        onCancelReplace={onCancelReplace}
      />
      {skin.owned && (
        <ActionButton
          action={() => onChangeVisibility(skin.visibility === "private" ? "public" : "private")}
          className={`secondary ${style.action}`}
          disabled={actionBusy || detailBusy}
          label={skin.visibility === "private" ? "公开" : "设为私有"}
        />
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
        unpublishable={skin.visibility === "public"}
        actionBusy={actionBusy}
        ratingDescription="我的评分（安装后可评，可重新选择）"
        unpublishMessage={`下架后其他用户无法再下载，下载数和评分会清空；本地皮肤会保留并以私有方式同步。只想不让别人看到，可以改用“设为私有”。确定下架“${skin.name}”吗？`}
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
