import { useEffect, useRef, useState } from "react";
import {
  cloudClipboardRelayTokenIssue,
  cloudClipboardRelayUrlIssue,
  cloudClipboardWebDavPasswordIssue,
  cloudClipboardWebDavUrlIssue,
  cloudClipboardWebDavUsernameIssue,
  type CloudClipboardRelayPreferences,
  type CloudClipboardWebDavPreferences,
} from "./cloud-clipboard-relay";
import { errorCode } from "../core/error-code";

export type CloudClipboardSyncMode = "account" | "relay" | "webdav";

/** 「测试连接」的状态：没测过、正在测、通过，或者失败时给人看的一句话。 */
export type CloudClipboardWebDavProbeState =
  | { kind: "idle" }
  | { kind: "probing" }
  | { kind: "passed" }
  | { kind: "failed"; message: string };

export interface UseCloudClipboardRelayOptions {
  /** 已保存的中继配置；没有配置时为 `undefined`。 */
  relay: CloudClipboardRelayPreferences | undefined;
  /** 已保存的 WebDAV 网盘配置；没有配置时为 `undefined`。 */
  webdav?: CloudClipboardWebDavPreferences | undefined;
  /** 宿主的「测试连接」（`cloud_clipboard_webdav_probe`）：通过时 resolve，失败时以带 `code` 的错误 reject。没有它时不提供 WebDAV。 */
  probeWebDav?: (draft: CloudClipboardWebDavPreferences) => Promise<void>;
  disabled?: boolean;
  /** 只有显式保存或改回账号才写入，表单草稿不参与自动保存。 */
  onChange: (relay: CloudClipboardRelayPreferences | undefined) => void;
  /** 保存 WebDAV 网盘（同时去掉中继）。 */
  onWebDavChange?: (webdav: CloudClipboardWebDavPreferences) => void;
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
  /** 宿主能不能用 WebDAV 网盘。 */
  webdavAvailable: boolean;
  webdavConfigured: boolean;
  savedWebDavUrl: string;
  webdavUrl: string;
  webdavUsername: string;
  /** 仅包含本次输入，不回填已保存的应用密码。 */
  webdavPassword: string;
  hasSavedWebDavPassword: boolean;
  webdavUrlIssue: string;
  webdavUsernameIssue: string;
  webdavPasswordIssue: string;
  webdavProbe: CloudClipboardWebDavProbeState;
  canProbeWebDav: boolean;
  canSaveWebDav: boolean;
  editWebDavUrl: (url: string) => void;
  editWebDavUsername: (username: string) => void;
  editWebDavPassword: (password: string) => void;
  probeWebDav: () => Promise<void>;
  saveWebDav: () => Promise<void>;
}

/** 「测试连接」失败时给人看的一句话。错误码见 `crates/host-api/include/msime_client.h` 的 `msime_client_cloud_clipboard_webdav_probe`。 */
export function cloudClipboardWebDavProbeMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "relay_not_configured":
      return "地址、账号或应用密码不完整。";
    case "webdav_unsupported":
      return "这个网盘不支持云剪贴板需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。";
    case "relay_unauthorized":
      return "网盘拒绝了这个账号或应用密码。";
    case "relay_unavailable":
      return "连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。";
    case "webdav_rate_limited":
      return "网盘暂时限制了请求次数（坚果云免费账号每 30 分钟 600 次，与其他应用共用），请稍后再试。";
  }
  return "测试连接失败，请稍后再试。";
}

function initialMode(relayUrl: string, webdavUrl: string): CloudClipboardSyncMode {
  if (webdavUrl) return "webdav";
  if (relayUrl) return "relay";
  return "account";
}

