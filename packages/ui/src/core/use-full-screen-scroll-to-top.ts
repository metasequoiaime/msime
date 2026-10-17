import { useLayoutEffect, useRef, type RefObject } from "react";

/** 全屏设置子视图打开时把自己的滚动容器带回顶部，避免继承所在页面的滚动位置。 */
export function useFullScreenScrollToTop<T extends HTMLElement>(
  enabled = true,
): RefObject<T | null> {
  const surface = useRef<T>(null);
  useLayoutEffect(() => {
    if (enabled) surface.current?.scrollIntoView?.({ block: "start" });
  }, [enabled]);
  return surface;
}
