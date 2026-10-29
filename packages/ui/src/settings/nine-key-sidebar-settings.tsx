import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { GroupList } from "../core/platform-controls";
import { row as rowStyle, rowTitle, textInput } from "../core/platform-controls-style";
import { ActionButton } from "../core/action-button";
import { ControlledInput } from "../core/controlled-input";
import { FluentIcon } from "../core/fluent-icons";
import { fullScreenColumn, fullScreenSurface } from "../core/full-screen-style";
import { SettingsPageHeader } from "./settings-page-header";
import {
  canAddNineKeySidebarSymbol,
  nineKeySidebarAddableSymbols,
  nineKeySidebarDefaults,
  nineKeySidebarMaxSymbols,
  nineKeySidebarNote,
  nineKeySidebarTitle,
  numberPadSidebarNote,
  numberPadSidebarTitle,
  validNineKeySidebar,
  type NineKeySidebarKind,
} from "../keyboard/touch-key-layout-styles";

/** 两张表的标题和顶部说明，三端逐字相同。 */
const titles: Readonly<Record<NineKeySidebarKind, string>> = {
  letter: nineKeySidebarTitle,
  digit: numberPadSidebarTitle,
};
const notes: Readonly<Record<NineKeySidebarKind, string>> = {
  letter: nineKeySidebarNote,
  digit: numberPadSidebarNote,
};

/** 把 `from` 那一项挪到 `to`，其余次序不变。 */
function moved(symbols: readonly string[], from: number, to: number): string[] {
  const next = [...symbols];
  const [symbol] = next.splice(from, 1);
  next.splice(to, 0, symbol);
  return next;
}

const iconButton =
  "m-0 inline-flex size-11 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 bg-transparent p-0 [color:var(--p-sub)] focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-default disabled:opacity-40";

/**
 * 九键左侧符号栏一张表的编辑子页（#10223），照 Android `NineKeySidebarSheet` 的交互，字母表和数字表共用：顶部一行说明；「当前顺序」逐项列出，可删除、按住把手上下拖动排序（读屏和键盘用 ↑ ↓ 挪一格）；「可加入的常用符号」是 `nineKeySidebarAddableSymbols` 里还不在表里的项，点一下接到末尾；也可以手动输入一项。「恢复默认」把表换回鸿蒙出厂表，「保存」在整张表合规时才可点（`validNineKeySidebar`）。
 *
 * 改动只在这一页里，保存时才经 `onSave` 交出整张表（写空数组还是用户的表由调用方按 `nineKeySidebarStoredValue` 定），返回不保存。不画键盘预览，回到「按键布局」在 9 键预览里看。
 */
