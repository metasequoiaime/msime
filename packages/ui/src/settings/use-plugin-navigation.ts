import { useEffect, useRef, useState } from "react";
import type { AuthoredKind, PluginKind } from "./plugin-types";
import { createMobileSettingsHistoryState } from "./mobile-history-state";
import { pushMobileSettingsState } from "./mobile-navigation";
import { useMobilePopState } from "./use-mobile-pop-state";

/** 「我的插件」的视图只保存类型和 id，由入口从最新目录查找，包消失时回退到列表。 */
export type PluginView =
  | { kind: "list" }
  | { kind: "pack"; packKind: PluginKind; id: string }
  | { kind: "missing"; packKind: PluginKind; id: string }
  | { kind: "sound-effects" }
  | { kind: "mentions" }
  /** 设置页插件编辑器：新建时先选类型。 */
  | { kind: "create" }
  /** 编辑一个已安装的用户插件包。 */
  | { kind: "edit"; packKind: AuthoredKind; id: string };

const listView: PluginView = { kind: "list" };

/** 从列表行键取得包类型；声音与效果、名单入口没有包类型。 */
function rowKind(rowKey: string): string | null {
  return /^(?:pack|missing)\/([^/]+)\//.exec(rowKey)?.[1] ?? null;
}

/** 历史记录里的子视图：只认得出形状的才算，其余（包括没有这个字段）都是列表。 */
function historyView(state: unknown): PluginView {
  const value =
    state && typeof state === "object" ? (state as Record<string, unknown>).pluginView : undefined;
  if (!value || typeof value !== "object") return listView;
  const view = value as Record<string, unknown>;
  switch (view.kind) {
    case "pack":
    case "missing":
    case "edit":
      return typeof view.packKind === "string" && typeof view.id === "string"
        ? (view as PluginView)
        : listView;
    case "create":
    case "sound-effects":
    case "mentions":
      return { kind: view.kind };
    default:
      return listView;
  }
}

/** 当前历史记录是不是插件页的子视图。 */
function inHistoryView(): boolean {
  return typeof window !== "undefined" && historyView(window.history.state).kind !== "list";
}

/**
 * 管理插件子视图和渲染后的焦点恢复，失活时只关闭视图。
 *
 * 触屏设置（`mobile`）里每打开一层子视图就推入一条历史记录，系统返回键和标题栏的返回都退回上一层，和触屏设置的其他子页一样；桌面只改状态。
 */
export function usePluginNavigation(active: boolean, mobile = false) {
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

  useMobilePopState(mobile, (event) => {
    const state = event.state as Record<string, unknown> | null;
    if (state?.page !== "plugins") return;
    const next = historyView(state);
    pendingFocus.current = next.kind === "list" ? openedFrom.current : "back";
    setView(next);
  });

  /** 打开一层子视图。`replace` 时顶替当前这层（新建的包保存后换成它的详情），不再多推一条。 */
  const openView = (next: PluginView, rowKey: string, replace = false) => {
    openedFrom.current = rowKey;
    pendingFocus.current = "back";
    if (mobile && typeof window !== "undefined") {
      const patch = { page: "plugins", pluginView: next };
      if (replace && inHistoryView()) {
        window.history.replaceState(
          createMobileSettingsHistoryState(window.history.state, patch),
          "",
        );
      } else {
        pushMobileSettingsState(patch);
      }
    }
    setView(next);
  };
  /** 退回上一层：触屏设置里子视图有自己的历史记录时交给返回，由 popstate 换视图；否则按 `fallback` 换。 */
  const goBack = (fallback: () => void) => {
    if (mobile && inHistoryView()) {
      window.history.back();
      return;
    }
    fallback();
  };
  const backToList = () =>
    goBack(() => {
      pendingFocus.current = openedFrom.current;
      setView(listView);
    });

  return { root, view, openView, goBack, backToList };
}
