import { useId } from "react";
import * as controls from "../core/platform-controls-style";

/** 打开设置子页的插件导航行；描述和尾部标记存在时才加入无障碍关联。 */
export interface PluginNavRowProps {
  title: string;
  description?: string;
  marker?: string | null;
  rowKey: string;
  onOpen: () => void;
}

/** 一行插件导航入口，`rowKey` 供详情页关闭后恢复焦点。 */
export function PluginNavRow({ title, description, marker, rowKey, onOpen }: PluginNavRowProps) {
  const descriptionId = useId();
  const markerId = useId();
  const describedBy = [description ? descriptionId : null, marker ? markerId : null]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type="button"
      className={`${controls.row} w-full cursor-pointer border-0 text-left hover:bg-[var(--p-hover)]`}
      aria-label={title}
      aria-describedby={describedBy || undefined}
      data-plugin-row={rowKey}
      onClick={onOpen}
    >
      <span className={controls.rowText}>
        <span className={controls.rowTitle}>{title}</span>
        {description && (
          <span id={descriptionId} className={controls.rowDescription}>
            {description}
          </span>
        )}
      </span>
      {marker && (
        <span id={markerId} className={`${controls.rowDescription} shrink-0`}>
          {marker}
        </span>
      )}
      <span className={controls.rowDescription} aria-hidden="true">
        ›
      </span>
    </button>
  );
}
