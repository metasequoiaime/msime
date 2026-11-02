import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
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
  nineKeySidebarOrdinal,
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

/** 把 `from` 那一项挪到 `to`，其余次序不变（挪动，不是交换），与 Android `ReorderPolicy.moved` 相同。 */
function moved(symbols: readonly string[], from: number, to: number): string[] {
  const next = [...symbols];
  const [symbol] = next.splice(from, 1);
  next.splice(to, 0, symbol);
  return next;
}

/** 拖动途中第 `index` 行此刻显示在第几位：被拖的那一行在 `to`，夹在两端之间的行朝 `from` 那一侧让一格，其余不动，与 Android `ReorderPolicy.dragSlot` 相同。 */
function dragSlot(index: number, from: number, to: number): number {
  if (index === from) return to;
  if (from < index && index <= to) return index - 1;
  if (to <= index && index < from) return index + 1;
  return index;
}

/** 手指离滚动区上下沿多近时开始自动滚动（px），以及每帧最多滚多少。 */
const edgeBand = 56;
const edgeStep = 12;

/** 手指在滚动区上下边缘带里时每帧滚多少（负数朝上），进入越深越快，与 Android `ReorderPolicy.edgeScrollStep` 相同；滚不动的方向是 0。 */
function edgeScrollStep(clientY: number, scroller: HTMLElement): number {
  const box = scroller.getBoundingClientRect();
  if (box.bottom - box.top <= 2 * edgeBand) return 0;
  const depth =
    clientY < box.top + edgeBand
      ? clientY - (box.top + edgeBand)
      : clientY > box.bottom - edgeBand
        ? clientY - (box.bottom - edgeBand)
        : 0;
  if (depth === 0) return 0;
  const step = Math.max(1, Math.round(edgeStep * Math.min(1, Math.abs(depth) / edgeBand)));
  if (depth < 0) return scroller.scrollTop > 0 ? -step : 0;
  return scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight ? step : 0;
}

/** 一次拖动在拿起时量好的几何：各行相对第一行的上沿、下沿和中线都是拿起那一刻的布局位置，之后只看手指和滚动，不再量行，行身上的跟手、让位位移不进落点判断。`listTop` 是列表上沿当时在视口里的位置，拖动中它挪了多少就是滚动区滚了多少。`pointerId` 是拿起这一行的那根手指，别的手指按在另一行把手上时发来的移动和抬起不算数。 */
type Gesture = {
  pointerId: number;
  from: number;
  to: number;
  offset: number;
  startY: number;
  lastY: number;
  listTop: number;
  tops: number[];
  bottoms: number[];
  centers: number[];
  scroller: HTMLElement | null;
};

/** 拖动中要画出来的那部分：谁被拿起、此刻落在哪、跟手位移和让位距离。 */
type Drag = { from: number; to: number; offset: number; height: number };

/** 松手后被拖行滑进自己格子的那一下：先带着离格子的距离画出来，隔一次绘制再过渡到 0。 */
type Settle = { symbol: string; offset: number; running: boolean };

/** 挪一格后要把焦点放回哪一行的哪个控件：表重排时 React 可能把那一行的节点摘下再插回，浏览器会把焦点丢回页面。 */
type Refocus = { symbol: string; control: "handle" | "up" | "down" };

/** 被拖行还能朝 `step` 的方向挪才自动滚动：它已经夹在列表上沿或下沿时再滚，列表带着它滚走，行就离开手指了；与 Android `ReorderPolicy.scrollTowardRoom` 相同。 */
function scrollTowardRoom(step: number, gesture: Gesture): number {
  if (
    step > 0 &&
    gesture.offset >= gesture.bottoms[gesture.bottoms.length - 1] - gesture.bottoms[gesture.from]
  )
    return 0;
  if (step < 0 && gesture.offset <= gesture.tops[0] - gesture.tops[gesture.from]) return 0;
  return step;
}

const iconButton =
  "m-0 inline-flex size-11 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 bg-transparent p-0 [color:var(--p-sub)] focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-default disabled:opacity-40";

