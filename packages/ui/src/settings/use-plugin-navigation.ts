import { useEffect, useRef, useState } from "react";
import type { PluginKind } from "./plugin-types";

/** 「我的插件」的视图只保存类型和 id，由入口从最新目录查找，包消失时回退到列表。 */
export type PluginView =
  | { kind: "list" }
  | { kind: "pack"; packKind: PluginKind; id: string }
  | { kind: "missing"; packKind: PluginKind; id: string }
  | { kind: "sound-effects" }
  | { kind: "mentions" };

const listView: PluginView = { kind: "list" };

/** 从列表行键取得包类型；声音与效果、名单入口没有包类型。 */
function rowKind(rowKey: string): string | null {
  return /^(?:pack|missing)\/([^/]+)\//.exec(rowKey)?.[1] ?? null;
}

/** 管理插件子视图和渲染后的焦点恢复，失活时只关闭视图。 */
export function usePluginNavigation(active: boolean) {
  const [view, setView] = useState<PluginView>(listView);
  const root = useRef<HTMLDivElement>(null);
  // 打开后聚焦返回按钮，返回后聚焦原入口；等渲染提交后再访问节点。
  const pendingFocus = useRef<string | null>(null);
  const openedFrom = useRef<string | null>(null);

  // 离开页面或切到社区插件时关闭详情，下次访问从列表开始。
  useEffect(() => {
    if (active) return;
    pendingFocus.current = null;
    setView(listView);
  }, [active]);

  useEffect(() => {
    const target = pendingFocus.current;
    if (target === null) return;
    pendingFocus.current = null;
    if (target === "back") {
      root.current?.querySelector<HTMLElement>("[data-plugin-back]")?.focus();
      return;
    }
    const rows = Array.from(root.current?.querySelectorAll<HTMLElement>("[data-plugin-row]") ?? []);
    // 原入口被删除或缺失项已恢复时，先找同类型首行，再找列表首行，避免焦点落到页面正文。
    const kind = rowKind(target);
    const element =
      rows.find((row) => row.dataset.pluginRow === target) ??
      (kind ? rows.find((row) => rowKind(row.dataset.pluginRow ?? "") === kind) : undefined) ??
      rows[0];
    element?.focus();
  }, [view]);

  const openView = (next: PluginView, rowKey: string) => {
    openedFrom.current = rowKey;
    pendingFocus.current = "back";
    setView(next);
  };
  const backToList = () => {
    pendingFocus.current = openedFrom.current;
    setView(listView);
  };

  return { root, view, openView, backToList };
}
