import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import * as style from "./platform-controls-style";

/** 气泡与「?」之间、与窗口边缘之间留出的距离（px）。 */
const GAP = 6;
const EDGE = 8;

/**
 * 设置项旁边的「?」：说明不常驻页面，悬停、键盘聚焦或点按「?」时在气泡里显示，点别处或按 Esc 收起。点按会让气泡保持打开，HarmonyOS 手机没有悬停，靠的就是这一下。
 *
 * 说明文字始终以视觉隐藏的形式留在原处并带上 `id`，行内控件的 `aria-describedby` 照旧指向它，读屏和按文字查找都不受影响；气泡只是它的可见副本，挂到设置窗口根节点（`[data-settings-shell]`）下按固定定位画出，不会被分组卡片的 `overflow-hidden` 裁掉，也照样拿到平台的配色 token。
 *
 * `passive` 用在本身就是按钮或 `<label>` 的元素里（整行是按钮的跳转行、旧式 label 包裹的字段）：那里不能再嵌按钮，嵌进 label 还会让它取代输入框成为 label 指向的控件，所以「?」只是一个悬停才出气泡的图标，说明由外层元素的描述或标签读出。
 */
export function InfoTip({
  id,
  label = "说明",
  passive = false,
  className,
  children,
}: {
  /** 视觉隐藏的说明文字的 id，供外层控件的 `aria-describedby` 引用。 */
  id?: string;
  /** 「?」按钮的名称。缺省「说明」：不以设置标题开头，免得和按标题找控件的查询、读屏的按首字跳转混在一起；说明内容由 `aria-describedby` 读出。 */
  label?: string;
  passive?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const ownId = useId();
  const textId = id ?? ownId;
  const anchor = useRef<HTMLSpanElement>(null);
  const bubble = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);
  const open = hovered || focused || pinned;

  // 只在打开时定位，关上时由清理函数复位：设置页一次挂上几百个「?」，挂载时都不应多一次状态更新。
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const target = anchor.current?.getBoundingClientRect();
      const box = bubble.current?.getBoundingClientRect();
      if (!target || !box) return;
      // 左缘略早于「?」，读起来像从它下面展开；放不下时贴着窗口右缘。
      const left = Math.min(target.left - 10, window.innerWidth - box.width - EDGE);
      const below = target.bottom + GAP;
      const top =
        below + box.height > window.innerHeight - EDGE && target.top - GAP - box.height >= EDGE
          ? target.top - GAP - box.height
          : below;
      setPosition({ top, left: Math.max(EDGE, left) });
    };
    place();
    return () => setPosition(null);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    // 位置只在打开时量一次，滚动或缩放后气泡会脱离它的行，所以连同键盘聚焦打开的那种一起收起；再次 Tab 到「?」会重新打开并重新定位。
    const close = () => {
      setHovered(false);
      setPinned(false);
      setFocused(false);
    };
    const outside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (anchor.current?.contains(target) || bubble.current?.contains(target)) return;
      close();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    document.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  const hover = {
    onPointerEnter: (event: ReactPointerEvent) => {
      if (event.pointerType === "mouse") setHovered(true);
    },
    onPointerLeave: (event: ReactPointerEvent) => {
      if (event.pointerType === "mouse") setHovered(false);
    },
  };
  const glyph = <span className={style.infoTipGlyph} aria-hidden="true" />;
  return (
    <span ref={anchor} className={className ? `${style.infoTip} ${className}` : style.infoTip}>
      {passive ? (
        <span className={style.infoTipPassive} {...hover}>
          {glyph}
        </span>
      ) : (
        <button
          type="button"
          // 设置搜索定位到一行时跳过它，聚焦行里真正的控件。
          data-info-tip=""
          className={style.infoTipButton}
          aria-label={label}
          aria-describedby={textId}
          aria-expanded={pinned}
          {...hover}
          onFocus={(event) => setFocused(event.currentTarget.matches(":focus-visible"))}
          onBlur={() => setFocused(false)}
          onClick={(event) => {
            // 行本身可能响应点击（例如整行可点的开关行），「?」只管说明。
            event.stopPropagation();
            event.preventDefault();
            setPinned((current) => !current);
          }}
        >
          {glyph}
        </button>
      )}
      <span id={textId} className="sr-only">
        {children}
      </span>
      {open &&
        createPortal(
          <div
            ref={bubble}
            className={style.infoTipBubble}
            aria-hidden="true"
            style={{
              position: "fixed",
              top: position?.top ?? 0,
              left: position?.left ?? 0,
              visibility: position ? "visible" : "hidden",
            }}
          >
            {children}
          </div>,
          anchor.current?.closest("[data-settings-shell]") ?? document.body,
        )}
    </span>
  );
}
