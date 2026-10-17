import { ActionButton } from "../core/action-button";
import { CommunityMetrics } from "./community-metrics";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityDetailHeader } from "./community-detail-header";
import { CommunityRatingMetrics } from "./community-rating-metrics";
import { CommunityReportSection } from "./community-report";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityResourceEditor } from "./community-resource-editor";
import { canInstallCommunityDictionary } from "./community-resource-actions";
import type { CommunityResourceClient } from "./community-resource-model";
import * as style from "./community-style";
import {
  useCommunityResourceDetail,
  type CommunityResourceDetailOptions,
} from "./use-community-resource-detail";

export interface CommunityResourceDetailProps extends CommunityResourceDetailOptions {
  client: CommunityResourceDetailOptions["client"] & Pick<CommunityResourceClient, "publish">;
}

export function CommunityResourceDetail({
  client,
  initial,
  onClose,
  localDictionary,
}: CommunityResourceDetailProps) {
  const {
    item,
    error,
    notice,
    editing,
    confirmDelete,
    busy,
    save,
    apply,
    applyLocal,
    storeReply,
    rateResource,
    removeReply,
    unpublish,
    report,
    openEditor,
    closeEditor,
    requestUnpublish,
    cancelUnpublish,
    onPublished,
  } = useCommunityResourceDetail({ client, initial, onClose, localDictionary });
  return (
    <CommunityDetailFrame backDisabled={busy} onBack={onClose} backAriaLabel="← 社区" error={error}>
      <CommunityDetailHeader
        title={item.name}
        note={`${item.author} · v${item.revision}`}
        owned={item.owned}
        moderation={item.moderation}
        description={item.description}
      />
      <CommunityRatingMetrics
        count={item.saves}
        countLabel="收藏"
        ratingCount={item.rating_count}
        ratingAverage={item.rating_average}
      />
      {item.kind === "dictionary" ? (
        <>
          <h3>词条预览 · {(item.content.entries ?? []).length} 条</h3>
          <div className={style.entryPreview}>
            {(item.content.entries ?? []).map((entry, index) => (
              <div key={`${entry.kind}-${entry.code}-${index}`}>
                <span>{entry.word}</span>
                <code>{entry.code}</code>
              </div>
            ))}
          </div>
          {canInstallCommunityDictionary(localDictionary) && (
            <ActionButton
              action={applyLocal}
              className={`primary ${style.action}`}
              disabled={busy}
              label="导入这版词库到本机"
            />
          )}
          <ActionButton
            action={apply}
            className={`secondary ${style.action}`}
            disabled={busy}
            label="导入这版词库到云端"
          />
          <CommunityMetrics className={style.divided}>
            本机导入只更新当前设备；云端导入会合并到账号云词库。版本发生变化时云端导入会停止并要求重新查看。
          </CommunityMetrics>
        </>
      ) : (
        <>
          <h3>提示词预览</h3>
          <pre className={style.promptPreview}>{item.content.prompt}</pre>
          {client.storeReply && (
            <ActionButton
              action={storeReply}
              className={`primary ${style.action}`}
              disabled={busy}
              label="添加到高情商回复键盘"
            />
          )}
          {client.removeReply && (
            <ActionButton
              action={removeReply}
              className={`secondary ${style.action}`}
              disabled={busy}
              label="从本机高情商回复键盘移除"
            />
          )}
        </>
      )}
      {notice && <CommunityActionNotice>{notice}</CommunityActionNotice>}
      <ActionButton
        action={save}
        className={`secondary ${style.action}`}
        disabled={busy}
        label={item.saved ? "取消收藏" : "收藏，关注后续更新"}
      />
      {item.owned && (
        <ActionButton
          action={openEditor}
          className={`secondary ${style.action}`}
          disabled={busy}
          label="编辑并发布新版本"
        />
      )}
      <CommunityModerationSection
        owned={item.owned}
        actionBusy={busy}
        ratingDescription="我的评分（可重新选择）"
        unpublishMessage={
          <>
            下架后其他用户无法获取此作品，已有本地回复模板和云词库副本不会被删除。确定下架“
            {item.name}”吗？
          </>
        }
        confirmUnpublish={confirmDelete}
        onRate={rateResource}
        onRequestUnpublish={requestUnpublish}
        onUnpublish={unpublish}
        onCancelUnpublish={cancelUnpublish}
        unpublishButtonClassName="danger-text community-unpublish"
        unpublishLabel="下架作品"
        unpublishConfirmLabel="确认下架作品"
      />
      {!item.owned && client.report && (
        <CommunityReportSection actionBusy={busy} onReport={report} />
      )}
      {editing && (
        <CommunityResourceEditor
          client={client}
          kind={item.kind}
          existing={item}
          localDictionary={localDictionary}
          onClose={closeEditor}
          onPublished={onPublished}
        />
      )}
    </CommunityDetailFrame>
  );
}
