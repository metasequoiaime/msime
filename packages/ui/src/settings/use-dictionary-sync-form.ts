import { useEffect, useRef, useState } from "react";
import {
  cloudClipboardWebDavPasswordIssue,
  cloudClipboardWebDavUrlIssue,
  cloudClipboardWebDavUsernameIssue,
} from "./cloud-clipboard-relay";
import {
  dictionarySyncErrorMessage,
  type DictionarySyncWebDavPreferences,
} from "./dictionary-sync";
import type { CloudClipboardWebDavProbeState } from "./use-cloud-clipboard-relay";

export interface UseDictionarySyncFormOptions {
  /** 已保存的网盘配置；没有配置时为 `undefined`。 */
  saved: DictionarySyncWebDavPreferences | undefined;
  /** 宿主的「测试连接」：通过时 resolve，失败时以带 `code` 的错误 reject。 */
  probe: (draft: DictionarySyncWebDavPreferences) => Promise<void>;
  disabled?: boolean;
  /** 只有通过「测试连接」的显式保存或「停止同步」才写入，草稿不参与自动保存。 */
  onChange: (webdav: DictionarySyncWebDavPreferences | undefined) => void;
}

/** 词库同步的网盘草稿、校验与保存：应用密码留空沿用已保存的，不回填到输入框；保存前先「测试连接」，不通过不保存、草稿留着。外部配置变化时重置草稿。规则与云剪贴板的 WebDAV 网盘相同。 */
export function useDictionarySyncForm({
  saved,
  probe,
  disabled = false,
  onChange,
}: UseDictionarySyncFormOptions) {
  const savedUrl = saved?.url ?? "";
  const savedUsername = saved?.username ?? "";
  const savedPassword = saved?.password ?? "";
  const configured = savedUrl !== "";
  const [url, setUrl] = useState(savedUrl);
  const [username, setUsername] = useState(savedUsername);
  // 已保存的应用密码只参与保存组装，不进入表单草稿或返回状态。
  const [password, setPassword] = useState("");
  const [probeState, setProbeState] = useState<CloudClipboardWebDavProbeState>({ kind: "idle" });
  // 草稿每改一次加一，旧的「测试连接」结果回来时据此丢弃。
  const draftGeneration = useRef(0);

  useEffect(() => {
    setUrl(savedUrl);
    setUsername(savedUsername);
    setPassword("");
    setProbeState({ kind: "idle" });
    draftGeneration.current += 1;
  }, [savedUrl, savedUsername, savedPassword]);

  const trimmedUrl = url.trim();
  const trimmedUsername = username.trim();
  // 应用密码原样使用，不去掉首尾空白：有的网盘允许它们出现在密码里。
  const effectivePassword = password !== "" ? password : savedPassword;
  const urlIssue = cloudClipboardWebDavUrlIssue(trimmedUrl);
  const usernameIssue = cloudClipboardWebDavUsernameIssue(trimmedUsername);
  const passwordIssue =
    password || !savedPassword ? cloudClipboardWebDavPasswordIssue(password) : "";
  const draft: DictionarySyncWebDavPreferences = {
    url: trimmedUrl,
    username: trimmedUsername,
    password: effectivePassword,
  };
  const valid = !urlIssue && !usernameIssue && !passwordIssue;
  const changed =
    trimmedUrl !== savedUrl ||
    trimmedUsername !== savedUsername ||
    effectivePassword !== savedPassword;
  const probing = probeState.kind === "probing";
  const canProbe = !disabled && valid && !probing;
  const canSave = canProbe && changed;

  const edit = (set: (value: string) => void) => (value: string) => {
    draftGeneration.current += 1;
    setProbeState({ kind: "idle" });
    set(value);
  };

  /** 跑一次「测试连接」；返回是否通过，草稿在等待期间改过时返回 `false` 且不显示结果。 */
  const runProbe = async (): Promise<boolean> => {
    const generation = draftGeneration.current;
    setProbeState({ kind: "probing" });
    try {
      await probe(draft);
    } catch (error) {
      if (generation === draftGeneration.current) {
        setProbeState({ kind: "failed", message: dictionarySyncErrorMessage(error, true) });
      }
      return false;
    }
    if (generation !== draftGeneration.current) return false;
    setProbeState({ kind: "passed" });
    return true;
  };

  return {
    configured,
    savedUrl,
    url,
    username,
    password,
    hasSavedPassword: Boolean(savedPassword),
    urlIssue,
    usernameIssue,
    passwordIssue,
    probeState,
    canProbe,
    canSave,
    canStop: !disabled && configured,
    editUrl: edit(setUrl),
    editUsername: edit(setUsername),
    editPassword: edit(setPassword),
    probe: async () => {
      if (canProbe) await runProbe();
    },
    save: async () => {
      if (!canSave) return;
      const next = draft;
      // 不通过「测试连接」就不保存：网盘不支持独占创建时，几台设备会把日志写坏。
      if (await runProbe()) onChange(next);
    },
    stop: () => {
      if (configured && !disabled) onChange(undefined);
    },
  };
}
