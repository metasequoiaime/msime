import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  EmojiPanel,
  StatusMessage,
  type ClipboardCaptureStatus,
  type EmojiPanelClient,
} from "@msime/ui";

/** 独立窗口和设置页内的表情、剪贴板面板共用桌面粘贴能力与关闭装配。 */
export function DesktopEmojiPanel({
  theme,
  initialPage = "home",
  client: providedClient,
  close,
  hostPlatform,
}: {
  theme: "dark" | "light";
  initialPage?: "home" | "clipboard";
  client: EmojiPanelClient;
  close?: () => Promise<void>;
  /** 面板窗口所在的宿主平台。Linux 上插入成功后关闭面板、独立剪贴板窗口用自己的页头并报告采集状态；Linux 和 macOS 的剪贴板行可以发到云剪贴板（macOS 原生剪贴板行本来就有这一项）。 */
  hostPlatform?: string;
}) {
  const [emojiClient, setEmojiClient] = useState<EmojiPanelClient | null>(null);
  const panelLabel = initialPage === "clipboard" ? "clipboard-panel" : "emoji-panel";
  const linux = hostPlatform === "linux";
  useEffect(() => {
    let active = true;
    const capability = isTauri()
      ? invoke<boolean>("supports_clipboard_paste").catch(() => false)
      : Promise.resolve(false);
    void capability.then((supported) => {
      if (!active) return;
      const baseClient = providedClient;
      const closePanel = close ?? (() => invoke("close_panel", { label: panelLabel }));
      const clipboard = baseClient.clipboard
        ? {
            ...baseClient.clipboard,
            ...(supported
              ? { paste: (text: string) => invoke<void>("paste_clipboard_text", { text }) }
              : {}),
            ...(hostPlatform === "linux" || hostPlatform === "macos"
              ? {
                  sendToCloud: (text: string) =>
                    invoke("cloud_clipboard_request", {
                      action: { operation: "add", text },
                    }).then(() => undefined),
                }
              : {}),
            ...(linux
              ? {
                  captureStatus: () =>
                    invoke<ClipboardCaptureStatus | null>("clipboard_capture_status"),
                }
              : {}),
          }
        : undefined;
      setEmojiClient({
        ...baseClient,
        // 内嵌页面和独立剪贴板没有表情窗口的几何权限，不把操作转发给它们所在的窗口。
        ...(close || panelLabel !== "emoji-panel"
          ? { beginWindowDrag: undefined, resizeWindow: undefined }
          : {}),
        close: closePanel,
        clipboard,
        // 设置页里内嵌的面板（`close` 由设置页提供）不是独立窗口，插入后不关闭。
        ...(linux && !close ? { closeAfterInsert: true } : {}),
      });
    });
    return () => {
      active = false;
    };
  }, [close, hostPlatform, linux, panelLabel, providedClient]);
  return emojiClient ? (
    <EmojiPanel
      client={emojiClient}
      theme={theme}
      initialPage={initialPage}
      standaloneClipboard={linux && !close && initialPage === "clipboard"}
    />
  ) : (
    <StatusMessage role="status">正在连接面板…</StatusMessage>
  );
}
