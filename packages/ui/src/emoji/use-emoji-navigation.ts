import { isComposingKeyEvent } from "../core/ime-key-event";
import { focusAndRevealNearest } from "../core/focus-and-reveal-nearest";
import { useLayoutEffect, useRef, type KeyboardEvent } from "react";

// Use rendered positions so navigation follows both responsive grids and
// variable-width kaomoji rows, including movement between catalog groups.
export function useEmojiNavigation(onEscape: () => void, viewKey?: string) {
  const panelRef = useRef<HTMLElement>(null);

  useLayoutEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    for (const viewport of panel.querySelectorAll<HTMLElement>(".emoji-panel-content")) {
      viewport.scrollTop = 0;
      viewport.scrollLeft = 0;
    }
    // Keep native focus on search/category controls. If a retained item has
    // focus across a view change, reset item navigation to the new first row.
    const focused = panel.ownerDocument.activeElement;
    if (
      focused instanceof HTMLElement &&
      panel.contains(focused) &&
      focused.matches("[data-emoji-navigation-item]")
    ) {
      const first = panel.querySelector<HTMLButtonElement>(
        "[data-emoji-navigation-item]:not(:disabled)",
      );
      if (first) first.focus({ preventScroll: true });
      else
        panel
          .querySelector<HTMLInputElement>(".emoji-panel-search input")
          ?.focus({ preventScroll: true });
    }
  }, [viewKey]);

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || isComposingKeyEvent(event)) return;
    if (
      event.ctrlKey &&
      !event.altKey &&
      !event.metaKey &&
      !event.shiftKey &&
      event.key.toLowerCase() === "f"
    ) {
      const search = panelRef.current?.querySelector<HTMLInputElement>(
        ".emoji-panel-search input:not(:disabled)",
      );
      if (search) {
        event.preventDefault();
        focusAndRevealNearest(search);
        search.select();
      }
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onEscape();
      return;
    }
    const target = event.target as HTMLElement;
    const fromSearch = target.matches(".emoji-panel-search input");
    const current = target.closest<HTMLButtonElement>("[data-emoji-navigation-item]");
    if ((!current && !fromSearch) || (fromSearch && event.key !== "ArrowDown")) return;
    const keys = [
      "ArrowLeft",
      "ArrowRight",
      "ArrowUp",
      "ArrowDown",
      "Home",
      "End",
      "PageUp",
      "PageDown",
    ];
    if (!keys.includes(event.key)) return;
    const items = Array.from(
      panelRef.current?.querySelectorAll<HTMLButtonElement>(
        "[data-emoji-navigation-item]:not(:disabled)",
      ) ?? [],
    );
    if (!items.length) return;
    event.preventDefault();
    const index = current ? items.indexOf(current) : -1;
    let next = Math.max(0, index);
    if (index < 0 || event.key === "Home") next = 0;
    else if (event.key === "End") next = items.length - 1;
    else if (event.key === "ArrowLeft") next = Math.max(0, index - 1);
    else if (event.key === "ArrowRight") next = Math.min(items.length - 1, index + 1);
    else {
      const origin = items[index].getBoundingClientRect();
      const originX = origin.left + origin.width / 2;
      const direction = event.key === "ArrowUp" || event.key === "PageUp" ? -1 : 1;
      const pageStep = event.key === "PageUp" || event.key === "PageDown";
      const viewport = items[index].closest(".emoji-panel-content");
      const destinationY =
        origin.top + (pageStep ? direction * (viewport?.clientHeight || origin.height) : 0);
      let bestVertical = Infinity;
      let bestHorizontal = Infinity;
      for (let candidate = 0; candidate < items.length; candidate++) {
        const rect = items[candidate].getBoundingClientRect();
        if ((rect.top - origin.top) * direction <= 1) continue;
        const vertical = Math.abs(rect.top - destinationY);
        const horizontal = Math.abs(rect.left + rect.width / 2 - originX);
        if (
          vertical < bestVertical - 1 ||
          (Math.abs(vertical - bestVertical) <= 1 && horizontal < bestHorizontal)
        ) {
          next = candidate;
          bestVertical = vertical;
          bestHorizontal = horizontal;
        }
      }
      // Windows clamps ordinary grids to their first/last item at the edge;
      // flow-layout kaomoji keep their geometric vertical-navigation behavior.
      if (bestVertical === Infinity && (pageStep || !items[index].closest(".emoji-panel-flow"))) {
        next = direction < 0 ? 0 : items.length - 1;
      }
    }
    focusAndRevealNearest(items[next]);
  }

  return { ref: panelRef, onKeyDown };
}
