import { candidateSkinMegabytes } from "./community-helpers";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityDetailSummary } from "./community-detail-summary";
import { CommunityInstallActions } from "./community-install-actions";
import { CommunityMetrics } from "./community-metrics";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityReportSection } from "./community-report";
import type { CommunityReportReason } from "./community-report-model";
import { kindLabels } from "../settings/plugin-catalog-helpers";
import type { CommunityPlugin } from "./community-plugin-model";

export interface CommunityPluginDetailProps {
  plugin: CommunityPlugin;
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
  onRate: (stars: number) => void;
  onRequestUnpublish: () => void;
  onUnpublish: () => void;
  onCancelUnpublish: () => void;
  onReport?: (reason: CommunityReportReason, detail: string) => Promise<boolean>;
}

/** 插件详情的摘要、安装、评分、下架和举报展示；请求与确认状态由图库页面持有。 */
export function CommunityPluginDetail({
  plugin,
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
  onRate,
  onRequestUnpublish,
  onUnpublish,
  onCancelUnpublish,
  onReport,
}: CommunityPluginDetailProps) {
  return (
    <CommunityDetailFrame
      backDisabled={actionBusy}
      onBack={onBack}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
    >
      <CommunityDetailSummary
        title={plugin.name}
        noteParts={[kindLabels[plugin.kind], plugin.author, plugin.version && `v${plugin.version}`]}
        owned={plugin.owned}
        moderation={plugin.moderation}
        description={plugin.description}
        metrics={
          <CommunityMetrics>
            {[
              plugin.plugin_id,
              plugin.license && `授权 ${plugin.license}`,
              candidateSkinMegabytes(plugin.size),
            ]
              .filter(Boolean)
              .join(" · ")}
          </CommunityMetrics>
        }
        downloads={plugin.downloads}
        ratingCount={plugin.rating_count}
        ratingAverage={plugin.rating_average}
        myRating={plugin.my_rating}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        loadingText="正在读取插件详情…"
      />
      <CommunityInstallActions
        installed={installed}
        installedContent={
          <CommunityActionNotice>已安装到插件目录，可在「我的插件」中选用。</CommunityActionNotice>
        }
        actionBusy={actionBusy}
        detailBusy={detailBusy}
        confirmReplace={confirmReplace}
        replaceAriaLabel="确认替换插件"
        replaceMessage={
          <>
            已安装同 id 的{kindLabels[plugin.kind]}“{plugin.plugin_id}”，安装会整体替换它。
          </>
        }
        onInstall={onInstall}
        onCancelReplace={onCancelReplace}
      />
      <CommunityModerationSection
        owned={plugin.owned}
        actionBusy={actionBusy}
        ratingDescription="我的评分（安装后可评，可重新选择）"
        unpublishMessage={`下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。确定下架“${plugin.name}”吗？`}
        confirmUnpublish={confirmUnpublish}
        onRate={onRate}
        onRequestUnpublish={onRequestUnpublish}
        onUnpublish={onUnpublish}
        onCancelUnpublish={onCancelUnpublish}
        unpublishLabel="下架这个插件"
        unpublishConfirmLabel="确认下架插件"
      />
      {!plugin.owned && onReport && (
        <CommunityReportSection actionBusy={actionBusy} onReport={onReport} />
      )}
    </CommunityDetailFrame>
  );
}
