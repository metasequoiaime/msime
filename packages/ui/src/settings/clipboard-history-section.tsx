import { SettingsGroupNote } from "./settings-group-note";
import type { ReactNode } from "react";
import * as settings from "./settings-style";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerActions } from "./settings-manager-actions";
import { GroupList, Row } from "../core/platform-controls";
import type { CloudClipboardRequest } from "./cloud-clipboard-send";
import { ActionButton } from "./action-button";
import { SwitchRow } from "./switch-row";
import { useClipboardHistory } from "./use-clipboard-history";
import { useClipboardHistoryCloud } from "./use-clipboard-history-cloud";
import type { ClipboardHistoryClient } from "./clipboard-history-model";
import { formatDateTime } from "../core/format-date";
import {
  clipboardHistoryLimit,
  clipboardHistoryLimits,
  defaultClipboardHistoryLimit,
} from "./clipboard-history-preferences";
import { SelectRow } from "./select-row";

export type { ClipboardHistoryClient, ClipboardHistoryEntry } from "./clipboard-history-model";

export interface ClipboardHistorySectionProps {
  client?: ClipboardHistoryClient;
  historyEnabled: boolean;
  historyLimit?: number;
  persistedHistoryEnabled: boolean;
  revision?: number;
  page?: string;
  ios: boolean;
  onToggle: (enabled: boolean) => void;
  onLimitChange?: (limit: number) => void;
  onError: (message: string) => void;
  /** 页面可见且宿主支持云剪贴板时提供；用户选择某条记录后才发送。 */
  cloudRequest?: CloudClipboardRequest;
  /** 云剪贴板现在走哪条路，例如已保存的自建中继地址和令牌；它一变（同一页上保存或改回了中继）就重新查一次可用状态，不沿用另一条路的结果。 */
  cloudRoute?: string;
  /** 「剪贴板」组里一直显示的一句说明，例如 macOS 上云剪贴板要从输入法菜单打开。 */
  note?: ReactNode;
  children?: ReactNode;
}

/** 桌面和移动设置宿主共用的本地剪贴板历史控件：「剪贴板」页的「剪贴板」和「历史记录」两组，后面接宿主追加的内容（它的云剪贴板）。 */
export function ClipboardHistorySection({
  client,
  historyEnabled,
  historyLimit = defaultClipboardHistoryLimit,
  persistedHistoryEnabled,
  revision,
  page,
  ios,
  onToggle,
  onLimitChange,
  onError,
  cloudRequest,
  cloudRoute,
  note,
  children,
}: ClipboardHistorySectionProps) {
  const { entries, clearArmed, historyBusy, historyShown, toggle, clear, copy, remove, setPinned } =
    useClipboardHistory({
      client,
      historyEnabled,
      persistedHistoryEnabled,
      revision,
      page,
      ios,
      onToggle,
      onError,
    });
  const { cloud, sending, cloudNote, sendToCloud } = useClipboardHistoryCloud({
    cloudRequest,
    cloudRoute,
    historyShown,
  });

  const clipboardDescription = ios
    ? "由键盘的“允许完全访问”权限控制；记录仅保存在本机。"
    : "开启后记录复制的文本；保存关闭设置后清空已保存记录，且只记录文本类型。";

  return (
    <>
      <GroupList title="剪贴板">
        {note && <SettingsGroupNote>{note}</SettingsGroupNote>}
        {ios ? (
          <Row title="剪贴板历史" description={clipboardDescription} />
        ) : (
          <SwitchRow
            title="剪贴板历史"
            description={clipboardDescription}
            aria-label="剪贴板历史"
            checked={historyEnabled}
            disabled={historyBusy}
            onChange={toggle}
          />
        )}
        <SelectRow
          title="历史记录上限"
          description="达到上限后，最旧的未固定记录会被新记录替换。"
          value={String(clipboardHistoryLimit({ clipboard_history_limit: historyLimit }))}
          disabled={historyBusy}
          onChange={(event) => onLimitChange?.(Number(event.currentTarget.value))}
        >
          {clipboardHistoryLimits.map((limit) => (
            <option key={limit} value={limit}>
              {limit} 条
            </option>
          ))}
        </SelectRow>
        {client && entries.length > 0 && (
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton
                action={() => void clear()}
                disabled={!historyEnabled || historyBusy}
                label={clearArmed ? "确认清空" : "清空历史"}
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        )}
      </GroupList>
      {historyShown && (
        <GroupList title="历史记录">
          {cloudNote && <SettingsGroupNote role="status">{cloudNote}</SettingsGroupNote>}
          <div className={settings.clipboardList} aria-label="剪贴板历史">
            {entries.length === 0 ? (
              <SettingsEmptyMessage>暂无历史记录</SettingsEmptyMessage>
            ) : (
              entries.map((entry) => (
                <div className={settings.clipboardRow} data-clipboard-entry-row="" key={entry.text}>
                  <span className={settings.clipboardEntry}>
                    <span title={entry.text}>{entry.text}</span>
                    <small>
                      {entry.pinned ? "已固定 · " : ""}
                      {entry.timestampMs > 1_000_000_000_000
                        ? formatDateTime(entry.timestampMs)
                        : "旧记录"}
                    </small>
                  </span>
                  <span className={settings.clipboardActions}>
                    {client?.copy && (
                      <ActionButton action={() => void copy(entry.text)} label="重新复制" />
                    )}
                    {cloudRequest && (
                      <ActionButton
                        action={() => void sendToCloud(entry.text)}
                        disabled={cloud !== "ready" || sending}
                        label="发到云剪贴板"
                      />
                    )}
                    {client?.setPinned && (
                      <ActionButton
                        action={() => void setPinned(entry.text, !entry.pinned)}
                        ariaLabel={`${entry.pinned ? "取消固定" : "固定"}剪贴板记录`}
                        disabled={historyBusy}
                        label={entry.pinned ? "取消固定" : "固定"}
                      />
                    )}
                    {client?.remove && (
                      <ActionButton
                        action={() => void remove(entry.text)}
                        ariaLabel="删除剪贴板记录"
                        disabled={historyBusy}
                        label="删除"
                      />
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
