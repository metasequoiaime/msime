import { useEffect, useState, type ReactNode } from "react";
import * as settings from "./settings-style";
import { GroupList, Row, Switch } from "../core/platform-controls";

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
  children?: ReactNode;
}

/** Shared local clipboard history controls used by desktop and mobile settings hosts: the 剪贴板 and 历史记录 groups of the 云剪贴板 page, followed by whatever the host adds (its cloud panels). */
export function ClipboardHistorySection({
  client,
  historyEnabled,
  persistedHistoryEnabled,
  revision,
  page,
  ios,
  onToggle,
  onError,
  children,
}: ClipboardHistorySectionProps) {
  const [entries, setEntries] = useState<ClipboardHistoryEntry[]>([]);
  const [clearArmed, setClearArmed] = useState(false);

  useEffect(() => {
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
        if (active) {
          setEntries(next);
          setClearArmed(false);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client, ios, page, persistedHistoryEnabled, revision]);

  const mutate = async (action: () => Promise<void>, failure: string) => {
    try {
      await action();
      setClearArmed(false);
      if (client?.list) setEntries(await client.list());
    } catch {
      onError(failure);
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
                  onClick={() =>
                    void client.sync!()
                      .then((next) => {
                        setEntries(next);
                        setClearArmed(false);
                      })
                      .catch(() => onError("无法同步剪贴板历史"))
                  }
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
