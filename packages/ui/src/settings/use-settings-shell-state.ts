import { useEffect, useRef, useState } from "react";
import type { DesktopSkinTab } from "./desktop-skin-tabs";
import type { SettingsPageId } from "./settings-page-registry";

export type SettingsShellView = "mine" | "community";

/** 设置外壳的临时视图、导航查询和对话页挂载状态。 */
export function useSettingsShellState(page: SettingsPageId) {
  const [pluginView, setPluginView] = useState<SettingsShellView>("mine");
  const [skinView, setSkinView] = useState<DesktopSkinTab>("mine");
  const [navQuery, setNavQuery] = useState("");
  const [titleCollapsedOn, setTitleCollapsedOn] = useState<string | null>(null);
  const [chatMounted, setChatMounted] = useState(page === "chat");
  const [chatFocusRequest, setChatFocusRequest] = useState(0);
  useEffect(() => {
    if (page === "chat") setChatMounted(true);
  }, [page]);
  const pageColumnRef = useRef<HTMLDivElement>(null);
  const launchPage = useRef(page);

  return {
    pluginView,
    setPluginView,
    skinView,
    setSkinView,
    navQuery,
    setNavQuery,
    titleCollapsedOn,
    setTitleCollapsedOn,
    chatMounted,
    chatFocusRequest,
    setChatFocusRequest,
    pageColumnRef,
    launchPage,
  };
}
