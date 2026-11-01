import { focusAndRevealNearest } from "../core/focus-and-reveal-nearest";
import type { SettingsPageId } from "./settings-page-registry";
import { settingsSearchRevealEvent } from "./settings-search-reveal";

export interface SettingsSearchTarget {
  page: SettingsPageId;
  title: string;
  path: string[];
  element: HTMLElement;
}

/** 只读显式行标题，不扫描表单值、说明、历史或诊断内容。 */
export function collectSettingsSearchTargets(
  root: HTMLElement,
  availablePages: ReadonlyMap<SettingsPageId, string>,
): SettingsSearchTarget[] {
  const targets: SettingsSearchTarget[] = [];
  for (const element of root.querySelectorAll<HTMLElement>("[data-settings-search-label]")) {
    const scope = element.closest<HTMLElement>("[data-settings-page]");
    const page = scope?.dataset.settingsPage as SettingsPageId | undefined;
    if (!page || !availablePages.has(page) || element.closest("[data-settings-search-ignore]"))
      continue;
    // 页面、标签和显式下钻容器可以由导航展开；其他 hidden 表示前置条件未满足。
    let unavailable = false;
    const path: string[] = [];
    for (
      let parent: HTMLElement | null = element;
      parent && root.contains(parent);
      parent = parent.parentElement
    ) {
      if (
        parent.hidden &&
        !parent.matches("[data-settings-page], [role=tabpanel], [data-settings-search-reveal]")
      ) {
        unavailable = true;
        break;
      }
      const groupTitle = parent.matches("section")
        ? parent
            .querySelector(
              ":scope > [data-group-title], :scope > [data-group-header] > [data-group-title]",
            )
            ?.textContent?.trim()
        : undefined;
      if (groupTitle) path.unshift(groupTitle);
      if (parent.getAttribute("role") === "tabpanel") {
        const tab = root.ownerDocument.getElementById(parent.getAttribute("aria-labelledby") ?? "");
        if (tab?.textContent) path.unshift(tab.textContent.trim());
      }
      if (parent.hasAttribute("data-settings-search-reveal") && parent.getAttribute("aria-label"))
        path.unshift(parent.getAttribute("aria-label")!);
    }
    if (unavailable) continue;
    const title = element.dataset.settingsSearchLabel?.trim();
    if (title) targets.push({ page, title, path: [availablePages.get(page)!, ...path], element });
  }
  return targets;
}

/** 页面提交后按由外到内的顺序展开，再把焦点交给目标控件。 */
export function revealSettingsSearchTarget(target: SettingsSearchTarget): void {
  const ancestors: HTMLElement[] = [];
  for (let node: HTMLElement | null = target.element; node; node = node.parentElement)
    ancestors.unshift(node);
  for (const node of ancestors) {
    if (node.matches("[data-settings-search-reveal]"))
      node.dispatchEvent(new Event(settingsSearchRevealEvent));
    if (node.getAttribute("role") === "tabpanel")
      node.ownerDocument.getElementById(node.getAttribute("aria-labelledby") ?? "")?.click();
    if (node instanceof HTMLDetailsElement) node.open = true;
  }
}

export function focusSettingsSearchTarget(target: SettingsSearchTarget): void {
  const row = target.element;
  if (!row.isConnected || row.closest("[hidden]")) return;
  const control = row.matches("button, input, select, textarea")
    ? row
    : row.querySelector<HTMLElement>(
        'input:not([type="hidden"]):not(:disabled), select:not(:disabled), textarea:not(:disabled), button:not(:disabled):not([data-info-tip]), [tabindex="0"]',
      );
  if (!control) row.tabIndex = -1;
  focusAndRevealNearest(control ?? row);
}
