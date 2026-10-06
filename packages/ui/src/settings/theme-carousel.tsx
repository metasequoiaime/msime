import {
  Children,
  isValidElement,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import * as settings from "./settings-style";
import { ActionButton } from "../core/action-button";

/** 导航最多显示的圆点数。每个已安装的皮肤都是一张卡片，主题多于这个数时，圆点改为跟随当前卡片的一段窗口，导航行的宽度不随皮肤数量增长。 */
export const MAX_CAROUSEL_DOTS = 9;

/** `index` 附近那段圆点窗口的第一张卡片，窗口始终落在卡片范围之内。 */
export function carouselDotWindowStart(
  index: number,
  count: number,
  size = MAX_CAROUSEL_DOTS,
): number {
  if (count <= size) return 0;
  return Math.min(Math.max(index - Math.floor(size / 2), 0), count - size);
}

export interface ThemeCarouselProps {
  /** One name per slide, in order; the dots are labelled with them. */
  labels: string[];
  /** The slide to bring into view when the page opens and whenever the selected theme changes. */
  selectedIndex: number;
  /** 每张卡片的 `key` 标识它是哪个主题：重扫皮肤目录增删了卡片时，轮播停在原来那张卡片上，只有选中的主题换了才跳过去。 */
  children: ReactNode;
}

/** Shows the theme cards one at a time, switched with the arrows, the dots, the arrow keys, or a horizontal swipe on the track. */
export function ThemeCarousel({ labels, selectedIndex, children }: ThemeCarouselProps) {
  const track = useRef<HTMLDivElement>(null);
  const slides = Children.toArray(children);
  const count = slides.length;
  const keys = slides.map((slide, at) => (isValidElement(slide) ? String(slide.key) : `#${at}`));
  const [index, setIndex] = useState(selectedIndex);
  // The resize handler re-snaps to this without re-subscribing on every slide change.
  const current = useRef(index);
  // 当前显示的卡片和最新的卡片列表；`show` 读它们，因此不必随卡片数量变化而重建。
  const shownKey = useRef<string | undefined>(keys[index]);
  const latestKeys = useRef(keys);
  latestKeys.current = keys;
  const windowStart = carouselDotWindowStart(index, labels.length);

  const show = useCallback((next: number, smooth: boolean) => {
    const clamped = Math.min(Math.max(next, 0), Math.max(latestKeys.current.length - 1, 0));
    current.current = clamped;
    shownKey.current = latestKeys.current[clamped];
    setIndex(clamped);
    const element = track.current;
    element?.scrollTo?.({
      left: clamped * element.clientWidth,
      behavior: smooth ? "smooth" : "auto",
    });
  }, []);

  // 只在选中的卡片换了时跟过去。重扫目录在前面插入一款皮肤会让选中卡片的序号变化，但它还是同一张，不应把用户正在看的卡片换掉。
  const selectedKey = keys[selectedIndex] ?? `#${selectedIndex}`;
  useEffect(() => show(selectedIndex, false), [selectedKey, show]);

  // 卡片增删之后留在原来那张卡片上；它被删掉了就停在原位置，越界时退到最后一张。
  const keySignature = keys.join("\n");
  // 在绘制前挪回去，免得有一帧显示成插进来的那张卡片。
  useLayoutEffect(() => {
    const at = shownKey.current === undefined ? -1 : latestKeys.current.indexOf(shownKey.current);
    show(at >= 0 ? at : current.current, false);
  }, [keySignature, show]);

  // The page mounts inside a hidden fieldset, where the track has no width; snapping again once it is laid out (and on every later resize) keeps the chosen card in view.
  useEffect(() => {
    const element = track.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => show(current.current, false));
    observer.observe(element);
    return () => observer.disconnect();
  }, [show]);

  function onScroll() {
    const element = track.current;
    if (!element || element.clientWidth === 0) return;
    const next = Math.round(element.scrollLeft / element.clientWidth);
    current.current = next;
    shownKey.current = latestKeys.current[next];
    setIndex(next);
  }

  function onKeyDown(event: import("react").KeyboardEvent<HTMLElement>) {
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      show(index + (event.key === "ArrowLeft" ? -1 : 1), true);
    }
  }

  return (
    <section aria-roledescription="carousel" aria-label="主题列表" onKeyDown={onKeyDown}>
      <div ref={track} className={settings.themeCarouselTrack} onScroll={onScroll}>
        {slides.map((child, slide) => (
          <div
            key={keys[slide]}
            className={settings.themeCarouselSlide}
            aria-roledescription="slide"
            aria-label={`${slide + 1} / ${count}`}
          >
            {child}
          </div>
        ))}
      </div>
      <div className={settings.themeCarouselNav}>
        <ActionButton
          action={() => show(index - 1, true)}
          className={settings.themeCarouselArrow}
          ariaLabel="上一个主题"
          disabled={index <= 0}
          label="‹"
        />
        <div className={settings.themeCarouselDots}>
          {labels.slice(windowStart, windowStart + MAX_CAROUSEL_DOTS).map((label, offset) => {
            const slide = windowStart + offset;
            // 窗口没有到达第一张或最后一张时，两端的圆点画小一号，提示外面还有主题。
            const edge =
              (offset === 0 && windowStart > 0) ||
              (offset === MAX_CAROUSEL_DOTS - 1 && windowStart + MAX_CAROUSEL_DOTS < labels.length);
            return (
              <ActionButton
                action={() => show(slide, true)}
                key={slide}
                className={settings.themeCarouselDot(slide === index, edge)}
                ariaLabel={`查看${label}`}
                ariaCurrent={slide === index ? "true" : undefined}
                label={null}
              />
            );
          })}
        </div>
        <span className={settings.themeCarouselCount}>
          {index + 1} / {count}
        </span>
        <ActionButton
          action={() => show(index + 1, true)}
          className={settings.themeCarouselArrow}
          ariaLabel="下一个主题"
          disabled={index >= count - 1}
          label="›"
        />
      </div>
    </section>
  );
}
