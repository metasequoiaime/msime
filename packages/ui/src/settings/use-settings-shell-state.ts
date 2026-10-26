import { useEffect, useRef, useState } from "react";
import type { DesktopSkinTab } from "./desktop-skin-tabs";
import type { PluginKind } from "./plugin-types";
import type { SettingsPageId } from "./settings-page-registry";

export type SettingsShellView = "mine" | "community";

/** 让「我的插件」打开某个包的详情：社区装好脚本插件后用它跳到授权页。`serial` 每次请求加一，同一个包装两次也会再打开一次。 */
export type PluginOpenRequest = { kind: PluginKind; id: string; serial: number };

/** 设置外壳的临时视图、导航查询和对话页挂载状态。 */
export function useSettingsShellState(page: SettingsPageId) {
  const [pluginView, setPluginView] = useState<SettingsShellView>("mine");
  const [pluginOpenRequest, setPluginOpenRequest] = useState<PluginOpenRequest | null>(null);
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
    pluginOpenRequest,
    setPluginOpenRequest,
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
