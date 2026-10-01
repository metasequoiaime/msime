import { useEffect, useRef, useState, type ReactNode } from "react";
import * as settings from "./settings-style";
import { GroupList, Row, Switch } from "../core/platform-controls";
import {
  CLOUD_CLIPBOARD_MAX_UTF16,
  cloudClipboardAvailabilityNote,
  cloudClipboardFailure,
  type CloudClipboardAvailability,
  type CloudClipboardRequest,
} from "./cloud-clipboard-send";

export type ClipboardHistoryEntry = { text: string; timestampMs: number; pinned: boolean };

export type ClipboardHistoryClient = {
  clear(): Promise<void>;
  list?(): Promise<ClipboardHistoryEntry[]>;
  sync?(): Promise<ClipboardHistoryEntry[]>;
  copy?(text: string): Promise<void>;
  remove?(text: string): Promise<void>;
  setPinned?(text: string, pinned: boolean): Promise<void>;
};

export interface ClipboardHistorySectionProps {
  client?: ClipboardHistoryClient;
  historyEnabled: boolean;
  persistedHistoryEnabled: boolean;
  revision?: number;
  page?: string;
  ios: boolean;
  onToggle: (enabled: boolean) => void;
  onError: (message: string) => void;
  /** Present while the page is shown on a host that offers the cloud clipboard; each history entry then gets a 发到云剪贴板 action. Nothing is uploaded unless the user picks an entry. */
  cloudRequest?: CloudClipboardRequest;
  children?: ReactNode;
}

