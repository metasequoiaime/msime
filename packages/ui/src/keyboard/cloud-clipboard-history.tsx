import type { CloudClipboardItem } from "./panel-client";
import * as cloud from "./cloud-panel-style";

export interface CloudClipboardHistoryProps {
  items: readonly CloudClipboardItem[];
  busy: boolean;
  copyAvailable: boolean;
  onChoose: (item: CloudClipboardItem, copyOnly?: boolean) => void;
  onRemove: (id: string) => void;
}

/** 受控的云端历史列表，宿主操作与刷新由调用方负责。 */
export function CloudClipboardHistory({
  items,
  busy,
  copyAvailable,
  onChoose,
  onRemove,
}: CloudClipboardHistoryProps) {
  return (
    <div className={cloud.clipboardList} aria-label="云端历史">
      {items.length ? (
        items.map((item) => (
          <article className={cloud.clipboardItem} key={item.id}>
            <button type="button" disabled={busy} onClick={() => void onChoose(item)}>
              {item.text}
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
