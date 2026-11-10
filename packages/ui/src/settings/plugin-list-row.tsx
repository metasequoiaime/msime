import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Switch } from "../core/platform-controls";
import { FluentIcon } from "../core/fluent-icons";
import { PluginNavRow, type PluginNavRowProps } from "./plugin-nav-row";

/** 行尾的启用开关：只有指令表、短语表这类可以同时启用多个的插件才有。 */
export interface PluginRowToggle {
  enabled: boolean;
  /** 已到启用上限、这一个又没启用时不能再打开。 */
  disabled: boolean;
  onChange: (enabled: boolean) => void;
}

/** 行尾的排序把手：`previous`、`next` 是同组里相邻两行的排序键，在首尾时为空。 */
export interface PluginRowReorder {
  /** 这一行的排序键（`pluginKey`）。 */
  key: string;
  previous?: string;
  next?: string;
  /** 把 `source` 挪到 `target` 现在的位置；只改展示顺序。 */
  onMove: (source: string, target: string) => void;
}

const iconButton =
  "m-0 inline-flex size-11 shrink-0 cursor-pointer touch-none items-center justify-center rounded-full border-0 bg-transparent p-0 [color:var(--p-sub)] focus-visible:outline-2 focus-visible:outline-accent";

/** 手指此刻压着同一组里的哪一行；不在任何一行上、或在别的组里时为空。 */
function rowUnder(event: PointerEvent<HTMLElement>, group: Element | null): string | undefined {
  const node = document.elementFromPoint(event.clientX, event.clientY);
  const row = node?.closest<HTMLElement>("[data-plugin-order-key]");
  if (!row || !group || !group.contains(row)) return undefined;
  return row.dataset.pluginOrderKey;
}

/**
 * 分组插件列表的一行：点行本身照旧打开详情，行尾可以带启用开关和排序把手（#11076）。把手用指针事件而不是 HTML5 拖放，触屏的 ArkWeb 里才拖得动：按住把手拖到同组另一行上松手，就挪到那一行的位置；键盘在把手上按 ↑ ↓ 挪一格，触屏读屏用视觉隐藏的「上移」「下移」。没有开关也没有把手时就是原来的导航行。
 */
export function PluginListRow({
  toggle,
  reorder,
  ...row
}: PluginNavRowProps & { toggle?: PluginRowToggle; reorder?: PluginRowReorder }) {
  const [target, setTarget] = useState<string | null>(null);
  const dragging = useRef<number | null>(null);
  if (!toggle && !reorder) return <PluginNavRow {...row} />;

  const group = (node: Element) => node.closest("[data-plugin-order-group]");
  const start = (event: PointerEvent<HTMLButtonElement>) => {
    event.currentTarget.setPointerCapture?.(event.pointerId);
    dragging.current = event.pointerId;
    setTarget(reorder?.key ?? null);
  };
  const move = (event: PointerEvent<HTMLButtonElement>) => {
    if (dragging.current !== event.pointerId) return;
    setTarget(rowUnder(event, group(event.currentTarget)) ?? reorder?.key ?? null);
  };
  const end = (event: PointerEvent<HTMLButtonElement>) => {
    if (dragging.current !== event.pointerId) return;
    dragging.current = null;
    const over =
      event.type === "pointerup" ? rowUnder(event, group(event.currentTarget)) : undefined;
    setTarget(null);
    if (reorder && over && over !== reorder.key) reorder.onMove(reorder.key, over);
  };
  const nudge = (event: KeyboardEvent<HTMLButtonElement>) => {
    const to =
      event.key === "ArrowUp"
        ? reorder?.previous
        : event.key === "ArrowDown"
          ? reorder?.next
          : undefined;
    if (!reorder || !to) return;
    event.preventDefault();
    reorder.onMove(reorder.key, to);
  };

  return (
    <div
      className="flex min-w-0 items-center"
      data-plugin-order-key={reorder?.key}
      // 拖动中标出把手所在的行，松手前看得出抓的是哪一个。
      data-plugin-dragging={target !== null ? "" : undefined}
    >
      <div className="min-w-0 flex-1">
        <PluginNavRow {...row} />
      </div>
      {toggle && (
        <span className="shrink-0 pr-2">
          <Switch
            checked={toggle.enabled}
            disabled={toggle.disabled}
            onChange={toggle.onChange}
            aria-label={`${row.title}启用`}
          />
        </span>
      )}
      {reorder && (
        <>
          <button
            type="button"
            className={iconButton}
            aria-label={`拖动调整 ${row.title} 的顺序`}
            onPointerDown={start}
            onPointerMove={move}
            onPointerUp={end}
            onPointerCancel={end}
            onKeyDown={nudge}
            data-plugin-order-handle=""
          >
            <FluentIcon name="arrow_sort" />
          </button>
          {reorder.previous && (
            <button
              type="button"
              className="sr-only"
              tabIndex={-1}
              aria-label={`上移 ${row.title}`}
              onClick={() => reorder.previous && reorder.onMove(reorder.key, reorder.previous)}
            />
          )}
          {reorder.next && (
            <button
              type="button"
              className="sr-only"
              tabIndex={-1}
              aria-label={`下移 ${row.title}`}
              onClick={() => reorder.next && reorder.onMove(reorder.key, reorder.next)}
            />
          )}
        </>
      )}
    </div>
  );
}