/** Shared local clipboard history controls used by desktop and mobile settings hosts: the 剪贴板 and 历史记录 groups of the 剪贴板 page, followed by whatever the host adds (its cloud panels). */
export function ClipboardHistorySection({
  client,
  historyEnabled,
  persistedHistoryEnabled,
  revision,
  page,
  ios,
  onToggle,
  onError,
  cloudRequest,
  children,
}: ClipboardHistorySectionProps) {
  const [entries, setEntries] = useState<ClipboardHistoryEntry[]>([]);
  const [clearArmed, setClearArmed] = useState(false);
  const [cloud, setCloud] = useState<CloudClipboardAvailability>("checking");
  const [cloudNotice, setCloudNotice] = useState("");
  const [sending, setSending] = useState(false);
  const cloudRevision = useRef(0);
  const historyGeneration = useRef(0);
  const historyActionBusy = useRef(false);
  const historyShown = historyEnabled && Boolean(client?.list);

  // The account state and the server's enabled flag are read once each time the page is opened with the history showing; there is no polling.
  useEffect(() => {
    const revision = ++cloudRevision.current;
    setCloud("checking");
    setCloudNotice("");
    setSending(false);
    if (!cloudRequest || !historyShown) return;
    cloudRequest({ operation: "list", search: "" }).then(
      (result) => {
        if (revision === cloudRevision.current)
          setCloud(result.enabled === false ? "disabled" : "ready");
      },
      (error: unknown) => {
        if (revision === cloudRevision.current) setCloud(cloudClipboardFailure(error));
      },
    );
    return () => {
      cloudRevision.current++;
    };
  }, [cloudRequest, historyShown]);

  const sendToCloud = async (text: string) => {
    if (!cloudRequest || cloud !== "ready" || sending) return;
    if (text.length > CLOUD_CLIPBOARD_MAX_UTF16) {
      setCloudNotice(`这条记录超过 ${CLOUD_CLIPBOARD_MAX_UTF16} 个 UTF-16 单元，无法发到云剪贴板`);
      return;
    }
    const revision = cloudRevision.current;
    setSending(true);
    setCloudNotice("正在发到云剪贴板…");
    try {
      await cloudRequest({ operation: "add", text });
      if (revision === cloudRevision.current) setCloudNotice("已发到云剪贴板");
    } catch (error) {
      if (revision !== cloudRevision.current) return;
      if (cloudClipboardFailure(error) === "signed_out") {
        setCloud("signed_out");
        setCloudNotice("");
      } else {
        setCloudNotice("发到云剪贴板失败，请稍后重试");
      }
    } finally {
      if (revision === cloudRevision.current) setSending(false);
    }
  };
  const cloudNote = cloudRequest ? cloudNotice || cloudClipboardAvailabilityNote(cloud) : undefined;

  useEffect(() => {
    const currentGeneration = ++historyGeneration.current;
    let active = true;
    if (!ios && !persistedHistoryEnabled) {
      setEntries([]);
      setClearArmed(false);
      return;
    }
    if (!client?.list) return;
    void client
      .list()
      .then((next) => {
        if (active && currentGeneration === historyGeneration.current) {
          setEntries(next);
          setClearArmed(false);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
      historyGeneration.current++;
      historyActionBusy.current = false;
    };
  }, [client, ios, page, persistedHistoryEnabled, revision]);

  const mutate = async (action: () => Promise<void>, failure: string) => {
    if (historyActionBusy.current) return;
    historyActionBusy.current = true;
    const currentGeneration = historyGeneration.current;
    const currentClient = client;
    try {
      await action();
      if (currentGeneration !== historyGeneration.current) return;
      setClearArmed(false);
      if (currentClient?.list) {
        const next = await currentClient.list();
        if (currentGeneration !== historyGeneration.current) return;
        setEntries(next);
      }
    } catch {
      if (currentGeneration === historyGeneration.current) onError(failure);
    } finally {
      if (currentGeneration === historyGeneration.current) historyActionBusy.current = false;
    }
  };

  const toggle = (enabled: boolean) => {
    onToggle(enabled);
    if (!enabled) {
      setEntries([]);
      setClearArmed(false);
    }
  };

  return (
    <>
      <GroupList title="剪贴板">
        <Row
          title="剪贴板管理"
          description={
            ios
              ? "由键盘的“允许完全访问”权限控制；记录仅保存在本机。"
              : "开启后记录复制的文本；保存关闭设置后清空已保存记录，且只记录文本类型。"
          }
        >
          {!ios && <Switch aria-label="剪贴板管理" checked={historyEnabled} onChange={toggle} />}
        </Row>
        {((!ios && client?.sync) || (client && entries.length > 0)) && (
          <div className={settings.managerBlock}>
            <div className={settings.managerActions}>
              {!ios && client?.sync && (
                <button
                  type="button"
                  className="secondary"
                  disabled={!historyEnabled || !persistedHistoryEnabled}
                  onClick={() => {
                    const currentGeneration = historyGeneration.current;
                    void client.sync!()
                      .then((next) => {
                        if (currentGeneration !== historyGeneration.current) return;
                        setEntries(next);
                        setClearArmed(false);
                      })
                      .catch(() => {
                        if (currentGeneration === historyGeneration.current)
                          onError("无法同步剪贴板历史");
                      });
                  }}
                >
                  从系统剪贴板同步
                </button>
              )}
              {client && entries.length > 0 && (
                <button
                  type="button"
                  className="secondary"
                  disabled={!historyEnabled}
                  onClick={() => {
                    if (!clearArmed) {
                      setClearArmed(true);
                      return;
                    }
                    void mutate(() => client.clear(), "无法清空剪贴板历史，请稍后重试。");
                  }}
                >
                  {clearArmed ? "确认清空" : "清空历史"}
                </button>
              )}
            </div>
          </div>
        )}
      </GroupList>
      {historyEnabled && client?.list && (
        <GroupList title="历史记录">
          {cloudNote && (
            <p className={settings.groupNote} role="status">
              {cloudNote}
            </p>
          )}
          <div className={settings.clipboardList} aria-label="剪贴板历史">
            {entries.length === 0 ? (
              <p className={settings.clipboardEmpty}>暂无历史记录</p>
            ) : (
              entries.map((entry) => (
                <div className={settings.clipboardRow} data-clipboard-entry-row="" key={entry.text}>
                  <span className={settings.clipboardEntry}>
                    <span title={entry.text}>{entry.text}</span>
                    <small>
                      {entry.pinned ? "已固定 · " : ""}
                      {entry.timestampMs > 1_000_000_000_000
                        ? new Date(entry.timestampMs).toLocaleString()
                        : "旧记录"}
                    </small>
                  </span>
                  <span className={settings.clipboardActions}>
                    {client.copy && (
                      <button
                        type="button"
                        className="secondary"
                        onClick={() => void client.copy!(entry.text)}
                      >
                        重新复制
                      </button>
                    )}
                    {cloudRequest && (
                      <button
                        type="button"
                        className="secondary"
                        disabled={cloud !== "ready" || sending}
                        onClick={() => void sendToCloud(entry.text)}
                      >
                        发到云剪贴板
                      </button>
                    )}
                    {client.setPinned && (
                      <button
                        type="button"
                        className="secondary"
                        aria-label={`${entry.pinned ? "取消固定" : "固定"}剪贴板记录`}
                        onClick={() =>
                          void mutate(
                            () => client.setPinned!(entry.text, !entry.pinned),
                            "无法更新剪贴板固定状态",
                          )
                        }
                      >
                        {entry.pinned ? "取消固定" : "固定"}
                      </button>
                    )}
                    {client.remove && (
                      <button
                        type="button"
                        className="secondary"
                        aria-label="删除剪贴板记录"
                        onClick={() =>
                          void mutate(() => client.remove!(entry.text), "无法删除剪贴板记录")
                        }
                      >
                        删除
                      </button>
                    )}
                  </span>
                </div>
              ))
            )}
          </div>
        </GroupList>
      )}
      {children}
    </>
  );
}
