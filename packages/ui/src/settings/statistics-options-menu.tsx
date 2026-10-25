import { ControlledCheckbox } from "../core/controlled-checkbox";
import { ActionButton } from "./action-button";
import { retentionChoices, type StatisticsRetention } from "./typing-statistics-model";

// 手机菜单沿用原生 details/summary 展开状态，不另建 React 状态。
const menuSummary =
  "grid h-10 w-10 cursor-pointer list-none place-items-center rounded-xl border border-edge bg-card-solid text-2xl leading-none text-secondary hover:bg-[var(--button-secondary-hover)] hover:text-body [&::-webkit-details-marker]:hidden";
const menuPopover =
  "absolute top-12 right-0 flex w-[320px] max-w-[calc(100vw-32px)] flex-col overflow-hidden rounded-[20px] border border-edge bg-card-solid p-2 text-sm text-body shadow-[0_12px_40px_rgba(0,0,0,0.18)]";
const menuItem =
  "flex min-h-12 w-full items-center gap-3 rounded-xl border-0 bg-transparent px-3 py-3 text-left text-sm font-medium leading-5 text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";

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
          <div className="px-3 pt-2 pb-3">
            <p className="m-0 text-base font-semibold leading-6">统计选项</p>
            <p className="mt-1 mb-0 text-xs leading-5 text-secondary">
              统计只包含计数，不记录输入内容
            </p>
          </div>
          <label className={`${menuItem} justify-between`}>
            <span className="min-w-0">记录打字统计</span>
            <ControlledCheckbox
              aria-label="记录打字统计"
              className="toggle shrink-0"
              checked={enabled}
              disabled={busy}
              onChange={onEnabledChange}
            />
          </label>
          {onRetentionChange && (
            <label className="flex flex-col gap-2 px-3 pt-2 pb-4">
              <span className="text-sm font-medium leading-5">自动清理</span>
              <select
                aria-label="自动清理"
                className="min-h-11 w-full rounded-xl border border-edge bg-subtle px-3 py-2 text-sm leading-5 text-body"
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
          <div className="mx-3 my-1 border-t border-edge" />
          <ActionButton
            action={onRefresh}
            ariaBusy={busy}
            className={menuItem}
            disabled={busy}
            label={busy ? "处理中…" : "刷新统计"}
            role="menuitem"
          />
          <ActionButton
            action={onReset}
            className={`${menuItem} text-danger not-disabled:hover:bg-[color-mix(in_srgb,var(--danger-text)_8%,transparent)]`}
            disabled={busy}
            label="清空统计"
            role="menuitem"
          />
        </div>
      </details>
    </div>
  );
}