export function NineKeySidebarSettings({
  kind,
  symbols,
  onSave,
  onBack,
}: {
  kind: NineKeySidebarKind;
  /** 打开时这张表实际画的符号，空表时是出厂表。 */
  symbols: readonly string[];
  onSave: (symbols: string[]) => void;
  onBack: () => void;
}) {
  const [draft, setDraft] = useState<string[]>(() => [...symbols]);
  const [typed, setTyped] = useState("");
  const [dragging, setDragging] = useState<number | null>(null);
  const list = useRef<HTMLOListElement>(null);
  const title = titles[kind];
  const manual = typed.trim();
  const addable = nineKeySidebarAddableSymbols.filter((symbol) => !draft.includes(symbol));
  const full = draft.length >= nineKeySidebarMaxSymbols;

  const add = (symbol: string) => {
    if (canAddNineKeySidebarSymbol(draft, symbol)) setDraft([...draft, symbol]);
  };
  const remove = (index: number) => setDraft(draft.filter((_, at) => at !== index));
  // 拖动中手指所在的位置落在哪一项上：照 Android `dragTarget`，越过某一项的中线就算到了它那里。
  const target = (clientY: number) => {
    const rows = [...(list.current?.children ?? [])];
    const index = rows.findIndex((row) => {
      const box = row.getBoundingClientRect();
      return clientY < box.top + box.height / 2;
    });
    return index < 0 ? Math.max(0, rows.length - 1) : index;
  };
  const startDrag = (index: number) => (event: PointerEvent<HTMLButtonElement>) => {
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setDragging(index);
  };
  const drag = (event: PointerEvent<HTMLButtonElement>) => {
    if (dragging === null) return;
    const to = target(event.clientY);
    if (to === dragging) return;
    setDraft(moved(draft, dragging, to));
    setDragging(to);
  };
  const endDrag = () => setDragging(null);
  const nudge = (index: number) => (event: KeyboardEvent<HTMLButtonElement>) => {
    const to =
      event.key === "ArrowUp" ? index - 1 : event.key === "ArrowDown" ? index + 1 : undefined;
    if (to === undefined) return;
    event.preventDefault();
    if (to < 0 || to >= draft.length) return;
    setDraft(moved(draft, index, to));
  };

  return (
    <div
      className={fullScreenSurface}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-settings-search-ignore=""
      data-nine-key-sidebar-editor={kind}
    >
      <section className={fullScreenColumn}>
        <SettingsPageHeader title={title} onBack={onBack} level={2} />
        <p className="m-0 px-1 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]">{notes[kind]}</p>
        <GroupList title="当前顺序">
          {draft.length === 0 ? (
            <p className={`${rowStyle} m-0 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]`}>
              当前没有符号，至少留一个才能保存。
            </p>
          ) : (
            <ol ref={list} className="m-0 flex list-none flex-col p-0" aria-label="当前顺序">
              {draft.map((symbol, index) => (
                <li
                  key={symbol}
                  className={`${rowStyle} ${dragging === index ? "opacity-70" : ""}`}
                  data-sidebar-symbol={symbol}
                >
                  <span className={`${rowTitle} min-w-0 flex-1`}>
                    <span className="inline-flex min-w-9 items-center justify-center rounded-[10px] bg-[var(--accent-soft)] px-3 py-1">
                      {symbol}
                    </span>
                  </span>
                  <button
                    type="button"
                    className={`${iconButton} touch-none`}
                    aria-label={`拖动调整 ${symbol} 的顺序`}
                    onPointerDown={startDrag(index)}
                    onPointerMove={drag}
                    onPointerUp={endDrag}
                    onPointerCancel={endDrag}
                    onKeyDown={nudge(index)}
                  >
                    <FluentIcon name="arrow_sort" />
                  </button>
                  <button
                    type="button"
                    className={iconButton}
                    aria-label={`删除 ${symbol}`}
                    onClick={() => remove(index)}
                  >
                    <FluentIcon name="delete" />
                  </button>
                </li>
              ))}
            </ol>
          )}
        </GroupList>
        <GroupList title="可加入的常用符号">
          <div className={`${rowStyle} flex-wrap gap-2`}>
            {addable.map((symbol) => (
              <button
                key={symbol}
                type="button"
                className="m-0 min-w-11 cursor-pointer rounded-full border-0 bg-[var(--p-seg-bg)] px-3 py-2 text-[15px] [color:var(--p-text)] [font-family:inherit] focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-default disabled:opacity-40"
                aria-label={`添加 ${symbol}`}
                disabled={!canAddNineKeySidebarSymbol(draft, symbol)}
                onClick={() => add(symbol)}
              >
                {symbol}
              </button>
            ))}
          </div>
          <div className={`${rowStyle} gap-2`}>
            <ControlledInput
              className={`${textInput} min-w-0 flex-1`}
              aria-label="手动输入一个符号"
              placeholder="手动输入一个符号"
              value={typed}
              onChange={setTyped}
            />
            <ActionButton
              label="添加"
              ariaLabel="添加输入的符号"
              disabled={!canAddNineKeySidebarSymbol(draft, manual)}
              action={() => {
                add(manual);
                setTyped("");
              }}
            />
          </div>
          {full && (
            <p className={`${rowStyle} m-0 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]`}>
              最多 {nineKeySidebarMaxSymbols} 个符号。
            </p>
          )}
        </GroupList>
        <div className="flex justify-end gap-2 px-1">
          <ActionButton
            label="恢复默认"
            action={() => setDraft([...nineKeySidebarDefaults[kind]])}
          />
          <ActionButton
            label="保存"
            className="primary"
            disabled={!validNineKeySidebar(draft)}
            action={() => onSave(draft)}
          />
        </div>
      </section>
    </div>
  );
}
