import { ControlledCheckbox } from "../core/controlled-checkbox";
import { ActionButton } from "./action-button";
import { retentionChoices, type StatisticsRetention } from "./typing-statistics-model";

// 手机菜单沿用原生 details/summary 展开状态，不另建 React 状态。
const menuSummary =
  "grid h-[30px] w-[34px] cursor-pointer list-none place-items-center rounded-[9px] border border-edge bg-card text-xl leading-none text-secondary hover:bg-[var(--button-secondary-hover)] hover:text-body [&::-webkit-details-marker]:hidden";
const menuPopover =
  "absolute top-9 right-0 flex min-w-[180px] flex-col gap-[3px] rounded-[10px] border border-edge bg-card p-1.5 shadow-card";
const menuItem =
  "flex min-h-[34px] w-full items-center justify-between gap-3 rounded-[7px] border-0 bg-transparent px-[9px] py-1.5 text-left text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";

export function StatisticsOptionsMenu({
  className,
  enabled,
  busy,
  retention,
  onEnabledChange,
  onRetentionChange,
  onRefresh,
  onReset,
}: {
  className: string;
  enabled: boolean;
  busy: boolean;
  retention?: StatisticsRetention;
  onEnabledChange: (enabled: boolean) => void;
  onRetentionChange?: (retention: StatisticsRetention) => void;
  onRefresh: () => void;
  onReset: () => void;
}) {
  return (
    <div className={`${className} flex min-h-0 justify-end`}>
      <details className="relative z-[3]">
        <summary className={menuSummary} aria-label="统计选项">
          ⋯
        </summary>
        <div className={menuPopover} role="menu" aria-label="统计选项">
          <label className={menuItem}>
            <span>记录打字统计</span>
            <ControlledCheckbox
              aria-label="记录打字统计"
              className="toggle"
              checked={enabled}
              disabled={busy}
              onChange={onEnabledChange}
            />
          </label>
          {onRetentionChange && (
            <label className={menuItem}>
              <span>自动清理</span>
              <select
                aria-label="自动清理"
                className="max-w-[140px] rounded-[6px] border border-edge bg-transparent px-1.5 py-1 text-xs text-body"
                value={retention ?? "forever"}
                disabled={busy}
                onChange={(event) => {
                  onRetentionChange(event.target.value as StatisticsRetention);
                }}
              >
                {retentionChoices.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          )}
          <ActionButton
            action={onRefresh}
            ariaBusy={busy}
            className=""
            disabled={busy}
            label={busy ? "处理中…" : "刷新统计"}
            role="menuitem"
          />
          <ActionButton
            action={onReset}
            className={`${menuItem} text-danger`}
            disabled={busy}
            label="清空统计"
            role="menuitem"
          />
        </div>
      </details>
    </div>
  );
}
