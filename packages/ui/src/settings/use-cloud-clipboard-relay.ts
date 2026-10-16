import { useEffect, useState } from "react";
import {
  cloudClipboardRelayTokenIssue,
  cloudClipboardRelayUrlIssue,
  type CloudClipboardRelayPreferences,
} from "./cloud-clipboard-relay";

export type CloudClipboardSyncMode = "account" | "relay";

export interface UseCloudClipboardRelayOptions {
  /** 已保存的中继配置；没有配置时为 `undefined`。 */
  relay: CloudClipboardRelayPreferences | undefined;
  disabled?: boolean;
  /** 只有显式保存或改回账号才写入，表单草稿不参与自动保存。 */
  onChange: (relay: CloudClipboardRelayPreferences | undefined) => void;
}

export interface CloudClipboardRelayState {
  mode: CloudClipboardSyncMode;
  url: string;
  /** 仅包含本次输入，不回填已保存令牌。 */
  token: string;
  savedUrl: string;
  hasSavedToken: boolean;
  configured: boolean;
  urlIssue: string;
  tokenIssue: string;
  canSave: boolean;
  canSwitchToAccount: boolean;
  setMode: (mode: CloudClipboardSyncMode) => void;
  editUrl: (url: string) => void;
  editToken: (token: string) => void;
  save: () => void;
  switchToAccount: () => void;
}

/** 中继表单的草稿、校验与显式保存；留空沿用已保存令牌，外部配置变化时重置草稿。 */
export function useCloudClipboardRelay({
  relay,
  disabled = false,
  onChange,
}: UseCloudClipboardRelayOptions): CloudClipboardRelayState {
  const savedUrl = relay?.url ?? "";
  const savedToken = relay?.token ?? "";
  const configured = savedUrl !== "";
  const [mode, setMode] = useState<CloudClipboardSyncMode>(configured ? "relay" : "account");
  const [url, editUrl] = useState(savedUrl);
  // 已保存令牌只参与保存组装，不进入表单草稿或返回状态。
  const [token, editToken] = useState("");

  useEffect(() => {
    setMode(savedUrl ? "relay" : "account");
    editUrl(savedUrl);
    editToken("");
  }, [savedUrl, savedToken]);

  const trimmedUrl = url.trim();
  const trimmedToken = token.trim();
  const urlIssue = cloudClipboardRelayUrlIssue(trimmedUrl);
  const tokenIssue = trimmedToken || !savedToken ? cloudClipboardRelayTokenIssue(trimmedToken) : "";
  const changed = trimmedUrl !== savedUrl || (trimmedToken !== "" && trimmedToken !== savedToken);
  const canSave = !disabled && mode === "relay" && changed && !urlIssue && !tokenIssue;
  const canSwitchToAccount = !disabled && mode === "account" && configured;

  return {
    mode,
    url,
    token,
    savedUrl,
    hasSavedToken: Boolean(savedToken),
    configured,
    urlIssue,
    tokenIssue,
    canSave,
    canSwitchToAccount,
    setMode,
    editUrl,
    editToken,
    save: () => {
      if (canSave) onChange({ url: trimmedUrl, token: trimmedToken || savedToken });
    },
    switchToAccount: () => {
      if (canSwitchToAccount) onChange(undefined);
    },
  };
}
