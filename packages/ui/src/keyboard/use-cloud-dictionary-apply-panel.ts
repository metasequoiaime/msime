import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { subscribeToInterval } from "../core/use-interval";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { usePanelAction } from "./use-panel-action";
import { cloudDictionaryErrorMessage } from "./cloud-dictionary-errors";
import { cloudResponseRequest } from "./cloud-response";
import type {
  CloudDictionaryPanelClient,
  CloudDictionarySnapshotMetadata,
  CloudDictionarySnapshotRequest,
} from "./panel-client";

const PANEL_SNAPSHOT_RETRY_NOTICE =
  "快照已入列，这次没能马上应用；请保持这个面板打开，稍后会自动重试";

/** 由面板自己处理队列的宿主（`snapshotAppliesInPanel`）按请求状态给出的提示。结束的结果在下一次轮询时被取走、处理结果一节随之消失，所以提示自己说清结局，不指向那一节。 */
function panelSnapshotNotice(status: CloudDictionarySnapshotRequest["status"] | undefined): string {
  switch (status) {
    case "applied":
      return "云端快照已应用到本机";
    case "queued":
    case "preparing":
      return PANEL_SNAPSHOT_RETRY_NOTICE;
    case "conflict":
      return "快照因冲突作废，本机词库未改变";
    case "cancelled":
      return "已取消待应用快照";
    default:
      return "快照没有应用";
  }
}

/** 管理完整云词库快照的预览、确认、队列与状态轮询。 */
export function useCloudDictionaryApplyPanel(client: CloudDictionaryPanelClient) {
  const { confirm, dismiss, confirmation } = useConfirm();
  const [localVersion, setLocalVersion] = useState<string | null>(null);
  const [request, setRequest] = useState<CloudDictionarySnapshotRequest | null>(null);
  const [preview, setPreview] = useState<CloudDictionarySnapshotMetadata | null>(null);
  const [previewToken, setPreviewToken] = useState<string | null>(null);
  const [notice, setNotice] = useState("先获取本机词库版本，再下载并预览云端快照");
  const {
    busy,
    busyRef,
    revisionRef: statusRevision,
    run: runAction,
    invalidate,
    isCurrent,
  } = usePanelAction(setNotice, cloudDictionaryErrorMessage);
  const lifecycleRevision = useAsyncGeneration(client);
  const statusRead = useRef<number | null>(null);

  async function refreshStatus() {
    if (busyRef.current || statusRead.current !== null) return;
    const lifecycle = lifecycleRevision.current;
    const revision = ++statusRevision.current;
    // 读取可能比轮询间隔慢，等待它结束，避免每次 tick 都作废即将到达的回答。
    statusRead.current = revision;
    try {
      const result = await client.request({ operation: "snapshot_status" });
      if (lifecycle !== lifecycleRevision.current || !isCurrent(revision)) return;
      setLocalVersion(typeof result.localVersion === "string" ? result.localVersion : null);
      const current = cloudResponseRequest<CloudDictionarySnapshotRequest>(result);
      setRequest(current);
      // 面板重试成功或作废以后，换掉「请保持面板打开」那句，否则结果被取走后它一直留在那里。
      if (client.snapshotAppliesInPanel && current) {
        const notice = panelSnapshotNotice(current.status);
        setNotice((shown) => (shown === PANEL_SNAPSHOT_RETRY_NOTICE ? notice : shown));
      }
    } catch {
      if (lifecycle === lifecycleRevision.current && isCurrent(revision))
        setNotice("无法读取本机词库状态，请确认键盘已启用");
    } finally {
      if (statusRead.current === revision) statusRead.current = null;
    }
  }

  function run(action: (revision: number, lifecycle: number) => Promise<void>, failure: string) {
    const lifecycle = lifecycleRevision.current;
    return runAction((revision) => action(revision, lifecycle), failure);
  }

  function download() {
    if (!localVersion) {
      setNotice("尚未获取本机词库版本，请先打开水杉键盘");
      return;
    }
    void run(async (revision, lifecycle) => {
      const result = await client.request({ operation: "snapshot_preview" });
      if (lifecycle !== lifecycleRevision.current || !isCurrent(revision)) return;
      if (!result.snapshot || typeof result.previewToken !== "string")
        throw new Error("invalid preview");
      setPreview(result.snapshot);
      setPreviewToken(result.previewToken);
      setNotice("云端快照已校验，请确认后替换本机词库");
    }, "下载云词库快照失败");
  }

  async function enqueue() {
    const token = previewToken;
    if (!token || !preview || busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const confirmed = await confirm({
      title: "替换本机词库",
      message: client.snapshotAppliesInPanel
        ? "这份云端快照会替换本机个人词库和学习记录。替换马上进行，期间输入会暂停；请保持这个面板打开，直到状态显示已应用。"
        : "这份云端快照会替换本机个人词库和学习记录，输入法将在下一次空闲边界应用。",
      confirmLabel: "替换",
      danger: true,
    });
    if (!confirmed || busyRef.current || lifecycle !== lifecycleRevision.current) return;
    void run(async (revision, currentLifecycle) => {
      const result = await client.request({ operation: "snapshot_enqueue", token });
      if (currentLifecycle !== lifecycleRevision.current || !isCurrent(revision)) return;
      setPreview(null);
      setPreviewToken(null);
      const queued = cloudResponseRequest<CloudDictionarySnapshotRequest>(result);
      setRequest(queued);
      setNotice(
        client.snapshotAppliesInPanel
          ? panelSnapshotNotice(queued?.status)
          : "快照已入列，将在输入法空闲时应用",
      );
    }, "快照入列失败，本机词库未改变");
  }

  async function cancel() {
    if (busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const confirmed = await confirm({
      title: "取消待应用快照",
      message: "待应用的云词库快照会被撤销。",
      confirmLabel: "取消快照",
      cancelLabel: "保留",
    });
    if (!confirmed || busyRef.current || lifecycle !== lifecycleRevision.current) return;
    void run(async (revision, currentLifecycle) => {
      const result = await client.request({ operation: "snapshot_cancel" });
      if (currentLifecycle !== lifecycleRevision.current || !isCurrent(revision)) return;
      setRequest(cloudResponseRequest<CloudDictionarySnapshotRequest>(result));
      setNotice("已取消待应用快照");
    }, "取消快照失败");
  }

  useEffect(() => {
    invalidate();
    dismiss();
    statusRead.current = null;
    setPreview(null);
    setPreviewToken(null);
    void refreshStatus();
    const stopPoll = subscribeToInterval(() => {
      void refreshStatus();
    }, 2000);
    return () => {
      invalidate();
      statusRead.current = null;
      stopPoll();
    };
  }, [client, dismiss, invalidate]);

  function discardPreview() {
    setPreview(null);
    setPreviewToken(null);
  }

  return {
    confirmation,
    localVersion,
    request,
    preview,
    previewToken,
    notice,
    busy,
    refreshStatus,
    download,
    enqueue,
    cancel,
    discardPreview,
  };
}

export type CloudDictionaryApplyPanelState = ReturnType<typeof useCloudDictionaryApplyPanel>;
