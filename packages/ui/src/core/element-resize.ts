/** 观察元素尺寸并返回清理函数；没有观察 API 时由调用方选择回退。 */
export function subscribeToElementResize(
  element: Element,
  listener: ResizeObserverCallback,
): (() => void) | undefined {
  if (typeof ResizeObserver === "undefined") return;
  const observer = new ResizeObserver(listener);
  observer.observe(element);
  return () => observer.disconnect();
}
