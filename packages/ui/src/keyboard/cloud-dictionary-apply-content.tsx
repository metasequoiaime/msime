import type {
  CloudDictionarySnapshotMetadata,
  CloudDictionarySnapshotRequest,
} from "./panel-client";
import * as cloud from "./cloud-panel-style";

export interface CloudDictionaryApplyContentProps {
  localVersion: string | null;
  request: CloudDictionarySnapshotRequest | null;
  preview: CloudDictionarySnapshotMetadata | null;
  canApply: boolean;
  busy: boolean;
  appliesInPanel: boolean;
  onDownload: () => void;
  onRefresh: () => void;
  onEnqueue: () => void;
  onDiscardPreview: () => void;
  onCancel: () => void;
}

export function CloudDictionaryApplyContent({
  localVersion,
  request,
  preview,
  canApply,
  busy,
  appliesInPanel,
  onDownload,
  onRefresh,
  onEnqueue,
  onDiscardPreview,
  onCancel,
}: CloudDictionaryApplyContentProps) {
  return (
    <>
      <section className={cloud.dictionarySection} aria-label="准备本机词库">
        <h2>准备本机词库</h2>
        <p className={cloud.dictionaryNote}>
          请启用水杉键盘并打开一次，让宿主提供当前本机词库版本。测试区或编辑器内容不会上传。
        </p>
        <div className={cloud.dictionarySnapshotFact}>
          {localVersion ? (
            <>
              <strong>已获取本机词库版本</strong>
              <small>{localVersion}</small>
            </>
          ) : (
            <strong>尚未获取本机词库版本</strong>
          )}
        </div>
        <button
          className={cloud.dictionaryButton}
          type="button"
          onClick={onDownload}
          disabled={
            busy || !localVersion || request?.status === "queued" || request?.status === "preparing"
          }
        >
          下载云词库并预览
        </button>
        <button
          type="button"
          className="secondary"
          onClick={() => void onRefresh()}
          disabled={busy}
        >
          刷新状态
        </button>
      </section>
      {preview && (
        <section className={cloud.dictionarySection} aria-label="确认应用">
          <h2>确认应用</h2>
          <p className={cloud.dictionaryNote}>
            云端 revision {preview.cloudRevision} · {preview.records} 条记录 · {preview.bytes} 字节
          </p>
          <p className={cloud.dictionaryNote}>
            词条 {preview.entries} · 覆盖 {preview.overlays} · 固定 {preview.positions} · 选择{" "}
            {preview.selections}
          </p>
          <button type="button" onClick={onEnqueue} disabled={busy || !canApply}>
            替换本机词库
          </button>
          <button type="button" className="secondary" onClick={onDiscardPreview} disabled={busy}>
            丢弃预览
          </button>
        </section>
      )}
      {request && (
        <section className={cloud.dictionarySection} aria-label="处理结果">
          <h2>处理结果</h2>
          <p className={cloud.dictionaryNote}>
            状态：{request.status} · 云端 revision {request.cloudRevision}
          </p>
          {appliesInPanel && (request.status === "queued" || request.status === "preparing") && (
            <p className={cloud.dictionaryNote}>
              快照只在这个面板开着时应用，关掉面板会停在待应用；之后输入改变了本机词库，这份快照会因冲突作废。
            </p>
          )}
          {(request.status === "queued" || request.status === "preparing") && (
            <button type="button" className="secondary" onClick={onCancel} disabled={busy}>
              取消待应用快照
            </button>
          )}
        </section>
      )}
    </>
  );
}
