import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

export interface SheetOption {
  value: string;
  label: string;
  /** 加粗并带勾选标记绘制：当前选择。 */
  selected?: boolean;
  /** 以红色绘制，用于移除或丢弃某些内容的选项。 */
  destructive?: boolean;
  disabled?: boolean;
  /** 选择此选项会把面板切换到下一级而不是选中它；层级可以嵌套。 */
  submenu?: { title: string; options: SheetOption[] };
}

// 没有 `[data-platform]` 祖先设置平台 token 时，颜色回退为页面强调色和中性灰。
const scrim =
  "fixed inset-0 z-50 animate-ms-fade-in bg-[rgba(0,0,0,0.35)] motion-reduce:animate-none";
const frame =
  "pointer-events-none fixed inset-x-2 top-16 bottom-[calc(8px+env(safe-area-inset-bottom))] z-50 flex flex-col justify-end gap-2 [font-family:var(--p-font,inherit)] animate-ms-sheet-up motion-reduce:animate-none";
const card =
  "pointer-events-auto flex min-h-0 flex-[0_1_auto] flex-col overflow-hidden rounded-[14px] bg-[rgba(44,44,46,0.96)] backdrop-blur-xl light-theme:bg-[rgba(250,250,250,0.96)]";
const header =
  "relative flex flex-none flex-col gap-[3px] px-4 pt-3.5 pb-3 text-center text-[13px] [color:var(--p-sub,rgba(127,127,127,0.9))] [box-shadow:inset_0_-0.5px_0_var(--p-hair,rgba(127,127,127,0.3))]";
const back =
  "absolute top-1/2 left-2 flex size-8 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full border-0 bg-transparent p-0 [color:var(--p-accent-text,var(--accent-color))] active:bg-[var(--p-press,rgba(127,127,127,0.15))]";
const list = "min-h-0 flex-[0_1_auto] overflow-y-auto overscroll-contain [scrollbar-width:none]";
const option =
  "relative m-0 flex h-14 w-full flex-none cursor-pointer items-center justify-center border-0 bg-transparent px-12 text-[19px] [font-family:inherit] active:[background-image:linear-gradient(var(--p-press,rgba(127,127,127,0.15)),var(--p-press,rgba(127,127,127,0.15)))] disabled:cursor-default disabled:opacity-[.38] disabled:active:[background-image:none]";
const optionSeparator = "[box-shadow:inset_0_0.5px_0_var(--p-hair,rgba(127,127,127,0.3))]";
const cancel =
  "pointer-events-auto m-0 h-14 w-full flex-none cursor-pointer rounded-[14px] border-0 bg-[#2C2C2E] p-0 text-[19px] font-semibold [font-family:inherit] [color:var(--p-accent-text,var(--accent-color))] light-theme:bg-white active:[background-image:linear-gradient(var(--p-press,rgba(127,127,127,0.15)),var(--p-press,rgba(127,127,127,0.15)))]";

/**
 * 设计稿中的选择面板，对应 Android 的 `OptionSheet` 的 Web 版本：小号居中标题（及可选副标题），下方是一列居中的强调色选项，取消单独放在下面的一张卡片上。当前选项加粗，并在距末端 20px 处带勾选标记；破坏性选项为红色；带 `submenu` 的选项显示 ›，点击后把面板切换到该层级，标题栏有返回按钮可回到上一级。
 *
 * 选择某个选项时先关闭面板再报告选择，正如 `OptionSheet` 先关闭再执行动作，这样打开另一个面板或对话框的选择不会叠在这个面板上。遮罩、取消和 Escape 都会关闭它；在子菜单里按 Escape 则返回上一级。
 *
 * 它就地渲染并固定在视口上，所以继承打开它的页面的平台和季节 token；固定定位的元素不会被它周围的圆角分组裁切。设置页滑入完成后，不会对分组的任何祖先应用 transform 或 filter，否则面板会被固定到那个祖先而不是视口。
 */
export function ActionSheet({
  open,
  title,
  subtitle,
  options,
  onSelect,
  onClose,
}: {
  open: boolean;
  title: string;
  subtitle?: string;
  options: readonly SheetOption[];
  onSelect: (value: string) => void;
  onClose: () => void;
}) {
  // 目前已打开的子菜单选项，最外层在前；顶层时为空。
  const [trail, setTrail] = useState<SheetOption[]>([]);
  const dialog = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useLayoutEffect(() => {
    if (!open) return;
    const active = document.activeElement;
    opener.current = active instanceof HTMLElement ? active : null;
    setTrail([]);
    return () => {
      // 焦点回到原来的位置，除非选择把它移到了别处且那个元素仍然存在。
      const previous = opener.current;
      opener.current = null;
      if (
        previous?.isConnected &&
        (!document.activeElement || document.activeElement === document.body)
      ) {
        previous.focus();
      }
    };
  }, [open]);

  const level = trail.at(-1)?.submenu;
  const shown = level?.options ?? options;

  useEffect(() => {
    if (!open) return;
    dialog.current?.querySelector<HTMLButtonElement>("[data-sheet-option]:not(:disabled)")?.focus();
  }, [open, trail]);

  const choose = (item: SheetOption) => {
    if (item.disabled) return;
    if (item.submenu) {
      setTrail((current) => [...current, item]);
      return;
    }
    onClose();
    onSelect(item.value);
  };

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.stopPropagation();
      if (trail.length > 0) setTrail((current) => current.slice(0, -1));
      else onClose();
      return;
    }
    if (event.key !== "Tab") return;
    // 面板处于模态时焦点留在面板内：在最后一个控件上按 Tab 会回到第一个。
    const focusable = [
      ...(dialog.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []),
    ];
    const first = focusable[0];
    const last = focusable.at(-1);
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  if (!open) return null;
  // `display: contents` 不产生自己的盒子，所以放在分组各行之间的面板既不占用行间距，也不绘制行的分隔线。
  return (
    <div className="contents">
      <div className={scrim} aria-hidden="true" onClick={onClose} />
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={frame}
        onKeyDown={keyDown}
      >
        <div className={card}>
          <div className={header}>
            {level && (
              <button
                type="button"
                className={back}
                aria-label="返回上一级"
                onClick={() => setTrail((current) => current.slice(0, -1))}
              >
                <svg
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.6"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                >
                  <path d="m15 6-6 6 6 6" />
                </svg>
              </button>
            )}
            <span id={titleId} className="font-semibold">
              {level?.title ?? title}
            </span>
            {!level && subtitle && <span>{subtitle}</span>}
          </div>
          <div className={list}>
            {shown.map((item, index) => (
              <button
                key={item.value}
                type="button"
                data-sheet-option=""
                className={`${option} ${index > 0 ? optionSeparator : ""} ${
                  item.selected ? "font-semibold" : "font-normal"
                } ${
                  item.destructive
                    ? "[color:#FF3B30]"
                    : "[color:var(--p-accent-text,var(--accent-color))]"
                }`}
                disabled={item.disabled}
                aria-current={item.selected ? "true" : undefined}
                onClick={() => choose(item)}
              >
                {item.label}
                {item.submenu && <span aria-hidden="true">{" ›"}</span>}
                {item.selected && (
                  <svg
                    width="18"
                    height="18"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.6"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    aria-hidden="true"
                    className="absolute right-5"
                  >
                    <path d="m5 12.5 4.5 4.5L19 7.5" />
                  </svg>
                )}
              </button>
            ))}
          </div>
        </div>
        <button type="button" className={cancel} onClick={onClose}>
          取消
        </button>
      </div>
    </div>
  );
}