/**
 * 九键左侧符号栏一张表的编辑子页（#10223），照 Android `NineKeySidebarSheet` 的交互，字母表和数字表共用：顶部一行说明；「当前顺序」逐项列出，每项前有序号 1..N，可删除、按住把手上下拖动排序（#10396：被拖行浮起跟着手指走，其余行滑开让位，序号随位次实时变化，贴近页面上下沿时自动滚动，被拖行到了列表首尾就不再滚；键盘在把手上用 ↑ ↓ 挪一格，触屏读屏用每行视觉隐藏的「上移」「下移」按钮，对应 Android 把手上的同名无障碍操作）；「可加入的常用符号」是 `nineKeySidebarAddableSymbols` 里还不在表里的项，点一下接到末尾；也可以手动输入一项。「恢复默认」把表换回鸿蒙出厂表，「保存」在整张表合规时才可点（`validNineKeySidebar`）。
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
  const [drag, setDrag] = useState<Drag | null>(null);
  const [settle, setSettle] = useState<Settle | null>(null);
  const gestureRef = useRef<Gesture | null>(null);
  const frame = useRef<number | null>(null);
  const refocus = useRef<Refocus | null>(null);
  const list = useRef<HTMLOListElement>(null);
  const title = titles[kind];
  const manual = typed.trim();
  const addable = nineKeySidebarAddableSymbols.filter((symbol) => !draft.includes(symbol));
  const full = draft.length >= nineKeySidebarMaxSymbols;

  const add = (symbol: string) => {
    if (canAddNineKeySidebarSymbol(draft, symbol)) setDraft([...draft, symbol]);
  };
  const remove = (index: number) => {
    cancelDrag();
    setDraft(draft.filter((_, at) => at !== index));
  };
  const rowsOf = () =>
    [...(list.current?.children ?? [])].filter(
      (node): node is HTMLElement => node instanceof HTMLElement,
    );
  // 跟着手指走：位移是手指移动的距离加上拖动中滚动区滚过的距离（列表上沿在屏幕上挪了多少），夹在列表上下沿之间；落点只比拿起时量好的各行中线，不量此刻带着位移的行（照 Android `ReorderPolicy.dragTarget`）。
  const follow = () => {
    const gesture = gestureRef.current;
    if (!gesture || !list.current) return;
    const scrolled = gesture.listTop - list.current.getBoundingClientRect().top;
    const top = gesture.tops[gesture.from];
    const bottom = gesture.bottoms[gesture.from];
    const offset = Math.min(
      Math.max(gesture.lastY - gesture.startY + scrolled, gesture.tops[0] - top),
      gesture.bottoms[gesture.bottoms.length - 1] - bottom,
    );
    const center = gesture.centers[gesture.from] + offset;
    gesture.offset = offset;
    // 中线碰到哪一行的中线就算到了那一行：被拖行夹在列表里，拖到底时中线正好压在最后一行的中线上。
    gesture.to = gesture.centers.filter((other, index) =>
      index < gesture.from ? other < center : index > gesture.from && other <= center,
    ).length;
    setDrag({ from: gesture.from, to: gesture.to, offset, height: bottom - top });
    scheduleScroll();
  };
  // 贴近滚动区上下沿时每帧滚一步再重新跟手，滚到头、被拖行到了列表首尾或离开边缘带就停。
  const pageStep = (gesture: Gesture) =>
    gesture.scroller
      ? scrollTowardRoom(edgeScrollStep(gesture.lastY, gesture.scroller), gesture)
      : 0;
  const scrollStep = () => {
    frame.current = null;
    const gesture = gestureRef.current;
    if (!gesture?.scroller) return;
    const step = pageStep(gesture);
    if (step === 0) return;
    gesture.scroller.scrollTop += step;
    follow();
  };
  const scheduleScroll = () => {
    const gesture = gestureRef.current;
    if (frame.current !== null || !gesture) return;
    if (pageStep(gesture) !== 0) frame.current = requestAnimationFrame(scrollStep);
  };
  const stopScroll = () => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
  };
  // 拖动中表被别的操作改掉（另一指点了删除、恢复默认）时作废这次拖动，拿起时量的几何已经对不上了。
  const cancelDrag = () => {
    gestureRef.current = null;
    stopScroll();
    setDrag(null);
  };
  // 拖动中离开这一页时不留着自动滚动的帧回调。
  useEffect(
    () => () => {
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    },
    [],
  );
  // 松手后的落位：带着离格子的距离画完一帧，下一帧再加上过渡回到 0；中间隔一次绘制，过渡才有起点。
  useLayoutEffect(() => {
    if (!settle || settle.running) return;
    let next = 0;
    const first = requestAnimationFrame(() => {
      next = requestAnimationFrame(() => setSettle({ ...settle, running: true }));
    });
    return () => {
      cancelAnimationFrame(first);
      cancelAnimationFrame(next);
    };
  }, [settle]);
  // 键盘 ↑ ↓ 或读屏「上移」「下移」挪完一格后，焦点回到挪过去那一行的同一个控件；挪到首尾、那个方向的按钮没了时换成另一个方向的。
  useLayoutEffect(() => {
    const target = refocus.current;
    refocus.current = null;
    const row = target && rowsOf().find((node) => node.dataset.sidebarSymbol === target.symbol);
    if (!target || !row) return;
    const order =
      target.control === "handle"
        ? ["handle"]
        : target.control === "up"
          ? ["up", "down"]
          : ["down", "up"];
    for (const control of order) {
      const button = row.querySelector<HTMLButtonElement>(`[data-sidebar-control="${control}"]`);
      if (button) {
        button.focus();
        return;
      }
    }
  }, [draft]);
  const startDrag = (index: number) => (event: PointerEvent<HTMLButtonElement>) => {
    const rows = rowsOf();
    if (gestureRef.current || !list.current || !rows[index]) return;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    // 行的位置取 offsetTop / offsetHeight：它们是布局位置，不含 transform，上一次落位的过渡还没走完、或别的行正带着让位位移时量出来也不偏。
    const base = rows[0].offsetTop;
    const tops = rows.map((row) => row.offsetTop - base);
    const heights = rows.map((row) => row.offsetHeight);
    gestureRef.current = {
      pointerId: event.pointerId,
      from: index,
      to: index,
      offset: 0,
      startY: event.clientY,
      lastY: event.clientY,
      listTop: list.current.getBoundingClientRect().top,
      tops,
      bottoms: tops.map((top, at) => top + heights[at]),
      centers: tops.map((top, at) => top + heights[at] / 2),
      scroller: list.current.closest<HTMLElement>("[data-nine-key-sidebar-editor]"),
    };
    setSettle(null);
    setDrag({ from: index, to: index, offset: 0, height: heights[index] });
  };
  const dragMove = (event: PointerEvent<HTMLButtonElement>) => {
    const gesture = gestureRef.current;
    if (!gesture || event.pointerId !== gesture.pointerId) return;
    gesture.lastY = event.clientY;
    follow();
  };
  // 松手：表按挪动语义改好，其余行此刻已经在新位置上，重画后不动；被拖行从手指处滑进自己的格子。
  const endDrag = (event: PointerEvent<HTMLButtonElement>) => {
    const gesture = gestureRef.current;
    if (!gesture || event.pointerId !== gesture.pointerId) return;
    gestureRef.current = null;
    stopScroll();
    const { from, to, offset } = gesture;
    const slot =
      to > from
        ? gesture.bottoms[to] - gesture.bottoms[from]
        : gesture.tops[to] - gesture.tops[from];
    setDrag(null);
    if (to !== from) setDraft(moved(draft, from, to));
    // 系统要求减少动态效果时没有落位过渡，也就等不到 transitionend，直接落进格子。
    const reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    setSettle(
      offset === slot || reducedMotion
        ? null
        : { symbol: draft[from], offset: offset - slot, running: false },
    );
  };
  // 挪一格（键盘 ↑ ↓、读屏「上移」「下移」），拖动中不挪。
  const moveOne = (index: number, to: number, control: Refocus["control"]) => {
    if (to < 0 || to >= draft.length || gestureRef.current) return;
    setSettle(null);
    refocus.current = { symbol: draft[index], control };
    setDraft(moved(draft, index, to));
  };
  const nudge = (index: number) => (event: KeyboardEvent<HTMLButtonElement>) => {
    const to =
      event.key === "ArrowUp" ? index - 1 : event.key === "ArrowDown" ? index + 1 : undefined;
    if (to === undefined) return;
    event.preventDefault();
    moveOne(index, to, "handle");
  };
  // 一行此刻的样子：拖动中被拖行跟手、浮在其余行上面，其余行按位次让位并带过渡；松手后落位的那一行先停在离格子的距离，再过渡回去。
  // 浮起的阴影改写行自己的 `--p-row-shadow`：行模板的 `[box-shadow:var(--p-row-shadow)]` 在样式表里排在 `shadow-*` 工具类后面，各平台这个变量又都是 none，另加 `shadow-lg` 会被它盖掉，浮起的行就和别的行一样平。
  const rowLook = (symbol: string, index: number) => {
    const motion = "transition-transform duration-150 ease-out motion-reduce:transition-none";
    const lifted =
      "relative z-10 [--p-row-shadow:0_8px_20px_rgb(0_0_0/0.24)] [background:linear-gradient(var(--p-group-bg),var(--p-group-bg)),var(--p-bg)]";
    let style: CSSProperties | undefined;
    let className = "";
    let position = index;
    if (drag) {
      position = dragSlot(index, drag.from, drag.to);
      if (index === drag.from) {
        style = { transform: `translateY(${drag.offset}px)` };
        className = lifted;
      } else {
        const shift = (position - index) * drag.height;
        if (shift !== 0) style = { transform: `translateY(${shift}px)` };
        className = motion;
      }
    } else if (settle?.symbol === symbol) {
      if (!settle.running) style = { transform: `translateY(${settle.offset}px)` };
      className = settle.running ? `${lifted} ${motion}` : lifted;
    }
    return { style, className, ordinal: nineKeySidebarOrdinal(position) };
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
              {draft.map((symbol, index) => {
                const look = rowLook(symbol, index);
                return (
                  <li
                    key={symbol}
                    className={`${rowStyle} ${look.className}`}
                    style={look.style}
                    data-sidebar-symbol={symbol}
                    onTransitionEnd={() => {
                      if (settle?.running && settle.symbol === symbol) setSettle(null);
                    }}
                  >
                    {/* 序号按两位数留宽、右对齐、等宽数字；读屏从有序列表本身得知位置，序号不念。 */}
                    <span
                      aria-hidden="true"
                      className="w-8 shrink-0 text-right tabular-nums [color:var(--p-sub)]"
                      data-sidebar-ordinal=""
                    >
                      {look.ordinal}
                    </span>
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
                      onPointerMove={dragMove}
                      onPointerUp={endDrag}
                      onPointerCancel={endDrag}
                      onKeyDown={nudge(index)}
                      data-sidebar-control="handle"
                    >
                      <FluentIcon name="arrow_sort" />
                    </button>
                    {/* 触屏读屏没有方向键，把手又只能拖：每行另给视觉隐藏的「上移」「下移」，读屏逐项滑动能读到、双击就挪一格；不进 Tab 次序，键盘照旧在把手上按 ↑ ↓。 */}
                    {index > 0 && (
                      <button
                        type="button"
                        className="sr-only"
                        tabIndex={-1}
                        aria-label={`上移 ${symbol}`}
                        data-sidebar-control="up"
                        onClick={() => moveOne(index, index - 1, "up")}
                      />
                    )}
                    {index < draft.length - 1 && (
                      <button
                        type="button"
                        className="sr-only"
                        tabIndex={-1}
                        aria-label={`下移 ${symbol}`}
                        data-sidebar-control="down"
                        onClick={() => moveOne(index, index + 1, "down")}
                      />
                    )}
                    <button
                      type="button"
                      className={iconButton}
                      aria-label={`删除 ${symbol}`}
                      onClick={() => remove(index)}
                    >
                      <FluentIcon name="delete" />
                    </button>
                  </li>
                );
              })}
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
            action={() => {
              cancelDrag();
              setDraft([...nineKeySidebarDefaults[kind]]);
            }}
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
