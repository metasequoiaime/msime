import { useId } from "react";
import type { CloudClipboardItem } from "./panel-client";
import * as cloud from "./cloud-panel-style";

export interface CloudClipboardHistoryProps {
  items: readonly CloudClipboardItem[];
  busy: boolean;
  copyAvailable: boolean;
  onChoose: (item: CloudClipboardItem, copyOnly?: boolean) => void;
  onRemove: (id: string) => void;
}

/** 条目下方的元信息，拼法与 macOS 原生云剪贴板窗口一致：置顶的写「已置顶」，再接来源设备名，用「 · 」连接。 */
export function cloudClipboardItemMeta(
  item: Pick<CloudClipboardItem, "pinned" | "device">,
): string {
  return [item.pinned === true ? "已置顶" : "", item.device ?? ""].filter(Boolean).join(" · ");
}

/** 受控的云端历史列表，宿主操作与刷新由调用方负责。 */
export function CloudClipboardHistory({
  items,
  busy,
  copyAvailable,
  onChoose,
  onRemove,
}: CloudClipboardHistoryProps) {
  const listId = useId();
  return (
    <div className={cloud.clipboardList} aria-label="云端历史">
      {items.length ? (
        items.map((item, index) => (
          <article className={cloud.clipboardItem} key={item.id}>
            <button
              type="button"
              disabled={busy}
              // 元信息画在按钮里、跟着文字换行，但按钮的名称仍只是条目文字，元信息作为描述读出。
              aria-label={cloudClipboardItemMeta(item) ? item.text : undefined}
              aria-describedby={cloudClipboardItemMeta(item) ? `${listId}-${index}` : undefined}
              onClick={() => void onChoose(item)}
            >
              {item.text}
              {cloudClipboardItemMeta(item) && (
                <small className={cloud.clipboardItemMeta} id={`${listId}-${index}`}>
                  {cloudClipboardItemMeta(item)}
                </small>
              )}
            </button>
            {copyAvailable && (
              <button
                type="button"
                disabled={busy}
                aria-label="复制此条云端记录"
                onClick={() => void onChoose(item, true)}
              >
                复制
              </button>
            )}
            <button
              type="button"
              className={cloud.clipboardDelete}
              aria-label={`删除 ${item.text}`}
              onClick={() => void onRemove(item.id)}
              disabled={busy}
            >
              删除
            </button>
          </article>
        ))
      ) : (
        <p className={cloud.clipboardNote}>暂无云端历史</p>
      )}
    </div>
  );
}
