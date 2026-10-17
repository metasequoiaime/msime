/** 在不移动页面的情况下聚焦元素，再让它在最近的滚动容器中保持可见。 */
export function focusAndRevealNearest(element: HTMLElement | null | undefined): void {
  element?.focus({ preventScroll: true });
  element?.scrollIntoView({ block: "nearest", inline: "nearest" });
}