/** 云剪贴板同步方式的草稿、校验与显式保存：中继留空令牌沿用已保存的令牌，WebDAV 留空应用密码沿用已保存的密码；WebDAV 保存前先「测试连接」，不通过不保存。外部配置变化时重置草稿。 */
export function useCloudClipboardRelay({
  relay,
  webdav,
  probeWebDav,
  disabled = false,
  onChange,
  onWebDavChange,
}: UseCloudClipboardRelayOptions): CloudClipboardRelayState {
  const savedUrl = relay?.url ?? "";
  const savedToken = relay?.token ?? "";
  const savedWebDavUrl = webdav?.url ?? "";
  const savedWebDavUsername = webdav?.username ?? "";
  const savedWebDavPassword = webdav?.password ?? "";
  const webdavAvailable = Boolean(probeWebDav && onWebDavChange);
  const configured = savedUrl !== "";
  const webdavConfigured = savedWebDavUrl !== "";
  const [mode, setMode] = useState<CloudClipboardSyncMode>(initialMode(savedUrl, savedWebDavUrl));
  const [url, editUrl] = useState(savedUrl);
  // 已保存令牌只参与保存组装，不进入表单草稿或返回状态。
  const [token, editToken] = useState("");
  const [webdavUrl, setWebDavUrl] = useState(savedWebDavUrl);
  const [webdavUsername, setWebDavUsername] = useState(savedWebDavUsername);
  const [webdavPassword, setWebDavPassword] = useState("");
  const [webdavProbe, setWebDavProbe] = useState<CloudClipboardWebDavProbeState>({
    kind: "idle",
  });
  // 草稿每改一次加一，旧的「测试连接」结果回来时据此丢弃，不让它说中新的草稿。
  const draftGeneration = useRef(0);

  useEffect(() => {
    setMode(initialMode(savedUrl, savedWebDavUrl));
    editUrl(savedUrl);
    editToken("");
    setWebDavUrl(savedWebDavUrl);
    setWebDavUsername(savedWebDavUsername);
    setWebDavPassword("");
    setWebDavProbe({ kind: "idle" });
    draftGeneration.current += 1;
  }, [savedUrl, savedToken, savedWebDavUrl, savedWebDavUsername, savedWebDavPassword]);

  const trimmedUrl = url.trim();
  const trimmedToken = token.trim();
  const urlIssue = cloudClipboardRelayUrlIssue(trimmedUrl);
  const tokenIssue = trimmedToken || !savedToken ? cloudClipboardRelayTokenIssue(trimmedToken) : "";
  const changed =
    trimmedUrl !== savedUrl ||
    (trimmedToken !== "" && trimmedToken !== savedToken) ||
    webdavConfigured;
  const canSave = !disabled && mode === "relay" && changed && !urlIssue && !tokenIssue;
  const canSwitchToAccount = !disabled && mode === "account" && (configured || webdavConfigured);

  const trimmedWebDavUrl = webdavUrl.trim();
  const trimmedWebDavUsername = webdavUsername.trim();
  // 应用密码原样使用，不去掉首尾空白：有的网盘允许它们出现在密码里。
  const effectivePassword = webdavPassword !== "" ? webdavPassword : savedWebDavPassword;
  const webdavUrlIssue = cloudClipboardWebDavUrlIssue(trimmedWebDavUrl);
  const webdavUsernameIssue = cloudClipboardWebDavUsernameIssue(trimmedWebDavUsername);
  const webdavPasswordIssue =
    webdavPassword || !savedWebDavPassword ? cloudClipboardWebDavPasswordIssue(webdavPassword) : "";
  const webdavDraft: CloudClipboardWebDavPreferences = {
    url: trimmedWebDavUrl,
    username: trimmedWebDavUsername,
    password: effectivePassword,
  };
  const webdavValid = !webdavUrlIssue && !webdavUsernameIssue && !webdavPasswordIssue;
  const webdavChanged =
    trimmedWebDavUrl !== savedWebDavUrl ||
    trimmedWebDavUsername !== savedWebDavUsername ||
    effectivePassword !== savedWebDavPassword ||
    configured;
  const probing = webdavProbe.kind === "probing";
  const canProbeWebDav =
    webdavAvailable && !disabled && mode === "webdav" && webdavValid && !probing;
  const canSaveWebDav = canProbeWebDav && webdavChanged;

  const edit =
    <T>(set: (value: T) => void) =>
    (value: T) => {
      draftGeneration.current += 1;
      setWebDavProbe({ kind: "idle" });
      set(value);
    };

  /** 跑一次「测试连接」；返回是否通过，草稿在等待期间改过时返回 `false` 且不显示结果。 */
  const runProbe = async (): Promise<boolean> => {
    if (!probeWebDav) return false;
    const generation = draftGeneration.current;
    setWebDavProbe({ kind: "probing" });
    try {
      await probeWebDav(webdavDraft);
    } catch (error) {
      if (generation === draftGeneration.current) {
        setWebDavProbe({ kind: "failed", message: cloudClipboardWebDavProbeMessage(error) });
      }
      return false;
    }
    if (generation !== draftGeneration.current) return false;
    setWebDavProbe({ kind: "passed" });
    return true;
  };

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
    webdavAvailable,
    webdavConfigured,
    savedWebDavUrl,
    webdavUrl,
    webdavUsername,
    webdavPassword,
    hasSavedWebDavPassword: Boolean(savedWebDavPassword),
    webdavUrlIssue,
    webdavUsernameIssue,
    webdavPasswordIssue,
    webdavProbe,
    canProbeWebDav,
    canSaveWebDav,
    editWebDavUrl: edit(setWebDavUrl),
    editWebDavUsername: edit(setWebDavUsername),
    editWebDavPassword: edit(setWebDavPassword),
    probeWebDav: async () => {
      if (canProbeWebDav) await runProbe();
    },
    saveWebDav: async () => {
      if (!canSaveWebDav || !onWebDavChange) return;
      const draft = webdavDraft;
      // 不通过「测试连接」就不保存：网盘不支持独占创建时，几台设备会把日志写坏。
      if (await runProbe()) onWebDavChange(draft);
    },
  };
}
